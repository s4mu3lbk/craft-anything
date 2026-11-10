//! Context management for long agent conversations (R2.1): old tool results are trimmed, then the
//! oldest part is replaced by a summary when the conversation still doesn't fit, and the whole
//! conversation can be kept on disk (`transcript.json`) so a later message resumes it.
//!
//! Every edit keeps the shape providers require: the first message is the person's, each tool call
//! keeps its result, and the most recent messages are never touched.

use crate::model::{Block, Msg, Speaker};
use craft_anything_evidence::ToolError;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Messages at the end that trimming leaves alone (the current step and a few before it).
pub const KEEP_RECENT: usize = 12;
/// A trimmed tool result keeps this many characters.
const TRIMMED_KEEP: usize = 400;
/// What a cloud model's conversation may grow to (input tokens) before it is trimmed.
pub const CLOUD_CONTEXT_TOKENS: u64 = 150_000;
/// The same for a local model, whose context is usually small.
pub const LOCAL_CONTEXT_TOKENS: u64 = 24_000;
/// The version of `transcript.json`.
pub const TRANSCRIPT_VERSION: u32 = 2;

/// What the model is asked when the oldest part of a conversation is replaced.
pub const SUMMARY_REQUEST: &str = "The conversation is getting too long to continue. Write a summary of the work so far that you can carry on from without the earlier messages: the task, what has been done (files made or changed, commands that worked), decisions and the reasons for them, what is left to do, and anything the person asked for. Be specific and complete; leave out pleasantries. Don't call any tools.";

/// About how many tokens a conversation takes (4 characters a token), for when no turn has
/// reported its real size yet.
pub fn estimate_tokens(history: &[Msg]) -> u64 {
    let chars: usize = history
        .iter()
        .flat_map(|m| &m.blocks)
        .map(|b| match b {
            Block::Text { text } => text.len(),
            Block::Thinking { thinking, .. } => thinking.len(),
            Block::RedactedThinking { data } => data.len(),
            Block::ToolUse { input, name, .. } => name.len() + input.to_string().len(),
            Block::ToolExtra { extra, .. } => extra.to_string().len(),
            Block::ToolResult { content, .. } => content.len(),
        })
        .sum();
    (chars / 4) as u64
}

/// About how many tokens go with every turn besides the conversation: the system prompt, and each
/// tool's name, description and schema (4 characters a token, as [`estimate_tokens`]). A server that
/// reports no usage leaves these out of its count, so they are counted from their text (#731).
pub fn estimate_overhead(system: &str, tools: &[crate::model::ToolDef]) -> u64 {
    let tool_chars: usize = tools.iter().map(|t| t.name.len() + t.description.len() + t.schema.to_string().len()).sum();
    ((system.len() + tool_chars) / 4) as u64
}

/// Shorten tool results and drop reasoning in all but the last [`KEEP_RECENT`] messages. Returns
/// how many blocks changed.
pub fn trim_old(history: &mut [Msg]) -> usize {
    let end = history.len().saturating_sub(KEEP_RECENT);
    let mut changed = 0;
    for m in history.iter_mut().take(end) {
        let before = m.blocks.len();
        // Earlier turns' reasoning isn't needed again (providers ignore it outside the current step).
        m.blocks.retain(|b| !matches!(b, Block::Thinking { .. } | Block::RedactedThinking { .. }));
        changed += before - m.blocks.len();
        for b in &mut m.blocks {
            if let Block::ToolResult { content, .. } = b
                && content.len() > TRIMMED_KEEP + 100
                && !content.ends_with(TRIMMED_NOTE)
            {
                let mut cut = TRIMMED_KEEP;
                while !content.is_char_boundary(cut) {
                    cut -= 1;
                }
                content.truncate(cut);
                content.push_str(TRIMMED_NOTE);
                changed += 1;
            }
        }
    }
    changed
}

const TRIMMED_NOTE: &str = "… (trimmed to save room; run the tool again if you need the rest)";

