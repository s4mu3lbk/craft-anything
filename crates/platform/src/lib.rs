//! Native platform services for Craft Anything: owned processes, file safety and program lookup.
//!
//! Every external program Craft Anything runs goes through [`process`]: stdin is closed or fed from
//! bytes we own, every run has wall and idle deadlines, output is capped and drained on threads,
//! and the whole process group (Unix) or Job Object (Windows) is killed on timeout, cancel or drop.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

pub mod cancel;
pub mod capture;
pub mod display;
pub mod fs;
pub mod process;
pub mod serve;
pub mod standin_server;
pub mod sync;
pub mod which;
pub mod window;

pub use capture::{Capture, Event, EventKind, Sig, Spec as CaptureSpec};
pub use fs::{backup_once, write_atomic, write_atomic_private};
pub use process::{Cancel, Ended, Launch, Launched, LineSink, Output, Process, RunningProcess, StdinMode, open_path};
pub use which::{cli_file_name, self_cli_path, which, which_all};

/// Whether this app runs in a Flatpak sandbox (it sets `FLATPAK_ID` and makes `/.flatpak-info`).
pub fn in_flatpak() -> bool {
    static IN: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *IN.get_or_init(|| std::env::var_os("FLATPAK_ID").is_some() || std::path::Path::new("/.flatpak-info").exists())
}

/// Owns the helper that keeps a build awake until the guard is released. Dropping it waits up to
/// five seconds for supervised cleanup and logs a warning if that deadline passes.
#[derive(Debug)]
pub struct KeepAwake(RunningProcess);

impl Drop for KeepAwake {
    fn drop(&mut self) {
        if !self.0.kill_and_wait(std::time::Duration::from_secs(5)) {
            log::warn!("keep-awake helper {} did not confirm stopping within five seconds", self.0.pid());
        }
    }
}

/// Keep the computer from sleeping while the returned guard lives:
/// a clone that runs for hours must not be cut off by sleep, which would also trip every time
/// limit at once on waking. The screen may still turn off. `None` where there's no helper.
// shortcut: Windows (a PowerShell helper, since this workspace forbids unsafe FFI) and macOS
// (caffeinate) only; add `systemd-inhibit` for Linux when it's needed.
pub fn keep_awake() -> Option<KeepAwake> {
    keep_awake_while(std::process::id(), std::time::Duration::from_secs(30))
}

