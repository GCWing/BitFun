//! Runtime helpers for session-scoped file read state used by Read/Edit/Write tools.

use crate::agentic::coordination::get_global_coordinator;
use crate::agentic::session::FileReadState;
use crate::agentic::tools::framework::ToolPathResolution;
use crate::agentic::tools::tool_context_runtime::ToolUseContext;
use crate::util::errors::BitFunResult;
pub use bitfun_agent_runtime::file_read_state::{
    assert_file_not_unexpectedly_modified, content_unchanged_since_full_read,
    FILE_UNEXPECTEDLY_MODIFIED_ERROR,
};
use bitfun_agent_runtime::file_read_state::{
    validate_edit_content_freshness_against_read_state, validate_prior_read_state,
    validate_write_content_freshness_against_read_state,
    validate_write_mtime_freshness_against_read_state, FileMutationKind,
};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};
use tool_runtime::fs::read_file::ReadFileResult;
use tool_runtime::util::read_line_prefix::read_tool_output_to_file_content;

pub fn validate_write_has_prior_read(
    context: &ToolUseContext,
    resolved: &ToolPathResolution,
) -> Option<String> {
    let session_id = context.session_id.as_deref()?;
    let coordinator = get_global_coordinator()?;
    let read_state = coordinator
        .get_session_manager()
        .get_file_read_state(session_id, &resolved.logical_path);
    validate_prior_read_state(
        &resolved.logical_path,
        read_state.as_ref(),
        FileMutationKind::Write,
    )
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
    let state = FileReadState::from_read_tool_content(
        read_tool_output_to_file_content(&read_result.content),
        timestamp_ms,
        read_result.start_line,
        read_result.end_line,
        read_result.total_lines,
    );

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
        return validate_write_mtime_freshness_against_read_state(
            &resolved.logical_path,
            &read_state,
            current_mtime_ms,
        );
    }

    let current_content = read_current_file_content(context, resolved).await.ok()?;
    validate_write_content_freshness_against_read_state(
        &resolved.logical_path,
        &read_state,
        &current_content,
    )
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

pub fn validate_edit_has_prior_read(
    context: &ToolUseContext,
    resolved: &ToolPathResolution,
) -> Option<String> {
    let session_id = context.session_id.as_deref()?;
    let coordinator = get_global_coordinator()?;
    let read_state = coordinator
        .get_session_manager()
        .get_file_read_state(session_id, &resolved.logical_path);
    validate_prior_read_state(
        &resolved.logical_path,
        read_state.as_ref(),
        FileMutationKind::Edit,
    )
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

    let state = FileReadState::from_full_content(content, timestamp_ms);

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
            loaded_deferred_tool_specs: Vec::new(),
            primary_model_facts: tool_runtime::context::PrimaryModelFacts::default(),
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
}
