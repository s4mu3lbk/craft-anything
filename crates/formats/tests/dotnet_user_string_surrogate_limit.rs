use craft_anything_formats::dotnet::meta::Metadata;

fn metadata_with_user_string(units: &[u16]) -> Vec<u8> {
    let mut heap = vec![0, (units.len() * 2 + 1) as u8];
    for unit in units {
        heap.extend_from_slice(&unit.to_le_bytes());
    }
    heap.push(0);
    let mut data = vec![0; 72];
    data[..4].copy_from_slice(b"BSJB");
    data[4..6].copy_from_slice(&1u16.to_le_bytes());
    data[12..16].copy_from_slice(&4u32.to_le_bytes());
    data[16..19].copy_from_slice(b"v4\0");
    data[22..24].copy_from_slice(&2u16.to_le_bytes());
    data[24..28].copy_from_slice(&48u32.to_le_bytes());
    data[28..32].copy_from_slice(&24u32.to_le_bytes());
    data[32..35].copy_from_slice(b"#~\0");
    data[36..40].copy_from_slice(&72u32.to_le_bytes());
    data[40..44].copy_from_slice(&(heap.len() as u32).to_le_bytes());
    data[44..48].copy_from_slice(b"#US\0");
    data.extend_from_slice(&heap);
    data
}

#[test]
fn the_character_limit_preserves_complete_surrogate_pairs() {
    let units: Vec<u16> = "\u{1f600}A\u{1f642}".encode_utf16().collect();
    let data = metadata_with_user_string(&units);
    let metadata = Metadata::parse(&data).unwrap();
    assert_eq!(metadata.user_string(1, 1).as_deref(), Some("\u{1f600}"));
    assert_eq!(metadata.user_string(1, 2).as_deref(), Some("\u{1f600}A"));
    assert_eq!(metadata.user_string(1, 3).as_deref(), Some("\u{1f600}A\u{1f642}"));
    assert_eq!(metadata.user_string(1, 0).as_deref(), Some(""));
}

#[test]
fn unpaired_surrogates_still_decode_lossily_without_consuming_the_next_character() {
    let data = metadata_with_user_string(&[0xd800, 0x0041, 0xdc00]);
    let metadata = Metadata::parse(&data).unwrap();
    assert_eq!(metadata.user_string(1, 1).as_deref(), Some("\u{fffd}"));
    assert_eq!(metadata.user_string(1, 2).as_deref(), Some("\u{fffd}A"));
    assert_eq!(metadata.user_string(1, 3).as_deref(), Some("\u{fffd}A\u{fffd}"));
}
