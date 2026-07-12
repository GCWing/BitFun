//! Test selection gate: before a dialog turn ends naturally, check whether the
//! files edited in this turn have had their associated tests executed. If not,
//! inject a single structured reminder listing narrowly-scoped suggested
//! commands (mirroring the verification scope rules in the agentic prompt) and
//! give the model one more round. Fires at most once per turn and never blocks
//! the second completion attempt.
//!
//! Direction is deliberately asymmetric: suggested commands are narrow (single
//! Go package, single test file) to avoid long compile times, while coverage
//! detection is lenient (a full-suite run also counts) to avoid nagging.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

const MAX_REMINDER_ITEMS: usize = 5;
/// Directories never traversed when searching for test files.
const SKIP_DIRS: &[&str] = &[
    "node_modules",
    ".git",
    "vendor",
    "target",
    "dist",
    "build",
    ".venv",
    "venv",
    "__pycache__",
];
/// Upper bound on directories visited per lookup so huge repos stay cheap.
const MAX_VISITED_DIRS: usize = 4000;

const RUNNER_KEYWORDS: &[&str] = &[
    "go test",
    "pytest",
    "py.test",
    "jest",
    "vitest",
    "mocha",
    "ava ",
    "npm test",
    "npm run test",
    "yarn test",
    "pnpm test",
    "cargo test",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Lang {
    Go,
    Python,
    JsTs,
    Rust,
}

impl Lang {
    fn from_path(path: &Path) -> Option<Self> {
        match path.extension().and_then(|e| e.to_str())? {
            "go" => Some(Lang::Go),
            "py" => Some(Lang::Python),
            "ts" | "tsx" | "js" | "jsx" | "mjs" | "cjs" => Some(Lang::JsTs),
            "rs" => Some(Lang::Rust),
            _ => None,
        }
    }

    fn is_test_file(&self, path: &Path) -> bool {
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default();
        let in_dir = |dir: &str| path.components().any(|c| c.as_os_str() == dir);
        match self {
            Lang::Go => name.ends_with("_test.go"),
            Lang::Python => name.starts_with("test_") || name.ends_with("_test.py"),
            Lang::JsTs => {
                name.contains(".test.") || name.contains(".spec.") || in_dir("__tests__")
            }
            Lang::Rust => in_dir("tests"),
        }
    }

    fn is_full_run(&self, command: &str) -> bool {
        match self {
            Lang::Go => command.contains("./..."),
            // Any pytest invocation that doesn't name a specific file counts
            // as broad; directory-scoped runs are treated leniently too.
            Lang::Python => !command.contains(".py"),
            Lang::JsTs => !command.contains('/'),
            Lang::Rust => command.contains("--workspace") || !command.contains("-p "),
        }
    }
}

#[derive(Debug, Clone)]
struct TestTarget {
    /// Workspace-relative edited file that produced this requirement.
    source_file: String,
    /// Narrowly-scoped command mirroring the prompt's verification rules.
    suggested_cmd: String,
    /// Any of these substrings appearing in a test-runner command marks the
    /// target as covered.
    match_tokens: Vec<String>,
    lang: Lang,
}

#[derive(Debug, Default)]
pub struct TestSelectionGate {
    edited_files: BTreeSet<PathBuf>,
    bash_commands: Vec<String>,
    reminded: bool,
}

impl TestSelectionGate {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn reminded(&self) -> bool {
        self.reminded
    }

    /// Feed every executed tool call (with its success flag) into the tracker.
    pub fn observe_tool_call(
        &mut self,
        tool_name: &str,
        arguments: &serde_json::Value,
        is_error: bool,
    ) {
        if is_error {
            return;
        }
        match tool_name {
            "Edit" | "Write" | "MultiEdit" => {
                if let Some(p) = arguments.get("file_path").and_then(|v| v.as_str()) {
                    self.edited_files.insert(PathBuf::from(p));
                }
            }
            "Bash" | "ExecCommand" => {
                // Bash uses `command`; ExecCommand uses `cmd`.
                if let Some(c) = arguments
                    .get("command")
                    .or_else(|| arguments.get("cmd"))
                    .and_then(|v| v.as_str())
                {
                    self.bash_commands.push(c.to_string());
                }
            }
            _ => {}
        }
    }

    /// At natural turn completion: returns the reminder text when uncovered
    /// test targets exist. Returns `None` (and stays silent forever after the
    /// first reminder) otherwise. Marks the gate as reminded on `Some`.
    pub fn build_reminder(&mut self, workspace_root: Option<&Path>) -> Option<String> {
        if self.reminded || self.edited_files.is_empty() {
            return None;
        }
        let root = workspace_root?;
        let mut uncovered: Vec<TestTarget> = Vec::new();
        for file in &self.edited_files {
            let rel = normalize_relative(file, root);
            let Some(target) = discover_target(&rel, root) else {
                continue;
            };
            if !self.is_covered(&target) {
                uncovered.push(target);
            }
            if uncovered.len() >= MAX_REMINDER_ITEMS {
                break;
            }
        }
        if uncovered.is_empty() {
            return None;
        }
        self.reminded = true;
        let mut text = String::from(
            "You edited the following files in this session but never ran their associated tests:\n",
        );
        for t in &uncovered {
            text.push_str(&format!("- {}  →  run: {}\n", t.source_file, t.suggested_cmd));
        }
        text.push_str(
            "Run these tests now and fix any failures before finishing. \
             Keep the scope exactly as suggested (do not widen to whole-repo runs). \
             If a listed test is genuinely not applicable to your change, you may finish without running it.",
        );
        Some(text)
    }

    fn is_covered(&self, target: &TestTarget) -> bool {
        self.bash_commands.iter().any(|cmd| {
            let is_runner = RUNNER_KEYWORDS.iter().any(|k| cmd.contains(k));
            if !is_runner {
                return false;
            }
            target.match_tokens.iter().any(|t| cmd.contains(t.as_str()))
                || target.lang.is_full_run(cmd)
        })
    }
}

/// Make an absolute or already-relative edited path workspace-relative.
fn normalize_relative(path: &Path, root: &Path) -> PathBuf {
    path.strip_prefix(root).map(Path::to_path_buf).unwrap_or_else(|_| path.to_path_buf())
}

fn rel_str(p: &Path) -> String {
    p.to_string_lossy().replace('\\', "/")
}

fn discover_target(rel: &Path, root: &Path) -> Option<TestTarget> {
    let lang = Lang::from_path(rel)?;
    let source = rel_str(rel);

    // An edited test file is its own target.
    if lang.is_test_file(rel) {
        return Some(match lang {
            Lang::Go => go_target(rel.parent().unwrap_or(Path::new(".")), &source),
            Lang::Python => TestTarget {
                suggested_cmd: format!("python -m pytest {}", source),
                match_tokens: name_tokens(rel, &source),
                source_file: source,
                lang,
            },
            Lang::JsTs => TestTarget {
                suggested_cmd: js_command(root, &source),
                match_tokens: name_tokens(rel, &source),
                source_file: source,
                lang,
            },
            Lang::Rust => rust_target(rel, root, &source)?,
        });
    }

    match lang {
        Lang::Go => {
            let dir = rel.parent().unwrap_or(Path::new("."));
            let has_tests = std::fs::read_dir(root.join(dir)).ok()?.flatten().any(|e| {
                e.file_name()
                    .to_str()
                    .is_some_and(|n| n.ends_with("_test.go"))
            });
            has_tests.then(|| go_target(dir, &source))
        }
        Lang::Python => {
            let stem = rel.file_stem()?.to_str()?;
            let names = [format!("test_{stem}.py"), format!("{stem}_test.py")];
            let found = find_file_by_names(root, &names)?;
            let found_rel = rel_str(&found);
            Some(TestTarget {
                suggested_cmd: format!("python -m pytest {}", found_rel),
                match_tokens: vec![
                    found.file_name().unwrap().to_string_lossy().into_owned(),
                    found_rel,
                ],
                source_file: source,
                lang,
            })
        }
        Lang::JsTs => {
            let stem = js_stem(rel)?;
            let dir = rel.parent().unwrap_or(Path::new(""));
            let mut candidates: Vec<PathBuf> = Vec::new();
            for ext in ["ts", "tsx", "js", "jsx"] {
                for pat in [
                    format!("{stem}.test.{ext}"),
                    format!("{stem}.spec.{ext}"),
                ] {
                    candidates.push(dir.join(&pat));
                    candidates.push(dir.join("__tests__").join(&pat));
                    candidates.push(dir.join("tests").join(&pat));
                }
            }
            let found = candidates.into_iter().find(|c| root.join(c).is_file())?;
            let found_rel = rel_str(&found);
            Some(TestTarget {
                suggested_cmd: js_command(root, &found_rel),
                match_tokens: vec![
                    found.file_name().unwrap().to_string_lossy().into_owned(),
                    found_rel,
                ],
                source_file: source,
                lang,
            })
        }
        Lang::Rust => rust_target(rel, root, &source),
    }
}

fn go_target(dir: &Path, source: &str) -> TestTarget {
    let dir_rel = rel_str(dir);
    let dir_rel = dir_rel.trim_start_matches("./").trim_end_matches('/');
    if dir_rel.is_empty() || dir_rel == "." {
        TestTarget {
            suggested_cmd: "go test .".to_string(),
            match_tokens: vec!["go test .".to_string()],
            source_file: source.to_string(),
            lang: Lang::Go,
        }
    } else {
        TestTarget {
            suggested_cmd: format!("go test ./{}/", dir_rel),
            match_tokens: vec![dir_rel.to_string()],
            source_file: source.to_string(),
            lang: Lang::Go,
        }
    }
}

fn rust_target(rel: &Path, root: &Path, source: &str) -> Option<TestTarget> {
    // Walk up to the nearest Cargo.toml with a [package] name.
    let mut dir = rel.parent().unwrap_or(Path::new(""));
    loop {
        let manifest = root.join(dir).join("Cargo.toml");
        if let Ok(body) = std::fs::read_to_string(&manifest) {
            if let Some(name) = parse_crate_name(&body) {
                return Some(TestTarget {
                    suggested_cmd: format!("cargo test -p {}", name),
                    match_tokens: vec![name],
                    source_file: source.to_string(),
                    lang: Lang::Rust,
                });
            }
        }
        dir = dir.parent()?;
    }
}

fn parse_crate_name(manifest: &str) -> Option<String> {
    let mut in_package = false;
    for line in manifest.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_package = line == "[package]";
            continue;
        }
        if in_package {
            if let Some(rest) = line.strip_prefix("name") {
                let rest = rest.trim_start().strip_prefix('=')?.trim();
                return Some(rest.trim_matches('"').to_string());
            }
        }
    }
    None
}

