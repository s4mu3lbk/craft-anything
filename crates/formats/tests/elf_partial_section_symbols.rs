use craft_anything_formats::parse_binary;
use craft_anything_testkit::flutter::SymbolTable;
use craft_anything_testkit::sectionless::sectionless_elf;
use craft_anything_testkit::{ElfArch, elf64};

fn put(data: &mut [u8], at: usize, width: usize, value: u64, big: bool) {
    let bytes = if big { value.to_be_bytes() } else { value.to_le_bytes() };
    let bytes = if big { &bytes[8 - width..] } else { &bytes[..width] };
    data[at..at + width].copy_from_slice(bytes);
}

fn align(data: &mut Vec<u8>) {
    while !data.len().is_multiple_of(8) {
        data.push(0);
    }
}

fn with_retained_sections(is64: bool, big: bool) -> Vec<u8> {
    let mut data = sectionless_elf(is64, big, &["dynamic_entry"], SymbolTable::DynamicHash);
    let address = parse_binary(&data).unwrap().exports.iter().find(|symbol| symbol.name == "dynamic_entry").unwrap().addr;
    let strings = b"\0kept_local\0dynamic_entry\0";
    let strings_at = data.len();
    data.extend_from_slice(strings);
    align(&mut data);
    let symbols_at = data.len();
    let symbol_size = if is64 { 24 } else { 16 };
    data.resize(symbols_at + 3 * symbol_size, 0);
    for (index, (name, info)) in [(1u64, 2u8), (12, 0x12)].into_iter().enumerate() {
        let at = symbols_at + (index + 1) * symbol_size;
        put(&mut data, at, 4, name, big);
        if is64 {
            data[at + 4] = info;
            put(&mut data, at + 6, 2, 1, big);
            put(&mut data, at + 8, 8, address, big);
            put(&mut data, at + 16, 8, 16, big);
        } else {
            put(&mut data, at + 4, 4, address, big);
            put(&mut data, at + 8, 4, 16, big);
            data[at + 12] = info;
            put(&mut data, at + 14, 2, 1, big);
        }
    }
    let names = b"\0.text\0.symtab\0.strtab\0.shstrtab\0";
    let names_at = data.len();
    data.extend_from_slice(names);
    align(&mut data);
    let sections_at = data.len();
    let section_size = if is64 { 64 } else { 40 };
    data.resize(sections_at + 5 * section_size, 0);
    let rows = [
        (1u64, 1u64, 6u64, address, address, 16u64, 0u64, 0u64, 16u64, 0u64),
        (7, 2, 0, 0, symbols_at as u64, (3 * symbol_size) as u64, 3, 2, 8, symbol_size as u64),
        (15, 3, 0, 0, strings_at as u64, strings.len() as u64, 0, 0, 1, 0),
        (23, 3, 0, 0, names_at as u64, names.len() as u64, 0, 0, 1, 0),
    ];
    for (index, (name, kind, flags, addr, offset, size, link, info, alignment, entry_size)) in rows.into_iter().enumerate() {
        let at = sections_at + (index + 1) * section_size;
        put(&mut data, at, 4, name, big);
        put(&mut data, at + 4, 4, kind, big);
        if is64 {
            for (field, value) in [(8, flags), (16, addr), (24, offset), (32, size), (48, alignment), (56, entry_size)] {
                put(&mut data, at + field, 8, value, big);
            }
            put(&mut data, at + 40, 4, link, big);
            put(&mut data, at + 44, 4, info, big);
        } else {
            for (field, value) in [(8, flags), (12, addr), (16, offset), (20, size), (24, link), (28, info), (32, alignment), (36, entry_size)] {
                put(&mut data, at + field, 4, value, big);
            }
        }
    }
    if is64 {
        put(&mut data, 40, 8, sections_at as u64, big);
        put(&mut data, 58, 2, 64, big);
        put(&mut data, 60, 2, 5, big);
        put(&mut data, 62, 2, 4, big);
    } else {
        put(&mut data, 32, 4, sections_at as u64, big);
        put(&mut data, 46, 2, 40, big);
        put(&mut data, 48, 2, 5, big);
        put(&mut data, 50, 2, 4, big);
    }
    data
}

#[test]
fn retained_sections_without_dynsym_still_recover_dynamic_imports_and_exports() {
    for is64 in [false, true] {
        for big in [false, true] {
            let binary = parse_binary(&with_retained_sections(is64, big)).unwrap();
            assert!(binary.sections.iter().any(|section| section.name == ".text"));
            assert!(binary.symbols.iter().any(|symbol| symbol.name == "kept_local" && !symbol.dynamic));
            assert_eq!(binary.symbols.iter().filter(|symbol| symbol.name == "dynamic_entry").count(), 1);
            assert_eq!(binary.imports.iter().filter(|import| import.name == "puts").count(), 1);
            assert_eq!(binary.exports.iter().filter(|export| export.name == "dynamic_entry").count(), 1);
        }
    }
}

#[test]
fn complete_dynamic_sections_are_not_recovered_twice() {
    let (data, _) = elf64(ElfArch::X86_64, true);
    let binary = parse_binary(&data).unwrap();
    assert_eq!(binary.imports.iter().filter(|import| import.name == "puts").count(), 1);
    assert_eq!(binary.symbols.iter().filter(|symbol| symbol.name == "puts").count(), 1);
}
