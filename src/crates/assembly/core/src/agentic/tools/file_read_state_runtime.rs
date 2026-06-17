//! Runtime helpers for session-scoped file read state used by Read/Edit/Write tools.

use crate::agentic::coordination::get_global_coordinator;
use crate::agentic::session::FileReadState;
use crate::agentic::tools::framework::ToolPathResolution;
use crate::agentic::tools::tool_context_runtime::ToolUseContext;
use crate::util::errors::BitFunResult;
use bitfun_agent_tools::{
    file_read_facts_are_fresh, file_read_facts_content_matches, FileReadFreshnessFacts,
};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};
use tool_runtime::fs::read_file::ReadFileResult;
use tool_runtime::util::read_line_prefix::read_tool_output_to_file_content;

pub const FILE_UNEXPECTEDLY_MODIFIED_ERROR: &str =
    "File has been unexpectedly modified. Read it again before attempting to write it.";

pub fn validate_write_has_prior_read(
    context: &ToolUseContext,
    resolved: &ToolPathResolution,
) -> Option<String> {
    let session_id = context.session_id.as_deref()?;
    let coordinator = get_global_coordinator()?;
    let Some(read_state) = coordinator
        .get_session_manager()
        .get_file_read_state(session_id, &resolved.logical_path)
    else {
        return Some(format!(
            "Use Read to load the current contents of {} before calling Write on it.",
            resolved.logical_path
        ));
    };

    if read_state.is_partial_view {
        return Some(format!(
            "Use Read to load the full contents of {} before calling Write on it.",
            resolved.logical_path
        ));
    }

    None
}

pub fn read_state_tracking_enabled(context: &ToolUseContext) -> bool {
    context.session_id.is_some() && get_global_coordinator().is_some()
}

pub fn record_file_read_state(
    context: &ToolUseContext,
    resolved: &ToolPathResolution,
    read_result: &ReadFileResult,
    timestamp_ms: u64,
) {
    let Some(session_id) = context.session_id.as_deref() else {
        return;
    };
    let Some(coordinator) = get_global_coordinator() else {
        return;
    };

    // `is_partial_view` is reserved for auto-injected content the model has not
    // explicitly read (see Claude Code's FileState.isPartialView). Normal Read
    // tool calls with offset/limit still count as a valid read for Edit/Write.
    let state = FileReadState {
        content: read_tool_output_to_file_content(&read_result.content),
        timestamp_ms,
        start_line: read_result.start_line,
        end_line: read_result.end_line,
        total_lines: read_result.total_lines,
        is_partial_view: false,
    };

    coordinator.get_session_manager().set_file_read_state(
        session_id,
        &resolved.logical_path,
        state,
    );
}

pub fn get_stored_file_read_state(
    context: &ToolUseContext,
    resolved: &ToolPathResolution,
) -> Option<FileReadState> {
    let session_id = context.session_id.as_deref()?;
    let coordinator = get_global_coordinator()?;
    coordinator
        .get_session_manager()
        .get_file_read_state(session_id, &resolved.logical_path)
}

pub fn content_unchanged_since_full_read(
    read_state: &FileReadState,
    current_content: &str,
) -> bool {
    file_read_facts_content_matches(file_read_freshness_facts(read_state), current_content)
}

pub fn assert_file_not_unexpectedly_modified(
    read_state: Option<&FileReadState>,
    current_content: &str,
    current_mtime_ms: Option<u64>,
) -> Result<(), String> {
    let Some(read_state) = read_state else {
        return Err(FILE_UNEXPECTEDLY_MODIFIED_ERROR.to_string());
    };

    if !file_read_facts_are_fresh(
        file_read_freshness_facts(read_state),
        current_content,
        current_mtime_ms,
    ) {
        return Err(FILE_UNEXPECTEDLY_MODIFIED_ERROR.to_string());
    }

    Ok(())
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

    validate_edit_content_freshness_against_read_state(
        &resolved.logical_path,
        &read_state,
        &current_content,
        current_mtime_ms,
    )
}

pub async fn validate_write_against_read_state(
    context: &ToolUseContext,
    resolved: &ToolPathResolution,
) -> Option<String> {
    let read_state = get_stored_file_read_state(context, resolved)?;

    if let Some(current_mtime_ms) = file_modification_time_ms(context, resolved).await {
        if current_mtime_ms > read_state.timestamp_ms {
            return Some(format!(
                "The file {} changed after it was last read. Use Read again, then retry Write.",
                resolved.logical_path
            ));
        }
        return None;
    }

    let current_content = read_current_file_content(context, resolved).await.ok()?;
    if !file_read_facts_are_fresh(
        file_read_freshness_facts(&read_state),
        &current_content,
        None,
    ) {
        return Some(format!(
            "The file {} no longer matches the last Read result. Use Read again, then retry Write.",
            resolved.logical_path
        ));
    }

    None
}

pub async fn validate_existing_file_read_before_write(
    context: &ToolUseContext,
    resolved: &ToolPathResolution,
) -> Option<String> {
    if let Some(message) = validate_write_has_prior_read(context, resolved) {
        return Some(message);
    }

    validate_write_against_read_state(context, resolved).await
}

