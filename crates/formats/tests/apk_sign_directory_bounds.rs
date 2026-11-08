use craft_anything_formats::apk_sign;
use craft_anything_testkit::{
    android::{certificate_der, notes_apk, signed_apk, signing_block, with_signing_block},
    electron::zip_stored,
};

fn apk(files: &[(&str, &[u8])]) -> Vec<u8> {
    with_signing_block(&zip_stored(files), &signing_block(&certificate_der("Craft Test", "Craft Org")))
}

fn directory(bytes: &[u8]) -> (usize, usize, usize) {
    let eocd = bytes.len() - 22;
    let size = u32::from_le_bytes(bytes[eocd + 12..eocd + 16].try_into().unwrap()) as usize;
    let start = u32::from_le_bytes(bytes[eocd + 16..eocd + 20].try_into().unwrap()) as usize;
    (start, size, eocd)
}

fn append_directory_tail(bytes: &[u8], tail: &[u8]) -> Vec<u8> {
    let (_, size, eocd) = directory(bytes);
    let mut out = bytes.to_vec();
    out.splice(eocd..eocd, tail.iter().copied());
    let new_eocd = eocd + tail.len();
    out[new_eocd + 12..new_eocd + 16].copy_from_slice(&((size + tail.len()) as u32).to_le_bytes());
    out
}

#[test]
fn ordinary_apks_keep_their_signers() {
    for bytes in [notes_apk(), signed_apk(false)] {
        let found = apk_sign::parse(&bytes).unwrap();
        assert_eq!(found.signers.len(), 2);
        assert!(found.warnings.is_empty(), "{:?}", found.warnings);
    }
}

#[test]
fn an_empty_directory_keeps_the_signing_block() {
    let bytes = apk(&[]);
    let (start, size, eocd) = directory(&bytes);
    assert_eq!((start, size), (eocd, 0));
    let found = apk_sign::parse(&bytes).unwrap();
    assert_eq!(found.entries.len(), 3);
    assert_eq!(found.signers.len(), 2);
    assert!(found.warnings.is_empty());
}

#[test]
fn the_declared_directory_extent_must_end_at_eocd() {
    let bytes = apk(&[("entry.txt", b"payload")]);
    let (_, size, eocd) = directory(&bytes);
    for declared_size in [size as u32 - 1, size as u32 + 1, u32::MAX] {
        let mut changed = bytes.clone();
        changed[eocd + 12..eocd + 16].copy_from_slice(&declared_size.to_le_bytes());
        assert!(apk_sign::parse(&changed).is_none(), "directory size {declared_size}");
    }
}

#[test]
fn the_declared_record_count_must_match_the_directory() {
    let bytes = apk(&[("a.txt", b"a"), ("b.txt", b"b")]);
    let (_, _, eocd) = directory(&bytes);
    for count in [0u16, 1, 3, u16::MAX] {
        let mut changed = bytes.clone();
        changed[eocd + 8..eocd + 10].copy_from_slice(&count.to_le_bytes());
        changed[eocd + 10..eocd + 12].copy_from_slice(&count.to_le_bytes());
        assert!(apk_sign::parse(&changed).is_none(), "record count {count}");
    }
}

#[test]
fn variable_fields_cannot_extend_beyond_the_directory() {
    let bytes = apk(&[("entry.txt", b"payload")]);
    let (start, _, _) = directory(&bytes);
    for field in [28, 30, 32] {
        let mut changed = bytes.clone();
        changed[start + field..start + field + 2].copy_from_slice(&u16::MAX.to_le_bytes());
        assert!(apk_sign::parse(&changed).is_none(), "length field {field}");
    }
}

#[test]
fn a_truncated_directory_record_is_not_accepted() {
    let bytes = apk(&[("entry.txt", b"payload")]);
    let (start, size, eocd) = directory(&bytes);
    for kept in 0..size {
        let mut changed = bytes.clone();
        changed.drain(start + kept..eocd);
        let new_eocd = start + kept;
        changed[new_eocd + 12..new_eocd + 16].copy_from_slice(&(kept as u32).to_le_bytes());
        assert!(apk_sign::parse(&changed).is_none(), "directory truncated to {kept} bytes");
    }
}

#[test]
fn ordinary_extra_fields_and_file_comments_keep_the_signers() {
    let bytes = apk(&[("entry.txt", b"payload")]);
    let expected = apk_sign::parse(&bytes).unwrap();
    let (start, _, _) = directory(&bytes);
    let extra = [0xfe, 0xca, 2, 0, 7, 9];
    let comment = b"file comment";
    let mut fields = extra.to_vec();
    fields.extend_from_slice(comment);
    let mut changed = append_directory_tail(&bytes, &fields);
    changed[start + 30..start + 32].copy_from_slice(&(extra.len() as u16).to_le_bytes());
    changed[start + 32..start + 34].copy_from_slice(&(comment.len() as u16).to_le_bytes());
    assert_eq!(apk_sign::parse(&changed), Some(expected));
}

#[test]
fn one_complete_directory_digital_signature_is_allowed() {
    let bytes = apk(&[("entry.txt", b"payload")]);
    let expected = apk_sign::parse(&bytes).unwrap();
    let tail = b"PK\x05\x05\x03\0sig";
    assert_eq!(apk_sign::parse(&append_directory_tail(&bytes, tail)), Some(expected));
}

#[test]
fn directory_digital_signatures_must_be_complete_and_unique() {
    let bytes = apk(&[("entry.txt", b"payload")]);
    let tail = b"PK\x05\x05\x03\0sig";
    for kept in 1..tail.len() {
        assert!(apk_sign::parse(&append_directory_tail(&bytes, &tail[..kept])).is_none(), "signature truncated to {kept} bytes");
    }
    let mut repeated = tail.to_vec();
    repeated.extend_from_slice(tail);
    assert!(apk_sign::parse(&append_directory_tail(&bytes, &repeated)).is_none());
    assert!(apk_sign::parse(&append_directory_tail(&bytes, b"padding")).is_none());
}
