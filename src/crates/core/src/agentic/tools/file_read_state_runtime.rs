//! Runtime helpers for session-scoped file read state used by Read/Edit/Write tools.

use crate::agentic::coordination::get_global_coordinator;
use crate::agentic::session::FileReadState;
use crate::agentic::tools::framework::{ToolPathResolution, ToolUseContext};
use crate::util::errors::BitFunResult;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};
use tool_runtime::fs::read_file::ReadFileResult;
use tool_runtime::util::read_line_prefix::read_tool_output_to_file_content;
use tool_runtime::util::string::normalize_string;

pub fn record_file_read_state(
    context: &ToolUseContext,
    resolved: &ToolPathResolution,
    read_result: &ReadFileResult,
    requested_start_line: usize,
    requested_limit: usize,
    timestamp_ms: u64,
) {
    let Some(session_id) = context.session_id.as_deref() else {
        return;
    };
    let Some(coordinator) = get_global_coordinator() else {
        return;
    };

    let is_partial_view = requested_start_line != 1
        || read_result.end_line < read_result.total_lines
        || read_result.hit_total_char_limit
        || requested_limit < read_result.total_lines;

    let state = FileReadState {
        content: read_tool_output_to_file_content(&read_result.content),
        timestamp_ms,
        start_line: read_result.start_line,
        end_line: read_result.end_line,
        total_lines: read_result.total_lines,
        is_partial_view,
    };

    coordinator.get_session_manager().set_file_read_state(
        session_id,
        &resolved.logical_path,
        state,
    );
}

pub async fn validate_edit_against_read_state(
    context: &ToolUseContext,
    resolved: &ToolPathResolution,
) -> Option<String> {
    let session_id = context.session_id.as_deref()?;
    let coordinator = get_global_coordinator()?;
    let read_state = coordinator
        .get_session_manager()
        .get_file_read_state(session_id, &resolved.logical_path)?;

    if read_state.is_partial_view {
        return Some(format!(
            "File {} was only partially read (lines {}-{} of {}). Read the full target area again before editing.",
            resolved.logical_path,
            read_state.start_line,
            read_state.end_line,
            read_state.total_lines
        ));
    }

    let current_content = match read_current_file_content(context, resolved).await {
        Ok(content) => content,
        Err(error) => {
            return Some(format!(
                "File {} could not be re-read before editing ({}). Read it again when the workspace is available.",
                resolved.logical_path, error
            ));
        }
    };
    let current_mtime_ms = file_modification_time_ms(context, resolved).await;

    validate_content_freshness_against_read_state(
        &resolved.logical_path,
        &read_state,
        &current_content,
        current_mtime_ms,
    )
}

fn validate_content_freshness_against_read_state(
    logical_path: &str,
    read_state: &FileReadState,
    current_content: &str,
    current_mtime_ms: Option<u64>,
) -> Option<String> {
    if let Some(current_mtime_ms) = current_mtime_ms {
        if current_mtime_ms > read_state.timestamp_ms {
            if read_state.is_full_file_read()
                && normalize_string(current_content) == normalize_string(&read_state.content)
            {
                return None;
            }

            return Some(format!(
                "File {} has been modified since it was last read (by the user, another tool, or a linter). Read it again before editing.",
                logical_path
            ));
        }
    } else if normalize_string(current_content) != normalize_string(&read_state.content) {
        return Some(format!(
            "File {} no longer matches the last Read result. Read it again before editing.",
            logical_path
        ));
    }

    None
}

pub fn validate_edit_has_prior_read(
    context: &ToolUseContext,
    resolved: &ToolPathResolution,
) -> Option<String> {
    let session_id = context.session_id.as_deref()?;
    let coordinator = get_global_coordinator()?;
    let has_read = coordinator
        .get_session_manager()
        .get_file_read_state(session_id, &resolved.logical_path)
        .is_some();

    if has_read {
        return None;
    }

    Some(format!(
        "File {} has not been read yet in this session. Use the Read tool on it before editing.",
        resolved.logical_path
    ))
}

pub fn update_file_read_state_after_mutation(
    context: &ToolUseContext,
    resolved: &ToolPathResolution,
    content: &str,
    timestamp_ms: u64,
) {
    let Some(session_id) = context.session_id.as_deref() else {
        return;
    };
    let Some(coordinator) = get_global_coordinator() else {
        return;
    };

    let line_count = content.lines().count();
    let (start_line, end_line) = if line_count == 0 {
        (0, 0)
    } else {
        (1, line_count)
    };
    let state = FileReadState {
        content: content.to_string(),
        timestamp_ms,
        start_line,
        end_line,
        total_lines: line_count,
        is_partial_view: false,
    };

    coordinator.get_session_manager().set_file_read_state(
        session_id,
        &resolved.logical_path,
        state,
    );
}

