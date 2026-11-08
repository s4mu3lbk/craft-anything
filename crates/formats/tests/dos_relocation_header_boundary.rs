use craft_anything_formats::dos;
use craft_anything_testkit::dos_mz;

#[test]
fn relocation_records_cannot_extend_into_the_load_module() {
    let mut data = dos_mz();
    data[8..10].copy_from_slice(&2u16.to_le_bytes());
    let parsed = dos::parse_mz(&data).unwrap();
    assert_eq!(parsed.header_bytes, 32);
    assert_eq!(parsed.relocations.len(), 1);
    assert_eq!(parsed.relocations[0].offset, 3);
    assert!(!parsed.problems.is_empty());
}

#[test]
fn a_partial_relocation_does_not_borrow_the_module_start() {
    let mut data = dos_mz();
    data[6..8].copy_from_slice(&1u16.to_le_bytes());
    data[8..10].copy_from_slice(&2u16.to_le_bytes());
    data[24..26].copy_from_slice(&30u16.to_le_bytes());
    let parsed = dos::parse_mz(&data).unwrap();
    assert!(parsed.relocations.is_empty());
    assert!(!parsed.problems.is_empty());
}

#[test]
fn relocation_records_ending_at_the_header_boundary_are_preserved() {
    let original = dos::parse_mz(&dos_mz()).unwrap();
    assert_eq!(original.relocations.len(), 2);
    let mut data = dos_mz();
    data[6..8].copy_from_slice(&1u16.to_le_bytes());
    data[8..10].copy_from_slice(&2u16.to_le_bytes());
    let parsed = dos::parse_mz(&data).unwrap();
    assert_eq!(parsed.relocations.len(), 1);
    assert_eq!(parsed.relocations[0], original.relocations[0]);
}
