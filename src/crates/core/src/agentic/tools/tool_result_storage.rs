//! Shared handling for oversized tool results.
//!
//! The model should not receive unbounded tool output. Large outputs are stored
//! under the session runtime directory and replaced, for the assistant only, by
//! a small preview plus a stable reference to the full content.

use crate::agentic::core::ToolResult;
use crate::agentic::tools::framework::ToolUseContext;
use crate::util::errors::{BitFunError, BitFunResult};
use bitfun_agent_tools::GET_TOOL_SPEC_TOOL_NAME;
use log::{debug, warn};
use std::collections::HashSet;
use std::path::Path;

pub(crate) const DEFAULT_MAX_TOOL_RESULT_CHARS: usize = 50_000;
pub(crate) const READ_MAX_TOOL_RESULT_CHARS: usize = 16_000;
pub(crate) const MAX_TOOL_RESULTS_PER_ROUND_CHARS: usize = 200_000;
pub(crate) const TOOL_RESULT_PREVIEW_CHARS: usize = 2_000;
pub(crate) const PERSISTED_OUTPUT_TAG: &str = "<persisted-output>";
pub(crate) const PERSISTED_OUTPUT_CLOSING_TAG: &str = "</persisted-output>";

const READ_TOOL_NAME: &str = "Read";
const BASH_TOOL_NAME: &str = "Bash";
const SHELL_MAX_TOOL_RESULT_CHARS: usize = 30_000;

#[derive(Debug, Clone, Copy)]
pub(crate) struct ToolResultStoragePolicy {
    pub per_tool_limit_chars: usize,
    pub per_round_limit_chars: usize,
    pub preview_chars: usize,
}

impl Default for ToolResultStoragePolicy {
    fn default() -> Self {
        Self {
            per_tool_limit_chars: DEFAULT_MAX_TOOL_RESULT_CHARS,
            per_round_limit_chars: MAX_TOOL_RESULTS_PER_ROUND_CHARS,
            preview_chars: TOOL_RESULT_PREVIEW_CHARS,
        }
    }
}

#[derive(Debug, Clone)]
struct PersistedToolResult {
    reference: String,
    original_chars: usize,
    line_count: usize,
    preview: String,
    has_more: bool,
    metadata: Vec<(String, String)>,
}

#[derive(Debug, Clone)]
struct ToolResultCandidate {
    index: usize,
    visible_chars: usize,
}

pub(crate) async fn maybe_persist_large_tool_result(
    mut result: ToolResult,
    context: &ToolUseContext,
) -> ToolResult {
    let policy = ToolResultStoragePolicy::default();
    if should_skip_tool_result(&result) || visible_content_is_compacted(&result) {
        return result;
    }

    let per_tool_limit = effective_per_tool_limit(&result.tool_name, policy);
    let visible_chars = result_visible_content(&result).chars().count();
    let content_override = content_override_if_oversized(&result, per_tool_limit);
    if visible_chars <= per_tool_limit
        && content_override.is_none()
        && !json_result_is_oversized(&result, per_tool_limit)
    {
        return result;
    }

    match persist_and_render_replacement(&result, context, policy, content_override).await {
        Ok(replacement) => {
            result.result_for_assistant = Some(replacement);
            result
        }
        Err(error) => {
            warn!(
                "Failed to persist oversized tool result: tool_name={}, tool_id={}, error={}",
                result.tool_name, result.tool_id, error
            );
            result
        }
    }
}

