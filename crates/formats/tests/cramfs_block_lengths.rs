use craft_anything_formats::fsread::{self, Limits};

#[test]
fn a_short_final_block_is_not_a_recovered_file() {
    let mut data = craft_anything_testkit::firmware::cramfs_image();
    data[80..84].copy_from_slice(&20u32.to_le_bytes());
    let image = fsread::cramfs::read(&data, &Limits::default());
    let hello = image.entries.iter().find(|e| e.path == "hello.txt").unwrap();
    assert_eq!(hello.size, 20);
    assert!(hello.data.is_none());
    assert!(!image.complete);
    assert!(image.warnings.iter().any(|w| w.contains("block")));
    assert_eq!(image.entries.iter().find(|e| e.path == "dir/inner").unwrap().data.as_deref(), Some(b"nested".as_slice()));
}

#[test]
fn an_overlong_final_block_is_not_silently_clipped() {
    let mut data = craft_anything_testkit::firmware::cramfs_image();
    data[80..84].copy_from_slice(&18u32.to_le_bytes());
    let image = fsread::cramfs::read(&data, &Limits::default());
    assert!(image.entries.iter().find(|e| e.path == "hello.txt").unwrap().data.is_none());
    assert!(!image.complete);
}

#[test]
fn an_exact_partial_block_still_recovers() {
    let image = fsread::cramfs::read(&craft_anything_testkit::firmware::cramfs_image(), &Limits::default());
    assert!(image.complete, "{:?}", image.warnings);
    assert_eq!(image.entries.iter().find(|e| e.path == "hello.txt").unwrap().data.as_deref(), Some(b"Hello from cramfs!\n".as_slice()));
}

#[test]
fn an_exact_full_final_block_still_recovers() {
    let mut data = craft_anything_testkit::firmware::cramfs_image();
    data.resize(data.len().div_ceil(4) * 4, 0);
    let pointers = data.len();
    data.extend_from_slice(&[0; 4]);
    let expected = vec![b'a'; 4096];
    data.extend_from_slice(&miniz_oxide::deflate::compress_to_vec_zlib(&expected, 6));
    let end = data.len() as u32;
    data[4..8].copy_from_slice(&end.to_le_bytes());
    data[pointers..pointers + 4].copy_from_slice(&end.to_le_bytes());
    data[80..84].copy_from_slice(&4096u32.to_le_bytes());
    data[84..88].copy_from_slice(&(((pointers as u32 / 4) << 6) | 3).to_le_bytes());
    let image = fsread::cramfs::read(&data, &Limits::default());
    let hello = image.entries.iter().find(|e| e.path == "hello.txt").unwrap();
    assert_eq!(hello.data.as_deref(), Some(expected.as_slice()));
    assert!(image.complete, "{:?}", image.warnings);
}

#[test]
fn a_short_middle_block_does_not_shift_later_file_bytes() {
    for first_size in [4095, 4096] {
        let mut data = craft_anything_testkit::firmware::cramfs_image();
        data.resize(data.len().div_ceil(4) * 4, 0);
        let pointers = data.len();
        data.extend_from_slice(&[0; 8]);
        let first = miniz_oxide::deflate::compress_to_vec_zlib(&vec![b'a'; first_size], 6);
        data.extend_from_slice(&first);
        let first_end = data.len() as u32;
        let last = miniz_oxide::deflate::compress_to_vec_zlib(b"finish", 6);
        data.extend_from_slice(&last);
        let last_end = data.len() as u32;
        data[4..8].copy_from_slice(&last_end.to_le_bytes());
        data[pointers..pointers + 4].copy_from_slice(&first_end.to_le_bytes());
        data[pointers + 4..pointers + 8].copy_from_slice(&last_end.to_le_bytes());
        data[80..84].copy_from_slice(&4102u32.to_le_bytes());
        data[84..88].copy_from_slice(&(((pointers as u32 / 4) << 6) | 3).to_le_bytes());
        let image = fsread::cramfs::read(&data, &Limits::default());
        let hello = image.entries.iter().find(|e| e.path == "hello.txt").unwrap();
        if first_size == 4095 {
            assert!(hello.data.is_none());
            assert!(!image.complete);
        } else {
            let body = hello.data.as_ref().unwrap();
            assert_eq!(&body[4096..], b"finish");
            assert!(image.complete, "{:?}", image.warnings);
        }
    }
}

#[test]
fn directory_size_covers_the_whole_padded_name_even_when_only_a_prefix_is_requested() {
    let mut data = craft_anything_testkit::firmware::cramfs_image();
    for size in [11u32, 12, 23] {
        data[68..72].copy_from_slice(&size.to_le_bytes());
        let image = fsread::cramfs::read(&data, &Limits { max_name: 1, ..Limits::default() });
        assert!(image.entries.is_empty(), "size {size}");
        assert!(!image.complete);
    }
    data[68..72].copy_from_slice(&24u32.to_le_bytes());
    let image = fsread::cramfs::read(&data, &Limits::default());
    assert!(image.complete);
    assert_eq!(image.entries.len(), 1);
    assert_eq!(image.entries[0].path, "hello.txt");
}
