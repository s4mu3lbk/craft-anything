use craft_anything_formats::{FormatError, unity_bundle};

fn lzma_piece(plain: &[u8], end_marker: bool) -> Vec<u8> {
    let options =
        lzma_rs::compress::Options { unpacked_size: lzma_rs::compress::UnpackedSize::WriteToHeader(if end_marker { None } else { Some(plain.len() as u64) }) };
    let mut alone = Vec::new();
    lzma_rs::lzma_compress_with_options(&mut std::io::Cursor::new(plain), &mut alone, &options).unwrap();
    alone.drain(5..13);
    alone
}

#[test]
fn stored_blocks_must_match_the_declared_output_length() {
    for expected in [0, 2, 4] {
        assert!(matches!(unity_bundle::decompress(0, b"abc", expected), Err(FormatError::Malformed(_))), "declared {expected} bytes");
    }
    assert!(unity_bundle::decompress(0, &[], 1).is_err());
    assert_eq!(unity_bundle::decompress(0, b"abc", 3).unwrap(), b"abc");
    assert!(unity_bundle::decompress(0, &[], 0).unwrap().is_empty());
}

#[test]
fn compressed_blocks_keep_their_exact_length_contract() {
    let plain = b"a small Unity bundle block";
    let lz4 = craft_anything_testkit::games::lz4_literals(plain);
    for kind in [2, 3] {
        assert_eq!(unity_bundle::decompress(kind, &lz4, plain.len() as u32).unwrap(), plain);
        assert!(unity_bundle::decompress(kind, &lz4, plain.len() as u32 + 1).is_err());
        assert!(unity_bundle::decompress(kind, &lz4, plain.len() as u32 - 1).is_err());
    }
    let unity_lzma = lzma_piece(plain, true);
    assert_eq!(unity_bundle::decompress(1, &unity_lzma, plain.len() as u32).unwrap(), plain);
    assert!(unity_bundle::decompress(1, &unity_lzma, plain.len() as u32 + 1).is_err());
}

#[test]
fn lzma_rejects_short_and_long_declarations_after_checking_completion() {
    let plain = b"a small Unity bundle block";
    for end_marker in [false, true] {
        let piece = lzma_piece(plain, end_marker);
        assert_eq!(unity_bundle::decompress(1, &piece, plain.len() as u32).unwrap(), plain);
        for declared in [0, 1, plain.len() as u32 - 1, plain.len() as u32 + 1] {
            assert!(unity_bundle::decompress(1, &piece, declared).is_err(), "end marker {end_marker}, declared {declared}");
        }
    }
}

#[test]
fn lzma_keeps_empty_and_short_zero_streams_without_requiring_end_markers() {
    for end_marker in [false, true] {
        for len in 0..=64 {
            let plain = vec![0; len];
            let piece = lzma_piece(&plain, end_marker);
            assert_eq!(unity_bundle::decompress(1, &piece, len as u32).unwrap(), plain, "end marker {end_marker}, length {len}");
        }
    }
}

#[test]
fn lzma_bounds_dictionary_growth_and_output_across_window_flushes() {
    let plain = vec![0xa5; 8192];
    for end_marker in [false, true] {
        let mut piece = lzma_piece(&plain, end_marker);
        for dictionary in [4096u32, u32::MAX] {
            piece[1..5].copy_from_slice(&dictionary.to_le_bytes());
            assert_eq!(unity_bundle::decompress(1, &piece, plain.len() as u32).unwrap(), plain);
            assert!(unity_bundle::decompress(1, &piece, 5000).is_err(), "end marker {end_marker}, dictionary {dictionary}");
        }
    }
}

#[test]
fn lzma_limits_an_oversized_dictionary_before_probing_past_a_tiny_output() {
    for end_marker in [false, true] {
        let mut piece = lzma_piece(b"ab", end_marker);
        piece[1..5].copy_from_slice(&u32::MAX.to_le_bytes());
        assert_eq!(unity_bundle::decompress(1, &piece, 2).unwrap(), b"ab");
        assert!(unity_bundle::decompress(1, &piece, 1).is_err());
    }
}

#[cfg(target_pointer_width = "32")]
#[test]
fn lzma_rejects_output_lengths_that_exceed_the_target_vector_limit() {
    let piece = lzma_piece(b"", false);
    assert!(unity_bundle::decompress(1, &piece, (isize::MAX as u32) + 1).is_err());
}

#[test]
fn lzma_rejects_truncated_or_unused_compressed_bytes() {
    let plain = b"a small Unity bundle block";
    for end_marker in [false, true] {
        let piece = lzma_piece(plain, end_marker);
        assert!(unity_bundle::decompress(1, &piece[..piece.len() - 1], plain.len() as u32).is_err());
        let mut trailing = piece;
        trailing.extend_from_slice(b"unused");
        assert!(unity_bundle::decompress(1, &trailing, plain.len() as u32).is_err());
    }
}
