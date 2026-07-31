use bitfun_services_core::bounded_fs::{
    collect_bounded_regular_files_with_prune, BoundedDirectoryWalkError, BoundedDirectoryWalkLimits,
};
use bitfun_services_core::local_instructions::{
    local_instruction_path_exists, read_local_instruction_file, read_local_text_file,
    LocalInstructionFile, LocalInstructionFiles, MAX_LOCAL_INSTRUCTION_FILES,
};
use globset::{GlobBuilder, GlobMatcher};
use serde_json::Value;
use std::path::{Component, Path, PathBuf};

use bitfun_services_core::jsonc::strip_jsonc;

#[derive(Debug, Clone)]
pub struct OpenCodeInstructionSourceOptions {
    pub config_dir: Option<PathBuf>,
    pub home_dir: Option<PathBuf>,
    pub workspace_root: Option<PathBuf>,
    pub display_root: String,
}

impl OpenCodeInstructionSourceOptions {
    pub fn from_environment() -> Self {
        let home_dir = dirs::home_dir().filter(|path| path.is_absolute());
        let (config_dir, display_root) = match std::env::var_os("XDG_CONFIG_HOME") {
            Some(value) => {
                let path = PathBuf::from(value);
                (
                    path.is_absolute().then(|| path.join("opencode")),
                    "$XDG_CONFIG_HOME/opencode".to_string(),
                )
            }
            None => (
                home_dir
                    .as_deref()
                    .map(|home| home.join(".config/opencode")),
                "~/.config/opencode".to_string(),
            ),
        };
        Self {
            config_dir,
            home_dir,
            workspace_root: None,
            display_root,
        }
    }
}

impl Default for OpenCodeInstructionSourceOptions {
    fn default() -> Self {
        Self::from_environment()
    }
}

