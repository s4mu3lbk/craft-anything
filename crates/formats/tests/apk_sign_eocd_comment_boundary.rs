use craft_anything_formats::apk_sign;
use craft_anything_testkit::android::notes_apk;

#[test]
fn an_eocd_marker_inside_the_comment_does_not_hide_the_signature() {
    for fake_comment_length in [0u16, 1] {
        let mut apk = notes_apk();
        let original = apk_sign::parse(&apk).unwrap();
        let eocd = apk.len() - 22;
        assert_eq!(&apk[eocd..eocd + 4], b"PK\x05\x06");

        let mut comment = vec![0; 22];
        comment[..4].copy_from_slice(b"PK\x05\x06");
        comment[20..22].copy_from_slice(&fake_comment_length.to_le_bytes());
        apk[eocd + 20..eocd + 22].copy_from_slice(&(comment.len() as u16).to_le_bytes());
        apk.extend_from_slice(&comment);

        let found = apk_sign::parse(&apk).expect("the original EOCD still frames the APK");
        assert_eq!(found.offset, original.offset);
        assert_eq!(found.entries.len(), original.entries.len());
        assert_eq!(found.signers.len(), original.signers.len());
    }
}

#[test]
fn an_incomplete_eocd_comment_is_not_accepted() {
    let mut apk = notes_apk();
    let len = apk.len();
    apk[len - 2..].copy_from_slice(&1u16.to_le_bytes());
    assert!(apk_sign::parse(&apk).is_none());
}

#[test]
fn an_eocd_marker_in_a_signing_block_value_does_not_hide_the_signature() {
    for points_to_real_directory in [false, true] {
        let mut apk = notes_apk();
        let original = apk_sign::parse(&apk).unwrap();
        let eocd = apk.len() - 22;
        let cd = u32::from_le_bytes(apk[eocd + 16..eocd + 20].try_into().unwrap()) as usize;
        let pair_at = cd - 24;
        let marker_at = pair_at + 12;
        let added = 8 + 4 + 22;
        let new_cd = cd + added;

        let mut value = vec![0; 22];
        value[..4].copy_from_slice(b"PK\x05\x06");
        let fake_cd = if points_to_real_directory { new_cd as u32 } else { 0 };
        value[16..20].copy_from_slice(&fake_cd.to_le_bytes());
        let comment_len = u16::try_from(apk.len() + added - marker_at - 22).unwrap();
        value[20..22].copy_from_slice(&comment_len.to_le_bytes());
        let mut pair = 26u64.to_le_bytes().to_vec();
        pair.extend_from_slice(&0x1357_2468u32.to_le_bytes());
        pair.extend_from_slice(&value);
        apk.splice(pair_at..pair_at, pair);

        let size = original.size + added as u64;
        let start = original.offset as usize;
        apk[start..start + 8].copy_from_slice(&size.to_le_bytes());
        apk[new_cd - 24..new_cd - 16].copy_from_slice(&size.to_le_bytes());
        apk[eocd + added + 16..eocd + added + 20].copy_from_slice(&(new_cd as u32).to_le_bytes());

        let found = apk_sign::parse(&apk).expect("a marker inside an unknown signing-block value is not the ZIP record");
        assert_eq!((found.offset, found.size), (original.offset, size));
        assert_eq!(&found.entries[..original.entries.len()], &original.entries);
        assert_eq!(found.entries.len(), original.entries.len() + 1);
        assert_eq!(found.entries.last().unwrap().id, "0x13572468");
        assert_eq!(found.signers, original.signers);
        assert!(found.warnings.is_empty());
    }
}

