//! Agent sessions a turn has started, kept on disk while the turn runs (#1115). A turn that ends
//! deletes its own unfinished session; this covers the turns that never got to: Craft Anything
//! killed, crashed or uninstalled mid-turn. Each turn holds an OS lock on its record, which the
//! system releases when the process ends, so a record nobody holds belongs to a turn that's gone,
//! and [`sweep`] deletes its session. One file per turn, so turns never write each other's.

use super::adapters::AgentAdapters;
use serde::{Deserialize, Serialize};
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

const VERSION: u32 = 1;
/// Sweeps that try a record's delete before giving up on it (a host that's gone, a session the
/// person deleted already).
const TRIES: u32 = 3;
static COUNTER: AtomicU64 = AtomicU64::new(0);

/// The folder for the records inside the logs folder ([`crate::failure_log::dir_in`]).
pub fn dir_in(logs: &Path) -> PathBuf {
    logs.join("unfinished-sessions")
}

/// What it takes to delete one session later.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Entry {
    pub version: u32,
    /// The agent id, to find its adapter ([`AgentAdapters::for_agent`]).
    pub agent: String,
    pub program: String,
    /// The SSH host it ran on; None: this computer.
    pub host: Option<String>,
    pub id: String,
    #[serde(default)]
    pub tries: u32,
}

impl Entry {
    /// A record of session `id`, started by `agent`'s `program` on `host` (None: this computer).
    pub fn new(agent: &str, program: &str, host: Option<&str>, id: &str) -> Entry {
        Entry { version: VERSION, agent: agent.into(), program: program.into(), host: host.map(str::to_string), id: id.into(), tries: 0 }
    }
}

/// A running turn's record, locked until it's dropped.
#[derive(Debug)]
pub struct Held {
    file: File,
    path: PathBuf,
}

impl Held {
    /// Record `entry` in `dir`, locked before anything is written so no sweep can take it.
    pub fn record(dir: &Path, entry: &Entry) -> std::io::Result<Held> {
        std::fs::create_dir_all(dir)?;
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        // The time too: a dead process's leftover may carry a pid the system has handed out again.
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or_default();
        let path = dir.join(format!("{}-{now}-{n}.json", std::process::id()));
        let mut file = OpenOptions::new().read(true).write(true).create_new(true).open(&path)?;
        file.lock()?;
        file.write_all(&serde_json::to_vec(entry).map_err(std::io::Error::other)?)?;
        file.sync_all()?;
        Ok(Held { file, path })
    }

    /// Its session is gone or finished: retire the record while it's still locked.
    pub fn done(self) {
        retire(&self.file, &self.path);
    }
}

/// Empty a record, then remove it. A sweep that opened it a moment earlier can still take its
/// lock once it's released, but finds nothing to act on.
fn retire(file: &File, path: &Path) {
    let _ = file.set_len(0);
    let _ = std::fs::remove_file(path);
}

/// Sweep the records in a config folder's logs, as the desktop app does when it starts (#1115):
/// a turn that was killed mid-turn left its record, and the next agent turn may not come for a
/// while. Returns what to tell the person, as [`sweep`] does.
pub fn sweep_at_startup(config_dir: &Path, adapters: &AgentAdapters) -> Vec<String> {
    let logs = crate::failure_log::dir_in(config_dir);
    sweep(&dir_in(&logs), adapters)
}

/// Delete the session of every record no running turn holds. Returns what to tell the person:
/// a line per session deleted, or given up on.
pub fn sweep(dir: &Path, adapters: &AgentAdapters) -> Vec<String> {
    let Ok(rd) = std::fs::read_dir(dir) else { return Vec::new() };
    let mut said = Vec::new();
    for path in rd.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "json")) {
        // Deleted under us: not ours to touch.
        let Ok(file) = OpenOptions::new().read(true).write(true).open(&path) else { continue };
        said.extend(sweep_one(&path, file, adapters));
    }
    said
}

