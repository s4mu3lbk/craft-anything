//! A start that never reached a window (#74, #104, #33).
//!
//! A graphics driver that crashes natively as the window opens takes the whole process with it:
//! no Rust code runs afterwards, so no dialog and no log line can say what happened, and the app
//! just vanishes. What can be done is to leave a mark before trying and remove it once the window
//! is open and has shown its first frames (the driver may crash while presenting them, after
//! eframe's setup callback has run); a mark found at the next start means the last one never got
//! there, and the app draws another way (see `main` and `renderer`). A start that does open the
//! window clears the mark.
//!
//! A start holds a lock on its mark, which the system lets go of when the process dies. A mark
//! still locked is a start under way (the icon double-clicked), not one that failed: on Windows
//! that would move the app to another renderer for good.

use craft_anything_platform::sync::Mutex;
use std::fs::{File, OpenOptions, TryLockError};
use std::path::{Path, PathBuf};
use std::sync::PoisonError;

/// Beside the settings, with `desktop.log`.
const MARKER: &str = "starting";

/// This start's mark, kept open to keep its lock.
static HELD: Mutex<Option<File>> = Mutex::new(None);

fn marker(dir: &Path) -> PathBuf {
    dir.join(MARKER)
}

/// Whether the last start ended before its window opened.
pub fn last_start_failed(dir: &Path) -> bool {
    File::open(marker(dir)).is_ok_and(|mark| !matches!(mark.try_lock(), Err(TryLockError::WouldBlock)))
}

/// Called right before the window is asked for. Best effort: a folder that can't be written to
/// only means no fallback.
pub fn begin(dir: &Path) {
    let _ = std::fs::create_dir_all(dir);
    // Not truncated: the mark may be locked by a start that is still going.
    let Ok(mark) = OpenOptions::new().write(true).create(true).truncate(false).open(marker(dir)) else { return };
    if mark.try_lock().is_ok() {
        *HELD.lock().unwrap_or_else(PoisonError::into_inner) = Some(mark);
    }
}

/// Lets go of the mark but leaves it, as dying would: for a start that hands over to another.
#[cfg(any(windows, test))]
pub fn release() {
    HELD.lock().unwrap_or_else(PoisonError::into_inner).take();
}

/// Called once the window is open.
pub fn opened(dir: &Path) {
    HELD.lock().unwrap_or_else(PoisonError::into_inner).take();
    let _ = std::fs::remove_file(marker(dir));
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One test at a time: they share this process's held mark, and one's `opened()` would let
    /// go of another's lock.
    static ONE_AT_A_TIME: Mutex<()> = Mutex::new(());

    /// #74: a start that dies before its window leaves the mark; one that opens it doesn't.
    #[test]
    fn a_start_that_never_opened_a_window_is_seen_by_the_next_one() {
        let _one = ONE_AT_A_TIME.lock().unwrap_or_else(PoisonError::into_inner);
        let d = tempfile::tempdir().unwrap();
        let dir = d.path().join("config");
        assert!(!last_start_failed(&dir), "a first start has nothing to retry");
        begin(&dir);
        assert!(!last_start_failed(&dir), "a second launch while this one opens is no failure");
        release();
        assert!(last_start_failed(&dir), "the process died before opened()");
        begin(&dir);
        release();
        assert!(last_start_failed(&dir), "a second start in a row still sees it");
        opened(&dir);
        assert!(!last_start_failed(&dir), "a window that opened clears it");
        opened(&dir);
        assert!(!last_start_failed(&dir), "clearing twice is fine");
    }

    #[test]
    fn a_folder_that_cannot_be_written_means_no_fallback_and_no_crash() {
        let _one = ONE_AT_A_TIME.lock().unwrap_or_else(PoisonError::into_inner);
        let d = tempfile::tempdir().unwrap();
        let file = d.path().join("not-a-folder");
        std::fs::write(&file, "").unwrap();
        begin(&file);
        assert!(!last_start_failed(&file));
        opened(&file);
    }
}
