use craft_anything_formats::binary::{Arch, Binary};
use craft_anything_formats::{macho, macho_deps};
use craft_anything_testkit::apple;

fn enriched(data: &[u8]) -> Binary {
    let mut binary = Binary::raw(Arch::X86_64, 0, 0, 0);
    macho::enrich(data, &mut binary);
    binary
}

#[test]
fn a_short_command_cannot_borrow_fields_from_the_next_command() {
    let mut short = apple::lc_build_version();
    short.truncate(8);
    short[4..8].copy_from_slice(&8u32.to_le_bytes());
    let data = apple::macho_min(apple::CPU_X86_64, 2, &[short, apple::lc_uuid()], &[]);
    let dependencies = macho_deps::parse_thin(&data).unwrap();
    assert!(dependencies.build_versions.is_empty());
    assert!(dependencies.uuid.is_some(), "a later complete command remains readable");
    assert!(!dependencies.warnings.is_empty());
    let binary = enriched(&data);
    assert!(!binary.info.contains_key("platform"));
    assert!(!binary.warnings.is_empty());
}

#[test]
fn commands_must_fit_in_sizeofcmds_even_when_the_file_has_more_bytes() {
    for size in [0u32, 4, 24] {
        let mut data = apple::macho_min(apple::CPU_X86_64, 2, &[apple::lc_build_version()], &[]);
        data[20..24].copy_from_slice(&size.to_le_bytes());
        assert!(macho_deps::parse_thin(&data).unwrap().build_versions.is_empty(), "size={size}");
        assert!(!enriched(&data).info.contains_key("platform"), "size={size}");
    }
}

#[test]
fn build_tools_cannot_be_read_from_the_next_command() {
    let mut build = apple::lc_build_version();
    build.truncate(24);
    build[4..8].copy_from_slice(&24u32.to_le_bytes());
    let data = apple::macho_min(apple::CPU_X86_64, 2, &[build, apple::lc_uuid()], &[]);
    let dependencies = macho_deps::parse_thin(&data).unwrap();
    assert_eq!(dependencies.build_versions.len(), 1);
    assert_eq!(dependencies.build_versions[0].platform, "macos");
    assert!(dependencies.build_versions[0].tools.is_empty());
    assert!(dependencies.uuid.is_some());
}

/// One complete section header followed by a build command, with two sections declared.
fn segment_with_an_incomplete_section_array() -> Vec<u8> {
    let mut data = apple::macho_min(apple::CPU_X86_64, 2, &[apple::lc_build_version()], &[("__text", &[0; 4])]);
    data[32 + 64..32 + 68].copy_from_slice(&2u32.to_le_bytes());
    data
}

#[test]
fn dependency_sections_cannot_be_read_from_the_next_command() {
    let parsed = macho_deps::parse_thin(&segment_with_an_incomplete_section_array()).unwrap();
    assert_eq!(parsed.sections.len(), 1);
    assert_eq!(parsed.sections[0].sectname, "__text");
    assert!(parsed.warnings.iter().any(|warning| warning.contains("section headers are cut short")));
    assert_eq!(parsed.build_versions.len(), 1, "a later complete command remains readable");
    assert_eq!(parsed.build_versions[0].platform, "macos");
    assert_eq!(parsed.build_versions[0].tools, ["ld 907.0.0"]);
}

#[test]
fn enriched_sections_cannot_be_read_from_the_next_command() {
    let binary = enriched(&segment_with_an_incomplete_section_array());
    let sections = binary.details["sections"].as_array().unwrap();
    assert_eq!(sections.len(), 1);
    assert_eq!(sections[0]["name"], "__text");
    assert!(binary.warnings.iter().any(|warning| warning.contains("section headers run past the load command")));
    assert_eq!(binary.info.get("platform").map(String::as_str), Some("macos"), "a later complete command remains readable");
}

#[test]
fn complete_build_commands_are_read_in_both_byte_orders() {
    let data = apple::macho_min(apple::CPU_X86_64, 2, &[apple::lc_build_version()], &[]);
    assert_eq!(enriched(&data).info.get("platform").map(String::as_str), Some("macos"));
    for big in [false, true] {
        let mut bytes = data.clone();
        if big {
            for word in bytes.as_chunks_mut::<4>().0 {
                word.reverse();
            }
        }
        let parsed = macho_deps::parse_thin(&bytes).unwrap();
        assert_eq!(parsed.build_versions[0].platform, "macos");
        assert_eq!(parsed.build_versions[0].tools, ["ld 907.0.0"]);
        assert!(parsed.warnings.is_empty());
    }
}