pub(crate) async fn apply_round_tool_result_budget(
    mut results: Vec<ToolResult>,
    context: &ToolUseContext,
) -> Vec<ToolResult> {
    let policy = ToolResultStoragePolicy::default();
    let candidates = collect_round_budget_candidates(&results);
    let total_visible_chars = candidates
        .iter()
        .map(|candidate| candidate.visible_chars)
        .sum::<usize>();

    if total_visible_chars <= policy.per_round_limit_chars {
        return results;
    }

    let selected = select_candidates_to_persist(
        &candidates,
        total_visible_chars,
        policy.per_round_limit_chars,
    );
    if selected.is_empty() {
        return results;
    }

    let selected_indices = selected.into_iter().collect::<HashSet<_>>();
    let mut replaced_count = 0usize;
    for (index, result) in results.iter_mut().enumerate() {
        if !selected_indices.contains(&index) {
            continue;
        }

        match persist_and_render_replacement(result, context, policy, None).await {
            Ok(replacement) => {
                result.result_for_assistant = Some(replacement);
                replaced_count += 1;
            }
            Err(error) => {
                warn!(
                    "Failed to persist round-budget tool result: tool_name={}, tool_id={}, error={}",
                    result.tool_name, result.tool_id, error
                );
            }
        }
    }

    if replaced_count > 0 {
        debug!(
            "Round tool result budget enforced: replaced={}, total_visible_chars={}, limit={}",
            replaced_count, total_visible_chars, policy.per_round_limit_chars
        );
    }

    results
}

fn should_skip_tool_result(result: &ToolResult) -> bool {
    result.tool_name == GET_TOOL_SPEC_TOOL_NAME
        || result
            .image_attachments
            .as_ref()
            .is_some_and(|v| !v.is_empty())
}

fn collect_round_budget_candidates(results: &[ToolResult]) -> Vec<ToolResultCandidate> {
    results
        .iter()
        .enumerate()
        .filter(|(_, result)| !should_skip_tool_result(result))
        .filter(|(_, result)| !visible_content_is_compacted(result))
        .map(|(index, result)| ToolResultCandidate {
            index,
            visible_chars: result_visible_content(result).chars().count(),
        })
        .collect()
}

fn select_candidates_to_persist(
    candidates: &[ToolResultCandidate],
    total_visible_chars: usize,
    limit: usize,
) -> Vec<usize> {
    let mut sorted = candidates.to_vec();
    sorted.sort_by(|a, b| b.visible_chars.cmp(&a.visible_chars));

    let mut selected = Vec::new();
    let mut remaining = total_visible_chars;
    for candidate in sorted {
        if remaining <= limit {
            break;
        }
        selected.push(candidate.index);
        remaining = remaining.saturating_sub(candidate.visible_chars);
    }
    selected
}

async fn persist_and_render_replacement(
    result: &ToolResult,
    context: &ToolUseContext,
    policy: ToolResultStoragePolicy,
    content_override: Option<String>,
) -> BitFunResult<String> {
    let persisted =
        persist_tool_result(result, context, policy.preview_chars, content_override).await?;
    Ok(build_persisted_output_message(
        &persisted,
        policy.preview_chars,
    ))
}

async fn persist_tool_result(
    result: &ToolResult,
    context: &ToolUseContext,
    preview_chars: usize,
    content_override: Option<String>,
) -> BitFunResult<PersistedToolResult> {
    let session_id = context.session_id.as_deref().ok_or_else(|| {
        BitFunError::tool("A session id is required to persist tool results".to_string())
    })?;

    let (serialized, is_json) = if let Some(content) = content_override {
        (content, false)
    } else {
        serialize_tool_result_content(result)?
    };
    let file_name = tool_result_file_name(&result.tool_id, is_json);
    let path = context.current_workspace_session_tool_result_path(session_id, &file_name)?;

    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await.map_err(|error| {
            BitFunError::io(format!(
                "Failed to create tool result directory {}: {}",
                parent.display(),
                error
            ))
        })?;
    }

    write_once(&path, &serialized).await?;

    let reference = context
        .build_session_runtime_artifact_reference(
            session_id,
            &format!("tool-results/{}", file_name),
        )
        .unwrap_or_else(|_| path.display().to_string());
    let (preview, has_more) = generate_preview(&serialized, preview_chars);

    debug!(
        "Persisted oversized tool result: tool_name={}, tool_id={}, chars={}, path={}",
        result.tool_name,
        result.tool_id,
        serialized.chars().count(),
        path.display()
    );

    Ok(PersistedToolResult {
        reference,
        original_chars: serialized.chars().count(),
        line_count: count_text_lines(&serialized),
        preview,
        has_more,
        metadata: tool_result_metadata(result),
    })
}

