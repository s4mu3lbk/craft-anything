use craft_anything_formats::fsread::{self, Limits};

fn root_image(root_name: &[u8]) -> Vec<u8> {
    let mut data = vec![0u8; 112];
    data[..8].copy_from_slice(b"-rom1fs-");
    data[8..12].copy_from_slice(&112u32.to_be_bytes());
    data[16..23].copy_from_slice(b"volume\0");
    data[32..36].copy_from_slice(&1u32.to_be_bytes());
    data[36..40].copy_from_slice(&64u32.to_be_bytes());
    data[48..48 + root_name.len()].copy_from_slice(root_name);
    data[64..68].copy_from_slice(&2u32.to_be_bytes());
    data[72..76].copy_from_slice(&5u32.to_be_bytes());
    data[80..86].copy_from_slice(b"hello\0");
    data[96..101].copy_from_slice(b"world");
    data
}

#[test]
fn dot_named_root_is_a_container_for_its_children() {
    let image = fsread::romfs::read(&root_image(b".\0"), &Limits::default());
    assert!(image.complete, "{:?}", image.warnings);
    assert_eq!(image.entries.len(), 1);
    assert_eq!(image.entries[0].path, "hello");
    assert_eq!(image.entries[0].data.as_deref(), Some(b"world".as_slice()));
}

#[test]
fn an_empty_root_name_is_not_a_reason_to_drop_its_children() {
    let image = fsread::romfs::read(&root_image(b"\0"), &Limits::default());
    assert_eq!(image.entries.len(), 1);
    assert_eq!(image.entries[0].path, "hello");
}

#[test]
fn root_children_still_obey_the_entry_limit() {
    let image = fsread::romfs::read(&root_image(b".\0"), &Limits { max_entries: 0, ..Limits::default() });
    assert!(image.entries.is_empty());
    assert!(!image.complete);
}

#[test]
fn appended_bytes_do_not_supply_a_file_payload() {
    let mut data = root_image(b".\0");
    data[8..12].copy_from_slice(&98u32.to_be_bytes());
    let image = fsread::romfs::read(&data, &Limits::default());
    assert_eq!(image.entries[0].size, 5);
    assert!(image.entries[0].data.is_none());
    assert!(!image.complete);
    data[8..12].copy_from_slice(&101u32.to_be_bytes());
    let image = fsread::romfs::read(&data, &Limits::default());
    assert!(image.complete);
    assert_eq!(image.entries[0].data.as_deref(), Some(b"world".as_slice()));
}

#[test]
fn missing_entry_pointers_are_incomplete_but_an_empty_directory_is_complete() {
    let mut data = root_image(b".\0");
    for first in [112u32, 96 + 1, u32::MAX] {
        data[36..40].copy_from_slice(&first.to_be_bytes());
        let image = fsread::romfs::read(&data, &Limits::default());
        assert!(!image.complete, "{first}");
        assert!(image.entries.is_empty());
    }
    data[36..40].fill(0);
    assert!(fsread::romfs::read(&data, &Limits::default()).complete);

    data[36..40].copy_from_slice(&64u32.to_be_bytes());
    for next in [112u32, 64] {
        data[64..68].copy_from_slice(&(next | 2).to_be_bytes());
        let image = fsread::romfs::read(&data, &Limits::default());
        assert!(!image.complete);
        assert_eq!(image.entries.len(), 1);
    }
}

#[test]
fn symlink_targets_share_the_payload_budget_and_retain_their_declared_size() {
    let mut data = craft_anything_testkit::firmware::romfs_image();
    data[64..68].copy_from_slice(&3u32.to_be_bytes()); // readme.txt becomes a link
    for limits in [Limits { max_file_bytes: 10, ..Limits::default() }, Limits { max_total_bytes: 23, ..Limits::default() }] {
        let image = fsread::romfs::read(&data, &limits);
        let link = image.entries.iter().find(|e| e.path == "readme.txt").unwrap();
        assert!(link.symlink_target.is_none());
        assert_eq!(link.size, 11);
        assert!(!image.complete);
    }
    let image = fsread::romfs::read(&data, &Limits { max_total_bytes: 24, ..Limits::default() });
    assert!(image.complete);
    assert_eq!(image.bytes, 24);
    assert_eq!(image.entries.iter().find(|e| e.path == "readme.txt").unwrap().symlink_target.as_deref(), Some("hello romfs"));
}
