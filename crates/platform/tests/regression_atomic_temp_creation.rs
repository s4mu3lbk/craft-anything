use craft_anything_platform::fs::write_atomic;
#[cfg(unix)]
use craft_anything_platform::fs::write_atomic_private;
use std::path::{Path, PathBuf};

fn temporary(dir: &Path, counter: u64) -> PathBuf {
    dir.join(format!(".craft-anything.tmp-{}-{counter}", std::process::id()))
}

#[test]
fn atomic_writes_preserve_colliding_files_and_links() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("settings.json");
    let stale: Vec<_> = (0..4).map(|n| temporary(dir.path(), n)).collect();
    for file in &stale {
        std::fs::write(file, b"existing temporary data").unwrap();
    }
    write_atomic(&target, b"new settings").unwrap();
    assert_eq!(std::fs::read(&target).unwrap(), b"new settings");
    for file in &stale {
        assert_eq!(std::fs::read(file).unwrap(), b"existing temporary data");
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::{PermissionsExt, symlink};
        let private_dir = tempfile::tempdir().unwrap();
        let other = private_dir.path().join("other.txt");
        std::fs::write(&other, b"keep this file").unwrap();
        let links: Vec<_> = (0..16).map(|n| temporary(private_dir.path(), n)).collect();
        for link in &links {
            symlink(&other, link).unwrap();
        }
        let private = private_dir.path().join("private.json");
        write_atomic_private(&private, b"private fixture").unwrap();
        assert_eq!(std::fs::read(&other).unwrap(), b"keep this file");
        assert_eq!(std::fs::read(&private).unwrap(), b"private fixture");
        assert_eq!(std::fs::metadata(&private).unwrap().permissions().mode() & 0o777, 0o600);
        assert!(links.iter().all(|link| std::fs::symlink_metadata(link).unwrap().file_type().is_symlink()));

        std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o640)).unwrap();
        write_atomic(&target, b"replacement settings").unwrap();
        assert_eq!(std::fs::metadata(&target).unwrap().permissions().mode() & 0o777, 0o640);
    }

    // A bounded retry must leave both the destination and every colliding file alone.
    let crowded_dir = tempfile::tempdir().unwrap();
    let destination = crowded_dir.path().join("crowded.json");
    std::fs::write(&destination, b"original settings").unwrap();
    let occupied: Vec<_> = (0..256).map(|n| temporary(crowded_dir.path(), n)).collect();
    for file in &occupied {
        std::fs::write(file, b"someone else's temporary data").unwrap();
    }
    assert!(write_atomic(&destination, b"replacement").is_err());
    assert_eq!(std::fs::read(&destination).unwrap(), b"original settings");
    for file in &occupied {
        assert_eq!(std::fs::read(file).unwrap(), b"someone else's temporary data");
    }

    // An error after creating our temporary file cleans up only that new file.
    let blocked_dir = tempfile::tempdir().unwrap();
    let blocked = blocked_dir.path().join("directory.json");
    std::fs::create_dir(&blocked).unwrap();
    let mut before: Vec<_> = std::fs::read_dir(blocked_dir.path()).unwrap().map(|entry| entry.unwrap().path()).collect();
    before.sort();
    assert!(write_atomic(&blocked, b"cannot replace a directory").is_err());
    let mut after: Vec<_> = std::fs::read_dir(blocked_dir.path()).unwrap().map(|entry| entry.unwrap().path()).collect();
    after.sort();
    assert_eq!(after, before);

    // Probe at most 255 names, retaining the longest this filesystem can create.
    let long_dir = tempfile::tempdir().unwrap();
    let mut long_target = None;
    for length in (1..=255).rev() {
        let path = long_dir.path().join("s".repeat(length));
        match std::fs::write(&path, b"original long-name settings") {
            Ok(()) => {
                long_target = Some(path);
                break;
            }
            Err(error) if error.kind() == std::io::ErrorKind::InvalidFilename => continue,
            Err(error) => panic!("could not create the long-name fixture: {error}"),
        }
    }
    let long_target = long_target.expect("the test filesystem must support a destination filename");
    write_atomic(&long_target, b"replacement long-name settings").unwrap();
    assert_eq!(std::fs::read(&long_target).unwrap(), b"replacement long-name settings");
    assert_eq!(std::fs::read_dir(long_dir.path()).unwrap().count(), 1);
}
