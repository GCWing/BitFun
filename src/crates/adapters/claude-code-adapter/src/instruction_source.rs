use openbitfun_services_core::bounded_fs::{
    collect_bounded_regular_files, BoundedDirectoryWalkError, BoundedDirectoryWalkLimits,
};
use openbitfun_services_core::instruction_scope::{
    parse_instruction_path_scope, InstructionPathScope,
};
use openbitfun_services_core::local_instructions::{
    read_local_instruction_file, LocalInstructionFile, LocalInstructionFiles,
    MAX_LOCAL_INSTRUCTION_FILES, MAX_LOCAL_INSTRUCTION_TOTAL_BYTES,
};
use std::collections::HashSet;
use std::path::{Component, Path, PathBuf};

const MAX_CLAUDE_IMPORT_DEPTH: usize = 5;

#[derive(Debug, Clone)]
pub struct ClaudeCodeInstructionSourceOptions {
    pub config_dir: Option<PathBuf>,
    pub home_dir: Option<PathBuf>,
    pub display_root: String,
}

impl ClaudeCodeInstructionSourceOptions {
    pub fn from_environment() -> Self {
        let home_dir = dirs::home_dir().filter(|path| path.is_absolute());
        let (config_dir, display_root) = match std::env::var_os("CLAUDE_CONFIG_DIR") {
            Some(value) => {
                let path = PathBuf::from(value);
                (
                    path.is_absolute().then_some(path),
                    "$CLAUDE_CONFIG_DIR".to_string(),
                )
            }
            None => (
                home_dir.as_deref().map(|home| home.join(".claude")),
                "~/.claude".to_string(),
            ),
        };
        Self {
            config_dir,
            home_dir,
            display_root,
        }
    }
}

impl Default for ClaudeCodeInstructionSourceOptions {
    fn default() -> Self {
        Self::from_environment()
    }
}

pub fn load_claude_code_user_instructions(
    options: &ClaudeCodeInstructionSourceOptions,
) -> Result<Vec<LocalInstructionFile>, String> {
    let Some(config_dir) = options.config_dir.as_deref() else {
        return Ok(Vec::new());
    };
    let mut files = LocalInstructionFiles::default();
    let mut read_attempts = 0usize;
    append_source_tree(
        &mut files,
        &mut read_attempts,
        options,
        config_dir,
        config_dir.join("CLAUDE.md"),
        None,
    )?;

    let rules_root = config_dir.join("rules");
    let rules = match collect_bounded_regular_files(
        &rules_root,
        BoundedDirectoryWalkLimits {
            max_depth: 32,
            max_entries: 4096,
            max_directories: 1024,
            max_files: 4096,
        },
        |path| {
            path.extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| extension.eq_ignore_ascii_case("md"))
        },
    ) {
        Ok(rules) => rules,
        Err(BoundedDirectoryWalkError::LimitExceeded(_)) => Vec::new(),
        Err(error) => {
            return Err(format!("Failed to scan Claude Code user rules: {error}"));
        }
    };
    for rule in rules {
        if files.is_at_capacity() {
            break;
        }
        let Some(file) = read_source(options, config_dir, &rule)? else {
            continue;
        };
        match parse_instruction_path_scope(&file.content) {
            Ok(InstructionPathScope::Unscoped) => append_source_tree(
                &mut files,
                &mut read_attempts,
                options,
                config_dir,
                rule,
                Some(file),
            )?,
            Ok(InstructionPathScope::Scoped { paths, body }) => {
                let mut file = file;
                file.content = expand_scoped_rule_imports(
                    &mut read_attempts,
                    options,
                    config_dir,
                    &rule,
                    body,
                )?;
                file.path_patterns = paths;
                files.push(file);
            }
            Err(error) => {
                log::warn!("Ignoring invalid Claude Code rule front matter: {error}");
            }
        }
    }
    Ok(files.into_files())
}

