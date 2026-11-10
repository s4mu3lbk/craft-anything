use craft_anything_formats::binary::{Arch, Binary};
use craft_anything_formats::macho;
use craft_anything_testkit::Buf;

fn state(flavor: u32, count: u32, bytes: usize, pc_offset: usize, pc: u64, wide: bool) -> Vec<u8> {
    let mut b = Buf::with_len(8 + bytes);
    b.u32(0, flavor);
    b.u32(4, count);
    if wide {
        b.u64(8 + pc_offset, pc);
    } else {
        b.u32(8 + pc_offset, pc as u32);
    }
    b.into_vec()
}

fn decode(cpu: u32, wide: bool, states: &[Vec<u8>]) -> Binary {
    let header = if wide { 32 } else { 28 };
    let length = 8 + states.iter().map(Vec::len).sum::<usize>();
    let mut b = Buf::with_len(header + length);
    b.u32(0, if wide { 0xfeed_facf } else { 0xfeed_face });
    b.u32(4, cpu);
    b.u32(12, 2);
    b.u32(16, 1);
    b.u32(20, length as u32);
    b.u32(header, 5);
    b.u32(header + 4, length as u32);
    let mut offset = header + 8;
    for record in states {
        b.put(offset, record);
        offset += record.len();
    }
    let data = b.into_vec();
    let mut binary = Binary::raw(if wide { Arch::X86_64 } else { Arch::X86 }, 0, data.len() as u64, 0);
    macho::enrich(&data, &mut binary);
    binary
}

#[test]
fn a_program_counter_requires_a_complete_declared_state() {
    for count in [0, 1, 34, 41] {
        assert_eq!(decode(0x0100_0007, true, &[state(4, count, 168, 128, 0x1234, true)]).entry, None, "count {count}");
    }
    assert_eq!(decode(0x0100_0007, true, &[state(4, 42, 136, 128, 0x1234, true)]).entry, None);
    assert_eq!(decode(0x0100_000c, true, &[state(6, 68, 264, 256, 0x1234, true)]).entry, None);
}

#[test]
fn flavor_numbers_are_interpreted_for_the_image_cpu() {
    assert_eq!(decode(0x0100_000c, true, &[state(4, 42, 168, 128, 0x1234, true)]).entry, None);
    assert_eq!(decode(0x0100_0007, true, &[state(6, 68, 272, 256, 0x1234, true)]).entry, None);
    assert_eq!(decode(12, false, &[state(1, 17, 68, 40, 0x1234, false)]).entry, None);
}

#[test]
fn a_later_matching_state_is_reached_using_word_counts() {
    let unknown = state(99, 2, 8, 0, 0, true);
    let matching = state(4, 42, 168, 128, 0x1234, true);
    assert_eq!(decode(0x0100_0007, true, &[unknown, matching]).entry, Some(0x1234));
}

#[test]
fn complete_supported_states_keep_their_program_counters() {
    assert_eq!(decode(0x0100_0007, true, &[state(4, 42, 168, 128, 0x1234, true)]).entry, Some(0x1234));
    assert_eq!(decode(0x0100_000c, true, &[state(6, 68, 272, 256, 0x1234, true)]).entry, Some(0x1234));
    assert_eq!(decode(7, false, &[state(1, 16, 64, 40, 0x1234, false)]).entry, Some(0x1234));
}
