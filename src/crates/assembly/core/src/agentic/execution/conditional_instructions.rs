use crate::agentic::core::{InternalReminderKind, Message, MessageContent};
use crate::agentic::workspace::WorkspaceServices;
use crate::agentic::WorkspaceBinding;
use crate::service::instruction_context::{
    load_local_conditional_instruction_files, load_local_conditional_instruction_files_with_fs,
    load_workspace_conditional_instruction_files_with_fs, render_instruction_documents,
};
use crate::util::errors::OpenBitFunResult;
use log::warn;
use openbitfun_services_core::workspace_instructions::{
    WorkspaceInstructionFile, WorkspaceInstructionPathMatcher,
};
use std::collections::HashSet;

const MAX_CONDITIONAL_INSTRUCTION_FILES: usize = 256;
const MAX_CONDITIONAL_INSTRUCTION_BYTES: usize = 2 * 1024 * 1024;
const CONDITIONAL_INSTRUCTION_HEADER: &str = "## Path-scoped instructions\n\nThe files just read made these instructions applicable. Follow them while working with matching files.\n";

struct CompiledConditionalInstruction {
    file: WorkspaceInstructionFile,
    matcher: WorkspaceInstructionPathMatcher,
}

pub(crate) struct ConditionalInstructionCatalog {
    entries: Vec<CompiledConditionalInstruction>,
}

pub(crate) async fn build_conditional_instruction_reminder(
    workspace: &WorkspaceBinding,
    workspace_services: Option<&WorkspaceServices>,
    read_paths: &[String],
    messages: &[Message],
    turn_id: &str,
    round_id: &str,
) -> OpenBitFunResult<Option<Message>> {
    Ok(
        ConditionalInstructionCatalog::load(workspace, workspace_services)
            .await?
            .build_reminder(read_paths, messages, turn_id, round_id),
    )
}

impl ConditionalInstructionCatalog {
    pub(crate) async fn load(
        workspace: &WorkspaceBinding,
        workspace_services: Option<&WorkspaceServices>,
    ) -> OpenBitFunResult<Self> {
        let files = if workspace.is_remote() {
            let Some(services) = workspace_services else {
                warn!(
                    "Remote conditional instruction discovery skipped because workspace services are unavailable"
                );
                return Ok(Self::from_files(Vec::new()));
            };
            load_workspace_conditional_instruction_files_with_fs(
                services.fs.as_ref(),
                &workspace.root_path_string(),
            )
            .await?
        } else if let Some(services) = workspace_services {
            load_local_conditional_instruction_files_with_fs(
                workspace.root_path(),
                services.fs.as_ref(),
                &workspace.root_path_string(),
            )
            .await?
        } else {
            load_local_conditional_instruction_files(workspace.root_path()).await?
        };
        Ok(Self::from_files(files))
    }

    pub(crate) fn from_files(files: Vec<WorkspaceInstructionFile>) -> Self {
        let mut entries = Vec::new();
        let mut source_names = HashSet::new();
        let mut total_bytes = 0usize;

        for file in files {
            if entries.len() >= MAX_CONDITIONAL_INSTRUCTION_FILES
                || total_bytes.saturating_add(file.content.len())
                    > MAX_CONDITIONAL_INSTRUCTION_BYTES
            {
                break;
            }
            if file.path_patterns.is_empty()
                || file.content.trim().is_empty()
                || !source_names.insert(file.name.clone())
            {
                continue;
            }
            let Some(matcher) =
                WorkspaceInstructionPathMatcher::compile(&file.path_patterns, &file.name)
            else {
                continue;
            };
            total_bytes += file.content.len();
            entries.push(CompiledConditionalInstruction { file, matcher });
        }

        Self { entries }
    }

    pub(crate) fn build_reminder(
        &self,
        read_paths: &[String],
        messages: &[Message],
        turn_id: &str,
        round_id: &str,
    ) -> Option<Message> {
        let active_sources = messages
            .iter()
            .flat_map(Message::activated_instruction_sources)
            .map(String::as_str)
            .collect::<HashSet<_>>();
        let matched_files = self
            .entries
            .iter()
            .filter(|entry| !active_sources.contains(entry.file.name.as_str()))
            .filter(|entry| read_paths.iter().any(|path| entry.matcher.is_match(path)))
            .map(|entry| &entry.file)
            .collect::<Vec<_>>();
        let (content, activated_sources) =
            render_instruction_documents(CONDITIONAL_INSTRUCTION_HEADER, matched_files);
        let content = content?;

        Some(
            Message::internal_reminder(InternalReminderKind::ConditionalInstructions, content)
                .with_turn_id(turn_id.to_string())
                .with_round_id(round_id.to_string())
                .with_activated_instruction_sources(activated_sources),
        )
    }
}

