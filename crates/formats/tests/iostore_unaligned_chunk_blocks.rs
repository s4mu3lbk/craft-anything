use craft_anything_formats::unreal_iostore;

fn file_for_chunk(offset: u64, length: u64) -> unreal_iostore::File {
    let mut toc = unreal_iostore::toc(&craft_anything_testkit::games::unreal_utoc(9, u32::MAX)).unwrap();
    toc.chunks[0] = (offset, length);
    for (i, block) in toc.blocks.iter_mut().enumerate() {
        block.compressed_size = 10 * (i as u32 + 1);
        block.method = u8::from(i != 0);
    }
    unreal_iostore::files(&toc, 10).unwrap().1.into_iter().find(|file| file.chunk == 0).unwrap()
}

#[test]
fn unaligned_chunks_count_every_block_their_bytes_touch() {
    for (offset, length, stored) in [(65535, 2, 30), (1, 65536, 30), (65535, 65538, 60), (65537, 65536, 50)] {
        let file = file_for_chunk(offset, length);
        assert_eq!((file.size, file.stored_size), (length, stored), "offset {offset}, length {length}");
        assert_eq!(file.compression, if offset < 65536 { "none" } else { "Oodle" });
    }
}

#[test]
fn aligned_empty_and_exact_boundary_chunks_keep_their_sizes() {
    for (offset, length, stored) in [(0, 65536, 10), (65536, 65536, 20), (65535, 1, 10), (1, 65535, 10), (65535, 0, 0)] {
        let file = file_for_chunk(offset, length);
        assert_eq!((file.size, file.stored_size), (length, stored), "offset {offset}, length {length}");
    }
}
