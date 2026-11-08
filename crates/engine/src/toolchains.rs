//! Shared project toolchains for build checks, Play and screenshot/self-test runs.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// How a project builds and tests itself, whatever it's written in.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Deserialize)]
pub struct Toolchain {
    pub name: String,
    /// Files that name it: exact relative paths or a root filename suffix such as `*.csproj`.
    pub found_by: Vec<String>,
    /// The program that builds it and its arguments, run in the project's folder.
    #[serde(default)]
    pub build: Vec<String>,
    /// The same for its tests.
    #[serde(default)]
    pub test: Vec<String>,
    /// What its output says when no tests ran, which isn't a failure.
    #[serde(default)]
    pub no_tests: Vec<String>,
    /// The package.json script that is its tests. Whether there are any is then read from that
    /// file, as what a script prints can be about one it runs, and `no_tests` are whole scripts
    /// that are only placeholders.
    #[serde(default)]
    pub test_script: Option<String>,
    /// The project's own copies of the program, used first when one's there (`.venv/bin/python`).
    #[serde(default)]
    pub local_programs: Vec<String>,
    /// The program's name on Windows, where it differs (`python`).
    #[serde(default)]
    pub windows_program: Option<String>,
    /// Optional ways to start the app, in preference order. No recipe means no inferred launch.
    #[serde(default)]
    pub launch: Vec<LaunchRecipe>,
    /// Detect a script-only project from its launch entrypoint, after checking all manifest markers.
    #[serde(default)]
    pub standalone_entrypoints: bool,
}

/// A declared entrypoint and argv template. A wildcard file must identify exactly one file.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
pub struct LaunchRecipe {
    pub file: String,
    /// `{file}` is the matched relative filename. `{package}` is its parent with an explicit `./`.
    pub command: Vec<String>,
}

#[derive(Default, serde::Deserialize)]
struct Table {
    toolchain: Vec<Toolchain>,
}

/// Built-in toolchains in detection order. A malformed catalogue is covered by a regression test.
pub fn catalogue() -> &'static [Toolchain] {
    static TABLE: OnceLock<Table> = OnceLock::new();
    &TABLE.get_or_init(|| toml::from_str(include_str!("toolchains.toml")).unwrap_or_default()).toolchain
}

/// The first toolchain whose marker files exist in this folder.
pub fn found_in(dir: &Path) -> Option<&'static Toolchain> {
    catalogue()
        .iter()
        .find(|t| t.found_by.iter().any(|pattern| !matching_files(dir, pattern).is_empty()))
        .or_else(|| catalogue().iter().find(|t| t.standalone_entrypoints && t.launch(dir).is_some()))
}

/// Match exact files, root suffixes, or one named file below each child of a fixed directory.
fn matching_files(dir: &Path, pattern: &str) -> Vec<PathBuf> {
    if let Some((folder, filename)) = pattern.split_once("/*/") {
        return scan_entries(&dir.join(folder), |entry| {
            let file = entry.path().join(filename);
            Ok((entry.file_type()?.is_dir() && file.is_file()).then_some(file))
        });
    }
    if let Some(suffix) = pattern.strip_prefix('*') {
        return scan_entries(dir, |entry| {
            let file = entry.path();
            Ok((entry.file_name().to_string_lossy().ends_with(suffix) && file.is_file()).then_some(file))
        });
    }
    let file = dir.join(pattern);
    file.is_file().then_some(file).into_iter().collect()
}

fn scan_entries(dir: &Path, candidate: impl Fn(std::fs::DirEntry) -> std::io::Result<Option<PathBuf>>) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else { return files };
    for (i, entry) in entries.enumerate() {
        // A partial scan cannot establish that the entrypoint is unique.
        if i >= 256 {
            return Vec::new();
        }
        let Ok(file) = entry.and_then(&candidate) else { return Vec::new() };
        files.extend(file);
    }
    files.sort();
    files
}

impl Toolchain {
    /// Resolve a command, preferring the project's own interpreter/build wrapper on this OS.
    pub fn command(&self, app: &Path, command: &[String]) -> Option<(PathBuf, Vec<String>)> {
        self.command_for(app, command, cfg!(windows))
    }

