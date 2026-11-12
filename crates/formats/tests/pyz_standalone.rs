//! A PyInstaller PYZ archive given on its own, not inside a program's CArchive.

use craft_anything_formats::packed::{PackedFormat, detect};
use craft_anything_formats::pyinstaller::{PyInstaller, PyzArchive};
use craft_anything_testkit::packed::{code_311, pyz};

const MAGIC: u16 = 3531;

/// A 3.12 PYZ holding a package, a module and a modern namespace package.
fn archive() -> (Vec<u8>, Vec<u8>) {
    let code = code_311("<module>", "<module>", "pkg/util.py", 1, &["os"], &["util doc"], &[]);
    let data = pyz(MAGIC, &[("pkg", 1, &code), ("pkg.util", 0, &code), ("pkg.ns", 3, b"")], false, true);
    (data, code)
}

#[test]
fn a_standalone_pyz_lists_its_modules_and_hands_one_back() {
    let (data, code) = archive();
    assert_eq!(detect(&data).map(|r| r.id()), Some("pyinstaller_pyz"), "it's the PYZ reader's file");
    assert!(!PyInstaller.detect(&data), "a bare archive has no CArchive cookie");
    let listing = PyzArchive.list(&data).unwrap();
    let paths: Vec<&str> = listing.entries.iter().map(|e| e.path.as_str()).collect();
    assert_eq!(paths, ["pyz.modules/pkg/__init__.pyc", "pyz.modules/pkg/util.pyc"]);
    assert_eq!(listing.facts["magic"], MAGIC);
    assert_eq!(listing.facts["modules"], 3);
    assert_eq!(listing.facts["namespace_packages"], serde_json::json!(["pkg.ns"]));
    assert_eq!(listing.facts["encrypted"], false);
    assert_eq!(listing.facts["python"], "3.12");
    assert!(listing.notes.is_empty(), "{:?}", listing.notes);
    assert!(!listing.truncated);

    let module = &listing.entries[1];
    let pyc = PyzArchive.read(&data, module, 4096).unwrap();
    assert_eq!(&pyc[..2], &MAGIC.to_le_bytes(), "the .pyc header goes in front");
    assert_eq!(&pyc[16..], code.as_slice(), "the module's marshalled code follows it");
}

#[test]
fn a_pyz_without_pyz_magic_is_left_to_other_readers() {
    let (mut data, _) = archive();
    // A Python zip application named `.pyz` starts like any ZIP: the reader must not claim it.
    data[..4].copy_from_slice(b"PK\x03\x04");
    assert!(!PyzArchive.detect(&data));
    // PyInstaller's magic needs its line-ending bytes too.
    let mut short = b"PYZ\0\x1b\r\n".to_vec();
    short[6] = b'x';
    assert!(!PyzArchive.detect(&short));
}

#[test]
fn a_table_that_lies_says_why_nothing_is_listed() {
    let (mut data, _) = archive();
    data[8..12].copy_from_slice(&u32::MAX.to_be_bytes());
    let err = PyzArchive.list(&data).unwrap_err().to_string();
    assert!(err.contains("table"), "{err}");
    // A cut archive is refused rather than listed empty.
    let cut = &data[..16];
    assert!(PyzArchive.list(cut).is_err());
}
