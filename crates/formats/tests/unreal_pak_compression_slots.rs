use craft_anything_formats::unreal_pak::{self, Footer, Index};

fn footer(version: i32, names: &[&str]) -> Footer {
    let mut tail = vec![0; 17];
    tail.extend_from_slice(&unreal_pak::MAGIC.to_le_bytes());
    tail.extend_from_slice(&version.to_le_bytes());
    tail.extend_from_slice(&0u64.to_le_bytes());
    tail.extend_from_slice(&100u64.to_le_bytes());
    tail.extend_from_slice(&[0; 20]);
    for name in names {
        let mut slot = name.as_bytes().to_vec();
        slot.resize(32, 0);
        tail.extend_from_slice(&slot);
    }
    unreal_pak::footer(&tail, 4096).unwrap()
}

fn string(data: &mut Vec<u8>, value: &str) {
    data.extend_from_slice(&(value.len() as i32 + 1).to_le_bytes());
    data.extend_from_slice(value.as_bytes());
    data.push(0);
}

fn plain_method(footer: &Footer, method: u8) -> String {
    let mut data = Vec::new();
    string(&mut data, "");
    data.extend_from_slice(&1i32.to_le_bytes());
    string(&mut data, "file.bin");
    for value in [0u64, if method == 0 { 7 } else { 3 }, 7] {
        data.extend_from_slice(&value.to_le_bytes());
    }
    data.push(method); // Version 8 with four method slots stores the index in one byte.
    data.extend_from_slice(&[0; 20]);
    if method != 0 {
        data.extend_from_slice(&1i32.to_le_bytes());
        data.extend_from_slice(&70u64.to_le_bytes());
        data.extend_from_slice(&73u64.to_le_bytes());
    }
    data.push(0);
    data.extend_from_slice(&65536u32.to_le_bytes());
    unreal_pak::index(&data, footer).unwrap().entries[0].compression.clone()
}

fn packed_method(footer: &Footer, method: u8) -> String {
    let bits = (1u32 << 31) | (1 << 30) | (1 << 29) | (u32::from(method) << 23) | (u32::from(method != 0) << 6);
    let mut encoded = bits.to_le_bytes().to_vec();
    encoded.extend_from_slice(&0u32.to_le_bytes());
    encoded.extend_from_slice(&7u32.to_le_bytes());
    if method != 0 {
        encoded.extend_from_slice(&3u32.to_le_bytes());
    }
    let index = Index { mount_point: String::new(), file_count: 1, directory_index: None, entries: Vec::new(), encoded, plain: Vec::new() };
    let mut data = 1i32.to_le_bytes().to_vec();
    string(&mut data, "");
    data.extend_from_slice(&1i32.to_le_bytes());
    string(&mut data, "file.bin");
    data.extend_from_slice(&0i32.to_le_bytes());
    unreal_pak::directory(&data, footer, &index, 1).unwrap()[0].compression.clone()
}

#[test]
fn sparse_method_tables_keep_their_original_indices() {
    let old = footer(8, &["Zlib", "", "Oodle", ""]);
    let new = footer(11, &["Zlib", "", "Oodle", "", ""]);
    for (method, expected) in [(0, "none"), (1, "Zlib"), (2, "method 2"), (3, "Oodle"), (4, "method 4"), (6, "method 6")] {
        assert_eq!(plain_method(&old, method), expected, "plain method {method}");
        assert_eq!(packed_method(&new, method), expected, "packed method {method}");
    }
    assert_eq!(old.compression_methods, ["Zlib", "", "Oodle"]);
    assert_eq!(new.compression_methods, ["Zlib", "", "Oodle"]);
    assert_eq!((old.method_slots, new.method_slots), (4, 5));
}

#[test]
fn an_empty_first_slot_does_not_rename_method_one() {
    let f = footer(11, &["", "Oodle", "", "", ""]);
    assert_eq!(packed_method(&f, 1), "method 1");
    assert_eq!(packed_method(&f, 2), "Oodle");
}

#[test]
fn contiguous_method_tables_keep_their_existing_representation() {
    let f = footer(11, &["Zlib", "Oodle", "", "", ""]);
    assert_eq!(f.compression_methods, ["Zlib", "Oodle"]);
    assert_eq!(packed_method(&f, 1), "Zlib");
    assert_eq!(packed_method(&f, 2), "Oodle");
}