#[test]
fn a_signing_block_and_eocd_inside_the_comment_do_not_replace_the_original() {
    let mut apk = notes_apk();
    let original = apk_sign::parse(&apk).unwrap();
    let eocd = apk.len() - 22;
    let mut comment = 24u64.to_le_bytes().to_vec();
    comment.extend_from_slice(&24u64.to_le_bytes());
    comment.extend_from_slice(b"APK Sig Block 42");
    let fake_cd = apk.len() + comment.len();
    let mut fake_eocd = vec![0; 22];
    fake_eocd[..4].copy_from_slice(b"PK\x05\x06");
    fake_eocd[16..20].copy_from_slice(&(fake_cd as u32).to_le_bytes());
    comment.extend_from_slice(&fake_eocd);
    apk[eocd + 20..eocd + 22].copy_from_slice(&(comment.len() as u16).to_le_bytes());
    apk.extend_from_slice(&comment);
    assert_eq!(apk_sign::parse(&apk).unwrap(), original);
}

#[test]
fn recoverable_size_and_pair_damage_still_returns_signing_evidence() {
    let original_bytes = notes_apk();
    let original = apk_sign::parse(&original_bytes).unwrap();
    let start = original.offset as usize;
    let mut size_mismatch = original_bytes.clone();
    size_mismatch[start..start + 8].copy_from_slice(&(original.size + 1).to_le_bytes());
    let found = apk_sign::parse(&size_mismatch).expect("header size disagreement is reported as a warning");
    assert_eq!(found.signers, original.signers);
    assert_eq!(found.entries, original.entries);
    assert!(found.warnings.iter().any(|w| w.contains("size fields disagree")));

    let mut bad_pair = original_bytes;
    let third_pair = start + 8 + original.entries.iter().take(2).map(|entry| entry.size as usize + 12).sum::<usize>();
    bad_pair[third_pair..third_pair + 8].copy_from_slice(&original.size.to_le_bytes());
    let found = apk_sign::parse(&bad_pair).expect("a malformed later pair keeps the preceding signers");
    assert_eq!(found.signers, original.signers);
    assert_eq!(found.entries.len(), 2);
    assert!(found.warnings.iter().any(|w| w.contains("malformed id-value pair")));
}

fn block_with_pairs(pairs: &[(u32, Vec<u8>)]) -> Vec<u8> {
    let mut body = Vec::new();
    for (id, value) in pairs {
        body.extend_from_slice(&((value.len() + 4) as u64).to_le_bytes());
        body.extend_from_slice(&id.to_le_bytes());
        body.extend_from_slice(value);
    }
    let size = (body.len() + 24) as u64;
    let mut block = size.to_le_bytes().to_vec();
    block.extend_from_slice(&body);
    block.extend_from_slice(&size.to_le_bytes());
    block.extend_from_slice(b"APK Sig Block 42");
    block
}

fn empty_eocd(offset: usize, comment_len: usize) -> Vec<u8> {
    let mut end = vec![0; 22];
    end[..4].copy_from_slice(b"PK\x05\x06");
    end[16..20].copy_from_slice(&u32::try_from(offset).unwrap().to_le_bytes());
    end[20..22].copy_from_slice(&u16::try_from(comment_len).unwrap().to_le_bytes());
    end
}

fn signed_zip(entries: &[(&str, &[u8])]) -> Vec<u8> {
    use craft_anything_testkit::{android, electron::zip_stored};
    android::with_signing_block(&zip_stored(entries), &android::signing_block(&android::certificate_der("Craft Test", "Craft Org")))
}

fn with_nested_candidate(mut apk: Vec<u8>, fake_block: &[u8]) -> (Vec<u8>, apk_sign::SigningBlock) {
    let mut expected = apk_sign::parse(&apk).unwrap();
    let eocd = apk.len() - 22;
    let cd = u32::from_le_bytes(apk[eocd + 16..eocd + 20].try_into().unwrap()) as usize;
    let pair_at = cd - 24;
    let marker_at = pair_at + 12 + fake_block.len();
    let added = 12 + fake_block.len() + 22;
    let mut value = fake_block.to_vec();
    value.extend_from_slice(&empty_eocd(marker_at, apk.len() + added - marker_at - 22));
    let mut pair = ((value.len() + 4) as u64).to_le_bytes().to_vec();
    pair.extend_from_slice(&0x1357_2468u32.to_le_bytes());
    pair.extend_from_slice(&value);
    apk.splice(pair_at..pair_at, pair);

    expected.size += added as u64;
    expected.entries.push(apk_sign::BlockEntry { id: "0x13572468".into(), name: "unknown", size: value.len() as u64 });
    let start = expected.offset as usize;
    let new_cd = cd + added;
    apk[start..start + 8].copy_from_slice(&expected.size.to_le_bytes());
    apk[new_cd - 24..new_cd - 16].copy_from_slice(&expected.size.to_le_bytes());
    apk[eocd + added + 16..eocd + added + 20].copy_from_slice(&(new_cd as u32).to_le_bytes());
    (apk, expected)
}

