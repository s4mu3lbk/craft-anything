use craft_anything_formats::binary::{Arch, Binary};
use craft_anything_formats::macho;

fn function_starts(payload: &[u8], following: &[u8], base: u64) -> Binary {
    let mut data = vec![0; 120];
    data[..4].copy_from_slice(&0xfeed_facfu32.to_le_bytes());
    data[16..20].copy_from_slice(&2u32.to_le_bytes());
    data[20..24].copy_from_slice(&88u32.to_le_bytes());
    data[32..36].copy_from_slice(&0x19u32.to_le_bytes());
    data[36..40].copy_from_slice(&72u32.to_le_bytes());
    data[40..46].copy_from_slice(b"__TEXT");
    data[56..64].copy_from_slice(&base.to_le_bytes());
    data[104..108].copy_from_slice(&0x26u32.to_le_bytes());
    data[108..112].copy_from_slice(&16u32.to_le_bytes());
    data[112..116].copy_from_slice(&120u32.to_le_bytes());
    data[116..120].copy_from_slice(&(payload.len() as u32).to_le_bytes());
    data.extend_from_slice(payload);
    data.extend_from_slice(following);
    let mut binary = Binary::raw(Arch::X86_64, 0, 0, 0);
    macho::enrich(&data, &mut binary);
    binary
}

#[test]
fn a_function_delta_cannot_finish_in_adjacent_payload_bytes() {
    let binary = function_starts(&[0x80], &[0x01], 0x1000);
    assert!(binary.function_starts.is_empty());
    assert!(!binary.warnings.is_empty());
}

#[test]
fn complete_deltas_before_a_cut_off_one_are_preserved() {
    let binary = function_starts(&[4, 0x80], &[0x01], 0x1000);
    assert_eq!(binary.function_starts, [0x1004]);
}

#[test]
fn function_addresses_do_not_wrap_on_overflow() {
    let binary = function_starts(&[5, 0], &[], u64::MAX - 4);
    assert!(binary.function_starts.is_empty());
    assert!(!binary.warnings.is_empty());
}

#[test]
fn complete_starts_before_an_overflowing_delta_are_preserved() {
    let binary = function_starts(&[4, 1, 0], &[], u64::MAX - 4);
    assert_eq!(binary.function_starts, [u64::MAX]);
    assert!(!binary.warnings.is_empty());
}

#[test]
fn complete_multibyte_deltas_and_the_terminator_are_preserved() {
    let binary = function_starts(&[0x80, 0x01, 0x10, 0], &[0x7f], 0x1000);
    assert_eq!(binary.function_starts, [0x1080, 0x1090]);
    assert!(binary.warnings.is_empty());
}
