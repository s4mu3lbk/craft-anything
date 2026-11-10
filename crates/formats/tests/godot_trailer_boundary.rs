use craft_anything_formats::godot_pck::{self, GodotPck};
use craft_anything_formats::packed::{Compression, PackedFormat};
use craft_anything_testkit::games::{GodotFile, godot_embedded, godot_pck, godot_pck_at};

fn sample(format: u32) -> Vec<u8> {
    let files = [GodotFile { path: "a", data: b"payload", flags: 0, md5: [0; 16] }];
    godot_embedded(&[0x7f; 64], &godot_pck_at(format, 2, 64, &files))
}

#[test]
fn an_embedded_file_cannot_read_the_trailer() {
    for format in [1, 2, 3] {
        let mut data = sample(format);
        let pack = godot_pck::parse(&data).unwrap();
        let directory = pack.start as usize + pack.header.directory_offset.unwrap() as usize;
        let path_len = u32::from_le_bytes(data[directory + 4..directory + 8].try_into().unwrap()) as usize;
        let member_offset = directory + 8 + path_len;
        let end = data.len() - godot_pck::TRAILER_SIZE as usize;
        let base = if format == 1 { 0 } else { pack.start + pack.header.file_base };
        data[member_offset..member_offset + 8].copy_from_slice(&(end as u64 - base).to_le_bytes());
        let listing = GodotPck.list(&data).unwrap();
        assert_eq!(listing.entries[0].compression, Compression::Unreadable);
        assert!(listing.notes.iter().any(|note| note.contains("outside")));
        assert!(GodotPck.read(&data, &listing.entries[0], 100).is_err());
        let mut claimed_readable = listing.entries[0].clone();
        claimed_readable.compression = Compression::None;
        assert!(GodotPck.read(&data, &claimed_readable, 100).is_err());
    }
}

#[test]
fn a_directory_count_in_the_trailer_is_not_a_pack_table() {
    let mut pack = godot_pck(3, 2, &[]);
    let end = pack.len() as u64;
    pack[32..40].copy_from_slice(&end.to_le_bytes());
    let data = godot_embedded(&[0x7f; 64], &pack);
    assert!(godot_pck::parse(&data).is_err());
}

#[test]
fn a_table_record_cannot_finish_in_the_trailer() {
    let files = [GodotFile { path: "a", data: b"", flags: 0, md5: [0; 16] }];
    let mut pack = godot_pck(2, 2, &files);
    let directory = godot_pck::header(&pack).unwrap().directory_offset.unwrap() as usize;
    let path_len = u32::from_le_bytes(pack[directory + 4..directory + 8].try_into().unwrap()) as usize;
    pack.truncate(directory + 8 + path_len + 32);
    let data = godot_embedded(&[0x7f; 64], &pack);
    let listing = GodotPck.list(&data).unwrap();
    assert!(listing.truncated);
    assert!(listing.entries.is_empty());
}

#[test]
fn complete_embedded_and_standalone_members_still_read() {
    for format in [1, 2, 3] {
        let data = sample(format);
        let listing = GodotPck.list(&data).unwrap();
        assert!(!listing.truncated);
        assert_eq!(GodotPck.read(&data, &listing.entries[0], 100).unwrap(), b"payload");
        let files = [GodotFile { path: "a", data: b"payload", flags: 0, md5: [0; 16] }];
        let data = godot_pck(format, 2, &files);
        let listing = GodotPck.list(&data).unwrap();
        assert_eq!(GodotPck.read(&data, &listing.entries[0], 100).unwrap(), b"payload");
    }
}
