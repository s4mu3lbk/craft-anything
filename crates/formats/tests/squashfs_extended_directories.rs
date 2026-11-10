use craft_anything_formats::fsread::{self, Limits};
use craft_anything_testkit::firmware::{squashfs_image, squashfs_image_with_extended_directories};

fn check_directories(root: bool, nested: bool) {
    let limits = Limits::default();
    let expected = fsread::read("squashfs", &squashfs_image(), &limits).unwrap();
    assert!(expected.complete);
    assert_eq!(expected.entries.len(), 3);
    let bytes = squashfs_image_with_extended_directories(root, nested);
    let actual = fsread::read("squashfs", &bytes, &limits).unwrap();
    // Compare the names, kinds, contents and completeness, not only the number of entries.
    assert_eq!(actual, expected);
}

#[test]
fn reads_an_extended_root_directory() {
    check_directories(true, false);
}

#[test]
fn reads_an_extended_nested_directory_at_a_nonzero_offset() {
    check_directories(false, true);
}

#[test]
fn reads_extended_root_and_nested_directories_together() {
    check_directories(true, true);
}

#[test]
fn tables_and_file_blocks_must_be_inside_bytes_used() {
    let mut data = squashfs_image();
    let limits = Limits::default();
    let good = fsread::squashfs::read(&data, &limits);
    let used = data.len();
    data.extend_from_slice(craft_anything_testkit::firmware::SQUASHFS_README);
    assert_eq!(fsread::squashfs::read(&data, &limits), good);
    let inode_table = u64::from_le_bytes(data[64..72].try_into().unwrap()) as usize;
    data[inode_table + 18..inode_table + 22].copy_from_slice(&(used as u32).to_le_bytes());
    let image = fsread::squashfs::read(&data, &limits);
    assert!(image.entries.iter().find(|e| e.path == "readme").unwrap().data.is_none());
    assert!(!image.complete);

    let dir_table = u64::from_le_bytes(data[72..80].try_into().unwrap()) as usize;
    for end in [95, inode_table, inode_table + 3, dir_table, dir_table + 3] {
        data[40..48].copy_from_slice(&(end as u64).to_le_bytes());
        let image = fsread::squashfs::read(&data, &limits);
        assert!(image.entries.is_empty());
        assert!(!image.complete);
    }
}

#[test]
fn a_directory_with_no_entries_needs_no_directory_table_data() {
    let mut data = squashfs_image();
    let inodes = u64::from_le_bytes(data[64..72].try_into().unwrap()) as usize;
    let root = u64::from_le_bytes(data[32..40].try_into().unwrap()) as usize;
    let size = inodes + 2 + root + 24;
    data[size..size + 2].copy_from_slice(&3u16.to_le_bytes());
    let dirs = u64::from_le_bytes(data[72..80].try_into().unwrap());
    data[40..48].copy_from_slice(&dirs.to_le_bytes());
    let image = fsread::squashfs::read(&data, &Limits::default());
    assert!(image.complete, "{:?}", image.warnings);
    assert!(image.entries.is_empty());
}