/// Where to split a conversation so the part before it can be summarised: an assistant message
/// with about the last third after it. `None` when fewer than three messages would be summarised
/// (one is usually the previous summary, so there's nothing to gain).
pub fn cut_point(history: &[Msg]) -> Option<usize> {
    let start = history.len().saturating_sub(history.len() / 3).max(1);
    (MIN_SUMMARISED..=start.min(history.len().saturating_sub(2))).rev().find(|&i| history[i].role == Speaker::Assistant)
}

/// Fewest messages worth replacing by a summary.
const MIN_SUMMARISED: usize = 3;

/// The conversation with everything before `cut` replaced by `summary`.
pub fn summarised(history: &[Msg], cut: usize, summary: &str) -> Vec<Msg> {
    let mut out =
        vec![Msg::user_text(format!("(Earlier messages were replaced by this summary to save room.)\n\n{}\n\nContinue the task from here.", summary.trim()))];
    out.extend(history.iter().skip(cut).cloned());
    out
}

/// The conversation plus the summary request, appended to the last message when that is the
/// person's (two in a row aren't allowed everywhere).
pub fn with_summary_request(history: &[Msg]) -> Vec<Msg> {
    let mut v = history.to_vec();
    match v.last_mut() {
        Some(m) if m.role == Speaker::User => m.blocks.push(Block::Text { text: SUMMARY_REQUEST.into() }),
        _ => v.push(Msg::user_text(SUMMARY_REQUEST)),
    }
    v
}

/// Add the person's text: appended to the last message when that is already theirs (a resumed
/// conversation can end with tool results).
pub fn push_user_text(history: &mut Vec<Msg>, text: &str) {
    match history.last_mut() {
        Some(m) if m.role == Speaker::User => m.blocks.push(Block::Text { text: text.into() }),
        _ => history.push(Msg::user_text(text)),
    }
}

#[derive(Serialize, Deserialize)]
struct Transcript {
    version: u32,
    /// The mode the conversation ran in (`true`: Deep). A transcript saved before this field was
    /// kept counts as Deep: resuming it in Clean room could carry what it read there.
    #[serde(default = "deep_by_default")]
    deep: bool,
    history: Vec<Msg>,
}

/// A transcript saved before the mode was kept may have read the original, so it counts as Deep.
fn deep_by_default() -> bool {
    true
}

/// Write the conversation to `path`, recording the mode it ran in.
pub fn save(path: &Path, history: &[Msg], deep: bool) -> Result<(), ToolError> {
    let text = serde_json::to_vec(&Transcript { version: TRANSCRIPT_VERSION, deep, history: history.to_vec() })
        .map_err(|e| ToolError::internal(format!("couldn't write the transcript: {e}")))?;
    craft_anything_engine::project::write_atomic(path, &text)
}

