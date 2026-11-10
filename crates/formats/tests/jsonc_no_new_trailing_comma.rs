use craft_anything_formats::jsonc::parse;
use serde_json::json;

/// A JSONC config that is plain JSON (as Claude Desktop's is) stays plain JSON when its last member
/// is removed: the comma that separated it from the one before goes too.
#[test]
fn removing_the_last_member_of_plain_json_leaves_no_trailing_comma() {
    for original in [
        "{\n  \"mcpServers\": {\n    \"other\": {\"command\": \"o\"},\n    \"craft-anything\": {\"command\": \"c\"}\n  }\n}\n",
        "{\r\n  \"mcpServers\": {\r\n    \"other\": {\"command\": \"o\"},\r\n    \"craft-anything\": {\"command\": \"c\"}\r\n  }\r\n}\r\n",
        // Our name twice at the end (an older copy left behind): both go.
        "{\n  \"mcpServers\": {\n    \"other\": 1,\n    \"craft-anything\": 2,\n    \"craft-anything\": 3\n  }\n}\n",
    ] {
        let edited = parse(original, false).unwrap().remove(&["mcpServers", "craft-anything"]).unwrap();
        let strict = parse(&edited, true).unwrap_or_else(|e| panic!("{original:?} became {edited:?}: {e}"));
        assert_eq!(strict.get(&["mcpServers", "craft-anything"]), None);
        assert!(strict.get(&["mcpServers", "other"]).is_some());
    }
    for strict in [false, true] {
        let edited = parse("{\n  \"a\": 1,\n  \"b\": 2\n}\n", strict).unwrap().remove(&["b"]).unwrap();
        assert_eq!(edited, "{\n  \"a\": 1\n}\n", "strict: {strict}");
    }
}

/// A file that already ends its members with commas keeps that style, both when a member is added
/// and when it is removed again.
#[test]
fn trailing_comma_style_is_kept_both_ways() {
    let original = "{\n  \"servers\": {\n    \"other\": {\"command\": \"o\"}, // keep me\n  },\n}\n";
    let doc = parse(original, false).unwrap();
    let added = doc.set(&["servers", "craft-anything"], &json!({"command": "c"})).unwrap();
    assert!(added.contains("\"command\": \"c\"\n    },\n  },"), "{added}");
    let removed = parse(&added, false).unwrap().remove(&["servers", "craft-anything"]).unwrap();
    assert_eq!(removed, original);
}
