use craft_anything_formats::binary::{Arch, Binary};
use craft_anything_formats::macho;
use craft_anything_testkit::Buf;

const LIMIT: usize = 500_000;
const INDIRECT: usize = 0x1020;

fn fixture(sections: &[(usize, u32)], indices: &[u32]) -> Binary {
    let mut b = Buf::with_len(INDIRECT + indices.len() * 4);
    b.u32(0, 0xfeed_facf);
    b.u32(4, 0x0100_0007);
    b.u32(12, 2);
    b.u32(16, 3);
    let segment_size = 72 + sections.len() * 80;
    b.u32(20, (segment_size + 24 + 80) as u32);
    b.u32(32, 0x19);
    b.u32(36, segment_size as u32);
    b.u32(32 + 64, sections.len() as u32);
    for (i, &(count, first)) in sections.iter().enumerate() {
        let at = 32 + 72 + i * 80;
        b.put(at, b"__slots");
        b.u64(at + 32, 0x1_0000_0000 + i as u64 * 0x100_0000);
        b.u64(at + 40, count as u64 * 8);
        b.u32(at + 64, 6);
        b.u32(at + 68, first);
    }
    let symtab = 32 + segment_size;
    b.u32(symtab, 2);
    b.u32(symtab + 4, 24);
    b.u32(symtab + 8, 0x1000);
    b.u32(symtab + 12, 1);
    b.u32(symtab + 16, 0x1010);
    b.u32(symtab + 20, 8);
    let dysymtab = symtab + 24;
    b.u32(dysymtab, 0xb);
    b.u32(dysymtab + 4, 80);
    b.u32(dysymtab + 56, INDIRECT as u32);
    b.u32(dysymtab + 60, indices.len() as u32);
    b.u32(0x1000, 1);
    b.put(0x1010, b"\0_entry\0");
    for (i, &index) in indices.iter().enumerate() {
        b.u32(INDIRECT + i * 4, index);
    }
    let data = b.into_vec();
    let mut binary = Binary::raw(Arch::X86_64, 0, data.len() as u64, 0);
    macho::enrich(&data, &mut binary);
    binary
}

#[test]
fn repeated_indirect_tables_share_one_output_and_scan_budget() {
    let binary = fixture(&[(LIMIT, 0), (1, 0)], &vec![0; LIMIT]);
    assert_eq!(binary.import_slots.len(), LIMIT);
    assert!(binary.warnings.iter().any(|warning| warning.contains("indirect") && warning.contains("limit")));
}

#[test]
fn local_and_invalid_indices_still_consume_the_global_scan_budget() {
    for sentinel in [0x8000_0000, 1] {
        let mut indices = vec![sentinel; LIMIT];
        indices.push(0);
        let binary = fixture(&[(LIMIT, 0), (1, LIMIT as u32)], &indices);
        assert!(binary.import_slots.is_empty());
        assert!(binary.warnings.iter().any(|warning| warning.contains("indirect") && warning.contains("limit")));
    }
}

#[test]
fn small_tables_and_an_exact_budget_are_not_marked_truncated() {
    let small = fixture(&[(2, 0), (1, 0)], &[0, 0]);
    assert_eq!(small.import_slots.len(), 3);
    assert!(small.warnings.is_empty());
    let exact = fixture(&[(LIMIT, 0)], &vec![0x8000_0000; LIMIT]);
    assert!(exact.import_slots.is_empty());
    assert!(exact.warnings.is_empty());
}
