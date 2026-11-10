use craft_anything_formats::FormatError;
use craft_anything_formats::node_sea::NodeSea;
use craft_anything_formats::packed::{MAX_ENTRIES, PackedFormat};

const TEXT_BUDGET: usize = 8 * 1024 * 1024;

fn part(blob: &mut Vec<u8>, bytes: &[u8]) {
    blob.extend((bytes.len() as u64).to_le_bytes());
    blob.extend(bytes);
}

fn blob(path: &[u8], assets: &[(&[u8], &[u8])], arguments: &[&[u8]]) -> Vec<u8> {
    let mut bytes = vec![0x20, 0xda, 0x43, 0x01, 0x18, 0, 0, 0, 1];
    part(&mut bytes, path);
    part(&mut bytes, b"code");
    bytes.extend((assets.len() as u64).to_le_bytes());
    for (key, value) in assets {
        part(&mut bytes, key);
        part(&mut bytes, value);
    }
    bytes.extend((arguments.len() as u64).to_le_bytes());
    for argument in arguments {
        part(&mut bytes, argument);
    }
    bytes
}

#[test]
fn asset_retention_includes_the_main_entry_in_its_limit() {
    let assets = vec![(b"a".as_slice(), b"v".as_slice()); MAX_ENTRIES];
    let bytes = blob(b"main.js", &assets, &[b"--ok"]);
    let listing = NodeSea.list(&bytes).unwrap();
    assert_eq!(listing.entries.len(), MAX_ENTRIES);
    assert_eq!(listing.facts["assets"], MAX_ENTRIES);
    assert_eq!(listing.facts["assets_listed"], MAX_ENTRIES - 1);
    assert_eq!(listing.facts["exec_argv"][0], "--ok");
    assert!(listing.truncated);
    assert!(listing.notes.iter().any(|note| note.contains("metadata listing")));
}

#[test]
fn argument_retention_is_bounded_even_for_empty_strings() {
    let mut arguments = vec![b"".as_slice(); MAX_ENTRIES + 1];
    let ambiguous = blob(b"main.js", &[], &arguments);
    assert!(matches!(NodeSea.list(&ambiguous), Err(FormatError::Unsupported(message)) if message.contains("more than one")));
    // A nonempty final argument keeps legacy layouts from accepting the empty records as padding.
    arguments[MAX_ENTRIES] = b"--end";
    let bytes = blob(b"main.js", &[], &arguments);
    let listing = NodeSea.list(&bytes).unwrap();
    assert_eq!(listing.facts["exec_argv"].as_array().unwrap().len(), MAX_ENTRIES);
    assert_eq!(listing.facts["exec_argv_count"], MAX_ENTRIES + 1);
    assert!(listing.truncated);
}

#[test]
fn skipped_fields_still_have_to_fit_the_complete_blob() {
    let arguments = vec![b"x".as_slice(); MAX_ENTRIES + 1];
    let mut bytes = blob(b"main.js", &[], &arguments);
    bytes.pop();
    assert!(NodeSea.list(&bytes).is_err());

    let assets = vec![(b"a".as_slice(), b"v".as_slice()); MAX_ENTRIES];
    let mut bytes = blob(b"main.js", &assets, &[]);
    bytes.push(1);
    assert!(NodeSea.list(&bytes).is_err());
}

#[test]
fn oversized_keys_are_omitted_whole_and_do_not_hide_later_assets() {
    let key = vec![b'x'; TEXT_BUDGET];
    let bytes = blob(b"main.js", &[(&key, b"large"), (b"ok", b"small")], &[b"--ok"]);
    let listing = NodeSea.list(&bytes).unwrap();
    assert_eq!(listing.entries.len(), 2);
    assert_eq!(listing.entries[1].path, "assets/ok");
    assert_eq!(NodeSea.read(&bytes, &listing.entries[1], 100).unwrap(), b"small");
    assert_eq!(listing.facts["assets"], 2);
    assert_eq!(listing.facts["assets_listed"], 1);
    assert_eq!(listing.facts["exec_argv"][0], "--ok");
    assert!(listing.truncated);
}

#[test]
fn replacement_characters_spend_the_decoded_byte_budget() {
    let key = vec![0xff; TEXT_BUDGET / 3 + 1];
    let bytes = blob(b"main.js", &[(&key, b"v")], &[b"--ok"]);
    let listing = NodeSea.list(&bytes).unwrap();
    assert_eq!(listing.entries.len(), 1);
    assert_eq!(listing.facts["assets"], 1);
    assert_eq!(listing.facts["assets_listed"], 0);
    assert_eq!(listing.facts["exec_argv"][0], "--ok");
    assert!(listing.truncated);
}

#[test]
fn oversized_main_paths_use_the_existing_fallback_name() {
    let path = vec![b'x'; TEXT_BUDGET + 1];
    let bytes = blob(&path, &[(b"ok", b"v")], &[b"--ok"]);
    let listing = NodeSea.list(&bytes).unwrap();
    assert_eq!(listing.entries[0].path, "main/main.js");
    assert!(listing.facts["code_path"].is_null());
    assert_eq!(listing.entries[1].path, "assets/ok");
    assert!(listing.truncated);
}

#[test]
fn ordinary_metadata_is_preserved_without_truncation() {
    let bytes = blob(b"/work/main.js", &[(b"name", b"value")], &[b"--no-warnings", b"--stack-size=512"]);
    let listing = NodeSea.list(&bytes).unwrap();
    assert_eq!(listing.entries[0].path, "main/main.js");
    assert_eq!(listing.entries[1].path, "assets/name");
    assert_eq!(listing.facts["code_path"], "/work/main.js");
    assert_eq!(listing.facts["assets"], 1);
    assert_eq!(listing.facts["assets_listed"], 1);
    assert_eq!(listing.facts["exec_argv_count"], 2);
    assert_eq!(listing.facts["exec_argv"][1], "--stack-size=512");
    assert!(!listing.truncated);
}
