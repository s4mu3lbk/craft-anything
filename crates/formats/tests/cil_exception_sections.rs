use craft_anything_formats::dotnet::cil;

fn method(section: &[u8]) -> Vec<u8> {
    let mut data = vec![0x0b, 0x30, 8, 0, 4, 0, 0, 0, 0, 0, 0, 0];
    data.extend_from_slice(&[0, 0, 0, 0x2a]);
    data.extend_from_slice(section);
    data
}

fn small_section(kind: u8) -> Vec<u8> {
    // A finally clause whose try and handler both fit the four-byte method.
    vec![kind, 16, 0, 0, 2, 0, 0, 0, 1, 1, 0, 1, 0, 0, 0, 0]
}

#[test]
fn valid_small_and_fat_exception_sections_keep_their_handlers() {
    let small = cil::parse_body(&method(&small_section(1)), 0).unwrap();
    assert_eq!(small.exception_clauses.len(), 1);
    assert_eq!(small.exception_clauses[0].kind, "finally");
    assert!(small.warnings.is_empty());

    let mut fat = vec![0x41, 28, 0, 0];
    for field in [2u32, 0, 1, 1, 1, 0] {
        fat.extend_from_slice(&field.to_le_bytes());
    }
    let body = cil::parse_body(&method(&fat), 0).unwrap();
    assert_eq!(body.exception_clauses.len(), 1);
    assert_eq!(body.exception_clauses[0].handler_offset, 1);
    assert!(body.warnings.is_empty());
}

#[test]
fn unknown_odd_section_kinds_are_not_exception_handlers() {
    for kind in [3, 5, 0x3f] {
        let body = cil::parse_body(&method(&small_section(kind)), 0).unwrap();
        assert!(body.exception_clauses.is_empty(), "kind {kind}");
        assert!(body.warnings.iter().any(|w| w.contains("section") && w.contains("kind")));
    }
}

#[test]
fn a_section_must_fit_before_any_of_its_clauses_are_used() {
    let mut section = small_section(1);
    section[1] = 28;
    let body = cil::parse_body(&method(&section), 0).unwrap();
    assert!(body.exception_clauses.is_empty());
    assert!(body.warnings.iter().any(|w| w.contains("section") && w.contains("file")));
}

#[test]
fn a_partial_clause_does_not_turn_a_malformed_section_into_valid_handlers() {
    let mut section = small_section(1);
    section[1] = 17;
    section.push(0);
    let body = cil::parse_body(&method(&section), 0).unwrap();
    assert!(body.exception_clauses.is_empty());
    assert!(body.warnings.iter().any(|w| w.contains("partial") && w.contains("clause")));
}

#[test]
fn unknown_sections_can_be_skipped_without_losing_the_next_exception_table() {
    let mut section = small_section(0x83);
    section.extend_from_slice(&small_section(1));
    let body = cil::parse_body(&method(&section), 0).unwrap();
    assert_eq!(body.exception_clauses.len(), 1);
    assert!(body.warnings.iter().any(|w| w.contains("section") && w.contains("kind")));
}

#[test]
fn incomplete_section_headers_keep_the_method_and_report_the_missing_bytes() {
    for section in [&[1u8][..], &[1, 4], &[0x41, 4, 0]] {
        let body = cil::parse_body(&method(section), 0).unwrap();
        assert_eq!(body.code_size, 4);
        assert!(body.exception_clauses.is_empty());
        assert!(body.warnings.iter().any(|w| w.contains("section") && w.contains("header")));
    }
}