fn js_stem(rel: &Path) -> Option<String> {
    let name = rel.file_name()?.to_str()?;
    let stem = name
        .rsplit_once('.')
        .map(|(s, _)| s)
        .unwrap_or(name);
    Some(stem.to_string())
}

/// Pick the repo's own JS test runner from package.json's test script.
fn js_command(root: &Path, test_rel: &str) -> String {
    let script = std::fs::read_to_string(root.join("package.json"))
        .ok()
        .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
        .and_then(|v| {
            v.get("scripts")
                .and_then(|s| s.get("test"))
                .and_then(|t| t.as_str())
                .map(str::to_string)
        })
        .unwrap_or_default();
    if script.contains("vitest") {
        format!("npx vitest run {}", test_rel)
    } else if script.contains("mocha") {
        format!("npx mocha {}", test_rel)
    } else if script.contains("jest") || script.is_empty() {
        format!("npx jest {}", test_rel)
    } else {
        format!("npm test -- {}", test_rel)
    }
}

/// Bounded breadth-first search for any of `names` under `root`.
fn find_file_by_names(root: &Path, names: &[String]) -> Option<PathBuf> {
    let mut queue: Vec<PathBuf> = vec![PathBuf::new()];
    let mut visited = 0usize;
    while let Some(dir) = queue.pop() {
        visited += 1;
        if visited > MAX_VISITED_DIRS {
            return None;
        }
        let entries = match std::fs::read_dir(root.join(&dir)) {
            Ok(e) => e,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let file_name = entry.file_name();
            let Some(name) = file_name.to_str() else {
                continue;
            };
            let path = dir.join(name);
            match entry.file_type() {
                Ok(t) if t.is_dir() => {
                    if !name.starts_with('.') && !SKIP_DIRS.contains(&name) {
                        queue.push(path);
                    }
                }
                Ok(t) if t.is_file() => {
                    if names.iter().any(|n| n == name) {
                        return Some(path);
                    }
                }
                _ => {}
            }
        }
    }
    None
}

