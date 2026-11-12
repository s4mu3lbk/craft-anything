//! A runner such as `go run` or `dotnet run` may own no window. Only inspect its descendants;
//! a process with the same name elsewhere on the desktop is not part of this capture.

use craft_anything_evidence::{ErrorCode, ToolError};
use craft_anything_platform::process::Process;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::time::Duration;

const MAX_PROCESSES: usize = 32_768;

/// Read only numeric process relationships, with bounded output and a deadline.
pub(super) fn snapshot(cancel: &crate::tool::CancelToken) -> Result<String, ToolError> {
    #[cfg(windows)]
    let command = {
        let windir = std::env::var_os("SystemRoot").map(std::path::PathBuf::from).unwrap_or_else(|| std::path::PathBuf::from(r"C:\Windows"));
        Process::new(windir.join(r"System32\WindowsPowerShell\v1.0\powershell.exe")).args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            "$ErrorActionPreference = 'Stop'; Get-CimInstance Win32_Process | ForEach-Object { '{0} {1}' -f $_.ProcessId, $_.ParentProcessId }",
        ])
    };
    #[cfg(not(windows))]
    // One `-o` per column: after `=` the rest of the argument is the header (POSIX), so FreeBSD
    // reads `pid=,ppid=` as one `pid` column headed ",ppid=" and no parent is ever listed.
    let command = Process::new("/bin/ps").args(["-A", "-o", "pid=", "-o", "ppid="]);
    let cancel = cancel.clone();
    let out = command
        .timeout(Duration::from_secs(10))
        .idle_timeout(Duration::from_secs(10))
        .stdout_cap(1024 * 1024)
        .cancel_when(move || cancel.is_cancelled())
        .run_checked()?;
    if out.stdout_truncated {
        return Err(ToolError::new(ErrorCode::ProgramFailed, "The process list was too large to identify the application's window safely."));
    }
    Ok(out.stdout_text())
}

/// Breadth-first descendants, regardless of snapshot order. Bound malformed/cyclic input too.
fn descendants(root: u32, text: &str) -> Vec<u32> {
    let mut children = BTreeMap::<u32, Vec<u32>>::new();
    for line in text.lines().take(MAX_PROCESSES) {
        let mut words = line.split_whitespace();
        if let (Some(pid), Some(parent), None) = (words.next(), words.next(), words.next())
            && let (Ok(pid), Ok(parent)) = (pid.parse::<u32>(), parent.parse::<u32>())
            && pid != 0
        {
            children.entry(parent).or_default().push(pid);
        }
    }
    let mut seen = BTreeSet::from([root]);
    let mut pending = VecDeque::from([root]);
    let mut result = Vec::new();
    while let Some(parent) = pending.pop_front() {
        for pid in children.remove(&parent).unwrap_or_default() {
            if seen.insert(pid) {
                result.push(pid);
                pending.push_back(pid);
            }
        }
    }
    result
}

/// Return the window owner's PID alongside its picture so accessibility uses that same process.
pub(super) fn capture<T>(
    root: u32,
    snapshot: impl FnOnce() -> Result<String, ToolError>,
    mut picture: impl FnMut(u32) -> Result<T, ToolError>,
) -> Result<(u32, T), ToolError> {
    let missing = match picture(root) {
        Ok(shot) => return Ok((root, shot)),
        Err(e) if e.code == ErrorCode::NotFound => e,
        Err(e) => return Err(e),
    };
    let candidates = descendants(root, &snapshot()?);
    if candidates.len() > 64 {
        return Err(ToolError::new(ErrorCode::ProgramFailed, "The program started too many processes to identify its window safely."));
    }
    for pid in candidates {
        match picture(pid) {
            Ok(shot) => return Ok((pid, shot)),
            Err(e) if e.code == ErrorCode::NotFound => {}
            Err(e) => return Err(e),
        }
    }
    Err(missing)
}

