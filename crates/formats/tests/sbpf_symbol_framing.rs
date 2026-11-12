use craft_anything_formats::solana;

fn header(elf: &[u8], index: usize) -> usize {
    u64::from_le_bytes(elf[40..48].try_into().unwrap()) as usize + index * 64
}

fn set_u64(elf: &mut [u8], offset: usize, value: u64) {
    elf[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}

#[test]
fn unrepresentable_symbol_offsets_do_not_panic() {
    let mut elf = craft_anything_testkit::sbpf::program();
    let symbols = header(&elf, 3);
    set_u64(&mut elf, symbols + 24, u64::MAX);
    let report = solana::inspect(&elf).unwrap();
    assert!(report.symbols.is_empty());
    assert!(report.warnings.iter().any(|w| w.contains("symbol") && w.contains("truncated")));
}

#[test]
fn short_or_absent_symbol_strides_are_not_replaced_with_a_guessed_layout() {
    for stride in [0, 8, 23] {
        let mut elf = craft_anything_testkit::sbpf::program();
        let symbols = header(&elf, 3);
        set_u64(&mut elf, symbols + 56, stride);
        let report = solana::inspect(&elf).unwrap();
        assert!(report.symbols.is_empty(), "stride {stride}");
        assert!(report.warnings.iter().any(|w| w.contains("symbol") && w.contains("size")));
    }
}

#[test]
fn symbol_names_require_a_linked_string_table_and_a_terminator() {
    let original = craft_anything_testkit::sbpf::program();
    for wrong_type in [false, true] {
        let mut elf = original.clone();
        let strings = header(&elf, 2);
        if wrong_type {
            elf[strings + 4..strings + 8].copy_from_slice(&1u32.to_le_bytes());
        } else {
            // End inside the first name; unrelated bytes still follow in the file.
            set_u64(&mut elf, strings + 32, 8);
        }
        let report = solana::inspect(&elf).unwrap();
        assert!(report.symbols.iter().all(|s| s.name.is_empty()));
        assert!(report.relocations.iter().all(|r| r.symbol.is_empty()));
        assert!(report.syscalls.is_empty());
    }
    let report = solana::inspect(&original).unwrap();
    assert_eq!(report.entry_symbol.as_deref(), Some("entrypoint"));
    assert_eq!(report.syscalls[0].name, "sol_log_");
}

#[test]
fn symbol_strides_include_their_declared_padding() {
    let mut elf = craft_anything_testkit::sbpf::program();
    let symbols = header(&elf, 3);
    let old = u64::from_le_bytes(elf[symbols + 24..symbols + 32].try_into().unwrap()) as usize;
    let records = elf[old..old + 72].to_vec();
    let new = elf.len();
    for record in records.as_chunks::<24>().0 {
        elf.extend_from_slice(record);
        elf.extend_from_slice(&[0; 8]);
    }
    set_u64(&mut elf, symbols + 24, new as u64);
    set_u64(&mut elf, symbols + 32, 96);
    set_u64(&mut elf, symbols + 56, 32);
    assert_eq!(solana::inspect(&elf).unwrap().symbols.len(), 2);
    let report = solana::inspect(&elf[..elf.len() - 8]).unwrap();
    assert_eq!(report.symbols.len(), 1);
    assert_eq!(report.symbols[0].name, "sol_log_");
    assert!(report.warnings.iter().any(|w| w.contains("symbol") && w.contains("truncated")));
}

#[test]
fn partial_trailing_symbol_records_are_reported() {
    let mut elf = craft_anything_testkit::sbpf::program();
    let symbols = header(&elf, 3);
    set_u64(&mut elf, symbols + 32, 73);
    let report = solana::inspect(&elf).unwrap();
    assert_eq!(report.symbols.len(), 2);
    assert!(report.warnings.iter().any(|w| w.contains("symbol") && w.contains("partial")));
}
