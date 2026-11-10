use craft_anything_setup::Host;
use std::path::PathBuf;

const TEST: &str = "the_home_folder_is_the_one_the_apps_use";
/// Set when this test runs itself again with chosen HOME and USERPROFILE.
const INNER: &str = "CRAFT_ANYTHING_TEST_HOST_HOME";

/// The home folder `Host::current` finds in a copy of this test started with these variables.
fn current_home(home: Option<&str>, profile: Option<&str>) -> PathBuf {
    let mut cmd = std::process::Command::new(std::env::current_exe().unwrap());
    cmd.args([TEST, "--exact", "--nocapture", "--test-threads=1"]).env(INNER, "1").env_remove("HOME").env_remove("USERPROFILE");
    if let Some(h) = home {
        cmd.env("HOME", h);
    }
    if let Some(p) = profile {
        cmd.env("USERPROFILE", p);
    }
    let out = cmd.output().unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    let line = stdout
        .lines()
        .find_map(|l| l.split_once("HOME ").map(|(_, home)| home.to_string()))
        .unwrap_or_else(|| panic!("no home in {stdout}\n{}", String::from_utf8_lossy(&out.stderr)));
    PathBuf::from(line)
}

/// The apps keep their settings under the folder they take for home. On Windows that is the user
/// profile: Node, Python and Go read USERPROFILE there, Rust's `dirs` asks Windows for the profile
/// folder, and none of them reads HOME, which a shell such as MSYS2 or Cygwin points elsewhere.
/// Elsewhere it is HOME.
#[test]
fn the_home_folder_is_the_one_the_apps_use() {
    if std::env::var_os(INNER).is_some() {
        println!("HOME {}", Host::current().home.display());
        return;
    }
    let (home, profile) = if cfg!(windows) { (r"C:\msys64\home\someone", r"C:\Users\someone") } else { ("/home/someone", "/elsewhere") };
    let expected = if cfg!(windows) { profile } else { home };
    assert_eq!(current_home(Some(home), Some(profile)), PathBuf::from(expected));
    // With only one of them set, that one is used. An empty value counts as not set.
    assert_eq!(current_home(Some(home), None), PathBuf::from(home));
    assert_eq!(current_home(None, Some(profile)), PathBuf::from(profile));
    assert_eq!(current_home(Some(home), Some("")), PathBuf::from(home));
    assert_eq!(current_home(Some(""), Some(profile)), PathBuf::from(profile));
    assert_eq!(current_home(Some(""), Some("")), PathBuf::from("."));
}
