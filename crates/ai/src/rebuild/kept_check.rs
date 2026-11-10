//! A check round in which every check worked stands while nothing it looks at changes, so a
//! prompt that changes nothing in app/ doesn't build and start the app again (and the check
//! after the builder's turn doesn't repeat the builder's own `rebuild_check`). A round with a
//! failure is never kept: something outside the project (a program installed, a browser back)
//! may make it work, and Try again must look again.

use super::kinds::{TargetKind, plugin_ids};
use super::*;
use craft_anything_engine::workspace::progress::CheckedBuild;

/// The run's kept round, and the scan cache its digests reuse.
#[derive(Default)]
pub(super) struct KeptChecks {
    files: AppFingerprintCache,
    kept: Option<(String, Vec<CheckResult>)>,
}

impl KeptChecks {
    /// Start from the round `progress` kept, when `craft.json` still has it.
    pub(super) fn from_saved(progress: &BuildProgress, ws: &Workspace) -> KeptChecks {
        let kept = progress.checked.as_ref().and_then(|c| {
            let last = ws.manifest.checks.last()?.round;
            let round: Vec<CheckResult> = ws.manifest.checks.iter().filter(|r| r.round == c.round).cloned().collect();
            (last == c.round && all_worked(&round)).then(|| (c.key.clone(), round))
        });
        KeptChecks { files: AppFingerprintCache::default(), kept }
    }

    /// What to save for `round`: the kept key when the kept results are this round's.
    pub(super) fn saved_for(&self, round: u32) -> Option<CheckedBuild> {
        self.kept.as_ref().filter(|(_, r)| r.first().is_some_and(|c| c.round == round)).map(|(key, _)| CheckedBuild { key: key.clone(), round })
    }
}

fn all_worked(round: &[CheckResult]) -> bool {
    !round.is_empty() && round.iter().all(|c| c.passed)
}

impl Worker {
    /// `run`'s results, or the kept round's (numbered `round`) when what the checks look at
    /// hasn't changed since it ran. A digest that can't be taken just means checking again.
    pub(super) fn checks_unless_unchanged(
        &self,
        ws: &Workspace,
        kind: &dyn TargetKind,
        round: u32,
        run: impl FnOnce() -> Result<Vec<CheckResult>, ToolError>,
    ) -> Result<Vec<CheckResult>, ToolError> {
        let extra = {
            let session = self.engine.session();
            // The folder too: a copy of the project has none of the build outputs the round checked.
            let mut extra =
                vec![ws.root.display().to_string(), kind.id().to_string(), ws.manifest.stack.clone(), format!("clean_room={}", session.clean_room())];
            extra.extend(plugin_ids(&session, kind));
            extra
        };
        let extra: Vec<&str> = extra.iter().map(String::as_str).collect();
        let mut kept = self.kept_checks.lock().unwrap_or_else(PoisonError::into_inner);
        let before = ws.check_fingerprint(&mut kept.files, &extra, &self.stop).ok();
        if let (Some(now), Some((key, results))) = (&before, &kept.kept)
            && now == key
        {
            let results: Vec<CheckResult> = results.iter().map(|c| CheckResult { round, ..c.clone() }).collect();
            kept.kept = Some((key.clone(), results.clone()));
            drop(kept);
            self.emit(RunEvent::Agent(AgentEvent::Status {
                text: "Nothing the check looks at has changed since it last worked, so its results stand.".into(),
            }));
            return Ok(results);
        }
        // Checks take minutes; the lock isn't held while they run.
        drop(kept);
        let results = run()?;
        let mut kept = self.kept_checks.lock().unwrap_or_else(PoisonError::into_inner);
        // Taken after the round: a build writes files of its own that the next scan sees too.
        let after = if all_worked(&results) { ws.check_fingerprint(&mut kept.files, &extra, &self.stop).ok() } else { None };
        kept.kept = after.map(|key| (key, results.clone()));
        Ok(results)
    }
}
