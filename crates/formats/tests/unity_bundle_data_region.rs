use craft_anything_formats::{FormatError, unity_bundle};

/// A stored table describing blocks without requiring their contents to decompress.
fn bundle(version: u32, flags: u32, sizes: &[u32], data_len: usize) -> (Vec<u8>, Vec<u8>) {
    let mut table = vec![0; 16];
    table.extend_from_slice(&(sizes.len() as i32).to_be_bytes());
    for (i, size) in sizes.iter().enumerate() {
        table.extend_from_slice(&1u32.to_be_bytes());
        table.extend_from_slice(&size.to_be_bytes());
        table.extend_from_slice(&((i % 4) as u16).to_be_bytes());
    }
    table.extend_from_slice(&0i32.to_be_bytes());
    let mut bytes = b"UnityFS\0".to_vec();
    bytes.extend_from_slice(&version.to_be_bytes());
    bytes.extend_from_slice(b"fixture\0fixture\0");
    let size_at = bytes.len();
    bytes.extend_from_slice(&0u64.to_be_bytes());
    bytes.extend_from_slice(&(table.len() as u32).to_be_bytes());
    bytes.extend_from_slice(&(table.len() as u32).to_be_bytes());
    bytes.extend_from_slice(&flags.to_be_bytes());
    if version >= 7 {
        bytes.resize(bytes.len().next_multiple_of(16), 0);
    }
    if flags & 0x80 == 0 {
        bytes.extend_from_slice(&table);
    }
    if flags & 0x200 != 0 {
        bytes.resize(bytes.len().next_multiple_of(16), 0);
    }
    bytes.resize(bytes.len() + data_len, 0);
    if flags & 0x80 != 0 {
        bytes.extend_from_slice(&table);
    }
    let size = bytes.len() as u64;
    bytes[size_at..size_at + 8].copy_from_slice(&size.to_be_bytes());
    (bytes, table)
}

/// Rejects individual and cumulative overruns even when table or suffix bytes could fill them.
#[test]
fn blocks_cannot_borrow_trailing_table_or_appended_bytes() {
    for version in [6, 7, 8] {
        for flags in [0, 0x80, 0x200, 0x280] {
            for sizes in [&[6][..], &[2, 0, 3, 1][..]] {
                let (mut bytes, table) = bundle(version, flags, sizes, 5);
                bytes.extend_from_slice(&[0; 64]);
                let h = unity_bundle::header(&bytes, bytes.len() as u64).unwrap();
                assert!(matches!(unity_bundle::table(&table, &h), Err(FormatError::Malformed(_))), "version {version}, flags {flags:#x}, sizes {sizes:?}");
            }
        }
    }
}

/// Preserves exact fits, unused data bytes and zero-length blocks across supported layouts.
#[test]
fn exact_boundaries_spare_data_and_empty_blocks_remain_valid() {
    for version in [6, 7, 8] {
        for flags in [0, 0x80, 0x200, 0x280] {
            for sizes in [&[][..], &[0][..], &[5][..], &[2, 0, 3, 0][..]] {
                for data_len in [5, 7] {
                    let (bytes, table) = bundle(version, flags, sizes, data_len);
                    let h = unity_bundle::header(&bytes, bytes.len() as u64).unwrap();
                    let (blocks, nodes) = unity_bundle::table(&table, &h).unwrap();
                    assert_eq!(blocks.iter().map(|b| b.compressed_size).collect::<Vec<_>>(), sizes);
                    assert!(nodes.is_empty());
                }
            }
            let (bytes, table) = bundle(version, flags, &[0, 0], 0);
            let h = unity_bundle::header(&bytes, bytes.len() as u64).unwrap();
            assert_eq!(unity_bundle::table(&table, &h).unwrap().0.len(), 2);
        }
    }
}

/// Excludes the alignment gap from the space available for compressed block contents.
#[test]
fn alignment_padding_is_not_available_to_blocks() {
    for flags in [0x200, 0x280] {
        let (bytes, table) = bundle(6, flags, &[1], 0);
        let h = unity_bundle::header(&bytes, bytes.len() as u64).unwrap();
        assert_eq!(h.data_offset % 16, 0);
        assert!(unity_bundle::table(&table, &h).is_err());
    }
}

/// Checks totals above 32 bits using a large declared extent without allocating its contents.
#[test]
fn cumulative_block_sizes_do_not_wrap_at_32_bits() {
    let (bytes, table) = bundle(8, 0, &[u32::MAX, 2], 1);
    let mut h = unity_bundle::header(&bytes, bytes.len() as u64).unwrap();
    // A header-only reader can describe a multi-gigabyte file without allocating it.
    h.size = h.data_offset + u64::from(u32::MAX) + 1;
    assert!(unity_bundle::table(&table, &h).is_err());
    h.size += 1;
    assert_eq!(unity_bundle::table(&table, &h).unwrap().0.len(), 2);
}
