//! Shared types and the bounded NDJSON transport for external Deep decompilers.

pub mod client;
pub mod protocol;

pub use client::Client;
pub use protocol::{DeepError, Request, Response, ct_eq};

use serde::Serialize;

/// A function as a Deep engine sees it.
#[derive(Debug, Clone, Serialize)]
pub struct DeepFunction {
    pub entry: u64,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signature: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pseudocode: Option<String>,
    /// The saved analysis had no function here; the bridge made one for this run only.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub created: bool,
}

/// An external source of decompilation, consulted only in Deep mode.
pub trait DeepDecompiler: Send + Sync {
    /// Engine id recorded in evidence (`ghidra` or `ida`).
    fn engine(&self) -> &'static str;
    fn engine_version(&self) -> Option<String>;
    /// The processor/language the engine was pinned to, when one was forced.
    fn language_id(&self) -> Option<String> {
        None
    }
    /// Where the engine loaded the program, when it said.
    fn image_base(&self) -> Option<u64> {
        None
    }
    /// Decompile the function at `entry` (an address in the engine's own space). `Ok(None)` means
    /// the engine has no better answer. `waiting` gets a line now and then while it waits (#1714).
    fn decompile(&self, entry: u64, waiting: &dyn Fn(&str)) -> Result<Option<DeepFunction>, DeepError>;
    /// List the functions the engine found.
    fn list_functions(&self) -> Result<Vec<DeepFunction>, DeepError>;
}

/// Moves addresses between our built-in engine's view and a Deep engine's view of one program.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rebase {
    ours: u64,
    theirs: u64,
}

impl Rebase {
    /// `ours`: our image base; `theirs`: the engine's, `None` when it didn't say (then no move).
    pub fn new(ours: u64, theirs: Option<u64>) -> Rebase {
        Rebase { ours, theirs: theirs.unwrap_or(ours) }
    }

    pub fn ours(&self) -> u64 {
        self.ours
    }

    pub fn theirs(&self) -> u64 {
        self.theirs
    }

    pub fn is_identity(&self) -> bool {
        self.ours == self.theirs
    }

    /// Our address as the engine knows it.
    pub fn to_engine(&self, a: u64) -> u64 {
        a.checked_sub(self.ours).and_then(|o| o.checked_add(self.theirs)).unwrap_or(a)
    }

    /// The engine's address as we know it.
    pub fn from_engine(&self, a: u64) -> u64 {
        a.checked_sub(self.theirs).and_then(|o| o.checked_add(self.ours)).unwrap_or(a)
    }
}
