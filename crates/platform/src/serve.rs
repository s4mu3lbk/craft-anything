//! A tiny static file server on loopback, for previewing and checking a built website.
//!
//! GET/HEAD only, one short-lived connection at a time per request thread, paths confined to the
//! root (no `..`, symlinks resolved and re-checked), unknown extension-less paths fall back to
//! `index.html` (single-page apps). It stops when the [`StaticServer`] is dropped.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::{Duration, Instant};

const MAX_CONNECTIONS: usize = 64;
const CLOSE_BUDGET: Duration = Duration::from_millis(200);
const UNAVAILABLE: &[u8] = b"HTTP/1.1 503 Service Unavailable\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";

pub struct StaticServer {
    pub port: u16,
    pub root: PathBuf,
    stop: Arc<AtomicBool>,
    accept_thread: Option<std::thread::JoinHandle<()>>,
    refusal_thread: Option<std::thread::JoinHandle<()>>,
}

impl StaticServer {
    pub fn url(&self) -> String {
        format!("http://127.0.0.1:{}/", self.port)
    }
}

impl Drop for StaticServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(thread) = self.accept_thread.take() {
            let _ = thread.join();
        }
        // Joining the accept thread closes the sender. At most one current refusal and one
        // queued refusal remain, each with its own short close budget.
        if let Some(thread) = self.refusal_thread.take() {
            let _ = thread.join();
        }
    }
}

struct PreviewConnection(Arc<AtomicUsize>);

impl Drop for PreviewConnection {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

/// Serve `root` on `127.0.0.1` at a free port.
pub fn serve_dir(root: &Path) -> std::io::Result<StaticServer> {
    let root = std::fs::canonicalize(root)?;
    let listener = TcpListener::bind("127.0.0.1:0")?;
    listener.set_nonblocking(true)?;
    let port = listener.local_addr()?.port();
    let stop = Arc::new(AtomicBool::new(false));
    let (r, s) = (root.clone(), stop.clone());
    let active = Arc::new(AtomicUsize::new(0));
    // A fixed worker and one waiting connection keep rejection itself bounded. When both
    // are occupied, try_send drops the excess connection instead of creating more workers.
    let (refuse, refused) = std::sync::mpsc::sync_channel(1);
    let refusal_thread = std::thread::Builder::new().name("static-refusal".into()).spawn(move || {
        while let Ok(stream) = refused.recv() {
            close_after_response(stream, UNAVAILABLE);
        }
    })?;
    let accept_thread = std::thread::Builder::new().name("static-server".into()).spawn(move || {
        while !s.load(Ordering::SeqCst) {
            match listener.accept() {
                Ok((stream, _)) => {
                    if active.load(Ordering::SeqCst) >= MAX_CONNECTIONS {
                        let _ = refuse.try_send(stream);
                        continue;
                    }
                    active.fetch_add(1, Ordering::SeqCst);
                    let connection = PreviewConnection(active.clone());
                    let r = r.clone();
                    let _ = std::thread::Builder::new().name("static-conn".into()).spawn(move || {
                        let _connection = connection;
                        let _ = handle(stream, &r);
                    });
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => std::thread::sleep(Duration::from_millis(15)),
                Err(_) => std::thread::sleep(Duration::from_millis(50)),
            }
        }
    });
    let accept_thread = match accept_thread {
        Ok(thread) => thread,
        Err(error) => {
            // A failed spawn drops its closure, including the only sender.
            let _ = refusal_thread.join();
            return Err(error);
        }
    };
    Ok(StaticServer { port, root, stop, accept_thread: Some(accept_thread), refusal_thread: Some(refusal_thread) })
}

/// Send the response and a FIN before draining unread input, with a shared time and byte cap.
fn close_after_response(mut stream: TcpStream, mut response: &[u8]) {
    if stream.set_nonblocking(true).is_err() {
        return;
    }
    let until = Instant::now() + CLOSE_BUDGET;
    while !response.is_empty() {
        if Instant::now() >= until {
            return;
        }
        match stream.write(response) {
            Ok(0) => return,
            Ok(n) => response = response.get(n..).unwrap_or_default(),
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => std::thread::sleep(Duration::from_millis(1)),
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
            Err(_) => return,
        }
    }
    let _ = stream.shutdown(Shutdown::Write);
    let mut incoming = (&stream).take(16 * 1024);
    let mut buf = [0u8; 1024];
    while Instant::now() < until {
        match incoming.read(&mut buf) {
            Ok(0) => break,
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => std::thread::sleep(Duration::from_millis(1)),
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
            Err(_) => break,
        }
    }
}

/// A whole-request read budget cannot be extended by a client periodically sending another byte.
struct RequestReader<'a> {
    stream: &'a TcpStream,
    deadline: Instant,
}

impl Read for RequestReader<'_> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let left = self.deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return Err(std::io::Error::new(std::io::ErrorKind::TimedOut, "Preview request read deadline exceeded"));
        }
        self.stream.set_read_timeout(Some(left))?;
        let count = self.stream.read(buf)?;
        if Instant::now() >= self.deadline {
            return Err(std::io::Error::new(std::io::ErrorKind::TimedOut, "Preview request read deadline exceeded"));
        }
        Ok(count)
    }
}