async fn write_once(path: &Path, content: &str) -> BitFunResult<()> {
    match tokio::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(path)
        .await
    {
        Ok(mut file) => {
            use tokio::io::AsyncWriteExt;
            file.write_all(content.as_bytes()).await.map_err(|error| {
                BitFunError::io(format!(
                    "Failed to write tool result file {}: {}",
                    path.display(),
                    error
                ))
            })
        }
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => Ok(()),
        Err(error) => Err(BitFunError::io(format!(
            "Failed to create tool result file {}: {}",
            path.display(),
            error
        ))),
    }
}

fn serialize_tool_result_content(result: &ToolResult) -> BitFunResult<(String, bool)> {
    if let Some(text) = result.result_for_assistant.as_ref() {
        return Ok((text.clone(), false));
    }

    serde_json::to_string_pretty(&result.result)
        .or_else(|_| serde_json::to_string(&result.result))
        .map(|text| (text, true))
        .map_err(|error| {
            BitFunError::serialization(format!("Failed to serialize tool result: {}", error))
        })
}

fn effective_per_tool_limit(tool_name: &str, policy: ToolResultStoragePolicy) -> usize {
    match tool_name {
        READ_TOOL_NAME => READ_MAX_TOOL_RESULT_CHARS,
        BASH_TOOL_NAME => SHELL_MAX_TOOL_RESULT_CHARS,
        _ => policy.per_tool_limit_chars,
    }
}

fn content_override_if_oversized(result: &ToolResult, limit: usize) -> Option<String> {
    if result.tool_name != BASH_TOOL_NAME {
        return None;
    }

    let output = result
        .result
        .get("output")
        .and_then(|value| value.as_str())?;
    (output.chars().count() > limit).then(|| output.to_string())
}

fn json_result_is_oversized(result: &ToolResult, limit: usize) -> bool {
    if result.result_for_assistant.is_some() {
        return false;
    }

    serde_json::to_string_pretty(&result.result)
        .or_else(|_| serde_json::to_string(&result.result))
        .map(|text| text.chars().count() > limit)
        .unwrap_or(false)
}

fn result_visible_content(result: &ToolResult) -> String {
    if let Some(text) = result
        .result_for_assistant
        .as_ref()
        .filter(|text| !text.is_empty())
    {
        return text.clone();
    }

    serde_json::to_string_pretty(&result.result)
        .or_else(|_| serde_json::to_string(&result.result))
        .unwrap_or_else(|_| format!("Tool {} execution completed", result.tool_name))
}

fn visible_content_is_compacted(result: &ToolResult) -> bool {
    result
        .result_for_assistant
        .as_deref()
        .is_some_and(|text| text.starts_with(PERSISTED_OUTPUT_TAG))
}

fn tool_result_file_name(tool_id: &str, is_json: bool) -> String {
    let safe_id = sanitize_file_component(tool_id);
    let ext = if is_json { "json" } else { "txt" };
    format!("{}.{}", safe_id, ext)
}

fn sanitize_file_component(value: &str) -> String {
    let mut sanitized = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_') {
                ch
            } else {
                '_'
            }
        })
        .collect::<String>();

    if sanitized.is_empty() {
        sanitized = uuid::Uuid::new_v4().to_string();
    }
    sanitized
}

fn generate_preview(content: &str, max_chars: usize) -> (String, bool) {
    if content.chars().count() <= max_chars {
        return (content.to_string(), false);
    }

    let prefix = content.chars().take(max_chars).collect::<String>();
    let cut_point = prefix
        .char_indices()
        .filter_map(|(idx, ch)| (ch == '\n').then_some(idx))
        .last()
        .filter(|idx| *idx > prefix.len() / 2)
        .unwrap_or(prefix.len());

    (prefix[..cut_point].to_string(), true)
}

fn count_text_lines(content: &str) -> usize {
    if content.is_empty() {
        0
    } else {
        content.lines().count()
    }
}

