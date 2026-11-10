//! Aider (`aider --message`).
//!
//! Aider reads positional arguments as files to edit, so the message goes to `--message`. The
//! plain adapter passed it as a file name and every run stopped at once (#72).
//!
//! A one-shot run still asks questions and can open pages before the model is reached: it offers
//! to make a git repository and to add `.aider*` to `.gitignore`, checks for a new version, shows
//! release notes, asks about analytics and Playwright, and offers documentation URLs for model
//! warnings or a missing key. The flags below turn each of those off, and `BROWSER` is pointed at
//! a command that does nothing, so no page can open from a run. `--yes-always` is deliberately
//! not one of them: answering every confirmation with "yes" would also accept a page that the
//! person should decide on, and the confirmations a build needs already default to "yes" when
//! nobody can answer them.
//!
//! Before a run, [`check_setup`] refuses the two cases where Aider would wait five minutes for a
//! sign-in that nobody is there to finish: no model and no API key at all, and an OpenRouter model
//! with no OpenRouter key (#72).
//!
//! Outside a build the message may only read, so `--dry-run` stops Aider writing files. Its own
//! reads can't be checked, so a Clean room run is refused in `validate_access`, as for Grok (R2.2).

use super::*;
use std::path::{Path, PathBuf};

/// What stops Aider asking, updating, or offering to open a page.
const QUIET: &[&str] = &[
    "--no-check-update",
    "--no-show-release-notes",
    "--no-pretty",
    "--no-fancy-input",
    "--no-git",
    "--no-gitignore",
    "--no-analytics",
    "--no-detect-urls",
    "--no-suggest-shell-commands",
    "--no-show-model-warnings",
    "--disable-playwright",
];

/// The keys Aider's own default-model picker looks for when no model is chosen.
const DEFAULT_MODEL_KEYS: &[&str] = &["OPENROUTER_API_KEY", "ANTHROPIC_API_KEY", "DEEPSEEK_API_KEY", "OPENAI_API_KEY", "GEMINI_API_KEY", "VERTEXAI_PROJECT"];

/// The key file Aider's own OpenRouter sign-in writes, in the home folder.
const SIGN_IN_KEYS: &str = ".aider/oauth-keys.env";

/// The most of a settings file we read while checking.
const SETTINGS_FILE_CAP: u64 = 1024 * 1024;

/// Aider's model shortcut flags; each one chooses the model, so none needs `--model`.
const MODEL_SHORTCUTS: &[&str] = &[
    "--opus",
    "--sonnet",
    "--haiku",
    "--4",
    "-4",
    "--4o",
    "--mini",
    "--4-turbo",
    "--35turbo",
    "--35-turbo",
    "--3",
    "-3",
    "--deepseek",
    "--o1-mini",
    "--o1-preview",
];

pub struct Aider;

