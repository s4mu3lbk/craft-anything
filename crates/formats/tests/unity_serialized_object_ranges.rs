use craft_anything_formats::{FormatError, unity_serialized};

fn metadata(version: u32, big: bool, start: u64, size: u32) -> (Vec<u8>, unity_serialized::Header) {
    let mut data = vec![0; if version >= 22 { 48 } else { 20 }];
    let metadata_offset = data.len() as u64;
    let u32s = |data: &mut Vec<u8>, value: u32| data.extend_from_slice(&if big { value.to_be_bytes() } else { value.to_le_bytes() });
    let u64s = |data: &mut Vec<u8>, value: u64| data.extend_from_slice(&if big { value.to_be_bytes() } else { value.to_le_bytes() });
    data.extend_from_slice(b"2022.3.1f1\0");
    u32s(&mut data, 0); // Platform.
    data.push(0); // No type trees.
    u32s(&mut data, 1); // One TextAsset type.
    u32s(&mut data, 49);
    data.push(0); // Not stripped.
    data.extend_from_slice(&[0xff; 2]); // No script type index.
    data.extend_from_slice(&[0; 16]); // Type hash.
    u32s(&mut data, 1); // One object.
    data.resize(data.len().next_multiple_of(4), 0);
    u64s(&mut data, 7);
    if version >= 22 {
        u64s(&mut data, start);
    } else {
        u32s(&mut data, start as u32);
    }
    u32s(&mut data, size);
    u32s(&mut data, 0); // Type index.
    u32s(&mut data, 0); // Script references.
    u32s(&mut data, 0); // Externals.
    let h = unity_serialized::Header {
        version,
        big_endian: big,
        metadata_offset,
        metadata_size: data.len() as u64 - metadata_offset,
        data_offset: 256,
        file_size: 512,
    };
    (data, h)
}

#[test]
fn objects_must_fit_the_declared_file() {
    for version in [17, 22] {
        for big in [false, true] {
            for (start, size) in [(256, 1), (257, 0), (255, 2), (0, u32::MAX)] {
                let (data, h) = metadata(version, big, start, size);
                assert!(
                    matches!(unity_serialized::metadata(&data, &h), Err(FormatError::Malformed(_))),
                    "version {version}, big {big}, start {start}, size {size}"
                );
            }
            for (start, size) in [(0, 32), (255, 1), (256, 0)] {
                let (data, h) = metadata(version, big, start, size);
                let m = unity_serialized::metadata(&data, &h).unwrap();
                assert_eq!((m.objects[0].path_id, m.objects[0].offset, m.objects[0].size), (7, 256 + start, size));
            }
        }
    }
}

#[test]
fn object_offset_and_end_overflow_are_rejected() {
    for (start, size) in [(u64::MAX, 0), (u64::MAX, 1), (u64::MAX - 255, 0)] {
        let (data, h) = metadata(22, false, start, size);
        assert!(unity_serialized::metadata(&data, &h).is_err());
    }
    let (data, mut h) = metadata(22, false, 16, 1);
    h.data_offset = u64::MAX - 16;
    h.file_size = u64::MAX;
    assert!(unity_serialized::metadata(&data, &h).is_err(), "offset fits but offset plus size overflows");
}

#[test]
fn existing_serialized_objects_keep_their_ranges() {
    let data = craft_anything_testkit::games::unity_assets();
    let h = unity_serialized::header(&data, data.len() as u64).unwrap();
    let m = unity_serialized::metadata(&data, &h).unwrap();
    assert_eq!(m.objects.len(), 4);
    assert!(m.objects.iter().all(|o| o.offset >= h.data_offset && o.offset + u64::from(o.size) <= h.file_size));
}
