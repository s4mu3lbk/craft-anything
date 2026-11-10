use craft_anything_formats::pe;
use craft_anything_testkit::pe64;

const LIMIT: usize = 200_000;
const FUNCTIONS: usize = 0x800;

fn fixture(functions: usize, names: usize) -> Vec<u8> {
    let name_table = FUNCTIONS + functions * 4;
    let ordinal_table = name_table + names * 4;
    let text = ordinal_table + names * 2;
    let mut data = pe64();
    data.resize(text + 16, 0);
    data[FUNCTIONS..].fill(0);
    let rva = |offset: usize| 0x3000 + (offset - FUNCTIONS) as u32;
    for (at, value) in [(0x7b4, functions as u32), (0x7b8, names as u32), (0x7bc, rva(FUNCTIONS)), (0x7c0, rva(name_table)), (0x7c4, rva(ordinal_table))] {
        data[at..at + 4].copy_from_slice(&value.to_le_bytes());
    }
    let section = 0x98 + 240 + 2 * 40;
    let size = (data.len() - FUNCTIONS) as u32;
    data[section + 8..section + 12].copy_from_slice(&size.to_le_bytes());
    data[section + 16..section + 20].copy_from_slice(&size.to_le_bytes());
    for i in 0..names {
        let at = name_table + i * 4;
        data[at..at + 4].copy_from_slice(&rva(text).to_le_bytes());
        let at = ordinal_table + i * 2;
        data[at..at + 2].copy_from_slice(&1u16.to_le_bytes());
    }
    data[text..text + 6].copy_from_slice(b"alias\0");
    data
}

fn function(data: &mut [u8], index: usize, address: u32) {
    let at = FUNCTIONS + index * 4;
    data[at..at + 4].copy_from_slice(&address.to_le_bytes());
}

#[test]
fn aliases_share_the_export_row_budget_with_ordinal_only_functions() {
    let mut data = fixture(2, LIMIT);
    function(&mut data, 0, 0x1000);
    function(&mut data, 1, 0x1010);
    let table = pe::export_table(&data, &pe::headers(&data).unwrap()).unwrap();
    assert_eq!(table.entries.len(), LIMIT);
    assert!(table.truncated);
    assert_eq!(table.entries[0].name, None);
    assert!(table.entries[1..].iter().all(|entry| entry.name.as_deref() == Some("alias")));
}

#[test]
fn empty_function_slots_still_consume_the_scan_budget() {
    let mut data = fixture(LIMIT + 1, 0);
    function(&mut data, LIMIT, 0x1000);
    let table = pe::export_table(&data, &pe::headers(&data).unwrap()).unwrap();
    assert!(table.entries.is_empty());
    assert!(table.truncated);
    assert_eq!(table.declared_functions, (LIMIT + 1) as u32);
    assert!(!table.warnings.is_empty());
}

#[test]
fn ordinary_aliases_and_ordinal_only_exports_are_unchanged() {
    let mut data = fixture(2, 1);
    function(&mut data, 0, 0x1000);
    function(&mut data, 1, 0x1010);
    let table = pe::export_table(&data, &pe::headers(&data).unwrap()).unwrap();
    assert!(!table.truncated);
    assert_eq!(table.entries.len(), 2);
    assert_eq!(table.entries[0].ordinal, 1);
    assert_eq!(table.entries[0].name, None);
    assert_eq!(table.entries[1].ordinal, 2);
    assert_eq!(table.entries[1].name.as_deref(), Some("alias"));
}