/// One record, opened at `path`: what to tell the person, if anything.
fn sweep_one(path: &Path, mut file: File, adapters: &AgentAdapters) -> Option<String> {
    // Held by a running turn.
    file.try_lock().ok()?;
    // Removed while we waited, its emptying failed or not: its owner finished it. (On Windows a
    // file waiting to be deleted can't be looked up either.)
    std::fs::symlink_metadata(path).ok()?;
    let mut text = String::new();
    file.read_to_string(&mut text).ok()?;
    if text.is_empty() {
        // Retired: its owner removed it, or was stopped before it could.
        let _ = std::fs::remove_file(path);
        return None;
    }
    // Unreadable or from a newer version: leave it as it is.
    let mut entry = serde_json::from_str::<Entry>(&text).ok().filter(|e| e.version == VERSION)?;
    let adapter = adapters.for_agent(&entry.agent);
    // The adapter here doesn't delete sessions (a fork's agent, or one a fork replaced): keep the
    // record for a build that has the adapter that does.
    if !adapter.discards_sessions() {
        return None;
    }
    let on = entry.host.as_ref().map(|h| format!(" on {h}")).unwrap_or_default();
    match adapter.discard_unfinished(&entry.program, &entry.id, entry.host.as_deref()) {
        Ok(()) => {
            retire(&file, path);
            Some(format!("Removed session {}{on}, left unfinished by a build Craft Anything didn't get to stop.", entry.id))
        }
        Err(e) => {
            entry.tries += 1;
            if entry.tries >= TRIES {
                retire(&file, path);
                let fix = e.remediation.map(|r| format!(" {r}")).unwrap_or_default();
                return Some(format!("Couldn't remove session {}{on}, left unfinished by an earlier build: {}.{fix}", entry.id, e.message));
            }
            if let Ok(bytes) = serde_json::to_vec(&entry) {
                let _ = file.set_len(0).and_then(|_| file.rewind()).and_then(|_| file.write_all(&bytes));
            }
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli_agents::adapters::AgentAdapter;
    use craft_anything_evidence::{ErrorCode, ToolError};
    use craft_anything_platform::sync::Mutex;
    use std::sync::{Arc, PoisonError};

    const OWNER_ENV: &str = "CRAFT_ANYTHING_TEST_UNFINISHED_OWNER";
    /// What the stub was asked to delete: program, session, host.
    type Asked = Arc<Mutex<Vec<(String, String, Option<String>)>>>;

    /// Records what it was asked to delete; fails when told to.
    struct Stub {
        fail: bool,
        asked: Asked,
    }
    impl AgentAdapter for Stub {
        /// Register the fixture without replacing any built-in agent.
        fn ids(&self) -> &'static [&'static str] {
            &["test-sweep"]
        }
        /// It stands in for an agent whose sessions are deleted.
        fn discards_sessions(&self) -> bool {
            true
        }
        /// Never run: the sweep only asks it to delete.
        fn args(&self, _cx: &crate::cli_agents::adapters::ArgsCx) -> Vec<String> {
            Vec::new()
        }
        fn discard_unfinished(&self, program: &str, id: &str, host: Option<&str>) -> Result<(), ToolError> {
            self.asked.lock().unwrap_or_else(PoisonError::into_inner).push((program.into(), id.into(), host.map(str::to_string)));
            if self.fail { Err(ToolError::new(ErrorCode::ProgramFailed, "it said no").with_remediation("Do it by hand.")) } else { Ok(()) }
        }
    }

    fn adapters(fail: bool) -> (AgentAdapters, Asked) {
        let asked = Arc::new(Mutex::new(Vec::new()));
        let mut a = AgentAdapters::default();
        a.register(Arc::new(Stub { fail, asked: asked.clone() }));
        (a, asked)
    }

    fn records(dir: &Path) -> usize {
        std::fs::read_dir(dir).map(|rd| rd.flatten().count()).unwrap_or(0)
    }

    /// The owner of `a_killed_turns_session_is_deleted_by_the_next_sweep`: records a session, then
    /// ends the process without cleaning up, as a crash or a kill does. Run as an ordinary test, it
    /// does nothing.
    #[test]
    fn killed_owner_child() {
        let Some(dir) = std::env::var_os(OWNER_ENV) else { return };
        let held = Held::record(Path::new(&dir), &Entry::new("test-sweep", "agent-cli", Some("studio"), "ses_killed"));
        assert!(held.is_ok(), "{held:?}");
        std::process::exit(0);
    }

    #[test]
    fn a_killed_turns_session_is_deleted_by_the_next_sweep() {
        let d = tempfile::tempdir().unwrap();
        let dir = dir_in(d.path());
        let run = craft_anything_platform::process::Process::new(std::env::current_exe().unwrap().as_os_str())
            .args(["--exact", "--test-threads=1", "cli_agents::unfinished::tests::killed_owner_child"])
            .env(OWNER_ENV, dir.as_os_str())
            .timeout(std::time::Duration::from_secs(60))
            .run();
        assert!(run.as_ref().is_ok_and(|o| o.success), "{run:?}");
        assert_eq!(records(&dir), 1, "the owner left its record");
        let (a, asked) = adapters(false);
        let said = sweep(&dir, &a);
        assert_eq!(*asked.lock().unwrap_or_else(PoisonError::into_inner), [("agent-cli".into(), "ses_killed".into(), Some("studio".into()))]);
        assert_eq!(said.len(), 1, "{said:?}");
        assert_eq!(records(&dir), 0);
    }

    /// The desktop app's start-up sweep finds the records under its config folder's logs and
    /// deletes what the previous process left behind; with no folder it says nothing.
    #[test]
    fn a_startup_sweep_deletes_leftovers_in_the_config_folder() {
        let d = tempfile::tempdir().unwrap();
        let (a, asked) = adapters(false);
        assert!(sweep_at_startup(d.path(), &a).is_empty(), "nothing there yet");
        let records_dir = dir_in(&crate::failure_log::dir_in(d.path()));
        drop(Held::record(&records_dir, &Entry::new("test-sweep", "agent-cli", None, "ses_startup")).unwrap());
        // A sweep can find the record still locked while another test's child runs, and count no
        // try: sweep until the delete is asked for.
        let mut said = Vec::new();
        for _ in 0..100 {
            said = sweep_at_startup(d.path(), &a);
            if !said.is_empty() {
                break;
            }
            assert_eq!(records(&records_dir), 1, "kept for the next sweep");
        }
        assert_eq!(*asked.lock().unwrap_or_else(PoisonError::into_inner), [("agent-cli".into(), "ses_startup".into(), None)]);
        assert_eq!(said.len(), 1, "{said:?}");
        assert_eq!(records(&records_dir), 0);
    }

    /// A delete that fails at start-up keeps its record for the next sweep, as any sweep does.
    #[test]
    fn a_startup_sweep_keeps_a_failed_delete_for_the_next_sweep() {
        let d = tempfile::tempdir().unwrap();
        let (a, asked) = adapters(true);
        let records_dir = dir_in(&crate::failure_log::dir_in(d.path()));
        drop(Held::record(&records_dir, &Entry::new("test-sweep", "agent-cli", None, "ses_stuck_startup")).unwrap());
        for _ in 0..100 {
            if sweep_at_startup(d.path(), &a).is_empty() {
                let asked = asked.lock().unwrap_or_else(PoisonError::into_inner);
                if asked.len() == 1 {
                    break;
                }
            }
            assert_eq!(records(&records_dir), 1, "kept for the next sweep");
        }
        assert_eq!(asked.lock().unwrap_or_else(PoisonError::into_inner).len(), 1, "one try isn't a give-up yet");
        assert_eq!(records(&records_dir), 1, "kept for the next sweep");
    }

    #[test]
    fn a_running_turns_record_is_left_alone_and_done_removes_it() {
        let d = tempfile::tempdir().unwrap();
        let (a, asked) = adapters(false);
        let held = Held::record(d.path(), &Entry::new("test-sweep", "agent-cli", None, "ses_live")).unwrap();
        assert!(sweep(d.path(), &a).is_empty());
        assert!(asked.lock().unwrap_or_else(PoisonError::into_inner).is_empty(), "a held record isn't swept");
        held.done();
        assert_eq!(records(d.path()), 0);
        assert!(sweep(d.path(), &a).is_empty());
        assert!(asked.lock().unwrap_or_else(PoisonError::into_inner).is_empty());
    }

    #[test]
    fn a_failing_delete_is_tried_again_then_given_up_on() {
        let d = tempfile::tempdir().unwrap();
        let (a, asked) = adapters(true);
        // Dropped without `done`: its lock goes, as when the process ends.
        drop(Held::record(d.path(), &Entry::new("test-sweep", "agent-cli", None, "ses_stuck")).unwrap());
        // A sweep can find the record still locked: a program another test starts holds a copy of
        // this process's open files until it runs, and that sweep counts no try. So sweep until it
        // gives up, and count the tries it asked for.
        let mut said = Vec::new();
        for _ in 0..100 {
            said = sweep(d.path(), &a);
            if !said.is_empty() {
                break;
            }
            assert_eq!(records(d.path()), 1, "kept for the next sweep");
        }
        assert!(said.len() == 1 && said[0].contains("ses_stuck") && said[0].contains("Do it by hand."), "{said:?}");
        assert_eq!(records(d.path()), 0);
        assert_eq!(asked.lock().unwrap_or_else(PoisonError::into_inner).len(), TRIES as usize);
    }

    #[test]
    fn a_record_it_cant_read_or_act_on_is_left_as_it_is() {
        let d = tempfile::tempdir().unwrap();
        std::fs::write(d.path().join("x.json"), "{\"version\":99}").unwrap();
        std::fs::write(d.path().join("y.json"), "not json").unwrap();
        // Agents whose adapter here deletes nothing: one no adapter knows (the plain adapter), and
        // a built-in one a fork's adapter may have recorded under its id.
        drop(Held::record(d.path(), &Entry::new("no-such-agent", "agent-cli", None, "ses_other")).unwrap());
        drop(Held::record(d.path(), &Entry::new("claude", "agent-cli", None, "ses_fork")).unwrap());
        let (a, asked) = adapters(false);
        assert!(sweep(d.path(), &a).is_empty());
        assert!(asked.lock().unwrap_or_else(PoisonError::into_inner).is_empty());
        assert_eq!(records(d.path()), 4);
    }

    /// A sweep that opened a record just before its turn finished takes the lock once the turn
    /// lets go, and must not delete the finished session.
    #[test]
    fn a_record_retired_while_a_sweep_had_it_open_is_left_alone() {
        let d = tempfile::tempdir().unwrap();
        let (a, asked) = adapters(false);
        let held = Held::record(d.path(), &Entry::new("test-sweep", "agent-cli", None, "ses_done")).unwrap();
        let path = std::fs::read_dir(d.path()).unwrap().flatten().next().unwrap().path();
        let early = OpenOptions::new().read(true).write(true).open(&path).unwrap();
        held.done();
        assert_eq!(sweep_one(&path, early, &a), None);
        assert!(asked.lock().unwrap_or_else(PoisonError::into_inner).is_empty(), "a finished session isn't deleted");
        assert_eq!(records(d.path()), 0);

        // The same when emptying it failed and the record was only removed.
        let held = Held::record(d.path(), &Entry::new("test-sweep", "agent-cli", None, "ses_done2")).unwrap();
        let early = OpenOptions::new().read(true).write(true).open(&held.path).unwrap();
        let path = held.path.clone();
        std::fs::remove_file(&held.path).unwrap();
        drop(held);
        assert_eq!(sweep_one(&path, early, &a), None);
        assert!(asked.lock().unwrap_or_else(PoisonError::into_inner).is_empty(), "a removed record isn't acted on");
    }
}
