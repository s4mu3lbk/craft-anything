//! Talking to a Cutter (or any Rizin) session the person opened to us: in Cutter's console they
//! run `=h& 9090`, which serves that session's Rizin over HTTP on this computer; each command is
//! one `GET /cmd/<command>` answered with its text output. Only 127.0.0.1 is ever contacted.

use craft_anything_evidence::{ErrorCode, ToolError};
use serde_json::{Value, json};
use std::io::Read;
use std::time::Duration;

/// Rizin's default HTTP port (`http.port`).
pub const DEFAULT_PORT: u16 = 9090;
/// Text returned per command.
const MAX_BODY: u64 = 4 << 20;

/// How the person opens a session to us.
pub const HOW_TO_OPEN: &str = "In Cutter, open the Console (Windows → Console) and run `=h& 9090` to share that session on this computer.";

/// `cmd` with everything but unreserved characters percent-encoded, for the URL path.
fn encode(cmd: &str) -> String {
    let mut out = String::with_capacity(cmd.len() * 3);
    for b in cmd.bytes() {
        if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) {
            out.push(char::from(b));
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// Run one command in the session on `port`; the text and whether it was cut at 4 MiB.
pub fn command(port: u16, cmd: &str, timeout: Duration) -> Result<(String, bool), ToolError> {
    let url = format!("http://127.0.0.1:{port}/cmd/{}", encode(cmd));
    let agent = ureq::Agent::config_builder()
        .http_status_as_error(false)
        .user_agent("CraftAnything-dev")
        .timeout_connect(Some(Duration::from_secs(2)))
        .timeout_global(Some(timeout))
        .build()
        .new_agent();
    let resp = agent.get(&url).call().map_err(|e| {
        let code = if matches!(e, ureq::Error::Timeout(_)) { ErrorCode::TimedOut } else { ErrorCode::NotFound };
        ToolError::new(code, format!("No Cutter session answered on port {port}: {e}")).with_remediation(HOW_TO_OPEN)
    })?;
    let status = resp.status().as_u16();
    if !(200..300).contains(&status) {
        return Err(ToolError::new(ErrorCode::NotFound, format!("The session on port {port} answered HTTP {status}"))
            .with_remediation("Check that the port belongs to a Cutter or Rizin session (`=h& 9090` in its console)."));
    }
    let mut body = Vec::new();
    resp.into_body()
        .into_reader()
        .take(MAX_BODY + 1)
        .read_to_end(&mut body)
        .map_err(|e| ToolError::new(ErrorCode::NotFound, format!("The session on port {port} broke off: {e}")))?;
    let truncated = body.len() as u64 > MAX_BODY;
    body.truncate(MAX_BODY as usize);
    Ok((String::from_utf8_lossy(&body).into_owned(), truncated))
}

/// What the session on `port` has open, or `None` when nothing answers there.
pub fn probe(port: u16) -> Option<Value> {
    let (text, _) = command(port, "ij", Duration::from_secs(3)).ok()?;
    let info: Value = serde_json::from_str(text.trim()).unwrap_or(Value::Null);
    let core = info.get("core");
    let bin = info.get("bin");
    Some(json!({
        "port": port,
        "file": core.and_then(|c| c.get("file")).cloned().unwrap_or(Value::Null),
        "format": core.and_then(|c| c.get("format")).cloned().unwrap_or(Value::Null),
        "arch": bin.and_then(|b| b.get("arch")).cloned().unwrap_or(Value::Null),
        "bits": bin.and_then(|b| b.get("bits")).cloned().unwrap_or(Value::Null),
        "answered_info": !info.is_null(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::net::{TcpListener, TcpStream};
    use std::sync::mpsc;

    /// A stand-in session: answers one request with `body` and hands back the request line.
    fn stand_in(body: &'static str) -> (u16, std::thread::JoinHandle<String>) {
        stand_in_observing_reads(body, None)
    }

    fn stand_in_observing_reads(body: &'static str, waiting_for_read: Option<mpsc::Sender<usize>>) -> (u16, std::thread::JoinHandle<String>) {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = l.local_addr().unwrap().port();
        let h = std::thread::spawn(move || {
            let (mut s, _) = l.accept().unwrap();
            s.set_write_timeout(Some(Duration::from_secs(3))).unwrap();
            let deadline = std::time::Instant::now() + Duration::from_secs(3);
            let mut buf = [0u8; 4096];
            let mut head = Vec::new();
            while !head.windows(4).any(|w| w == b"\r\n\r\n") {
                let left = deadline.saturating_duration_since(std::time::Instant::now());
                assert!(!left.is_zero(), "the fixture request did not complete in time");
                s.set_read_timeout(Some(left)).unwrap();
                if let Some(waiting) = &waiting_for_read {
                    waiting.send(head.len()).unwrap();
                }
                let n = s.read(&mut buf).unwrap();
                assert_ne!(n, 0, "the request ended before its headers were complete");
                head.extend_from_slice(&buf[..n]);
                assert!(head.len() <= 16 * 1024, "the fixture request exceeded its header limit");
            }
            let req = String::from_utf8_lossy(&head).lines().next().unwrap_or("").to_string();
            write!(s, "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
            req
        });
        (port, h)
    }

    #[test]
    fn sends_the_command_in_the_path() {
        assert_eq!(encode("pdf @ main"), "pdf%20%40%20main");
        let (port, h) = stand_in("0x00401000  push rbp\n");
        let (text, cut) = command(port, "pd 1 @ entry0", Duration::from_secs(5)).unwrap();
        assert_eq!((text.as_str(), cut), ("0x00401000  push rbp\n", false));
        assert_eq!(h.join().unwrap(), "GET /cmd/pd%201%20%40%20entry0 HTTP/1.1");
    }

    #[test]
    fn waits_for_fragmented_request_headers_before_answering() {
        let (waiting_tx, waiting_rx) = mpsc::channel();
        let (port, h) = stand_in_observing_reads("ok", Some(waiting_tx));
        let mut client = TcpStream::connect(("127.0.0.1", port)).unwrap();
        client.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
        client.set_write_timeout(Some(Duration::from_secs(3))).unwrap();
        let partial = b"GET /cmd/ij HTTP/1.1\r\nHost: 127.0.0.1\r\n\r";
        client.write_all(partial).unwrap();

        // Withhold the final byte until the server has consumed the fragment and needs another read.
        // A single-read fixture closes the observer channel without reaching this state.
        loop {
            let consumed = waiting_rx.recv_timeout(Duration::from_secs(3)).expect("the fixture must read again after the partial header");
            assert!(consumed <= partial.len());
            if consumed == partial.len() {
                break;
            }
        }
        client.write_all(b"\n").unwrap();

        let mut response = String::new();
        client.read_to_string(&mut response).unwrap();
        assert_eq!(h.join().unwrap(), "GET /cmd/ij HTTP/1.1");
        assert_eq!(response, "HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok");
    }

    #[test]
    fn probe_reads_what_is_open() {
        let (port, h) = stand_in(r#"{"core":{"file":"C:\\a\\app.exe","format":"pe64"},"bin":{"arch":"x86","bits":64}}"#);
        let v = probe(port).unwrap();
        h.join().unwrap();
        assert_eq!(v["file"], "C:\\a\\app.exe");
        assert_eq!(v["bits"], 64);
        // Port zero cannot be reassigned to another fixture's listener.
        assert!(probe(0).is_none());
    }
}