impl AgentAdapter for Aider {
    fn ids(&self) -> &'static [&'static str] {
        &["aider"]
    }

    fn args(&self, cx: &ArgsCx) -> Vec<String> {
        let mut a = Vec::new();
        if let Some(m) = &cx.options.cli_model {
            a.extend(["--model".into(), m.clone()]);
        }
        a.extend(QUIET.iter().copied().map(str::to_string));
        if !cx.builder {
            a.push("--dry-run".into());
        }
        a.extend(cx.options.args.iter().cloned());
        // Last, so a saved extra argument can't be mistaken for the message.
        a.extend(["--message".into(), format!("{}{}", cx.preamble, cx.prompt)]);
        a
    }

    /// The message flag and the two that keep a run from asking about git or the console. Aider's
    /// plain form reads the prompt as a file name, so a failed check keeps this adapter rather
    /// than falling back to the plain one (#72).
    fn required_flags(&self) -> (&'static [&'static str], &'static [&'static str]) {
        (&["--help"], &["--message", "--no-fancy-input", "--no-git"])
    }

    fn plain_fallback(&self) -> bool {
        false
    }
    /// Aider reads whatever files it likes and never asks us, so a Clean room run can't hold it
    /// back. Grok is refused for the same reason (R2.2).
    fn validate_access(&self, access: Access) -> Result<(), ToolError> {
        match access {
            Access::CleanRoomAnalyse | Access::CleanRoomBuild | Access::CleanRoomQuestion => {
                Err(ToolError::invalid("Aider reads files on its own without asking, so a Clean room run can't hold it back.")
                    .with_remediation("Use a deep build for Aider. Its reads aren't checked, so it can't be kept out of the original's files."))
            }
            _ => Ok(()),
        }
    }

    fn streams_json(&self) -> bool {
        false
    }

    /// Refuse before Aider starts when it would stop at a sign-in nobody can finish (#72).
    fn check_native_options(&self, options: &BackendOptions, cwd: Option<&Path>) -> Result<(), ToolError> {
        check_setup(
            options,
            &|name| std::env::var(name).ok(),
            &Files::for_run(cwd, dirs::home_dir().as_deref(), flag_value(&options.args, "--env-file").as_deref()),
        )
    }

    /// Nothing Aider offers to open is worth a page during a run, offline or not.
    fn process(&self, program: &str) -> Process {
        Process::new(program).env("BROWSER", "true")
    }
}

/// The settings files Aider reads for a run, each list with the file it loads last (whose
/// settings win) first.
struct Files {
    env: Vec<PathBuf>,
    conf: Vec<PathBuf>,
}

impl Files {
    /// Aider looks in the folder it runs in, the top of its git repository (found from there even
    /// with `--no-git`), then the home folder. Its saved OpenRouter key loads before any `.env`.
    /// The folder is taken as Aider sees it, links resolved, so the repository is the same one.
    /// An `--env-file` in its arguments loads last of all, relative to that folder.
    fn for_run(cwd: Option<&Path>, home: Option<&Path>, env_file: Option<&str>) -> Files {
        let cwd = cwd.and_then(|d| d.canonicalize().or_else(|_| std::path::absolute(d)).ok());
        let git_top = cwd.as_deref().and_then(|d| d.ancestors().find(|a| is_repository_top(a)));
        let mut folders: Vec<&Path> = Vec::new();
        for folder in [cwd.as_deref(), git_top, home].into_iter().flatten() {
            if !folders.contains(&folder) {
                folders.push(folder);
            }
        }
        let named = env_file.map(Path::new).and_then(|f| if f.is_absolute() { Some(f.to_path_buf()) } else { cwd.as_deref().map(|c| c.join(f)) });
        let mut env: Vec<PathBuf> = named.into_iter().chain(folders.iter().map(|f| f.join(".env"))).collect();
        env.extend(home.map(|h| h.join(SIGN_IN_KEYS)));
        Files { conf: folders.iter().map(|f| f.join(".aider.conf.yml")).collect(), env }
    }
}

/// Whether `dir` is a git repository's top as GitPython finds it: a `.git` folder that holds a
/// `HEAD`, or a worktree's `.git` file.
fn is_repository_top(dir: &Path) -> bool {
    let git = dir.join(".git");
    git.is_file() || git.join("HEAD").is_file()
}

/// The model Aider will run: ours if chosen, else one its own arguments, environment or config
/// name. `None` means Aider would pick one from whichever key it finds, or offer a sign-in. A
/// `.env` comes before the environment, as Aider loads its `.env` files over it.
fn configured_model(options: &BackendOptions, env: &dyn Fn(&str) -> Option<String>, files: &Files) -> Option<String> {
    model_in_args(&options.args)
        .or_else(|| options.args.iter().find(|a| MODEL_SHORTCUTS.contains(&a.as_str())).cloned())
        .or_else(|| options.cli_model.as_deref().map(str::trim).filter(|m| !m.is_empty()).map(str::to_string))
        .or_else(|| files.env.iter().find_map(|file| env_file_value(&read_small(file)?, "AIDER_MODEL")))
        .or_else(|| env("AIDER_MODEL").map(|m| m.trim().to_string()).filter(|m| !m.is_empty()))
        .or_else(|| files.conf.iter().find_map(|file| model_in_conf(&read_small(file)?)))
}