fn expand_scoped_rule_imports(
    read_attempts: &mut usize,
    options: &ClaudeCodeInstructionSourceOptions,
    config_dir: &Path,
    rule_path: &Path,
    body: String,
) -> Result<String, String> {
    let mut expanded = body;
    let mut seen = HashSet::from([normalize_path_lexically(rule_path)]);
    let mut pending = claude_import_paths(
        config_dir,
        options.home_dir.as_deref(),
        rule_path,
        &expanded,
    )
    .into_iter()
    .rev()
    .map(|path| (path, 1usize))
    .collect::<Vec<_>>();

    while let Some((path, depth)) = pending.pop() {
        if *read_attempts >= MAX_LOCAL_INSTRUCTION_FILES {
            break;
        }
        *read_attempts += 1;
        let Some(file) = read_source(options, config_dir, &path)? else {
            continue;
        };
        if !seen.insert(file.canonical_path.clone()) {
            continue;
        }
        let imports = if depth < MAX_CLAUDE_IMPORT_DEPTH {
            claude_import_paths(
                config_dir,
                options.home_dir.as_deref(),
                &path,
                &file.content,
            )
        } else {
            Vec::new()
        };
        if !file.content.trim().is_empty() {
            if expanded
                .len()
                .saturating_add(file.content.len())
                .saturating_add(3)
                > MAX_LOCAL_INSTRUCTION_TOTAL_BYTES
            {
                log::warn!("Claude Code scoped rule import byte limit reached");
                break;
            }
            expanded.push_str("\n\n");
            expanded.push_str(file.content.trim());
            expanded.push('\n');
        }
        pending.extend(imports.into_iter().rev().map(|import| (import, depth + 1)));
    }
    Ok(expanded)
}

fn append_source_tree(
    files: &mut LocalInstructionFiles,
    read_attempts: &mut usize,
    options: &ClaudeCodeInstructionSourceOptions,
    config_dir: &Path,
    path: PathBuf,
    initial_file: Option<LocalInstructionFile>,
) -> Result<(), String> {
    let mut pending = vec![(path, 0usize, initial_file)];
    while let Some((path, depth, prefetched)) = pending.pop() {
        if files.is_at_capacity() {
            break;
        }
        let file = match prefetched {
            Some(file) => Some(file),
            None => {
                if *read_attempts >= MAX_LOCAL_INSTRUCTION_FILES {
                    break;
                }
                *read_attempts += 1;
                read_source(options, config_dir, &path)?
            }
        };
        let Some(file) = file else { continue };
        if files.contains_path(&file.canonical_path) {
            continue;
        }
        let imports = if depth < MAX_CLAUDE_IMPORT_DEPTH {
            claude_import_paths(
                config_dir,
                options.home_dir.as_deref(),
                &path,
                &file.content,
            )
        } else {
            Vec::new()
        };
        if !files.push(file) {
            continue;
        }
        pending.extend(
            imports
                .into_iter()
                .rev()
                .map(|import| (import, depth + 1, None)),
        );
    }
    Ok(())
}

fn read_source(
    options: &ClaudeCodeInstructionSourceOptions,
    config_dir: &Path,
    path: &Path,
) -> Result<Option<LocalInstructionFile>, String> {
    let relative = normalize_path_lexically(path)
        .strip_prefix(normalize_path_lexically(config_dir))
        .ok()
        .map(|path| path.to_string_lossy().replace('\\', "/"))
        .unwrap_or_else(|| "instruction.md".to_string());
    read_local_instruction_file(
        path,
        config_dir,
        format!("{}/{relative}", options.display_root),
    )
}

fn claude_import_paths(
    root: &Path,
    home_dir: Option<&Path>,
    source: &Path,
    content: &str,
) -> Vec<PathBuf> {
    let root = normalize_path_lexically(root);
    let source_parent = source.parent().unwrap_or(&root);
    let mut imports = Vec::new();
    let mut seen = HashSet::new();
    for token in content.split_whitespace() {
        let token = token.trim_start_matches(['(', '[', '{', '"', '\'']);
        let Some(raw_path) = token.strip_prefix('@') else {
            continue;
        };
        let raw_path =
            raw_path.trim_end_matches(['.', ',', ';', ':', '!', '?', ')', ']', '}', '"', '\'']);
        if raw_path.is_empty()
            || raw_path.starts_with("http://")
            || raw_path.starts_with("https://")
        {
            continue;
        }
        let candidate = if let Some(relative) = raw_path.strip_prefix("~/") {
            let Some(home_dir) = home_dir else {
                continue;
            };
            normalize_path_lexically(&home_dir.join(relative))
        } else if Path::new(raw_path).is_absolute() {
            normalize_path_lexically(Path::new(raw_path))
        } else {
            normalize_path_lexically(&source_parent.join(raw_path))
        };
        if candidate.starts_with(&root) && seen.insert(candidate.clone()) {
            imports.push(candidate);
        }
    }
    imports
}

fn normalize_path_lexically(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            component => normalized.push(component.as_os_str()),
        }
    }
    normalized
}

