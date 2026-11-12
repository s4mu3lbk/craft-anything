//! Audit literal UI lookup keys and engine tool titles against both Chinese catalogs.

use std::collections::BTreeSet;
use std::path::Path;
use syn::visit::{self, Visit};

#[derive(Default)]
struct Strings(BTreeSet<String>);

impl<'ast> Visit<'ast> for Strings {
    fn visit_expr_call(&mut self, _: &'ast syn::ExprCall) {}

    fn visit_expr_if(&mut self, expr: &'ast syn::ExprIf) {
        self.visit_block(&expr.then_branch);
        if let Some((_, otherwise)) = &expr.else_branch {
            self.visit_expr(otherwise);
        }
    }

    fn visit_lit_str(&mut self, s: &'ast syn::LitStr) {
        let value = s.value();
        if value.chars().any(|c| c.is_ascii_alphabetic()) {
            self.0.insert(value);
        }
    }
}

#[derive(Default)]
struct Lookups(Strings);

impl<'ast> Visit<'ast> for Lookups {
    fn visit_item_mod(&mut self, module: &'ast syn::ItemMod) {
        if module.ident != "tests" {
            visit::visit_item_mod(self, module);
        }
    }

    fn visit_expr_call(&mut self, call: &'ast syn::ExprCall) {
        if let syn::Expr::Path(path) = &*call.func {
            let name = path.path.segments.last().map(|s| s.ident.to_string()).unwrap_or_default();
            let owner = path.path.segments.iter().rev().nth(1).map(|s| s.ident.to_string()).unwrap_or_default();
            let positions: &[usize] = match (owner.as_str(), name.as_str()) {
                (_, "tr" | "trf") => &[0],
                ("widgets", "primary" | "link" | "suggestion" | "caption" | "pill" | "working" | "working_since" | "segmented" | "segmented_sized") => &[1],
                ("widgets", "named") => &[1],
                ("widgets", "primary_icon" | "primary_sized" | "secondary") => &[2],
                ("widgets", "toggle") => &[2],
                ("widgets", "choice_tile" | "big_card") => &[3, 4],
                ("widgets", "empty_state") => &[2, 3],
                ("icons", "button") => &[4],
                _ => &[],
            };
            for &i in positions {
                if let Some(syn::Expr::Lit(syn::ExprLit { lit: syn::Lit::Str(s), .. })) = call.args.iter().nth(i) {
                    self.0.visit_lit_str(s);
                }
                // Conditional literals in tr(if ...), and string arrays in segmented(...).
                if let Some(expr @ (syn::Expr::If(_) | syn::Expr::Array(_) | syn::Expr::Reference(_))) = call.args.iter().nth(i) {
                    self.0.visit_expr(expr);
                }
            }
        }
        visit::visit_expr_call(self, call);
    }
}

/// Compare every key the UI looks up against each catalogue, print what a catalogue is missing, and
/// report whether all of them resolve.
pub fn check(root: &Path) -> bool {
    let mut lookups = Lookups::default();
    let source = root.join("crates/ui-egui/src");
    let Ok(paths) = std::fs::read_dir(&source) else {
        eprintln!("i18n: can't read {}", source.display());
        return false;
    };
    for path in paths {
        let Ok(path) = path.map(|p| p.path()) else { return false };
        if path.extension().is_none_or(|e| e != "rs") || path.file_stem().is_some_and(|n| n == "demo" || n == "i18n" || n == "icon_data") {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&path) else { return false };
        let Ok(file) = syn::parse_file(&text) else {
            eprintln!("i18n: can't parse {}", path.display());
            return false;
        };
        lookups.visit_file(&file);
    }
    for spec in craft_anything_engine::Registry::with_all_tools().all() {
        lookups.0.0.insert(spec.title.to_string());
        lookups.0.0.insert(spec.toolset.title().to_string());
    }
    let awaiting_path = source.join("i18n").join(AWAITING);
    let Ok(awaiting) = std::fs::read_to_string(&awaiting_path).map(|text| awaiting_keys(&text)) else {
        eprintln!("i18n: can't read {}", awaiting_path.display());
        return false;
    };
    let mut catalogs = Vec::new();
    for code in ["zh-hans", "zh-hant", "ru", "ko", "cs", "de", "ka", "es", "it", "vi", "pt-br", "tr", "th", "et"] {
        let path = source.join("i18n").join(format!("{code}.tsv"));
        let Ok(text) = std::fs::read_to_string(&path) else {
            eprintln!("i18n: can't read {}", path.display());
            return false;
        };
        catalogs.push((code, catalog_keys(&text)));
    }
    let report = compare(&lookups.0.0, &awaiting, &catalogs);
    for line in &report.problems {
        eprintln!("i18n: {line}");
    }
    for line in &report.counts {
        println!("{line}");
    }
    report.problems.is_empty()
}

/// The list of strings that may be missing from the catalogs for now, beside them in `i18n/`.
const AWAITING: &str = "awaiting.txt";

/// A catalog's keys, read as the app reads them (`i18n::parse`): a key with an empty translation
/// shows in English, so it isn't translated.
fn catalog_keys(text: &str) -> BTreeSet<String> {
    text.lines()
        .filter(|l| !l.trim_start().starts_with('#'))
        .filter_map(|l| l.split_once('\t'))
        .filter(|(k, v)| !k.is_empty() && !v.trim().is_empty())
        .map(|(k, _)| k.replace("\\n", "\n"))
        .collect()
}

/// One string per line, written as in a catalog; `#` starts a comment line.
fn awaiting_keys(text: &str) -> BTreeSet<String> {
    text.lines().filter(|l| !l.starts_with('#') && !l.trim().is_empty()).map(|k| k.replace("\\n", "\n")).collect()
}

struct Report {
    problems: Vec<String>,
    counts: Vec<String>,
}

/// What each catalog is missing, leaving out the strings awaiting translation. An awaiting string
/// the UI no longer looks up, or one every catalog has now, is a problem too, so the list empties
/// as the translations land.
fn compare(lookups: &BTreeSet<String>, awaiting: &BTreeSet<String>, catalogs: &[(&str, BTreeSet<String>)]) -> Report {
    let mut problems = Vec::new();
    let mut counts = Vec::new();
    for key in awaiting.difference(lookups) {
        problems.push(format!("{AWAITING}: {key:?} isn't shown anywhere; take it out of the list"));
    }
    for key in awaiting.intersection(lookups) {
        if catalogs.iter().all(|(_, keys)| keys.contains(key)) {
            problems.push(format!("{AWAITING}: {key:?} is translated in every catalog now; take it out of the list"));
        }
    }
    for (code, keys) in catalogs {
        let missing: Vec<_> = lookups.difference(keys).collect();
        let (waiting, missing): (Vec<_>, Vec<_>) = missing.into_iter().partition(|k| awaiting.contains(*k));
        for key in &missing {
            problems.push(format!("{code}: missing {key:?}"));
        }
        counts.push(format!(
            "{code}: {} lookup keys checked, {} missing, {} awaiting translation ({} catalog entries)",
            lookups.len(),
            missing.len(),
            waiting.len(),
            keys.len()
        ));
    }
    Report { problems, counts }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collects_rendered_literals_without_machine_ids_or_arguments() {
        let file = syn::parse_file(
            r#"
            fn screen() {
                tr(if mode == "internal" { "Simple" } else { "Advanced" });
                widgets::primary_icon(ui, Some("play"), "Start", true);
                icons::button(ui, "x", 20.0, false, "Close");
                widgets::segmented(ui, &["One", "Two"], 0, 30.0);
                widgets::named(resp, &trf("Open {name}", &[("name", "sample.exe")]));
            }
            #[cfg(test)] mod tests { fn other() { tr("Test only"); } }
            "#,
        )
        .unwrap();
        let mut lookups = Lookups::default();
        lookups.visit_file(&file);
        assert_eq!(lookups.0.0, ["Advanced", "Close", "One", "Open {name}", "Simple", "Start", "Two"].into_iter().map(str::to_string).collect());
    }

    fn set(keys: &[&str]) -> BTreeSet<String> {
        keys.iter().map(|k| k.to_string()).collect()
    }

    #[test]
    fn a_string_awaiting_translation_may_be_missing_until_every_catalog_has_it() {
        let shown = set(&["Open", "Wrap lines", "Copy"]);
        let catalogs = [("de", set(&["Open", "Copy"])), ("ko", set(&["Open"]))];
        // Missing "Copy" in ko is still a problem; "Wrap lines" is waiting.
        let r = compare(&shown, &set(&["Wrap lines"]), &catalogs);
        assert_eq!(r.problems, ["ko: missing \"Copy\""]);
        assert_eq!(r.counts[1], "ko: 3 lookup keys checked, 1 missing, 1 awaiting translation (1 catalog entries)");
        // Waiting for both: nothing to fix yet.
        assert!(compare(&shown, &set(&["Wrap lines", "Copy"]), &catalogs).problems.is_empty());
        // Translated everywhere, or no longer shown: the list must lose it.
        let r = compare(&shown, &set(&["Open", "Gone"]), &catalogs);
        // ...and "Wrap lines" and "Copy" are missing again, three times between them.
        assert_eq!(r.problems.len(), 5, "{:?}", r.problems);
        assert!(r.problems.iter().any(|p| p.contains("\"Gone\" isn't shown anywhere")));
        assert!(r.problems.iter().any(|p| p.contains("\"Open\" is translated in every catalog")));
    }

    #[test]
    fn the_awaiting_list_reads_like_a_catalog_key_column() {
        assert_eq!(awaiting_keys("# strings waiting\n\nWrap lines\nTwo\\nlines\n"), set(&["Wrap lines", "Two\nlines"]));
        assert_eq!(catalog_keys("# c\nOpen\tÖffnen\nTwo\\nlines\tx\n"), set(&["Open", "Two\nlines"]));
        // An empty translation shows in English, so it isn't one; nor is an indented comment.
        assert_eq!(catalog_keys("Open\t \n  # Close\tx\nCopy\tKopieren\n"), set(&["Copy"]));
    }

    /// The list in the repository parses, so CI's check reads what's committed.
    #[test]
    fn the_committed_list_is_readable() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../crates/ui-egui/src/i18n").join(AWAITING);
        assert!(std::fs::read_to_string(path).is_ok());
    }
}
