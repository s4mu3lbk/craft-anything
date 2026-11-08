use craft_anything_formats::pe;
use craft_anything_testkit::packed::{pe_rcdata, pe_section};

fn set_directory_size(data: &mut [u8], size: u32) {
    let headers = pe::headers(data).unwrap();
    let dirs = headers.pe_offset + 24 + if headers.pe32_plus { 112 } else { 96 };
    data[dirs + 20..dirs + 24].copy_from_slice(&size.to_le_bytes());
}

#[test]
fn resource_subdirectories_cannot_escape_the_directory_size() {
    let mut data = pe_rcdata("inside", b"payload");
    set_directory_size(&mut data, 24);
    let resources = pe::parse_resources(&data, 10).unwrap();
    assert!(resources.entries.is_empty());
    assert!(!resources.warnings.is_empty());
}

#[test]
fn a_resource_name_cannot_be_read_from_bytes_after_the_directory() {
    let mut data = pe_rcdata("outside", b"payload");
    set_directory_size(&mut data, 0x60);
    let resources = pe::parse_resources(&data, 10).unwrap();
    assert!(resources.entries.is_empty());
    assert!(!resources.warnings.is_empty());
}

#[test]
fn a_partial_resource_data_entry_is_not_reported_as_complete() {
    for size in [0x50, 0x54, 0x57] {
        let mut data = pe_rcdata("inside", b"payload");
        let headers = pe::headers(&data).unwrap();
        let root = headers.rva_to_offset(headers.dir(2).unwrap().0).unwrap();
        data[root + 0x28..root + 0x2c].copy_from_slice(&7u32.to_le_bytes());
        set_directory_size(&mut data, size);
        let resources = pe::parse_resources(&data, 10).unwrap();
        assert!(resources.entries.is_empty(), "size={size}");
        assert!(!resources.warnings.is_empty());
    }
}

#[test]
fn resource_payloads_still_use_their_own_rva() {
    let mut data = pe_rcdata("inside", b"payload");
    set_directory_size(&mut data, 0x60 + 2 + 6 * 2);
    let resources = pe::parse_resources(&data, 10).unwrap();
    assert_eq!(resources.entries.len(), 1);
    let entry = &resources.entries[0];
    assert_eq!(entry.name, "inside");
    assert_eq!(entry.size, 7);
    let off = entry.file_offset.unwrap() as usize;
    assert_eq!(&data[off..off + 7], b"payload");
    assert!(resources.warnings.is_empty());
}

fn resource_with_long_name() -> Vec<u8> {
    let data = pe_rcdata("inside", b"payload");
    let headers = pe::headers(&data).unwrap();
    let (rva, size) = headers.dir(2).unwrap();
    let root = headers.rva_to_offset(rva).unwrap();
    let mut resource = data[root..root + size as usize].to_vec();
    let name_at = (resource.len() + 1) & !1;
    resource.resize(name_at, 0);
    resource.extend(4097u16.to_le_bytes());
    for _ in 0..4096 {
        resource.extend(u16::from(b'x').to_le_bytes());
    }
    resource.extend(u16::from(b'z').to_le_bytes());
    resource[0x28..0x2c].copy_from_slice(&(0x8000_0000 | name_at as u32).to_le_bytes());
    // Rebuild the section so its raw and virtual extents contain the complete name.
    let mut data = pe_section(".rsrc", &resource);
    let headers = pe::headers(&data).unwrap();
    let dirs = headers.pe_offset + 24 + if headers.pe32_plus { 112 } else { 96 };
    data[dirs + 16..dirs + 20].copy_from_slice(&rva.to_le_bytes());
    set_directory_size(&mut data, resource.len() as u32);
    data
}

#[test]
fn a_long_resource_name_must_fit_its_full_declared_span() {
    let mut data = resource_with_long_name();
    let headers = pe::headers(&data).unwrap();
    let (rva, size) = headers.dir(2).unwrap();
    let last_unit = headers.rva_to_offset(rva + size - 2).unwrap();
    set_directory_size(&mut data, size - 2);
    // The last unit remains mapped and present in the file, outside the declared directory.
    assert_eq!(&data[last_unit..last_unit + 2], b"z\0");
    let resources = pe::parse_resources(&data, 10).unwrap();
    assert!(resources.entries.is_empty());
    assert!(resources.warnings.iter().any(|warning| warning == "resource name runs past the directory; skipped an entry"));
}

