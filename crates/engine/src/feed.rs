//! A project's conversation, as data: what the person asked, what the builder did and said, its
//! checklist, questions and checks. It's kept in the project folder (`feed.json`); a rebuild run
//! appends to it as it goes (`ai::rebuild::feed`), and every client reads the same file.

use crate::inbox::{Inbox, Request};
use crate::workspace::{CheckResult, Status, Step, Workspace};
use craft_anything_evidence::{ErrorCode, ToolError};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// Where the conversation is kept in the project folder.
pub const FEED_FILE: &str = "feed.json";

/// The format written today (5 marks open plan steps stopped at a run's end; 4 saves a dismissed
/// plan card with it; 3 saved a prompt's pictures); files without a version are the earlier bare
/// list. An older Craft Anything won't rewrite a file it can't read (`save` refuses a newer version).
pub const FEED_VERSION: u32 = 5;

/// What the person sent: as the feed shows it, and the pictures attached to it (their copies in
/// the project's `attachments/`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum UserItem {
    /// No pictures: saved as a plain string, as every earlier feed was.
    Text(String),
    WithPictures {
        text: String,
        pictures: Vec<PathBuf>,
    },
}

impl UserItem {
    pub fn new(text: String, pictures: Vec<PathBuf>) -> Self {
        if pictures.is_empty() { UserItem::Text(text) } else { UserItem::WithPictures { text, pictures } }
    }

    pub fn text(&self) -> &str {
        match self {
            UserItem::Text(text) | UserItem::WithPictures { text, .. } => text,
        }
    }

    /// The copies in the project's `attachments/`, in the order they were attached.
    pub fn pictures(&self) -> &[PathBuf] {
        match self {
            UserItem::Text(_) => &[],
            UserItem::WithPictures { pictures, .. } => pictures,
        }
    }
}

impl From<String> for UserItem {
    fn from(text: String) -> Self {
        UserItem::Text(text)
    }
}

/// One step of the builder's own checklist (from its TodoWrite list).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlanStep {
    pub text: String,
    /// What it says while doing this step ("Drawing the map").
    pub doing: String,
    pub status: PlanStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlanStatus {
    Todo,
    Doing,
    Done,
    /// A run ended with the step still open: stopped, not done (#504).
    Stopped,
}

/// One entry in a project's activity feed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum FeedItem {
    /// The builder's checklist, kept up to date in place (shown pinned above the prompt box).
    Plan(Vec<PlanStep>),
    User(UserItem),
    Step {
        step: Step,
        text: String,
    },
    /// The agent's words (streamed).
    Agent {
        text: String,
        streaming: bool,
    },
    /// A run of tool calls, collapsed in Simple mode: (tool, finished ok?).
    Work {
        tools: Vec<(String, Option<bool>)>,
        notes: Vec<String>,
    },
    /// A question or approval; `answer` is the person's reply once given.
    Ask {
        request: Request,
        answer: Option<String>,
    },
    Check(CheckResult),
    Error {
        message: String,
        remediation: Option<String>,
        /// A log with everything known about the failure, to open and inspect.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        log: Option<std::path::PathBuf>,
    },
    /// The person paused the run (not a failure); a message or Continue carries on.
    Paused,
    /// The person folded the plan card above the prompt box (#504). It marks the plan before it as
    /// hidden; the steps are kept, and a later run's checklist shows again.
    PlanDismissed,
}

/// The plan card to show: the checklist, unless the person folded it after it was last updated.
pub fn visible_plan(feed: &[FeedItem]) -> Option<&[PlanStep]> {
    let at = feed.iter().rposition(|i| matches!(i, FeedItem::Plan(_)))?;
    if feed[at + 1..].iter().any(|i| matches!(i, FeedItem::PlanDismissed)) {
        return None;
    }
    match &feed[at] {
        FeedItem::Plan(steps) => Some(steps.as_slice()),
        _ => None,
    }
}

/// Fold the plan card in a feed. Returns false when there's no plan to fold or it's folded already.
pub fn dismiss_plan(feed: &mut Vec<FeedItem>) -> bool {
    if visible_plan(feed).is_none() {
        return false;
    }
    feed.push(FeedItem::PlanDismissed);
    true
}

/// Fold the plan card in a project's saved conversation (the UI's write, while no run is going).
pub fn dismiss_plan_saved(ws: &Workspace) -> Result<bool, ToolError> {
    let mut feed = load(ws);
    let folded = dismiss_plan(&mut feed);
    if folded {
        save(&ws.root, &feed)?;
    }
    Ok(folded)
}