#[cfg(test)]
mod tests {
    use super::{
        claude_import_paths, load_claude_code_user_instructions, ClaudeCodeInstructionSourceOptions,
    };
    use std::path::{Path, PathBuf};

    #[test]
    fn user_memory_resolves_bounded_imports_and_preserves_scoped_rules() {
        let temp = tempfile::tempdir().expect("tempdir");
        let config_dir = temp.path().join("claude");
        std::fs::create_dir_all(config_dir.join("imports")).expect("imports directory");
        std::fs::create_dir_all(config_dir.join("rules/nested")).expect("rules directory");
        std::fs::write(
            config_dir.join("CLAUDE.md"),
            "Claude root\n@imports/base.md\n@../outside.md\n",
        )
        .expect("Claude memory");
        std::fs::write(
            config_dir.join("imports/base.md"),
            "Imported base\n@nested.md\n@nested.md\n",
        )
        .expect("base import");
        std::fs::write(
            config_dir.join("imports/nested.md"),
            "Nested import\n@../CLAUDE.md\n",
        )
        .expect("nested import");
        std::fs::write(config_dir.join("rules/z-last.md"), "Last rule\n").expect("last rule");
        std::fs::write(config_dir.join("rules/nested/a-first.md"), "First rule\n")
            .expect("first rule");
        std::fs::write(
            config_dir.join("rules/scoped.md"),
            "---\npaths:\n  - src/**/*.rs\n---\nScoped rule\n",
        )
        .expect("scoped rule");
        let options = ClaudeCodeInstructionSourceOptions {
            config_dir: Some(config_dir),
            home_dir: None,
            display_root: "$CLAUDE_CONFIG_DIR".to_string(),
        };

        let files = load_claude_code_user_instructions(&options).expect("instructions");
        let names = files
            .iter()
            .map(|file| file.name.as_str())
            .collect::<Vec<_>>();

        assert_eq!(
            names,
            vec![
                "$CLAUDE_CONFIG_DIR/CLAUDE.md",
                "$CLAUDE_CONFIG_DIR/imports/base.md",
                "$CLAUDE_CONFIG_DIR/imports/nested.md",
                "$CLAUDE_CONFIG_DIR/rules/nested/a-first.md",
                "$CLAUDE_CONFIG_DIR/rules/scoped.md",
                "$CLAUDE_CONFIG_DIR/rules/z-last.md",
            ]
        );
        assert_eq!(
            files
                .iter()
                .find(|file| file.name.ends_with("/scoped.md"))
                .expect("scoped rule")
                .path_patterns,
            vec!["src/**/*.rs"]
        );
    }

    #[test]
    fn user_instruction_sources_preserve_path_scoped_rules_without_frontmatter() {
        let temp = tempfile::tempdir().expect("tempdir");
        let config_dir = temp.path().join("claude");
        std::fs::create_dir_all(config_dir.join("rules")).expect("rules directory");
        std::fs::write(
            config_dir.join("rules/scoped.md"),
            "---\npaths:\n  - src/**/*.{rs,toml}\n  - tests/**/*.rs\n---\n\nScoped rule\n",
        )
        .expect("scoped rule");
        let options = ClaudeCodeInstructionSourceOptions {
            config_dir: Some(config_dir),
            home_dir: None,
            display_root: "$CLAUDE_CONFIG_DIR".to_string(),
        };

        let files = load_claude_code_user_instructions(&options).expect("sources");

        assert_eq!(files.len(), 1);
        assert_eq!(files[0].name, "$CLAUDE_CONFIG_DIR/rules/scoped.md");
        assert_eq!(
            files[0].path_patterns,
            vec!["src/**/*.{rs,toml}", "tests/**/*.rs"]
        );
        assert_eq!(files[0].content, "Scoped rule\n");
    }

