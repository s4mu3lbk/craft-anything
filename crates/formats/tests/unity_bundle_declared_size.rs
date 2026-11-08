use craft_anything_formats::{FormatError, unity_bundle};

struct Bundle {
    bytes: Vec<u8>,
    size_at: usize,
    header_end: u64,
    table_at: u64,
    data_at: u64,
}

fn bundle(version: u32, flags: u32) -> Bundle {
    let mut bytes = b"UnityFS\0".to_vec();
    bytes.extend_from_slice(&version.to_be_bytes());
    bytes.extend_from_slice(b"5.x.x\0fixture\0");
    let size_at = bytes.len();
    bytes.extend_from_slice(&0u64.to_be_bytes());
    bytes.extend_from_slice(&24u32.to_be_bytes());
    bytes.extend_from_slice(&24u32.to_be_bytes());
    bytes.extend_from_slice(&flags.to_be_bytes());
    if version >= 7 {
        bytes.resize(bytes.len().next_multiple_of(16), 0);
    }
    let header_end = bytes.len() as u64;
    let table_at;
    if flags & 0x80 == 0 {
        table_at = bytes.len() as u64;
        bytes.extend_from_slice(&[0; 24]); // Hash, zero blocks, zero nodes.
    } else {
        table_at = 0;
    }
    if flags & 0x200 != 0 {
        bytes.resize(bytes.len().next_multiple_of(16), 0);
    }
    let data_at = bytes.len() as u64;
    // A table with no blocks needs no data, but spare bytes are permitted in the data region.
    bytes.extend_from_slice(b"data!");
    let table_at = if flags & 0x80 != 0 {
        let at = bytes.len() as u64;
        bytes.extend_from_slice(&[0; 24]);
        at
    } else {
        table_at
    };
    let size = bytes.len() as u64;
    bytes[size_at..size_at + 8].copy_from_slice(&size.to_be_bytes());
    Bundle { bytes, size_at, header_end, table_at, data_at }
}

fn declare(bundle: &mut Bundle, size: u64) {
    bundle.bytes[bundle.size_at..bundle.size_at + 8].copy_from_slice(&size.to_be_bytes());
}

#[test]
fn trailing_bytes_do_not_move_the_declared_bundle_table() {
    for version in [6, 7, 8] {
        for flags in [0, 0x80, 0x200, 0x280] {
            let mut b = bundle(version, flags);
            let size = b.bytes.len() as u64;
            b.bytes.extend_from_slice(b"an unrelated suffix");
            let h = unity_bundle::header(&b.bytes, b.bytes.len() as u64).unwrap();
            assert_eq!((h.size, h.table_offset, h.data_offset), (size, b.table_at, b.data_at), "version {version}, flags {flags:#x}");
            let table = &b.bytes[h.table_offset as usize..h.table_offset as usize + h.table_size as usize];
            assert_eq!(unity_bundle::table(table, &h).unwrap(), (Vec::new(), Vec::new()));
        }
    }
}

#[test]
fn header_prefixes_describe_a_larger_bundle_without_reading_its_data() {
    let mut data = craft_anything_testkit::games::unity_bundle();
    assert!(data.len() > unity_bundle::HEADER_READ);
    let size = data.len() as u64;
    let expected = unity_bundle::header(&data, size).unwrap();
    assert_eq!(unity_bundle::header(&data[..unity_bundle::HEADER_READ], size).unwrap(), expected);
    data.extend_from_slice(b"suffix");
    assert_eq!(unity_bundle::header(&data[..unity_bundle::HEADER_READ], data.len() as u64).unwrap(), expected);
}

#[test]
fn declared_extent_must_contain_the_header_table_and_data_alignment() {
    for flags in [0, 0x80, 0x200, 0x280] {
        let b = bundle(8, flags);
        for size in [0, b.header_end - 1, b.bytes.len() as u64 + 1] {
            let mut bad = bundle(8, flags);
            declare(&mut bad, size);
            assert!(matches!(unity_bundle::header(&bad.bytes, bad.bytes.len() as u64), Err(FormatError::Malformed(_))), "flags {flags:#x}, size {size}");
        }
    }
    let mut front = bundle(8, 0);
    let size = front.table_at + 23;
    declare(&mut front, size);
    assert!(unity_bundle::header(&front.bytes, front.bytes.len() as u64).is_err(), "the front table crosses the declared end");

    let mut tail = bundle(8, 0x80);
    let size = tail.header_end + 23;
    declare(&mut tail, size);
    assert!(unity_bundle::header(&tail.bytes, tail.bytes.len() as u64).is_err(), "the tail table overlaps the header");

    let mut padded = bundle(8, 0x200);
    let size = padded.table_at + 24;
    assert!(size < padded.data_at);
    declare(&mut padded, size);
    assert!(unity_bundle::header(&padded.bytes, padded.bytes.len() as u64).is_err(), "data alignment crosses the declared end");

    let mut padded_tail = bundle(6, 0x280);
    assert!(padded_tail.header_end < padded_tail.data_at);
    let size = padded_tail.header_end + 24;
    declare(&mut padded_tail, size);
    assert!(unity_bundle::header(&padded_tail.bytes, padded_tail.bytes.len() as u64).is_err(), "the tail table overlaps aligned data");
}