pub(crate) fn successful_workspace_read_paths(
    messages: &[Message],
    workspace: &WorkspaceBinding,
) -> Vec<String> {
    let mut paths = Vec::new();
    let mut seen = HashSet::new();
    for message in messages {
        let MessageContent::ToolResult {
            tool_name,
            effective_tool_name,
            result,
            is_error,
            ..
        } = &message.content
        else {
            continue;
        };
        let effective_name = effective_tool_name.as_deref().unwrap_or(tool_name);
        if *is_error || effective_name != "Read" {
            continue;
        }
        let Some(file_path) = result.get("file_path").and_then(serde_json::Value::as_str) else {
            continue;
        };
        let Some(relative_path) = workspace_relative_path(workspace, file_path) else {
            continue;
        };
        if seen.insert(relative_path.clone()) {
            paths.push(relative_path);
        }
    }
    paths
}

fn workspace_relative_path(workspace: &WorkspaceBinding, file_path: &str) -> Option<String> {
    if file_path.starts_with("openbitfun://") {
        return None;
    }
    let file_path = file_path.trim().replace('\\', "/");
    if !path_is_effectively_absolute(&file_path, workspace.is_remote()) {
        return normalize_relative_read_path(&file_path);
    }

    let root = workspace.root_path_string().replace('\\', "/");
    let root = root.trim_end_matches('/');
    let prefix = format!("{root}/");
    let relative = if cfg!(windows) && !workspace.is_remote() {
        file_path
            .get(..prefix.len())
            .filter(|candidate| candidate.eq_ignore_ascii_case(&prefix))?;
        &file_path[prefix.len()..]
    } else {
        file_path.strip_prefix(&prefix)?
    };
    normalize_relative_read_path(relative)
}

fn path_is_effectively_absolute(path: &str, remote: bool) -> bool {
    if remote {
        return path.starts_with('/');
    }
    std::path::Path::new(path).is_absolute() || has_windows_drive_prefix(path)
}

fn has_windows_drive_prefix(path: &str) -> bool {
    let bytes = path.as_bytes();
    bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':'
}

fn normalize_relative_read_path(path: &str) -> Option<String> {
    let mut components = Vec::new();
    for component in path.split('/') {
        match component {
            "" | "." => {}
            ".." => {
                components.pop()?;
            }
            component => components.push(component),
        }
    }
    (!components.is_empty()).then(|| components.join("/"))
}

#[cfg(test)]
mod tests {
    #[cfg(feature = "external-sources")]
    use super::build_conditional_instruction_reminder;
    use super::{successful_workspace_read_paths, ConditionalInstructionCatalog};
    use crate::agentic::core::{Message, MessageContent};
    use crate::agentic::WorkspaceBinding;
    #[cfg(feature = "external-sources")]
    use crate::instruction_sources::test_support::{lock_environment, EnvironmentGuard};
    use openbitfun_services_core::workspace_instructions::WorkspaceInstructionFile;
    use serde_json::json;

    fn conditional_file(name: &str, patterns: &[&str], content: &str) -> WorkspaceInstructionFile {
        WorkspaceInstructionFile {
            name: name.to_string(),
            content: content.to_string(),
            path_patterns: patterns
                .iter()
                .map(|pattern| (*pattern).to_string())
                .collect(),
        }
    }