fn assert_nested_candidate_keeps_signers(fake_block: &[u8]) {
    for apk in [notes_apk(), signed_zip(&[])] {
        let (apk, expected) = with_nested_candidate(apk, fake_block);
        assert_eq!(apk_sign::parse(&apk).unwrap(), expected);
    }
}

#[test]
fn an_empty_block_inside_a_pair_cannot_hide_readable_signers() {
    assert_nested_candidate_keeps_signers(&block_with_pairs(&[]));
}

#[test]
fn an_unknown_pair_block_inside_a_pair_cannot_hide_readable_signers() {
    assert_nested_candidate_keeps_signers(&block_with_pairs(&[(0x2468_1357, vec![1, 2, 3, 4])]));
}

#[test]
fn an_unreadable_signature_pair_cannot_hide_readable_signers() {
    assert_nested_candidate_keeps_signers(&block_with_pairs(&[(apk_sign::V2_ID, vec![0; 4])]));
}

#[test]
fn a_framed_unknown_pair_spanning_the_original_directory_does_not_hide_signers() {
    let mut apk = signed_zip(&[("pad", &[0; 64])]);
    let original = apk_sign::parse(&apk).unwrap();
    let len = apk.len();
    let start = 30 + "pad".len();
    let size = (len + 16 - start) as u64;
    apk[start..start + 8].copy_from_slice(&size.to_le_bytes());
    apk[start + 8..start + 16].copy_from_slice(&((len - start - 16) as u64).to_le_bytes());
    apk[start + 16..start + 20].copy_from_slice(&0x1357_2468u32.to_le_bytes());
    apk[len - 2..].copy_from_slice(&46u16.to_le_bytes());
    apk.extend_from_slice(&size.to_le_bytes());
    apk.extend_from_slice(b"APK Sig Block 42");
    apk.extend_from_slice(&empty_eocd(len + 24, 0));

    assert_eq!(apk_sign::parse(&apk).unwrap(), original);
}

#[test]
fn different_readable_signing_blocks_report_ambiguous_metadata() {
    use craft_anything_testkit::android;
    let mut apk = notes_apk();
    let original = apk_sign::parse(&apk).unwrap();
    let eocd = apk.len() - 22;
    let other = android::signing_block(&android::certificate_der("Other Signer", "Craft Org"));
    let mut comment = other;
    comment.extend_from_slice(&empty_eocd(apk.len() + comment.len(), 0));
    apk[eocd + 20..eocd + 22].copy_from_slice(&(comment.len() as u16).to_le_bytes());
    apk.extend_from_slice(&comment);

    let found = apk_sign::parse(&apk).unwrap();
    assert_eq!((found.offset, found.size), (original.offset, original.size));
    assert_eq!(found.entries, original.entries);
    assert_eq!(found.signers, original.signers);
    assert!(found.warnings.iter().any(|warning| warning.contains("signing metadata is ambiguous")));
}

#[test]
fn a_single_empty_signing_block_remains_readable() {
    let apk = craft_anything_testkit::android::with_signing_block(&craft_anything_testkit::electron::zip_stored(&[]), &block_with_pairs(&[]));
    let found = apk_sign::parse(&apk).unwrap();
    assert_eq!((found.offset, found.size), (0, 24));
    assert!(found.entries.is_empty());
    assert!(found.signers.is_empty());
    assert!(found.warnings.is_empty());
}

