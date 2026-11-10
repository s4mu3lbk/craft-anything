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
    /// Detect a script-only project from a root launch entrypoint, after checking all manifest
    /// markers. A wildcard one such as a package folder could be any project's helper.
    #[serde(default)]
    pub standalone_entrypoints: bool,
    /// Use a single installed PEP 621 console script after checking the fixed entrypoints.
    #[serde(default)]
    pub python_console_script: bool,
}

/// A declared entrypoint and argv template. A wildcard file must identify exactly one file.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
pub struct LaunchRecipe {
    pub file: String,
    /// `{file}` is the matched relative filename. `{package}` is its parent with an explicit `./`,
    /// `{module}` its parent as a Python module name.
    pub command: Vec<String>,
    /// Count only jars whose manifest names a `Main-Class`, which `java -jar` needs: a build can
    /// also leave a plain library jar, sources or the original of a repackaged one.
    #[serde(default)]
    pub runnable_jar: bool,
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
        .or_else(|| catalogue().iter().find(|t| t.standalone_entrypoints && t.launch.iter().any(|r| !r.file.contains('*') && dir.join(&r.file).is_file())))
}

/// Match exact files, suffixes in a fixed directory (`*.csproj`, `target/*.jar`), or one named
/// file below each child of a fixed directory (`cmd/*/main.go`, `*/__main__.py`).
fn matching_files(dir: &Path, pattern: &str) -> Vec<PathBuf> {
    if let Some((folder, filename)) = pattern.split_once("*/") {
        return scan_entries(&dir.join(folder), |entry| {
            let file = entry.path().join(filename);
            Ok((entry.file_type()?.is_dir() && file.is_file()).then_some(file))
        });
    }
    let (folder, name) = pattern.rsplit_once('/').unwrap_or(("", pattern));
    if let Some(suffix) = name.strip_prefix('*') {
        return scan_entries(&dir.join(folder), |entry| {
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
        // Only the toolchain's own program (its build's or tests') has local copies and a Windows
        // name: a Maven project's `java -jar` must not become `mvnw -jar`.
        let own = [&self.build, &self.test].iter().any(|c| c.first() == Some(program));
        let local =
            self.local_programs.iter().filter(|_| own).filter(here).map(|p| app.join(p)).find(|p| p.is_file()).map(std::path::absolute).transpose().ok()?;
        let system = self.windows_program.as_ref().filter(|_| windows && own).unwrap_or(program);
        Some((local.unwrap_or_else(|| PathBuf::from(system)), args.to_vec()))
    }

    /// Infer a launch only from a recipe's unambiguous entrypoint. Never guess among projects.
    pub fn launch(&self, app: &Path) -> Option<(PathBuf, Vec<String>)> {
        for recipe in &self.launch {
            let mut files = matching_files(app, &recipe.file);
            if recipe.runnable_jar {
                files.retain(|f| runnable_jar(f));
            }
            // `python -m` reads a dot as a subpackage and refuses a hidden folder's leading one.
            if recipe.command.iter().any(|a| a == "{module}") {
                files.retain(|f| f.parent().and_then(Path::file_name).is_some_and(|n| !n.to_string_lossy().contains('.')));
            }
            if files.len() != 1 {
                continue;
            }
            let relative = files.first()?.strip_prefix(app).ok()?;
            let file = relative.to_str()?;
            let parent = relative.parent()?;
            let folders: Vec<String> = parent.components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect();
            // Go reads `\` as a separator too, so a folder named with one (possible on Unix)
            // would name another package: no launch is inferred for it.
            if recipe.command.iter().any(|a| a == "{package}") && folders.iter().any(|f| f.contains('\\')) {
                continue;
            }
            let command: Vec<String> = recipe
                .command
                .iter()
                .map(|arg| match arg.as_str() {
                    // Go names packages with `/` on every system.
                    "{package}" => format!("./{}", folders.join("/")),
                    "{module}" => parent.to_string_lossy().replace(['/', '\\'], "."),
                    _ => arg.replace("{file}", file),
                })
                .collect();
            return self.command(app, &command);
        }
        if self.python_console_script {
            let (python, _) = self.command(app, &self.build)?;
            let bin = python.parent().filter(|p| p.is_absolute())?;
            return python_package_script(app, bin).map(|program| (program, Vec::new()));
        }
        None
    }
}

/// Use the wrapper installed by the project's own Python, without guessing among entrypoints
/// or searching PATH for a same-named program from a different package.
fn python_package_script(app: &Path, bin: &Path) -> Option<PathBuf> {
    let bytes = crate::tools::artifact_tools::read_capped(&app.join("pyproject.toml"), 1024 * 1024).ok()?;
    let manifest: toml::Table = std::str::from_utf8(&bytes).ok()?.parse().ok()?;
    let scripts = manifest.get("project")?.get("scripts")?.as_table()?;
    let mut entries = scripts.iter();
    let (name, entrypoint) = entries.next()?;
    if entries.next().is_some() || entrypoint.as_str()?.trim().is_empty() {
        return None;
    }
    // A script name is a filename on every OS, never an absolute path or traversal.
    if !name.starts_with(|c: char| c.is_ascii_alphanumeric()) || !name.bytes().all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b)) {
        return None;
    }
    let file = bin.join(if cfg!(windows) { format!("{name}.exe") } else { name.clone() });
    let metadata = file.metadata().ok()?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o111 == 0 {
            return None;
        }
    }
    metadata.is_file().then_some(file)
}