/// The request line and its Host and Origin values. Keeps the request size limit and shares one
/// deadline across the request line and headers.
fn request_head(stream: &TcpStream) -> std::io::Result<(String, Vec<String>, Vec<String>)> {
    let mut reader = BufReader::new(RequestReader { stream, deadline: Instant::now() + Duration::from_secs(5) }.take(16 * 1024));
    let mut line = String::new();
    reader.read_line(&mut line)?;
    let (mut hosts, mut origins) = (Vec::new(), Vec::new());
    let mut h = String::new();
    loop {
        if reader.read_line(&mut h)? == 0 {
            return Err(std::io::Error::new(std::io::ErrorKind::UnexpectedEof, "Preview request headers ended before the terminator"));
        }
        if matches!(h.as_str(), "\n" | "\r\n") {
            break;
        }
        if let Some((name, value)) = h.trim_end_matches(['\r', '\n']).split_once(':') {
            if name.eq_ignore_ascii_case("host") {
                hosts.push(value.trim().to_string());
            }
            if name.eq_ignore_ascii_case("origin") {
                origins.push(value.trim().to_string());
            }
        }
        h.clear();
    }
    Ok((line, hosts, origins))
}

fn content_type(p: &Path) -> &'static str {
    match p.extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase).as_deref() {
        Some("html" | "htm") => "text/html; charset=utf-8",
        Some("js" | "mjs") => "text/javascript; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("json" | "map") => "application/json",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("avif") => "image/avif",
        Some("ico") => "image/x-icon",
        Some("woff") => "font/woff",
        Some("woff2") => "font/woff2",
        Some("ttf") => "font/ttf",
        Some("otf") => "font/otf",
        Some("wasm") => "application/wasm",
        Some("txt") => "text/plain; charset=utf-8",
        Some("mp4") => "video/mp4",
        Some("webm") => "video/webm",
        _ => "application/octet-stream",
    }
}

fn percent_decode(s: &str) -> Option<String> {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        match b.get(i) {
            Some(b'%') => {
                let hex = s.get(i + 1..i + 3)?;
                if !hex.bytes().all(|h| h.is_ascii_hexdigit()) {
                    return None;
                }
                out.push(u8::from_str_radix(hex, 16).ok()?);
                i += 3;
            }
            Some(c) => {
                out.push(*c);
                i += 1;
            }
            None => break,
        }
    }
    String::from_utf8(out).ok()
}

/// Map a request path to a file under `root`, or `None` when it is outside or missing.
pub fn resolve(root: &Path, url_path: &str) -> Option<PathBuf> {
    let path = url_path.split(['?', '#']).next().unwrap_or("/");
    let decoded = percent_decode(path)?;
    let mut rel = PathBuf::new();
    for c in Path::new(decoded.trim_start_matches('/')).components() {
        match c {
            Component::Normal(n) => rel.push(n),
            Component::CurDir => {}
            _ => return None,
        }
    }
    let mut candidate = root.join(&rel);
    if candidate.is_dir() {
        candidate = candidate.join("index.html");
    }
    // A static export may write a page as `about.html` (Next.js without trailing slashes).
    if !candidate.is_file() && rel.extension().is_none() {
        let page = root.join(&rel).with_extension("html");
        candidate = if !rel.as_os_str().is_empty() && page.is_file() { page } else { root.join("index.html") };
    }
    let real = std::fs::canonicalize(&candidate).ok()?;
    (real.starts_with(root) && real.is_file()).then_some(real)
}