#[test]
fn comment_candidates_cannot_spend_the_genuine_directory_budget_repeatedly() {
    let names: Vec<_> = (0..32).map(|index| format!("entry-{index}")).collect();
    let entries: Vec<_> = names.iter().map(|name| (name.as_str(), b"x".as_slice())).collect();
    let mut apk = signed_zip(&entries);
    let original = apk_sign::parse(&apk).unwrap();
    assert_eq!(original.signers.len(), 2);
    let eocd = apk.len() - 22;
    let cd = u32::from_le_bytes(apk[eocd + 16..eocd + 20].try_into().unwrap()) as usize;
    let count = u16::from_le_bytes(apk[eocd + 10..eocd + 12].try_into().unwrap());
    let candidates = usize::from(u16::MAX) / 22;
    let comment_len = candidates * 22;
    let mut fake = apk[eocd..].to_vec();
    fake[8..10].copy_from_slice(&(count + 1).to_le_bytes());
    fake[10..12].copy_from_slice(&(count + 1).to_le_bytes());
    apk[eocd + 20..eocd + 22].copy_from_slice(&u16::try_from(comment_len).unwrap().to_le_bytes());
    for index in 0..candidates {
        fake[12..16].copy_from_slice(&u32::try_from(apk.len() - cd).unwrap().to_le_bytes());
        fake[20..22].copy_from_slice(&u16::try_from(comment_len - (index + 1) * 22).unwrap().to_le_bytes());
        apk.extend_from_slice(&fake);
    }
    // Every fake reaches the same real headers, then fails on the genuine EOCD. Rewalking
    // that prefix for each comment candidate would spend the entire scan-wide allowance.
    assert!(candidates * (usize::from(count) + 1) > apk.len() / 46 + 22 + usize::from(u16::MAX));
    assert_eq!(apk_sign::parse(&apk).expect("comment candidates must reuse directory boundaries"), original);
}

#[test]
fn framed_comment_candidates_reuse_signer_probes_before_the_genuine_eocd() {
    const CANDIDATES: usize = 300;
    let mut apk = notes_apk();
    let original = apk_sign::parse(&apk).unwrap();
    assert_eq!(original.signers.len(), 2);
    let shared_start = apk.len();

    // Put a readable signer last: its sequence contains one signer with an empty signed-data field.
    let mut pairs = vec![(0x1357_2468, Vec::new()); 255];
    pairs.push((apk_sign::V2_ID, vec![8, 0, 0, 0, 4, 0, 0, 0, 0, 0, 0, 0]));
    let block = block_with_pairs(&pairs);
    apk.extend_from_slice(&block[..block.len() - 24]);
    for index in 0..CANDIDATES {
        // Each footer points back to the same 256 physical pairs. Its empty directory meets
        // its EOCD exactly; the remaining footer/EOCD pairs constitute its ZIP comment.
        let size = (apk.len() - shared_start + 16) as u64;
        apk.extend_from_slice(&size.to_le_bytes());
        apk.extend_from_slice(b"APK Sig Block 42");
        let directory = apk.len();
        apk.extend_from_slice(&empty_eocd(directory, (CANDIDATES - index - 1) * (24 + 22)));
    }
    let comment_len = u16::try_from(apk.len() - shared_start).unwrap();
    apk[shared_start - 2..shared_start].copy_from_slice(&comment_len.to_le_bytes());
    assert!(CANDIDATES * pairs.len() > apk.len() / 46 + 22 + usize::from(u16::MAX));

    let found = apk_sign::parse(&apk).expect("repeated probes must leave enough work to read the genuine block");
    assert_eq!((found.offset, found.size), (original.offset, original.size));
    assert_eq!(found.signers, original.signers);
    // The warning proves a comment candidate reached a readable signer, rather than failing directory framing.
    assert!(found.warnings.iter().any(|warning| warning.contains("signing metadata is ambiguous")));
}
