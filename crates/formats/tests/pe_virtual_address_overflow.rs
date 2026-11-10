use craft_anything_formats::{parse_binary, pe};
use craft_anything_testkit::{PE_IAT_RVA, pe64};

fn image(plus: bool, base: u64, entry: u32) -> Vec<u8> {
    let mut data = pe64();
    let opt = 0x98;
    data[opt + 16..opt + 20].copy_from_slice(&entry.to_le_bytes());
    if plus {
        data[opt + 24..opt + 32].copy_from_slice(&base.to_le_bytes());
    } else {
        let directories = pe::headers(&data).unwrap().data_dirs;
        data[0x84..0x86].copy_from_slice(&0x14cu16.to_le_bytes());
        data[opt..opt + 2].copy_from_slice(&0x10bu16.to_le_bytes());
        data[opt + 28..opt + 32].copy_from_slice(&(base as u32).to_le_bytes());
        data[opt + 92..opt + 96].copy_from_slice(&16u32.to_le_bytes());
        for (index, (rva, size)) in directories.into_iter().enumerate() {
            let at = opt + 96 + index * 8;
            data[at..at + 4].copy_from_slice(&rva.to_le_bytes());
            data[at + 4..at + 8].copy_from_slice(&size.to_le_bytes());
        }
    }
    data
}

#[test]
fn image_base_overflow_leaves_entry_and_import_slot_unknown() {
    for (plus, base) in [(true, u64::MAX - 0x1000), (false, u64::from(u32::MAX) - 0x1000)] {
        let data = image(plus, base, 0x2000);
        let headers = pe::headers(&data).unwrap();
        let imports = pe::import_table(&data, &headers);
        assert_eq!(imports.rows[0].name.as_deref(), Some("puts"));
        assert_eq!(imports.rows[0].slot_va, None);
        assert!(pe::imports(&data, &headers).is_empty());
        let binary = parse_binary(&data).unwrap();
        assert_eq!(binary.entry, None);
        let import = binary.imports.iter().find(|import| import.name == "puts").unwrap();
        assert_eq!(import.slot, None);
        assert!(binary.import_slots.is_empty());
    }
}

#[test]
fn delay_slots_and_unwind_starts_do_not_wrap() {
    let mut data = image(true, u64::MAX - 0x1000, 0);
    for (at, value) in [(0x800, 1u32), (0x804, 0x2190), (0x80c, 0x3040), (0x810, 0x2140), (0x880, 0x2000)] {
        data[at..at + 4].copy_from_slice(&value.to_le_bytes());
    }
    let directories = 0x98 + 112;
    for (index, rva, size) in [(13, 0x3000u32, 64u32), (3, 0x3080, 12)] {
        let at = directories + index * 8;
        data[at..at + 4].copy_from_slice(&rva.to_le_bytes());
        data[at + 4..at + 8].copy_from_slice(&size.to_le_bytes());
    }
    let headers = pe::headers(&data).unwrap();
    let imports = pe::import_table(&data, &headers);
    assert!(imports.rows.iter().any(|import| import.kind == pe::ImportKind::Delay));
    assert!(imports.rows.iter().all(|import| import.slot_va.is_none()));
    let binary = parse_binary(&data).unwrap();
    assert!(binary.function_starts.is_empty());
    assert!(binary.import_slots.is_empty());
}

#[test]
fn an_iat_index_cannot_leave_the_32_bit_rva_space() {
    let mut data = image(false, 0, 0);
    data[0x710..0x714].copy_from_slice(&0xffff_fffcu32.to_le_bytes());
    data[0x744..0x748].copy_from_slice(&0x2180u32.to_le_bytes());
    let headers = pe::headers(&data).unwrap();
    let imports = pe::import_table(&data, &headers);
    assert_eq!(imports.rows.len(), 2);
    assert_eq!(imports.rows[0].slot_va, Some(u64::from(0xffff_fffcu32)));
    assert_eq!(imports.rows[1].slot_va, None);
}

#[test]
fn representable_high_addresses_still_decode() {
    for (plus, base) in [(true, u64::MAX - 0x4000), (false, 0xfff0_0000)] {
        let data = image(plus, base, 0x1000);
        let binary = parse_binary(&data).unwrap();
        assert_eq!(binary.entry, Some(base + 0x1000));
        assert_eq!(binary.imports.iter().find(|import| import.name == "puts").unwrap().slot, Some(base + PE_IAT_RVA));
    }
}

#[test]
fn an_overflowed_slot_is_not_recovered_from_a_same_named_valid_import() {
    let mut data = image(false, 0, 0);
    data[0x710..0x714].copy_from_slice(&0xffff_fffcu32.to_le_bytes());
    data[0x744..0x748].copy_from_slice(&0x2180u32.to_le_bytes());
    let binary = parse_binary(&data).unwrap();
    let slots: Vec<_> = binary.imports.iter().filter(|import| import.name == "puts").map(|import| import.slot).collect();
    assert_eq!(slots, [Some(u64::from(0xffff_fffcu32)), None]);
}