fn handle(mut stream: TcpStream, root: &Path) -> std::io::Result<()> {
    stream.set_nonblocking(false)?;
    stream.set_write_timeout(Some(Duration::from_secs(20)))?;
    let (line, hosts, origins) = match request_head(&stream) {
        Ok(head) => head,
        Err(error) => {
            // Incomplete reads still close without serving a file. Send the FIN before
            // draining so a timed-out client does not need to stop writing first.
            close_after_response(stream, &[]);
            return Err(error);
        }
    };
    if hosts.len() != 1 || origins.len() > 1 {
        return stream.write_all(b"HTTP/1.1 400 Bad Request\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
    }
    if !hosts.first().is_some_and(|h| crate::standin_server::host_is_loopback(h, Some(stream.local_addr().map_or(0, |a| a.port()))))
        || origins.first().is_some_and(|o| !crate::standin_server::origin_is_local(o))
    {
        return stream.write_all(b"HTTP/1.1 403 Forbidden\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
    }
    let mut parts = line.split_whitespace();
    let (method, target) = (parts.next().unwrap_or_default(), parts.next().unwrap_or("/"));
    if method != "GET" && method != "HEAD" {
        return stream.write_all(b"HTTP/1.1 405 Method Not Allowed\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
    }
    match resolve(root, target).and_then(|p| {
        let file = std::fs::File::open(&p).ok()?;
        let len = file.metadata().ok()?.len();
        Some((p, file, len))
    }) {
        Some((p, file, len)) => send_file(&mut stream, file, len, content_type(&p), method == "GET"),
        None => {
            stream.write_all(b"HTTP/1.1 404 Not Found\r\nContent-Type: text/plain\r\nContent-Length: 9\r\nConnection: close\r\n\r\n")?;
            if method != "HEAD" {
                stream.write_all(b"Not found")?;
            }
            Ok(())
        }
    }
}

fn send_file(stream: &mut impl Write, file: impl Read, len: u64, mime: &str, include_body: bool) -> std::io::Result<()> {
    let head = format!("HTTP/1.1 200 OK\r\nContent-Type: {mime}\r\nContent-Length: {len}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n");
    stream.write_all(head.as_bytes())?;
    if include_body {
        std::io::copy(&mut file.take(len), stream)?;
    }
    Ok(())
}

#[cfg(test)]
mod stream_tests;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percent_decode_rejects_signs_in_escapes() {
        assert_eq!(percent_decode("a%+41"), None);
        assert_eq!(percent_decode("%+4"), None);
        assert_eq!(percent_decode("%-4"), None);
        assert_eq!(percent_decode("%G1"), None);
        assert_eq!(percent_decode("%41").as_deref(), Some("A"));
    }

    fn get(port: u16, path: &str) -> String {
        let mut s = TcpStream::connect(("127.0.0.1", port)).unwrap();
        s.write_all(format!("GET {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n\r\n").as_bytes()).unwrap();
        let mut out = String::new();
        s.read_to_string(&mut out).unwrap();
        out
    }

    #[test]
    fn serves_files_spa_fallback_and_refuses_escapes() {
        let dir = tempfile::tempdir().unwrap();
        let site = dir.path().join("dist");
        std::fs::create_dir_all(site.join("assets")).unwrap();
        std::fs::write(site.join("index.html"), "<h1>home</h1>").unwrap();
        std::fs::write(site.join("assets/app.js"), "console.log(1)").unwrap();
        std::fs::write(dir.path().join("secret.txt"), "nope").unwrap();
        let srv = serve_dir(&site).unwrap();
        assert!(get(srv.port, "/").contains("<h1>home</h1>"));
        let js = get(srv.port, "/assets/app.js?v=1");
        assert!(js.contains("text/javascript") && js.contains("console.log(1)"));
        assert!(get(srv.port, "/pricing").contains("<h1>home</h1>"), "client-side routes fall back to index.html");
        std::fs::write(site.join("about.html"), "<h1>about</h1>").unwrap();
        assert!(get(srv.port, "/about").contains("<h1>about</h1>") && get(srv.port, "/about/").contains("<h1>about</h1>"), "an exported page");
        assert!(get(srv.port, "/missing.png").starts_with("HTTP/1.1 404"));
        for bad in ["/../secret.txt", "/%2e%2e/secret.txt", "/assets/../../secret.txt"] {
            assert!(!get(srv.port, bad).contains("nope"), "{bad}");
        }
        let url = srv.url();
        drop(srv);
        assert!(url.starts_with("http://127.0.0.1:"));
    }
}
