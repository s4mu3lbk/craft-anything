use craft_anything_formats::binary::{Binary, parse_binary};
use craft_anything_formats::macho;
use craft_anything_testkit::{MACHO_BASE, OBJC_SELECTOR, macho64};

const LIST: usize = 0x1088;
const ENTRY: usize = LIST + 8;

fn fixture(relative: bool, stride: u32) -> (Vec<u8>, Binary) {
    let mut data = macho64();
    let flags = if relative { 0xc000_0000 } else { 0 };
    data[LIST..LIST + 4].copy_from_slice(&(flags | stride).to_le_bytes());
    if relative {
        for (field, target) in [(0, 0x6f0i32), (4, 0x700), (8, 0x660)] {
            let at = ENTRY + field;
            data[at..at + 4].copy_from_slice(&(target - at as i32).to_le_bytes());
        }
    }
    let binary = parse_binary(&data).unwrap();
    (data, binary)
}

fn methods(data: &[u8], binary: &Binary) -> Vec<macho::ObjcMethod> {
    let metadata = macho::objc_metadata(data, binary);
    assert_eq!(metadata.classes.len(), 1);
    metadata.classes[0].methods.clone()
}

#[test]
fn absolute_records_cannot_borrow_fields_after_their_stride() {
    for stride in [4, 8, 12, 16, 20] {
        let (data, binary) = fixture(false, stride);
        assert!(methods(&data, &binary).is_empty(), "stride {stride}");
    }
}

#[test]
fn relative_records_cannot_borrow_fields_after_their_stride() {
    for stride in [4, 8] {
        let (data, binary) = fixture(true, stride);
        assert!(methods(&data, &binary).is_empty(), "stride {stride}");
    }
}

#[test]
fn incomplete_records_are_not_published_from_the_readable_prefix() {
    for relative in [false, true] {
        let stride = if relative { 12 } else { 24 };
        let (data, mut binary) = fixture(relative, stride);
        let section = binary.sections.iter_mut().find(|section| section.name == "__objc_const").unwrap();
        section.file_size = ENTRY as u64 - section.file_offset.unwrap() + if relative { 4 } else { 8 };
        assert!(methods(&data, &binary).is_empty());
    }
    let (data, binary) = fixture(false, 128);
    assert!(methods(&data, &binary).is_empty(), "declared padding also belongs to the record");
}

#[test]
fn complete_records_and_extended_strides_preserve_method_facts() {
    for (relative, stride) in [(false, 24), (false, 32), (true, 12), (true, 16)] {
        let (data, binary) = fixture(relative, stride);
        let methods = methods(&data, &binary);
        assert_eq!(methods.len(), 1);
        assert_eq!(methods[0].selector, OBJC_SELECTOR);
        assert_eq!(methods[0].types.as_deref(), Some("v24@0:8@16"));
        assert_eq!(methods[0].imp, Some(MACHO_BASE + 0x660));
    }
}
