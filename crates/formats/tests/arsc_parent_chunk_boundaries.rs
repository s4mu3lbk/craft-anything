use craft_anything_formats::arsc::{Limits, parse};
use craft_anything_testkit::android::resources_arsc;

fn u32_at(bytes: &[u8], at: usize) -> usize {
    u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap()) as usize
}

fn put32(bytes: &mut [u8], at: usize, value: usize) {
    bytes[at..at + 4].copy_from_slice(&(value as u32).to_le_bytes());
}

fn package_start(bytes: &[u8]) -> usize {
    12 + u32_at(bytes, 16)
}

fn table_with_children(children: &[u8]) -> Vec<u8> {
    let mut bytes = vec![0u8; 12 + 288];
    bytes[0..4].copy_from_slice(&[2, 0, 12, 0]);
    let len = bytes.len() + children.len();
    put32(&mut bytes, 4, len);
    put32(&mut bytes, 8, 1);
    bytes[12..16].copy_from_slice(&[0, 2, 32, 1]);
    put32(&mut bytes, 16, 288 + children.len());
    put32(&mut bytes, 20, 0x7f);
    for (i, unit) in "demo".encode_utf16().enumerate() {
        bytes[24 + i * 2..26 + i * 2].copy_from_slice(&unit.to_le_bytes());
    }
    bytes.extend(children);
    bytes
}

#[test]
fn packages_cannot_extend_past_the_declared_table() {
    let mut bytes = resources_arsc();
    let end = package_start(&bytes) + 288;
    put32(&mut bytes, 4, end);
    let table = parse(&bytes, Limits::default()).unwrap();
    assert_eq!(table.global_strings, 4);
    assert!(table.packages.is_empty());
    assert!(table.entries.is_empty());
    assert!(!table.warnings.is_empty());
}

#[test]
fn package_string_pool_offsets_cannot_read_the_following_payload() {
    let mut bytes = resources_arsc();
    let start = package_start(&bytes);
    put32(&mut bytes, start + 4, 288);
    let table = parse(&bytes, Limits::default()).unwrap();
    assert_eq!(table.packages.len(), 1);
    assert_eq!(table.packages[0].keys, 0);
    assert!(table.packages[0].types.is_empty());
    assert!(table.warnings.iter().any(|warning| warning.contains("string pool is outside")));
}

#[test]
fn a_child_crossing_its_package_does_not_hide_earlier_entries() {
    let mut bytes = resources_arsc();
    let start = package_start(&bytes);
    let size = u32_at(&bytes, start + 4);
    put32(&mut bytes, start + 4, size - 2);
    let table = parse(&bytes, Limits::default()).unwrap();
    assert_eq!(table.entries.len(), 4);
    assert!(table.entries.iter().all(|entry| entry.type_name == "string"));
    assert!(table.warnings.iter().any(|warning| warning.contains("unreadable chunk")));
}

#[test]
fn library_counts_do_not_consume_a_sibling_chunk_as_another_record() {
    let mut children = vec![0u8; 272 + 268];
    children[0..4].copy_from_slice(&[3, 2, 12, 0]);
    put32(&mut children, 4, 272);
    put32(&mut children, 8, 2);
    put32(&mut children, 12, 7);
    for (i, unit) in "library".encode_utf16().enumerate() {
        children[16 + i * 2..18 + i * 2].copy_from_slice(&unit.to_le_bytes());
    }
    children[272..276].copy_from_slice(&[0xff, 0x7f, 8, 0]);
    put32(&mut children, 276, 268);
    let table = parse(&table_with_children(&children), Limits::default()).unwrap();
    assert_eq!(table.packages[0].libraries, vec![(7, "library".to_string())]);
    assert!(table.warnings.iter().any(|warning| warning.contains("library records")));
}

#[test]
fn short_headers_do_not_borrow_fields_from_the_next_chunk() {
    let mut children = vec![2, 2, 8, 0, 8, 0, 0, 0, 2, 2, 16, 0, 16, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0];
    let table = parse(&table_with_children(&children), Limits::default()).unwrap();
    assert_eq!(table.packages[0].types.len(), 1);
    assert_eq!(table.packages[0].types[0].id, 1);
    assert!(table.warnings.iter().any(|warning| warning.contains("incomplete chunk header")));
    children.clear();
    let mut bytes = table_with_children(&children);
    bytes[14..16].copy_from_slice(&8u16.to_le_bytes());
    let table = parse(&bytes, Limits::default()).unwrap();
    assert!(table.packages.is_empty());
    assert!(table.warnings.iter().any(|warning| warning.contains("incomplete header")));
}

#[test]
fn a_truncated_bag_header_cannot_borrow_its_parent_from_a_sibling() {
    let mut bytes = resources_arsc();
    let start = package_start(&bytes);
    let mut at = start + 288;
    while at + u32_at(&bytes, at + 4) < bytes.len() {
        at += u32_at(&bytes, at + 4);
    }
    let entries_start = u32_at(&bytes, at + 16);
    put32(&mut bytes, at + 4, entries_start + 8);
    let table = parse(&bytes, Limits::default()).unwrap();
    let style = table.entries.iter().find(|entry| entry.type_name == "style").unwrap();
    assert!(style.parent.is_none());
    assert!(style.bag.is_empty());
}

#[test]
fn well_formed_table_entries_and_pools_are_unchanged() {
    for legacy in [false, true] {
        let mut bytes = resources_arsc();
        if legacy {
            let start = package_start(&bytes);
            let size = u32_at(&bytes, start + 4);
            let types = u32_at(&bytes, start + 268);
            let keys = u32_at(&bytes, start + 276);
            bytes.drain(start + 284..start + 288);
            bytes[start + 2..start + 4].copy_from_slice(&284u16.to_le_bytes());
            put32(&mut bytes, start + 4, size - 4);
            put32(&mut bytes, start + 268, types - 4);
            put32(&mut bytes, start + 276, keys - 4);
            let len = bytes.len();
            put32(&mut bytes, 4, len);
        }
        let table = parse(&bytes, Limits::default()).unwrap();
        assert!(table.warnings.is_empty(), "{:?}", table.warnings);
        assert_eq!(table.packages.len(), 1);
        assert_eq!(table.packages[0].keys, 3);
        assert_eq!(table.entries.len(), 5);
        let strings = table.strings_by_locale();
        assert_eq!(strings["fr"][0], ("app_name".to_string(), "Notes FR".to_string()));
        assert_eq!(table.entries[4].parent.as_deref(), Some("@0x01030005"));
    }
}