/// Look until something other than "no window yet" comes back, or `can_wait` says to stop.
pub(super) fn keep_looking<T>(mut look: impl FnMut() -> Result<T, ToolError>, mut can_wait: impl FnMut() -> bool, pause: Duration) -> Result<T, ToolError> {
    loop {
        match look() {
            Err(e) if e.code == ErrorCode::NotFound && can_wait() => std::thread::sleep(pause),
            done => return done,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_late_window_is_waited_for_but_not_forever_and_other_errors_end_it() {
        let mut looks = 0;
        let found = keep_looking(
            || {
                looks += 1;
                if looks < 3 { Err(missing()) } else { Ok("picture") }
            },
            || true,
            Duration::ZERO,
        );
        assert_eq!(found.unwrap(), "picture");
        assert_eq!(looks, 3);

        let mut waits = 0;
        let gave_up = keep_looking::<()>(
            || Err(missing()),
            || {
                waits += 1;
                waits < 3
            },
            Duration::ZERO,
        );
        assert_eq!(gave_up.unwrap_err().code, ErrorCode::NotFound);
        assert_eq!(waits, 3);

        let mut looks = 0;
        let denied = keep_looking::<()>(
            || {
                looks += 1;
                Err(ToolError::new(ErrorCode::ProgramFailed, "denied"))
            },
            || true,
            Duration::ZERO,
        );
        assert_eq!(denied.unwrap_err().message, "denied");
        assert_eq!(looks, 1, "only a missing window is worth another look");
    }

    fn missing() -> ToolError {
        ToolError::new(ErrorCode::NotFound, "No window")
    }

    #[test]
    fn runner_capture_returns_application_pid_and_ignores_unrelated_windows() {
        let mut looked_up = Vec::new();
        let (tree_pid, shot) = capture(
            10,
            || Ok("30 20\n99 1\n20 10\n".into()),
            |pid| {
                looked_up.push(pid);
                if pid == 30 || pid == 99 { Ok("application picture") } else { Err(missing()) }
            },
        )
        .unwrap();
        assert_eq!(looked_up, [10, 20, 30]);
        assert_eq!(tree_pid, 30, "UI tree must use the actual window owner");
        assert_eq!(shot, "application picture");
        assert!(capture::<()>(10, || Ok("99 1\n".into()), |_| Err(missing())).is_err());
    }

    #[test]
    fn direct_window_needs_no_process_list_and_capture_errors_are_preserved() {
        assert_eq!(capture(10, || panic!("unneeded process list"), |_| Ok("direct")).unwrap(), (10, "direct"));
        let error = capture::<()>(10, || panic!("capture permission failure"), |_| Err(ToolError::new(ErrorCode::ProgramFailed, "denied"))).unwrap_err();
        assert_eq!(error.message, "denied");
        assert!(capture::<()>(10, || Err(ToolError::cancelled()), |_| Err(missing())).is_err());
    }

    #[test]
    fn process_relationships_are_bounded_and_cycle_safe() {
        assert_eq!(descendants(10, "20 10\n30 20\n10 30\n20 10\n0 10\nbad\n40 10 extra\n"), [20, 30]);
        let text = format!("{}20 10\n", "bad\n".repeat(MAX_PROCESSES));
        assert!(descendants(10, &text).is_empty());
        let text = (20..85).map(|pid| format!("{pid} 10\n")).collect::<String>();
        let mut attempts = 0;
        let error = capture::<()>(
            10,
            || Ok(text),
            |_| {
                attempts += 1;
                Err(missing())
            },
        )
        .unwrap_err();
        assert_eq!(error.code, ErrorCode::ProgramFailed);
        assert_eq!(attempts, 1, "oversized process families must not trigger repeated window scans");
    }

    /// Exercise the OS snapshot without opening any windows or requiring a language toolchain.
    #[cfg(unix)]
    #[test]
    fn snapshot_finds_an_owned_child_on_this_host() {
        let mut child = Process::new("/bin/sleep").arg("20").timeout(Duration::from_secs(30)).spawn().unwrap();
        let family = snapshot(&crate::CancelToken::default()).map(|text| descendants(std::process::id(), &text));
        assert!(child.kill_and_wait(Duration::from_secs(5)));
        assert!(family.unwrap().contains(&child.pid()));
    }
}
