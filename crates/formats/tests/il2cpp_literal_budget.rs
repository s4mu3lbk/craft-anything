use craft_anything_formats::il2cpp::{self, Literal, Reading};
use craft_anything_testkit::games::{Il2cppSample, il2cpp_metadata};

fn literal(bytes: &[u8], cap: usize) -> Literal {
    let mut data = il2cpp_metadata(&Il2cppSample { version: 29, header_size: 256, type_size: 88, method_size: 32 });
    let Reading::Readable(metadata) = il2cpp::read(&data) else { panic!("fixture is readable") };
    let record = metadata.sections().iter().find(|section| section.name == "string_literals").unwrap().offset as usize;
    let payload = metadata.sections().iter().find(|section| section.name == "string_literal_data").unwrap().offset as usize;
    data[record..record + 4].copy_from_slice(&(bytes.len() as u32).to_le_bytes());
    data[record + 4..record + 8].copy_from_slice(&0u32.to_le_bytes());
    data[payload..payload + bytes.len()].copy_from_slice(bytes);
    let Reading::Readable(metadata) = il2cpp::read(&data) else { panic!("literal bytes do not change the layout") };
    metadata.literal(0, cap).unwrap()
}

#[test]
fn a_short_literal_ends_at_a_complete_character() {
    let bytes = "A中B🙂C".as_bytes();
    for (cap, expected) in [(0, ""), (1, "A"), (2, "A"), (3, "A"), (4, "A中"), (5, "A中B"), (8, "A中B"), (9, "A中B🙂"), (10, "A中B🙂C")] {
        let value = literal(bytes, cap);
        assert_eq!(value.text, expected, "cap {cap}");
        assert!(value.text.len() <= cap);
        assert_eq!(value.complete, cap >= bytes.len());
        assert_eq!(value.length, bytes.len() as u32);
    }
}

#[test]
fn lossy_replacements_spend_the_output_byte_budget() {
    for (cap, expected, complete) in [(1, "A", false), (2, "A", false), (3, "A", false), (4, "A�", false), (5, "A�B", true)] {
        let value = literal(&[b'A', 0xff, b'B'], cap);
        assert_eq!(value.text, expected);
        assert_eq!(value.complete, complete);
        assert!(value.text.len() <= cap);
    }
}

#[test]
fn malformed_final_bytes_are_distinct_from_a_capped_valid_character() {
    assert_eq!(literal(&[0xe4, 0xb8], 3).text, "�");
    assert!(literal(&[0xe4, 0xb8], 3).complete);
    assert_eq!(literal("中".as_bytes(), 2).text, "");
    assert!(!literal("中".as_bytes(), 2).complete);
    assert_eq!(literal(&[0xff, 0xe4, 0xb8, 0xad], 4).text, "�");
    assert!(!literal(&[0xff, 0xe4, 0xb8, 0xad], 4).complete);
}

#[test]
fn empty_and_ascii_literals_keep_their_completeness() {
    assert!(literal(b"", 0).complete);
    assert_eq!(literal(b"hello", 3).text, "hel");
    assert!(!literal(b"hello", 3).complete);
    assert_eq!(literal(b"hello", 5).text, "hello");
    assert!(literal(b"hello", 5).complete);
}
