use craft_anything_setup::yaml::{parse, remove, set};
use serde_json::json;

const BOM: &str = "\u{feff}";

/// A byte-order mark in front of a YAML config (as some Windows editors save one) isn't part of
/// its first key.
#[test]
fn a_byte_order_mark_is_not_part_of_the_first_key() {
    let v = parse(&format!("{BOM}mcp_servers:\n  other:\n    command: o\n")).unwrap();
    assert_eq!(v, json!({"mcp_servers": {"other": {"command": "o"}}}));
}

/// Our entry goes into the table the file already has, not into a second table of the same name
/// that hides the first from the app, and removing it gives the file back byte for byte.
#[test]
fn a_config_with_a_byte_order_mark_gets_one_table() {
    let entry = json!({"command": "/b/craft-anything-cli", "args": ["mcp"], "enabled": true});
    for eol in ["\n", "\r\n"] {
        let original = format!("{BOM}mcp_servers:{eol}  other:{eol}    command: o{eol}model: x{eol}");
        let new = set(&original, "mcp_servers", "craft-anything", &entry).unwrap();
        assert!(new.starts_with(BOM), "{new:?}");
        assert_eq!(new.matches("mcp_servers:").count(), 1, "{new:?}");
        let v = parse(&new).unwrap();
        assert_eq!(v["mcp_servers"]["other"]["command"], "o");
        assert_eq!(v["mcp_servers"]["craft-anything"], entry);
        assert_eq!(v["model"], "x");
        assert_eq!(remove(&new, "mcp_servers", "craft-anything").unwrap(), original);
    }
}

/// A comment right after the byte-order mark is still a comment.
#[test]
fn a_comment_after_the_byte_order_mark_stays() {
    let entry = json!({"command": "c", "args": ["mcp"]});
    let original = format!("{BOM}# Hermes config\nmcp_servers:\n  other:\n    command: o\n");
    let new = set(&original, "mcp_servers", "craft-anything", &entry).unwrap_or_else(|e| panic!("{e}"));
    assert!(new.starts_with(&format!("{BOM}# Hermes config\n")), "{new:?}");
    assert_eq!(parse(&new).unwrap()["mcp_servers"]["other"]["command"], "o");
    assert_eq!(remove(&new, "mcp_servers", "craft-anything").unwrap(), original);
}

/// A file holding only a byte-order mark is treated as empty.
#[test]
fn a_file_with_only_a_byte_order_mark_gets_a_new_table() {
    let entry = json!({"command": "c", "args": ["mcp"]});
    let new = set(BOM, "mcp_servers", "craft-anything", &entry).unwrap();
    assert_eq!(parse(&new).unwrap(), json!({"mcp_servers": {"craft-anything": entry}}));
}