/// Whether `java -jar` can start `jar`: its manifest names a main class. Only the archive's
/// directory and manifest are read, whatever the jar's size. A jar that can't be read this way
/// (a central directory over 16 MB, a damaged archive) doesn't count. Attribute names are
/// case-insensitive, as the JAR specification has them.
fn runnable_jar(jar: &Path) -> bool {
    let Ok(mut file) = std::fs::File::open(jar) else { return false };
    let manifest = craft_anything_formats::zip_file::read_named_entry(&mut file, "META-INF/MANIFEST.MF", 1 << 20);
    let manifest = manifest.ok().flatten().map(|m| crate::tools::java_tools::parse_manifest(&m)).unwrap_or_default();
    manifest.iter().any(|(name, class)| name.eq_ignore_ascii_case("Main-Class") && !class.is_empty())
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
        assert_eq!(launch(), Some(("go".into(), vec!["run".into(), "./cmd/viewer".into()])));
        std::fs::write(app.join("cmd/library/main.go"), "package main").unwrap();
        assert!(launch().is_none());
        std::fs::write(app.join("main.go"), "package main").unwrap();
        assert_eq!(launch(), Some(("go".into(), vec!["run".into(), ".".into()])));
    }

    /// On Unix a folder may be named with `\`, which Go reads as a separator: `go run` would start
    /// another package, so nothing is inferred for it.
    #[cfg(unix)]
    #[test]
    fn go_launch_skips_a_command_folder_named_with_a_backslash() {
        let dir = tempfile::tempdir().unwrap();
        let app = dir.path();
        std::fs::write(app.join("go.mod"), "module fixture").unwrap();
        std::fs::create_dir_all(app.join("cmd").join("foo\\bar")).unwrap();
        std::fs::write(app.join("cmd").join("foo\\bar").join("main.go"), "package main").unwrap();
        assert_eq!(crate::outputs::launch(app, None), None);
    }

    /// A Java project runs the one runnable jar its build left, with `java` even beside a build
    /// wrapper. A plain library jar beside it isn't a second choice.
    #[test]
    fn java_launch_uses_the_one_runnable_jar() {
        let jar = |manifest: &str| craft_anything_testkit::electron::zip_stored(&[("META-INF/MANIFEST.MF", manifest.as_bytes())]);
        let runnable = jar("Manifest-Version: 1.0\r\nMain-Class: app.Main\r\n\r\n");
        let library = jar("Manifest-Version: 1.0\r\n\r\n");
        for (marker, folder) in [("pom.xml", "target"), ("build.gradle", "build/libs")] {
            let dir = tempfile::tempdir().unwrap();
            let app = dir.path();
            for file in [marker, "mvnw", "mvnw.cmd", "gradlew", "gradlew.bat"] {
                std::fs::write(app.join(file), "").unwrap();
            }
            std::fs::create_dir_all(app.join(folder)).unwrap();
            let launch = || crate::outputs::launch(app, None);
            assert!(launch().is_none(), "{marker}: nothing built yet");
            std::fs::write(app.join(folder).join("app-plain.jar"), &library).unwrap();
            assert!(launch().is_none(), "{marker}: a jar without a main class can't be started");
            std::fs::write(app.join(folder).join("app.jar"), &runnable).unwrap();
            let expected = Some(("java".into(), vec!["-jar".into(), Path::new(folder).join("app.jar").display().to_string()]));
            assert_eq!(launch(), expected, "{marker}");
            let tool = found_in(app).unwrap();
            let command = tool.launch[0].command.clone();
            assert_eq!(tool.command_for(app, &command, true).unwrap().0, PathBuf::from("java"), "{marker}: not the Windows wrapper");
            // A second runnable jar, its attribute written in lower case.
            std::fs::write(app.join(folder).join("app-all.jar"), jar("Manifest-Version: 1.0\r\nmain-class: app.Main\r\n\r\n")).unwrap();
            assert!(launch().is_none(), "{marker}: never choose between runnable jars");
        }
    }

    /// A Python project's package runs as a module. A root `__main__.py` keeps priority, two
    /// packages give none, and a package alone doesn't make a folder a Python project.
    #[test]
    fn python_package_launches_as_a_module() {
        let dir = tempfile::tempdir().unwrap();
        let app = dir.path();
        let python = if cfg!(windows) { "python" } else { "python3" };
        std::fs::create_dir_all(app.join("game")).unwrap();
        std::fs::write(app.join("game/__main__.py"), "").unwrap();
        assert!(found_in(app).is_none(), "a package folder alone could be any project's helper");
        std::fs::write(app.join("pyproject.toml"), "").unwrap();
        let launch = || crate::outputs::launch(app, None);
        assert_eq!(launch(), Some((python.into(), vec!["-m".into(), "game".into()])));
        for hidden in [".cache", "game.v2"] {
            std::fs::create_dir_all(app.join(hidden)).unwrap();
            std::fs::write(app.join(hidden).join("__main__.py"), "").unwrap();
        }
        assert_eq!(launch(), Some((python.into(), vec!["-m".into(), "game".into()])), "python -m can't name a folder with a dot");
        std::fs::write(app.join("__main__.py"), "").unwrap();
        assert_eq!(launch(), Some((python.into(), vec![".".into()])), "the root entrypoint comes first");
        std::fs::remove_file(app.join("__main__.py")).unwrap();
        std::fs::create_dir_all(app.join("tools")).unwrap();
        std::fs::write(app.join("tools/__main__.py"), "").unwrap();
        assert!(launch().is_none(), "never choose between packages");
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

    /// An installed package with no root script launches only its single local console wrapper.
    #[test]
    fn python_package_scripts_require_one_declared_and_installed_entrypoint() {
        let project = tempfile::tempdir().unwrap();
        let app = project.path();
        let manifest = app.join("pyproject.toml");
        let bin = app.join(if cfg!(windows) { ".venv/Scripts" } else { ".venv/bin" });
        std::fs::create_dir_all(&bin).unwrap();
        std::fs::write(bin.join(if cfg!(windows) { "python.exe" } else { "python" }), "").unwrap();
        let wrapper = bin.join(if cfg!(windows) { "sample-cli.exe" } else { "sample-cli" });
        std::fs::write(&manifest, "[project.scripts]\nsample-cli = 'sample.cli:main'").unwrap();
        assert_eq!(crate::outputs::launch(app, None), None);
        std::fs::write(&wrapper, "").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(crate::outputs::launch(app, None), None);
            std::fs::set_permissions(&wrapper, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        assert_eq!(crate::outputs::launch(app, None), Some((wrapper, vec![])));
        for scripts in ["one = 'pkg:a'\ntwo = 'pkg:b'", "'../sample-cli' = 'pkg:a'", "sample-cli = 42"] {
            std::fs::write(&manifest, format!("[project.scripts]\n{scripts}")).unwrap();
            assert_eq!(crate::outputs::launch(app, None), None);
        }
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
