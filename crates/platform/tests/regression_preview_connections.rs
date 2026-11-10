use craft_anything_platform::serve::serve_dir;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

#[test]
fn regression_preview_limits_connections_and_releases_listener() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("index.html"), "ok").unwrap();
    let srv = serve_dir(dir.path()).unwrap();
    let held: Vec<_> = (0..64).map(|_| TcpStream::connect(("127.0.0.1", srv.port)).unwrap()).collect();
    std::thread::sleep(Duration::from_millis(250));
    let mut s = TcpStream::connect(("127.0.0.1", srv.port)).unwrap();
    s.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
    s.write_all(format!("GET / HTTP/1.1\r\nHost: 127.0.0.1:{}\r\n\r\n", srv.port).as_bytes()).unwrap();
    let mut out = String::new();
    s.read_to_string(&mut out).unwrap();
    assert!(out.starts_with("HTTP/1.1 503"), "{out}");
    // Keep the first rejected client's write side open and exceed the refusal queue. The read
    // timeout is set now: once the server has gone, macOS refuses socket options on a connection
    // it reset, with `Invalid argument`.
    let excess: Vec<_> = (0..16)
        .map(|_| {
            let stream = TcpStream::connect(("127.0.0.1", srv.port)).unwrap();
            stream.set_read_timeout(Some(Duration::from_secs(1))).unwrap();
            stream
        })
        .collect();
    let port = srv.port;
    let stopping = std::time::Instant::now();
    drop(srv);
    assert!(stopping.elapsed() < Duration::from_secs(2), "refusal cleanup must not grow with the number of excess clients");
    assert!(TcpStream::connect(("127.0.0.1", port)).is_err(), "the listener is gone when drop returns");
    for stream in excess {
        let mut response = Vec::new();
        if let Err(error) = (&stream).take(1024).read_to_end(&mut response) {
            // Closing the listener may reset connections still in its accept backlog.
            assert_eq!(error.kind(), std::io::ErrorKind::ConnectionReset, "excess connection did not close: {error}");
        }
        assert!(response.len() < 1024, "excess response reached the read limit before closure");
        assert!(
            response.is_empty() || response.as_slice() == b"HTTP/1.1 503 Service Unavailable\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
            "unexpected excess-connection response: {response:?}"
        );
    }
    drop(held);
}

#[test]
fn regression_preview_recovers_after_refusal_queue_overflow() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("index.html"), "recovered-preview").unwrap();
    let srv = serve_dir(dir.path()).unwrap();
    let held: Vec<_> = (0..64).map(|_| TcpStream::connect(("127.0.0.1", srv.port)).unwrap()).collect();
    std::thread::sleep(Duration::from_millis(250));
    let mut rejected = TcpStream::connect(("127.0.0.1", srv.port)).unwrap();
    rejected.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
    rejected.write_all(b"GET / HTTP/1.1\r\nHost: localhost\r\n\r\n").unwrap();
    let mut response = String::new();
    rejected.read_to_string(&mut response).unwrap();
    assert!(response.starts_with("HTTP/1.1 503"), "{response:?}");
    let excess: Vec<_> = (0..16).map(|_| TcpStream::connect(("127.0.0.1", srv.port)).unwrap()).collect();
    drop(excess);
    drop(rejected);
    // EOF closes the stream before the worker's slot guard necessarily drops.
    for mut stream in held {
        stream.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
        stream.shutdown(std::net::Shutdown::Write).unwrap();
        stream.read_to_end(&mut Vec::new()).unwrap();
    }
    for attempt in 0..20 {
        let mut normal = TcpStream::connect(("127.0.0.1", srv.port)).unwrap();
        normal.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
        normal.write_all(b"GET / HTTP/1.1\r\nHost: localhost\r\n\r\n").unwrap();
        response.clear();
        normal.read_to_string(&mut response).unwrap();
        if !response.starts_with("HTTP/1.1 503 ") || attempt == 19 {
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    assert!(response.starts_with("HTTP/1.1 200 ") && response.ends_with("recovered-preview"), "{response:?}");
}

#[test]
fn regression_preview_bounds_trickled_request_reads() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::Instant;

    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("index.html"), "normal-preview").unwrap();
    let srv = serve_dir(dir.path()).unwrap();
    let mut stream = TcpStream::connect(("127.0.0.1", srv.port)).unwrap();
    stream.set_read_timeout(Some(Duration::from_secs(7))).unwrap();
    stream.write_all(b"GET / HTTP/1.1\r\nX-Slow: ").unwrap();
    let mut writer = stream.try_clone().unwrap();
    let stop = Arc::new(AtomicBool::new(false));
    let done = stop.clone();
    let started = Instant::now();
    let trickle = std::thread::spawn(move || {
        let until = Instant::now() + Duration::from_secs(8);
        while !done.load(Ordering::SeqCst) && Instant::now() < until {
            if writer.write_all(b".").is_err() {
                break;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    });
    let mut response = String::new();
    let result = stream.read_to_string(&mut response);
    stop.store(true, Ordering::SeqCst);
    trickle.join().unwrap();
    result.expect("trickled bytes must not retain the connection beyond the whole-request deadline");
    assert!(started.elapsed() < Duration::from_secs(7));
    assert!(response.is_empty(), "a timed-out incomplete request must not serve the file: {response:?}");

    let mut normal = TcpStream::connect(("127.0.0.1", srv.port)).unwrap();
    normal.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
    normal.write_all(b"GET / HTTP/1.1\r\nHost: localhost\r\n\r\n").unwrap();
    response.clear();
    normal.read_to_string(&mut response).unwrap();
    assert!(response.starts_with("HTTP/1.1 200") && response.ends_with("normal-preview"), "{response:?}");
}