#[test]
fn a_long_resource_name_cannot_cross_the_mapped_raw_extent() {
    for delta in [0u32, 32] {
        let mut data = resource_with_long_name();
        let headers = pe::headers(&data).unwrap();
        let (rva, size) = headers.dir(2).unwrap();
        let root = headers.rva_to_offset(rva).unwrap();
        let section = &headers.sections[0];
        let optional_size = u16::from_le_bytes(data[headers.pe_offset + 20..headers.pe_offset + 22].try_into().unwrap());
        let section_at = headers.pe_offset + 24 + usize::from(optional_size);
        // Keep the resource at the same file offset, either at the section start or inside it.
        data[section_at + 8..section_at + 12].copy_from_slice(&(section.virtual_size + delta).to_le_bytes());
        data[section_at + 12..section_at + 16].copy_from_slice(&(section.rva - delta).to_le_bytes());
        data[section_at + 16..section_at + 20].copy_from_slice(&(delta + size - 2).to_le_bytes());
        data[section_at + 20..section_at + 24].copy_from_slice(&(section.raw_offset - delta).to_le_bytes());
        let mapped = pe::headers(&data).unwrap();
        let last_rva = rva + size - 2;
        let last_unit = root + size as usize - 2;
        assert_eq!(mapped.rva_to_offset(rva), Some(root));
        assert_eq!(rva - mapped.sections[0].rva, delta);
        assert!(last_rva - mapped.sections[0].rva < mapped.sections[0].virtual_size);
        assert_eq!(mapped.rva_to_offset(last_rva), None);
        assert_eq!(&data[last_unit..last_unit + 2], b"z\0");
        let resources = pe::parse_resources(&data, 10).unwrap();
        assert!(resources.entries.is_empty(), "delta={delta}");
        assert!(resources.warnings.iter().any(|warning| warning == "resource name runs past the directory; skipped an entry"), "delta={delta}");
    }
}

#[test]
fn resource_metadata_in_the_header_prefix_still_parses() {
    let mut data = pe_rcdata("inside", b"payload");
    let headers = pe::headers(&data).unwrap();
    let root = headers.rva_to_offset(headers.dir(2).unwrap().0).unwrap();
    let header_rva = 0x300u32;
    let metadata_size = 0x60 + 2 + 6 * 2;
    assert!(header_rva as usize + metadata_size <= headers.sections[0].raw_offset as usize);
    assert_eq!(headers.rva_to_offset(header_rva), Some(header_rva as usize));
    // Move only metadata into header padding; its payload keeps the original section RVA.
    data.copy_within(root..root + metadata_size, header_rva as usize);
    let dirs = headers.pe_offset + 24 + if headers.pe32_plus { 112 } else { 96 };
    data[dirs + 16..dirs + 20].copy_from_slice(&header_rva.to_le_bytes());
    set_directory_size(&mut data, metadata_size as u32);
    let resources = pe::parse_resources(&data, 10).unwrap();
    assert_eq!(resources.entries.len(), 1);
    let entry = &resources.entries[0];
    assert_eq!(entry.name, "inside");
    assert_eq!(entry.size, 7);
    let off = entry.file_offset.unwrap() as usize;
    assert_eq!(&data[off..off + 7], b"payload");
    assert!(resources.warnings.is_empty());
}

/// Add one mapping candidate without moving the original section's bytes.
fn add_mapping_section(mut data: Vec<u8>, first: bool, rva: u32, virtual_size: u32, raw_size: u32) -> Vec<u8> {
    let headers = pe::headers(&data).unwrap();
    assert_eq!(headers.sections.len(), 1);
    let optional_size = u16::from_le_bytes(data[headers.pe_offset + 20..headers.pe_offset + 22].try_into().unwrap());
    let section_at = headers.pe_offset + 24 + usize::from(optional_size);
    assert!(section_at + 80 <= headers.sections[0].raw_offset as usize);
    if first {
        data.copy_within(section_at..section_at + 40, section_at + 40);
    }
    let added = section_at + if first { 0 } else { 40 };
    data[added..added + 40].fill(0);
    data[added..added + 5].copy_from_slice(b".next");
    data[added + 8..added + 12].copy_from_slice(&virtual_size.to_le_bytes());
    data[added + 12..added + 16].copy_from_slice(&rva.to_le_bytes());
    data[added + 16..added + 20].copy_from_slice(&raw_size.to_le_bytes());
    data[added + 20..added + 24].copy_from_slice(&headers.sections[0].raw_offset.to_le_bytes());
    data[headers.pe_offset + 6..headers.pe_offset + 8].copy_from_slice(&2u16.to_le_bytes());
    data
}

#[test]
fn resource_metadata_obeys_section_table_priority() {
    let metadata_size = 0x60 + 2 + 6 * 2;
    let last_unit = metadata_size - 2;
    for (first, virtual_size, raw_size, distance, rejected) in [
        (true, 2, 2, last_unit, true),
        (true, 2, 0, last_unit, true),
        (true, 0, 2, last_unit, true),
        (false, 2, 2, last_unit, false),
        (true, 0, 0, last_unit, false),
        (true, 2, 2, metadata_size, false),
    ] {
        let mut data = pe_rcdata("inside", b"payload");
        set_directory_size(&mut data, metadata_size);
        let headers = pe::headers(&data).unwrap();
        let rva = headers.dir(2).unwrap().0;
        let root = headers.rva_to_offset(rva).unwrap();
        let data = add_mapping_section(data, first, rva + distance, virtual_size, raw_size);
        let mapped = pe::headers(&data).unwrap();
        assert_eq!(mapped.rva_to_offset(rva), Some(root));
        assert_eq!(mapped.rva_to_offset(rva + last_unit) != Some(root + last_unit as usize), rejected);
        let resources = pe::parse_resources(&data, 10).unwrap();
        if rejected {
            assert!(resources.entries.is_empty());
            assert!(resources.warnings.iter().any(|warning| warning == "resource name runs past the directory; skipped an entry"));
        } else {
            assert_eq!(resources.entries.len(), 1);
            assert_eq!(resources.entries[0].name, "inside");
            let off = resources.entries[0].file_offset.unwrap() as usize;
            assert_eq!(&data[off..off + 7], b"payload");
            assert!(resources.warnings.is_empty());
        }
    }
}