fn name_tokens(rel: &Path, rel_string: &str) -> Vec<String> {
    let mut tokens = vec![rel_string.to_string()];
    if let Some(name) = rel.file_name().and_then(|n| n.to_str()) {
        tokens.push(name.to_string());
    }
    tokens
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    struct TempWs(PathBuf);
    impl TempWs {
        fn new(files: &[&str]) -> Self {
            let root = std::env::temp_dir().join(format!(
                "tsg-test-{}-{:x}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            for f in files {
                let p = root.join(f);
                std::fs::create_dir_all(p.parent().unwrap()).unwrap();
                std::fs::write(&p, "x").unwrap();
            }
            TempWs(root)
        }
        fn root(&self) -> &Path {
            &self.0
        }
    }
    impl Drop for TempWs {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn edit(gate: &mut TestSelectionGate, path: &str) {
        gate.observe_tool_call("Edit", &json!({"file_path": path}), false);
    }
    fn bash(gate: &mut TestSelectionGate, cmd: &str) {
        gate.observe_tool_call("Bash", &json!({"command": cmd}), false);
    }

    #[test]
    fn go_edit_without_test_run_triggers_reminder() {
        let ws = TempWs::new(&["pkg/x/y.go", "pkg/x/y_test.go"]);
        let mut g = TestSelectionGate::new();
        edit(&mut g, ws.root().join("pkg/x/y.go").to_str().unwrap());
        let msg = g.build_reminder(Some(ws.root())).expect("should remind");
        assert!(msg.contains("go test ./pkg/x/"), "{msg}");
        // Second attempt must always pass through.
        assert!(g.build_reminder(Some(ws.root())).is_none());
    }

    #[test]
    fn go_package_scoped_run_counts_as_covered() {
        let ws = TempWs::new(&["pkg/x/y.go", "pkg/x/y_test.go"]);
        let mut g = TestSelectionGate::new();
        edit(&mut g, ws.root().join("pkg/x/y.go").to_str().unwrap());
        bash(&mut g, "go test ./pkg/x/ -run TestY");
        assert!(g.build_reminder(Some(ws.root())).is_none());
    }

    #[test]
    fn exec_command_tool_with_cmd_arg_counts_as_covered() {
        let ws = TempWs::new(&["pkg/x/y.go", "pkg/x/y_test.go"]);
        let mut g = TestSelectionGate::new();
        edit(&mut g, ws.root().join("pkg/x/y.go").to_str().unwrap());
        g.observe_tool_call(
            "ExecCommand",
            &json!({"cmd": "cd /w && go test ./pkg/x/"}),
            false,
        );
        assert!(g.build_reminder(Some(ws.root())).is_none());
    }

    #[test]
    fn go_full_repo_run_counts_as_covered() {
        let ws = TempWs::new(&["pkg/x/y.go", "pkg/x/y_test.go"]);
        let mut g = TestSelectionGate::new();
        edit(&mut g, ws.root().join("pkg/x/y.go").to_str().unwrap());
        bash(&mut g, "go test ./...");
        assert!(g.build_reminder(Some(ws.root())).is_none());
    }

    #[test]
    fn go_package_without_tests_is_silent() {
        let ws = TempWs::new(&["pkg/x/y.go"]);
        let mut g = TestSelectionGate::new();
        edit(&mut g, ws.root().join("pkg/x/y.go").to_str().unwrap());
        assert!(g.build_reminder(Some(ws.root())).is_none());
    }

    #[test]
    fn python_source_maps_to_test_file() {
        let ws = TempWs::new(&["a/foo.py", "tests/unit/test_foo.py"]);
        let mut g = TestSelectionGate::new();
        edit(&mut g, ws.root().join("a/foo.py").to_str().unwrap());
        let msg = g.build_reminder(Some(ws.root())).expect("should remind");
        assert!(msg.contains("python -m pytest tests/unit/test_foo.py"), "{msg}");
    }

    #[test]
    fn python_bare_pytest_counts_as_full_run() {
        let ws = TempWs::new(&["a/foo.py", "tests/test_foo.py"]);
        let mut g = TestSelectionGate::new();
        edit(&mut g, ws.root().join("a/foo.py").to_str().unwrap());
        bash(&mut g, "python -m pytest tests");
        assert!(g.build_reminder(Some(ws.root())).is_none());
    }

    #[test]
    fn python_unrelated_test_run_still_reminds() {
        let ws = TempWs::new(&["a/foo.py", "tests/test_foo.py"]);
        let mut g = TestSelectionGate::new();
        edit(&mut g, ws.root().join("a/foo.py").to_str().unwrap());
        bash(&mut g, "python -m pytest tests/test_bar.py");
        assert!(g.build_reminder(Some(ws.root())).is_some());
    }

    #[test]
    fn js_test_discovery_and_runner_detection() {
        let ws = TempWs::new(&["src/foo.ts", "src/foo.test.ts"]);
        std::fs::write(
            ws.root().join("package.json"),
            r#"{"scripts": {"test": "vitest run"}}"#,
        )
        .unwrap();
        let mut g = TestSelectionGate::new();
        edit(&mut g, ws.root().join("src/foo.ts").to_str().unwrap());
        let msg = g.build_reminder(Some(ws.root())).expect("should remind");
        assert!(msg.contains("npx vitest run src/foo.test.ts"), "{msg}");
    }

    #[test]
    fn js_bare_yarn_test_counts_as_full_run() {
        let ws = TempWs::new(&["src/foo.ts", "src/foo.test.ts"]);
        let mut g = TestSelectionGate::new();
        edit(&mut g, ws.root().join("src/foo.ts").to_str().unwrap());
        bash(&mut g, "yarn test");
        assert!(g.build_reminder(Some(ws.root())).is_none());
    }

    #[test]
    fn rust_edit_maps_to_crate_scoped_test() {
        let ws = TempWs::new(&["crates/c/src/lib.rs"]);
        std::fs::write(
            ws.root().join("crates/c/Cargo.toml"),
            "[package]\nname = \"my-crate\"\n",
        )
        .unwrap();
        let mut g = TestSelectionGate::new();
        edit(&mut g, ws.root().join("crates/c/src/lib.rs").to_str().unwrap());
        let msg = g.build_reminder(Some(ws.root())).expect("should remind");
        assert!(msg.contains("cargo test -p my-crate"), "{msg}");
        // covered by crate-scoped run
        let mut g2 = TestSelectionGate::new();
        edit(&mut g2, ws.root().join("crates/c/src/lib.rs").to_str().unwrap());
        bash(&mut g2, "cargo test -p my-crate");
        assert!(g2.build_reminder(Some(ws.root())).is_none());
    }

    #[test]
    fn edited_test_file_is_its_own_target() {
        let ws = TempWs::new(&["pkg/x/y_test.go"]);
        let mut g = TestSelectionGate::new();
        edit(&mut g, ws.root().join("pkg/x/y_test.go").to_str().unwrap());
        let msg = g.build_reminder(Some(ws.root())).expect("should remind");
        assert!(msg.contains("go test ./pkg/x/"), "{msg}");
    }

    #[test]
    fn non_code_edits_never_trigger() {
        let ws = TempWs::new(&["README.md", "config.yaml"]);
        let mut g = TestSelectionGate::new();
        edit(&mut g, ws.root().join("README.md").to_str().unwrap());
        edit(&mut g, ws.root().join("config.yaml").to_str().unwrap());
        assert!(g.build_reminder(Some(ws.root())).is_none());
    }

    #[test]
    fn failed_edits_are_ignored() {
        let ws = TempWs::new(&["pkg/x/y.go", "pkg/x/y_test.go"]);
        let mut g = TestSelectionGate::new();
        g.observe_tool_call(
            "Edit",
            &json!({"file_path": ws.root().join("pkg/x/y.go").to_str().unwrap()}),
            true,
        );
        assert!(g.build_reminder(Some(ws.root())).is_none());
    }
}