    /// Resolve for an explicit OS so platform names and local wrappers can be tested anywhere.
    fn command_for(&self, app: &Path, command: &[String], windows: bool) -> Option<(PathBuf, Vec<String>)> {
        let (program, args) = command.split_first()?;
        let here = |p: &&String| windows == [".bat", ".cmd", ".exe"].iter().any(|e| p.ends_with(e));
        // Callers run in `app`, so a project-relative executable must first be made absolute.
        // Keep virtualenv symlinks intact: canonicalizing can select the system interpreter.
        let local = self.local_programs.iter().filter(here).map(|p| app.join(p)).find(|p| p.is_file()).map(std::path::absolute).transpose().ok()?;
        let system = self.windows_program.as_ref().filter(|_| windows).unwrap_or(program);
        Some((local.unwrap_or_else(|| PathBuf::from(system)), args.to_vec()))
    }

    /// Infer a launch only from a recipe's unambiguous entrypoint. Never guess among projects.
    pub fn launch(&self, app: &Path) -> Option<(PathBuf, Vec<String>)> {
        for recipe in &self.launch {
            let files = matching_files(app, &recipe.file);
            if files.len() != 1 {
                continue;
            }
            let relative = files.first()?.strip_prefix(app).ok()?;
            let file = relative.to_str()?;
            let parent = relative.parent()?;
            let command: Vec<String> = recipe
                .command
                .iter()
                .map(|arg| match arg.as_str() {
                    "{package}" => format!("./{}", parent.display()),
                    _ => arg.replace("{file}", file),
                })
                .collect();
            return self.command(app, &command);
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every catalogue row still has its original build/test commands and valid launch recipes.
    #[test]
    fn the_shared_catalogue_is_complete() {
        let parsed: Table = toml::from_str(include_str!("toolchains.toml")).unwrap();
        assert_eq!(parsed.toolchain.len(), 8);
        for tool in catalogue() {
            assert!(!tool.found_by.is_empty() && !tool.build.is_empty() && !tool.test.is_empty(), "{tool:?}");
            for recipe in &tool.launch {
                assert!(!recipe.file.is_empty() && !recipe.command.is_empty(), "{recipe:?}");
            }
        }
    }

    /// Shared launch detection preserves argument boundaries and refuses ambiguous projects.
    #[test]
    fn project_launches_come_from_entrypoints() {
        let launch = crate::outputs::launch;
        for (marker, file, program, args) in [
            ("requirements.txt", "main.py", if cfg!(windows) { "python" } else { "python3" }, vec!["main.py"]),
            ("pyproject.toml", "app.py", if cfg!(windows) { "python" } else { "python3" }, vec!["app.py"]),
            ("pyproject.toml", "__main__.py", if cfg!(windows) { "python" } else { "python3" }, vec!["."]),
            ("go.mod", "main.go", "go", vec!["run", "."]),
            ("App & Sample.csproj", "App & Sample.csproj", "dotnet", vec!["run", "--no-build", "--project", "App & Sample.csproj", "--"]),
        ] {
            let dir = tempfile::tempdir().unwrap();
            std::fs::write(dir.path().join(marker), "").unwrap();
            std::fs::write(dir.path().join(file), "").unwrap();
            std::fs::write(dir.path().join("index.html"), "<h1>embedded page</h1>").unwrap();
            assert!(crate::outputs::site_dir(dir.path()).is_none(), "an app's HTML asset must not turn Play into a website preview");
            assert_eq!(launch(dir.path(), None), Some((PathBuf::from(program), args.into_iter().map(String::from).collect())), "{marker}");
        }
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("go.mod"), "module sample").unwrap();
        assert!(launch(dir.path(), None).is_none(), "a library has no conventional entrypoint");
        std::fs::remove_file(dir.path().join("go.mod")).unwrap();
        for name in ["First.csproj", "Second.csproj"] {
            std::fs::write(dir.path().join(name), "").unwrap();
        }
        assert!(launch(dir.path(), None).is_none(), "never choose arbitrarily between projects");
        std::fs::remove_file(dir.path().join("Second.csproj")).unwrap();
        for i in 0..256 {
            std::fs::write(dir.path().join(format!("note-{i}")), "").unwrap();
        }
        assert!(launch(dir.path(), None).is_none(), "an incomplete scan cannot prove uniqueness");
    }

    #[test]
    fn go_launch_uses_one_command_package_and_prefers_root() {
        let dir = tempfile::tempdir().unwrap();
        let app = dir.path();
        std::fs::write(app.join("go.mod"), "module fixture").unwrap();
        for name in ["viewer", "library"] {
            std::fs::create_dir_all(app.join("cmd").join(name)).unwrap();
        }
        std::fs::write(app.join("cmd/viewer/main.go"), "package main").unwrap();
        let launch = || crate::outputs::launch(app, None);
        let package = format!("./{}", Path::new("cmd").join("viewer").display());
        assert_eq!(launch(), Some(("go".into(), vec!["run".into(), package])));
        std::fs::write(app.join("cmd/library/main.go"), "package main").unwrap();
        assert!(launch().is_none());
        std::fs::write(app.join("main.go"), "package main").unwrap();
        assert_eq!(launch(), Some(("go".into(), vec!["run".into(), ".".into()])));
    }

    /// A helper script must not override a project's explicit Go, Rust or Node.js manifest.
    #[test]
    fn manifest_markers_take_precedence_over_standalone_scripts() {
        for (marker, name) in [("go.mod", "Go"), ("Cargo.toml", "Rust"), ("package.json", "Node.js")] {
            let dir = tempfile::tempdir().unwrap();
            std::fs::write(dir.path().join("main.py"), "").unwrap();
            assert_eq!(found_in(dir.path()).unwrap().name, "Python");
            std::fs::write(dir.path().join(marker), "").unwrap();
            assert_eq!(found_in(dir.path()).unwrap().name, name);
        }
    }

    /// Python build and run use the same project-local interpreter; Windows uses its own names.
    #[test]
    fn project_interpreters_and_platform_programs_are_shared() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("main.py"), "").unwrap();
        let tool = found_in(dir.path()).unwrap();
        for (windows, relative) in [(false, ".venv/bin/python"), (true, ".venv/Scripts/python.exe")] {
            let path = dir.path().join(relative);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, "fixture").unwrap();
            let command = vec!["python3".to_string(), "main.py".to_string()];
            assert_eq!(tool.command_for(dir.path(), &command, windows), Some((path.clone(), vec!["main.py".into()])));
            assert_eq!(tool.command_for(dir.path(), &tool.build, windows).unwrap().0, path);
        }
        assert_eq!(tool.launch(dir.path()).unwrap().0, dir.path().join(if cfg!(windows) { ".venv/Scripts/python.exe" } else { ".venv/bin/python" }));
    }

    /// A local runner remains resolvable after the caller changes to a relative project folder.
    #[test]
    fn relative_projects_resolve_local_programs_absolutely() {
        let cwd = std::env::current_dir().unwrap();
        let dir = tempfile::tempdir_in(&cwd).unwrap();
        let app = dir.path().strip_prefix(&cwd).unwrap();
        std::fs::write(app.join("main.py"), "").unwrap();
        let tool = found_in(app).unwrap();
        let command = vec!["python3".into(), "main.py".into(), "argument with spaces".into()];
        for (windows, relative, system) in [(false, ".venv/bin/python", "python3"), (true, ".venv/Scripts/python.exe", "python")] {
            assert_eq!(tool.command_for(app, &command, windows), Some((PathBuf::from(system), command[1..].to_vec())));
            let path = app.join(relative);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, "fixture").unwrap();
            let expected = std::path::absolute(&path).unwrap();
            assert_eq!(tool.command_for(app, &command, windows), Some((expected.clone(), command[1..].to_vec())));
            assert_eq!(tool.command_for(app, &tool.build, windows).unwrap().0, expected);
            if windows == cfg!(windows) {
                assert_eq!(crate::outputs::launch(app, None), Some((expected, vec!["main.py".into()])));
            }
        }
    }
}
