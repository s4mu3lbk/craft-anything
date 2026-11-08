//! The person's installed Grok Build CLI, driven over ACP (`grok agent stdio`). Sign-in stays in
//! `grok login`. Craft Anything checks that an auth file exists and does not read it.

use super::*;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};

/// Grok Build's ACP-only adapter. `acp = false` or an SSH host fails closed: the plain runner
/// would open the interactive TUI, which has no approval channel back to Craft Anything.
pub struct Grok;

impl AgentAdapter for Grok {
    fn ids(&self) -> &'static [&'static str] {
        &["grok"]
    }

    fn args(&self, _cx: &ArgsCx) -> Vec<String> {
        // The native runner refuses `requires_acp` before it asks for arguments.
        Vec::new()
    }

    fn requires_acp(&self) -> bool {
        true
    }

    fn has_model_catalog(&self) -> bool {
        true
    }

    /// Reasoning levels differ by model, so a model change has to open that model again.
    fn model_catalog_depends_on_selection(&self) -> bool {
        true
    }

    fn model_catalog(
        &self,
        config: &BackendConfig,
        options: &BackendOptions,
        directory: Option<&Path>,
        stop: &Stop,
    ) -> Result<Vec<models::AgentModel>, ToolError> {
        let metadata = super::super::acp_model_metadata(config, options, directory, stop)?;
        Ok(catalogue(&metadata, options))
    }

    fn validate_model(&self, config: &BackendConfig, options: &BackendOptions, directory: Option<&Path>, stop: &Stop) -> Result<(), ToolError> {
        stop.check()?;
        self.validate_options(options)?;
        if options.cli_model.is_none() && options.cli_thinking.is_none() {
            return Ok(());
        }
        models::validate(options, &self.model_catalog(config, options, directory, stop)?)
    }

    fn plain_fallback(&self) -> bool {
        false
    }

    fn answers_questions(&self) -> bool {
        true
    }

    fn model_in_use(&self, options: Option<&BackendOptions>) -> Option<String> {
        Some(
            options
                .and_then(|options| options.cli_model.as_deref().map(str::trim).filter(|model| !model.is_empty()).map(str::to_string))
                .unwrap_or_else(|| "Grok Build default".into()),
        )
    }

    fn model_choices(&self, config: &BackendConfig, options: &BackendOptions) -> Result<Vec<String>, ToolError> {
        Ok(self.model_catalog(config, options, None, &Stop::default())?.into_iter().map(|model| model.id).collect())
    }

    /// Reasoning is the shared thinking picker. Grok advertises no context window, and the
    /// Copilot context control would invent Standard and Extended tiers, so it stays off.
    fn supports_model_tuning(&self) -> bool {
        false
    }

    fn check_tool_server(&self, stop: &CancelToken) -> Result<(), ToolError> {
        cli_can_start(craft_anything_platform::self_cli_path().as_deref(), stop)
    }

    /// Extra arguments would land before `agent` and can request blanket approval. Supported
    /// settings are passed after `agent` by [`acp_mode_options`](Self::acp_mode_options).
    fn acp_options(&self, _options: &BackendOptions, _access: Access) -> Vec<String> {
        Vec::new()
    }

    fn validate_options(&self, options: &BackendOptions) -> Result<(), ToolError> {
        if options.args.is_empty() {
            Ok(())
        } else {
            Err(ToolError::invalid("Extra Grok arguments could bypass Craft Anything's approval checks.")
                .with_remediation("Remove this backend's extra arguments. Set its program, model and thinking through the backend options instead."))
        }
    }

    /// `grok agent` accepts no flag that stops native reads or writes, and a read does not ask
    /// permission. A saved permission or an always-approve setting can also edit without
    /// `session/request_permission`. Clean room, deep chat, and questions promise limits this
    /// agent cannot keep, so they are refused. A deep build may read, and an edit is gated only
    /// when Grok sends that request. Nothing here can force the request: the flags do not
    /// constrain `grok agent`, and replacing the person's Grok config would drop their servers.
    fn validate_access(&self, access: Access) -> Result<(), ToolError> {
        match access {
            Access::Build => Ok(()),
            Access::Analyse | Access::CleanRoomAnalyse | Access::CleanRoomBuild | Access::CleanRoomQuestion | Access::Question => {
                Err(ToolError::invalid("Grok Build reads and edits without asking, so a Clean room run, a deep chat, or a question can't hold it back.")
                    .with_remediation("Use a deep build. Grok asks before an edit only when its own permission mode sends a request."))
            }
        }
    }

    fn acp_mode_options(&self, options: &BackendOptions, _access: Access) -> Vec<String> {
        // `--no-leader` keeps this run off the person's shared Grok session, including one
        // already open in a terminal. Effort after `agent` is what the CLI actually applies.
        let mut args = vec!["--no-leader".to_string()];
        if let Some(effort) = chosen_effort(options) {
            args.extend(["--reasoning-effort".into(), effort]);
        }
        args
    }

    fn acp_trailing(&self) -> &'static [&'static str] {
        &["stdio"]
    }

    /// Grok rejects a server object that has no `type`. Other agents do not send this field.
    fn acp_mcp_stdio_type(&self) -> bool {
        true
    }

    fn acp_session_config(&self, options: &BackendOptions) -> Vec<(String, String)> {
        // Model first. Switching model can reset the effort the CLI flag just set, so the
        // effort override is sent again afterwards.
        let mut config = Vec::new();
        if let Some(model) = options.cli_model.as_deref().map(str::trim).filter(|model| !model.is_empty()) {
            config.push(("model".into(), model.to_string()));
        }
        if let Some(effort) = chosen_effort(options) {
            config.push(("reasoning_effort".into(), effort));
        }
        config
    }

    fn sign_in_fix(&self) -> Option<String> {
        Some("Sign in with `grok login` in a terminal, then try again. Grok keeps the sign-in; Craft Anything never sees it.".into())
    }

    fn signed_in(&self, _program: &str) -> Option<bool> {
        let grok_home = std::env::var_os("GROK_HOME");
        // Existence only. The file holds the person's credentials and is not read.
        // Missing is unknown: `XAI_API_KEY` can authenticate with no auth.json, and the run reports it.
        auth_present(auth_path(grok_home.as_deref(), dirs::home_dir().as_deref()))
    }
}

