use craft_anything_formats::{FormatError, unreal_iostore};

#[test]
fn all_reserved_header_bytes_must_be_present() {
    let fixture = craft_anything_testkit::games::unreal_utoc(9, u32::MAX);
    for len in 0..unreal_iostore::HEADER_SIZE {
        assert!(matches!(unreal_iostore::header(&fixture[..len]), Err(FormatError::Truncated(_))), "a {len}-byte header is incomplete");
    }
}

#[test]
fn complete_headers_read_for_each_supported_version() {
    let mut fixture = craft_anything_testkit::games::unreal_utoc(9, u32::MAX);
    for version in 1..=unreal_iostore::MAX_VERSION {
        fixture[16] = version;
        let h = unreal_iostore::header(&fixture[..unreal_iostore::HEADER_SIZE]).unwrap();
        assert_eq!((h.version, h.chunk_count, h.block_count, h.block_size), (version, 3, 4, 65536));
    }
}