fn build_persisted_output_message(result: &PersistedToolResult, preview_chars: usize) -> String {
    let mut message = format!(
        "{PERSISTED_OUTPUT_TAG}\nOutput too large ({} chars). Full output saved to: {}\nLine count: {}\n\nPreview (first {} chars):\n{}",
        result.original_chars, result.reference, result.line_count, preview_chars, result.preview
    );
    if result.has_more {
        message.push_str("\n...\n");
    } else {
        message.push('\n');
    }
    if !result.metadata.is_empty() {
        message.push_str("\nMetadata:\n");
        for (key, value) in &result.metadata {
            message.push_str(&format!("- {}: {}\n", key, value));
        }
    }
    message.push_str(PERSISTED_OUTPUT_CLOSING_TAG);
    message
}

fn tool_result_metadata(result: &ToolResult) -> Vec<(String, String)> {
    let Some(object) = result.result.as_object() else {
        return Vec::new();
    };

    [
        "success",
        "exit_code",
        "timed_out",
        "working_directory",
        "terminal_session_id",
    ]
    .into_iter()
    .filter_map(|key| {
        let value = object.get(key)?;
        let rendered = value
            .as_str()
            .map(str::to_string)
            .unwrap_or_else(|| value.to_string());
        Some((key.to_string(), rendered))
    })
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agentic::tools::ToolRuntimeRestrictions;
    use crate::agentic::WorkspaceBinding;
    use serde_json::json;
    use std::collections::HashMap;
    use std::path::PathBuf;

    fn test_context(root: PathBuf) -> ToolUseContext {
        ToolUseContext {
            tool_call_id: Some("call_1".to_string()),
            agent_type: Some("agent".to_string()),
            session_id: Some("session_1".to_string()),
            dialog_turn_id: Some("turn_1".to_string()),
            workspace: Some(WorkspaceBinding::new(None, root)),
            unlocked_collapsed_tools: Vec::new(),
            custom_data: HashMap::new(),
            computer_use_host: None,
            cancellation_token: None,
            runtime_tool_restrictions: ToolRuntimeRestrictions::default(),
            workspace_services: None,
        }
    }

    fn temp_workspace(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "bitfun-tool-result-storage-{}-{}",
            name,
            uuid::Uuid::new_v4()
        ))
    }

    fn tool_result(tool_id: &str, tool_name: &str, text: String) -> ToolResult {
        ToolResult {
            tool_id: tool_id.to_string(),
            tool_name: tool_name.to_string(),
            result: json!({ "content": text }),
            result_for_assistant: Some(text),
            is_error: false,
            duration_ms: None,
            image_attachments: None,
        }
    }

    fn bash_result(tool_id: &str, output: String, result_for_assistant: String) -> ToolResult {
        ToolResult {
            tool_id: tool_id.to_string(),
            tool_name: "Bash".to_string(),
            result: json!({
                "success": false,
                "output": output,
                "exit_code": 1,
                "timed_out": false,
                "working_directory": "/repo",
                "terminal_session_id": "term_1"
            }),
            result_for_assistant: Some(result_for_assistant),
            is_error: false,
            duration_ms: None,
            image_attachments: None,
        }
    }

    #[tokio::test]
    async fn single_large_result_persists_and_replaces_assistant_text() {
        let root = temp_workspace("single");
        let context = test_context(root.clone());
        let result = tool_result(
            "tool/one",
            "Bash",
            "x".repeat(DEFAULT_MAX_TOOL_RESULT_CHARS + 1),
        );

        let processed = maybe_persist_large_tool_result(result, &context).await;
        let assistant = processed.result_for_assistant.unwrap_or_default();

        assert!(assistant.starts_with(PERSISTED_OUTPUT_TAG));
        assert!(assistant.contains("Full output saved to:"));
        assert!(assistant.contains("Preview"));
        assert!(assistant.len() < DEFAULT_MAX_TOOL_RESULT_CHARS);

        let session_dir = context
            .current_workspace_session_tool_results_dir("session_1")
            .expect("session tool-results dir");
        assert!(session_dir.join("tool_one.txt").exists());

        let _ = std::fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn read_result_is_persisted_when_over_read_limit() {
        let root = temp_workspace("read");
        let context = test_context(root.clone());
        let text = "x".repeat(READ_MAX_TOOL_RESULT_CHARS + 1);
        let result = tool_result("read_1", "Read", text);

        let processed = maybe_persist_large_tool_result(result, &context).await;
        let assistant = processed.result_for_assistant.unwrap_or_default();

        assert!(assistant.starts_with(PERSISTED_OUTPUT_TAG));
        assert!(assistant.contains("Full output saved to:"));
        let session_dir = context
            .current_workspace_session_tool_results_dir("session_1")
            .expect("session tool-results dir");
        assert!(session_dir.join("read_1.txt").exists());

        let _ = std::fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn read_result_stays_inline_when_under_read_limit() {
        let root = temp_workspace("read-inline");
        let context = test_context(root.clone());
        let text = "x".repeat(READ_MAX_TOOL_RESULT_CHARS);
        let result = tool_result("read_1", "Read", text.clone());

        let processed = maybe_persist_large_tool_result(result, &context).await;

        assert_eq!(
            processed.result_for_assistant.as_deref(),
            Some(text.as_str())
        );
        let session_dir = context
            .current_workspace_session_tool_results_dir("session_1")
            .expect("session tool-results dir");
        assert!(!session_dir.join("read_1.txt").exists());

        let _ = std::fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn get_tool_spec_result_is_not_persisted_even_when_large() {
        let root = temp_workspace("get-tool-spec");
        let context = test_context(root.clone());
        let text = "x".repeat(DEFAULT_MAX_TOOL_RESULT_CHARS + 1);
        let result = tool_result("get_tool_spec_1", GET_TOOL_SPEC_TOOL_NAME, text.clone());

        let processed = maybe_persist_large_tool_result(result, &context).await;

        assert_eq!(
            processed.result_for_assistant.as_deref(),
            Some(text.as_str())
        );
        let session_dir = context
            .current_workspace_session_tool_results_dir("session_1")
            .expect("session tool-results dir");
        assert!(!session_dir.join("get_tool_spec_1.txt").exists());

        let _ = std::fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn bash_full_output_persists_even_when_assistant_text_is_already_truncated() {
        let root = temp_workspace("bash");
        let context = test_context(root.clone());
        let full_output = format!(
            "{}\nfinal-error",
            "x".repeat(SHELL_MAX_TOOL_RESULT_CHARS + 1)
        );
        let result = bash_result(
            "bash_1",
            full_output.clone(),
            "<output truncated=\"true\">tail only</output>".to_string(),
        );

        let processed = maybe_persist_large_tool_result(result, &context).await;
        let assistant = processed.result_for_assistant.unwrap_or_default();

        assert!(assistant.starts_with(PERSISTED_OUTPUT_TAG));
        assert!(assistant.contains("exit_code: 1"));
        assert!(assistant.contains("working_directory: /repo"));
        assert!(assistant.contains("Line count: 2"));
        let output_path = context
            .current_workspace_session_tool_result_path("session_1", "bash_1.txt")
            .expect("tool result path");
        let saved = std::fs::read_to_string(output_path).expect("saved output");
        assert_eq!(saved, full_output);

        let _ = std::fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn round_budget_persists_largest_results_including_read() {
        let root = temp_workspace("round");
        let context = test_context(root.clone());
        let read = tool_result("read_1", "Read", "a".repeat(170_000));
        let medium = tool_result("medium_1", "WebFetch", "b".repeat(60_000));
        let bash = tool_result("bash_1", "Bash", "c".repeat(30_000));

        let processed =
            apply_round_tool_result_budget(vec![read, medium, bash], &context).await;

        assert!(processed[0]
            .result_for_assistant
            .as_deref()
            .unwrap_or_default()
            .starts_with(PERSISTED_OUTPUT_TAG));
        assert!(!processed[1]
            .result_for_assistant
            .as_deref()
            .unwrap_or_default()
            .starts_with(PERSISTED_OUTPUT_TAG));
        assert!(!processed[2]
            .result_for_assistant
            .as_deref()
            .unwrap_or_default()
            .starts_with(PERSISTED_OUTPUT_TAG));

        let session_dir = context
            .current_workspace_session_tool_results_dir("session_1")
            .expect("session tool-results dir");
        assert!(session_dir.join("read_1.txt").exists());
        assert!(!session_dir.join("medium_1.txt").exists());
        assert!(!session_dir.join("bash_1.txt").exists());

        let _ = std::fs::remove_dir_all(root);
    }
}