/// Mark a question or approval answered, with the words shown for the answer.
pub fn answered(feed: &mut [FeedItem], id: &str, shown: &str) {
    for item in feed {
        if let FeedItem::Ask { request, answer: a @ None } = item
            && request.id == id
        {
            *a = Some(shown.to_string());
        }
    }
}

/// The saved file: `{version, items}`. Earlier files were the bare list.
#[derive(Serialize, Deserialize)]
struct Saved {
    version: u32,
    items: Vec<FeedItem>,
}

/// The feed for a project: what was saved, else rebuilt from its manifest. Questions still
/// waiting in its inbox are always there.
pub fn load(ws: &Workspace) -> Vec<FeedItem> {
    let saved = std::fs::read(ws.root.join(FEED_FILE)).ok().and_then(|b| {
        serde_json::from_slice::<Saved>(&b)
            .ok()
            .filter(|s| s.version <= FEED_VERSION)
            .map(|s| s.items)
            .or_else(|| serde_json::from_slice::<Vec<FeedItem>>(&b).ok())
    });
    let Some(mut feed) = saved else { return history(ws) };
    // Nothing is streaming any more after a restart.
    for item in &mut feed {
        if let FeedItem::Agent { streaming, .. } = item {
            *streaming = false;
        }
    }
    let shown: HashSet<String> = feed.iter().filter_map(|i| if let FeedItem::Ask { request, .. } = i { Some(request.id.clone()) } else { None }).collect();
    feed.extend(Inbox::new(ws.inbox_dir()).pending().into_iter().filter(|r| !shown.contains(&r.id)).map(|request| FeedItem::Ask { request, answer: None }));
    feed
}

/// A feed rebuilt from what a project saved: the request, the checks, the agent's last words,
/// and anything still waiting for an answer.
pub fn history(ws: &Workspace) -> Vec<FeedItem> {
    let m = &ws.manifest;
    let mut feed = vec![FeedItem::User(UserItem::new(m.request.clone(), Vec::new()))];
    feed.extend(m.checks.iter().cloned().map(FeedItem::Check));
    if !m.last_reply.is_empty() {
        feed.push(FeedItem::Agent { text: m.last_reply.clone(), streaming: false });
    }
    if matches!(m.status, Status::NeedsYou) && !m.note.is_empty() {
        feed.push(FeedItem::Error { message: m.note.clone(), remediation: None, log: None });
    }
    feed.extend(Inbox::new(ws.inbox_dir()).pending().into_iter().map(|request| FeedItem::Ask { request, answer: None }));
    feed
}

/// Write the feed into the project folder (only one that exists; never creates one). A feed
/// saved by a newer Craft Anything is never written over (docs/file-formats.md).
pub fn save(root: &Path, feed: &[FeedItem]) -> Result<(), ToolError> {
    if !root.is_dir() {
        return Ok(());
    }
    let newer = std::fs::read(root.join(FEED_FILE))
        .ok()
        .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
        .and_then(|v| v.get("version").and_then(Value::as_u64))
        .is_some_and(|v| v > u64::from(FEED_VERSION));
    if newer {
        return Err(ToolError::new(ErrorCode::NotSupported, "This project's conversation was saved by a newer Craft Anything, so it isn't changed.")
            .with_remediation("Update Craft Anything to keep its conversation up to date."));
    }
    let json = serde_json::to_vec(&Saved { version: FEED_VERSION, items: feed.to_vec() }).map_err(|e| ToolError::internal(e.to_string()))?;
    crate::project::write_atomic(&root.join(FEED_FILE), &json)
}

