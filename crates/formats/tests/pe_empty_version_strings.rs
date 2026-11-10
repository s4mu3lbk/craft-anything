use craft_anything_formats::pe;
use std::collections::BTreeMap;

/// One version-resource node: the header, the key, then the value and children, each 4-aligned.
fn node(key: &str, value: &[u8], value_len: u16, text: bool, children: &[Vec<u8>]) -> Vec<u8> {
    let mut b = vec![0u8; 6];
    b.extend(key.encode_utf16().chain([0]).flat_map(u16::to_le_bytes));
    for part in std::iter::once(value).chain(children.iter().map(Vec::as_slice)) {
        while !b.len().is_multiple_of(4) {
            b.push(0);
        }
        b.extend_from_slice(part);
    }
    let len = b.len() as u16;
    b[0..2].copy_from_slice(&len.to_le_bytes());
    b[2..4].copy_from_slice(&value_len.to_le_bytes());
    b[4..6].copy_from_slice(&u16::from(text).to_le_bytes());
    b
}

fn string(key: &str, value: &str) -> Vec<u8> {
    let units: Vec<u8> = value.encode_utf16().chain([0]).flat_map(u16::to_le_bytes).collect();
    node(key, &units, (units.len() / 2) as u16, true, &[])
}

/// A string value written with wType 0 (binary), as some linkers do.
fn binary_string(key: &str, value: &str) -> Vec<u8> {
    let bytes: Vec<u8> = value.encode_utf16().chain([0]).flat_map(u16::to_le_bytes).collect();
    node(key, &bytes, bytes.len() as u16, false, &[])
}

/// What Windows resource compilers write for an empty value: no value at all (wValueLength 0).
fn empty_string(key: &str) -> Vec<u8> {
    node(key, &[], 0, true, &[])
}

fn version_info(tables: &[Vec<u8>]) -> Vec<u8> {
    let mut fixed = vec![0u8; 52];
    fixed[0..4].copy_from_slice(&0xfeef_04bd_u32.to_le_bytes());
    let file_info = node("StringFileInfo", &[], 0, true, tables);
    node("VS_VERSION_INFO", &fixed, 52, false, &[file_info])
}

/// Keys with an empty value are kept. `OleSelfRegister` only means something by being there.
#[test]
fn version_strings_with_empty_values_are_kept() {
    let table = node("040904b0", &[], 0, true, &[string("CompanyName", "Craft"), empty_string("OleSelfRegister"), empty_string("InternalName")]);
    let info = pe::parse_version_info(&version_info(&[table])).unwrap();
    let expected: BTreeMap<String, String> =
        [("CompanyName", "Craft"), ("InternalName", ""), ("OleSelfRegister", "")].into_iter().map(|(k, v)| (k.into(), v.into())).collect();
    assert_eq!(info.strings, expected);
}

/// Only a node right under a string table is a string. A node inside another string's value (as in
/// a binary-typed `FileDescription`) or a node with no key isn't one.
#[test]
fn leaves_that_are_not_strings_stay_out() {
    let binary = node("FileDescription", &[0x4d, 0, 0x53, 0, 0x20, 0, 0x43, 0], 8, false, &[empty_string("Inner")]);
    let nameless = node("", &[], 0, true, &[]);
    let table = node("040904b0", &[], 0, true, &[binary, nameless, string("ProductName", "Craft")]);
    let info = pe::parse_version_info(&version_info(&[table])).unwrap();
    assert_eq!(info.strings.keys().collect::<Vec<_>>(), ["ProductName"]);
}

/// Some linkers write a string with wType 0; its value is still UTF-16LE.
#[test]
fn binary_typed_version_strings_are_read() {
    let table = node("040904b0", &[], 0, true, &[binary_string("FileDescription", "MS C"), string("ProductName", "Craft")]);
    let info = pe::parse_version_info(&version_info(&[table])).unwrap();
    assert_eq!(info.strings.get("FileDescription").map(String::as_str), Some("MS C"));
    assert_eq!(info.strings.get("ProductName").map(String::as_str), Some("Craft"));
}

/// A binary leaf deeper than the string table is not a string and must not overwrite one.
#[test]
fn a_binary_leaf_below_the_string_depth_does_not_overwrite() {
    let nested = binary_string("ProductName", "nested");
    let container = node("FileDescription", &[0x4d, 0, 0x53, 0], 4, false, &[nested]);
    let table = node("040904b0", &[], 0, true, &[string("ProductName", "Craft"), container]);
    let info = pe::parse_version_info(&version_info(&[table])).unwrap();
    assert_eq!(info.strings.get("ProductName").map(String::as_str), Some("Craft"));
    assert_eq!(info.strings.keys().collect::<Vec<_>>(), ["ProductName"]);
}

/// An empty value in one language's table doesn't hide the value another table gives the key.
#[test]
fn an_empty_value_does_not_replace_a_real_one() {
    let named = || node("040904b0", &[], 0, true, &[string("InternalName", "foo")]);
    let empty = || node("000004b0", &[], 0, true, &[empty_string("InternalName")]);
    for tables in [[named(), empty()], [empty(), named()]] {
        let info = pe::parse_version_info(&version_info(&tables)).unwrap();
        assert_eq!(info.strings.get("InternalName").map(String::as_str), Some("foo"));
    }
}

/// A string table with no strings is a table, not an empty string named after its language.
#[test]
fn an_empty_string_table_is_not_a_string() {
    let empty_table = node("000004b0", &[], 0, true, &[]);
    let table = node("040904b0", &[], 0, true, &[string("ProductName", "Craft")]);
    let info = pe::parse_version_info(&version_info(&[empty_table, table])).unwrap();
    assert_eq!(info.strings.keys().collect::<Vec<_>>(), ["ProductName"]);
}