    #[test]
    fn catalog_matches_relative_globs_in_source_order_and_deduplicates_active_sources() {
        let catalog = ConditionalInstructionCatalog::from_files(vec![
            conditional_file(
                "~/.claude/rules/typescript.md",
                &["src/**/*.{ts,tsx}"],
                "Use strict types.",
            ),
            conditional_file(
                ".claude/rules/api.md",
                &["src/api/**/*.ts", "tests/**/*.test.ts"],
                "Validate API inputs.",
            ),
            conditional_file(
                ".claude/rules/invalid.md",
                &["../outside/**"],
                "Must never load.",
            ),
        ]);

        let first = catalog
            .build_reminder(
                &["src/api/users/route.ts".to_string()],
                &[],
                "turn-1",
                "round-1",
            )
            .expect("matching reminder");

        assert_eq!(
            first.activated_instruction_sources(),
            &["~/.claude/rules/typescript.md", ".claude/rules/api.md"]
        );
        let MessageContent::Text(text) = &first.content else {
            panic!("conditional instructions must be an internal text reminder");
        };
        assert!(
            text.find("Use strict types.").unwrap() < text.find("Validate API inputs.").unwrap()
        );
        assert!(!text.contains("Must never load."));
        assert!(catalog
            .build_reminder(
                &["src/api/users/route.ts".to_string()],
                &[first],
                "turn-1",
                "round-2",
            )
            .is_none());
        assert!(
            crate::agentic::core::InternalReminderKind::ConditionalInstructions
                .should_drop_during_compaction()
        );
    }

    #[test]
    fn successful_read_paths_ignore_errors_other_tools_and_outside_files() {
        let temp = tempfile::tempdir().expect("tempdir");
        let workspace = WorkspaceBinding::new(None, temp.path().to_path_buf());
        let inside = temp.path().join("src/lib.rs").to_string_lossy().to_string();
        let outside = temp
            .path()
            .with_extension("outside.rs")
            .to_string_lossy()
            .to_string();
        let messages = vec![
            tool_result("Read", None, false, json!({"file_path": inside})),
            tool_result("Grep", None, false, json!({"file_path": "src/other.rs"})),
            tool_result("Read", None, true, json!({"file_path": "src/failed.rs"})),
            tool_result(
                "CallDeferredTool",
                Some("Read"),
                false,
                json!({"file_path": outside}),
            ),
        ];

        assert_eq!(
            successful_workspace_read_paths(&messages, &workspace),
            vec!["src/lib.rs"]
        );
    }

    #[cfg(feature = "external-sources")]
    #[tokio::test]
    async fn an_unmatched_read_does_not_freeze_rule_content_before_activation() {
        let _environment = lock_environment();
        let temp = tempfile::tempdir().expect("tempdir");
        let workspace_root = temp.path().join("workspace");
        let xdg = temp.path().join("xdg");
        let codex = temp.path().join("codex");
        let claude = temp.path().join("claude");
        std::fs::create_dir_all(workspace_root.join(".claude/rules")).expect("workspace rules");
        std::fs::create_dir_all(xdg.join("opencode")).expect("OpenCode config");
        std::fs::create_dir_all(&codex).expect("Codex config");
        std::fs::create_dir_all(&claude).expect("Claude config");
        let rule_path = workspace_root.join(".claude/rules/rust.md");
        std::fs::write(&rule_path, "---\npaths:\n  - src/**/*.rs\n---\nOld rule\n")
            .expect("old rule");
        let _guard = EnvironmentGuard::set(&[
            ("XDG_CONFIG_HOME", &xdg),
            ("CODEX_HOME", &codex),
            ("CLAUDE_CONFIG_DIR", &claude),
        ]);
        let workspace = WorkspaceBinding::new(None, workspace_root);

        assert!(build_conditional_instruction_reminder(
            &workspace,
            None,
            &["README.md".to_string()],
            &[],
            "turn-1",
            "round-1",
        )
        .await
        .expect("unmatched snapshot")
        .is_none());

        std::fs::write(&rule_path, "---\npaths:\n  - src/**/*.rs\n---\nNew rule\n")
            .expect("updated rule");
        let reminder = build_conditional_instruction_reminder(
            &workspace,
            None,
            &["src/lib.rs".to_string()],
            &[],
            "turn-1",
            "round-2",
        )
        .await
        .expect("matching snapshot")
        .expect("matching reminder");

        assert!(reminder.content.to_string().contains("New rule"));
        assert!(!reminder.content.to_string().contains("Old rule"));
    }

    fn tool_result(
        tool_name: &str,
        effective_tool_name: Option<&str>,
        is_error: bool,
        result: serde_json::Value,
    ) -> Message {
        Message {
            id: format!("{tool_name}-result"),
            role: crate::agentic::core::MessageRole::Tool,
            content: MessageContent::ToolResult {
                tool_id: format!("{tool_name}-id"),
                tool_name: tool_name.to_string(),
                effective_tool_name: effective_tool_name.map(str::to_string),
                result,
                result_for_assistant: None,
                is_error,
                image_attachments: None,
            },
            timestamp: std::time::SystemTime::now(),
            metadata: Default::default(),
        }
    }
}