#[test]
fn header_resource_metadata_stops_at_section_mapping_changes() {
    let metadata_size = 0x60 + 2 + 6 * 2;
    let last_unit = metadata_size - 2;
    for (virtual_size, raw_size, distance, rejected) in
        [(2, 2, last_unit, true), (2, 0, last_unit, true), (0, 0, last_unit, false), (2, 2, metadata_size, false)]
    {
        let mut data = pe_rcdata("inside", b"payload");
        let headers = pe::headers(&data).unwrap();
        let root = headers.rva_to_offset(headers.dir(2).unwrap().0).unwrap();
        let header_rva = 0x300u32;
        data.copy_within(root..root + metadata_size as usize, header_rva as usize);
        let dirs = headers.pe_offset + 24 + if headers.pe32_plus { 112 } else { 96 };
        data[dirs + 16..dirs + 20].copy_from_slice(&header_rva.to_le_bytes());
        set_directory_size(&mut data, metadata_size);
        // This later-listed section still takes over the header's identity mapping.
        let data = add_mapping_section(data, false, header_rva + distance, virtual_size, raw_size);
        let mapped = pe::headers(&data).unwrap();
        assert_eq!(mapped.rva_to_offset(header_rva), Some(header_rva as usize));
        assert_eq!(mapped.rva_to_offset(header_rva + last_unit) != Some((header_rva + last_unit) as usize), rejected);
        let resources = pe::parse_resources(&data, 10).unwrap();
        if rejected {
            assert!(resources.entries.is_empty());
            assert!(resources.warnings.iter().any(|warning| warning == "resource name runs past the directory; skipped an entry"));
        } else {
            assert_eq!(resources.entries.len(), 1);
            assert_eq!(resources.entries[0].name, "inside");
            let off = resources.entries[0].file_offset.unwrap() as usize;
            assert_eq!(&data[off..off + 7], b"payload");
            assert!(resources.warnings.is_empty());
        }
    }
}

#[test]
fn resource_metadata_stays_within_the_rva_address_space() {
    let metadata_size = 0x60 + 2 + 6 * 2;
    for remaining in [metadata_size - 1, metadata_size] {
        let mut data = pe_rcdata("inside", b"payload");
        let headers = pe::headers(&data).unwrap();
        let root = headers.rva_to_offset(headers.dir(2).unwrap().0).unwrap();
        let optional_size = u16::from_le_bytes(data[headers.pe_offset + 20..headers.pe_offset + 22].try_into().unwrap());
        let section_at = headers.pe_offset + 24 + usize::from(optional_size);
        let dirs = headers.pe_offset + 24 + if headers.pe32_plus { 112 } else { 96 };
        let rva = u32::MAX - remaining + 1;
        data[section_at + 12..section_at + 16].copy_from_slice(&rva.to_le_bytes());
        data[dirs + 16..dirs + 20].copy_from_slice(&rva.to_le_bytes());
        set_directory_size(&mut data, metadata_size);
        // Keep the payload independently mapped in the header prefix.
        data[0x300..0x307].copy_from_slice(b"payload");
        data[root + 0x48..root + 0x4c].copy_from_slice(&0x300u32.to_le_bytes());
        assert_eq!(pe::headers(&data).unwrap().rva_to_offset(rva), Some(root));
        assert_eq!(rva.checked_add(metadata_size - 1).is_some(), remaining == metadata_size);
        assert_eq!(&data[root + metadata_size as usize - 2..root + metadata_size as usize], b"e\0");
        let resources = pe::parse_resources(&data, 10).unwrap();
        if remaining < metadata_size {
            assert!(resources.entries.is_empty());
            assert!(resources.warnings.iter().any(|warning| warning == "resource name runs past the directory; skipped an entry"));
        } else {
            assert_eq!(resources.entries.len(), 1);
            assert_eq!(resources.entries[0].name, "inside");
            assert_eq!(resources.entries[0].file_offset, Some(0x300));
            assert!(resources.warnings.is_empty());
        }
    }
}

#[test]
fn a_complete_long_resource_name_keeps_the_existing_output_limit() {
    let data = resource_with_long_name();
    let resources = pe::parse_resources(&data, 10).unwrap();
    assert_eq!(resources.entries.len(), 1);
    let entry = &resources.entries[0];
    assert_eq!(entry.name, "x".repeat(4096));
    assert_eq!(entry.size, 7);
    let off = entry.file_offset.unwrap() as usize;
    assert_eq!(&data[off..off + 7], b"payload");
    assert!(resources.warnings.is_empty());
}
