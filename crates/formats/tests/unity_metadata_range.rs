use craft_anything_formats::unity_serialized::{self, Header};
use craft_anything_testkit::games::unity_assets;

fn fixture() -> (Vec<u8>, Header) {
    let data = unity_assets();
    let header = unity_serialized::header(&data, data.len() as u64).unwrap();
    (data, header)
}

#[test]
fn every_prefix_before_the_metadata_end_is_refused_without_a_panic() {
    let (data, header) = fixture();
    let end = (header.metadata_offset + header.metadata_size) as usize;
    for cut in 0..end {
        assert!(unity_serialized::metadata(&data[..cut], &header).is_err(), "cut {cut}");
    }
    assert!(unity_serialized::metadata(&data[..end], &header).is_ok());
}

#[test]
fn bytes_after_a_short_declared_metadata_region_cannot_complete_it() {
    let (data, header) = fixture();
    for size in [0, 1, header.metadata_size - 6] {
        let shorter = Header { metadata_size: size, ..header };
        assert!(unity_serialized::metadata(&data, &shorter).is_err(), "size {size}");
    }
    assert!(unity_serialized::metadata(&data, &header).is_ok());
}

#[test]
fn impossible_public_header_ranges_are_refused() {
    let (data, header) = fixture();
    for damaged in [
        Header { metadata_offset: u64::MAX, ..header },
        Header { metadata_size: u64::MAX, ..header },
        Header { metadata_offset: data.len() as u64 + 1, metadata_size: 0, ..header },
        Header { data_offset: header.metadata_offset, ..header },
        Header { file_size: header.metadata_offset, ..header },
    ] {
        assert!(unity_serialized::metadata(&data, &damaged).is_err());
    }
}

#[test]
fn a_whole_file_and_its_exact_metadata_prefix_produce_the_same_objects() {
    let (data, header) = fixture();
    let end = (header.metadata_offset + header.metadata_size) as usize;
    let whole = unity_serialized::metadata(&data, &header).unwrap();
    let prefix = unity_serialized::metadata(&data[..end], &header).unwrap();
    assert_eq!(whole, prefix);
    assert_eq!(whole.objects.len(), 4);
    assert_eq!(whole.externals, ["library/unity default resources"]);
}
