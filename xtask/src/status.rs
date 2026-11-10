//! Catch conflicting edits to an area in a status table before they land together.

use std::collections::BTreeMap;
use std::path::Path;

pub fn check(root: &Path) -> bool {
    let path = root.join("docs/STATUS.md");
    let result = std::fs::read_to_string(&path).map_err(|error| error.to_string()).and_then(|text| check_tables(&text));
    if let Err(error) = result {
        eprintln!("docs/STATUS.md: {error}");
        return false;
    }
    true
}

fn check_tables(text: &str) -> Result<(), String> {
    let mut first_lines = BTreeMap::new();
    let mut in_table = false;
    for (index, line) in text.lines().enumerate() {
        let Some(row) = line.trim().strip_prefix('|') else {
            in_table = false;
            continue;
        };
        // An escaped pipe is part of the name, not the end of the first cell.
        let mut escaped = false;
        let end = row.char_indices().find_map(|(i, c)| {
            let boundary = c == '|' && !escaped;
            escaped = c == '\\' && !escaped;
            boundary.then_some(i)
        });
        let Some(name) = end.and_then(|i| row.get(..i)).map(str::trim) else { continue };
        let divider = name.trim_matches(':');
        if !in_table && divider.len() >= 3 && divider.bytes().all(|b| b == b'-') {
            first_lines.clear();
            in_table = true;
        } else if in_table && let Some(first) = first_lines.insert(name, index + 1) {
            return Err(format!("area {name:?} appears on lines {first} and {}", index + 1));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_both_locations_in_a_conflicted_table() {
        let text = "| Area | State |\n|:---|---:|\n| Paths \\| files | ready |\n| Paths \\| tools | ready |\n|Paths \\| files| fixed |";
        let error = check_tables(text).unwrap_err();
        assert!(error.contains("lines 3 and 5"), "{error}");
    }

    #[test]
    fn area_names_are_local_to_each_table() {
        let table = "| Area | State |\n|---|---|\n| Area | Ready |";
        assert_eq!(check_tables(&format!("{table}\n\n{table}")), Ok(()));
    }
}
