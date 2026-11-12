//! Games and apps decompiled in Deep mode keep their values in `app/MECHANICS.md` for people
//! and `app/mechanics.json` for checks. Reading the sheet reports its values and recorded provenance,
//! not effective behavior: source literals cannot show which values a running build uses.

use super::*;
use craft_anything_engine::mechanics::{self, SheetProblem};

/// What every game brief asks for, so the check can read the numbers.
pub const MECHANICS_CONTRACT: &str = "Keep the same values in app/mechanics.json, which Craft Anything reads after each build: {\"version\": 1, \"values\": [{\"name\": \"walk speed\", \"group\": \"movement\", \"value\": 4.5, \"unit\": \"m/s\", \"source\": \"function 0x140012a0\"}, {\"name\": \"crouch\", \"group\": \"controls\", \"value\": \"hold\", \"estimated\": true}]}. A value is a number, text or true/false; give each one its source or \"estimated\": true, and implement the sheet's behavior. Equivalent calculations and unit conversions are allowed. Craft Anything reads the sheet but does not yet measure runtime behavior against its values; literal presence does not verify their use.";

/// At most this many names are listed in a note.
const LISTED: usize = 8;
pub struct MechanicsCheck;

impl MechanicsCheck {
    pub const ID: &'static str = "mechanics";
}

impl Check for MechanicsCheck {
    fn id(&self) -> &'static str {
        Self::ID
    }

    fn can_refine(&self, result: &CheckResult) -> bool {
        !result.passed
    }

    fn run(&self, cx: &mut CheckCx) -> Result<CheckResult, ToolError> {
        let round = cx.round;
        let sheet = match mechanics::read(&cx.ws.mechanics_path()) {
            Ok(s) => s,
            Err(p) => return Ok(CheckResult::new(self.id(), round, false, format!("{} {MECHANICS_CONTRACT}", problem_note(&p)))),
        };
        let estimated = sheet.values.iter().filter(|v| v.estimated).count();
        let sourced = sheet.values.iter().filter(|v| !v.source.trim().is_empty()).count();
        let mut note = format!("app/mechanics.json has {} values ({sourced} with a source, {estimated} estimated).", sheet.values.len());
        let unsourced: Vec<&str> = sheet.values.iter().filter(|v| v.unsourced()).map(|v| v.name.as_str()).collect();
        if !unsourced.is_empty() {
            note.push_str(&format!(" {} have neither a source nor \"estimated\": {}.", unsourced.len(), listed(&unsourced)));
        }
        note.push_str(" The sheet is readable; its behavior is unverified. No runtime measurement was compared with these values. A source literal or a core-loop self-test alone cannot establish that the build uses them.");
        Ok(CheckResult {
            unknowns: vec!["Mechanics behavior has not been measured against app/mechanics.json.".into()],
            ..CheckResult::new(self.id(), round, true, note)
        })
    }
}

fn problem_note(p: &SheetProblem) -> String {
    match p {
        SheetProblem::Missing => "There's no app/mechanics.json yet.".into(),
        SheetProblem::TooLarge => format!("app/mechanics.json is over {} KB: keep it to the values that shape play.", mechanics::MAX_BYTES / 1024),
        SheetProblem::Unreadable(why) => format!("app/mechanics.json can't be read: {why}."),
        SheetProblem::Newer(v) => format!("app/mechanics.json says version {v}; this Craft Anything reads version {}.", mechanics::MECHANICS_VERSION),
        SheetProblem::Empty => "app/mechanics.json has no values.".into(),
    }
}

/// "a, b, c and 4 more".
fn listed(names: &[&str]) -> String {
    let shown = names.iter().take(LISTED).copied().collect::<Vec<_>>().join(", ");
    match names.len().saturating_sub(LISTED) {
        0 => shown,
        more => format!("{shown} and {more} more"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notes_list_a_few_names() {
        let names: Vec<String> = (1..=10).map(|i| format!("v{i}")).collect();
        let refs: Vec<&str> = names.iter().map(String::as_str).collect();
        assert_eq!(listed(&refs), "v1, v2, v3, v4, v5, v6, v7, v8 and 2 more");
        assert_eq!(listed(&refs[..2]), "v1, v2");
    }
}
