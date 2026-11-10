use craft_anything_formats::binary::{Arch, Binary, Section};
use craft_anything_formats::elf;

fn put(data: &mut [u8], at: usize, width: usize, value: u64, big: bool) {
    let bytes = if big { value.to_be_bytes() } else { value.to_le_bytes() };
    let bytes = if big { &bytes[8 - width..] } else { &bytes[..width] };
    data[at..at + width].copy_from_slice(bytes);
}

fn section(name: &str, at: usize, len: usize) -> Section {
    Section {
        name: name.into(),
        segment: None,
        addr: at as u64,
        size: len as u64,
        file_offset: Some(at as u64),
        file_size: len as u64,
        read: true,
        write: false,
        exec: false,
        kind: "metadata",
        entropy: None,
        alloc: false,
    }
}

fn fixture(big: bool) -> (Vec<u8>, Binary) {
    let mut data = vec![0; 0x600];
    data[..4].copy_from_slice(b"\x7fELF");
    data[4] = 2;
    data[5] = if big { 2 } else { 1 };
    for (index, (name, defined)) in [(1u64, true), (0x20, false)].into_iter().enumerate() {
        let at = 0x100 + (index + 1) * 24;
        put(&mut data, at, 4, name, big);
        data[at + 4] = 0x12;
        put(&mut data, at + 6, 2, u64::from(defined), big);
        put(&mut data, at + 8, 8, if defined { 0x1000 } else { 0 }, big);
    }
    for (at, text) in [(0x201, b"offered\0".as_slice()), (0x220, b"needed\0"), (0x240, b"VERSION_1\0"), (0x260, b"libsample.so\0")] {
        data[at..at + text.len()].copy_from_slice(text);
    }
    for (at, width, value) in [
        (0x302, 2, 2u64),
        (0x304, 2, 3),
        (0x400, 2, 1),
        (0x404, 2, 2),
        (0x406, 2, 1),
        (0x40c, 4, 20),
        (0x414, 4, 0x40),
        (0x500, 2, 1),
        (0x502, 2, 1),
        (0x504, 4, 0x60),
        (0x508, 4, 16),
        (0x516, 2, 3),
        (0x518, 4, 0x40),
    ] {
        put(&mut data, at, width, value, big);
    }
    let mut binary = Binary::raw(Arch::X86_64, 0, data.len() as u64, 0);
    binary.sections = vec![
        section(".dynsym", 0x100, 72),
        section(".dynstr", 0x200, 128),
        section(".gnu.version", 0x300, 6),
        section(".gnu.version_d", 0x400, 28),
        section(".gnu.version_r", 0x500, 32),
    ];
    (data, binary)
}

fn cut(binary: &mut Binary, name: &str, len: u64) {
    let section = binary.sections.iter_mut().find(|section| section.name == name).unwrap();
    section.size = len;
    section.file_size = len;
}

#[test]
fn definitions_cannot_borrow_their_auxiliary_record_from_after_the_section() {
    for big in [false, true] {
        for len in [0, 16, 20, 23] {
            let (data, mut binary) = fixture(big);
            cut(&mut binary, ".gnu.version_d", len);
            let symbols = elf::dynamic_symbols(&data, &binary).unwrap();
            assert_eq!(symbols.exports[0].version, None, "big={big}, len={len}");
            assert_eq!(symbols.imports[0].version.as_deref(), Some("VERSION_1"));
        }
    }
}

#[test]
fn requirements_cannot_borrow_a_partial_auxiliary_record() {
    for big in [false, true] {
        for len in [0, 15, 16, 28] {
            let (data, mut binary) = fixture(big);
            cut(&mut binary, ".gnu.version_r", len);
            let symbols = elf::dynamic_symbols(&data, &binary).unwrap();
            assert_eq!(symbols.imports[0].version, None, "big={big}, len={len}");
            assert_eq!(symbols.imports[0].library, None);
            assert_eq!(symbols.exports[0].version.as_deref(), Some("VERSION_1"));
        }
    }
}

#[test]
fn version_indices_stop_at_their_declared_section() {
    for big in [false, true] {
        let (data, mut binary) = fixture(big);
        cut(&mut binary, ".gnu.version", 4);
        let symbols = elf::dynamic_symbols(&data, &binary).unwrap();
        assert_eq!(symbols.exports[0].version.as_deref(), Some("VERSION_1"));
        assert_eq!(symbols.imports[0].version, None);
        assert_eq!(symbols.imports[0].library, None);
    }
}

#[test]
fn complete_version_sections_keep_names_and_libraries() {
    for big in [false, true] {
        let (data, binary) = fixture(big);
        let symbols = elf::dynamic_symbols(&data, &binary).unwrap();
        assert_eq!(symbols.exports[0].version.as_deref(), Some("VERSION_1"));
        assert_eq!(symbols.imports[0].version.as_deref(), Some("VERSION_1"));
        assert_eq!(symbols.imports[0].library.as_deref(), Some("libsample.so"));
    }
}