/// `Some(true)` when `auth.json` exists as a file. Missing, or not a file, is unknown (`None`),
/// not "signed out".
fn auth_present(path: Option<PathBuf>) -> Option<bool> {
    path.and_then(|path| path.is_file().then_some(true))
}

/// `GROK_HOME/auth.json` when that variable is set, otherwise `~/.grok/auth.json`.
fn auth_path(grok_home: Option<&OsStr>, user_home: Option<&Path>) -> Option<PathBuf> {
    let dir = match grok_home {
        Some(home) if !home.is_empty() => PathBuf::from(home),
        _ => user_home?.join(".grok"),
    };
    Some(dir.join("auth.json"))
}

/// Thinking depth, in the same order Copilot uses: an explicit picker choice, then a saved
/// named effort, then the older `effort` field.
fn chosen_effort(options: &BackendOptions) -> Option<String> {
    options
        .cli_thinking
        .map(|effort| effort.as_str().to_string())
        .or_else(|| options.cli_reasoning_effort.map(|effort| effort.as_str().to_string()))
        .or_else(|| options.effort.map(|effort| effort.as_str().to_string()))
}

fn catalogue(metadata: &super::super::acp::ModelMetadata, options: &BackendOptions) -> Vec<models::AgentModel> {
    let tuning = crate::model_options::ModelTuning::from_metadata(metadata.current_model.clone(), &metadata.config_options);
    let active = options.cli_model.as_deref().map(str::trim).filter(|model| !model.is_empty()).unwrap_or(metadata.current_model.as_str());
    let thinking: Vec<_> = tuning.reasoning.iter().filter_map(|choice| crate::Effort::parse(choice.value.as_str())).collect();
    let mut ids = metadata.models.clone();
    if !metadata.current_model.is_empty() {
        ids.push(metadata.current_model.clone());
    }
    let mut ordered = Vec::new();
    for id in ids {
        if !id.is_empty() && !ordered.contains(&id) {
            ordered.push(id);
        }
    }
    ordered.into_iter().map(|id| models::AgentModel { name: id.clone(), thinking: if id == active { thinking.clone() } else { Vec::new() }, id }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grok_is_acp_only_and_registered() {
        let adapter = AgentAdapters::built_in().for_agent("grok");
        assert!(adapter.ids().contains(&"grok"));
        assert!(adapter.requires_acp() && !adapter.plain_fallback() && adapter.answers_questions());
        assert!(adapter.has_model_catalog() && adapter.model_catalog_depends_on_selection());
        assert!(!adapter.supports_model_tuning());
        assert_eq!(adapter.acp_trailing(), &["stdio"]);
        assert!(adapter.acp_mcp_stdio_type());
        assert!(!AgentAdapters::built_in().for_agent("maki").acp_mcp_stdio_type());
        assert_eq!(super::super::acp::acp_args("grok"), Some(&["agent"][..]));
        assert_eq!(super::super::acp::model_args("grok", Some("grok-4.5")), ["--model", "grok-4.5"]);
        assert!(super::super::acp::model_args("grok", Some("  ")).is_empty());
    }

    #[test]
    fn mode_flags_follow_agent_and_never_approve_everything() {
        let defaults = BackendOptions::default();
        assert_eq!(Grok.acp_mode_options(&defaults, Access::Question), ["--no-leader"]);
        assert!(Grok.acp_options(&defaults, Access::Build).is_empty());
        assert!(Grok.acp_session_config(&defaults).is_empty());
        let options = BackendOptions {
            cli_model: Some("  grok-4.6  ".into()),
            cli_thinking: Some(crate::Effort::Low),
            cli_reasoning_effort: Some(crate::config::CliReasoningEffort::High),
            effort: Some(crate::Effort::Medium),
            args: vec!["--always-approve".into()],
            ..Default::default()
        };
        // The saved extra arguments are refused and are not copied in front of `agent`.
        assert!(Grok.validate_options(&options).is_err());
        assert!(Grok.acp_options(&options, Access::Build).is_empty());
        assert_eq!(Grok.acp_mode_options(&options, Access::Build), ["--no-leader", "--reasoning-effort", "low"]);
        assert_eq!(Grok.acp_session_config(&options), [("model".into(), "grok-4.6".into()), ("reasoning_effort".into(), "low".into())]);
        assert!(!Grok.acp_mode_options(&options, Access::CleanRoomBuild).iter().any(|arg| arg.contains("always-approve")));
        for access in [Access::Analyse, Access::CleanRoomAnalyse, Access::CleanRoomBuild, Access::CleanRoomQuestion, Access::Question] {
            let error = Grok.validate_access(access).expect_err("clean room, deep chat, and questions are refused");
            assert!(error.message.contains("Clean room") && error.message.contains("deep chat") && error.message.contains("question"), "{}", error.message);
            assert!(error.remediation.as_deref().is_some_and(|fix| fix.contains("deep build")));
        }
        assert!(Grok.validate_access(Access::Build).is_ok());
        let named =
            BackendOptions { cli_reasoning_effort: Some(crate::config::CliReasoningEffort::Xhigh), effort: Some(crate::Effort::Low), ..Default::default() };
        assert_eq!(Grok.acp_session_config(&named), [("reasoning_effort".into(), "xhigh".into())]);
        assert_eq!(Grok.model_in_use(None).as_deref(), Some("Grok Build default"));
        assert_eq!(Grok.model_in_use(Some(&options)).as_deref(), Some("grok-4.6"));
        let fix = Grok.sign_in_fix().unwrap();
        assert!(fix.contains("grok login") && fix.contains("never sees"));
    }

    #[test]
    fn catalogue_offers_only_the_selected_models_advertised_thinking() {
        let metadata = super::super::acp::ModelMetadata {
            models: vec!["grok-4.7".into(), "grok-4.5".into()],
            current_model: "grok-4.7".into(),
            config_options: vec![json!({"id": "reasoning_effort", "options": [
                {"value": "low", "name": "Low"},
                {"value": "medium", "name": "Medium"},
                {"value": "high", "name": "High"},
                {"value": "xhigh", "name": "High"},
                {"value": "unsupported", "name": "Unknown"}
            ]})],
        };
        let mut options = BackendOptions { cli_model: Some("grok-4.5".into()), ..Default::default() };
        let models = catalogue(&metadata, &options);
        assert!(models.iter().all(|model| model.id != "auto"));
        assert_eq!(models.iter().map(|model| model.id.as_str()).collect::<Vec<_>>(), ["grok-4.7", "grok-4.5"]);
        assert!(models.iter().find(|model| model.id == "grok-4.7").unwrap().thinking.is_empty());
        options.cli_model = Some("grok-4.7".into());
        let models = catalogue(&metadata, &options);
        assert_eq!(
            models.iter().find(|model| model.id == "grok-4.7").unwrap().thinking,
            [crate::Effort::Low, crate::Effort::Medium, crate::Effort::High, crate::Effort::Xhigh]
        );
        options.cli_thinking = Some(crate::Effort::Xhigh);
        assert!(models::validate(&options, &models).is_ok());
        options.cli_thinking = Some(crate::Effort::Max);
        assert!(models::validate(&options, &models).is_err());
    }

    #[test]
    fn auth_path_uses_grok_home_and_does_not_need_the_real_one() {
        let custom = PathBuf::from("/tmp/grok-home");
        assert_eq!(auth_path(Some(custom.as_os_str()), None), Some(custom.join("auth.json")));
        let user = PathBuf::from("/home/u");
        assert_eq!(auth_path(Some(OsStr::new("")), Some(&user)), Some(user.join(".grok").join("auth.json")));
        assert_eq!(auth_path(None, Some(&user)), Some(user.join(".grok").join("auth.json")));
        assert_eq!(auth_path(None, None), None);
    }

    #[test]
    fn a_missing_auth_file_is_unknown_and_an_existing_file_is_signed_in() {
        let root = std::env::temp_dir().join(format!("craft-grok-auth-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let file = root.join("auth.json");
        // Not a credential. The check is existence only, and this file is empty.
        assert_eq!(auth_present(Some(file.clone())), None);
        std::fs::write(&file, b"").unwrap();
        assert_eq!(auth_present(Some(file.clone())), Some(true));
        std::fs::remove_file(&file).unwrap();
        std::fs::create_dir(&file).unwrap();
        assert_eq!(auth_present(Some(file)), None);
        assert_eq!(auth_present(None), None);
        let _ = std::fs::remove_dir_all(&root);
    }

    /// One real deep build: our MCP server is called, and one permission request is answered.
    /// Grok's home for this process is a temporary ask-mode config, so a shell write asks even
    /// when the person's own config would approve every tool. That config is not modified.
    /// Not part of the default suite.
    #[test]
    #[ignore = "live Grok Build session; needs grok signed in and craft-anything-cli built"]
    #[cfg(unix)]
    fn a_live_build_connects_tools_and_answers_one_permission() {
        use std::os::unix::fs::PermissionsExt;
        let pid = std::process::id();
        let root = std::env::temp_dir().join(format!("craft-grok-live-{pid}"));
        let home = std::env::temp_dir().join(format!("craft-grok-home-{pid}"));
        let _keep_root = RemoveOnDrop(root.clone());
        let _keep_home = RemoveOnDrop(home.clone());
        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::remove_dir_all(&home);
        std::fs::create_dir_all(root.join("inbox")).unwrap();
        std::fs::create_dir_all(&home).unwrap();
        std::fs::write(root.join("readme.txt"), "live check\n").unwrap();
        // Ask, and no personal MCP servers. `ui.permission_mode` is what the CLI reads.
        std::fs::write(
            home.join("config.toml"),
            "[ui]\npermission_mode = \"ask\"\nyolo = false\n\n[auto_mode]\nenabled = false\n\ndefault_auto_mode = false\ndisable_web_search = true\n\n[cli]\nauto_update = false\nuse_leader = false\n",
        )
        .unwrap();
        let user_home = dirs::home_dir().expect("a home directory");
        let auth = user_home.join(".grok").join("auth.json");
        assert!(auth.is_file(), "this live check needs an existing grok auth file; it is not read");
        std::os::unix::fs::symlink(&auth, home.join("auth.json")).unwrap();
        let listed = std::process::Command::new("sh").arg("-c").arg("command -v grok").output().expect("look up grok");
        let found = String::from_utf8_lossy(&listed.stdout).trim().to_string();
        let grok_bin = if found.is_empty() { user_home.join(".grok").join("bin").join("grok") } else { PathBuf::from(found) };
        assert!(grok_bin.is_file(), "grok has to be installed for this live check");
        let wrapper = root.join("grok-ask");
        let script = format!("#!/bin/sh\nexport GROK_HOME='{}'\nexec '{}' \"$@\"\n", home.display(), grok_bin.display());
        std::fs::write(&wrapper, script).unwrap();
        std::fs::set_permissions(&wrapper, std::fs::Permissions::from_mode(0o755)).unwrap();
        let _ = std::process::Command::new("git").arg("-C").arg(&root).arg("init").output();

        let permissions = std::sync::Arc::new(craft_anything_platform::sync::Mutex::new(Vec::<String>::new()));
        let record = permissions.clone();
        let approver: super::super::acp::Approver = std::sync::Arc::new(move |tool: &str, _input: &serde_json::Value| {
            record.lock().unwrap().push(tool.to_string());
            true
        });
        let mut spec = super::super::CliSpec::new(
            crate::BackendConfig {
                name: "Grok".into(),
                kind: crate::BackendKind::AgentCli,
                model: "grok".into(),
                base_url: None,
                ssh_host: None,
                enabled: true,
            },
            BackendOptions { command: Some(wrapper.display().to_string()), ..Default::default() },
            false,
            Some(root.clone()),
        );
        // Deep build: Clean room is refused, because Grok's native reads never ask.
        spec.deep = true;
        spec.builder = Some(super::super::Builder { investigation: Some(root.clone()), inbox: root.join("inbox") });
        let mut conv = super::super::builder_conversation(spec, approver);
        conv.send(
            "Do exactly these two things in this folder, then stop. First call the craft-anything MCP tool core_session_show and no other MCP server. Then run this shell command and no other command: mkdir grok-live-marker",
        );

        let started = std::time::Instant::now();
        let mut tools = Vec::new();
        let mut notes = Vec::new();
        let mut errors = Vec::new();
        let mut done = false;
        let mut saw_server = false;
        while started.elapsed() < std::time::Duration::from_secs(180) {
            for event in conv.poll() {
                match event {
                    crate::AgentEvent::ToolStarted { tool, input, .. } => {
                        let blob = format!("{tool} {input}");
                        let lower = blob.to_ascii_lowercase();
                        if lower.contains("session_show") || lower.contains("craft-anything") || lower.contains("craft_anything") {
                            saw_server = true;
                        }
                        let mut note = blob.chars().take(160).collect::<String>();
                        note = note.replace(&user_home.display().to_string(), "~");
                        notes.push(note);
                        tools.push(tool);
                    }
                    crate::AgentEvent::ToolFinished { tool, .. } => tools.push(tool),
                    crate::AgentEvent::Error { message, .. } => errors.push(message),
                    crate::AgentEvent::TurnDone { .. } => done = true,
                    crate::AgentEvent::ConsentNeeded { call_id, tool, .. } => {
                        permissions.lock().unwrap().push(tool);
                        conv.answer_consent(&call_id, true);
                    }
                    _ => {}
                }
            }
            if done && !conv.is_busy() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        conv.stop();
        let asked = permissions.lock().unwrap().clone();
        assert!(done, "the turn did not finish; tools={tools:?} permissions={asked:?} notes={notes:?} errors={errors:?}");
        assert!(!asked.is_empty(), "no permission was answered; tools={tools:?} notes={notes:?} errors={errors:?}");
        assert!(saw_server, "craft-anything's tools did not run; tools={tools:?} permissions={asked:?} notes={notes:?} errors={errors:?}");
    }

    #[cfg(unix)]
    struct RemoveOnDrop(PathBuf);
    #[cfg(unix)]
    impl Drop for RemoveOnDrop {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
}