/// Record an answer given while no run is going (straight into the saved feed). `request` is the
/// question, taken before it was answered: one the saved conversation doesn't have yet (posted
/// from outside a run) is added with its answer, rather than lost once it stops waiting (#176).
pub fn record_answer(ws: &Workspace, request: &Request, shown: &str) -> Result<(), ToolError> {
    let mut feed = load(ws);
    if !feed.iter().any(|i| matches!(i, FeedItem::Ask { request: r, .. } if r.id == request.id)) {
        feed.push(FeedItem::Ask { request: request.clone(), answer: None });
    }
    answered(&mut feed, &request.id, shown);
    save(&ws.root, &feed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inbox::RequestKind;
    use crate::workspace::TargetRef;

    #[test]
    fn file_formats_doc_lists_current_manifest_and_feed_versions() {
        let doc = include_str!("../../../docs/file-formats.md");
        let manifest = format!("`MANIFEST_VERSION` = {} ", crate::workspace::MANIFEST_VERSION);
        let feed = format!("`FEED_VERSION` = {} ", FEED_VERSION);
        assert!(doc.contains(&manifest), "docs/file-formats.md must list {manifest}");
        assert!(doc.contains(&feed), "docs/file-formats.md must list {feed}");
    }

    #[test]
    fn history_rebuilds_from_the_manifest() {
        let d = tempfile::tempdir().unwrap();
        let mut ws = Workspace::create(d.path(), "x", "Clone it", TargetRef::None).unwrap();
        ws.manifest.checks.push(CheckResult { similarity: Some(70.0), ..CheckResult::new(CheckResult::WEB_VISUAL, 1, true, "") });
        ws.manifest.last_reply = "Built a page.".into();
        ws.save().unwrap();
        Inbox::new(ws.inbox_dir()).post(RequestKind::Question, "Blue or red?", "", vec![], true, None).unwrap();
        let f = history(&ws);
        assert!(matches!(&f[0], FeedItem::User(r) if r.text() == "Clone it"));
        assert!(matches!(f.last(), Some(FeedItem::Ask { answer: None, .. })), "pending questions come back");
        assert_eq!(f.len(), 4);
    }

    #[test]
    fn a_prompt_keeps_its_pictures_with_it() {
        let d = tempfile::tempdir().unwrap();
        let ws = Workspace::create(d.path(), "x", "Clone it", TargetRef::None).unwrap();
        let pictures = vec![ws.root.join("attachments/picture-1.png"), ws.root.join("attachments/picture-2.webp")];
        save(&ws.root, &[FeedItem::User(UserItem::new("Make it blue".into(), pictures.clone()))]).unwrap();
        let loaded = load(&ws);
        assert!(matches!(&loaded[0], FeedItem::User(u) if u.text() == "Make it blue" && u.pictures() == pictures.as_slice()));
        // A prompt with no pictures is still saved as the plain string older feeds hold.
        save(&ws.root, &[FeedItem::User(UserItem::new("Add tags".into(), Vec::new()))]).unwrap();
        let raw: Value = serde_json::from_slice(&std::fs::read(ws.root.join(FEED_FILE)).unwrap()).unwrap();
        assert_eq!(raw["items"][0]["User"], "Add tags");
        assert!(raw["items"][0]["User"].is_string());
    }

    /// #504: a plan card can be folded, the steps stay in the conversation, and a folded plan stays
    /// folded after a restart. A file from before folding (version 3) still shows its plan.
    #[test]
    fn a_folded_plan_stays_folded_and_its_steps_are_kept() {
        let d = tempfile::tempdir().unwrap();
        let ws = Workspace::create(d.path(), "x", "A game", TargetRef::None).unwrap();
        let steps = vec![PlanStep { text: "Lay out the arena".into(), doing: "Laying out the arena".into(), status: PlanStatus::Doing }];
        save(&ws.root, &[FeedItem::User(UserItem::Text("A game".into())), FeedItem::Plan(steps.clone())]).unwrap();
        assert!(visible_plan(&load(&ws)).is_some(), "the plan shows before it's folded");
        assert!(dismiss_plan_saved(&ws).unwrap(), "the plan folds");
        let folded = load(&ws);
        assert!(visible_plan(&folded).is_none(), "folded after a restart too");
        assert!(folded.iter().any(|i| matches!(i, FeedItem::Plan(p) if p == &steps)), "the steps are kept");
        assert!(!dismiss_plan_saved(&ws).unwrap(), "folding again changes nothing");
        save(&ws.root, &[FeedItem::User(UserItem::Text("Hi".into()))]).unwrap();
        assert!(!dismiss_plan_saved(&ws).unwrap(), "a conversation with no plan has nothing to fold");
        std::fs::write(ws.root.join(FEED_FILE), br#"{"version":3,"items":[{"Plan":[{"text":"Lay out","doing":"Laying out","status":"Doing"}]}]}"#).unwrap();
        assert!(visible_plan(&load(&ws)).is_some(), "a version 3 plan still shows");
    }

    /// #504: a step marked stopped when a run ended stays in the saved conversation (it's part of
    /// the plan card's data, not a display flourish).
    #[test]
    fn a_stopped_step_is_kept_across_saves() {
        let d = tempfile::tempdir().unwrap();
        let ws = Workspace::create(d.path(), "x", "A game", TargetRef::None).unwrap();
        let steps = vec![
            PlanStep { text: "Lay out the arena".into(), doing: "Laying out the arena".into(), status: PlanStatus::Done },
            PlanStep { text: "Add the snake".into(), doing: "Adding the snake".into(), status: PlanStatus::Stopped },
            PlanStep { text: "Keep score".into(), doing: "Keeping score".into(), status: PlanStatus::Todo },
        ];
        save(&ws.root, &[FeedItem::User(UserItem::Text("A game".into())), FeedItem::Plan(steps)]).unwrap();
        let loaded = load(&ws);
        assert!(matches!(visible_plan(&loaded), Some(p) if p.iter().any(|s| s.status == PlanStatus::Stopped)));
    }

    #[test]
    fn older_files_load_answers_are_kept_and_newer_files_are_left_alone() {
        let d = tempfile::tempdir().unwrap();
        let ws = Workspace::create(d.path(), "x", "Clone it", TargetRef::None).unwrap();
        save(&ws.root, &[FeedItem::User(UserItem::Text("Clone it".into())), FeedItem::Agent { text: "Done.".into(), streaming: true }]).unwrap();
        assert!(matches!(load(&ws).as_slice(), [FeedItem::User(_), FeedItem::Agent { streaming: false, .. }]), "nothing streams after a restart");
        let raw: Value = serde_json::from_slice(&std::fs::read(ws.root.join(FEED_FILE)).unwrap()).unwrap();
        assert_eq!(raw["version"], FEED_VERSION);
        // A feed saved before versioning (the bare list) still loads.
        std::fs::write(ws.root.join(FEED_FILE), br#"[{"User":"Old request"}]"#).unwrap();
        assert!(matches!(load(&ws).as_slice(), [FeedItem::User(r)] if r.text() == "Old request"));
        // Answers given with no run going are saved too.
        let inbox = Inbox::new(ws.inbox_dir());
        let id = inbox.post(RequestKind::Question, "Blue or red?", "", vec![], true, None).unwrap();
        let question = inbox.pending().into_iter().find(|r| r.id == id).unwrap();
        inbox.answer(&id, &crate::inbox::Answer { choice: Some("Blue".into()), ..Default::default() }).unwrap();
        record_answer(&ws, &question, "Blue").unwrap();
        assert!(std::fs::read_to_string(ws.root.join(FEED_FILE)).unwrap().contains("\"Blue\""));
        // #176: one the saved conversation never had (posted from outside a run, answered before
        // anything saved it) is kept with its answer, once, and still after the inbox forgets it.
        save(&ws.root, &[FeedItem::User(UserItem::Text("Clone it".into()))]).unwrap();
        let id = inbox.post(RequestKind::Question, "Which scope?", "", vec![], true, None).unwrap();
        let question = inbox.pending().into_iter().find(|r| r.id == id).unwrap();
        inbox.answer(&id, &crate::inbox::Answer { choice: Some("Minimal".into()), ..Default::default() }).unwrap();
        let _ = std::fs::remove_file(inbox.dir().join(format!("{id}.request.json")));
        let _ = std::fs::remove_file(inbox.dir().join(format!("{id}.answer.json")));
        record_answer(&ws, &question, "Minimal").unwrap();
        let asks = |f: &[FeedItem]| f.iter().filter(|i| matches!(i, FeedItem::Ask { request, answer: Some(a) } if request.id == id && a == "Minimal")).count();
        assert_eq!(asks(&load(&ws)), 1, "kept with its answer");
        record_answer(&ws, &question, "Minimal").unwrap();
        assert_eq!(asks(&load(&ws)), 1, "not twice");
        // One from a newer Craft Anything is shown as far as we can, never written over.
        let newer = br#"{"version":99,"items":[{"User":"From the future"}],"extra":1}"#;
        std::fs::write(ws.root.join(FEED_FILE), newer).unwrap();
        assert!(save(&ws.root, &[FeedItem::User(UserItem::Text("x".into()))]).is_err());
        assert_eq!(std::fs::read(ws.root.join(FEED_FILE)).unwrap(), newer);
    }
}