async fn read_current_file_content(
    context: &ToolUseContext,
    resolved: &ToolPathResolution,
) -> BitFunResult<String> {
    if resolved.uses_remote_workspace_backend() {
        let ws_fs = context.ws_fs().ok_or_else(|| {
            crate::util::errors::BitFunError::tool(
                "Remote workspace file system is unavailable".to_string(),
            )
        })?;
        ws_fs
            .read_file_text(&resolved.resolved_path)
            .await
            .map_err(|error| {
                crate::util::errors::BitFunError::tool(format!("Failed to read file: {}", error))
            })
    } else {
        std::fs::read_to_string(&resolved.resolved_path).map_err(|error| {
            crate::util::errors::BitFunError::tool(format!(
                "Failed to read file {}: {}",
                resolved.logical_path, error
            ))
        })
    }
}

async fn file_modification_time_ms(
    _context: &ToolUseContext,
    resolved: &ToolPathResolution,
) -> Option<u64> {
    if resolved.uses_remote_workspace_backend() {
        return None;
    }

    let metadata = std::fs::metadata(&resolved.resolved_path).ok()?;
    let modified = metadata.modified().ok()?;
    modified
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|duration| duration.as_millis() as u64)
}

pub async fn file_mutation_timestamp_ms(
    context: &ToolUseContext,
    resolved: &ToolPathResolution,
) -> u64 {
    if let Some(timestamp_ms) = file_modification_time_ms(context, resolved).await {
        return timestamp_ms;
    }

    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

pub fn local_file_modification_time_ms(path: &Path) -> u64 {
    std::fs::metadata(path)
        .ok()
        .and_then(|metadata| metadata.modified().ok())
        .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or_else(|| {
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|duration| duration.as_millis() as u64)
                .unwrap_or(0)
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agentic::session::FileReadState;
    use crate::agentic::tools::framework::{ToolPathBackend, ToolUseContext};
    use crate::agentic::WorkspaceBinding;
    use std::collections::HashMap;
    use std::path::PathBuf;

    fn test_context(session_id: Option<&str>, root: PathBuf) -> ToolUseContext {
        ToolUseContext {
            tool_call_id: None,
            agent_type: None,
            session_id: session_id.map(str::to_string),
            dialog_turn_id: Some("turn-1".to_string()),
            workspace: Some(WorkspaceBinding::new(None, root)),
            unlocked_collapsed_tools: Vec::new(),
            custom_data: HashMap::new(),
            computer_use_host: None,
            cancellation_token: None,
            runtime_tool_restrictions: Default::default(),
            workspace_services: None,
        }
    }

    #[test]
    fn validate_edit_has_prior_read_skips_without_session_id() {
        let context = test_context(None, PathBuf::from("/tmp"));

        assert!(validate_edit_has_prior_read(
            &context,
            &ToolPathResolution {
                logical_path: "src/main.rs".to_string(),
                resolved_path: "src/main.rs".to_string(),
                requested_path: "src/main.rs".to_string(),
                backend: ToolPathBackend::Local,
                runtime_root: None,
                runtime_scope: None,
            }
        )
        .is_none());
    }

    #[test]
    fn validate_edit_has_prior_read_skips_without_coordinator() {
        let context = test_context(Some("session-1"), PathBuf::from("/tmp"));

        assert!(validate_edit_has_prior_read(
            &context,
            &ToolPathResolution {
                logical_path: "src/main.rs".to_string(),
                resolved_path: "src/main.rs".to_string(),
                requested_path: "src/main.rs".to_string(),
                backend: ToolPathBackend::Local,
                runtime_root: None,
                runtime_scope: None,
            }
        )
        .is_none());
    }

    fn read_state(content: &str, timestamp_ms: u64) -> FileReadState {
        FileReadState {
            content: content.to_string(),
            timestamp_ms,
            start_line: 1,
            end_line: 1,
            total_lines: 1,
            is_partial_view: false,
        }
    }

    #[test]
    fn validate_content_freshness_allows_matching_remote_content_without_mtime() {
        let state = read_state("alpha\n", 100);

        assert!(validate_content_freshness_against_read_state(
            "src/main.rs",
            &state,
            "alpha\n",
            None,
        )
        .is_none());
    }

    #[test]
    fn validate_content_freshness_rejects_changed_remote_content_without_mtime() {
        let state = read_state("alpha\n", 100);

        assert_eq!(
            validate_content_freshness_against_read_state(
                "src/main.rs",
                &state,
                "beta\n",
                None,
            )
            .as_deref(),
            Some(
                "File src/main.rs no longer matches the last Read result. Read it again before editing."
            )
        );
    }

    #[test]
    fn validate_content_freshness_allows_newer_mtime_when_full_read_content_matches() {
        let state = read_state("alpha\n", 100);

        assert!(validate_content_freshness_against_read_state(
            "src/main.rs",
            &state,
            "alpha\n",
            Some(200),
        )
        .is_none());
    }

    #[test]
    fn validate_content_freshness_rejects_newer_mtime_when_content_differs() {
        let state = read_state("alpha\n", 100);

        assert!(validate_content_freshness_against_read_state(
            "src/main.rs",
            &state,
            "beta\n",
            Some(200),
        )
        .is_some());
    }

    #[test]
    fn validate_content_freshness_ignores_older_mtime_even_when_content_differs() {
        let state = read_state("alpha\n", 200);

        assert!(validate_content_freshness_against_read_state(
            "src/main.rs",
            &state,
            "beta\n",
            Some(100),
        )
        .is_none());
    }
}
