use craft_anything_formats::elf;
use craft_anything_testkit::elf64_core;

fn core_with_files(declared: u64, rows: usize) -> Vec<u8> {
    let mut desc = Vec::new();
    desc.extend_from_slice(&declared.to_le_bytes());
    desc.extend_from_slice(&4096u64.to_le_bytes());
    for row in 0..rows {
        let start = (row as u64 + 1) * 4096;
        desc.extend_from_slice(&start.to_le_bytes());
        desc.extend_from_slice(&(start + 4096).to_le_bytes());
        desc.extend_from_slice(&0u64.to_le_bytes());
    }
    for _ in 0..rows {
        desc.extend_from_slice(b"/sample\0");
    }
    let mut note = Vec::new();
    note.extend_from_slice(&5u32.to_le_bytes());
    note.extend_from_slice(&(desc.len() as u32).to_le_bytes());
    note.extend_from_slice(&0x4649_4c45u32.to_le_bytes());
    note.extend_from_slice(b"CORE\0\0\0\0");
    note.extend_from_slice(&desc);
    let mut core = elf64_core(0, 0);
    let offset = core.len() as u64;
    core[0x48..0x50].copy_from_slice(&offset.to_le_bytes());
    core[0x60..0x68].copy_from_slice(&(note.len() as u64).to_le_bytes());
    core[0x68..0x70].copy_from_slice(&(note.len() as u64).to_le_bytes());
    core.extend_from_slice(&note);
    core
}

#[test]
fn capping_mappings_does_not_move_the_filename_table() {
    let count = 65_537;
    let core = elf::parse_core(&core_with_files(count as u64, count)).unwrap();
    assert_eq!(core.mapped_files.len(), 65_536);
    assert!(core.mapped_files.iter().all(|mapping| mapping.path == "/sample"));
    assert_eq!(core.mapped_files[0].start, 4096);
    assert_eq!(core.mapped_files[65_535].start, 65_536 * 4096);
}

#[test]
fn an_impossible_declared_table_cannot_supply_filename_bytes() {
    let core = elf::parse_core(&core_with_files(u64::MAX, 1)).unwrap();
    assert!(core.mapped_files.is_empty());
    assert!(core.warnings.iter().any(|warning| warning.contains("NT_FILE")));
}

#[test]
fn a_small_complete_mapping_table_is_unchanged() {
    let core = elf::parse_core(&core_with_files(2, 2)).unwrap();
    assert_eq!(core.mapped_files.len(), 2);
    assert_eq!(core.mapped_files[0].path, "/sample");
    assert_eq!(core.mapped_files[1].path, "/sample");
    assert_eq!(core.mapped_files[1].start, 8192);
}
