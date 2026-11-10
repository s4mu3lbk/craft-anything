use craft_anything_formats::elf;
use craft_anything_testkit::elf64_core;

fn core_with_names(names: &[u8], count: u64) -> Vec<u8> {
    let mut desc = Vec::new();
    desc.extend_from_slice(&count.to_le_bytes());
    desc.extend_from_slice(&4096u64.to_le_bytes());
    for row in 0..count {
        let start = (row + 1) * 4096;
        desc.extend_from_slice(&start.to_le_bytes());
        desc.extend_from_slice(&(start + 4096).to_le_bytes());
        desc.extend_from_slice(&0u64.to_le_bytes());
    }
    desc.extend_from_slice(names);
    let mut note = Vec::new();
    note.extend_from_slice(&5u32.to_le_bytes());
    note.extend_from_slice(&(desc.len() as u32).to_le_bytes());
    note.extend_from_slice(&0x4649_4c45u32.to_le_bytes());
    note.extend_from_slice(b"CORE\0\0\0\0");
    note.extend_from_slice(&desc);
    let mut data = elf64_core(0, 0);
    let offset = data.len() as u64;
    data[0x48..0x50].copy_from_slice(&offset.to_le_bytes());
    data[0x60..0x68].copy_from_slice(&(note.len() as u64).to_le_bytes());
    data[0x68..0x70].copy_from_slice(&(note.len() as u64).to_le_bytes());
    data.extend_from_slice(&note);
    data
}

#[test]
fn lossy_path_text_does_not_shift_the_next_mapping() {
    let core = elf::parse_core(&core_with_names(b"/bad-\xff\0/second\0", 2)).unwrap();
    assert_eq!(core.mapped_files[0].path, "/bad-\u{fffd}");
    assert_eq!(core.mapped_files[1].path, "/second");
    assert_eq!(core.mapped_files[1].start, 8192);
}

#[test]
fn capped_path_preview_skips_the_whole_original_path() {
    let mut names = vec![b'a'; 5000];
    names.extend_from_slice(b"\0/second\0");
    let core = elf::parse_core(&core_with_names(&names, 2)).unwrap();
    assert_eq!(core.mapped_files[0].path, "a".repeat(4096));
    assert_eq!(core.mapped_files[1].path, "/second");
}

#[test]
fn complete_utf8_paths_and_an_unterminated_last_path_keep_their_text() {
    let core = elf::parse_core(&core_with_names("/sample/界\0/last".as_bytes(), 2)).unwrap();
    assert_eq!(core.mapped_files[0].path, "/sample/界");
    assert_eq!(core.mapped_files[1].path, "/last");
}
