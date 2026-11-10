use craft_anything_formats::tarcpio::{member_data, tar};
use craft_anything_testkit::firmware as fx;

fn clear_member_size(archive: &mut [u8], offset: usize) {
    let header = &mut archive[offset..offset + 512];
    header[124..136].copy_from_slice(b"00000000000\0");
    header[148..156].fill(b' ');
    let sum: u32 = header.iter().map(|&b| u32::from(b)).sum();
    header[148..156].copy_from_slice(format!("{sum:06o}\0 ").as_bytes());
}

#[test]
fn pax_size_survives_a_gnu_long_name_record() {
    let mut archive = fx::tar_entries(&[
        ("PaxHeader", b'x', b"10 size=3\n", ""),
        ("././@LongLink", b'L', b"renamed\0", ""),
        ("short", b'0', b"abc", ""),
        ("next", b'0', b"later", ""),
    ]);
    clear_member_size(&mut archive, 2048);
    let listing = tar(&archive, 10).unwrap();
    assert!(listing.complete, "{:?}", listing.warnings);
    assert_eq!(listing.members.len(), 2);
    assert_eq!(listing.members[0].name, "renamed");
    assert_eq!(listing.members[0].size, 3);
    assert_eq!(member_data(&archive, &listing.members[0]), Some(b"abc".as_slice()));
    assert_eq!(listing.members[1].size, 5);
    assert_eq!(member_data(&archive, &listing.members[1]), Some(b"later".as_slice()));
}

#[test]
fn pax_size_without_intervening_metadata_still_applies_once() {
    let mut archive = fx::tar_entries(&[("PaxHeader", b'x', b"10 size=3\n", ""), ("short", b'0', b"abc", ""), ("next", b'0', b"later", "")]);
    clear_member_size(&mut archive, 1024);
    let listing = tar(&archive, 10).unwrap();
    assert!(listing.complete);
    assert_eq!(listing.members.iter().map(|m| m.size).collect::<Vec<_>>(), vec![3, 5]);
}

fn pax_record(key: &str, value: &str) -> String {
    let content = format!(" {key}={value}\n");
    let mut len = content.len() + 1;
    loop {
        let record = format!("{len}{content}");
        if record.len() == len {
            return record;
        }
        len = record.len();
    }
}

#[test]
fn malformed_pax_framing_stops_the_listing() {
    for invalid in [b"10 size=3!".as_slice(), b"10 size=3!\n", b"20 size=3\n", b"+12 path=no\n", b"5 =x\n", b"12 path=a=b\n11 path=bad"] {
        let archive = fx::tar_entries(&[("keep", b'0', b"one", ""), ("PaxHeader", b'x', invalid, ""), ("after", b'0', b"two", "")]);
        let listing = tar(&archive, 10).unwrap();
        assert!(!listing.complete);
        assert_eq!(listing.members.len(), 1);
        assert_eq!(listing.members[0].name, "keep");
        assert!(listing.warnings.iter().any(|s| s.contains("pax")));
    }
}

#[test]
fn pax_path_can_contain_an_equals_sign() {
    let archive = fx::tar_entries(&[("PaxHeader", b'x', b"12 path=a=b\n", ""), ("short", b'0', b"ok", "")]);
    let listing = tar(&archive, 10).unwrap();
    assert!(listing.complete);
    assert_eq!(listing.members[0].name, "a=b");
}

#[test]
fn the_name_limit_applies_only_to_pax_fields_kept_as_names() {
    let exact = "a".repeat(4096);
    let large = "b".repeat(100_000);
    let pax = pax_record("comment", &large) + &pax_record("path", &exact);
    let archive = fx::tar_entries(&[("PaxHeader", b'x', pax.as_bytes(), ""), ("short", b'0', b"ok", "")]);
    let listing = tar(&archive, 10).unwrap();
    assert!(listing.complete);
    assert_eq!(listing.members[0].name, exact);
    for key in ["path", "linkpath"] {
        let pax = pax_record(key, &large);
        let archive = fx::tar_entries(&[("PaxHeader", b'x', pax.as_bytes(), ""), ("short", b'2', b"", "link")]);
        let listing = tar(&archive, 10).unwrap();
        assert!(!listing.complete);
        assert!(listing.members.is_empty());
        assert!(listing.warnings.iter().any(|s| s.contains("4096")));
    }
}

#[test]
fn empty_pax_values_remove_an_earlier_override() {
    let pax = pax_record("path", "other") + &pax_record("path", "") + &pax_record("linkpath", "first") + &pax_record("linkpath", "");
    let archive = fx::tar_entries(&[("PaxHeader", b'x', pax.as_bytes(), ""), ("original", b'2', b"", "target")]);
    let listing = tar(&archive, 10).unwrap();
    assert!(listing.complete);
    assert!(listing.members[0].name.is_empty());
    assert!(listing.members[0].link_target.is_none());
}

#[test]
fn tar_sizes_allow_outer_padding_but_do_not_join_separated_digits() {
    for (field, valid) in [(b" 000000001\0 ", true), (b" 0000 0001\0 ", false), (b" 0000\x000001\0 ", false)] {
        let mut archive = fx::tar_entries(&[("one", b'0', b"1", ""), ("two", b'0', b"2", "")]);
        let header = &mut archive[..512];
        header[124..136].copy_from_slice(field);
        header[148..156].fill(b' ');
        let sum: u32 = header.iter().map(|&b| u32::from(b)).sum();
        header[148..156].copy_from_slice(format!("{sum:06o}\0 ").as_bytes());
        let listing = tar(&archive, 10).unwrap();
        assert_eq!(listing.complete, valid);
        assert_eq!(listing.members.len(), if valid { 2 } else { 0 });
    }
}

#[test]
fn excess_pax_records_do_not_hide_a_later_override() {
    let mut records = pax_record("comment", "").repeat(256);
    for excess in [false, true] {
        if excess {
            records.push_str(&pax_record("size", "9999"));
        }
        let archive = fx::tar_entries(&[("PaxHeader", b'x', records.as_bytes(), ""), ("member", b'0', b"ok", "")]);
        let listing = tar(&archive, 10).unwrap();
        assert_eq!(listing.complete, !excess);
        assert_eq!(listing.members.len(), usize::from(!excess));
    }
}
