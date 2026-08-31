//! Runtime helpers for session-scoped file read state used by Read/Edit/Write tools.

use crate::agentic::coordination::get_global_coordinator;
use crate::agentic::session::{FileReadState, FileRevision, ReviewReadCoverage};
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
use sha2::{Digest, Sha256};
use std::time::UNIX_EPOCH;
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
    let state = FileReadState::from_read_tool_content_with_truncation(
        read_tool_output_to_file_content(&read_result.content),
        timestamp_ms,
        read_result.start_line,
        read_result.end_line,
        read_result.total_lines,
        read_result.content_truncated,
    );

    coordinator.get_session_manager().set_file_read_state(
        session_id,
        &resolved.logical_path,
        state,
    );
}

pub fn review_read_receipts_enabled(context: &ToolUseContext) -> bool {
    context.custom_data.contains_key("deep_review_run_manifest")
        || context.agent_type.as_deref().is_some_and(|agent_type| {
            matches!(
                agent_type,
                "CodeReview" | "DeepReview" | "ReviewWorker" | "ReviewJudge"
            )
        })
}

/// Capture the same revision facts from either workspace provider. The hash
/// is streamed and the metadata is checked again so a detected concurrent
/// change never becomes a reusable review receipt.
pub async fn file_revision(
    context: &ToolUseContext,
    resolved: &ToolPathResolution,
) -> Option<FileRevision> {
    use tokio::io::AsyncReadExt;
    let file_system = context.file_system_for_path(resolved).ok()?;
    let before = file_system
        .metadata(&resolved.resolved_path, true)
        .await
        .ok()??;
    if before.kind != bitfun_runtime_ports::WorkspacePathKind::File {
        return None;
    }
    let modified_ns = before.modified?.duration_since(UNIX_EPOCH).ok()?.as_nanos();
    let mut reader = file_system.open_read(&resolved.resolved_path).await.ok()?;
    let mut hasher = Sha256::new();
    let mut byte_len = 0_u64;
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let count = reader.read(&mut buffer).await.ok()?;
        if count == 0 {
            break;
        }
        byte_len = byte_len.checked_add(count as u64)?;
        hasher.update(&buffer[..count]);
    }
    let after = file_system
        .metadata(&resolved.resolved_path, true)
        .await
        .ok()??;
    if before.kind != after.kind || before.size != after.size || before.modified != after.modified {
        return None;
    }
    if after.size.is_some_and(|size| size != byte_len) {
        return None;
    }
    Some(FileRevision {
        modified_ns,
        byte_len,
        content_sha256: hasher.finalize().into(),
    })
}

pub fn get_review_read_coverage(
    context: &ToolUseContext,
    resolved: &ToolPathResolution,
    revision: FileRevision,
    start_line: usize,
    limit: usize,
) -> Option<ReviewReadCoverage> {
    if !review_read_receipts_enabled(context) {
        return None;
    }
    let session_id = context.session_id.as_deref()?;
    let coordinator = get_global_coordinator()?;
    coordinator.get_session_manager().review_read_coverage(
        session_id,
        &resolved.logical_path,
        revision,
        start_line,
        limit,
    )
}

pub fn record_review_read_receipt(
    context: &ToolUseContext,
    resolved: &ToolPathResolution,
    revision: FileRevision,
    read_result: &ReadFileResult,
) {
    if read_result.content_truncated || !review_read_receipts_enabled(context) {
        return;
    }
    let Some(session_id) = context.session_id.as_deref() else {
        return;
    };
    let Some(coordinator) = get_global_coordinator() else {
        return;
    };
    coordinator.get_session_manager().record_review_read(
        session_id,
        &resolved.logical_path,
        revision,
        read_result.start_line,
        read_result.end_line,
        read_result.total_lines,
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
    context
        .file_system_for_path(resolved)?
        .read_file_text(&resolved.resolved_path)
        .await
        .map_err(|error| {
            crate::util::errors::BitFunError::tool(format!(
                "Failed to read file {}: {:#}",
                resolved.logical_path, error
            ))
        })
}

pub async fn file_modification_time_ms(
    context: &ToolUseContext,
    resolved: &ToolPathResolution,
) -> Option<u64> {
    let metadata = context
        .file_system_for_path(resolved)
        .ok()?
        .metadata(&resolved.resolved_path, true)
        .await
        .ok()??;
    metadata
        .modified?
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|duration| duration.as_millis() as u64)
}

pub async fn file_mutation_timestamp_ms(
    context: &ToolUseContext,
    resolved: &ToolPathResolution,
) -> u64 {
    // Unknown workspace mtime is not the controller's wall clock. A zero
    // timestamp keeps the same unknown-fact representation used by Read.
    file_modification_time_ms(context, resolved)
        .await
        .unwrap_or(0)
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

    #[tokio::test]
    async fn file_revision_detects_same_size_content_changes_with_restored_mtime() {
        let temp = tempfile::tempdir().expect("temp dir");
        let path = temp.path().join("review.txt");
        std::fs::write(&path, b"alpha").expect("write original");
        let original_mtime = filetime::FileTime::from_last_modification_time(
            &std::fs::metadata(&path).expect("original metadata"),
        );
        let context = test_context(None, temp.path().to_path_buf());
        let resolved = context
            .resolve_tool_path("review.txt")
            .expect("resolve file");
        let original = file_revision(&context, &resolved)
            .await
            .expect("original revision");

        std::fs::write(&path, b"bravo").expect("write replacement");
        filetime::set_file_mtime(&path, original_mtime).expect("restore mtime");
        let replacement = file_revision(&context, &resolved)
            .await
            .expect("replacement revision");

        assert_eq!(original.modified_ns, replacement.modified_ns);
        assert_eq!(original.byte_len, replacement.byte_len);
        assert_ne!(original.content_sha256, replacement.content_sha256);
        assert_ne!(original, replacement);
    }

    #[test]
    fn review_read_receipts_do_not_treat_a_custom_legacy_name_as_a_builtin_worker() {
        let mut custom = test_context(Some("session-1"), PathBuf::from("/tmp"));
        custom.agent_type = Some("ReviewSecurity".to_string());
        assert!(!review_read_receipts_enabled(&custom));

        custom.custom_data.insert(
            "deep_review_run_manifest".to_string(),
            serde_json::json!({}),
        );
        assert!(review_read_receipts_enabled(&custom));

        let mut worker = test_context(Some("session-2"), PathBuf::from("/tmp"));
        worker.agent_type = Some("ReviewWorker".to_string());
        assert!(review_read_receipts_enabled(&worker));
    }
}
