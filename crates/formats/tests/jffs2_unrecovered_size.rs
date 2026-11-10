use craft_anything_formats::fsread::{self, Limits};

#[test]
fn a_file_skipped_by_its_size_budget_keeps_its_declared_size() {
    let data = craft_anything_testkit::firmware::jffs2_image();
    for limits in [Limits { max_file_bytes: 0, ..Limits::default() }, Limits { max_total_bytes: 0, ..Limits::default() }] {
        let image = fsread::jffs2::read(&data, &limits);
        let file = image.entries.iter().find(|e| e.path == "config/settings").unwrap();
        assert_eq!(file.size, 10);
        assert!(file.data.is_none());
        assert!(!image.complete);
    }
}

#[test]
fn the_latest_inode_size_is_preserved_when_bytes_cannot_be_recovered() {
    let mut data = craft_anything_testkit::firmware::jffs2_image();
    let inode = data.windows(4).position(|w| w == [0x85, 0x19, 0x02, 0xe0]).unwrap();
    let mut newest = data[inode..].to_vec();
    newest[16..20].copy_from_slice(&2u32.to_le_bytes());
    newest[28..32].copy_from_slice(&20u32.to_le_bytes());
    data.extend_from_slice(&newest);
    let image = fsread::jffs2::read(&data, &Limits { max_file_bytes: 8, ..Limits::default() });
    let file = image.entries.iter().find(|e| e.path == "config/settings").unwrap();
    assert_eq!(file.size, 20);
    assert!(file.data.is_none());
}

#[test]
fn recovered_file_sizes_still_match_the_bytes() {
    let image = fsread::jffs2::read(&craft_anything_testkit::firmware::jffs2_image(), &Limits::default());
    let file = image.entries.iter().find(|e| e.path == "config/settings").unwrap();
    assert_eq!(file.size, 10);
    assert_eq!(file.data.as_deref(), Some(b"key=value\n".as_slice()));
}

#[test]
fn a_zero_extent_clears_only_the_part_kept_in_the_final_file() {
    let mut data = craft_anything_testkit::firmware::jffs2_image();
    let inode = data.windows(4).position(|w| w == [0x85, 0x19, 0x02, 0xe0]).unwrap();
    let mut zero = data[inode..inode + 68].to_vec();
    zero[4..8].copy_from_slice(&68u32.to_le_bytes());
    zero[16..20].copy_from_slice(&2u32.to_le_bytes());
    zero[28..32].copy_from_slice(&5u32.to_le_bytes());
    zero[44..48].copy_from_slice(&3u32.to_le_bytes());
    zero[48..52].fill(0);
    zero[52..56].copy_from_slice(&u32::MAX.to_le_bytes());
    zero[56] = 1;
    data.extend_from_slice(&zero);
    let image = fsread::jffs2::read(&data, &Limits { max_file_bytes: 5, ..Limits::default() });
    assert!(image.complete, "{:?}", image.warnings);
    assert_eq!(image.entries.iter().find(|e| e.path == "config/settings").unwrap().data.as_deref(), Some(b"key\0\0".as_slice()));
}

#[test]
fn a_short_node_cannot_borrow_a_name_or_payload_from_following_bytes() {
    let original = craft_anything_testkit::firmware::jffs2_image();
    for max_name in [1, 255] {
        let mut bad_name = original.clone();
        bad_name[4..8].copy_from_slice(&41u32.to_le_bytes());
        let image = fsread::jffs2::read(&bad_name, &Limits { max_name, ..Limits::default() });
        assert!(image.entries.is_empty());
        assert!(!image.complete);
    }

    let inode = original.windows(4).position(|w| w == [0x85, 0x19, 0x02, 0xe0]).unwrap();
    let mut bad_data = original;
    bad_data[inode + 4..inode + 8].copy_from_slice(&69u32.to_le_bytes());
    let image = fsread::jffs2::read(&bad_data, &Limits::default());
    assert!(image.entries.iter().find(|e| e.path == "config/settings").unwrap().data.is_none());
    assert!(!image.complete);
}