/// The helper for [`keep_awake`], which also ends by itself once process `owner` is gone (checked
/// every `every` on Windows), so a process that exits without dropping the guard (the window
/// closed during a build, a crash, a kill) never leaves the computer held awake.
fn keep_awake_while(owner: u32, every: std::time::Duration) -> Option<KeepAwake> {
    // ES_CONTINUOUS | ES_SYSTEM_REQUIRED (0x80000001, in decimal: Windows PowerShell reads the hex
    // literal as a negative Int32), held by the helper's thread until it's stopped. Any failure
    // ends the helper, so it never runs without doing its job. The owner is opened once and its
    // handle kept, so Windows can't give its id to another process the helper would then serve.
    let windows = format!(
        "$ErrorActionPreference = 'Stop'; $k = Add-Type -Name P -Namespace CraftAnything -PassThru -MemberDefinition '[DllImport(\"kernel32.dll\")] public static extern uint SetThreadExecutionState(uint f);'; $o = Get-Process -Id {owner}; $null = $o.Handle; while (-not $o.HasExited) {{ if ($k::SetThreadExecutionState(2147483649) -eq 0) {{ exit 1 }}; Start-Sleep -Seconds {} }}",
        every.as_secs().max(1)
    );
    let p = if cfg!(windows) {
        Process::new("powershell").args(["-NoProfile", "-NonInteractive", "-Command", windows.as_str()])
    } else if cfg!(target_os = "macos") {
        // caffeinate's own -w ends it when that process exits.
        Process::new("caffeinate").args(["-i".to_string(), "-w".into(), owner.to_string()])
    } else {
        return None;
    };
    p.timeout(std::time::Duration::from_secs(7 * 24 * 3600)).label("keep awake").spawn().ok().map(KeepAwake)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::{TcpListener, TcpStream};
    use std::time::{Duration, Instant};

    #[cfg(windows)]
    #[test]
    fn keep_awake_holds_until_dropped() {
        let mut guard = crate::keep_awake().expect("the helper starts");
        // A script error (the Windows call failing to load) would end it within moments.
        assert!(guard.0.wait_timeout(Duration::from_secs(4)).is_none(), "still holding the computer awake");
        let pid = guard.0.pid();
        drop(guard);
        let absent = Process::new("tasklist")
            .args(["/FI", &format!("PID eq {pid}"), "/NH"])
            .timeout(Duration::from_secs(5))
            .run()
            .map(|o| o.success && !o.stdout_text().contains(&pid.to_string()));
        assert_eq!(absent.ok(), Some(true), "dropping the guard stops the helper");
    }

    /// The window closed during a build: the app ends without dropping the guard, and its helper
    /// must not keep the computer awake for a week.
    #[cfg(any(windows, target_os = "macos"))]
    #[test]
    fn the_helper_ends_when_its_owner_is_gone() {
        let owner = if cfg!(windows) {
            Process::new("powershell").args(["-NoProfile", "-NonInteractive", "-Command", "Start-Sleep -Seconds 60"])
        } else {
            Process::new("sleep").arg("60")
        };
        let mut owner = owner.timeout(Duration::from_secs(90)).spawn().expect("the stand-in owner starts");
        let mut guard = keep_awake_while(owner.pid(), Duration::from_secs(1)).expect("the helper starts");
        assert!(guard.0.wait_timeout(Duration::from_secs(3)).is_none(), "holding while its owner runs");
        assert!(owner.kill_and_wait(Duration::from_secs(5)), "the stand-in owner stops");
        assert!(guard.0.wait_timeout(Duration::from_secs(15)).is_some(), "the helper ends by itself once its owner is gone");
    }

    fn stand_in(name: &str) -> Process {
        // argv[0] is the test binary even in a sandbox without /proc/self/exe.
        let exe = std::env::args_os().next().expect("test binary path");
        Process::new(exe).args(["--ignored", "--exact", name, "--nocapture"]).env("CRAFT_KEEP_AWAKE_STAND_IN", "1").timeout(Duration::from_secs(15))
    }

    fn helper() -> (KeepAwake, u16) {
        let (tx, rx) = std::sync::mpsc::channel();
        let process = stand_in("tests::keep_awake_helper_stand_in")
            .on_stdout_line(move |line| {
                if let Some((_, port)) = line.split_once("AWAKE_PORT=")
                    && let Ok(port) = port.trim().parse::<u16>()
                {
                    let _ = tx.send(port);
                }
            })
            .spawn()
            .unwrap();
        let mut guard = KeepAwake(process);
        let port = rx.recv_timeout(Duration::from_secs(5)).expect("the helper is listening");
        assert!(guard.0.wait_timeout(Duration::from_millis(50)).is_none(), "the helper stays alive until released");
        (guard, port)
    }

    /// If a failed check left the helper alive, tell that exact helper to exit before asserting.
    fn stop_if_alive(port: u16) -> bool {
        let Ok(mut stream) = TcpStream::connect(("127.0.0.1", port)) else { return false };
        stream.write_all(b"stop").unwrap();
        stream.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        let mut tail = Vec::new();
        let _ = stream.read_to_end(&mut tail);
        true
    }

    #[test]
    fn dropping_keep_awake_finishes_stopping_its_helper() {
        let (guard, port) = helper();
        assert!(TcpStream::connect(("127.0.0.1", port)).is_ok(), "the helper really started");
        drop(guard);
        assert!(!stop_if_alive(port), "the helper must be gone when dropping its guard returns");
    }

    #[test]
    fn a_cli_can_exit_immediately_after_releasing_keep_awake() {
        let output = stand_in("tests::keep_awake_cli_stand_in").run().unwrap();
        let stdout = output.stdout_text();
        let port: u16 = stdout.lines().find_map(|line| line.split_once("AWAKE_PORT=")).unwrap().1.trim().parse().unwrap();
        let alive = stop_if_alive(port);
        assert!(output.success, "{stdout}\n{}", output.stderr_text());
        assert!(!alive, "the CLI must finish cleanup before its supervisor threads disappear");
    }

    #[test]
    fn helper_handles_stop_arriving_after_connection_is_accepted() {
        let (mut guard, port) = helper();
        let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
        std::thread::sleep(Duration::from_millis(50));
        stream.write_all(b"stop").unwrap();
        let exited = guard.0.wait_timeout(Duration::from_secs(2)).is_some();
        if !exited {
            stop_if_alive(port);
        }
        assert!(exited, "the helper must process a stop that arrives after accept");
    }

    #[test]
    fn releasing_a_finished_keep_awake_helper_does_not_wait_for_the_deadline() {
        let mut process = stand_in("tests::keep_awake_finished_stand_in").spawn().unwrap();
        assert!(process.wait_timeout(Duration::from_secs(5)).unwrap().as_ref().unwrap().success);
        let start = Instant::now();
        drop(KeepAwake(process));
        assert!(start.elapsed() < Duration::from_secs(2));
    }

    #[test]
    #[ignore = "stand-in helper invoked by keep-awake lifecycle tests"]
    fn keep_awake_helper_stand_in() {
        if std::env::var_os("CRAFT_KEEP_AWAKE_STAND_IN").is_none() {
            return;
        }
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        listener.set_nonblocking(true).unwrap();
        println!("AWAKE_PORT={}", listener.local_addr().unwrap().port());
        std::io::stdout().flush().unwrap();
        let deadline = Instant::now() + Duration::from_secs(10);
        while Instant::now() < deadline {
            match listener.accept() {
                Ok((mut stream, _)) => {
                    stream.set_nonblocking(false).unwrap();
                    stream.set_read_timeout(Some(Duration::from_millis(200))).unwrap();
                    let mut byte = [0];
                    if stream.read(&mut byte).ok() == Some(1) {
                        return;
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => std::thread::sleep(Duration::from_millis(5)),
                Err(e) => panic!("helper listener failed: {e}"),
            }
        }
    }

    #[test]
    #[ignore = "stand-in CLI invoked by a_cli_can_exit_immediately_after_releasing_keep_awake"]
    fn keep_awake_cli_stand_in() {
        if std::env::var_os("CRAFT_KEEP_AWAKE_STAND_IN").is_none() {
            return;
        }
        let (guard, port) = helper();
        println!("AWAKE_PORT={port}");
        std::io::stdout().flush().unwrap();
        drop(guard);
        std::process::exit(0);
    }

    #[test]
    #[ignore = "stand-in completed helper invoked by keep-awake lifecycle tests"]
    fn keep_awake_finished_stand_in() {}
}