/// The last `--model` in the extra arguments, since Aider's parser keeps the last one.
fn model_in_args(args: &[String]) -> Option<String> {
    flag_value(args, "--model")
}

/// The last value of `flag` in the extra arguments, as `flag value` or `flag=value`.
fn flag_value(args: &[String], flag: &str) -> Option<String> {
    let joined = format!("{flag}=");
    let mut found = None;
    let mut rest = args.iter();
    while let Some(arg) = rest.next() {
        let value = if arg == flag { rest.next().map(String::as_str) } else { arg.strip_prefix(joined.as_str()) };
        if let Some(value) = value {
            let value = value.trim();
            found = (!value.is_empty()).then(|| value.to_string());
        }
    }
    found
}

/// A value without a trailing ` # comment`. Quoted values are left alone.
fn strip_comment(value: &str) -> &str {
    let value = value.trim();
    if value.starts_with(['"', '\'']) {
        return value;
    }
    if value.starts_with('#') {
        return "";
    }
    value.split_once(" #").map_or(value, |(v, _)| v).trim()
}

/// A top-level `model:` in Aider's own config file.
fn model_in_conf(text: &str) -> Option<String> {
    text.lines().find_map(|line| {
        let value = strip_comment(line.strip_prefix("model:")?).trim_matches(['"', '\'']);
        (!value.is_empty()).then(|| value.to_string())
    })
}

/// Whether `name` is set by Aider's own `--api-key provider=KEY` or `--set-env NAME=VALUE`.
fn key_in_args(args: &[String], name: &str) -> bool {
    args.iter().enumerate().any(|(i, arg)| {
        let (flag, value) = if let Some(v) = arg.strip_prefix("--api-key=") {
            ("--api-key", v)
        } else if let Some(v) = arg.strip_prefix("--set-env=") {
            ("--set-env", v)
        } else if arg == "--api-key" || arg == "--set-env" {
            (arg.as_str(), args.get(i + 1).map(String::as_str).unwrap_or_default())
        } else {
            return false;
        };
        let Some((key, value)) = value.split_once('=') else { return false };
        let key = if flag == "--api-key" { format!("{}_API_KEY", key.trim().to_ascii_uppercase()) } else { key.trim().to_string() };
        key == name && !value.trim().is_empty()
    })
}

/// Whether a key is set in the environment (bare or `AIDER_`-prefixed), in Aider's arguments,
/// or in one of its key or config files.
fn key_seen(name: &str, options: &BackendOptions, env: &dyn Fn(&str) -> Option<String>, files: &Files) -> bool {
    let prefixed = format!("AIDER_{name}");
    let names = [name, prefixed.as_str()];
    if names.iter().any(|n| env(n).is_some_and(|v| !v.trim().is_empty())) || key_in_args(&options.args, name) {
        return true;
    }
    files.env.iter().any(|file| read_small(file).is_some_and(|text| names.iter().any(|n| env_file_value(&text, n).is_some())))
        || files.conf.iter().any(|file| read_small(file).is_some_and(|text| conf_sets_key(&text, name)))
}

/// The value of `NAME=value` in a dotenv file, with or without `export` and around quotes.
fn env_file_value(text: &str, name: &str) -> Option<String> {
    text.lines().find_map(|line| {
        let line = line.trim();
        let line = line.strip_prefix("export ").unwrap_or(line);
        let (key, value) = line.split_once('=')?;
        let value = strip_comment(value).trim_matches(['"', '\'']);
        (key.trim() == name && !value.is_empty()).then(|| value.to_string())
    })
}

