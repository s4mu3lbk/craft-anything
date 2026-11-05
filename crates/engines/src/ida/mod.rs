//! Headless IDA analysis through an original IDAPython bridge (Deep mode only).
//!
//! IDA analyses a private snapshot once and saves a content-keyed database. Later sessions reopen
//! that database and use an authenticated, bounded loopback NDJSON connection. The external process
//! is owned by `craft_anything_platform::process`, so cancellation, timeout and drop stop its group.

use crate::deep::{Client, DeepDecompiler, DeepError, DeepFunction};
use craft_anything_platform::process::{Process, RunningProcess, StdinMode};
use craft_anything_platform::sync::Mutex;
use craft_anything_platform::write_atomic_private;
use serde::Serialize;
use serde_json::Value;
use std::ffi::OsString;
use std::net::{Ipv4Addr, SocketAddr};
use std::path::{Path, PathBuf};
use std::sync::PoisonError;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime};

pub const ID: &str = "ida";
const BRIDGE_SCRIPT_NAME: &str = "CraftAnythingBridge.py";
const BRIDGE_SCRIPT: &str = include_str!("CraftAnythingBridge.py");
const ANALYSED_MARKER: &str = "analysed.ok";
const DATABASE_NAME: &str = "target.i64";
const SESSION_LIFETIME: Duration = Duration::from_secs(7 * 24 * 3600);
const REQUEST_TIME: Duration = Duration::from_secs(15 * 60);
const LOCK_STALE: Duration = Duration::from_secs(90);

#[derive(Debug, Clone)]
pub struct Config {
    /// Path to IDA's console executable (`idat` or `idat64`).
    pub executable: PathBuf,
    pub startup_timeout: Duration,
    pub store_dir: PathBuf,
}

#[derive(Debug, Clone, Serialize)]
struct Descriptor {
    token: String,
    endpoint_path: String,
    database_path: String,
    prepare_only: bool,
    target_sha256: String,
    settings_hash: String,
}

pub struct Session {
    client: Mutex<Client>,
    process: Mutex<RunningProcess>,
    runtime_root: PathBuf,
    version: Option<String>,
    image_base: Option<u64>,
}

impl std::fmt::Debug for Session {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IdaSession").field("runtime_root", &self.runtime_root).finish()
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        if let Ok(mut client) = self.client.lock() {
            client.shutdown();
        }
        if let Ok(mut process) = self.process.lock() {
            process.kill_and_wait(Duration::from_secs(5));
        }
        remove_soon(&self.runtime_root);
    }
}

pub type Progress<'a> = dyn Fn(f32, &str) + Send + Sync + 'a;
pub type Cancel<'a> = dyn Fn() -> bool + Send + Sync + 'a;

fn random_token() -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let number = COUNTER.fetch_add(1, Ordering::Relaxed);
    let nanos = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let seed = format!("{}|{nanos}|{number}|ida-bridge", std::process::id());
    craft_anything_evidence::sha256_hex(seed.as_bytes())
}

pub fn project_cache_dir(store: &Path, target_sha256: &str, settings_hash: &str) -> PathBuf {
    let key = format!("{}-{}", target_sha256.get(..16).unwrap_or(target_sha256), settings_hash.get(..16).unwrap_or(settings_hash));
    store.join("ida-cache").join(key)
}

fn database_path(cache: &Path) -> PathBuf {
    cache.join(DATABASE_NAME)
}

pub fn is_analysed(cache: &Path) -> bool {
    cache.join(ANALYSED_MARKER).is_file() && database_path(cache).is_file()
}

fn make_private_dir(path: &Path) -> Result<(), DeepError> {
    let mut builder = std::fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt as _;
        builder.mode(0o700);
    }
    builder.create(path).map_err(|e| DeepError::Launch(format!("creating {}: {e}", path.display())))
}

fn private_dir(base: &Path, prefix: &str) -> Result<PathBuf, DeepError> {
    let nanos = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let path = base.join(format!("{prefix}-{}-{nanos}", std::process::id()));
    make_private_dir(&path)?;
    Ok(path)
}

