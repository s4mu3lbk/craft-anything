//! The NDJSON client: connects to the bridge socket, authenticates each request with the run
//! token, and reads responses with a deadline and a cancel check so it can never block forever.

use super::protocol::{DeepError, Request, Response};
use serde_json::Value;
use std::io::{BufRead, BufReader, Write};
use std::net::{Shutdown, SocketAddr, TcpStream};
use std::time::{Duration, Instant};

/// How long one socket read blocks before we re-check the deadline and the cancel flag.
const READ_TICK: Duration = Duration::from_millis(200);

/// Largest reply line we read. The bridge answers with lists that the target decides the size
/// of (every function in the program), so a reply is refused rather than read without end.
pub const MAX_REPLY_BYTES: usize = 128 * 1024 * 1024;

pub struct Client {
    writer: TcpStream,
    reader: BufReader<TcpStream>,
    token: String,
    next_id: u64,
    /// The reply line read so far. It's kept between read ticks, so a long reply that arrives
    /// over more than one tick isn't cut.
    pending: Vec<u8>,
    max_reply: usize,
}

impl std::fmt::Debug for Client {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DeepClient").field("next_id", &self.next_id).finish()
    }
}

impl Client {
    /// Connect to the bridge, retrying until the deadline (the bridge may not be listening yet).
    pub fn connect(addr: SocketAddr, token: &str, deadline: Instant) -> Result<Client, DeepError> {
        let mut last = String::new();
        loop {
            if Instant::now() >= deadline {
                return Err(DeepError::Timeout(format!("could not connect to the bridge at {addr}: {last}")));
            }
            match TcpStream::connect_timeout(&addr, Duration::from_millis(500)) {
                Ok(stream) => {
                    stream.set_read_timeout(Some(READ_TICK)).map_err(|e| DeepError::Connection(e.to_string()))?;
                    let reader = BufReader::new(stream.try_clone().map_err(|e| DeepError::Connection(e.to_string()))?);
                    return Ok(Client { writer: stream, reader, token: token.to_string(), next_id: 1, pending: Vec::new(), max_reply: MAX_REPLY_BYTES });
                }
                Err(e) => {
                    last = e.to_string();
                    std::thread::sleep(Duration::from_millis(100));
                }
            }
        }
    }

    /// Send one request and wait for its reply (bounded by `deadline` and `cancel`).
    pub fn request(&mut self, method: &str, params: Value, cancel: &dyn Fn() -> bool, deadline: Instant) -> Result<Value, DeepError> {
        let id = self.next_id;
        self.next_id = self.next_id.saturating_add(1);
        let req = Request { id, token: self.token.clone(), method: method.to_string(), params };
        let mut line = serde_json::to_string(&req).map_err(|e| DeepError::Protocol(e.to_string()))?;
        line.push('\n');
        self.writer.write_all(line.as_bytes()).map_err(|e| DeepError::Connection(e.to_string()))?;
        self.writer.flush().map_err(|e| DeepError::Connection(e.to_string()))?;

        loop {
            if cancel() {
                return Err(DeepError::Cancelled);
            }
            if Instant::now() >= deadline {
                return Err(DeepError::Timeout(format!("no reply to {method} within the deadline")));
            }
            match self.read_line_part() {
                Ok(None) => continue,
                Ok(Some(line)) => {
                    if line.iter().all(u8::is_ascii_whitespace) {
                        continue;
                    }
                    let resp: Response = serde_json::from_slice(&line).map_err(|e| DeepError::Protocol(format!("bad response: {e}")))?;
                    if resp.id != id {
                        // Out-of-order frame; keep reading for ours.
                        continue;
                    }
                    if resp.ok {
                        return Ok(resp.result.unwrap_or(Value::Null));
                    }
                    let err = resp.error.unwrap_or(super::protocol::WireError { code: "bridge_error".into(), message: "unknown error".into() });
                    return Err(DeepError::Bridge { code: err.code, message: err.message });
                }
                Err(e) => return Err(e),
            }
        }
    }

    /// Read what's available of the next reply line. `Some` is a whole line; `None` means the
    /// read tick passed first (the deadline and cancel flag are checked again before the next).
    fn read_line_part(&mut self) -> Result<Option<Vec<u8>>, DeepError> {
        let room = self.max_reply.saturating_sub(self.pending.len());
        if room == 0 {
            // The rest of this reply is still on the socket, so later replies can't be told
            // apart from it: close the connection rather than read on.
            let _ = self.writer.shutdown(Shutdown::Both);
            self.pending = Vec::new();
            return Err(DeepError::Protocol(format!("a reply from the bridge was larger than {} MiB", self.max_reply / (1024 * 1024))));
        }
        let buf = match self.reader.fill_buf() {
            Ok(buf) => buf,
            Err(e) if matches!(e.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut | std::io::ErrorKind::Interrupted) => {
                return Ok(None);
            }
            Err(e) => return Err(DeepError::Connection(e.to_string())),
        };
        if buf.is_empty() {
            return Err(DeepError::Connection("the bridge closed the connection".into()));
        }
        let (take, complete) = match buf.iter().position(|&b| b == b'\n') {
            Some(i) if i < room => (i + 1, true),
            _ => (buf.len().min(room), false),
        };
        self.pending.extend_from_slice(buf.get(..take).unwrap_or_default());
        self.reader.consume(take);
        Ok(complete.then(|| std::mem::take(&mut self.pending)))
    }

    /// Use a smaller reply limit (tests).
    #[cfg(test)]
    pub(crate) fn with_max_reply(mut self, bytes: usize) -> Client {
        self.max_reply = bytes;
        self
    }

    /// The handshake: returns the bridge's session info.
    pub fn ping(&mut self, cancel: &dyn Fn() -> bool, deadline: Instant) -> Result<Value, DeepError> {
        self.request("ping", Value::Null, cancel, deadline)
    }

    /// Best-effort clean shutdown; ignores errors (the process-group kill is the backstop).
    pub fn shutdown(&mut self) {
        let deadline = Instant::now() + Duration::from_secs(10);
        let _ = self.request("shutdown", Value::Null, &|| false, deadline);
    }
}
