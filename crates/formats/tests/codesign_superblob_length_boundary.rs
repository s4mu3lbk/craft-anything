use craft_anything_formats::codesign;
use craft_anything_testkit::apple;

#[test]
fn a_blob_outside_the_declared_signature_cannot_supply_cms_data() {
    let mut data = Vec::new();
    for value in [0xfade_0cc0u32, 20, 1, 0x10000, 20, 0xfade_0b01, 9] {
        data.extend_from_slice(&value.to_be_bytes());
    }
    data.push(1);

    let signature = codesign::parse(&data).unwrap();
    assert_eq!(signature.cms_signature_bytes, None);
    assert!(signature.ad_hoc);
    assert!(!signature.warnings.is_empty());

    let length = data.len() as u32;
    data[4..8].copy_from_slice(&length.to_be_bytes());
    let complete = codesign::parse(&data).unwrap();
    assert_eq!(complete.cms_signature_bytes, Some(1));
    assert!(!complete.ad_hoc);
}

#[test]
fn the_blob_index_must_fit_within_the_declared_signature() {
    let mut data = apple::code_signature_blob();
    data[4..8].copy_from_slice(&12u32.to_be_bytes());
    let signature = codesign::parse(&data).unwrap();
    assert!(signature.blobs.is_empty());
    assert!(signature.code_directories.is_empty());
    assert!(!signature.warnings.is_empty());
}

#[test]
fn a_signature_length_smaller_than_its_header_is_rejected() {
    let mut data = apple::code_signature_blob();
    data[4..8].copy_from_slice(&8u32.to_be_bytes());
    assert!(codesign::parse(&data).is_err());
}