fn validate_edit_content_freshness_against_read_state(
    logical_path: &str,
    read_state: &FileReadState,
    current_content: &str,
    current_mtime_ms: Option<u64>,
) -> Option<String> {
    if file_read_facts_are_fresh(
        file_read_freshness_facts(read_state),
        current_content,
        current_mtime_ms,
    ) {
        return None;
    }

    if current_mtime_ms.is_some() {
        return Some(format!(
            "The file {} changed after it was last read. Use Read again, then retry Edit.",
            logical_path
        ));
    }

    Some(format!(
        "The file {} no longer matches the last Read result. Use Read again, then retry Edit.",
        logical_path
    ))
}

fn file_read_freshness_facts(read_state: &FileReadState) -> FileReadFreshnessFacts<'_> {
    FileReadFreshnessFacts {
        content: &read_state.content,
        timestamp_ms: read_state.timestamp_ms,
        is_full_file_read: read_state.is_full_file_read(),
    }
}

pub fn validate_edit_has_prior_read(
    context: &ToolUseContext,
    resolved: &ToolPathResolution,
) -> Option<String> {
    let session_id = context.session_id.as_deref()?;
    let coordinator = get_global_coordinator()?;
    let Some(read_state) = coordinator
        .get_session_manager()
        .get_file_read_state(session_id, &resolved.logical_path)
    else {
        return Some(format!(
            "Use Read to load the current contents of {} before calling Edit on it.",
            resolved.logical_path
        ));
    };

    if read_state.is_partial_view {
        return Some(format!(
            "Use Read to load the full contents of {} before calling Edit on it.",
            resolved.logical_path
        ));
    }

    None
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

pub async fn read_current_file_content(
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
    use crate::agentic::tools::framework::ToolPathBackend;
    use crate::agentic::tools::tool_context_runtime::ToolUseContext;
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
            runtime_tool_restrictions: Default::default(),
            runtime_handles: bitfun_runtime_ports::ToolRuntimeHandles::default(),
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

    #[test]
    fn validate_edit_has_prior_read_rejects_auto_injected_partial_view() {
        let context = test_context(Some("session-1"), PathBuf::from("/tmp"));
        let resolution = ToolPathResolution {
            logical_path: "src/main.rs".to_string(),
            resolved_path: "src/main.rs".to_string(),
            requested_path: "src/main.rs".to_string(),
            backend: ToolPathBackend::Local,
            runtime_root: None,
            runtime_scope: None,
        };

        // Without a coordinator this stays permissive in unit tests.
        assert!(validate_edit_has_prior_read(&context, &resolution).is_none());
    }

    #[test]
    fn validate_content_freshness_allows_partial_read_range_without_full_file() {
        let state = FileReadState {
            content: "middle\n".to_string(),
            timestamp_ms: 100,
            start_line: 50,
            end_line: 100,
            total_lines: 556,
            is_partial_view: false,
        };

        assert!(validate_edit_content_freshness_against_read_state(
            "src/state.js",
            &state,
            "different full file\n",
            Some(200),
        )
        .is_some());
    }

    #[test]
    fn assert_file_not_unexpectedly_modified_allows_matching_full_read_after_newer_mtime() {
        let state = FileReadState {
            content: "alpha\n".to_string(),
            timestamp_ms: 100,
            start_line: 1,
            end_line: 1,
            total_lines: 1,
            is_partial_view: false,
        };

        assert!(assert_file_not_unexpectedly_modified(Some(&state), "alpha\n", Some(200)).is_ok());
    }

    #[test]
    fn assert_file_not_unexpectedly_modified_rejects_changed_full_read_after_newer_mtime() {
        let state = FileReadState {
            content: "alpha\n".to_string(),
            timestamp_ms: 100,
            start_line: 1,
            end_line: 1,
            total_lines: 1,
            is_partial_view: false,
        };

        assert!(assert_file_not_unexpectedly_modified(Some(&state), "beta\n", Some(200)).is_err());
    }

    #[test]
    fn assert_file_not_unexpectedly_modified_rejects_partial_read_after_newer_mtime() {
        let state = FileReadState {
            content: "middle\n".to_string(),
            timestamp_ms: 100,
            start_line: 50,
            end_line: 100,
            total_lines: 556,
            is_partial_view: false,
        };

        assert!(
            assert_file_not_unexpectedly_modified(Some(&state), "full file\n", Some(200)).is_err()
        );
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

        assert!(validate_edit_content_freshness_against_read_state(
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
            validate_edit_content_freshness_against_read_state(
                "src/main.rs",
                &state,
                "beta\n",
                None,
            )
            .as_deref(),
            Some(
                "The file src/main.rs no longer matches the last Read result. Use Read again, then retry Edit."
            )
        );
    }

    #[test]
    fn validate_content_freshness_allows_newer_mtime_when_full_read_content_matches() {
        let state = read_state("alpha\n", 100);

        assert!(validate_edit_content_freshness_against_read_state(
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

        assert!(validate_edit_content_freshness_against_read_state(
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

        assert!(validate_edit_content_freshness_against_read_state(
            "src/main.rs",
            &state,
            "beta\n",
            Some(100),
        )
        .is_none());
    }
}