pub fn load_opencode_user_instructions(
    options: &OpenCodeInstructionSourceOptions,
) -> Result<Vec<LocalInstructionFile>, String> {
    let Some(config_dir) = options.config_dir.as_deref() else {
        return Ok(Vec::new());
    };
    let mut files = LocalInstructionFiles::default();
    let agents_path = config_dir.join("AGENTS.md");
    if local_instruction_path_exists(&agents_path)? {
        files.extend(read_local_instruction_file(
            &agents_path,
            config_dir,
            format!("{}/AGENTS.md", options.display_root),
        )?);
    } else if let Some(home_dir) = options.home_dir.as_deref() {
        files.extend(read_local_instruction_file(
            &home_dir.join(".claude/CLAUDE.md"),
            home_dir,
            "~/.claude/CLAUDE.md",
        )?);
    }

    let mut configured_instructions = None;
    for config_name in ["config.json", "opencode.json", "opencode.jsonc"] {
        let Some(config) = read_local_text_file(
            &config_dir.join(config_name),
            config_dir,
            format!("{}/{config_name}", options.display_root),
        )?
        else {
            continue;
        };
        let value =
            serde_json::from_str::<Value>(&strip_jsonc(&config.content)).map_err(|error| {
                format!("Failed to parse OpenCode user config {config_name}: {error}")
            })?;
        if let Some(value) = value.get("instructions") {
            let instructions = value.as_array().ok_or_else(|| {
                format!("OpenCode user config {config_name} instructions must be an array")
            })?;
            configured_instructions = Some(
                instructions
                    .iter()
                    .map(|value| {
                        value.as_str().map(str::to_string).ok_or_else(|| {
                            format!(
                                "OpenCode user config {config_name} instructions must contain strings"
                            )
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()?,
            );
        }
    }

    if let Some(configured_instructions) = configured_instructions {
        for raw in configured_instructions
            .into_iter()
            .take(MAX_LOCAL_INSTRUCTION_FILES)
        {
            if raw.starts_with("http://") || raw.starts_with("https://") {
                continue;
            }
            if let Some(relative) = raw.strip_prefix("~/") {
                let Some(home_dir) = options.home_dir.as_deref() else {
                    continue;
                };
                append_configured_path(&mut files, home_dir, relative, "~")?;
                continue;
            }
            let path = Path::new(&raw);
            if path.is_absolute() {
                if has_glob_meta(&raw) {
                    let Some((root, pattern)) = split_absolute_glob(path) else {
                        continue;
                    };
                    append_configured_path(&mut files, &root, &pattern, "<configured-path>")?;
                    continue;
                }
                let Some(parent) = path.parent() else {
                    continue;
                };
                let file_name = path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("instruction");
                if let Some(file) = read_local_instruction_file(
                    path,
                    parent,
                    format!("<configured-path>/{file_name}"),
                )? {
                    files.push(file);
                }
                continue;
            }
            let Some(workspace_root) = options.workspace_root.as_deref() else {
                continue;
            };
            append_configured_path(&mut files, workspace_root, &raw, "<workspace>")?;
        }
    }
    Ok(files.into_files())
}

fn append_configured_path(
    files: &mut LocalInstructionFiles,
    root: &Path,
    raw: &str,
    display_root: &str,
) -> Result<(), String> {
    if files.is_at_capacity() {
        return Ok(());
    }
    let Some(normalized) = normalize_relative_pattern(raw) else {
        return Ok(());
    };
    if !has_glob_meta(&normalized) {
        if let Some(file) = read_local_instruction_file(
            &root.join(&normalized),
            root,
            format!("{display_root}/{}", normalized.replace('\\', "/")),
        )? {
            files.push(file);
        }
        return Ok(());
    }

    let matcher = GlobBuilder::new(&normalized.replace('\\', "/"))
        .literal_separator(true)
        .backslash_escape(false)
        .build()
        .map_err(|error| format!("Invalid OpenCode instruction glob {raw}: {error}"))?
        .compile_matcher();
    let prefix = glob_static_prefix(&normalized);
    let scan_root = if prefix.as_os_str().is_empty() {
        root.to_path_buf()
    } else {
        root.join(prefix)
    };
    let directory_matchers = glob_directory_matchers(&normalized)
        .map_err(|error| format!("Invalid OpenCode instruction glob {raw}: {error}"))?;
    let prune_root = root.to_path_buf();
    let matches = match collect_bounded_regular_files_with_prune(
        &scan_root,
        BoundedDirectoryWalkLimits {
            max_depth: 32,
            max_entries: 4096,
            max_directories: 1024,
            max_files: 4096,
        },
        |path| {
            should_descend_instruction_glob(path)
                && directory_matchers.as_ref().map_or(true, |matchers| {
                    path.strip_prefix(&prune_root).ok().is_some_and(|relative| {
                        let depth = relative.components().count();
                        matchers
                            .get(depth.saturating_sub(1))
                            .is_some_and(|matcher| {
                                matcher.is_match(relative.to_string_lossy().replace('\\', "/"))
                            })
                    })
                })
        },
        |path| {
            path.strip_prefix(root).ok().is_some_and(|relative| {
                matcher.is_match(relative.to_string_lossy().replace('\\', "/"))
            })
        },
    ) {
        Ok(matches) => matches,
        Err(BoundedDirectoryWalkError::LimitExceeded(_)) => return Ok(()),
        Err(error) => {
            return Err(format!(
                "Failed to expand OpenCode instruction glob {raw}: {error}"
            ));
        }
    };
    for path in matches {
        let relative = path
            .strip_prefix(root)
            .expect("bounded glob result stays below its root")
            .to_string_lossy()
            .replace('\\', "/");
        if let Some(file) =
            read_local_instruction_file(&path, root, format!("{display_root}/{relative}"))?
        {
            files.push(file);
        }
        if files.is_at_capacity() {
            break;
        }
    }
    Ok(())
}

fn should_descend_instruction_glob(path: &Path) -> bool {
    !path
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| matches!(name, ".git" | ".hg" | ".svn" | "node_modules" | "target"))
}

fn split_absolute_glob(path: &Path) -> Option<(PathBuf, String)> {
    let mut root = PathBuf::new();
    let mut pattern = Vec::new();
    let mut found_glob = false;
    for component in path.components() {
        let value = component.as_os_str().to_string_lossy();
        if found_glob || has_glob_meta(&value) {
            found_glob = true;
            pattern.push(value.into_owned());
        } else {
            root.push(component.as_os_str());
        }
    }
    (found_glob && !root.as_os_str().is_empty()).then(|| (root, pattern.join("/")))
}

fn normalize_relative_pattern(raw: &str) -> Option<String> {
    let path = Path::new(raw.trim());
    if path.as_os_str().is_empty()
        || path.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return None;
    }
    Some(
        path.components()
            .filter_map(|component| match component {
                Component::Normal(value) => Some(value.to_string_lossy()),
                Component::CurDir => None,
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("/"),
    )
}

fn has_glob_meta(value: &str) -> bool {
    value.contains(['*', '?', '[', '{'])
}

fn glob_static_prefix(pattern: &str) -> PathBuf {
    pattern
        .split('/')
        .take_while(|component| !has_glob_meta(component))
        .collect()
}

fn glob_directory_matchers(pattern: &str) -> Result<Option<Vec<GlobMatcher>>, globset::Error> {
    let components = pattern.split('/').collect::<Vec<_>>();
    let directory_components = &components[..components.len().saturating_sub(1)];
    if directory_components
        .iter()
        .any(|component| component.contains("**"))
    {
        return Ok(None);
    }
    let mut prefix = Vec::new();
    let mut matchers = Vec::with_capacity(directory_components.len());
    for component in directory_components {
        prefix.push(*component);
        matchers.push(
            GlobBuilder::new(&prefix.join("/"))
                .literal_separator(true)
                .backslash_escape(false)
                .build()?
                .compile_matcher(),
        );
    }
    Ok(Some(matchers))
}

#[cfg(test)]
mod tests {
    use super::{load_opencode_user_instructions, OpenCodeInstructionSourceOptions};

    #[test]
    fn global_config_uses_native_override_order_and_rejects_urls() {
        let temp = tempfile::tempdir().expect("tempdir");
        let config_dir = temp.path().join("config");
        let home_dir = temp.path().join("home");
        std::fs::create_dir_all(&config_dir).expect("config directory");
        std::fs::create_dir_all(&home_dir).expect("home directory");
        std::fs::write(config_dir.join("AGENTS.md"), "OpenCode base\n").expect("AGENTS");
        for name in ["from-config.md", "from-json.md", "from-jsonc.md"] {
            std::fs::write(home_dir.join(name), format!("{name}\n")).expect("instruction file");
        }
        std::fs::write(
            config_dir.join("config.json"),
            r#"{"instructions":["~/from-config.md"]}"#,
        )
        .expect("config.json");
        std::fs::write(
            config_dir.join("opencode.json"),
            r#"{"instructions":["~/from-json.md"]}"#,
        )
        .expect("opencode.json");
        std::fs::write(
            config_dir.join("opencode.jsonc"),
            r#"{
              // Later global config files override array fields.
              "instructions": ["~/from-jsonc.md", "https://example.invalid/rules.md"],
            }"#,
        )
        .expect("opencode.jsonc");
        let options = OpenCodeInstructionSourceOptions {
            config_dir: Some(config_dir),
            home_dir: Some(home_dir),
            workspace_root: None,
            display_root: "$XDG_CONFIG_HOME/opencode".to_string(),
        };

        let files = load_opencode_user_instructions(&options).expect("instructions");
        let names = files
            .iter()
            .map(|file| file.name.as_str())
            .collect::<Vec<_>>();

        assert_eq!(
            names,
            vec!["$XDG_CONFIG_HOME/opencode/AGENTS.md", "~/from-jsonc.md"]
        );
        assert!(!files.iter().any(|file| file.name.starts_with("http")));
    }

    #[test]
    fn configured_local_paths_resolve_exact_absolute_and_workspace_glob_sources() {
        let temp = tempfile::tempdir().expect("tempdir");
        let config_dir = temp.path().join("config");
        let home_dir = temp.path().join("home");
        let workspace = temp.path().join("workspace");
        let outside = temp.path().join("explicit.md");
        std::fs::create_dir_all(&config_dir).expect("config directory");
        std::fs::create_dir_all(&home_dir).expect("home directory");
        std::fs::create_dir_all(workspace.join("rules/nested")).expect("rules directory");
        std::fs::write(workspace.join("rules/b.md"), "b\n").expect("b rule");
        std::fs::write(workspace.join("rules/a.md"), "a\n").expect("a rule");
        std::fs::write(workspace.join("rules/nested/c.md"), "c\n").expect("nested rule");
        std::fs::write(workspace.join("exact.txt"), "exact\n").expect("exact instruction");
        std::fs::write(&outside, "explicit\n").expect("absolute instruction");
        let config = serde_json::json!({
            "instructions": [
                "rules/*.md",
                "exact.txt",
                outside.to_string_lossy(),
                "https://example.invalid/rules.md"
            ]
        });
        std::fs::write(
            config_dir.join("opencode.json"),
            serde_json::to_vec(&config).expect("serialized config"),
        )
        .expect("opencode.json");
        let options = OpenCodeInstructionSourceOptions {
            config_dir: Some(config_dir),
            home_dir: Some(home_dir),
            workspace_root: Some(workspace),
            display_root: "$XDG_CONFIG_HOME/opencode".to_string(),
        };

        let files = load_opencode_user_instructions(&options).expect("instructions");
        let names = files
            .iter()
            .map(|file| file.name.as_str())
            .collect::<Vec<_>>();

        assert_eq!(
            names,
            vec![
                "<workspace>/rules/a.md",
                "<workspace>/rules/b.md",
                "<workspace>/exact.txt",
                "<configured-path>/explicit.md",
            ]
        );
    }

    #[test]
    fn an_existing_empty_global_agents_file_suppresses_the_claude_fallback() {
        let temp = tempfile::tempdir().expect("tempdir");
        let config_dir = temp.path().join("config");
        let home_dir = temp.path().join("home");
        std::fs::create_dir_all(&config_dir).expect("config directory");
        std::fs::create_dir_all(home_dir.join(".claude")).expect("Claude directory");
        std::fs::write(config_dir.join("AGENTS.md"), "").expect("empty AGENTS");
        std::fs::write(
            home_dir.join(".claude/CLAUDE.md"),
            "fallback must stay suppressed\n",
        )
        .expect("Claude fallback");
        let options = OpenCodeInstructionSourceOptions {
            config_dir: Some(config_dir),
            home_dir: Some(home_dir),
            workspace_root: None,
            display_root: "$XDG_CONFIG_HOME/opencode".to_string(),
        };

        let files = load_opencode_user_instructions(&options).expect("instructions");

        assert!(files.is_empty());
    }

    #[test]
    fn configured_absolute_glob_is_bounded_to_its_declared_directory() {
        let temp = tempfile::tempdir().expect("tempdir");
        let config_dir = temp.path().join("config");
        let home_dir = temp.path().join("home");
        let external = temp.path().join("external");
        std::fs::create_dir_all(&config_dir).expect("config directory");
        std::fs::create_dir_all(&home_dir).expect("home directory");
        std::fs::create_dir_all(&external).expect("external directory");
        std::fs::create_dir_all(external.join("nested")).expect("nested directory");
        std::fs::write(external.join("b.md"), "b\n").expect("b rule");
        std::fs::write(external.join("a.md"), "a\n").expect("a rule");
        std::fs::write(external.join("nested/c.md"), "c\n").expect("nested rule");
        std::fs::write(external.join("ignored.txt"), "ignored\n").expect("ignored file");
        let pattern = external.join("**/*.md").to_string_lossy().to_string();
        std::fs::write(
            config_dir.join("opencode.json"),
            serde_json::to_vec(&serde_json::json!({ "instructions": [pattern] }))
                .expect("serialized config"),
        )
        .expect("OpenCode config");
        let options = OpenCodeInstructionSourceOptions {
            config_dir: Some(config_dir),
            home_dir: Some(home_dir),
            workspace_root: None,
            display_root: "$XDG_CONFIG_HOME/opencode".to_string(),
        };

        let files = load_opencode_user_instructions(&options).expect("instructions");
        let names = files
            .iter()
            .map(|file| file.name.as_str())
            .collect::<Vec<_>>();

        assert_eq!(
            names,
            vec![
                "<configured-path>/a.md",
                "<configured-path>/b.md",
                "<configured-path>/nested/c.md",
            ]
        );
    }

    #[test]
    fn an_existing_empty_config_is_reported_instead_of_treated_as_absent() {
        let temp = tempfile::tempdir().expect("tempdir");
        let config_dir = temp.path().join("config");
        std::fs::create_dir_all(&config_dir).expect("config directory");
        std::fs::write(config_dir.join("opencode.json"), "").expect("empty config");
        let options = OpenCodeInstructionSourceOptions {
            config_dir: Some(config_dir),
            home_dir: None,
            workspace_root: None,
            display_root: "$XDG_CONFIG_HOME/opencode".to_string(),
        };

        let error = load_opencode_user_instructions(&options).expect_err("invalid empty config");

        assert!(error.contains("Failed to parse OpenCode user config opencode.json"));
    }

    #[test]
    fn a_missing_config_root_does_not_fall_back_to_the_process_directory() {
        let files = load_opencode_user_instructions(&OpenCodeInstructionSourceOptions {
            config_dir: None,
            home_dir: None,
            workspace_root: None,
            display_root: "~/.config/opencode".to_string(),
        })
        .expect("instructions");

        assert!(files.is_empty());
    }

    #[test]
    fn broad_workspace_globs_skip_repository_and_dependency_directories() {
        let temp = tempfile::tempdir().expect("tempdir");
        let config_dir = temp.path().join("config");
        let workspace = temp.path().join("workspace");
        std::fs::create_dir_all(&config_dir).expect("config directory");
        for directory in ["docs", ".git", "node_modules/pkg", "target/debug"] {
            std::fs::create_dir_all(workspace.join(directory)).expect("workspace directory");
        }
        std::fs::write(workspace.join("docs/team.md"), "visible\n").expect("visible rule");
        std::fs::write(workspace.join(".git/private.md"), "git internals\n").expect("git rule");
        std::fs::write(
            workspace.join("node_modules/pkg/dependency.md"),
            "dependency\n",
        )
        .expect("dependency rule");
        std::fs::write(workspace.join("target/debug/output.md"), "build output\n")
            .expect("build rule");
        std::fs::write(
            config_dir.join("opencode.json"),
            r#"{"instructions":["**/*.md"]}"#,
        )
        .expect("OpenCode config");
        let options = OpenCodeInstructionSourceOptions {
            config_dir: Some(config_dir),
            home_dir: None,
            workspace_root: Some(workspace),
            display_root: "$XDG_CONFIG_HOME/opencode".to_string(),
        };

        let files = load_opencode_user_instructions(&options).expect("instructions");
        let names = files
            .iter()
            .map(|file| file.name.as_str())
            .collect::<Vec<_>>();

        assert_eq!(names, vec!["<workspace>/docs/team.md"]);
    }

    #[test]
    fn a_non_recursive_glob_does_not_scan_unrelated_descendants() {
        let temp = tempfile::tempdir().expect("tempdir");
        let config_dir = temp.path().join("config");
        let workspace = temp.path().join("workspace");
        std::fs::create_dir_all(&config_dir).expect("config directory");
        std::fs::create_dir_all(&workspace).expect("workspace directory");
        std::fs::write(workspace.join("root.md"), "root\n").expect("root rule");
        let mut unrelated = workspace.join("unrelated");
        for index in 0..33 {
            unrelated = unrelated.join(format!("level-{index}"));
        }
        std::fs::create_dir_all(&unrelated).expect("deep unrelated tree");
        std::fs::write(
            config_dir.join("opencode.json"),
            r#"{"instructions":["*.md"]}"#,
        )
        .expect("OpenCode config");
        let options = OpenCodeInstructionSourceOptions {
            config_dir: Some(config_dir),
            home_dir: None,
            workspace_root: Some(workspace),
            display_root: "$XDG_CONFIG_HOME/opencode".to_string(),
        };

        let files = load_opencode_user_instructions(&options).expect("instructions");

        assert_eq!(files.len(), 1);
        assert_eq!(files[0].name, "<workspace>/root.md");
    }

    #[test]
    fn wildcard_directory_components_prune_non_matching_sibling_trees() {
        let temp = tempfile::tempdir().expect("tempdir");
        let config_dir = temp.path().join("config");
        let workspace = temp.path().join("workspace");
        std::fs::create_dir_all(&config_dir).expect("config directory");
        std::fs::create_dir_all(workspace.join("team-alpha")).expect("matching directory");
        std::fs::write(workspace.join("team-alpha/rule.md"), "matching\n").expect("matching rule");
        let unrelated = workspace.join("vendor");
        std::fs::create_dir_all(&unrelated).expect("unrelated sibling tree");
        for index in 0..4097 {
            std::fs::write(unrelated.join(format!("entry-{index}.txt")), "")
                .expect("unrelated entry");
        }
        std::fs::write(
            config_dir.join("opencode.json"),
            r#"{"instructions":["team-*/*.md"]}"#,
        )
        .expect("OpenCode config");
        let options = OpenCodeInstructionSourceOptions {
            config_dir: Some(config_dir),
            home_dir: None,
            workspace_root: Some(workspace),
            display_root: "$XDG_CONFIG_HOME/opencode".to_string(),
        };

        let files = load_opencode_user_instructions(&options).expect("instructions");

        assert_eq!(files.len(), 1);
        assert_eq!(files[0].name, "<workspace>/team-alpha/rule.md");
    }

    #[test]
    fn a_bounded_glob_failure_does_not_discard_existing_global_instructions() {
        let temp = tempfile::tempdir().expect("tempdir");
        let config_dir = temp.path().join("config");
        let workspace = temp.path().join("workspace");
        std::fs::create_dir_all(&config_dir).expect("config directory");
        std::fs::write(config_dir.join("AGENTS.md"), "global\n").expect("global rule");
        let mut too_deep = workspace.join("docs");
        for index in 0..33 {
            too_deep = too_deep.join(format!("level-{index}"));
        }
        std::fs::create_dir_all(&too_deep).expect("deep instruction tree");
        std::fs::write(
            config_dir.join("opencode.json"),
            r#"{"instructions":["**/*.md"]}"#,
        )
        .expect("OpenCode config");
        let options = OpenCodeInstructionSourceOptions {
            config_dir: Some(config_dir),
            home_dir: None,
            workspace_root: Some(workspace),
            display_root: "$XDG_CONFIG_HOME/opencode".to_string(),
        };

        let files = load_opencode_user_instructions(&options).expect("instructions");

        assert_eq!(files.len(), 1);
        assert_eq!(files[0].name, "$XDG_CONFIG_HOME/opencode/AGENTS.md");
    }

    #[test]
    fn adapter_enforces_the_shared_file_and_total_byte_budgets() {
        let temp = tempfile::tempdir().expect("tempdir");
        let config_dir = temp.path().join("config");
        let workspace = temp.path().join("workspace");
        std::fs::create_dir_all(&config_dir).expect("config directory");
        std::fs::create_dir_all(&workspace).expect("workspace directory");
        for index in 0..257 {
            let name = format!("rules/{index:03}.md");
            let path = workspace.join(&name);
            std::fs::create_dir_all(path.parent().expect("rule parent")).expect("rule directory");
            std::fs::write(path, "instruction\n").expect("rule");
        }
        std::fs::write(
            config_dir.join("opencode.json"),
            serde_json::to_vec(&serde_json::json!({ "instructions": ["rules/*.md"] }))
                .expect("serialized config"),
        )
        .expect("OpenCode config");
        let options = OpenCodeInstructionSourceOptions {
            config_dir: Some(config_dir),
            home_dir: None,
            workspace_root: Some(workspace),
            display_root: "$XDG_CONFIG_HOME/opencode".to_string(),
        };

        let files = load_opencode_user_instructions(&options).expect("instructions");

        assert_eq!(files.len(), 256);

        let large_config_dir = temp.path().join("large-config");
        let large_workspace = temp.path().join("large-workspace");
        std::fs::create_dir_all(&large_config_dir).expect("large config directory");
        std::fs::create_dir_all(&large_workspace).expect("large workspace directory");
        for index in 0..3 {
            std::fs::write(
                large_workspace.join(format!("large-{index}.md")),
                vec![b'x'; 1024 * 1024],
            )
            .expect("large rule");
        }
        std::fs::write(
            large_config_dir.join("opencode.json"),
            r#"{"instructions":["large-0.md","large-1.md","large-2.md"]}"#,
        )
        .expect("large OpenCode config");
        let large_options = OpenCodeInstructionSourceOptions {
            config_dir: Some(large_config_dir),
            home_dir: None,
            workspace_root: Some(large_workspace),
            display_root: "$XDG_CONFIG_HOME/opencode".to_string(),
        };

        let large_files = load_opencode_user_instructions(&large_options).expect("large files");

        assert_eq!(large_files.len(), 2);
    }
}