    #[test]
    fn path_scoped_user_rule_imports_inherit_the_parent_scope() {
        let temp = tempfile::tempdir().expect("tempdir");
        let config_dir = temp.path().join("claude");
        std::fs::create_dir_all(config_dir.join("shared")).expect("shared directory");
        std::fs::create_dir_all(config_dir.join("rules")).expect("rules directory");
        std::fs::write(
            config_dir.join("rules/rust.md"),
            "---\npaths:\n  - src/**/*.rs\n---\nRust rule\n@../shared/base.md\n",
        )
        .expect("scoped rule");
        std::fs::write(
            config_dir.join("shared/base.md"),
            "Imported base\n@nested.md\n",
        )
        .expect("base import");
        std::fs::write(config_dir.join("shared/nested.md"), "Nested import\n")
            .expect("nested import");
        let options = ClaudeCodeInstructionSourceOptions {
            config_dir: Some(config_dir),
            home_dir: None,
            display_root: "$CLAUDE_CONFIG_DIR".to_string(),
        };

        let files = load_claude_code_user_instructions(&options).expect("sources");
        let rule = files
            .iter()
            .find(|file| file.name.ends_with("/rust.md"))
            .expect("scoped rule");

        assert_eq!(rule.path_patterns, vec!["src/**/*.rs"]);
        assert!(rule.content.contains("Rust rule"));
        assert!(rule.content.contains("Imported base"));
        assert!(rule.content.contains("Nested import"));
        assert!(files.iter().all(|file| {
            !file.name.ends_with("/base.md") && !file.name.ends_with("/nested.md")
        }));
    }

    #[test]
    fn import_containment_normalizes_the_config_root() {
        let root = Path::new("parent/../claude");
        let source = root.join("CLAUDE.md");

        let imports = claude_import_paths(root, None, &source, "@imports/base.md");

        assert_eq!(imports, vec![PathBuf::from("claude/imports/base.md")]);
    }

    #[test]
    fn rules_and_imports_share_one_file_budget() {
        let temp = tempfile::tempdir().expect("tempdir");
        let config_dir = temp.path().join("claude");
        std::fs::create_dir_all(config_dir.join("rules")).expect("rules directory");
        std::fs::create_dir_all(config_dir.join("imports")).expect("imports directory");
        for index in 0..129 {
            std::fs::write(
                config_dir.join(format!("rules/{index:03}.md")),
                format!("Rule {index}\n@../imports/{index:03}.md\n"),
            )
            .expect("rule");
            std::fs::write(
                config_dir.join(format!("imports/{index:03}.md")),
                format!("Import {index}\n"),
            )
            .expect("import");
        }
        let options = ClaudeCodeInstructionSourceOptions {
            config_dir: Some(config_dir),
            home_dir: None,
            display_root: "$CLAUDE_CONFIG_DIR".to_string(),
        };

        let files = load_claude_code_user_instructions(&options).expect("instructions");

        assert_eq!(files.len(), 256);
    }

    #[test]
    fn a_bounded_rules_scan_failure_does_not_discard_user_memory() {
        let temp = tempfile::tempdir().expect("tempdir");
        let config_dir = temp.path().join("claude");
        std::fs::create_dir_all(&config_dir).expect("config directory");
        std::fs::write(config_dir.join("CLAUDE.md"), "User memory\n").expect("memory");
        let mut too_deep = config_dir.join("rules");
        for index in 0..33 {
            too_deep = too_deep.join(format!("level-{index}"));
        }
        std::fs::create_dir_all(&too_deep).expect("deep rules tree");
        let options = ClaudeCodeInstructionSourceOptions {
            config_dir: Some(config_dir),
            home_dir: None,
            display_root: "$CLAUDE_CONFIG_DIR".to_string(),
        };

        let files = load_claude_code_user_instructions(&options).expect("instructions");

        assert_eq!(files.len(), 1);
        assert_eq!(files[0].name, "$CLAUDE_CONFIG_DIR/CLAUDE.md");
    }

    #[test]
    fn imports_accept_home_and_absolute_paths_only_when_they_stay_in_the_config_root() {
        let temp = tempfile::tempdir().expect("tempdir");
        let home = temp.path().join("home");
        let root = home.join(".claude");
        let source = root.join("CLAUDE.md");
        let absolute = root.join("imports/absolute.md");
        let outside = home.join("outside.md");
        let content = format!(
            "@~/.claude/imports/home.md @{} @{}",
            absolute.display(),
            outside.display()
        );

        let imports = claude_import_paths(&root, Some(&home), &source, &content);

        assert_eq!(
            imports,
            vec![
                root.join("imports/home.md"),
                root.join("imports/absolute.md")
            ]
        );
    }

    #[test]
    fn a_missing_config_root_does_not_fall_back_to_the_process_directory() {
        let files = load_claude_code_user_instructions(&ClaudeCodeInstructionSourceOptions {
            config_dir: None,
            home_dir: None,
            display_root: "~/.claude".to_string(),
        })
        .expect("instructions");

        assert!(files.is_empty());
    }
}