/// A saved conversation, ready to continue in `deep` mode: `None` when there is none, it's
/// unreadable, from a newer version, or from the other mode. An unfinished step at the end (tool
/// calls without results) is dropped.
pub fn load(path: &Path, deep: bool) -> Option<Vec<Msg>> {
    let t: Transcript = serde_json::from_slice(&std::fs::read(path).ok()?).ok()?;
    if t.version > TRANSCRIPT_VERSION || t.deep != deep {
        return None;
    }
    let mut h = t.history;
    while h.last().is_some_and(|m| m.role == Speaker::Assistant && !m.tool_uses().is_empty()) {
        h.pop();
    }
    if h.first().is_none_or(|m| m.role != Speaker::User) {
        return None;
    }
    Some(h)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn step(i: usize, result: &str) -> [Msg; 2] {
        let id = format!("t{i}");
        [
            Msg {
                role: Speaker::Assistant,
                blocks: vec![
                    Block::Thinking { thinking: "hmm".into(), signature: "s".into() },
                    Block::ToolUse { id: id.clone(), name: "build_file_read".into(), input: json!({"path": "a"}) },
                ],
            },
            Msg { role: Speaker::User, blocks: vec![Block::ToolResult { tool_use_id: id, content: result.into(), is_error: false }] },
        ]
    }

    fn convo(steps: usize) -> Vec<Msg> {
        let mut h = vec![Msg::user_text("Build it")];
        for i in 0..steps {
            h.extend(step(i, &"x".repeat(5000)));
        }
        h
    }

    /// Every tool call has its result right after it, and the first message is the person's.
    fn well_formed(h: &[Msg]) -> bool {
        h.first().is_some_and(|m| m.role == Speaker::User)
            && h.iter().enumerate().all(|(i, m)| {
                m.tool_uses().iter().all(|(id, _, _)| {
                    h.get(i + 1).is_some_and(|n| n.blocks.iter().any(|b| matches!(b, Block::ToolResult { tool_use_id, .. } if tool_use_id == id)))
                })
            })
    }

    #[test]
    fn trimming_keeps_recent_messages_and_every_pair() {
        let mut h = convo(20);
        let before = estimate_tokens(&h);
        assert!(trim_old(&mut h) > 0);
        assert!(estimate_tokens(&h) < before / 2, "much smaller");
        assert!(well_formed(&h));
        let recent = &h[h.len() - KEEP_RECENT..];
        assert!(recent.iter().any(|m| m.blocks.iter().any(|b| matches!(b, Block::Thinking { .. }))), "recent reasoning stays");
        assert!(recent.iter().all(|m| m.blocks.iter().all(|b| !matches!(b, Block::ToolResult { content, .. } if content.len() < 5000))));
        assert_eq!(trim_old(&mut h), 0, "trimming twice changes nothing");
    }

    #[test]
    fn a_summary_replaces_the_oldest_part() {
        let h = convo(9);
        let cut = cut_point(&h).unwrap();
        assert_eq!(h[cut].role, Speaker::Assistant);
        let s = summarised(&h, cut, "Made index.html.");
        assert!(s.len() < h.len() && s[0].text().contains("Made index.html.") && well_formed(&s));
        assert!(cut_point(&[Msg::user_text("hi")]).is_none());
        assert!(cut_point(&convo(1)).is_none(), "too little to summarise");
        assert!(cut_point(&s).is_none_or(|c| c >= 3), "a summary isn't summarised again on its own");
        // The request joins the last message when that is the person's.
        let r = with_summary_request(&h);
        assert_eq!(r.len(), h.len());
        assert!(r.last().unwrap().text().contains("summary"));
    }

    /// A conversation continues with its unfinished last step dropped; a newer or unreadable
    /// file is left alone.
    #[test]
    fn transcripts_resume_without_an_unfinished_step() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("transcript.json");
        assert!(load(&p, true).is_none());
        let mut h = convo(2);
        h.push(step(9, "")[0].clone());
        save(&p, &h, true).unwrap();
        let back = load(&p, true).unwrap();
        assert_eq!(back.len(), h.len() - 1, "the call without a result is dropped");
        assert!(well_formed(&back));
        let mut back = back;
        push_user_text(&mut back, "Make it blue");
        assert_eq!(back.len(), h.len() - 1, "joined to the last tool results");
        std::fs::write(&p, b"not a transcript").unwrap();
        assert!(load(&p, true).is_none(), "an unreadable transcript is left alone");
        std::fs::write(&p, r#"{"version":99,"deep":true,"history":[]}"#).unwrap();
        assert!(load(&p, true).is_none(), "a newer version is left alone");
    }

    /// R2.9: a transcript carries the mode it ran in, so Clean room can't continue a Deep one.
    #[test]
    fn a_transcript_is_resumed_only_in_the_mode_that_saved_it() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("transcript.json");
        let h = convo(1);
        save(&p, &h, true).unwrap();
        assert!(load(&p, true).is_some(), "the mode that saved it continues it");
        assert!(load(&p, false).is_none(), "a Deep transcript isn't resumed in Clean room");
        save(&p, &h, false).unwrap();
        assert!(load(&p, false).is_some(), "a Clean room transcript continues there");
        assert!(load(&p, true).is_none(), "and isn't resumed in Deep");
    }

    /// A version-1 transcript was saved before the mode was kept: it counts as Deep, so only a
    /// Deep run resumes it.
    #[test]
    fn a_transcript_saved_before_the_mode_was_kept_is_only_resumed_in_deep() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("transcript.json");
        let text = serde_json::to_vec(&json!({"version": 1, "history": convo(1)})).unwrap();
        std::fs::write(&p, text).unwrap();
        assert!(load(&p, true).is_some());
        assert!(load(&p, false).is_none(), "an old transcript may have read the original");
    }
}
