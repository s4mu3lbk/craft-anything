use craft_anything_setup::{Check, CheckStatus, DoctorRequest, Host, doctor};
use std::path::{Path, PathBuf};

const TEST: &str = "a_registration_by_bare_name_is_looked_up_on_path";
/// Set when this test runs itself again with a chosen PATH; holds the test's home folder.
const INNER_HOME: &str = "CRAFT_ANYTHING_TEST_DOCTOR_HOME";
const INNER_CLI: &str = "CRAFT_ANYTHING_TEST_DOCTOR_CLI";

/// Doctor's verdict on Claude Code's registration, worked out in a copy of this test started with
/// only `path_dir` on PATH, in the CLI's folder. Home and the local app folder point into the
/// test's own folder, so no real install is found.
fn registration(home: &Path, cli: &Path, path_dir: &Path) -> Check {
    let out = std::process::Command::new(std::env::current_exe().unwrap())
        .args([TEST, "--exact", "--nocapture", "--test-threads=1"])
        .current_dir(cli.parent().unwrap())
        .env("PATH", path_dir)
        .env("HOME", home)
        .env("USERPROFILE", home)
        .env("LOCALAPPDATA", home.join("local"))
        .env_remove("NPM_CONFIG_PREFIX")
        .env_remove("npm_config_prefix")
        .env_remove("NVM_HOME")
        .env(INNER_HOME, home)
        .env(INNER_CLI, cli)
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    let line = stdout
        .lines()
        .find_map(|l| l.split_once("CHECK ").map(|(_, json)| json))
        .unwrap_or_else(|| panic!("no check in {stdout}\n{}", String::from_utf8_lossy(&out.stderr)));
    serde_json::from_str(line).unwrap()
}

/// A registration may name the CLI by its bare name, as `claude mcp add -s user craft-anything --
/// craft-anything-cli mcp` writes it. The app finds it on PATH, and so does doctor now instead of
/// calling it missing. With nothing on PATH it is still missing, even when doctor runs in the
/// CLI's own folder.
#[test]
fn a_registration_by_bare_name_is_looked_up_on_path() {
    if let (Some(home), Some(cli)) = (std::env::var_os(INNER_HOME), std::env::var_os(INNER_CLI)) {
        let req =
            DoctorRequest { clients: vec!["claude_code".into()], cli_path: Some(PathBuf::from(cli)), self_test: false, engines: false, ..Default::default() };
        let check = doctor(&Host::with_home(PathBuf::from(home)), &req).checks.into_iter().find(|c| c.name == "registration:claude_code").unwrap();
        println!("CHECK {}", serde_json::to_string(&check).unwrap());
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    // Not `bin`: `~/bin` is one of the usual folders searched after PATH.
    let bin = home.join("on-path");
    let empty = home.join("empty");
    std::fs::create_dir_all(&bin).unwrap();
    std::fs::create_dir_all(&empty).unwrap();
    let cli = bin.join(if cfg!(windows) { "craft-anything-cli.exe" } else { "craft-anything-cli" });
    std::fs::write(&cli, "").unwrap();
    // A file by the bare name itself, which a check of the current folder would take.
    std::fs::write(bin.join("craft-anything-cli"), "").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(&cli, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    std::fs::write(home.join(".claude.json"), r#"{"mcpServers": {"craft-anything": {"command": "craft-anything-cli", "args": ["mcp"]}}}"#).unwrap();

    let on_path = registration(home, &cli, &bin);
    assert_ne!(on_path.status, CheckStatus::Fail, "{on_path:?}");
    assert!(!on_path.message.contains("no longer exists"), "{on_path:?}");

    let not_on_path = registration(home, &cli, &empty);
    assert_eq!(not_on_path.status, CheckStatus::Fail, "{not_on_path:?}");
    assert!(not_on_path.message.contains("no longer exists"), "{not_on_path:?}");
}
