use craft_anything_formats::{FormatError, unreal_iostore, unreal_pak};

fn index_mount(count: i32, bytes: &[u8]) -> Result<String, FormatError> {
    let footer = unreal_pak::Footer {
        version: 3,
        index_offset: 0,
        index_size: 0,
        encrypted_index: false,
        encryption_key_guid: None,
        compression_methods: Vec::new(),
        method_slots: 0,
    };
    let mut data = count.to_le_bytes().to_vec();
    data.extend_from_slice(bytes);
    data.extend_from_slice(&0i32.to_le_bytes()); // No entries.
    unreal_pak::index(&data, &footer).map(|index| index.mount_point)
}

#[test]
fn nonempty_fstrings_require_a_final_null_code_unit() {
    for (count, bytes) in [(3, &b"abc"[..]), (1, &b"x"[..]), (-2, &[b'A', 0, b'B', 0][..]), (-1, &[0, 1][..])] {
        assert!(matches!(index_mount(count, bytes), Err(FormatError::Malformed(_))), "count {count}, bytes {bytes:?}");
    }
}

#[test]
fn empty_and_terminated_ansi_and_utf16_strings_still_read() {
    assert_eq!(index_mount(0, &[]).unwrap(), "");
    assert_eq!(index_mount(1, &[0]).unwrap(), "");
    assert_eq!(index_mount(-1, &[0, 0]).unwrap(), "");
    assert_eq!(index_mount(5, b"Game\0").unwrap(), "Game");
    let name = "Game/地形/🧩";
    let units = name.encode_utf16().chain(std::iter::once(0)).collect::<Vec<_>>();
    let bytes = units.iter().flat_map(|u| u.to_le_bytes()).collect::<Vec<_>>();
    assert_eq!(index_mount(-(units.len() as i32), &bytes).unwrap(), name);
}

#[test]
fn iostore_uses_the_same_terminator_validation() {
    let mut toc = unreal_iostore::toc(&craft_anything_testkit::games::unreal_utoc(9, u32::MAX)).unwrap();
    let mount_len = i32::from_le_bytes(toc.directory_index[..4].try_into().unwrap()) as usize;
    assert_eq!(unreal_iostore::files(&toc, 10).unwrap().2, 3);
    toc.directory_index[4 + mount_len - 1] = b'!';
    assert!(matches!(unreal_iostore::files(&toc, 10), Err(FormatError::Malformed(_))));
}