/// A `provider=KEY` or `NAME=VALUE` list item in Aider's config (`api-key:` / `set-env:`).
fn conf_sets_key(text: &str, name: &str) -> bool {
    let provider = name.strip_suffix("_API_KEY").unwrap_or(name).to_ascii_lowercase();
    text.lines().any(|line| {
        let line = line.trim().trim_start_matches('-').trim().trim_matches(['"', '\'']);
        [name, provider.as_str()].iter().any(|n| line.strip_prefix(*n).and_then(|rest| rest.strip_prefix('=')).is_some_and(|v| !v.trim().is_empty()))
    })
}

/// A file only if it's small enough to read while checking, so a huge one can't stall a run.
fn read_small(path: &Path) -> Option<String> {
    let meta = std::fs::metadata(path).ok()?;
    (meta.is_file() && meta.len() <= SETTINGS_FILE_CAP).then(|| std::fs::read_to_string(path).ok()).flatten()
}

/// Refuse when Aider would offer its OpenRouter sign-in, which opens a page and waits five minutes
/// for an answer nobody can give. Otherwise a run is left to Aider's own error for that key.
fn check_setup(options: &BackendOptions, env: &dyn Fn(&str) -> Option<String>, files: &Files) -> Result<(), ToolError> {
    match configured_model(options, env, files) {
        None if DEFAULT_MODEL_KEYS.iter().any(|key| key_seen(key, options, env, files)) => Ok(()),
        None => Err(ToolError::new(
            ErrorCode::BadSettings,
            "Aider has no model or API key set up, so it would offer to sign in to OpenRouter and wait for a browser nobody is at.",
        )
        .with_remediation(
                        "Set OPENROUTER_API_KEY, ANTHROPIC_API_KEY or OPENAI_API_KEY for the computer Craft Anything runs on, or name a model in Aider's own ~/.aider.conf.yml. Aider's ~/.env counts too.",
        )),
        Some(model) if model.starts_with("openrouter/") && !key_seen("OPENROUTER_API_KEY", options, env, files) => Err(ToolError::new(
            ErrorCode::BadSettings,
            format!("Aider's model {model} needs an OpenRouter API key, and without one it would offer to sign in to OpenRouter and wait for a browser nobody is at."),
        )
        .with_remediation("Set OPENROUTER_API_KEY (in the environment, ~/.env or ~/.aider/oauth-keys.env), or choose a model from another provider for Aider.")),
        Some(_) => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(builder: bool, options: &BackendOptions) -> Vec<String> {
        let mcp = McpCommand { command: "craft-anything-cli".into(), args: vec!["mcp".into()], notes: None };
        Aider.args(&ArgsCx { options, prompt: "make a game", preamble: "PRE ", mcp: &mcp, resume: None, builder, reads_checked: false, question: false })
    }

    /// A check with no keys in the environment and no home folder, as on a fresh computer.
    fn no_keys(options: &BackendOptions) -> Result<(), ToolError> {
        check_setup(options, &|_| None, &at_home(None))
    }

    /// The files of a run outside any project, with this home folder.
    fn at_home(home: Option<&Path>) -> Files {
        Files::for_run(None, home, None)
    }

    #[test]
    fn the_message_goes_to_message_after_the_quiet_flags() {
        let a = args(true, &BackendOptions::default());
        assert_eq!(&a[a.len() - 2..], ["--message", "PRE make a game"]);
        for flag in ["--no-check-update", "--no-show-release-notes", "--no-git", "--no-gitignore", "--no-analytics", "--no-fancy-input"] {
            assert!(a.contains(&flag.to_string()), "{flag} is missing: {a:?}");
        }
        assert!(!a.iter().any(|x| x == "--yes-always"), "yes-always would also accept a sign-in: {a:?}");
        assert!(!a.contains(&"--dry-run".to_string()), "a build may write: {a:?}");
    }

    #[test]
    fn a_chat_without_a_build_cannot_write() {
        assert!(args(false, &BackendOptions::default()).contains(&"--dry-run".to_string()));
    }

    #[test]
    fn a_saved_model_and_extra_args_come_before_the_message() {
        let options = BackendOptions { cli_model: Some("openrouter/anthropic/claude-sonnet-4".into()), args: vec!["--architect".into()], ..Default::default() };
        let a = args(true, &options);
        assert_eq!(&a[..2], ["--model", "openrouter/anthropic/claude-sonnet-4"]);
        assert!(a.contains(&"--architect".to_string()));
        assert_eq!(&a[a.len() - 2..], ["--message", "PRE make a game"]);
    }

    #[test]
    fn aider_is_registered_and_never_run_plain() {
        assert!(AgentAdapters::built_in().for_agent("aider").ids().contains(&"aider"), "registered, not the plain adapter");
        assert_eq!(Aider.required_flags(), (&["--help"][..], &["--message", "--no-fancy-input", "--no-git"][..]));
        assert!(!Aider.plain_fallback() && !Aider.streams_json());
    }

    #[test]
    fn a_run_with_no_model_and_no_key_is_refused_before_aider_offers_a_sign_in() {
        let err = no_keys(&BackendOptions::default()).unwrap_err();
        assert_eq!(err.code, ErrorCode::BadSettings);
        assert!(err.message.contains("sign in to OpenRouter"), "{}", err.message);
        let fix = err.remediation.unwrap_or_default();
        assert!(fix.contains("~/.aider.conf.yml") && fix.contains("OPENAI_API_KEY"), "{fix}");
    }

    #[test]
    fn a_chosen_model_runs_and_a_missing_key_is_aiders_own_error() {
        let options = BackendOptions { cli_model: Some("anthropic/claude-sonnet-4".into()), ..Default::default() };
        assert!(no_keys(&options).is_ok(), "a model is enough to start; Aider reports a missing key plainly");
    }

    #[test]
    fn a_key_alone_lets_aider_pick_its_own_model() {
        let env = |name: &str| (name == "OPENAI_API_KEY").then(|| "sk-test".to_string());
        assert!(check_setup(&BackendOptions::default(), &env, &at_home(None)).is_ok());
    }

    #[test]
    fn an_openrouter_model_needs_an_openrouter_key() {
        let options = BackendOptions { cli_model: Some("openrouter/anthropic/claude-sonnet-4".into()), ..Default::default() };
        let err = no_keys(&options).unwrap_err();
        assert!(err.message.contains("OpenRouter API key"), "{}", err.message);
        assert!(err.remediation.unwrap_or_default().contains("OPENROUTER_API_KEY"));
        let env = |name: &str| (name == "OPENROUTER_API_KEY").then(|| "sk-test".to_string());
        assert!(check_setup(&options, &env, &at_home(None)).is_ok());
    }

    #[test]
    fn a_model_and_key_in_aiders_own_files_count() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(".aider.conf.yml"), "# my settings\nmodel: gemini/gemini-2.5-pro\nauto-commits: false\n").unwrap();
        assert!(check_setup(&BackendOptions::default(), &|_| None, &at_home(Some(dir.path()))).is_ok(), "a model in Aider's config is a model");

        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".aider")).unwrap();
        std::fs::write(dir.path().join(".aider/oauth-keys.env"), "OPENROUTER_API_KEY=\"sk-test\"\n").unwrap();
        let options = BackendOptions { cli_model: Some("openrouter/anthropic/claude-sonnet-4".into()), ..Default::default() };
        assert!(check_setup(&options, &|_| None, &at_home(Some(dir.path()))).is_ok(), "Aider's saved sign-in key counts");
    }

    #[test]
    fn a_key_passed_in_aiders_own_arguments_counts() {
        let options = BackendOptions { args: vec!["--api-key".into(), "openrouter=sk-test".into()], ..Default::default() };
        assert!(key_in_args(&options.args, "OPENROUTER_API_KEY"));
        assert!(check_setup(&options, &|_| None, &at_home(None)).is_ok(), "a key in the arguments is enough for Aider to pick a model");
        let options = BackendOptions { args: vec!["--model=anthropic/claude-sonnet-4".into()], ..Default::default() };
        assert_eq!(model_in_args(&options.args).as_deref(), Some("anthropic/claude-sonnet-4"));
        assert!(check_setup(&options, &|_| None, &at_home(None)).is_ok());
    }

    #[test]
    fn a_model_shortcut_flag_counts_as_a_model() {
        let options = BackendOptions { args: vec!["--sonnet".into()], ..Default::default() };
        assert!(no_keys(&options).is_ok(), "a shortcut flag chooses the model");
    }

    #[test]
    fn an_aider_prefixed_variable_counts_as_a_key() {
        let env = |name: &str| (name == "AIDER_OPENAI_API_KEY").then(|| "sk-test".to_string());
        assert!(check_setup(&BackendOptions::default(), &env, &at_home(None)).is_ok());
    }

    #[test]
    fn a_model_or_key_in_the_home_dotenv_or_config_list_counts() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(".env"), "AIDER_MODEL=gpt-4o\n").unwrap();
        assert!(check_setup(&BackendOptions::default(), &|_| None, &at_home(Some(dir.path()))).is_ok());

        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(".aider.conf.yml"), "api-key:\n  - openai=sk-test\n").unwrap();
        assert!(check_setup(&BackendOptions::default(), &|_| None, &at_home(Some(dir.path()))).is_ok());
    }

    /// A project's own settings count, where Aider itself reads them: the folder it runs in and the
    /// top of its git repository.
    #[test]
    fn a_model_or_key_in_the_projects_own_files_counts() {
        let (home, project) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        let app = project.path().join("app");
        std::fs::create_dir_all(&app).unwrap();
        let check = |options: &BackendOptions| check_setup(options, &|_| None, &Files::for_run(Some(&app), Some(home.path()), None));
        assert!(check(&BackendOptions::default()).is_err(), "nothing set up anywhere");
        std::fs::write(app.join(".aider.conf.yml"), "model: gpt-4o\n").unwrap();
        assert!(check(&BackendOptions::default()).is_ok(), "a model in the run's folder");

        let openrouter = BackendOptions { cli_model: Some("openrouter/anthropic/claude-sonnet-4".into()), ..Default::default() };
        assert!(check(&openrouter).is_err());
        std::fs::write(project.path().join(".env"), "OPENROUTER_API_KEY=sk-test\n").unwrap();
        assert!(check(&openrouter).is_err(), "not a git repository, so Aider doesn't read its top");
        std::fs::create_dir(project.path().join(".git")).unwrap();
        assert!(check(&openrouter).is_err(), "an empty .git folder isn't a repository to GitPython");
        std::fs::write(project.path().join(".git/HEAD"), "ref: refs/heads/main\n").unwrap();
        assert!(check(&openrouter).is_ok(), "a key at the top of the git repository");
        std::fs::remove_dir_all(project.path().join(".git")).unwrap();
        std::fs::write(project.path().join(".git"), "gitdir: /elsewhere/.git/worktrees/app\n").unwrap();
        assert!(check(&openrouter).is_ok(), "a worktree's .git file marks its top too");
        assert!(check_setup(&openrouter, &|_| None, &at_home(Some(home.path()))).is_err(), "a run without a folder reads only the home folder");
    }

    /// The file Aider loads last decides the model: the run's folder over the home folder.
    #[test]
    fn the_projects_model_wins_over_the_home_folders() {
        let (home, app) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        let check = || check_setup(&BackendOptions::default(), &|_| None, &Files::for_run(Some(app.path()), Some(home.path()), None));
        std::fs::write(home.path().join(".aider.conf.yml"), "model: openrouter/anthropic/claude-sonnet-4\n").unwrap();
        std::fs::write(app.path().join(".aider.conf.yml"), "model: gpt-4o\n").unwrap();
        assert!(check().is_ok(), "the project's model needs no OpenRouter key");
        std::fs::write(home.path().join(".aider.conf.yml"), "model: gpt-4o\n").unwrap();
        std::fs::write(app.path().join(".aider.conf.yml"), "model: openrouter/anthropic/claude-sonnet-4\n").unwrap();
        assert!(check().unwrap_err().message.contains("OpenRouter API key"), "the project's OpenRouter model needs its key");
    }

    /// Aider loads its `.env` files over the environment, so a model there wins over one set
    /// outside it.
    #[test]
    fn a_dotenv_model_wins_over_the_environments() {
        let app = tempfile::tempdir().unwrap();
        std::fs::write(app.path().join(".env"), "AIDER_MODEL=openrouter/anthropic/claude-sonnet-4\n").unwrap();
        let env = |name: &str| match name {
            "AIDER_MODEL" => Some("gpt-4o".to_string()),
            "OPENAI_API_KEY" => Some("sk-test".to_string()),
            _ => None,
        };
        let err = check_setup(&BackendOptions::default(), &env, &Files::for_run(Some(app.path()), None, None)).unwrap_err();
        assert!(err.message.contains("OpenRouter API key"), "{}", err.message);
    }

    /// An `--env-file` in Aider's arguments loads after every default `.env`, so it wins.
    #[test]
    fn a_named_env_file_wins_over_the_default_ones() {
        let app = tempfile::tempdir().unwrap();
        std::fs::write(app.path().join(".env"), "AIDER_MODEL=gpt-4o\n").unwrap();
        std::fs::write(app.path().join(".env.aider"), "AIDER_MODEL=openrouter/anthropic/claude-sonnet-4\n").unwrap();
        let options = BackendOptions { args: vec!["--env-file".into(), ".env.aider".into()], ..Default::default() };
        let files = Files::for_run(Some(app.path()), None, flag_value(&options.args, "--env-file").as_deref());
        assert!(check_setup(&options, &|_| None, &files).unwrap_err().message.contains("OpenRouter API key"));
        assert!(check_setup(&options, &|_| None, &Files::for_run(Some(app.path()), None, None)).is_ok(), "without it the project's .env decides");
    }

    /// A project reached through a link is read where it really is, with its real repository.
    #[cfg(unix)]
    #[test]
    fn a_linked_project_finds_its_real_repository() {
        let (real, links) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        let app = real.path().join("app");
        std::fs::create_dir_all(real.path().join(".git")).unwrap();
        std::fs::write(real.path().join(".git/HEAD"), "ref: refs/heads/main\n").unwrap();
        std::fs::create_dir(&app).unwrap();
        std::fs::write(real.path().join(".aider.conf.yml"), "model: gpt-4o\n").unwrap();
        std::os::unix::fs::symlink(&app, links.path().join("app")).unwrap();
        assert!(check_setup(&BackendOptions::default(), &|_| None, &Files::for_run(Some(&links.path().join("app")), None, None)).is_ok());
    }

    #[test]
    fn a_clean_room_run_is_refused_and_other_modes_are_not() {
        for access in [Access::CleanRoomAnalyse, Access::CleanRoomBuild, Access::CleanRoomQuestion] {
            let err = Aider.validate_access(access).expect_err("a clean room can't hold Aider back");
            assert!(err.remediation.unwrap_or_default().contains("deep build"));
        }
        for access in [Access::Build, Access::Analyse, Access::Question] {
            assert!(Aider.validate_access(access).is_ok());
        }
    }

    #[test]
    fn an_inline_comment_is_not_part_of_a_value() {
        assert_eq!(model_in_conf("model: gpt-4o # my favourite\n").as_deref(), Some("gpt-4o"));
        assert_eq!(model_in_conf("model: # none yet\n"), None);
        assert_eq!(env_file_value("AIDER_MODEL=gpt-4o # note\n", "AIDER_MODEL").as_deref(), Some("gpt-4o"));
        assert_eq!(env_file_value("AIDER_MODEL= # unset\n", "AIDER_MODEL"), None);
    }
}
