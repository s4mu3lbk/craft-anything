use craft_anything_setup::yaml::{parse, remove, set};
use serde_json::json;

/// `mcp_servers: null` (or `~`, with or without a comment) is an empty table, the same as
/// `mcp_servers: {}` or nothing after the colon. Our entry goes into it.
#[test]
fn a_null_table_takes_our_entry() {
    let entry = json!({"command": "/b/craft-anything-cli", "args": ["mcp"], "enabled": true});
    for value in ["null", "~", "null # none yet", "~  # none yet"] {
        let original = format!("model: x\nmcp_servers: {value}\nskills: on\n");
        assert_eq!(parse(&original).unwrap()["mcp_servers"], json!(null), "{original}");
        let new = set(&original, "mcp_servers", "craft-anything", &entry).unwrap_or_else(|e| panic!("{original:?}: {e}"));
        let v = parse(&new).unwrap();
        assert_eq!(v["mcp_servers"]["craft-anything"], entry, "{new}");
        assert_eq!((v["model"].clone(), v["skills"].clone()), (json!("x"), json!("on")), "{new}");
        assert_eq!(new.matches("mcp_servers").count(), 1, "{new}");
        if let Some((_, comment)) = value.split_once('#') {
            assert!(new.contains(&format!("mcp_servers: #{comment}\n")), "comment kept: {new}");
        }
        assert!(remove(&new, "mcp_servers", "craft-anything").is_some());
    }
    // An indented comment under it doesn't make it anything else.
    assert!(set("mcp_servers: null\n  # later\nmodel: x\n", "mcp_servers", "craft-anything", &entry).is_ok());
}

/// A table written in flow style with members in it is still left to the person, and so is a
/// value that only starts with `null` and carries on onto the next line (it reads as text).
#[test]
fn a_flow_table_with_members_is_still_refused() {
    let entry = json!({"command": "c"});
    assert!(set("mcp_servers: {a: 1}\n", "mcp_servers", "craft-anything", &entry).is_err());
    assert!(set("mcp_servers: nullish\n", "mcp_servers", "craft-anything", &entry).is_err());
    let continued = "mcp_servers: null\n      more\nmodel: x\n";
    assert_eq!(parse(continued).unwrap()["mcp_servers"], json!("null more"));
    assert!(set(continued, "mcp_servers", "craft-anything", &entry).is_err());
    assert!(remove(continued, "mcp_servers", "craft-anything").is_none());
}
