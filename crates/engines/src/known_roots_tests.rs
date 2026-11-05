//! Unpacked installs found in the folders where people keep them (#740). Each case has its own temp
//! folder; nothing here reads the real home folder or Program Files.

use super::*;

fn touch(p: &Path) {
    std::fs::create_dir_all(p.parent().expect("a file has a parent")).expect("create folders");
    std::fs::write(p, b"x").expect("write file");
}

#[test]
fn an_unzipped_release_next_to_its_folder_name_is_found() {
    let d = tempfile::tempdir().expect("tempdir");
    let launcher = d.path().join("ghidra_12.1.4_PUBLIC").join("support").join("analyzeHeadless");
    touch(&launcher);
    let roots = [(d.path().to_path_buf(), 1)];
    assert_eq!(find_in_known_roots("ghidra", &["analyzeHeadless"], &roots), Some(launcher));
}

#[test]
fn a_capitalised_folder_name_is_found_too() {
    let d = tempfile::tempdir().expect("tempdir");
    let launcher = d.path().join("Ghidra").join("support").join("analyzeHeadless.bat");
    touch(&launcher);
    assert_eq!(find_in_known_roots("ghidra", &["analyzeHeadless.bat"], &[(d.path().to_path_buf(), 1)]), Some(launcher));
}

#[test]
fn a_package_manager_folder_two_levels_down_is_found_when_the_depth_allows_it() {
    let d = tempfile::tempdir().expect("tempdir");
    // Homebrew: Caskroom/ghidra/<version>/ghidra_<version>_PUBLIC/support/analyzeHeadless
    let caskroom = d.path().join("ghidra");
    let launcher = caskroom.join("11.4").join("ghidra_11.4_PUBLIC").join("support").join("analyzeHeadless");
    touch(&launcher);
    assert_eq!(find_in_known_roots("ghidra", &["analyzeHeadless"], &[(caskroom.clone(), 2)]), Some(launcher));
    assert_eq!(find_in_known_roots("ghidra", &["analyzeHeadless"], &[(caskroom, 1)]), None, "one level is not enough");
}

#[test]
fn a_newer_version_is_preferred() {
    let d = tempfile::tempdir().expect("tempdir");
    let old = d.path().join("ghidra_11.4_PUBLIC").join("support").join("analyzeHeadless");
    let new = d.path().join("ghidra_12.1.4_PUBLIC").join("support").join("analyzeHeadless");
    touch(&old);
    touch(&new);
    assert_eq!(find_in_known_roots("ghidra", &["analyzeHeadless"], &[(d.path().to_path_buf(), 1)]), Some(new));
}

#[test]
fn other_programs_and_missing_folders_are_ignored() {
    let d = tempfile::tempdir().expect("tempdir");
    touch(&d.path().join("other_tool").join("support").join("analyzeHeadless"));
    let missing = d.path().join("not-there");
    assert_eq!(find_in_known_roots("ghidra", &["analyzeHeadless"], &[(d.path().to_path_buf(), 1), (missing, 1)]), None);
    assert_eq!(find_in_known_roots("ghidra", &["analyzeHeadless"], &[]), None);
}

#[test]
fn a_folder_without_the_launcher_is_not_an_install() {
    let d = tempfile::tempdir().expect("tempdir");
    std::fs::create_dir_all(d.path().join("ghidra_12.1.4_PUBLIC").join("Ghidra")).expect("create folders");
    assert_eq!(find_in_known_roots("ghidra", &["analyzeHeadless"], &[(d.path().to_path_buf(), 1)]), None);
}

#[test]
fn ida_installs_and_macos_bundles_are_found() {
    let d = tempfile::tempdir().expect("tempdir");
    let windows = d.path().join("IDA Professional 9.4").join("idat.exe");
    touch(&windows);
    assert_eq!(find_in_known_roots("ida", &["idat.exe"], &[(d.path().to_path_buf(), 1)]), Some(windows));
    let mac = d.path().join("IDA Pro.app").join("Contents/MacOS/idat");
    touch(&mac);
    assert_eq!(find_in_known_roots("ida", &["idat"], &[(d.path().to_path_buf(), 1)]), Some(mac));
}

#[test]
fn every_host_folder_for_ghidra_resolves_without_panicking() {
    let ghidra = engine("ghidra").expect("ghidra is in the table");
    for (folder, depth) in ghidra.known_root_folders() {
        assert!(depth >= 1, "{folder:?} must look at least one level down");
    }
}