struct Scratch(PathBuf);

impl Scratch {
    fn new(store: &Path, prefix: &str) -> Result<Scratch, DeepError> {
        let root = private_dir(store, prefix)?;
        for sub in ["home", "tmp"] {
            make_private_dir(&root.join(sub))?;
        }
        Ok(Scratch(root))
    }

    fn keep(self) -> PathBuf {
        let path = self.0.clone();
        std::mem::forget(self);
        path
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        remove_soon(&self.0);
    }
}

fn remove_soon(path: &Path) {
    for _ in 0..20 {
        if std::fs::remove_dir_all(path).is_ok() || !path.exists() {
            return;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

fn path_arg(prefix: &str, path: &Path) -> OsString {
    let mut arg = OsString::from(prefix);
    arg.push(path.as_os_str());
    arg
}

pub(crate) fn analyse_args(snapshot: &Path, database: &Path, log: &Path, script: &Path) -> Vec<OsString> {
    vec![OsString::from("-A"), OsString::from("-c"), path_arg("-o", database), path_arg("-L", log), path_arg("-S", script), snapshot.as_os_str().to_owned()]
}

pub(crate) fn bridge_args(database: &Path, log: &Path, script: &Path) -> Vec<OsString> {
    vec![OsString::from("-A"), path_arg("-L", log), path_arg("-S", script), database.as_os_str().to_owned()]
}

fn launch(cfg: &Config, runtime: &Path, args: Vec<OsString>, lifetime: Duration) -> Result<RunningProcess, DeepError> {
    // Keep IDA's own user configuration so its licence and configured Python remain available.
    // Analysis and temporary files are isolated instead of replacing that configuration.
    Process::new(&cfg.executable)
        .args(&args)
        .env("TMPDIR", runtime.join("tmp"))
        .env("TMP", runtime.join("tmp"))
        .env("TEMP", runtime.join("tmp"))
        .env("TERM", "dumb")
        .stdin(StdinMode::Closed)
        .timeout(lifetime)
        .stdout_cap(4 * 1024 * 1024)
        .stderr_cap(1024 * 1024)
        .label("IDA headless")
        .spawn()
        .map_err(|e| DeepError::Launch(e.message))
}

struct HeldLock(PathBuf);

impl Drop for HeldLock {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

fn lock_is_fresh(path: &Path) -> bool {
    std::fs::metadata(path).and_then(|m| m.modified()).is_ok_and(|time| time.elapsed().is_ok_and(|age| age < LOCK_STALE))
}

fn write_bridge(runtime: &Path, descriptor: &Descriptor) -> Result<PathBuf, DeepError> {
    write_atomic_private(&runtime.join(BRIDGE_SCRIPT_NAME), BRIDGE_SCRIPT.as_bytes()).map_err(|e| DeepError::Launch(format!("writing the IDA bridge: {e}")))?;
    let descriptor_path = runtime.join("session.json");
    let bytes = serde_json::to_vec(descriptor).map_err(|e| DeepError::Launch(format!("encoding the IDA session: {e}")))?;
    write_atomic_private(&descriptor_path, &bytes).map_err(|e| DeepError::Launch(format!("writing the IDA session: {e}")))?;
    Ok(runtime.join(BRIDGE_SCRIPT_NAME))
}

fn copy_log(log: &Path, error: &Path, dest: &Path) -> PathBuf {
    let mut bytes = std::fs::read(log).unwrap_or_default();
    if let Ok(extra) = std::fs::read(error) {
        bytes.extend_from_slice(b"\n");
        bytes.extend_from_slice(&extra);
    }
    if !bytes.is_empty() && std::fs::write(dest, bytes).is_ok() { dest.to_path_buf() } else { log.to_path_buf() }
}

/// Analyse and save a target once, sharing the cached database with other Craft Anything processes.
pub fn analyse(
    cfg: &Config,
    target_bytes: &[u8],
    target_sha256: &str,
    settings_hash: &str,
    cancel: &Cancel<'_>,
    progress: &Progress<'_>,
) -> Result<PathBuf, DeepError> {
    if craft_anything_evidence::sha256_hex(target_bytes) != target_sha256 {
        return Err(DeepError::Launch("target bytes do not match the expected sha256".into()));
    }
    let started = Instant::now();
    let deadline = started + cfg.startup_timeout;
    let cache = project_cache_dir(&cfg.store_dir, target_sha256, settings_hash);
    let database = database_path(&cache);
    let lock = cache.with_extension("analysing");
    make_private_dir(lock.parent().unwrap_or(&cache))?;
    loop {
        if is_analysed(&cache) {
            return Ok(database);
        }
        if cancel() {
            return Err(DeepError::Cancelled);
        }
        if Instant::now() >= deadline {
            return Err(DeepError::Timeout(format!("IDA's analysis didn't finish within {} s", cfg.startup_timeout.as_secs())));
        }
        if lock_is_fresh(&lock) {
            progress(0.05, "waiting for IDA's analysis of this program, started earlier");
            std::thread::sleep(Duration::from_secs(2));
            continue;
        }
        let _ = std::fs::remove_file(&lock);
        match std::fs::OpenOptions::new().write(true).create_new(true).open(&lock) {
            Ok(_) => break,
            Err(e) if e.kind() != std::io::ErrorKind::AlreadyExists => {
                return Err(DeepError::Launch(format!("taking the IDA analysis lock {}: {e}", lock.display())));
            }
            Err(_) => {}
        }
    }
    let held = HeldLock(lock.clone());
    remove_soon(&cache);
    make_private_dir(&cache)?;
    let scratch = Scratch::new(&cfg.store_dir, "ida-analysis")?;
    let runtime = scratch.0.clone();
    let snapshot = runtime.join(format!("target-{}", target_sha256.get(..12).unwrap_or(target_sha256)));
    write_atomic_private(&snapshot, target_bytes).map_err(|e| DeepError::Launch(format!("copying the target for IDA: {e}")))?;
    let log = cache.with_extension("log");
    let endpoint = runtime.join("bridge-address.json");
    let descriptor = Descriptor {
        token: random_token(),
        endpoint_path: endpoint.display().to_string(),
        database_path: database.display().to_string(),
        prepare_only: true,
        target_sha256: target_sha256.to_string(),
        settings_hash: settings_hash.to_string(),
    };
    let script = write_bridge(&runtime, &descriptor)?;
    progress(0.02, "IDA is analysing the program (once per program; a big one can take hours)");
    let remaining = deadline.saturating_duration_since(Instant::now());
    let mut running = launch(cfg, &runtime, analyse_args(&snapshot, &database, &log, &script), remaining)?;
    let mut last_progress = Instant::now();
    let result = loop {
        if cancel() {
            return Err(DeepError::Cancelled);
        }
        if let Some(result) = running.wait_timeout(Duration::from_millis(500)) {
            break result.as_ref().map(|output| output.success).map_err(|e| e.message.clone());
        }
        if last_progress.elapsed() >= Duration::from_secs(10) {
            let _ = std::fs::write(&lock, started.elapsed().as_secs().to_string());
            let minutes = started.elapsed().as_secs() / 60;
            progress(0.05, &format!("IDA is analysing the program ({minutes} min so far; once per program)"));
            last_progress = Instant::now();
        }
    };
    match result {
        Ok(true) if database.is_file() => {
            write_atomic_private(&cache.join(ANALYSED_MARKER), target_sha256.as_bytes())
                .map_err(|e| DeepError::Launch(format!("saving IDA's analysis marker: {e}")))?;
        }
        Ok(_) => {
            let kept = copy_log(&log, &runtime.join("bridge-error.txt"), &cache.with_extension("error.log"));
            return Err(DeepError::Launch(format!("IDA's analysis failed; see {}", kept.display())));
        }
        Err(message) => return Err(DeepError::Timeout(message)),
    }
    drop(held);
    Ok(database)
}

impl Session {
    pub fn start(
        cfg: &Config,
        target_bytes: &[u8],
        target_sha256: &str,
        settings_hash: &str,
        cancel: &Cancel<'_>,
        progress: &Progress<'_>,
    ) -> Result<Session, DeepError> {
        let database = analyse(cfg, target_bytes, target_sha256, settings_hash, cancel, progress)?;
        let scratch = Scratch::new(&cfg.store_dir, "ida-runtime")?;
        let runtime = scratch.0.clone();
        let endpoint = runtime.join("bridge-address.json");
        let descriptor = Descriptor {
            token: random_token(),
            endpoint_path: endpoint.display().to_string(),
            database_path: database.display().to_string(),
            prepare_only: false,
            target_sha256: target_sha256.to_string(),
            settings_hash: settings_hash.to_string(),
        };
        let token = descriptor.token.clone();
        let script = write_bridge(&runtime, &descriptor)?;
        let log = runtime.join("ida.log");
        progress(0.92, "opening the analysed program in IDA");
        let mut running = launch(cfg, &runtime, bridge_args(&database, &log, &script), SESSION_LIFETIME)?;
        let deadline = Instant::now() + cfg.startup_timeout;
        let address = loop {
            if cancel() {
                return Err(DeepError::Cancelled);
            }
            if let Some(address) = read_endpoint(&endpoint) {
                break address;
            }
            if running.is_finished() {
                let cache = database.parent().unwrap_or(&cfg.store_dir);
                let _ = std::fs::remove_file(cache.join(ANALYSED_MARKER));
                let kept = copy_log(&log, &runtime.join("bridge-error.txt"), &cache.with_extension("bridge.log"));
                return Err(DeepError::Launch(format!("IDA stopped before its bridge started; see {}", kept.display())));
            }
            if Instant::now() >= deadline {
                return Err(DeepError::Timeout(format!("IDA did not become ready within {} s", cfg.startup_timeout.as_secs())));
            }
            std::thread::sleep(Duration::from_millis(100));
        };
        progress(0.97, "connecting to the IDA analysis bridge");
        let mut client = Client::connect(address, &token, deadline)?;
        let info = client.ping(cancel, deadline)?;
        if info.get("decompiler_available").and_then(Value::as_bool) != Some(true) {
            return Err(DeepError::Bridge { code: "decompiler_unavailable".into(), message: "this IDA installation has no usable decompiler".into() });
        }
        let version = info.get("provider").and_then(|p| p.get("version")).and_then(Value::as_str).map(String::from);
        let image_base = info.get("target").and_then(|t| t.get("image_base")).and_then(Value::as_str).and_then(parse_hex);
        progress(1.0, "IDA is ready");
        Ok(Session { client: Mutex::new(client), process: Mutex::new(running), runtime_root: scratch.keep(), version, image_base })
    }

    fn lock(&self) -> craft_anything_platform::sync::MutexGuard<'_, Client> {
        self.client.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn request_deadline() -> Instant {
        Instant::now() + REQUEST_TIME
    }
}

impl DeepDecompiler for Session {
    fn engine(&self) -> &'static str {
        ID
    }

    fn engine_version(&self) -> Option<String> {
        self.version.clone()
    }

    fn image_base(&self) -> Option<u64> {
        self.image_base
    }

    fn decompile(&self, entry: u64) -> Result<Option<DeepFunction>, DeepError> {
        let never_cancel = || false;
        let value = self.lock().request(
            "native_function_pseudocode",
            serde_json::json!({"address": format!("{entry:#x}")}),
            &never_cancel,
            Self::request_deadline(),
        )?;
        Ok(decompiled(entry, &value))
    }

    fn list_functions(&self) -> Result<Vec<DeepFunction>, DeepError> {
        let never_cancel = || false;
        let value = self.lock().request("native_functions_list", serde_json::json!({}), &never_cancel, Self::request_deadline())?;
        let mut functions = Vec::new();
        for function in value.get("functions").and_then(Value::as_array).into_iter().flatten() {
            let entry = function.get("address").and_then(Value::as_str).and_then(parse_hex).unwrap_or(0);
            functions.push(DeepFunction {
                entry,
                name: function.get("name").and_then(Value::as_str).unwrap_or_default().to_string(),
                signature: None,
                pseudocode: None,
                created: false,
            });
        }
        Ok(functions)
    }
}

fn decompiled(entry: u64, value: &Value) -> Option<DeepFunction> {
    if value.is_null() {
        return None;
    }
    Some(DeepFunction {
        entry,
        name: value.get("name").and_then(Value::as_str).unwrap_or_default().to_string(),
        signature: value.get("signature").and_then(Value::as_str).map(String::from),
        pseudocode: value.get("pseudocode").and_then(Value::as_str).map(String::from),
        created: value.get("created").and_then(Value::as_bool).unwrap_or(false),
    })
}

fn parse_hex(text: &str) -> Option<u64> {
    let trimmed = text.trim();
    let digits = trimmed.strip_prefix("0x").or_else(|| trimmed.strip_prefix("0X")).unwrap_or(trimmed);
    u64::from_str_radix(digits, 16).ok()
}

fn read_endpoint(path: &Path) -> Option<SocketAddr> {
    let bytes = std::fs::read(path).ok()?;
    let value: Value = serde_json::from_slice(&bytes).ok()?;
    let host: Ipv4Addr = value.get("host").and_then(Value::as_str)?.parse().ok()?;
    let port = u16::try_from(value.get("port").and_then(Value::as_u64)?).ok()?;
    if port == 0 {
        return None;
    }
    Some(SocketAddr::from((host, port)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_lines_keep_paths_with_spaces_in_one_argument() {
        let snapshot = Path::new("/tmp/a target/program");
        let database = Path::new("/tmp/a target/target.i64");
        let log = Path::new("/tmp/a target/ida.log");
        let script = Path::new("/tmp/a target/CraftAnythingBridge.py");
        let args = analyse_args(snapshot, database, log, script);
        assert!(args.contains(&OsString::from("-A")));
        assert!(args.contains(&OsString::from("-o/tmp/a target/target.i64")));
        assert!(args.contains(&OsString::from("-S/tmp/a target/CraftAnythingBridge.py")));
        assert_eq!(args.last(), Some(&snapshot.as_os_str().to_owned()));
        let reopen = bridge_args(database, log, script);
        assert!(!reopen.contains(&OsString::from("-c")));
        assert_eq!(reopen.last(), Some(&database.as_os_str().to_owned()));
    }

    #[test]
    fn a_cache_is_complete_only_with_its_database_and_marker() {
        let temp = tempfile::tempdir().unwrap();
        let cache = project_cache_dir(temp.path(), &"a".repeat(64), &"b".repeat(64));
        std::fs::create_dir_all(&cache).unwrap();
        assert!(!is_analysed(&cache));
        std::fs::write(database_path(&cache), b"database").unwrap();
        assert!(!is_analysed(&cache));
        std::fs::write(cache.join(ANALYSED_MARKER), b"ok").unwrap();
        assert!(is_analysed(&cache));
        assert_ne!(cache, project_cache_dir(temp.path(), &"a".repeat(64), &"c".repeat(64)));
    }

    #[test]
    fn bridge_functions_and_addresses_are_parsed_without_inventing_results() {
        assert!(decompiled(0x1000, &Value::Null).is_none());
        let function = decompiled(
            0x1000,
            &serde_json::json!({"name":"sub_1000","signature":"int sub_1000(void)","pseudocode":"int sub_1000(void) { return 7; }","created":true}),
        )
        .unwrap();
        assert_eq!((function.entry, function.created), (0x1000, true));
        assert_eq!(function.pseudocode.as_deref(), Some("int sub_1000(void) { return 7; }"));
        assert_eq!(parse_hex("0X401000"), Some(0x401000));
        assert_eq!(parse_hex("not an address"), None);
    }
}
