//! Tool pipeline
//!
//! Manages the complete lifecycle of tools:
//! permission authorization, execution, caching, retries, etc.

use super::state_manager::{tool_task_state_kind, ToolStateManager};
use super::types::*;
use crate::agentic::core::{Message, ToolCall, ToolExecutionState, ToolResult as ModelToolResult};
use crate::agentic::events::types::ToolEventData;
use crate::agentic::tools::computer_use_host::ComputerUseHostRef;
use crate::agentic::tools::framework::ToolResult as FrameworkToolResult;
use crate::agentic::tools::product_runtime::{
    collect_product_loaded_deferred_tool_specs, resolve_product_get_tool_spec_results,
};
use crate::agentic::tools::registry::{ToolRef, ToolRegistry};
use crate::agentic::tools::restrictions::get_session_restrictions;
use crate::agentic::tools::tool_context_runtime;
use crate::agentic::tools::tool_context_runtime::ToolUseContext;
use crate::agentic::tools::tool_result_storage;
use crate::native_hooks::{self, NativeHookSessionFacts};
use crate::service::config::types::ExecutionThresholds;
use crate::util::elapsed_ms_u64;
use crate::util::errors::{BitFunError, BitFunResult};
use bitfun_agent_runtime::permission::{
    plan_permission_intents, PendingPermissionReceiver, PermissionIntentPlan,
    PermissionRequestManager, PermissionWaitOutcome,
};
use bitfun_agent_runtime::sdk::PermissionReplySource;
use bitfun_agent_stream::ToolArgumentRepairKind;
use bitfun_agent_tools::{
    build_invalid_tool_call_error_message, build_normal_tool_json_repair_notice,
    build_permission_denied_tool_presentation, build_tool_execution_error_presentation,
    build_tool_execution_timeout_presentation,
    build_user_rejected_tool_presentation_with_instruction,
    build_user_steering_interrupted_presentation, build_write_tail_closure_notice,
    is_write_like_tool_name, render_tool_result_for_assistant, validate_tool_execution_admission,
    LoadedDeferredToolSpec, PermissionIntent, ResolvedToolInvocation,
    ToolExecutionAdmissionRejection, ToolExecutionAdmissionRequest, ToolExecutionErrorPresentation,
    ToolRuntimeRestrictions, GET_TOOL_SPEC_TOOL_NAME, USER_STEERING_INTERRUPTED_MESSAGE,
};
use bitfun_runtime_ports::{
    PermissionReply, PermissionRequest, PermissionRequestSource, PermissionRequestSourceKind,
    PermissionResourceCaseSensitivity, RoundInjectionToolPreemption,
};
use futures::future::join_all;
use log::{debug, error, info, warn};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::Path;
use std::sync::Arc;
use std::time::{Instant, SystemTime};
use tokio::sync::{Mutex as TokioMutex, RwLock as TokioRwLock};
use tokio::time::{timeout, Duration};
use tokio_util::sync::CancellationToken;
use tool_runtime::pipeline::{
    partition_tool_batches, retry_delay_ms, should_cancel_tool_state, should_retry_tool_attempt,
    summarize_dialog_turn_cancellation, tool_call_concurrency_safe_for_batch,
    ToolCancellationTokenStore, ToolExecutionErrorClass, ToolRetryAttemptFacts,
};

fn resolve_contextual_tool(
    tool: Arc<dyn crate::agentic::tools::framework::Tool>,
    workspace_root: Option<&Path>,
    remote: bool,
) -> Option<Arc<dyn crate::agentic::tools::framework::Tool>> {
    #[cfg(feature = "external-sources")]
    {
        crate::external_tools::resolve_external_tool_for_workspace(
            tool,
            crate::external_tools::external_tool_route_root(workspace_root, remote),
        )
    }
    #[cfg(not(feature = "external-sources"))]
    {
        let _ = (workspace_root, remote);
        Some(tool)
    }
}

fn persisted_effective_tool_name(
    wire_tool_name: &str,
    effective_tool_name: &str,
) -> Option<String> {
    (wire_tool_name != effective_tool_name).then(|| effective_tool_name.to_string())
}

/// R-MR-11 读取/搜索类工具集合（工具注册名）。
const REPEATED_READ_TOOL_NAMES: &[&str] = &["Read", "Grep", "Glob", "LS", "WebSearch", "WebFetch"];

/// R-MR-11 目标指纹归一化。
///
/// - Read：文件路径（忽略 offset/limit/tail/render 等分段参数 → 十行读同文件 = 同目标）
/// - Grep：关键词 pattern + path（未提供 path 归一为 "."，同关键词同路径 = 同目标）
/// - Glob：pattern（忽略 path 变化，pattern 即目标）
/// - WebSearch：query
/// - WebFetch：url
/// - LS：path（未提供归一为 "."）
///
/// 非读取/搜索类工具返回 None。
fn repeated_read_target_fingerprint(
    tool_name: &str,
    arguments: &serde_json::Value,
) -> Option<String> {
    if !REPEATED_READ_TOOL_NAMES.contains(&tool_name) {
        return None;
    }
    let target = match tool_name {
        "Read" => arguments
            .get("file_path")
            .and_then(serde_json::Value::as_str)?
            .trim()
            .to_string(),
        "Grep" => {
            let pattern = arguments
                .get("pattern")
                .and_then(serde_json::Value::as_str)?
                .trim();
            let path = arguments
                .get("path")
                .and_then(serde_json::Value::as_str)
                .unwrap_or(".");
            format!("{pattern}@{}", path.trim())
        }
        "Glob" => arguments
            .get("pattern")
            .and_then(serde_json::Value::as_str)?
            .trim()
            .to_string(),
        "WebSearch" => arguments
            .get("query")
            .and_then(serde_json::Value::as_str)?
            .trim()
            .to_string(),
        "WebFetch" => arguments
            .get("url")
            .and_then(serde_json::Value::as_str)?
            .trim()
            .to_string(),
        "LS" => {
            let path = arguments
                .get("path")
                .and_then(serde_json::Value::as_str)
                .unwrap_or(".");
            path.trim().to_string()
        }
        _ => return None,
    };
    if target.is_empty() {
        return None;
    }
    Some(target)
}

/// 小文件特判的裸函数版（供单元测试直接验证）。
fn repeated_read_small_file_hint_impl(
    tool_name: &str,
    arguments: &serde_json::Value,
    small_file_line_threshold: usize,
) -> Option<String> {
    if tool_name != "Read" {
        return None;
    }
    let file_path = arguments
        .get("file_path")
        .and_then(serde_json::Value::as_str)?;
    if file_path.is_empty() || arguments.get("offset").is_none() {
        return None;
    }
    let small = std::fs::read_to_string(file_path)
        .map(|content| content.lines().count() < small_file_line_threshold)
        .unwrap_or(false);
    small.then(|| {
        format!(
            "文件较小（<{} 行），建议一次读全文",
            small_file_line_threshold
        )
    })
}

/// R-MR-11 纯判定：给定会话级连续状态，返回是否拦截（及提示）。
///
/// - 目标与当前连续目标一致 → 计数 +1；达到 limit 时拦截（第 N 次）。
/// - 目标变化 → 重置为 1（交叉引用 A→B→A 不误伤）。
/// - 拦截后计数保持，同目标后续调用继续拦截。
fn repeated_read_decide(
    thresholds: &ExecutionThresholds,
    tool_name: &str,
    target: &str,
    arguments: &serde_json::Value,
    state: &mut RepeatedReadSessionState,
) -> Option<String> {
    if !thresholds.repeated_read_enabled {
        return None;
    }
    let limit = thresholds.repeated_read_limit.max(2);

    if state.current_target.as_deref() != Some(target) {
        state.current_target = Some(target.to_string());
        state.consecutive_count = 1;
        return None;
    }

    state.consecutive_count += 1;
    if state.consecutive_count < limit {
        return None;
    }

    // 第 N 次：拦截。构造引导正确做法的提示。
    let message = if tool_name == "Read" && arguments.get("offset").is_some() {
        let small_hint = repeated_read_small_file_hint_impl(
            tool_name,
            arguments,
            thresholds.small_file_line_threshold,
        );
        match small_hint {
            Some(hint) => format!(
                "重复读取拦截（R-MR-11）：{tool_name} 目标 `{target}` 已连续调用 {} 次，本次未执行（零请求）。检测到碎片化读取（连续分段读同一目标 {} 次）。{}。正确做法：读全文（小文件）或搜索关键词定位（大文件），不要再逐行/逐段反复读取。",
                state.consecutive_count, state.consecutive_count, hint
            ),
            None => format!(
                "重复读取拦截（R-MR-11）：{tool_name} 目标 `{target}` 已连续调用 {} 次，本次未执行（零请求）。检测到碎片化读取（连续分段读同一目标 {} 次）。正确做法：读全文（小文件）或搜索关键词定位（大文件），不要再逐行/逐段反复读取。",
                state.consecutive_count, state.consecutive_count
            ),
        }
    } else {
        format!(
            "重复读取拦截（R-MR-11）：{tool_name} 目标 `{target}` 已连续调用 {} 次，本次未执行（零请求）。该目标已连续读取 {} 次，请基于已有内容继续，或明确新目标。正确做法：读全文（小文件）或搜索关键词定位（大文件），不要再重复读取同一目标。",
            state.consecutive_count, state.consecutive_count
        )
    };
    state.last_intercepted_message = Some(message.clone());
    Some(message)
}

/// Resolve the effective tool runtime restrictions for a session.
///
/// Per-session restrictions fully replace the context-level restrictions,
/// matching the precedence of
/// [`ToolUseContext::enforce_tool_runtime_restrictions`]: a session override
/// wins, otherwise the context-level template applies.
fn effective_runtime_tool_restrictions(
    session_id: &str,
    context_level: &ToolRuntimeRestrictions,
) -> ToolRuntimeRestrictions {
    get_session_restrictions(session_id).unwrap_or_else(|| context_level.clone())
}

/// Merge freshly collected deferred-tool specs into the existing set. A fresh
/// entry replaces the entry with the same tool name, mirroring the upsert
/// semantics of the loaded-spec collection channel.
fn merge_loaded_deferred_tool_specs(
    existing: &[LoadedDeferredToolSpec],
    fresh: &[LoadedDeferredToolSpec],
) -> Vec<LoadedDeferredToolSpec> {
    let mut merged: BTreeMap<String, LoadedDeferredToolSpec> = existing
        .iter()
        .map(|spec| (spec.tool_name.clone(), spec.clone()))
        .collect();
    for spec in fresh {
        merged.insert(spec.tool_name.clone(), spec.clone());
    }
    merged.into_values().collect()
}

/// Maximum auto-reload attempts for one stale deferred-tool spec invocation.
/// Each attempt re-runs GetToolSpec and re-checks admission; the loop ends
/// early as soon as admission passes or the tool is not reloadable.
const MAX_STALE_SPEC_RELOAD_ATTEMPTS: usize = 3;

/// Defensive upper bound for the session-scoped auto-reload cache. Entries are
/// small and only referenced while their session stays active, so this guard
/// simply prevents unbounded growth after very long-lived hosts.
const MAX_CACHED_SESSIONS_WITH_RELOADED_SPECS: usize = 1024;

/// Outcome of a stale deferred-tool spec reload attempt.
enum StaleSpecReloadOutcome {
    /// The reload observed a fresh spec and produced the merged loaded-
    /// spec set (existing entries plus the refreshed one).
    Reloaded(Vec<LoadedDeferredToolSpec>),
    /// The tool cannot be reloaded through the GetToolSpec runtime path —
    /// the execution call failed, returned no usable result, or the tool
    /// is no longer part of the contextual deferred catalog. The caller
    /// keeps the original admission rejection.
    NotReloadable(&'static str),
}

/// Convert framework::ToolResult to core::ToolResult
///
/// Ensure always has result_for_assistant, avoid tool message content being empty
fn convert_tool_result(
    framework_result: FrameworkToolResult,
    tool_id: &str,
    wire_tool_name: &str,
    effective_tool_name: &str,
) -> ModelToolResult {
    match framework_result {
        FrameworkToolResult::Result {
            data,
            result_for_assistant,
            image_attachments,
        } => {
            // If the tool does not provide result_for_assistant, pass the full
            // structured result through to the model. Summaries like
            // "completed successfully" can hide fields the model needs for the
            // next decision.
            let assistant_text = result_for_assistant
                .or_else(|| Some(render_tool_result_for_assistant(effective_tool_name, &data)));

            ModelToolResult {
                tool_id: tool_id.to_string(),
                tool_name: wire_tool_name.to_string(),
                effective_tool_name: persisted_effective_tool_name(
                    wire_tool_name,
                    effective_tool_name,
                ),
                result: data,
                result_for_assistant: assistant_text,
                is_error: false,
                duration_ms: None,
                image_attachments,
            }
        }
        FrameworkToolResult::Progress { content, .. } => {
            let assistant_text = Some(render_tool_result_for_assistant(
                effective_tool_name,
                &content,
            ));

            ModelToolResult {
                tool_id: tool_id.to_string(),
                tool_name: wire_tool_name.to_string(),
                effective_tool_name: persisted_effective_tool_name(
                    wire_tool_name,
                    effective_tool_name,
                ),
                result: content,
                result_for_assistant: assistant_text,
                is_error: false,
                duration_ms: None,
                image_attachments: None,
            }
        }
        FrameworkToolResult::StreamChunk { data, .. } => {
            let assistant_text = Some(render_tool_result_for_assistant(effective_tool_name, &data));

            ModelToolResult {
                tool_id: tool_id.to_string(),
                tool_name: wire_tool_name.to_string(),
                effective_tool_name: persisted_effective_tool_name(
                    wire_tool_name,
                    effective_tool_name,
                ),
                result: data,
                result_for_assistant: assistant_text,
                is_error: false,
                duration_ms: None,
                image_attachments: None,
            }
        }
    }
}

fn resolve_pipeline_invocation(
    tool_call: &ToolCall,
    context: &ToolExecutionContext,
) -> (ResolvedToolInvocation, Option<String>) {
    let invocation = match ResolvedToolInvocation::from_wire_call(
        tool_call.tool_name.clone(),
        tool_call.arguments.clone(),
    ) {
        Ok(invocation) => invocation,
        Err(error) => {
            return (
                ResolvedToolInvocation::direct(
                    tool_call.tool_name.clone(),
                    tool_call.arguments.clone(),
                ),
                Some(error.to_string()),
            );
        }
    };

    if invocation.is_deferred()
        && !context
            .deferred_tools
            .iter()
            .any(|tool_name| tool_name == &invocation.effective_tool_name)
    {
        let effective_tool_name = invocation.effective_tool_name.clone();
        return (
            invocation,
            Some(format!(
                "Tool '{effective_tool_name}' is not an available deferred tool in the current context"
            )),
        );
    }

    (invocation, None)
}

/// Convert core::ToolResult to framework::ToolResult
fn convert_to_framework_result(model_result: &ModelToolResult) -> FrameworkToolResult {
    FrameworkToolResult::Result {
        data: model_result.result.clone(),
        result_for_assistant: model_result.result_for_assistant.clone(),
        image_attachments: model_result.image_attachments.clone(),
    }
}

fn elapsed_ms_since(time: SystemTime) -> u64 {
    time.elapsed()
        .map(|duration| duration.as_millis().min(u128::from(u64::MAX)) as u64)
        .unwrap_or(0)
}

fn classify_tool_error(error: &BitFunError) -> &'static str {
    match error {
        BitFunError::Validation(_) => "invalid_arguments",
        BitFunError::Cancelled(_) => "cancelled",
        BitFunError::Timeout(_) => "timeout",
        BitFunError::NotFound(_) => "not_found",
        _ => "execution_error",
    }
}

fn build_error_execution_result(
    task_id: &str,
    task: Option<ToolTask>,
    error: &BitFunError,
) -> ToolExecutionResult {
    let error_message = error.to_string();
    let category = classify_tool_error(error);
    let (tool_id, wire_tool_name, effective_tool_name, execution_time_ms, provided_arguments) =
        if let Some(task) = task {
            // Parsed arguments are already present on the preceding tool call.
            // Preserve the complete provider output only when it could not be
            // parsed into that structured call.
            let provided_arguments = task
                .tool_call
                .is_error
                .then(|| task.tool_call.raw_arguments.clone())
                .flatten();
            (
                task.tool_call.tool_id,
                task.tool_call.tool_name,
                task.invocation.effective_tool_name,
                elapsed_ms_since(task.created_at),
                provided_arguments,
            )
        } else {
            warn!("Task not found in state manager: {}", task_id);
            (
                task_id.to_string(),
                "unknown".to_string(),
                "unknown".to_string(),
                0,
                None,
            )
        };
    let presentation = build_tool_execution_error_presentation(
        &effective_tool_name,
        category,
        &error_message,
        provided_arguments,
    );
    let persisted_effective_tool_name =
        persisted_effective_tool_name(&wire_tool_name, &effective_tool_name);

    ToolExecutionResult {
        tool_id: tool_id.clone(),
        tool_name: wire_tool_name.clone(),
        effective_tool_name,
        result: ModelToolResult {
            tool_id,
            tool_name: wire_tool_name,
            effective_tool_name: persisted_effective_tool_name,
            result: presentation.result_json,
            result_for_assistant: Some(presentation.result_for_assistant),
            is_error: true,
            duration_ms: Some(execution_time_ms),
            image_attachments: None,
        },
        execution_time_ms,
    }
}

fn build_user_steering_interrupted_result(
    task_id: &str,
    task: Option<ToolTask>,
) -> ToolExecutionResult {
    let (tool_id, wire_tool_name, effective_tool_name, execution_time_ms) = if let Some(task) = task
    {
        (
            task.tool_call.tool_id,
            task.tool_call.tool_name,
            task.invocation.effective_tool_name,
            elapsed_ms_since(task.created_at),
        )
    } else {
        warn!(
            "Task not found while building steering-interrupted result: {}",
            task_id
        );
        (
            task_id.to_string(),
            "unknown".to_string(),
            "unknown".to_string(),
            0,
        )
    };

    let presentation = build_user_steering_interrupted_presentation(&effective_tool_name);
    let persisted_effective_tool_name =
        persisted_effective_tool_name(&wire_tool_name, &effective_tool_name);

    ToolExecutionResult {
        tool_id: tool_id.clone(),
        tool_name: wire_tool_name.clone(),
        effective_tool_name,
        result: ModelToolResult {
            tool_id,
            tool_name: wire_tool_name,
            effective_tool_name: persisted_effective_tool_name,
            result: presentation.result_json,
            result_for_assistant: Some(presentation.result_for_assistant),
            // Skipped-by-steering is not a failure: the tool never executed, so
            // marking it `is_error: true` would push a fake failure to the model
            // (provider converters translate it into `tool_result.is_error` /
            // `[TOOL ERROR]`), causing retry / detour waste on an action that
            // merely yielded to a user steering message. The `status: "skipped"`
            // + `category: "user_steering_interrupted"` payload already tells the
            // model the tool did not run.
            is_error: false,
            duration_ms: Some(execution_time_ms),
            image_attachments: None,
        },
        execution_time_ms,
    }
}

fn build_user_rejected_tool_result(
    task_id: &str,
    task: Option<ToolTask>,
    feedback: Option<&str>,
) -> ToolExecutionResult {
    build_permission_rejected_tool_result(task_id, task, |tool_name| {
        build_user_rejected_tool_presentation_with_instruction(tool_name, feedback)
    })
}

fn build_permission_denied_tool_result(
    task_id: &str,
    task: Option<ToolTask>,
    reason: &str,
) -> ToolExecutionResult {
    build_permission_rejected_tool_result(task_id, task, |tool_name| {
        build_permission_denied_tool_presentation(tool_name, reason)
    })
}

fn build_permission_rejected_tool_result(
    task_id: &str,
    task: Option<ToolTask>,
    presentation_for: impl FnOnce(&str) -> ToolExecutionErrorPresentation,
) -> ToolExecutionResult {
    let (tool_id, wire_tool_name, effective_tool_name, execution_time_ms) = if let Some(task) = task
    {
        (
            task.tool_call.tool_id,
            task.tool_call.tool_name,
            task.invocation.effective_tool_name,
            elapsed_ms_since(task.created_at),
        )
    } else {
        warn!(
            "Task not found while building user-rejected result: {}",
            task_id
        );
        (
            task_id.to_string(),
            "unknown".to_string(),
            "unknown".to_string(),
            0,
        )
    };

    let presentation = presentation_for(&effective_tool_name);
    let persisted_effective_tool_name =
        persisted_effective_tool_name(&wire_tool_name, &effective_tool_name);

    ToolExecutionResult {
        tool_id: tool_id.clone(),
        tool_name: wire_tool_name.clone(),
        effective_tool_name,
        result: ModelToolResult {
            tool_id,
            tool_name: wire_tool_name,
            effective_tool_name: persisted_effective_tool_name,
            result: presentation.result_json,
            result_for_assistant: Some(presentation.result_for_assistant),
            is_error: false,
            duration_ms: Some(execution_time_ms),
            image_attachments: None,
        },
        execution_time_ms,
    }
}

const ROUND_INJECTION_RUNNING_TOOL_CANCELLED_MESSAGE: &str =
    "Tool execution cancelled because a pending round injection requested running-tool preemption for this turn.";

fn should_retry_tool_error(error: &BitFunError) -> bool {
    matches!(
        error,
        BitFunError::Timeout(_)
            | BitFunError::Io(_)
            | BitFunError::Http(_)
            | BitFunError::Service(_)
            | BitFunError::MCPError(_)
            | BitFunError::ProcessError(_)
            | BitFunError::Other(_)
    )
}

fn classify_tool_retry_error(error: &BitFunError) -> ToolExecutionErrorClass {
    if should_retry_tool_error(error) {
        ToolExecutionErrorClass::Retryable
    } else {
        ToolExecutionErrorClass::Terminal
    }
}

fn map_tool_execution_admission_rejection(error: ToolExecutionAdmissionRejection) -> BitFunError {
    match error {
        ToolExecutionAdmissionRejection::RuntimeRestriction(error) => error.into(),
        ToolExecutionAdmissionRejection::AllowedList(error) => {
            BitFunError::Validation(error.to_string())
        }
        ToolExecutionAdmissionRejection::Deferred(error) => {
            BitFunError::Validation(error.to_string())
        }
    }
}

fn recovered_write_has_potentially_truncated_marked_path(
    tool_name: &str,
    arguments: &serde_json::Value,
    repair_kind: ToolArgumentRepairKind,
    recovered_from_truncation: bool,
) -> bool {
    (repair_kind.is_write_tail_closure() || recovered_from_truncation)
        && tool_name == "Write"
        && arguments
            .get("payload")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|value| value.starts_with("+++ ") && !value.contains('\n'))
}

enum PermissionAuthorization {
    Allowed,
    UserRejected { feedback: Option<String> },
    PolicyDenied { reason: String },
}

fn user_rejection_audit_reason(tool_name: &str, feedback: Option<&str>) -> String {
    match feedback {
        Some(feedback) => {
            format!("User rejected permission for tool '{tool_name}' with feedback: {feedback}")
        }
        None => format!("User rejected permission for tool '{tool_name}'"),
    }
}

#[derive(Debug)]
enum PermissionExecutionPlan {
    Allowed,
    Rejected { reason: String },
    Awaiting(Vec<PendingPermissionReceiver>),
}

#[derive(Debug, Clone)]
enum PermissionPlanDraft {
    Allowed,
    Rejected { reason: String },
    Requests(Vec<PermissionRequest>),
}

pub fn permission_project_id_for_workspace_identity(
    identity: &bitfun_services_core::workspace_identity::WorkspaceSessionIdentity,
    is_remote: bool,
) -> BitFunResult<String> {
    if !is_remote {
        return Ok(
            bitfun_services_core::workspace_identity::local_workspace_stable_storage_id(
                identity.logical_workspace_path(),
            ),
        );
    }

    if identity.hostname == "_unresolved" {
        let connection_id = identity.remote_connection_id.as_deref().ok_or_else(|| {
            BitFunError::validation(
                "Unresolved remote workspace permission identity has no connection id".to_string(),
            )
        })?;
        let key = bitfun_services_core::workspace_identity::unresolved_remote_session_storage_key(
            connection_id,
            identity.logical_workspace_path(),
        );
        return Ok(format!("remote_unresolved_{key}"));
    }

    Ok(
        bitfun_services_core::workspace_identity::remote_workspace_stable_id(
            &identity.hostname,
            identity.logical_workspace_path(),
        ),
    )
}

fn permission_project_id(context: &ToolUseContext) -> BitFunResult<String> {
    let workspace = context.workspace.as_ref().ok_or_else(|| {
        BitFunError::validation("A workspace is required for file permissions".to_string())
    })?;
    permission_project_id_for_workspace_identity(&workspace.session_identity, workspace.is_remote())
}

fn permission_project_path(context: &ToolUseContext) -> BitFunResult<String> {
    let workspace = context.workspace.as_ref().ok_or_else(|| {
        BitFunError::validation("A workspace is required for file permissions".to_string())
    })?;
    Ok(workspace
        .session_identity
        .logical_workspace_path()
        .to_string())
}

const ACCOUNT_PERMISSION_SCOPE: &str = "account";
const ACCOUNT_PERMISSION_PROJECT_ID: &str = "__bitfun_account_actions__";
const ACCOUNT_PERMISSION_PROJECT_PATH: &str = "BitFun account";

fn permission_scope(
    context: &ToolUseContext,
    intents: &[PermissionIntent],
) -> BitFunResult<(String, String)> {
    if context.workspace.is_some() {
        return Ok((
            permission_project_id(context)?,
            permission_project_path(context)?,
        ));
    }

    let account_scoped = intents.iter().all(|intent| {
        intent
            .display_metadata
            .get("permissionScope")
            .and_then(serde_json::Value::as_str)
            == Some(ACCOUNT_PERMISSION_SCOPE)
    });
    if account_scoped {
        return Ok((
            ACCOUNT_PERMISSION_PROJECT_ID.to_string(),
            ACCOUNT_PERMISSION_PROJECT_PATH.to_string(),
        ));
    }

    Err(BitFunError::validation(
        "A workspace is required for file permissions".to_string(),
    ))
}

fn permission_resource_case_sensitivity(
    context: &ToolUseContext,
) -> PermissionResourceCaseSensitivity {
    if context.is_remote() || !cfg!(windows) {
        PermissionResourceCaseSensitivity::Sensitive
    } else {
        PermissionResourceCaseSensitivity::Insensitive
    }
}

const SUBAGENT_LAUNCH_TOOL_NAME: &str = "Task";

/// Native hook session facts derived from one tool task.
fn native_hook_session_facts<'a>(
    context: &'a ToolExecutionContext,
    options: &ToolExecutionOptions,
) -> NativeHookSessionFacts<'a> {
    NativeHookSessionFacts {
        session_id: &context.session_id,
        turn_id: Some(&context.dialog_turn_id),
        workspace_root: context
            .workspace
            .as_ref()
            .map(|workspace| workspace.root_path()),
        is_remote_workspace: context
            .workspace
            .as_ref()
            .is_some_and(|workspace| workspace.is_remote()),
        model: &context.primary_model_facts.model_id,
        bypass_permissions: options.auto_approve_ask,
    }
}

/// Tool pipeline
#[derive(Clone)]
pub struct ToolPipeline {
    tool_registry: Arc<TokioRwLock<ToolRegistry>>,
    state_manager: Arc<ToolStateManager>,
    cancellation_tokens: ToolCancellationTokenStore,
    computer_use_host: Option<ComputerUseHostRef>,
    permission_request_manager: Option<Arc<PermissionRequestManager>>,
    permission_plans: Arc<TokioMutex<HashMap<String, PermissionExecutionPlan>>>,
    /// Tool task ids a PreToolUse hook approved. The approval waives the
    /// interactive permission prompt only; policy denials still apply.
    hook_preapprovals: Arc<TokioMutex<HashSet<String>>>,
    /// Tool task ids whose admission was rejected before execution (stale
    /// tool catalog, deferred-tool gateway, runtime restrictions). Such
    /// rejections are protocol-layer outcomes, not execution violations.
    admission_rejected_tasks: Arc<TokioMutex<HashSet<String>>>,
    /// Session-scoped auto-reloaded deferred-tool specs (F2). A stale spec
    /// reloaded by [`Self::reload_stale_deferred_tool_spec`] is recorded here
    /// so later rounds that reconstruct loaded specs from the message history
    /// (the synthesized GetToolSpec result never becomes part of the
    /// conversation) can merge the refreshed generation back instead of
    /// re-triggering the reload every round.
    session_loaded_deferred_specs: Arc<TokioMutex<HashMap<String, Vec<LoadedDeferredToolSpec>>>>,
    /// R-MR-11 读取/搜索重复拦截：会话级「连续同目标指纹」追踪。
    ///
    /// 读取/搜索类工具（Read/Grep/Glob/LS/WebSearch/WebFetch）连续操作同一
    /// 目标（同文件路径 / 同关键词+路径 / 同 pattern / 同 query / 同 URL /
    /// 同路径）达 `repeated_read_limit` 次时，第 N 次调用被本地拦截——不执行
    /// 工具、不发起 LLM 请求（零请求），并把引导正确做法的提示作为 tool
    /// result 返回。中间插入其他工具调用 / 其他目标 / 文本产出 → 计数重置
    /// （交叉引用 A→B→A 不误伤）。配置：`ai.thresholds.execution.*`。
    repeated_read_states: Arc<TokioMutex<HashMap<String, RepeatedReadSessionState>>>,
    /// R-WF-22: write-like tool (Write/Edit/Delete/ExecCommand) in-flight
    /// protection. Tracks the task ids currently executing inside an atomic
    /// unit. When a round injection CancelRunning path sees a write-like tool
    /// still running, cancellation is deferred until the atomic unit completes
    /// to avoid half-written files. Zero type changes: consumer-side logic only.
    active_write_like_tools: Arc<TokioMutex<HashSet<String>>>,
}

/// R-MR-11 会话级重复读取拦截的连续计数状态。
#[derive(Debug, Clone, Default)]
struct RepeatedReadSessionState {
    /// 当前连续同目标指纹（None = 无连续目标，下个读取类调用直接建立）。
    current_target: Option<String>,
    /// 当前目标已连续出现的次数（含本次）。
    consecutive_count: usize,
    /// 最近一次被拦截提示的摘要（用于避免连续重复刷屏）。
    last_intercepted_message: Option<String>,
}

impl ToolPipeline {
    pub fn new(
        tool_registry: Arc<TokioRwLock<ToolRegistry>>,
        state_manager: Arc<ToolStateManager>,
        computer_use_host: Option<ComputerUseHostRef>,
    ) -> Self {
        Self {
            tool_registry,
            state_manager,
            cancellation_tokens: ToolCancellationTokenStore::new(),
            computer_use_host,
            permission_request_manager: None,
            permission_plans: Arc::new(TokioMutex::new(HashMap::new())),
            hook_preapprovals: Arc::new(TokioMutex::new(HashSet::new())),
            admission_rejected_tasks: Arc::new(TokioMutex::new(HashSet::new())),
            session_loaded_deferred_specs: Arc::new(TokioMutex::new(HashMap::new())),
            repeated_read_states: Arc::new(TokioMutex::new(HashMap::new())),
            active_write_like_tools: Arc::new(TokioMutex::new(HashSet::new())),
        }
    }

    pub fn with_permission_request_manager(
        mut self,
        permission_request_manager: Arc<PermissionRequestManager>,
    ) -> Self {
        self.permission_request_manager = Some(permission_request_manager);
        self
    }

    pub fn computer_use_host(&self) -> Option<ComputerUseHostRef> {
        self.computer_use_host.clone()
    }

    async fn draft_permission_plan(
        &self,
        task: ToolTask,
        tool_name: String,
        intents: Vec<PermissionIntent>,
        context: ToolUseContext,
    ) -> BitFunResult<PermissionPlanDraft> {
        if intents.is_empty() {
            return Ok(PermissionPlanDraft::Allowed);
        }

        let (project_id, project_path) = permission_scope(&context, &intents)?;
        let permission_policy = task.options.permission_policy.clone();
        let case_sensitivity = permission_resource_case_sensitivity(&context);
        let round_id = task.context.round_id.clone();
        let tool_call_id = task.tool_call.tool_id.clone();
        let session_id = task.context.session_id.clone();
        let agent_type = task.context.agent_type.clone();
        let permission_delegation = task.context.permission_delegation.clone().or_else(|| {
            task.context
                .subagent_parent_info
                .as_ref()
                .map(|parent| parent.permission_delegation_context(&agent_type))
        });
        let manager = self.permission_request_manager.clone();
        let grants = match manager {
            Some(ref manager) => manager
                .list_project_grants(&project_id)
                .await
                .map_err(|error| BitFunError::service(error.to_string()))?,
            None => Vec::new(),
        };
        let asks =
            match plan_permission_intents(intents, &permission_policy, &grants, case_sensitivity) {
                PermissionIntentPlan::Allowed => return Ok(PermissionPlanDraft::Allowed),
                PermissionIntentPlan::Denied(intent) => {
                    return Ok(PermissionPlanDraft::Rejected {
                        reason: format!(
                            "Permission policy denied '{}' for {}",
                            intent.action,
                            intent.resources.join(", ")
                        ),
                    });
                }
                PermissionIntentPlan::RequiresApproval(intents) => intents,
            };

        // A PreToolUse hook already approved this call. The approval reaches
        // here — after policy evaluation — precisely so that it waives only
        // the interactive prompt: a policy Deny above has already returned.
        if self.hook_preapprovals.lock().await.contains(&tool_call_id) {
            return Ok(PermissionPlanDraft::Allowed);
        }

        // The tool call would prompt the user: give PermissionRequest hooks
        // a chance to decide first. An explicit hook decision replaces the
        // interactive prompt for this invocation.
        if let Some(hook_decision) = native_hooks::dispatch_permission_request(
            native_hook_session_facts(&task.context, &task.options),
            &tool_name,
            &task.invocation.effective_arguments,
        )
        .await
        {
            if hook_decision.allow {
                info!(
                    "PermissionRequest hook allowed tool call without prompting: tool_name={}",
                    tool_name
                );
                return Ok(PermissionPlanDraft::Allowed);
            }
            let reason = hook_decision.message.unwrap_or_else(|| {
                format!("A PermissionRequest hook denied the '{tool_name}' tool call.")
            });
            info!(
                "PermissionRequest hook denied tool call: tool_name={}",
                tool_name
            );
            return Ok(PermissionPlanDraft::Rejected { reason });
        }

        if manager.is_none() {
            return Err(BitFunError::service(
                "Permission request manager is unavailable for a file tool request".to_string(),
            ));
        }

        let requests = asks
            .into_iter()
            .map(|intent| PermissionRequest {
                request_id: uuid::Uuid::new_v4().to_string(),
                round_id: round_id.clone(),
                order: task.tool_call_order,
                tool_call_id: Some(tool_call_id.clone()),
                project_path: Some(project_path.clone()),
                project_id: project_id.clone(),
                session_id: session_id.clone(),
                agent_id: agent_type.clone(),
                action: intent.action,
                resources: intent.resources,
                save_resources: intent.save_resources,
                source: PermissionRequestSource {
                    kind: PermissionRequestSourceKind::ToolCall,
                    identity: tool_name.clone(),
                },
                delegation: permission_delegation.clone(),
                display_metadata: intent.display_metadata,
            })
            .collect();

        Ok(PermissionPlanDraft::Requests(requests))
    }

    async fn register_permission_requests(
        &self,
        requests: Vec<PermissionRequest>,
        dialog_turn_id: &str,
        auto_approve: bool,
    ) -> BitFunResult<Vec<PendingPermissionReceiver>> {
        let manager = self.permission_request_manager.as_ref().ok_or_else(|| {
            BitFunError::service(
                "Permission request manager is unavailable for a file tool request".to_string(),
            )
        })?;

        let receivers = if auto_approve {
            manager
                .register_batch_non_interactive_for_turn(
                    requests.clone(),
                    dialog_turn_id.to_string(),
                )
                .await
        } else {
            manager
                .register_batch_for_turn(requests.clone(), dialog_turn_id.to_string())
                .await
        }
        .map_err(|error| BitFunError::service(error.to_string()))?;

        if auto_approve {
            for request in &requests {
                if let Err(error) = manager
                    .reply(
                        &request.request_id,
                        PermissionReply::Once,
                        bitfun_runtime_ports::PermissionReplySource::AutoApprove,
                    )
                    .await
                {
                    self.cancel_permission_request_ids(
                        requests
                            .iter()
                            .map(|request| request.request_id.clone())
                            .collect(),
                        "Automatic permission approval failed".to_string(),
                    )
                    .await;
                    return Err(BitFunError::service(error.to_string()));
                }
            }
        }

        Ok(receivers)
    }

    /// Run PreToolUse hooks for every valid task and record their decisions
    /// as pre-seeded permission plans. `updatedInput` rewrites the stored
    /// task arguments before validation and permission planning observe them.
    async fn apply_pre_tool_use_hooks(&self, task_ids: &[String]) {
        for task_id in task_ids {
            let Some(task) = self.state_manager.get_task(task_id) else {
                continue;
            };
            if task.invocation_resolution_error.is_some()
                || task.tool_call.tool_name.is_empty()
                || task.tool_call.is_error
            {
                continue;
            }
            let tool_name = task.invocation.effective_tool_name.clone();
            let decision = native_hooks::dispatch_pre_tool_use(
                native_hook_session_facts(&task.context, &task.options),
                &tool_name,
                &task.tool_call.tool_id,
                &task.invocation.effective_arguments,
            )
            .await;
            if let Some(updated_input) = decision.updated_input {
                if self
                    .state_manager
                    .update_task_arguments(task_id, updated_input)
                {
                    info!(
                        "PreToolUse hook rewrote tool arguments: tool_name={}, tool_id={}",
                        tool_name, task_id
                    );
                }
            }
            if let Some(reason) = decision.deny_reason {
                // A hook denial is strictly more restrictive than the
                // permission policy, so it can short-circuit planning.
                info!(
                    "PreToolUse hook denied tool call: tool_name={}, tool_id={}",
                    tool_name, task_id
                );
                self.permission_plans.lock().await.insert(
                    task_id.clone(),
                    PermissionExecutionPlan::Rejected { reason },
                );
            } else if decision.allow {
                // A hook approval only waives the interactive prompt. It is
                // recorded for the planner rather than short-circuiting it,
                // so a policy Deny still rejects the call.
                info!(
                    "PreToolUse hook approved tool call without prompting: tool_name={}, tool_id={}",
                    tool_name, task_id
                );
                self.hook_preapprovals.lock().await.insert(task_id.clone());
            }
        }
    }

    /// Run PostToolUse hooks for a completed tool call and fold blocking
    /// feedback and additional context into the model-visible result text.
    async fn apply_post_tool_use_hooks(
        &self,
        task: &ToolTask,
        tool_name: &str,
        tool_id: &str,
        tool_result: &mut ModelToolResult,
    ) {
        let tool_response = serde_json::json!({
            "result": match &tool_result.result_for_assistant {
                Some(text) => serde_json::Value::String(text.clone()),
                None => tool_result.result.clone(),
            },
            "is_error": tool_result.is_error,
        });
        let decision = native_hooks::dispatch_post_tool_use(
            native_hook_session_facts(&task.context, &task.options),
            tool_name,
            tool_id,
            &task.invocation.effective_arguments,
            &tool_response,
        )
        .await;
        let mut hook_sections = Vec::new();
        if let Some(reason) = decision.block_reason {
            info!(
                "PostToolUse hook returned blocking feedback: tool_name={}, tool_id={}",
                tool_name, tool_id
            );
            hook_sections.push(format!("PostToolUse hook feedback (blocking): {reason}"));
        }
        for context in decision.additional_context {
            hook_sections.push(format!("PostToolUse hook context: {context}"));
        }

        if hook_sections.is_empty() {
            return;
        }
        let original = tool_result.result_for_assistant.take().unwrap_or_default();
        let appended = hook_sections.join("\n");
        tool_result.result_for_assistant = Some(if original.is_empty() {
            appended
        } else {
            format!("{original}\n\n{appended}")
        });
    }

    async fn prepare_permission_plans(&self, task_ids: &[String]) -> BitFunResult<()> {
        let mut drafts = Vec::with_capacity(task_ids.len());
        let mut ordered_requests = Vec::new();

        for task_id in task_ids {
            // A PreToolUse hook decision already produced a plan for this
            // task; keep it instead of drafting (and possibly prompting).
            if self.permission_plans.lock().await.contains_key(task_id) {
                continue;
            }
            let Some(task) = self.state_manager.get_task(task_id) else {
                continue;
            };
            let tool_name = task.invocation.effective_tool_name.clone();
            if task.invocation_resolution_error.is_some()
                || task.tool_call.tool_name.is_empty()
                || task.tool_call.is_error
                || recovered_write_has_potentially_truncated_marked_path(
                    &tool_name,
                    &task.invocation.effective_arguments,
                    task.tool_call.repair_kind,
                    task.tool_call.recovered_from_truncation,
                )
            {
                continue;
            }
            let tool = {
                let registry = self.tool_registry.read().await;
                let effective_restrictions = effective_runtime_tool_restrictions(
                    &task.context.session_id,
                    &task.context.runtime_tool_restrictions,
                );
                if validate_tool_execution_admission(ToolExecutionAdmissionRequest {
                    tool_name: &tool_name,
                    allowed_tools: &task.context.allowed_tools,
                    runtime_tool_restrictions: &effective_restrictions,
                    user_enabled_tools: &task.context.user_enabled_tools,
                    tool_arguments: &task.invocation.effective_arguments,
                    deferred_tools: &task.context.deferred_tools,
                    loaded_deferred_tool_specs: &task.context.loaded_deferred_tool_specs,
                    current_catalog_generation: registry.current_snapshot_generation(),
                    get_tool_spec_tool_name: GET_TOOL_SPEC_TOOL_NAME,
                })
                .is_err()
                {
                    continue;
                }
                registry.get_tool(&tool_name)
            };
            let Some(tool) = tool else {
                continue;
            };
            let tool_context = self.build_tool_use_context(&task, CancellationToken::new());
            let validation = tool
                .validate_input(&task.invocation.effective_arguments, Some(&tool_context))
                .await;
            if !validation.result {
                continue;
            }
            let intents =
                tool.permission_intents(&task.invocation.effective_arguments, &tool_context)?;
            let draft = self
                .draft_permission_plan(
                    task.clone(),
                    tool_name.clone(),
                    intents,
                    tool_context.clone(),
                )
                .await?;
            if let PermissionPlanDraft::Requests(requests) = &draft {
                ordered_requests.extend(
                    requests
                        .iter()
                        .cloned()
                        .map(|request| (task_id.clone(), request)),
                );
            }
            drafts.push((task_id.clone(), draft));
        }

        if !ordered_requests.is_empty() {
            let batch_requests = ordered_requests
                .iter()
                .map(|(_, request)| request.clone())
                .collect::<Vec<_>>();
            let auto_approve = task_ids
                .first()
                .and_then(|task_id| self.state_manager.get_task(task_id))
                .is_some_and(|task| task.options.auto_approve_ask);
            let dialog_turn_id = task_ids
                .first()
                .and_then(|task_id| self.state_manager.get_task(task_id))
                .map(|task| task.context.dialog_turn_id)
                .ok_or_else(|| {
                    BitFunError::service("Permission batch lost its owning Dialog Turn".to_string())
                })?;
            let receivers = self
                .register_permission_requests(batch_requests, &dialog_turn_id, auto_approve)
                .await?;

            let mut receivers_by_task = HashMap::<String, Vec<PendingPermissionReceiver>>::new();
            for ((task_id, _), receiver) in ordered_requests.into_iter().zip(receivers) {
                receivers_by_task.entry(task_id).or_default().push(receiver);
            }
            for (task_id, draft) in &drafts {
                if let PermissionPlanDraft::Requests(_) = draft {
                    let receivers = receivers_by_task.remove(task_id).ok_or_else(|| {
                        BitFunError::service(format!(
                            "Permission plan lost its pending receivers for tool task '{task_id}'"
                        ))
                    })?;
                    self.permission_plans.lock().await.insert(
                        task_id.clone(),
                        PermissionExecutionPlan::Awaiting(receivers),
                    );
                }
            }
        }

        for (task_id, draft) in drafts {
            match draft {
                PermissionPlanDraft::Allowed => {
                    self.permission_plans
                        .lock()
                        .await
                        .insert(task_id, PermissionExecutionPlan::Allowed);
                }
                PermissionPlanDraft::Rejected { reason } => {
                    self.permission_plans
                        .lock()
                        .await
                        .insert(task_id, PermissionExecutionPlan::Rejected { reason });
                }
                PermissionPlanDraft::Requests(_) => {}
            }
        }

        Ok(())
    }

    async fn await_prepared_permission_plan(
        &self,
        task_id: &str,
        cancellation_token: &CancellationToken,
    ) -> BitFunResult<PermissionAuthorization> {
        let Some(plan) = self.permission_plans.lock().await.remove(task_id) else {
            return Ok(PermissionAuthorization::Allowed);
        };

        self.await_permission_execution_plan(plan, cancellation_token)
            .await
    }

    async fn await_permission_execution_plan(
        &self,
        plan: PermissionExecutionPlan,
        cancellation_token: &CancellationToken,
    ) -> BitFunResult<PermissionAuthorization> {
        let receivers = match plan {
            PermissionExecutionPlan::Allowed => return Ok(PermissionAuthorization::Allowed),
            PermissionExecutionPlan::Rejected { reason } => {
                return Ok(PermissionAuthorization::PolicyDenied { reason });
            }
            PermissionExecutionPlan::Awaiting(receivers) => receivers,
        };

        let mut receivers = receivers.into_iter();
        while let Some(pending) = receivers.next() {
            let request_id = pending.request_id().to_string();
            let outcome = tokio::select! {
                outcome = pending.wait() => outcome,
                _ = cancellation_token.cancelled() => {
                    let remaining = std::iter::once(request_id.clone())
                        .chain(receivers.map(|pending| pending.request_id().to_string()));
                    self.cancel_permission_request_ids(
                        remaining.collect(),
                        "Tool execution was cancelled".to_string(),
                    )
                    .await;
                    return Err(BitFunError::Cancelled(
                        "Tool execution was cancelled while awaiting permission".to_string(),
                    ));
                }
            };

            match outcome {
                PermissionWaitOutcome::Replied(PermissionReply::Once | PermissionReply::Always) => {
                }
                PermissionWaitOutcome::Replied(PermissionReply::Reject { feedback }) => {
                    self.cancel_permission_request_ids(
                        receivers
                            .map(|pending| pending.request_id().to_string())
                            .collect(),
                        "Another permission request for this tool was rejected".to_string(),
                    )
                    .await;
                    let feedback = feedback
                        .map(|feedback| feedback.trim().to_string())
                        .filter(|feedback| !feedback.is_empty());
                    return Ok(PermissionAuthorization::UserRejected { feedback });
                }
                PermissionWaitOutcome::Cancelled { reason } => {
                    self.cancel_permission_request_ids(
                        receivers
                            .map(|pending| pending.request_id().to_string())
                            .collect(),
                        "Another permission request for this tool was cancelled".to_string(),
                    )
                    .await;
                    return Err(BitFunError::Cancelled(reason));
                }
            }

            if cancellation_token.is_cancelled() {
                self.cancel_permission_request_ids(
                    receivers
                        .map(|pending| pending.request_id().to_string())
                        .collect(),
                    "Tool execution was cancelled".to_string(),
                )
                .await;
                return Err(BitFunError::Cancelled(
                    "Tool execution was cancelled after permission reply".to_string(),
                ));
            }
        }

        Ok(PermissionAuthorization::Allowed)
    }

    async fn cancel_permission_request_ids(&self, request_ids: Vec<String>, reason: String) {
        let Some(manager) = self.permission_request_manager.as_ref() else {
            return;
        };
        for request_id in request_ids {
            if let Err(error) = manager.cancel_request(&request_id, reason.clone()).await {
                warn!(
                    "Failed to cancel prepared permission request: request_id={}, error={}",
                    request_id, error
                );
            }
        }
    }

    async fn cleanup_permission_plans(&self, task_ids: &[String], reason: String) {
        {
            // Hook approvals are scoped to the batch that produced them; a
            // later call must be evaluated on its own merits.
            let mut preapprovals = self.hook_preapprovals.lock().await;
            for task_id in task_ids {
                preapprovals.remove(task_id);
            }
        }
        for task_id in task_ids {
            let Some(plan) = self.permission_plans.lock().await.remove(task_id) else {
                continue;
            };
            if let PermissionExecutionPlan::Awaiting(receivers) = plan {
                self.cancel_permission_request_ids(
                    receivers
                        .into_iter()
                        .map(|pending| pending.request_id().to_string())
                        .collect(),
                    reason.clone(),
                )
                .await;
            }
        }
    }

    async fn authorize_permission_intents(
        &self,
        task: &ToolTask,
        tool_name: &str,
        intents: Vec<PermissionIntent>,
        context: &ToolUseContext,
        cancellation_token: &CancellationToken,
    ) -> BitFunResult<PermissionAuthorization> {
        let draft = self
            .draft_permission_plan(
                task.clone(),
                tool_name.to_string(),
                intents,
                context.clone(),
            )
            .await?;
        let plan = match draft {
            PermissionPlanDraft::Allowed => PermissionExecutionPlan::Allowed,
            PermissionPlanDraft::Rejected { reason } => {
                PermissionExecutionPlan::Rejected { reason }
            }
            PermissionPlanDraft::Requests(requests) => PermissionExecutionPlan::Awaiting(
                self.register_permission_requests(
                    requests,
                    &task.context.dialog_turn_id,
                    task.options.auto_approve_ask,
                )
                .await?,
            ),
        };

        self.await_permission_execution_plan(plan, cancellation_token)
            .await
    }

    fn pending_round_injection_tool_preemption(
        &self,
        context: &ToolExecutionContext,
    ) -> RoundInjectionToolPreemption {
        context
            .steering_interrupt
            .as_ref()
            .map(|interrupt| interrupt.pending_tool_preemption())
            .unwrap_or(RoundInjectionToolPreemption::None)
    }

    /// R-WF-22: whether a write-like tool (matched by is_write_like_tool_name)
    /// is still executing inside an atomic unit. When true, round injection
    /// interruption/cancellation must be deferred until the tool fully
    /// completes to avoid half-written files.
    async fn has_active_write_like_tools(&self) -> bool {
        !self.active_write_like_tools.lock().await.is_empty()
    }

    async fn mark_write_like_tool_started(&self, tool_id: &str, tool_name: &str) {
        if is_write_like_tool_name(tool_name) {
            self.active_write_like_tools
                .lock()
                .await
                .insert(tool_id.to_string());
        }
    }

    async fn mark_write_like_tool_finished(&self, tool_id: &str) {
        self.active_write_like_tools.lock().await.remove(tool_id);
    }

    /// R-WF-22 injection decision consumer: while a write-like tool is
    /// running, both interrupt/cancel signals resolve to "wait for the
    /// current atomic unit" — the remaining tool plan is still skipped as
    /// before, but the in-flight write operation itself is not interrupted.
    /// Read-like tools keep the original immediate-interrupt semantics.
    async fn should_interrupt_for_round_injection(
        &self,
        context: &ToolExecutionContext,
        tool_name: &str,
    ) -> bool {
        let pending = self.pending_round_injection_tool_preemption(context);
        if !pending.should_interrupt_after_current_atomic_unit() {
            return false;
        }
        if is_write_like_tool_name(tool_name) && self.has_active_write_like_tools().await {
            // A write-like tool is inside its atomic unit: defer the
            // injection until it completes. Semantically equivalent to
            // InterruptAfterCurrentAtomicUnit — wait for the write.
            return false;
        }
        true
    }

    /// R-WF-22 write-tool protection consumer for the round injection
    /// interruption path (CancelRunningCooperatively/Forcefully → cancel_tool):
    /// returns true while a write-like tool is running, deferring the cancel
    /// until the atomic unit completes (the execution side cancels after the
    /// tool finishes); with no write-like tool running, cancel proceeds
    /// immediately as before.
    async fn should_defer_cancel_for_active_write_like_tools(&self) -> bool {
        self.has_active_write_like_tools().await
    }

    async fn build_steering_interrupted_results(
        &self,
        task_ids: impl IntoIterator<Item = String>,
    ) -> Vec<ToolExecutionResult> {
        let mut results = Vec::new();
        for task_id in task_ids {
            let task = self.state_manager.get_task(&task_id);
            self.state_manager
                .update_state(
                    &task_id,
                    ToolExecutionState::Cancelled {
                        reason: USER_STEERING_INTERRUPTED_MESSAGE.to_string(),
                        duration_ms: None,
                        queue_wait_ms: None,
                        preflight_ms: None,
                        confirmation_wait_ms: None,
                        execution_ms: None,
                    },
                )
                .await;
            results.push(build_user_steering_interrupted_result(&task_id, task));
        }
        results
    }

    async fn append_execution_result(
        &self,
        task_id: &str,
        result: BitFunResult<ToolExecutionResult>,
        all_results: &mut Vec<ToolExecutionResult>,
    ) {
        match result {
            Ok(execution_result) => {
                all_results.push(execution_result);
            }
            Err(error) => {
                error!("Tool execution failed: error={}", error);
                // F3: an admission rejection (stale catalog, deferred gate,
                // runtime restriction) is a protocol-layer outcome, not an
                // execution violation.
                let mut rejected = self.admission_rejected_tasks.lock().await;
                rejected.remove(task_id);
                let error_result = build_error_execution_result(
                    task_id,
                    self.state_manager.get_task(task_id),
                    &error,
                );
                all_results.push(error_result);
            }
        }
    }

    async fn cancel_tools_for_round_injection(
        &self,
        task_ids: impl IntoIterator<Item = String>,
    ) -> BitFunResult<()> {
        for task_id in task_ids {
            self.cancel_tool(
                &task_id,
                ROUND_INJECTION_RUNNING_TOOL_CANCELLED_MESSAGE.to_string(),
            )
            .await?;
        }
        Ok(())
    }

    fn spawn_round_injection_cancellation_watch(
        &self,
        task_ids: Vec<String>,
        interrupt: Option<crate::agentic::round_preempt::DialogRoundInjectionInterrupt>,
    ) -> Option<tokio::task::JoinHandle<()>> {
        interrupt.as_ref()?;

        let pipeline = self.clone();
        Some(tokio::spawn(async move {
            let Some(interrupt) = interrupt else {
                return;
            };

            loop {
                if interrupt.should_cancel_running_tools() {
                    // R-WF-22: while a write-like tool is running, defer the
                    // cancel until the atomic unit completes (avoid
                    // half-written files). With no write-like tool running,
                    // cancel proceeds immediately as before.
                    if pipeline
                        .should_defer_cancel_for_active_write_like_tools()
                        .await
                    {
                        tokio::time::sleep(Duration::from_millis(25)).await;
                        continue;
                    }
                    let _ = pipeline.cancel_tools_for_round_injection(task_ids).await;
                    break;
                }
                tokio::time::sleep(Duration::from_millis(25)).await;
            }
        }))
    }

    /// Execute multiple tool calls using partitioned mixed scheduling.
    ///
    /// Consecutive concurrency-safe calls are grouped into a single batch and
    /// run in parallel; each non-safe call forms its own batch and runs serially.
    /// Batches are executed in order so that write-after-read dependencies are
    /// respected while reads still benefit from parallelism.
    pub async fn execute_tools(
        &self,
        tool_calls: Vec<ToolCall>,
        context: ToolExecutionContext,
        options: ToolExecutionOptions,
    ) -> BitFunResult<Vec<ToolExecutionResult>> {
        if tool_calls.is_empty() {
            return Ok(vec![]);
        }

        // F2: merge the session-scoped auto-reload cache into the caller-
        // provided loaded-spec set. Each round reconstructs loaded specs from
        // the conversation history, which never contains the synthesized
        // GetToolSpec result produced by an auto-reload, so without this merge
        // a spec refreshed in an earlier round would be stale again on the
        // next round and re-trigger the reload.
        let mut context = context;
        let cached_specs = self
            .cached_session_loaded_deferred_specs(&context.session_id)
            .await;
        if !cached_specs.is_empty() {
            context.loaded_deferred_tool_specs = merge_loaded_deferred_tool_specs(
                &context.loaded_deferred_tool_specs,
                &cached_specs,
            );
        }

        info!("Executing tools: count={}", tool_calls.len());
        let resolved_tool_calls = tool_calls
            .iter()
            .map(|tool_call| {
                let (invocation, resolution_error) =
                    resolve_pipeline_invocation(tool_call, &context);
                (tool_call.clone(), invocation, resolution_error)
            })
            .collect::<Vec<_>>();
        let tool_names = resolved_tool_calls
            .iter()
            .map(|(_, invocation, _)| invocation.effective_tool_name.clone())
            .collect::<Vec<_>>();

        let subagent_call_count = resolved_tool_calls
            .iter()
            .filter(|(_, invocation, _)| {
                invocation.effective_tool_name == SUBAGENT_LAUNCH_TOOL_NAME
            })
            .count();

        // Determine concurrency safety for each tool call
        let concurrency_flags: Vec<bool> = {
            let registry = self.tool_registry.read().await;
            resolved_tool_calls
                .iter()
                .map(|(_, invocation, resolution_error)| {
                    if resolution_error.is_some() {
                        return false;
                    }
                    let tool_is_concurrency_safe = registry
                        .get_tool(&invocation.effective_tool_name)
                        .and_then(|tool| {
                            resolve_contextual_tool(
                                tool,
                                context
                                    .workspace
                                    .as_ref()
                                    .map(|workspace| workspace.root_path()),
                                context
                                    .workspace
                                    .as_ref()
                                    .is_some_and(|workspace| workspace.is_remote()),
                            )
                        })
                        .map(|tool| tool.is_concurrency_safe(Some(&invocation.effective_arguments)))
                        .unwrap_or(false);
                    tool_call_concurrency_safe_for_batch(
                        &invocation.effective_tool_name,
                        tool_is_concurrency_safe,
                        subagent_call_count,
                        options.subagent_batch_execution_policy,
                    )
                })
                .collect()
        };
        let concurrency_safe_count = concurrency_flags.iter().filter(|&&flag| flag).count();

        // Create tasks for all tool calls
        let mut task_ids = Vec::with_capacity(resolved_tool_calls.len());
        for (tool_call_order, (tool_call, invocation, resolution_error)) in
            resolved_tool_calls.into_iter().enumerate()
        {
            let mut task = ToolTask::new_resolved(
                tool_call,
                invocation,
                resolution_error,
                context.clone(),
                options.clone(),
            );
            task.tool_call_order = tool_call_order as u32;
            let tool_id = self.state_manager.create_task(task).await;
            task_ids.push(tool_id);
        }

        // PreToolUse hooks run before permission planning so a hook decision
        // (deny / pre-approve / rewritten input) is visible to the planner
        // and no permission prompt is raised for calls a hook already decided.
        self.apply_pre_tool_use_hooks(&task_ids).await;

        if let Err(error) = self.prepare_permission_plans(&task_ids).await {
            self.cleanup_permission_plans(&task_ids, "Permission planning failed".to_string())
                .await;
            return Err(error);
        }

        if !options.allow_parallel {
            debug!(
                "Tool execution plan: total_tools={}, batches=1, concurrency_safe={}, non_concurrency_safe={}, allow_parallel=false, tools={}",
                task_ids.len(),
                concurrency_safe_count,
                task_ids.len().saturating_sub(concurrency_safe_count),
                tool_names.join(", ")
            );
            let result = self.execute_sequential(task_ids.clone()).await;
            self.cleanup_permission_plans(&task_ids, "Tool execution finished".to_string())
                .await;
            return result;
        }

        // Partition into batches of consecutive same-safety tool calls
        let batches = partition_tool_batches(&task_ids, &concurrency_flags);
        debug!(
            "Tool execution plan: total_tools={}, batches={}, concurrency_safe={}, non_concurrency_safe={}, allow_parallel=true, tools={}",
            task_ids.len(),
            batches.len(),
            concurrency_safe_count,
            task_ids.len().saturating_sub(concurrency_safe_count),
            tool_names.join(", ")
        );

        debug!(
            "Partitioned {} tools into {} batches for mixed execution",
            task_ids.len(),
            batches.len()
        );

        let mut all_results = Vec::with_capacity(task_ids.len());
        let mut batch_iter = batches.into_iter().enumerate().peekable();
        while let Some((batch_idx, batch)) = batch_iter.next() {
            let batch_context = batch
                .task_ids
                .first()
                .and_then(|task_id| self.state_manager.get_task(task_id))
                .map(|task| task.context);
            let batch_tool_name = batch
                .task_ids
                .first()
                .and_then(|task_id| self.state_manager.get_task(task_id))
                .map(|task| task.effective_tool_name().to_string());
            let batch_should_interrupt = match (batch_context.as_ref(), batch_tool_name.as_deref())
            {
                (Some(context), Some(tool_name)) => {
                    self.should_interrupt_for_round_injection(context, tool_name)
                        .await
                }
                _ => false,
            };
            if batch_should_interrupt {
                let remaining_task_ids = batch
                    .task_ids
                    .into_iter()
                    .chain(batch_iter.flat_map(|(_, batch)| batch.task_ids.into_iter()));
                all_results.extend(
                    self.build_steering_interrupted_results(remaining_task_ids)
                        .await,
                );
                break;
            }

            debug!(
                "Executing batch {}: {} tool(s), concurrent={}",
                batch_idx,
                batch.task_ids.len(),
                batch.is_concurrent
            );
            let batch_results = if batch.is_concurrent {
                self.execute_parallel(batch.task_ids).await?
            } else {
                self.execute_sequential(batch.task_ids).await?
            };
            all_results.extend(batch_results);
        }

        self.cleanup_permission_plans(&task_ids, "Tool execution finished".to_string())
            .await;
        Ok(all_results)
    }

    /// Execute tools in parallel
    async fn execute_parallel(
        &self,
        task_ids: Vec<String>,
    ) -> BitFunResult<Vec<ToolExecutionResult>> {
        let batch_interrupt = task_ids
            .first()
            .and_then(|task_id| self.state_manager.get_task(task_id))
            .and_then(|task| task.context.steering_interrupt.clone());
        let watch_handle =
            self.spawn_round_injection_cancellation_watch(task_ids.clone(), batch_interrupt);

        let futures: Vec<_> = task_ids
            .iter()
            .map(|id| self.execute_single_tool(id.clone()))
            .collect();

        let results = join_all(futures).await;
        if let Some(handle) = watch_handle {
            handle.abort();
            let _ = handle.await;
        }

        // Collect results, including failed results
        let mut all_results = Vec::new();
        for (idx, result) in results.into_iter().enumerate() {
            let task_id = &task_ids[idx];
            self.append_execution_result(task_id, result, &mut all_results)
                .await;
        }

        Ok(all_results)
    }

    /// Execute tools sequentially
    async fn execute_sequential(
        &self,
        task_ids: Vec<String>,
    ) -> BitFunResult<Vec<ToolExecutionResult>> {
        let mut results = Vec::new();

        let mut task_iter = task_ids.into_iter().peekable();
        while let Some(task_id) = task_iter.next() {
            let task = self.state_manager.get_task(&task_id);
            let should_interrupt = match task.as_ref() {
                Some(task) => {
                    self.should_interrupt_for_round_injection(
                        &task.context,
                        task.effective_tool_name(),
                    )
                    .await
                }
                None => false,
            };
            if should_interrupt {
                let remaining_task_ids = std::iter::once(task_id).chain(task_iter);
                results.extend(
                    self.build_steering_interrupted_results(remaining_task_ids)
                        .await,
                );
                break;
            }

            let interrupt = task.and_then(|task| task.context.steering_interrupt.clone());
            let watch_handle =
                self.spawn_round_injection_cancellation_watch(vec![task_id.clone()], interrupt);
            let result = self.execute_single_tool(task_id.clone()).await;
            if let Some(handle) = watch_handle {
                handle.abort();
                let _ = handle.await;
            }
            self.append_execution_result(&task_id, result, &mut results)
                .await;
        }

        Ok(results)
    }

    /// Resolve the admission gate and registered tool for one invocation.
    async fn resolve_tool_admission(
        &self,
        task: &ToolTask,
        tool_name: &str,
        tool_args: &serde_json::Value,
    ) -> (Result<(), ToolExecutionAdmissionRejection>, Option<ToolRef>) {
        let registry = self.tool_registry.read().await;
        let effective_restrictions = effective_runtime_tool_restrictions(
            &task.context.session_id,
            &task.context.runtime_tool_restrictions,
        );
        let admission = validate_tool_execution_admission(ToolExecutionAdmissionRequest {
            tool_name,
            allowed_tools: &task.context.allowed_tools,
            runtime_tool_restrictions: &effective_restrictions,
            user_enabled_tools: &task.context.user_enabled_tools,
            tool_arguments: tool_args,
            deferred_tools: &task.context.deferred_tools,
            loaded_deferred_tool_specs: &task.context.loaded_deferred_tool_specs,
            current_catalog_generation: registry.current_snapshot_generation(),
            get_tool_spec_tool_name: GET_TOOL_SPEC_TOOL_NAME,
        });
        (admission, registry.get_tool(tool_name))
    }

    /// Reload a stale deferred-tool spec through the GetToolSpec runtime path.
    ///
    /// Returns [`StaleSpecReloadOutcome::Reloaded`] with the refreshed
    /// loaded-spec set (existing entries merged with the reloaded one) when a
    /// fresh spec was observed, or [`StaleSpecReloadOutcome::NotReloadable`]
    /// with a classified reason when the reload cannot succeed — the caller
    /// then keeps the original admission rejection.
    async fn reload_stale_deferred_tool_spec(
        &self,
        task: &ToolTask,
        stale_tool_name: &str,
    ) -> StaleSpecReloadOutcome {
        let cancellation_token = task
            .options
            .parent_cancellation_token
            .as_ref()
            .map(CancellationToken::child_token)
            .unwrap_or_default();
        let tool_context = self.build_tool_use_context(task, cancellation_token);
        let input = serde_json::json!({ "tool_name": stale_tool_name });
        let results = match resolve_product_get_tool_spec_results(
            &input,
            &tool_context,
            GET_TOOL_SPEC_TOOL_NAME,
        )
        .await
        {
            Ok(results) => results,
            Err(error) => {
                warn!(
                    "Stale deferred-tool spec reload failed during GetToolSpec execution: tool_name={}, session_id={}, error={}",
                    stale_tool_name, task.context.session_id, error
                );
                return StaleSpecReloadOutcome::NotReloadable("GetToolSpec execution failed");
            }
        };
        let Some(result) = results.into_iter().next() else {
            warn!(
                "Stale deferred-tool spec reload returned no GetToolSpec result: tool_name={}, session_id={}",
                stale_tool_name, task.context.session_id
            );
            return StaleSpecReloadOutcome::NotReloadable("GetToolSpec returned no result");
        };
        let FrameworkToolResult::Result {
            data,
            result_for_assistant,
            image_attachments,
        } = result
        else {
            warn!(
                "Stale deferred-tool spec reload received a non-result GetToolSpec outcome: tool_name={}, session_id={}",
                stale_tool_name, task.context.session_id
            );
            return StaleSpecReloadOutcome::NotReloadable("GetToolSpec returned an error result");
        };
        // Synthesize a GetToolSpec ToolResult message and feed it through the
        // loaded-spec state collection channel so the refreshed generation is
        // observed by the same path that tracks model-initiated loads.
        let message = Message::tool_result(ModelToolResult {
            tool_id: task.tool_call.tool_id.clone(),
            tool_name: GET_TOOL_SPEC_TOOL_NAME.to_string(),
            effective_tool_name: None,
            result: data,
            result_for_assistant,
            is_error: false,
            duration_ms: Some(0),
            image_attachments,
        });
        let refreshed =
            collect_product_loaded_deferred_tool_specs(&[message], &task.context.deferred_tools);
        if refreshed.is_empty() {
            warn!(
                "Stale deferred-tool spec is not reloadable: tool_name={}, session_id={} — the tool is no longer part of the contextual deferred catalog or the GetToolSpec result lacks a catalog generation",
                stale_tool_name, task.context.session_id
            );
            return StaleSpecReloadOutcome::NotReloadable(
                "tool is not reloadable: not in the deferred catalog or result lacks catalog_generation",
            );
        }
        StaleSpecReloadOutcome::Reloaded(merge_loaded_deferred_tool_specs(
            &task.context.loaded_deferred_tool_specs,
            &refreshed,
        ))
    }

    /// Record freshly reloaded deferred-tool specs for a session so later
    /// rounds merge them back into the message-history-derived loaded-spec
    /// set instead of re-triggering the reload. Entries upsert by tool name.
    async fn record_session_loaded_deferred_specs(
        &self,
        session_id: &str,
        specs: &[LoadedDeferredToolSpec],
    ) {
        let mut cache = self.session_loaded_deferred_specs.lock().await;
        if cache.len() >= MAX_CACHED_SESSIONS_WITH_RELOADED_SPECS {
            // Defensive upper bound: drop the whole cache rather than letting
            // stale sessions accumulate unboundedly. Losing a session entry
            // only forces one extra auto-reload for that session.
            cache.clear();
        }
        let merged = merge_loaded_deferred_tool_specs(
            cache.get(session_id).map(Vec::as_slice).unwrap_or_default(),
            specs,
        );
        cache.insert(session_id.to_string(), merged);
    }

    /// Read the recorded auto-reloaded deferred-tool specs of a session.
    async fn cached_session_loaded_deferred_specs(
        &self,
        session_id: &str,
    ) -> Vec<LoadedDeferredToolSpec> {
        self.session_loaded_deferred_specs
            .lock()
            .await
            .get(session_id)
            .cloned()
            .unwrap_or_default()
    }

    /// R-MR-11 读取/搜索重复拦截判定。
    ///
    /// 命中「连续同目标 N 次」时返回拦截提示，否则返回 None（正常执行）。
    /// 副作用：更新会话级连续计数状态；中间有产出（写入类工具 / 其他工具 /
    /// 不同目标）时自动重置计数。
    async fn repeated_read_interception(
        &self,
        session_id: &str,
        tool_name: &str,
        arguments: &serde_json::Value,
    ) -> Option<String> {
        let thresholds = Self::execution_thresholds().await;
        if !thresholds.repeated_read_enabled {
            return None;
        }

        let Some(target) = repeated_read_target_fingerprint(tool_name, arguments) else {
            // 非读取/搜索类工具：重置连续计数（中间有产出/其他工具 → 重置）。
            self.reset_repeated_read_state(session_id).await;
            return None;
        };

        let mut states = self.repeated_read_states.lock().await;
        let state = states
            .entry(session_id.to_string())
            .or_insert_with(RepeatedReadSessionState::default);

        repeated_read_decide(&thresholds, tool_name, &target, arguments, state)
    }

    async fn reset_repeated_read_state(&self, session_id: &str) {
        if let Some(state) = self.repeated_read_states.lock().await.get_mut(session_id) {
            state.current_target = None;
            state.consecutive_count = 0;
            state.last_intercepted_message = None;
        }
    }

    /// 读取 `ai.thresholds.execution.*` 配置（R-MR-07 配置域扩展）。
    ///
    /// R-MR-07 未完成时按契约回退到常量默认值（enabled=true, limit=3,
    /// small_file_line_threshold=200），配置服务不可用/加载失败不影响拦截
    /// 可用性。
    async fn execution_thresholds() -> ExecutionThresholds {
        match crate::service::config::get_global_config_service().await {
            Ok(service) => service
                .get_config::<ExecutionThresholds>(Some("ai.thresholds.execution"))
                .await
                .unwrap_or_default(),
            Err(_) => ExecutionThresholds::default(),
        }
    }

    /// Execute single tool
    async fn execute_single_tool(&self, tool_id: String) -> BitFunResult<ToolExecutionResult> {
        // R-WF-22: write-like atomic-unit protection — register on entry;
        // every return path (success/failure/cancel/reject/timeout) must
        // pair with mark_write_like_tool_finished.
        let tool_name = self
            .state_manager
            .get_task(&tool_id)
            .map(|task| task.effective_tool_name().to_string())
            .unwrap_or_default();
        self.mark_write_like_tool_started(&tool_id, &tool_name)
            .await;
        let write_guard_result = self.execute_single_tool_inner(tool_id.clone()).await;
        self.mark_write_like_tool_finished(&tool_id).await;
        write_guard_result
    }

    async fn execute_single_tool_inner(
        &self,
        tool_id: String,
    ) -> BitFunResult<ToolExecutionResult> {
        let start_time = Instant::now();

        debug!("Starting tool execution: tool_id={}", tool_id);

        // Get task
        let mut task = self
            .state_manager
            .get_task(&tool_id)
            .ok_or_else(|| BitFunError::NotFound(format!("Tool task not found: {}", tool_id)))?;

        let wire_tool_name = task.tool_call.tool_name.clone();
        let tool_name = task.invocation.effective_tool_name.clone();
        let tool_args = task.invocation.effective_arguments.clone();
        let tool_is_error = task.tool_call.is_error;
        let repair_kind = task.tool_call.repair_kind;
        let recovered_from_truncation =
            repair_kind.is_write_tail_closure() || task.tool_call.recovered_from_truncation;
        let queue_wait_ms = elapsed_ms_since(task.created_at);
        let confirmation_wait_ms = 0;

        debug!(
            "Tool task details: tool_name={}, wire_tool_name={}, tool_id={}, queue_wait_ms={}",
            tool_name, wire_tool_name, tool_id, queue_wait_ms
        );

        let invalid_call_error = if let Some(error) = task.invocation_resolution_error.clone() {
            Some(error)
        } else if wire_tool_name.is_empty() || tool_is_error {
            Some(build_invalid_tool_call_error_message(
                &wire_tool_name,
                tool_is_error,
                recovered_from_truncation,
                None,
            ))
        } else if recovered_write_has_potentially_truncated_marked_path(
            &tool_name,
            &tool_args,
            repair_kind,
            recovered_from_truncation,
        ) {
            Some(
                "Recovered Write arguments are missing the newline separator between the path and content; refusing to execute because the path may be truncated."
                    .to_string(),
            )
        } else {
            None
        };

        if let Some(error_msg) = invalid_call_error {
            self.state_manager
                .update_state(
                    &tool_id,
                    ToolExecutionState::Failed {
                        error: error_msg.clone(),
                        is_retryable: false,
                        duration_ms: None,
                        queue_wait_ms: None,
                        preflight_ms: None,
                        confirmation_wait_ms: None,
                        execution_ms: None,
                    },
                )
                .await;

            return Err(BitFunError::Validation(error_msg));
        }

        match repair_kind {
            ToolArgumentRepairKind::WriteTailClosure => warn!(
                "Tool arguments recovered with Write close-only repair: tool_name={}, tool_id={}, session_id={}",
                tool_name, tool_id, task.context.session_id
            ),
            ToolArgumentRepairKind::PermissiveNormalToolJsonRepair => warn!(
                "Tool arguments repaired after normal tool-use completion: tool_name={}, tool_id={}, session_id={}",
                tool_name, tool_id, task.context.session_id
            ),
            ToolArgumentRepairKind::None if recovered_from_truncation => warn!(
                "Executing legacy recovered Write tool call without repair provenance: tool_name={}, tool_id={}, session_id={}",
                tool_name, tool_id, task.context.session_id
            ),
            ToolArgumentRepairKind::None => {}
        }

        // R-MR-11 读取/搜索重复拦截：连续同目标 N 次 → 拦截不执行。
        // 拦截 = 不调工具 + 不调 LLM（零请求）：本地构造提示并作为
        // tool result 返回，随消息历史回到模型侧。
        if let Some(block_message) = self
            .repeated_read_interception(&task.context.session_id, &tool_name, &tool_args)
            .await
        {
            warn!(
                "Repeated read intercepted (R-MR-11): session_id={}, tool_name={}, tool_id={}, message={}",
                task.context.session_id, tool_name, tool_id, block_message
            );
            self.state_manager
                .update_state(
                    &tool_id,
                    ToolExecutionState::Failed {
                        error: block_message.clone(),
                        is_retryable: false,
                        duration_ms: None,
                        queue_wait_ms: Some(queue_wait_ms),
                        preflight_ms: None,
                        confirmation_wait_ms: Some(confirmation_wait_ms),
                        execution_ms: None,
                    },
                )
                .await;
            return Ok(ToolExecutionResult {
                tool_id: tool_id.clone(),
                tool_name: wire_tool_name.clone(),
                effective_tool_name: tool_name.clone(),
                result: ModelToolResult {
                    tool_id,
                    tool_name: wire_tool_name.clone(),
                    effective_tool_name: persisted_effective_tool_name(&wire_tool_name, &tool_name),
                    result: serde_json::json!({
                        "category": "repeated_read_blocked",
                        "status": "skipped",
                        "message": block_message,
                    }),
                    result_for_assistant: Some(block_message),
                    is_error: false,
                    duration_ms: Some(elapsed_ms_u64(start_time)),
                    image_attachments: None,
                },
                execution_time_ms: elapsed_ms_u64(start_time),
            });
        }

        // Repetition alone is not execution failure: polling and status checks
        // may legitimately reuse identical arguments. The execution engine
        // evaluates repeated patterns only after observing actual tool results.
        let (admission, tool) = self
            .resolve_tool_admission(&task, &tool_name, &tool_args)
            .await;

        // F2: stale deferred-tool specs are refreshed automatically instead of
        // surfacing a protocol-layer admission failure. The GetToolSpec reload
        // goes through the same runtime path a model-initiated load uses, and
        // the refreshed spec is fed back through the loaded-spec state
        // collection channel before admission is re-run. Reloads are retried
        // in a loop (bounded by `MAX_STALE_SPEC_RELOAD_ATTEMPTS`) so a catalog
        // refresh racing the reload cannot leave the invocation stale, and
        // each successful reload is recorded in the session-scoped cache so
        // later rounds do not re-trigger the recovery. `RequiresGetToolSpec`
        // is intentionally not auto-recovered: the model must still unlock the
        // tool explicitly.
        let (admission, tool) = if let Err(err) = &admission {
            match err {
                ToolExecutionAdmissionRejection::Deferred(stale) if stale.is_stale_spec() => {
                    let mut admission = admission;
                    let mut tool = tool;
                    let mut reload_attempts = 0usize;
                    while matches!(
                        &admission,
                        Err(ToolExecutionAdmissionRejection::Deferred(stale))
                            if stale.is_stale_spec()
                    ) {
                        if reload_attempts >= MAX_STALE_SPEC_RELOAD_ATTEMPTS {
                            let last_rejection = match &admission {
                                Err(rejection) => rejection.to_string(),
                                Ok(()) => String::new(),
                            };
                            warn!(
                                "Stale deferred-tool spec reload attempts exhausted: tool_name={}, tool_id={}, session_id={}, attempts={}, last_rejection={}",
                                tool_name, tool_id, task.context.session_id, reload_attempts, last_rejection
                            );
                            break;
                        }
                        reload_attempts += 1;
                        match self
                            .reload_stale_deferred_tool_spec(&task, &tool_name)
                            .await
                        {
                            StaleSpecReloadOutcome::Reloaded(updated_specs) => {
                                task.context.loaded_deferred_tool_specs = updated_specs.clone();
                                self.record_session_loaded_deferred_specs(
                                    &task.context.session_id,
                                    &updated_specs,
                                )
                                .await;
                                info!(
                                    "Automatically reloaded stale deferred-tool spec: tool_name={}, tool_id={}, session_id={}, attempt={}",
                                    tool_name, tool_id, task.context.session_id, reload_attempts
                                );
                                (admission, tool) = self
                                    .resolve_tool_admission(&task, &tool_name, &tool_args)
                                    .await;
                            }
                            StaleSpecReloadOutcome::NotReloadable(reason) => {
                                warn!(
                                    "Stale deferred-tool spec reload skipped, keeping admission rejection: tool_name={}, tool_id={}, session_id={}, reason={}",
                                    tool_name, tool_id, task.context.session_id, reason
                                );
                                break;
                            }
                        }
                    }
                    (admission, tool)
                }
                _ => (admission, tool),
            }
        } else {
            (admission, tool)
        };

        if let Err(err) = admission {
            let error_msg = err.to_string();
            if task.invocation.is_deferred() {
                warn!("Deferred tool gateway admission rejected: {}", error_msg);
            } else {
                warn!("Tool execution admission rejected: {}", error_msg);
            }

            // F3: mark the task so the result sink reports `AdmissionRejected`
            // — admission rejections (stale catalog, deferred gateway,
            // runtime restrictions) are protocol-layer outcomes.
            self.admission_rejected_tasks
                .lock()
                .await
                .insert(tool_id.clone());

            self.state_manager
                .update_state(
                    &tool_id,
                    ToolExecutionState::Failed {
                        error: error_msg,
                        is_retryable: false,
                        duration_ms: None,
                        queue_wait_ms: None,
                        preflight_ms: None,
                        confirmation_wait_ms: None,
                        execution_ms: None,
                    },
                )
                .await;

            return Err(map_tool_execution_admission_rejection(err));
        }

        let registered_tool = tool.ok_or_else(|| {
            let error_msg = format!("Tool '{}' is not registered or enabled.", tool_name);
            error!("{}", error_msg);
            BitFunError::tool(error_msg)
        })?;

        let cancellation_token = task
            .options
            .parent_cancellation_token
            .as_ref()
            .map(CancellationToken::child_token)
            .unwrap_or_default();
        if cancellation_token.is_cancelled() {
            self.state_manager
                .update_state(
                    &tool_id,
                    ToolExecutionState::Cancelled {
                        reason: "Tool was cancelled before validation".to_string(),
                        duration_ms: Some(elapsed_ms_u64(start_time)),
                        queue_wait_ms: Some(queue_wait_ms),
                        preflight_ms: Some(elapsed_ms_u64(start_time)),
                        confirmation_wait_ms: Some(0),
                        execution_ms: None,
                    },
                )
                .await;
            return Err(BitFunError::Cancelled(
                "Tool was cancelled before validation".to_string(),
            ));
        }
        let tool_context = self.build_tool_use_context(&task, cancellation_token.clone());
        // Keep the registered mux in the execution path. It rechecks the
        // persisted conflict choice immediately before dispatch and applies
        // remote fail-closed routing from the full ToolUseContext.
        let tool = registered_tool;
        let validation = tool.validate_input(&tool_args, Some(&tool_context)).await;
        if !validation.result {
            let error_msg = validation
                .message
                .unwrap_or_else(|| format!("Invalid input for tool '{}'", tool_name));
            self.state_manager
                .update_state(
                    &tool_id,
                    ToolExecutionState::Failed {
                        error: error_msg.clone(),
                        is_retryable: false,
                        duration_ms: None,
                        queue_wait_ms: None,
                        preflight_ms: None,
                        confirmation_wait_ms: None,
                        execution_ms: None,
                    },
                )
                .await;
            return Err(BitFunError::Validation(error_msg));
        }
        if let Some(message) = validation
            .message
            .filter(|message| !message.trim().is_empty())
        {
            warn!(
                "Tool input validation warning: tool_name={}, warning={}",
                tool_name, message
            );
        }

        // Register cancellation only after deterministic validation and registry lookup succeed.
        self.cancellation_tokens
            .insert(tool_id.to_string(), cancellation_token.clone());

        if cancellation_token.is_cancelled() {
            self.state_manager
                .update_state(
                    &tool_id,
                    ToolExecutionState::Cancelled {
                        reason: "Tool was cancelled during validation".to_string(),
                        duration_ms: Some(elapsed_ms_u64(start_time)),
                        queue_wait_ms: Some(queue_wait_ms),
                        preflight_ms: Some(elapsed_ms_u64(start_time)),
                        confirmation_wait_ms: Some(0),
                        execution_ms: None,
                    },
                )
                .await;
            self.cancellation_tokens.remove(&tool_id);
            return Err(BitFunError::Cancelled(
                "Tool was cancelled during validation".to_string(),
            ));
        }

        let has_prepared_plan = self.permission_plans.lock().await.contains_key(&tool_id);
        let permission_authorization = if has_prepared_plan {
            self.await_prepared_permission_plan(&tool_id, &cancellation_token)
                .await
        } else {
            let permission_intents = tool.permission_intents(&tool_args, &tool_context)?;
            self.authorize_permission_intents(
                &task,
                &tool_name,
                permission_intents,
                &tool_context,
                &cancellation_token,
            )
            .await
        };

        let rejected = match permission_authorization {
            Ok(PermissionAuthorization::Allowed) => None,
            Ok(PermissionAuthorization::UserRejected { feedback }) => {
                let reason = user_rejection_audit_reason(&tool_name, feedback.as_deref());
                let result = build_user_rejected_tool_result(
                    &tool_id,
                    self.state_manager.get_task(&tool_id),
                    feedback.as_deref(),
                );
                Some((reason, result))
            }
            Ok(PermissionAuthorization::PolicyDenied { reason }) => {
                let result = build_permission_denied_tool_result(
                    &tool_id,
                    self.state_manager.get_task(&tool_id),
                    &reason,
                );
                Some((reason, result))
            }
            Err(error) => {
                self.cancellation_tokens.remove(&tool_id);
                return Err(error);
            }
        };

        if let Some((reason, result)) = rejected {
            let preflight_ms = elapsed_ms_u64(start_time);
            self.state_manager
                .update_state(
                    &tool_id,
                    ToolExecutionState::Rejected {
                        reason,
                        duration_ms: Some(preflight_ms),
                        queue_wait_ms: Some(queue_wait_ms),
                        preflight_ms: Some(preflight_ms),
                        confirmation_wait_ms: Some(0),
                        execution_ms: None,
                    },
                )
                .await;
            self.cancellation_tokens.remove(&tool_id);
            return Ok(result);
        }

        debug!("Executing tool: tool_name={}", tool_name);

        let is_streaming = tool.supports_streaming();
        let preflight_ms = elapsed_ms_u64(start_time);

        if cancellation_token.is_cancelled() {
            self.state_manager
                .update_state(
                    &tool_id,
                    ToolExecutionState::Cancelled {
                        reason: "Tool was cancelled before execution".to_string(),
                        duration_ms: Some(elapsed_ms_u64(start_time)),
                        queue_wait_ms: Some(queue_wait_ms),
                        preflight_ms: Some(preflight_ms),
                        confirmation_wait_ms: Some(confirmation_wait_ms),
                        execution_ms: None,
                    },
                )
                .await;
            self.cancellation_tokens.remove(&tool_id);
            return Err(BitFunError::Cancelled(
                "Tool was cancelled before execution".to_string(),
            ));
        }

        // Set initial state
        if is_streaming {
            self.state_manager
                .update_state(
                    &tool_id,
                    ToolExecutionState::Streaming {
                        started_at: std::time::SystemTime::now(),
                        chunks_received: 0,
                    },
                )
                .await;
        } else {
            self.state_manager
                .update_state(
                    &tool_id,
                    ToolExecutionState::Running {
                        started_at: std::time::SystemTime::now(),
                        progress: None,
                    },
                )
                .await;
        }

        let execution_started_at = Instant::now();
        let tool_context = self.build_tool_use_context(&task, cancellation_token.clone());
        let result = self
            .execute_with_retry(&task, cancellation_token.clone(), tool)
            .await;
        let execution_ms = elapsed_ms_u64(execution_started_at);

        self.cancellation_tokens.remove(&tool_id);

        match result {
            Ok(tool_result) => {
                let duration_ms = elapsed_ms_u64(start_time);
                let mut tool_result =
                    tool_result_storage::maybe_persist_large_tool_result_for_tool(
                        tool_result,
                        &tool_name,
                        &tool_context,
                    )
                    .await;
                tool_result.duration_ms = Some(duration_ms);

                if !matches!(repair_kind, ToolArgumentRepairKind::None) || recovered_from_truncation
                {
                    let original = tool_result.result_for_assistant.unwrap_or_default();
                    let notice = match repair_kind {
                        ToolArgumentRepairKind::WriteTailClosure => {
                            build_write_tail_closure_notice(&tool_name)
                        }
                        ToolArgumentRepairKind::PermissiveNormalToolJsonRepair => {
                            build_normal_tool_json_repair_notice(&tool_name)
                        }
                        // Old persisted calls carry only the legacy boolean.
                        ToolArgumentRepairKind::None => build_write_tail_closure_notice(&tool_name),
                    };
                    tool_result.result_for_assistant = Some(if original.is_empty() {
                        notice.trim_end().to_string()
                    } else {
                        format!("{notice}{original}")
                    });
                }

                self.apply_post_tool_use_hooks(&task, &tool_name, &tool_id, &mut tool_result)
                    .await;

                self.state_manager
                    .update_state(
                        &tool_id,
                        ToolExecutionState::Completed {
                            result: convert_to_framework_result(&tool_result),
                            duration_ms,
                            queue_wait_ms: Some(queue_wait_ms),
                            preflight_ms: Some(preflight_ms),
                            confirmation_wait_ms: Some(confirmation_wait_ms),
                            execution_ms: Some(execution_ms),
                        },
                    )
                    .await;

                info!(
                    "Tool completed: tool_name={}, duration_ms={}, queue_wait_ms={}, preflight_ms={}, confirmation_wait_ms={}, execution_ms={}, streaming={}",
                    tool_name,
                    duration_ms,
                    queue_wait_ms,
                    preflight_ms,
                    confirmation_wait_ms,
                    execution_ms,
                    is_streaming
                );

                Ok(ToolExecutionResult {
                    tool_id,
                    tool_name: wire_tool_name,
                    effective_tool_name: tool_name,
                    result: tool_result,
                    execution_time_ms: duration_ms,
                })
            }
            Err(e) => {
                // Cancellation is a first-class terminal state, not a failure.
                // Preserve Cancelled here so a late cancel cannot be overwritten
                // by the generic Failed branch below.
                if let BitFunError::Cancelled(reason) = &e {
                    self.state_manager
                        .update_state(
                            &tool_id,
                            ToolExecutionState::Cancelled {
                                reason: reason.clone(),
                                duration_ms: Some(elapsed_ms_u64(start_time)),
                                queue_wait_ms: Some(queue_wait_ms),
                                preflight_ms: Some(preflight_ms),
                                confirmation_wait_ms: Some(confirmation_wait_ms),
                                execution_ms: Some(execution_ms),
                            },
                        )
                        .await;

                    info!(
                        "Tool cancelled during execution: tool_name={}, reason={}, duration_ms={}, queue_wait_ms={}, preflight_ms={}, confirmation_wait_ms={}, execution_ms={}",
                        tool_name,
                        reason,
                        elapsed_ms_u64(start_time),
                        queue_wait_ms,
                        preflight_ms,
                        confirmation_wait_ms,
                        execution_ms
                    );

                    return Err(e);
                }

                if matches!(e, BitFunError::Timeout(_)) {
                    let duration_ms = elapsed_ms_u64(start_time);
                    let presentation = build_tool_execution_timeout_presentation(
                        &tool_name,
                        task.options.timeout_secs,
                    );
                    let timed_out_tool_id = tool_id.clone();
                    let timed_out_tool_name = tool_name.clone();

                    self.state_manager
                        .update_state(
                            &tool_id,
                            ToolExecutionState::Cancelled {
                                reason: presentation.result_for_assistant.clone(),
                                duration_ms: Some(duration_ms),
                                queue_wait_ms: Some(queue_wait_ms),
                                preflight_ms: Some(preflight_ms),
                                confirmation_wait_ms: Some(confirmation_wait_ms),
                                execution_ms: Some(execution_ms),
                            },
                        )
                        .await;

                    warn!(
                        "Tool execution timed out: tool_name={}, duration_ms={}, queue_wait_ms={}, preflight_ms={}, confirmation_wait_ms={}, execution_ms={}",
                        tool_name,
                        duration_ms,
                        queue_wait_ms,
                        preflight_ms,
                        confirmation_wait_ms,
                        execution_ms
                    );

                    return Ok(ToolExecutionResult {
                        tool_id: timed_out_tool_id.clone(),
                        tool_name: wire_tool_name.clone(),
                        effective_tool_name: timed_out_tool_name.clone(),
                        result: ModelToolResult {
                            tool_id: timed_out_tool_id,
                            effective_tool_name: persisted_effective_tool_name(
                                &wire_tool_name,
                                &timed_out_tool_name,
                            ),
                            tool_name: wire_tool_name,
                            result: presentation.result_json,
                            result_for_assistant: Some(presentation.result_for_assistant),
                            is_error: false,
                            duration_ms: Some(duration_ms),
                            image_attachments: None,
                        },
                        execution_time_ms: duration_ms,
                    });
                }

                let error_msg = e.to_string();
                let is_retryable = task.options.max_retries > 0;

                self.state_manager
                    .update_state(
                        &tool_id,
                        ToolExecutionState::Failed {
                            error: error_msg.clone(),
                            is_retryable,
                            duration_ms: Some(elapsed_ms_u64(start_time)),
                            queue_wait_ms: Some(queue_wait_ms),
                            preflight_ms: Some(preflight_ms),
                            confirmation_wait_ms: Some(confirmation_wait_ms),
                            execution_ms: Some(execution_ms),
                        },
                    )
                    .await;

                error!(
                    "Tool failed: tool_name={}, error={}, duration_ms={}, queue_wait_ms={}, preflight_ms={}, confirmation_wait_ms={}, execution_ms={}",
                    tool_name,
                    error_msg,
                    elapsed_ms_u64(start_time),
                    queue_wait_ms,
                    preflight_ms,
                    confirmation_wait_ms,
                    execution_ms
                );

                Err(e)
            }
        }
    }

    /// Execute with retry
    async fn execute_with_retry(
        &self,
        task: &ToolTask,
        cancellation_token: CancellationToken,
        tool: Arc<dyn crate::agentic::tools::framework::Tool>,
    ) -> BitFunResult<ModelToolResult> {
        let mut attempts = 0;
        let max_attempts = task.options.max_retries + 1;

        loop {
            // Check cancellation token
            if cancellation_token.is_cancelled() {
                return Err(BitFunError::Cancelled(
                    "Tool execution was cancelled".to_string(),
                ));
            }

            attempts += 1;

            let result = self
                .execute_tool_impl(task, cancellation_token.clone(), tool.clone())
                .await;

            match result {
                Ok(r) => return Ok(r),
                Err(e) => {
                    if !should_retry_tool_attempt(ToolRetryAttemptFacts {
                        attempts,
                        max_attempts,
                        error_class: classify_tool_retry_error(&e),
                    }) {
                        return Err(e);
                    }

                    debug!(
                        "Retrying tool execution: attempt={}/{}, error={}",
                        attempts, max_attempts, e
                    );

                    // Wait for a period of time and retry
                    tokio::time::sleep(Duration::from_millis(retry_delay_ms(attempts))).await;
                }
            }
        }
    }

    /// Actual execution of tool
    async fn execute_tool_impl(
        &self,
        task: &ToolTask,
        cancellation_token: CancellationToken,
        tool: Arc<dyn crate::agentic::tools::framework::Tool>,
    ) -> BitFunResult<ModelToolResult> {
        // Check cancellation token
        if cancellation_token.is_cancelled() {
            return Err(BitFunError::Cancelled(
                "Tool execution was cancelled".to_string(),
            ));
        }

        let tool_context = self.build_tool_use_context(task, cancellation_token);

        let execution_future = tool.call(task.effective_arguments(), &tool_context);

        let timeout_owner = resolve_contextual_tool(
            Arc::clone(&tool),
            tool_context.workspace_root(),
            tool_context.is_remote(),
        );
        let pipeline_timeout_secs = if timeout_owner
            .as_ref()
            .is_some_and(|selected| selected.manages_own_execution_timeout())
        {
            None
        } else {
            task.options.timeout_secs
        };

        let tool_results = match pipeline_timeout_secs {
            Some(timeout_secs) => {
                let timeout_duration = Duration::from_secs(timeout_secs);
                let result = timeout(timeout_duration, execution_future)
                    .await
                    .map_err(|_| {
                        BitFunError::Timeout(format!(
                            "Tool execution timeout: {}",
                            task.effective_tool_name()
                        ))
                    })?;
                result?
            }
            None => execution_future.await?,
        };

        if tool.supports_streaming() && tool_results.len() > 1 {
            self.handle_streaming_results(task, &tool_results).await?;
        }

        tool_results
            .into_iter()
            .last()
            .map(|r| {
                convert_tool_result(
                    r,
                    &task.tool_call.tool_id,
                    &task.tool_call.tool_name,
                    task.effective_tool_name(),
                )
            })
            .ok_or_else(|| {
                BitFunError::Tool(format!(
                    "Tool did not return result: {}",
                    task.effective_tool_name()
                ))
            })
    }

    fn build_tool_use_context(
        &self,
        task: &ToolTask,
        cancellation_token: CancellationToken,
    ) -> ToolUseContext {
        tool_context_runtime::build_tool_use_context_for_task(
            task,
            self.computer_use_host.clone(),
            cancellation_token,
        )
    }

    /// Handle streaming results
    async fn handle_streaming_results(
        &self,
        task: &ToolTask,
        results: &[FrameworkToolResult],
    ) -> BitFunResult<()> {
        let mut chunks_received = 0;

        for result in results {
            if let FrameworkToolResult::StreamChunk {
                data,
                chunk_index: _,
                is_final: _,
            } = result
            {
                chunks_received += 1;

                // Update state
                self.state_manager
                    .update_state(
                        &task.tool_call.tool_id,
                        ToolExecutionState::Streaming {
                            started_at: std::time::SystemTime::now(),
                            chunks_received,
                        },
                    )
                    .await;

                // Send StreamChunk event
                let _event_data = ToolEventData::StreamChunk {
                    identity: bitfun_events::ToolEventIdentity::resolved(
                        task.tool_call.tool_id.clone(),
                        task.invocation.wire_tool_name.clone(),
                        task.effective_tool_name().to_string(),
                    ),
                    data: data.clone(),
                };
            }
        }

        Ok(())
    }

    /// Cancel tool execution
    pub async fn cancel_tool(&self, tool_id: &str, reason: String) -> BitFunResult<()> {
        let Some(task) = self.state_manager.get_task(tool_id) else {
            debug!(
                "Ignoring cancel request for unknown tool: tool_id={}",
                tool_id
            );
            return Ok(());
        };

        if tool_task_state_kind(&task.state).is_terminal() {
            debug!(
                    "Ignoring duplicate cancel request for tool in terminal state: tool_id={}, state={:?}",
                    tool_id, task.state
                );
            return Ok(());
        }

        // 1. Trigger cancellation token
        if self.cancellation_tokens.cancel(tool_id) {
            debug!("Cancellation token triggered: tool_id={}", tool_id);
        } else {
            debug!(
                "Cancellation token not found (tool may have completed): tool_id={}",
                tool_id
            );
        }

        // 2. Update state to cancelled
        self.state_manager
            .update_state(
                tool_id,
                ToolExecutionState::Cancelled {
                    reason: reason.clone(),
                    duration_ms: None,
                    queue_wait_ms: None,
                    preflight_ms: None,
                    confirmation_wait_ms: None,
                    execution_ms: None,
                },
            )
            .await;

        info!(
            "Tool execution cancelled: tool_id={}, reason={}",
            tool_id, reason
        );
        Ok(())
    }

    pub async fn reply_to_tool(&self, tool_id: &str, reply: PermissionReply) -> BitFunResult<()> {
        let manager = self.permission_request_manager.as_ref().ok_or_else(|| {
            BitFunError::service("Permission request manager is unavailable".to_string())
        })?;
        let request = manager
            .pending_requests()
            .into_iter()
            .find(|request| {
                request.tool_call_id.as_deref() == Some(tool_id) || request.request_id == tool_id
            })
            .ok_or_else(|| {
                BitFunError::NotFound(format!("Permission request not found for tool: {tool_id}"))
            })?;
        manager
            .reply(&request.request_id, reply, PermissionReplySource::User)
            .await
            .map(|_| ())
            .map_err(|error| BitFunError::service(error.to_string()))
    }

    /// Cancel all tools for a dialog turn
    pub async fn cancel_dialog_turn_tools(&self, dialog_turn_id: &str) -> BitFunResult<()> {
        info!(
            "Cancelling all tools for dialog turn: dialog_turn_id={}",
            dialog_turn_id
        );

        let tasks = self.state_manager.get_dialog_turn_tasks(dialog_turn_id);
        debug!("Found {} tool tasks for dialog turn", tasks.len());

        let summary = summarize_dialog_turn_cancellation(
            tasks.iter().map(|task| tool_task_state_kind(&task.state)),
        );

        for task in tasks {
            if should_cancel_tool_state(tool_task_state_kind(&task.state)) {
                debug!(
                    "Cancelling tool: tool_id={}, state={:?}",
                    task.tool_call.tool_id, task.state
                );
                self.cancel_tool(&task.tool_call.tool_id, "Dialog turn cancelled".to_string())
                    .await?;
            } else {
                debug!(
                    "Skipping tool (state not cancellable): tool_id={}, state={:?}",
                    task.tool_call.tool_id, task.state
                );
            }
        }

        info!(
            "Tool cancellation completed: cancelled={}, skipped={}",
            summary.cancelled, summary.skipped
        );
        Ok(())
    }

    #[cfg(test)]
    pub(crate) async fn insert_tool_task_for_test(&self, task: ToolTask) {
        self.state_manager.create_task(task).await;
    }

    #[cfg(test)]
    pub(crate) async fn session_loaded_specs_for_test(
        &self,
        session_id: &str,
    ) -> Vec<LoadedDeferredToolSpec> {
        self.cached_session_loaded_deferred_specs(session_id).await
    }

    #[cfg(test)]
    pub(crate) fn tool_task_is_cancelled_for_test(&self, tool_id: &str) -> bool {
        self.state_manager
            .get_task(tool_id)
            .is_some_and(|task| matches!(task.state, ToolExecutionState::Cancelled { .. }))
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::field_reassign_with_default)] // test fixtures build options via field assignment
    use super::*;
    use crate::agentic::core::ToolExecutionState;
    use crate::agentic::events::{EventQueue, EventQueueConfig};
    use crate::agentic::round_preempt::{
        DialogRoundInjectionInterrupt, SessionRoundInjectionBuffer,
    };
    use crate::agentic::tools::framework::{Tool, ToolResult, ValidationResult};
    use crate::agentic::tools::implementations::task::TaskTool;
    use crate::agentic::tools::tool_context_runtime::ToolUseContext;
    use crate::agentic::tools::ToolRuntimeRestrictions;
    use crate::agentic::WorkspaceBinding;
    use async_trait::async_trait;
    use bitfun_agent_tools::{
        LoadedDeferredToolSpec, CALL_DEFERRED_TOOL_NAME, USER_REJECTED_TOOL_MESSAGE,
    };
    use bitfun_runtime_ports::{
        ClockPort, PermissionAuditEvent, PermissionAuditRecord, PermissionAuditStorePort,
        PermissionConstraintLayer, PermissionEffect, PermissionGrant, PermissionGrantKey,
        PermissionGrantStorePort, PermissionReplyStorePort, PermissionRule, PortResult,
        ResolvedPermissionPolicy, RoundInjection, RoundInjectionExecutionPolicy,
        RoundInjectionKind, RoundInjectionTarget, RoundInjectionToolPreemption,
        RuntimeServiceCapability, RuntimeServicePort,
    };
    use serde_json::json;
    use std::collections::HashMap;
    #[cfg(feature = "external-sources")]
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};
    use std::time::SystemTime;
    use tokio::time::{sleep, Duration};

    fn loaded_spec(tool_name: &str, catalog_generation: u64) -> LoadedDeferredToolSpec {
        LoadedDeferredToolSpec {
            tool_name: tool_name.to_string(),
            catalog_generation,
        }
    }

    #[test]
    fn recovered_write_without_separator_is_rejected_as_potentially_truncated_path() {
        assert!(recovered_write_has_potentially_truncated_marked_path(
            "Write",
            &json!({ "payload": "+++ C:/workspace/truncated" }),
            Default::default(),
            true,
        ));
    }

    #[test]
    fn complete_path_only_write_is_not_treated_as_truncation_recovery() {
        assert!(!recovered_write_has_potentially_truncated_marked_path(
            "Write",
            &json!({ "payload": "+++ C:/workspace/empty.txt" }),
            Default::default(),
            false,
        ));
        assert!(!recovered_write_has_potentially_truncated_marked_path(
            "Write",
            &json!({ "payload": "+++ C:/workspace/empty.txt\n" }),
            Default::default(),
            true,
        ));
    }

    #[test]
    fn recovered_write_without_marker_can_fall_back_safely() {
        assert!(!recovered_write_has_potentially_truncated_marked_path(
            "Write",
            &json!({ "payload": "partial content without a path" }),
            Default::default(),
            true,
        ));
        assert!(!recovered_write_has_potentially_truncated_marked_path(
            "Write",
            &json!({ "payload": "+++ C:/workspace/main.rs\npartial content" }),
            Default::default(),
            true,
        ));
    }

    #[test]
    fn account_scoped_permission_works_without_a_workspace() {
        let mut intent = PermissionIntent::new(
            "page_publish",
            vec!["page:demo; visibility=private; deploy=saved-version-only".to_string()],
        );
        intent.display_metadata.insert(
            "permissionScope".to_string(),
            json!(ACCOUNT_PERMISSION_SCOPE),
        );
        intent
            .display_metadata
            .insert("requiresFreshApproval".to_string(), json!(true));
        let context = ToolUseContext::for_tool_listing(None, None);
        assert_eq!(
            permission_scope(&context, &[intent.clone()]).expect("account scope"),
            (
                ACCOUNT_PERMISSION_PROJECT_ID.to_string(),
                ACCOUNT_PERMISSION_PROJECT_PATH.to_string(),
            )
        );
    }

    #[test]
    fn ordinary_permission_intents_still_require_a_workspace() {
        let context = ToolUseContext::for_tool_listing(None, None);
        let intent = PermissionIntent::new("edit", vec!["src/main.rs".to_string()]);
        assert!(permission_scope(&context, &[intent]).is_err());
    }

    struct StaticTestTool {
        name: String,
        response: serde_json::Value,
        delay_ms: u64,
        readonly: bool,
    }

    struct CapturingTestTool {
        name: String,
        received_arguments: Arc<Mutex<Option<serde_json::Value>>>,
    }

    struct V2FileTestTool {
        intents: Vec<PermissionIntent>,
        call_count: Arc<AtomicUsize>,
    }

    #[async_trait]
    impl Tool for V2FileTestTool {
        fn name(&self) -> &str {
            "Write"
        }

        fn is_readonly(&self) -> bool {
            // Keep the test tool eligible for the parallel batch scheduler
            // while its explicit permission intent still exercises permission prompts.
            true
        }

        async fn description(&self) -> BitFunResult<String> {
            Ok("File permission test tool".to_string())
        }

        fn short_description(&self) -> String {
            "File permission test tool".to_string()
        }

        fn input_schema(&self) -> serde_json::Value {
            json!({ "type": "object" })
        }

        fn permission_intents(
            &self,
            _input: &serde_json::Value,
            _context: &ToolUseContext,
        ) -> BitFunResult<Vec<PermissionIntent>> {
            Ok(self.intents.clone())
        }

        async fn call_impl(
            &self,
            _input: &serde_json::Value,
            _context: &ToolUseContext,
        ) -> BitFunResult<Vec<ToolResult>> {
            self.call_count.fetch_add(1, Ordering::SeqCst);
            Ok(vec![ToolResult::Result {
                data: json!({ "written": true }),
                result_for_assistant: None,
                image_attachments: None,
            }])
        }
    }

    #[derive(Default)]
    struct MemoryPermissionStore {
        grants: Mutex<Vec<PermissionGrant>>,
        audit: Mutex<Vec<PermissionAuditRecord>>,
    }

    impl RuntimeServicePort for MemoryPermissionStore {
        fn capability(&self) -> RuntimeServiceCapability {
            RuntimeServiceCapability::Permission
        }
    }

    #[async_trait]
    impl PermissionGrantStorePort for MemoryPermissionStore {
        async fn list_project_grants(&self, project_id: &str) -> PortResult<Vec<PermissionGrant>> {
            Ok(self
                .grants
                .lock()
                .expect("permission grant lock")
                .iter()
                .filter(|grant| grant.project_id == project_id)
                .cloned()
                .collect())
        }

        async fn add_project_grants(&self, grants: Vec<PermissionGrant>) -> PortResult<()> {
            self.grants
                .lock()
                .expect("permission grant lock")
                .extend(grants);
            Ok(())
        }

        async fn remove_project_grant(&self, key: PermissionGrantKey) -> PortResult<bool> {
            let mut grants = self.grants.lock().expect("permission grant lock");
            let original_len = grants.len();
            grants.retain(|grant| grant.key() != key);
            Ok(grants.len() != original_len)
        }

        async fn clear_project_grants(&self, project_id: &str) -> PortResult<usize> {
            let mut grants = self.grants.lock().expect("permission grant lock");
            let original_len = grants.len();
            grants.retain(|grant| grant.project_id != project_id);
            Ok(original_len - grants.len())
        }
    }

    #[async_trait]
    impl PermissionAuditStorePort for MemoryPermissionStore {
        async fn append_permission_audit(&self, record: PermissionAuditRecord) -> PortResult<()> {
            self.audit
                .lock()
                .expect("permission audit lock")
                .push(record);
            Ok(())
        }

        async fn list_project_permission_audit(
            &self,
            project_id: &str,
        ) -> PortResult<Vec<PermissionAuditRecord>> {
            Ok(self
                .audit
                .lock()
                .expect("permission audit lock")
                .iter()
                .filter(|record| record.request.project_id == project_id)
                .cloned()
                .collect())
        }
    }

    #[async_trait]
    impl PermissionReplyStorePort for MemoryPermissionStore {
        async fn commit_permission_reply(
            &self,
            grants: Vec<PermissionGrant>,
            audit: Vec<PermissionAuditRecord>,
        ) -> PortResult<()> {
            self.grants
                .lock()
                .expect("permission grant lock")
                .extend(grants);
            self.audit
                .lock()
                .expect("permission audit lock")
                .extend(audit);
            Ok(())
        }
    }

    struct FixedPermissionClock;

    impl RuntimeServicePort for FixedPermissionClock {
        fn capability(&self) -> RuntimeServiceCapability {
            RuntimeServiceCapability::Clock
        }
    }

    impl ClockPort for FixedPermissionClock {
        fn now_unix_millis(&self) -> i64 {
            42
        }
    }

    #[async_trait]
    impl Tool for CapturingTestTool {
        fn name(&self) -> &str {
            &self.name
        }

        async fn description(&self) -> BitFunResult<String> {
            Ok("capturing test tool".to_string())
        }

        fn short_description(&self) -> String {
            "capturing test tool".to_string()
        }

        fn is_readonly(&self) -> bool {
            true
        }

        fn input_schema(&self) -> serde_json::Value {
            json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["city"],
                "properties": {
                    "city": { "type": "string" }
                }
            })
        }

        async fn validate_input(
            &self,
            input: &serde_json::Value,
            _context: Option<&ToolUseContext>,
        ) -> ValidationResult {
            let valid = input
                .get("city")
                .and_then(serde_json::Value::as_str)
                .is_some()
                && input.as_object().is_some_and(|object| object.len() == 1);
            ValidationResult {
                result: valid,
                message: (!valid).then(|| "city must be the only target argument".to_string()),
                error_code: (!valid).then_some(400),
                meta: None,
            }
        }

        async fn call_impl(
            &self,
            input: &serde_json::Value,
            _context: &ToolUseContext,
        ) -> BitFunResult<Vec<ToolResult>> {
            *self
                .received_arguments
                .lock()
                .expect("capturing tool argument lock") = Some(input.clone());
            Ok(vec![ToolResult::Result {
                data: json!({ "received": input }),
                result_for_assistant: None,
                image_attachments: None,
            }])
        }
    }

    #[async_trait]
    impl Tool for StaticTestTool {
        fn name(&self) -> &str {
            &self.name
        }

        async fn description(&self) -> BitFunResult<String> {
            Ok("static test tool".to_string())
        }

        fn short_description(&self) -> String {
            "static test tool".to_string()
        }

        fn is_readonly(&self) -> bool {
            self.readonly
        }

        fn input_schema(&self) -> serde_json::Value {
            json!({ "type": "object" })
        }

        async fn validate_input(
            &self,
            _input: &serde_json::Value,
            _context: Option<&ToolUseContext>,
        ) -> ValidationResult {
            ValidationResult {
                result: true,
                message: None,
                error_code: None,
                meta: None,
            }
        }

        async fn call_impl(
            &self,
            _input: &serde_json::Value,
            _context: &ToolUseContext,
        ) -> BitFunResult<Vec<ToolResult>> {
            if self.delay_ms > 0 {
                sleep(Duration::from_millis(self.delay_ms)).await;
            }
            Ok(vec![ToolResult::Result {
                data: self.response.clone(),
                result_for_assistant: Some(render_tool_result_for_assistant(
                    &self.name,
                    &self.response,
                )),
                image_attachments: None,
            }])
        }
    }

    fn test_tool_pipeline() -> ToolPipeline {
        let registry = Arc::new(TokioRwLock::new(ToolRegistry::new()));
        let event_queue = Arc::new(EventQueue::new(EventQueueConfig::default()));
        let state_manager = Arc::new(ToolStateManager::new(event_queue));
        ToolPipeline::new(registry, state_manager, None)
    }

    fn test_tool_call(tool_id: &str, tool_name: &str) -> ToolCall {
        ToolCall {
            tool_id: tool_id.to_string(),
            tool_name: tool_name.to_string(),
            arguments: json!({ "path": "src/main.rs" }),
            raw_arguments: None,
            is_error: false,
            parse_error: None,
            recovered_from_truncation: false,
            repair_kind: Default::default(),
        }
    }

    fn test_tool_execution_context() -> ToolExecutionContext {
        ToolExecutionContext {
            session_id: "session_1".to_string(),
            dialog_turn_id: "turn_1".to_string(),
            round_id: "round_1".to_string(),
            attempt_id: None,
            attempt_index: None,
            agent_type: "agent".to_string(),
            workspace: None,
            primary_model_facts: tool_runtime::context::PrimaryModelFacts::default(),
            context_vars: HashMap::new(),
            subagent_parent_info: None,
            permission_delegation: None,
            delegation_policy: bitfun_runtime_ports::DelegationPolicy::top_level(),
            deferred_tools: Vec::new(),
            loaded_deferred_tool_specs: Vec::new(),
            allowed_tools: Vec::new(),
            user_enabled_tools: Vec::new(),
            runtime_tool_restrictions: ToolRuntimeRestrictions::default(),
            steering_interrupt: None,
            workspace_services: None,
            terminal_port: None,
            remote_exec_port: None,
        }
    }

    fn test_tool_task(tool_id: &str, tool_name: &str) -> ToolTask {
        ToolTask::new(
            test_tool_call(tool_id, tool_name),
            test_tool_execution_context(),
            ToolExecutionOptions::default(),
        )
    }

    #[cfg(feature = "external-sources")]
    #[test]
    fn remote_workspace_route_root_isolated_from_same_local_path() {
        let pipeline = test_tool_pipeline();
        let root = std::env::current_dir().expect("absolute test workspace root");

        let mut local_task = test_tool_task("local-route", "Read");
        local_task.context.workspace = Some(WorkspaceBinding::new(None, root.clone()));
        let local = pipeline.build_tool_use_context(&local_task, CancellationToken::new());

        let session_identity =
            crate::service::remote_ssh::workspace_state::workspace_session_identity(
                root.to_string_lossy().as_ref(),
                Some("remote-connection"),
                Some("remote.example"),
            )
            .expect("remote workspace identity");
        let mut remote_task = test_tool_task("remote-route", "Read");
        remote_task.context.workspace = Some(WorkspaceBinding::new_remote(
            None,
            PathBuf::from(&root),
            "remote-connection".to_string(),
            "Remote".to_string(),
            session_identity,
        ));
        let remote = pipeline.build_tool_use_context(&remote_task, CancellationToken::new());

        assert_eq!(
            crate::external_tools::external_tool_route_root(
                local.workspace_root(),
                local.is_remote(),
            ),
            Some(root.as_path())
        );
        let remote_route_root = crate::external_tools::external_tool_route_root(
            remote.workspace_root(),
            remote.is_remote(),
        );
        assert_eq!(remote_route_root, Some(std::path::Path::new("\0")));
        assert!(dunce::canonicalize(remote_route_root.expect("remote sentinel")).is_err());
    }

    async fn register_static_test_tool(
        pipeline: &ToolPipeline,
        name: &str,
        response: serde_json::Value,
        delay_ms: u64,
    ) {
        pipeline
            .tool_registry
            .write()
            .await
            .register_tool(Arc::new(StaticTestTool {
                name: name.to_string(),
                response,
                delay_ms,
                readonly: true,
            }));
    }

    async fn register_capturing_test_tool(
        pipeline: &ToolPipeline,
        name: &str,
        received_arguments: Arc<Mutex<Option<serde_json::Value>>>,
    ) {
        pipeline
            .tool_registry
            .write()
            .await
            .register_tool(Arc::new(CapturingTestTool {
                name: name.to_string(),
                received_arguments,
            }));
    }

    async fn current_registry_generation(pipeline: &ToolPipeline) -> u64 {
        pipeline
            .tool_registry
            .read()
            .await
            .current_snapshot_generation()
    }

    async fn register_v2_file_test_tool(
        pipeline: &ToolPipeline,
        intents: Vec<PermissionIntent>,
        call_count: Arc<AtomicUsize>,
    ) {
        pipeline
            .tool_registry
            .write()
            .await
            .register_tool(Arc::new(V2FileTestTool {
                intents,
                call_count,
            }));
    }

    fn permission_test_context() -> ToolExecutionContext {
        let mut context = test_tool_execution_context();
        context.workspace = Some(WorkspaceBinding::new(
            None,
            std::env::temp_dir().join("bitfun-permission-test"),
        ));
        context
    }

    fn subagent_permission_test_context(parent_tool_call_id: &str) -> ToolExecutionContext {
        let mut context = permission_test_context();
        context.session_id = "subagent-session".to_string();
        context.dialog_turn_id = "subagent-turn".to_string();
        context.agent_type = "Explore".to_string();
        context.subagent_parent_info = Some(SubagentParentInfo {
            session_id: "parent-session".to_string(),
            dialog_turn_id: "parent-turn".to_string(),
            tool_call_id: parent_tool_call_id.to_string(),
            depth: None,
        });
        context
    }

    #[tokio::test]
    async fn non_readonly_tools_use_v2_custom_tool_fallback() {
        let pipeline = test_tool_pipeline();
        pipeline
            .tool_registry
            .write()
            .await
            .register_tool(Arc::new(StaticTestTool {
                name: "UnclassifiedMutation".to_string(),
                response: json!({ "unexpected": true }),
                delay_ms: 0,
                readonly: false,
            }));
        let mut options = ToolExecutionOptions::default();
        options.permission_policy = ResolvedPermissionPolicy::new(
            vec![PermissionRule::new(
                "custom_tool",
                "UnclassifiedMutation",
                PermissionEffect::Deny,
            )],
            Vec::new(),
        );

        let results = pipeline
            .execute_tools(
                vec![test_tool_call("fallback-deny", "UnclassifiedMutation")],
                permission_test_context(),
                options,
            )
            .await
            .expect("fallback policy denial");

        assert!(matches!(
            pipeline
                .state_manager
                .get_task("fallback-deny")
                .map(|task| task.state),
            Some(ToolExecutionState::Rejected { .. })
        ));
        assert_eq!(results[0].result.result["category"], "permission_denied");
        assert!(results[0]
            .result
            .result_for_assistant
            .as_deref()
            .is_some_and(|message| message.contains("current permission policy")));
    }

    #[tokio::test]
    async fn runtime_operation_class_restriction_rejects_tool_in_pipeline() {
        let pipeline = test_tool_pipeline();
        register_static_test_tool(&pipeline, "Bash", json!({ "ok": true }), 0).await;

        // Read-only operation class is allowed; Bash resolves to ExecuteCode by
        // default, so the operation-level gate must reject it inside the
        // pipeline before any tool side effect can run.
        let mut context = test_tool_execution_context();
        let mut restrictions = ToolRuntimeRestrictions::default();
        restrictions
            .allowed_operation_classes
            .insert(bitfun_agent_tools::OperationClass::ReadOnly);
        context.runtime_tool_restrictions = restrictions;

        let results = pipeline
            .execute_tools(
                vec![test_tool_call("op-gate", "Bash")],
                context,
                ToolExecutionOptions::default(),
            )
            .await
            .expect("operation-class denial surfaces as a tool result");

        assert!(matches!(
            pipeline
                .state_manager
                .get_task("op-gate")
                .map(|task| task.state),
            Some(ToolExecutionState::Failed { .. })
        ));
        assert!(results[0]
            .result
            .result_for_assistant
            .as_deref()
            .is_some_and(|message| message.contains("not allowed by runtime restrictions")));
    }

    fn permission_test_manager(store: Arc<MemoryPermissionStore>) -> Arc<PermissionRequestManager> {
        Arc::new(
            PermissionRequestManager::new(
                store.clone(),
                store.clone(),
                Arc::new(FixedPermissionClock),
            )
            .with_grant_store(store),
        )
    }

    async fn wait_for_permission_request(
        manager: &PermissionRequestManager,
    ) -> bitfun_runtime_ports::PermissionRequest {
        for _ in 0..100 {
            if let Some(request) = manager.pending_requests().into_iter().next() {
                return request;
            }
            sleep(Duration::from_millis(5)).await;
        }
        panic!("permission request was not registered");
    }

    async fn wait_for_permission_request_count(
        manager: &PermissionRequestManager,
        expected: usize,
    ) -> Vec<bitfun_runtime_ports::PermissionRequest> {
        for _ in 0..100 {
            let requests = manager.pending_requests();
            if requests.len() >= expected {
                return requests;
            }
            sleep(Duration::from_millis(5)).await;
        }
        panic!("expected {expected} permission requests to be registered");
    }

    #[tokio::test]
    async fn v2_allow_and_deny_are_enforced_before_tool_side_effects() {
        let pipeline = test_tool_pipeline();
        let calls = Arc::new(AtomicUsize::new(0));
        register_v2_file_test_tool(
            &pipeline,
            vec![PermissionIntent::new(
                "edit",
                vec!["src/main.rs".to_string(), "src/private/key.rs".to_string()],
            )],
            Arc::clone(&calls),
        )
        .await;

        let mut allow_options = ToolExecutionOptions::default();
        allow_options.permission_policy = ResolvedPermissionPolicy::new(
            vec![PermissionRule::new(
                "edit",
                "src/*",
                PermissionEffect::Allow,
            )],
            Vec::new(),
        );
        let results = pipeline
            .execute_tools(
                vec![test_tool_call("allow", "Write")],
                permission_test_context(),
                allow_options,
            )
            .await
            .expect("allowed tool should execute");
        assert!(!results[0].result.is_error);
        assert_eq!(calls.load(Ordering::SeqCst), 1);

        let mut deny_options = ToolExecutionOptions::default();
        deny_options.auto_approve_ask = true;
        deny_options.permission_policy = ResolvedPermissionPolicy::new(
            vec![
                PermissionRule::new("edit", "src/*", PermissionEffect::Allow),
                PermissionRule::new("edit", "src/private/*", PermissionEffect::Deny),
            ],
            Vec::new(),
        );
        let results = pipeline
            .execute_tools(
                vec![test_tool_call("deny", "Write")],
                permission_test_context(),
                deny_options,
            )
            .await
            .expect("denied tool should return a structured rejection");
        assert!(!results[0].result.is_error);
        assert!(matches!(
            pipeline
                .state_manager
                .get_task("deny")
                .map(|task| task.state),
            Some(ToolExecutionState::Rejected { .. })
        ));
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert_eq!(results[0].result.result["category"], "permission_denied");
    }

    #[tokio::test]
    async fn independent_permission_constraints_tighten_but_never_widen_host_policy() {
        let pipeline = test_tool_pipeline();
        let calls = Arc::new(AtomicUsize::new(0));
        register_v2_file_test_tool(
            &pipeline,
            vec![PermissionIntent::new(
                "edit",
                vec!["src/generated/output.rs".to_string()],
            )],
            Arc::clone(&calls),
        )
        .await;

        let mut host_deny = ToolExecutionOptions::default();
        host_deny.auto_approve_ask = true;
        host_deny.permission_policy = ResolvedPermissionPolicy::new(
            vec![PermissionRule::new(
                "edit",
                "src/generated/*",
                PermissionEffect::Deny,
            )],
            vec![PermissionConstraintLayer::new(vec![PermissionRule::new(
                "edit",
                "*",
                PermissionEffect::Allow,
            )])],
        );
        pipeline
            .execute_tools(
                vec![test_tool_call("host-deny", "Write")],
                permission_test_context(),
                host_deny,
            )
            .await
            .expect("constraint allow must not widen host denial");

        let mut external_deny = ToolExecutionOptions::default();
        external_deny.auto_approve_ask = true;
        external_deny.permission_policy = ResolvedPermissionPolicy::new(
            vec![PermissionRule::new("edit", "*", PermissionEffect::Allow)],
            vec![PermissionConstraintLayer::new(vec![PermissionRule::new(
                "edit",
                "src/generated/*",
                PermissionEffect::Deny,
            )])],
        );
        pipeline
            .execute_tools(
                vec![test_tool_call("external-deny", "Write")],
                permission_test_context(),
                external_deny,
            )
            .await
            .expect("constraint denial should tighten host allow");

        assert_eq!(calls.load(Ordering::SeqCst), 0);
        for tool_id in ["host-deny", "external-deny"] {
            assert!(matches!(
                pipeline
                    .state_manager
                    .get_task(tool_id)
                    .map(|task| task.state),
                Some(ToolExecutionState::Rejected { .. })
            ));
        }
    }

    /// A PreToolUse hook approval waives the interactive permission prompt.
    /// It must never widen the policy: a rule that denies the call still
    /// rejects it, and the tool never runs.
    #[tokio::test]
    async fn hook_approval_does_not_override_a_permission_deny_rule() {
        let pipeline = test_tool_pipeline();
        let calls = Arc::new(AtomicUsize::new(0));
        register_v2_file_test_tool(
            &pipeline,
            vec![PermissionIntent::new(
                "edit",
                vec!["src/private/key.rs".to_string()],
            )],
            Arc::clone(&calls),
        )
        .await;

        // Stand in for a hook that returned permissionDecision: "allow".
        pipeline
            .hook_preapprovals
            .lock()
            .await
            .insert("hook-approved".to_string());

        let mut deny_options = ToolExecutionOptions::default();
        deny_options.permission_policy = ResolvedPermissionPolicy::new(
            vec![PermissionRule::new(
                "edit",
                "src/private/*",
                PermissionEffect::Deny,
            )],
            Vec::new(),
        );
        let results = pipeline
            .execute_tools(
                vec![test_tool_call("hook-approved", "Write")],
                permission_test_context(),
                deny_options,
            )
            .await
            .expect("denied tool should return a structured rejection");

        assert!(matches!(
            pipeline
                .state_manager
                .get_task("hook-approved")
                .map(|task| task.state),
            Some(ToolExecutionState::Rejected { .. })
        ));
        assert_eq!(results[0].result.result["category"], "permission_denied");
        assert_eq!(
            calls.load(Ordering::SeqCst),
            0,
            "a denied tool must not execute even when a hook approved it"
        );
    }

    /// The same approval does waive an interactive prompt when the policy
    /// only asks, so the call proceeds without a permission request.
    #[tokio::test]
    async fn hook_approval_waives_the_permission_prompt() {
        let store = Arc::new(MemoryPermissionStore::default());
        let manager = permission_test_manager(Arc::clone(&store));
        let pipeline = test_tool_pipeline().with_permission_request_manager(Arc::clone(&manager));
        let calls = Arc::new(AtomicUsize::new(0));
        register_v2_file_test_tool(
            &pipeline,
            vec![PermissionIntent::new(
                "edit",
                vec!["src/main.rs".to_string()],
            )],
            Arc::clone(&calls),
        )
        .await;

        pipeline
            .hook_preapprovals
            .lock()
            .await
            .insert("hook-approved".to_string());

        // No rule matches, so the policy would ask; nobody answers the prompt
        // in this test, so completing at all proves the prompt was waived.
        let results = pipeline
            .execute_tools(
                vec![test_tool_call("hook-approved", "Write")],
                permission_test_context(),
                ToolExecutionOptions::default(),
            )
            .await
            .expect("hook-approved tool should execute");

        assert!(!results[0].result.is_error);
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn v2_rejecting_one_parallel_tool_does_not_reject_sibling() {
        let store = Arc::new(MemoryPermissionStore::default());
        let manager = permission_test_manager(Arc::clone(&store));
        let pipeline = test_tool_pipeline().with_permission_request_manager(Arc::clone(&manager));
        let calls = Arc::new(AtomicUsize::new(0));
        register_v2_file_test_tool(
            &pipeline,
            vec![PermissionIntent::new(
                "edit",
                vec!["src/main.rs".to_string()],
            )],
            Arc::clone(&calls),
        )
        .await;

        let mut permission_events = manager.subscribe();
        let running_pipeline = pipeline.clone();
        let execution = tokio::spawn(async move {
            running_pipeline
                .execute_tools(
                    vec![
                        test_tool_call("reject-me", "Write"),
                        test_tool_call("keep-going", "Write"),
                    ],
                    permission_test_context(),
                    ToolExecutionOptions::default(),
                )
                .await
        });

        let requests = wait_for_permission_request_count(&manager, 2).await;
        assert_eq!(requests.len(), 2);
        let expected_project_path = std::env::temp_dir()
            .join("bitfun-permission-test")
            .to_string_lossy()
            .to_string();
        assert_eq!(
            requests[0].project_path.as_deref(),
            Some(expected_project_path.as_str())
        );
        assert_eq!(requests[0].tool_call_id.as_deref(), Some("reject-me"));
        assert_eq!(requests[0].order, 0);
        assert_eq!(requests[1].tool_call_id.as_deref(), Some("keep-going"));
        assert_eq!(requests[1].order, 1);
        for (event, expected_request) in [
            permission_events.recv().await.expect("first asked event"),
            permission_events.recv().await.expect("second asked event"),
        ]
        .into_iter()
        .zip(requests.iter())
        {
            match event {
                bitfun_runtime_ports::PermissionRequestEvent::Asked { request } => {
                    assert_eq!(request.request_id, expected_request.request_id);
                }
                other => panic!("expected asked event, got {other:?}"),
            }
        }
        let rejected_request = requests
            .iter()
            .find(|request| request.tool_call_id.as_deref() == Some("reject-me"))
            .expect("rejected tool permission request");
        let sibling_request = requests
            .iter()
            .find(|request| request.tool_call_id.as_deref() == Some("keep-going"))
            .expect("sibling tool permission request");

        manager
            .reply(
                &rejected_request.request_id,
                PermissionReply::Reject { feedback: None },
                bitfun_runtime_ports::PermissionReplySource::User,
            )
            .await
            .expect("reject one tool");
        assert_eq!(
            manager
                .pending_requests()
                .iter()
                .map(|request| request.request_id.as_str())
                .collect::<Vec<_>>(),
            vec![sibling_request.request_id.as_str()]
        );

        manager
            .reply(
                &sibling_request.request_id,
                PermissionReply::Once,
                bitfun_runtime_ports::PermissionReplySource::User,
            )
            .await
            .expect("allow sibling tool");

        let results = execution
            .await
            .expect("parallel tool execution join")
            .expect("parallel tool execution");
        assert_eq!(results.len(), 2);
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert_eq!(results[0].result.result["category"], "user_rejected");
        assert!(results[0].result.result["instruction"].is_null());
        assert_eq!(
            results[0].result.result_for_assistant.as_deref(),
            Some(USER_REJECTED_TOOL_MESSAGE)
        );
        assert!(!results[1].result.is_error);
    }

    #[tokio::test]
    async fn v2_rejection_feedback_is_preserved_for_the_assistant() {
        let store = Arc::new(MemoryPermissionStore::default());
        let manager = permission_test_manager(Arc::clone(&store));
        let pipeline = test_tool_pipeline().with_permission_request_manager(Arc::clone(&manager));
        let calls = Arc::new(AtomicUsize::new(0));
        register_v2_file_test_tool(
            &pipeline,
            vec![PermissionIntent::new(
                "edit",
                vec!["src/main.rs".to_string()],
            )],
            Arc::clone(&calls),
        )
        .await;

        let running_pipeline = pipeline.clone();
        let execution = tokio::spawn(async move {
            running_pipeline
                .execute_tools(
                    vec![test_tool_call("reject-with-feedback", "Write")],
                    permission_test_context(),
                    ToolExecutionOptions::default(),
                )
                .await
        });

        let request = wait_for_permission_request(&manager).await;
        manager
            .reply(
                &request.request_id,
                PermissionReply::Reject {
                    feedback: Some("Use a read-only path".to_string()),
                },
                bitfun_runtime_ports::PermissionReplySource::User,
            )
            .await
            .expect("reject request with feedback");

        let results = execution
            .await
            .expect("feedback rejection task join")
            .expect("feedback rejection should return a structured result");
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        assert_eq!(results[0].result.result["category"], "user_rejected");
        assert_eq!(
            results[0].result.result["instruction"],
            "Use a read-only path"
        );
        assert_eq!(
            results[0].result.result_for_assistant.as_deref(),
            Some(
                "The user rejected this tool call with the following instruction: \"Use a read-only path\". Do not retry it unless the user explicitly asks you to. If you cannot complete the task without running this tool call, stop and ask the user how to proceed."
            )
        );
    }

    #[tokio::test]
    async fn v2_subagent_request_projects_exact_parent_task_context() {
        let store = Arc::new(MemoryPermissionStore::default());
        let manager = permission_test_manager(Arc::clone(&store));
        let pipeline = test_tool_pipeline().with_permission_request_manager(Arc::clone(&manager));
        let calls = Arc::new(AtomicUsize::new(0));
        register_v2_file_test_tool(
            &pipeline,
            vec![PermissionIntent::new(
                "edit",
                vec!["src/main.rs".to_string()],
            )],
            Arc::clone(&calls),
        )
        .await;

        let running_pipeline = pipeline.clone();
        let execution = tokio::spawn(async move {
            running_pipeline
                .execute_tools(
                    vec![test_tool_call("child-write", "Write")],
                    subagent_permission_test_context("parent-task-call"),
                    ToolExecutionOptions::default(),
                )
                .await
        });

        let request = wait_for_permission_request(&manager).await;
        assert_eq!(request.session_id, "subagent-session");
        assert_eq!(request.tool_call_id.as_deref(), Some("child-write"));
        let delegation = request
            .delegation
            .as_ref()
            .expect("subagent request should project delegation context");
        assert_eq!(delegation.parent_session_id, "parent-session");
        assert_eq!(
            delegation.parent_dialog_turn_id.as_deref(),
            Some("parent-turn")
        );
        assert_eq!(delegation.parent_tool_call_id, "parent-task-call");
        assert_eq!(delegation.subagent_type, "Explore");

        manager
            .reply(
                &request.request_id,
                PermissionReply::Once,
                bitfun_runtime_ports::PermissionReplySource::User,
            )
            .await
            .expect("allow child request");
        execution
            .await
            .expect("child task join")
            .expect("child execution");
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn v2_request_routes_partial_persisted_subagent_delegation() {
        let store = Arc::new(MemoryPermissionStore::default());
        let manager = permission_test_manager(Arc::clone(&store));
        let pipeline = test_tool_pipeline().with_permission_request_manager(Arc::clone(&manager));
        let calls = Arc::new(AtomicUsize::new(0));
        register_v2_file_test_tool(
            &pipeline,
            vec![PermissionIntent::new(
                "edit",
                vec!["src/main.rs".to_string()],
            )],
            Arc::clone(&calls),
        )
        .await;

        let mut context = permission_test_context();
        context.session_id = "subagent-session".to_string();
        context.agent_type = "Explore".to_string();
        context.permission_delegation = Some(bitfun_runtime_ports::PermissionDelegationContext {
            parent_session_id: "parent-session".to_string(),
            parent_dialog_turn_id: None,
            parent_tool_call_id: "parent-task-call".to_string(),
            subagent_type: "Explore".to_string(),
        });

        let running_pipeline = pipeline.clone();
        let execution = tokio::spawn(async move {
            running_pipeline
                .execute_tools(
                    vec![test_tool_call("child-write", "Write")],
                    context,
                    ToolExecutionOptions::default(),
                )
                .await
        });

        let request = wait_for_permission_request(&manager).await;
        let delegation = request
            .delegation
            .as_ref()
            .expect("partial subagent lineage should route permission requests");
        assert_eq!(delegation.parent_session_id, "parent-session");
        assert_eq!(delegation.parent_dialog_turn_id, None);
        assert_eq!(delegation.parent_tool_call_id, "parent-task-call");

        manager
            .reply(
                &request.request_id,
                PermissionReply::Once,
                bitfun_runtime_ports::PermissionReplySource::User,
            )
            .await
            .expect("allow child request");
        execution
            .await
            .expect("child task join")
            .expect("child execution");
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn once_and_always_replies_control_execution_and_remembered_grants() {
        let store = Arc::new(MemoryPermissionStore::default());
        let manager = permission_test_manager(Arc::clone(&store));
        let pipeline = test_tool_pipeline().with_permission_request_manager(Arc::clone(&manager));
        let calls = Arc::new(AtomicUsize::new(0));
        register_v2_file_test_tool(
            &pipeline,
            vec![PermissionIntent::new(
                "edit",
                vec!["src/main.rs".to_string(), "src/private/key.rs".to_string()],
            )],
            Arc::clone(&calls),
        )
        .await;

        let once_pipeline = pipeline.clone();
        let once = tokio::spawn(async move {
            once_pipeline
                .execute_tools(
                    vec![test_tool_call("once", "Write")],
                    permission_test_context(),
                    ToolExecutionOptions::default(),
                )
                .await
        });
        let request = wait_for_permission_request(&manager).await;
        assert_eq!(request.tool_call_id.as_deref(), Some("once"));
        assert!(request.delegation.is_none());
        manager
            .reply(
                &request.request_id,
                PermissionReply::Once,
                bitfun_runtime_ports::PermissionReplySource::User,
            )
            .await
            .expect("once reply");
        once.await.expect("once task join").expect("once execution");
        assert_eq!(calls.load(Ordering::SeqCst), 1);

        let always_pipeline = pipeline.clone();
        let always = tokio::spawn(async move {
            always_pipeline
                .execute_tools(
                    vec![test_tool_call("always", "Write")],
                    permission_test_context(),
                    ToolExecutionOptions::default(),
                )
                .await
        });
        let request = wait_for_permission_request(&manager).await;
        manager
            .reply(
                &request.request_id,
                PermissionReply::Always,
                bitfun_runtime_ports::PermissionReplySource::User,
            )
            .await
            .expect("always reply");
        always
            .await
            .expect("always task join")
            .expect("always execution");
        assert_eq!(calls.load(Ordering::SeqCst), 2);

        pipeline
            .execute_tools(
                vec![test_tool_call("remembered", "Write")],
                permission_test_context(),
                ToolExecutionOptions::default(),
            )
            .await
            .expect("remembered grant should allow the same project");
        assert_eq!(calls.load(Ordering::SeqCst), 3);

        assert_eq!(
            store.audit.lock().expect("permission audit lock").len(),
            4,
            "once and always should each persist requested and replied audit facts"
        );

        let mut other_project_context = permission_test_context();
        other_project_context.workspace = Some(WorkspaceBinding::new(
            None,
            std::env::temp_dir().join("bitfun-permission-other-project"),
        ));
        let other_pipeline = pipeline.clone();
        let other_project = tokio::spawn(async move {
            other_pipeline
                .execute_tools(
                    vec![test_tool_call("other-project", "Write")],
                    other_project_context,
                    ToolExecutionOptions::default(),
                )
                .await
        });
        let other_request = wait_for_permission_request(&manager).await;
        let remembered_project_id = store
            .grants
            .lock()
            .expect("permission grant lock")
            .first()
            .expect("remembered grant")
            .project_id
            .clone();
        assert_ne!(other_request.project_id, remembered_project_id);
        manager
            .reply(
                &other_request.request_id,
                PermissionReply::Reject { feedback: None },
                bitfun_runtime_ports::PermissionReplySource::User,
            )
            .await
            .expect("reject other project request");
        other_project
            .await
            .expect("other project task join")
            .expect("other project rejection");
        assert_eq!(calls.load(Ordering::SeqCst), 3);

        let mut remote_context = permission_test_context();
        let local_root = remote_context
            .workspace
            .as_ref()
            .expect("local permission workspace")
            .root_path()
            .to_path_buf();
        let remote_identity =
            crate::service::remote_ssh::workspace_state::workspace_session_identity(
                local_root.to_string_lossy().as_ref(),
                Some("permission-remote-connection"),
                Some("remote.example"),
            )
            .expect("remote permission identity");
        remote_context.workspace = Some(WorkspaceBinding::new_remote(
            None,
            local_root,
            "permission-remote-connection".to_string(),
            "Remote permission test".to_string(),
            remote_identity,
        ));
        let remote_pipeline = pipeline.clone();
        let remote_execution = tokio::spawn(async move {
            remote_pipeline
                .execute_tools(
                    vec![test_tool_call("remote-project", "Write")],
                    remote_context,
                    ToolExecutionOptions::default(),
                )
                .await
        });
        let remote_request = wait_for_permission_request(&manager).await;
        assert_ne!(remote_request.project_id, remembered_project_id);
        assert!(remote_request.project_id.starts_with("remote_"));
        manager
            .reply(
                &remote_request.request_id,
                PermissionReply::Reject { feedback: None },
                bitfun_runtime_ports::PermissionReplySource::User,
            )
            .await
            .expect("reject remote project request");
        remote_execution
            .await
            .expect("remote project task join")
            .expect("remote project rejection");
        assert_eq!(calls.load(Ordering::SeqCst), 3);

        let mut deny_options = ToolExecutionOptions::default();
        deny_options.permission_policy = ResolvedPermissionPolicy::new(
            vec![
                PermissionRule::new("edit", "src/*", PermissionEffect::Allow),
                PermissionRule::new("edit", "src/private/*", PermissionEffect::Deny),
            ],
            Vec::new(),
        );
        pipeline
            .execute_tools(
                vec![test_tool_call("deny-after-grant", "Write")],
                permission_test_context(),
                deny_options,
            )
            .await
            .expect("policy denial should be structured");
        assert_eq!(calls.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn v2_auto_approve_subagent_ask_preserves_lineage_without_interactive_event() {
        let store = Arc::new(MemoryPermissionStore::default());
        let manager = permission_test_manager(Arc::clone(&store));
        let mut events = manager.subscribe();
        let pipeline = test_tool_pipeline().with_permission_request_manager(Arc::clone(&manager));
        let calls = Arc::new(AtomicUsize::new(0));
        register_v2_file_test_tool(
            &pipeline,
            vec![PermissionIntent::new(
                "edit",
                vec!["src/main.rs".to_string()],
            )],
            Arc::clone(&calls),
        )
        .await;

        let mut options = ToolExecutionOptions::default();
        options.auto_approve_ask = true;
        pipeline
            .execute_tools(
                vec![test_tool_call("auto", "Write")],
                subagent_permission_test_context("background-task-call"),
                options,
            )
            .await
            .expect("auto-approved tool should execute");

        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert!(store
            .grants
            .lock()
            .expect("permission grant lock")
            .is_empty());
        let audit = store.audit.lock().expect("permission audit lock");
        assert_eq!(audit.len(), 2);
        assert!(audit.iter().all(|record| {
            record
                .request
                .delegation
                .as_ref()
                .is_some_and(|delegation| {
                    delegation.parent_tool_call_id == "background-task-call"
                        && delegation.subagent_type == "Explore"
                })
        }));
        assert!(matches!(audit[0].event, PermissionAuditEvent::Requested));
        assert!(matches!(
            audit[1].event,
            PermissionAuditEvent::Replied {
                reply: PermissionReply::Once,
                source: bitfun_runtime_ports::PermissionReplySource::AutoApprove,
            }
        ));
        assert!(matches!(
            events.try_recv(),
            Err(tokio::sync::broadcast::error::TryRecvError::Empty)
        ));
        assert!(manager.pending_requests().is_empty());
    }

    #[tokio::test]
    async fn v2_cancellation_clears_pending_request_without_side_effect() {
        let store = Arc::new(MemoryPermissionStore::default());
        let manager = permission_test_manager(Arc::clone(&store));
        let pipeline = test_tool_pipeline().with_permission_request_manager(Arc::clone(&manager));
        let calls = Arc::new(AtomicUsize::new(0));
        register_v2_file_test_tool(
            &pipeline,
            vec![PermissionIntent::new(
                "edit",
                vec!["src/main.rs".to_string()],
            )],
            Arc::clone(&calls),
        )
        .await;

        let running_pipeline = pipeline.clone();
        let task = tokio::spawn(async move {
            running_pipeline
                .execute_tools(
                    vec![test_tool_call("cancel", "Write")],
                    subagent_permission_test_context("cancelled-parent-task"),
                    ToolExecutionOptions::default(),
                )
                .await
        });
        let request = wait_for_permission_request(&manager).await;
        assert_eq!(
            request
                .delegation
                .as_ref()
                .map(|delegation| delegation.parent_tool_call_id.as_str()),
            Some("cancelled-parent-task")
        );
        pipeline
            .cancel_tool("cancel", "test cancellation".to_string())
            .await
            .expect("cancel tool");
        task.await
            .expect("cancel task join")
            .expect("cancel result");
        assert!(manager.pending_requests().is_empty());
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        assert!(store
            .audit
            .lock()
            .expect("permission audit lock")
            .iter()
            .any(|record| matches!(record.event, PermissionAuditEvent::Cancelled { .. })));
    }

    #[tokio::test]
    async fn deferred_gateway_normalizes_arguments_and_executes_effective_target() {
        let pipeline = test_tool_pipeline();
        let received_arguments = Arc::new(Mutex::new(None));
        register_capturing_test_tool(&pipeline, "get_weather", Arc::clone(&received_arguments))
            .await;

        let mut context = test_tool_execution_context();
        context.allowed_tools = vec![
            CALL_DEFERRED_TOOL_NAME.to_string(),
            "get_weather".to_string(),
        ];
        context.deferred_tools = vec!["get_weather".to_string()];
        context.loaded_deferred_tool_specs = vec![loaded_spec(
            "get_weather",
            current_registry_generation(&pipeline).await,
        )];

        let mut call = test_tool_call("deferred_1", CALL_DEFERRED_TOOL_NAME);
        call.arguments = json!({
            "tool_name": "get_weather",
            "args": { "city": "Shanghai" },
            "city": "Beijing"
        });

        let results = pipeline
            .execute_tools(vec![call], context, ToolExecutionOptions::default())
            .await
            .expect("deferred tool execution");

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].tool_name, CALL_DEFERRED_TOOL_NAME);
        assert_eq!(results[0].effective_tool_name, "get_weather");
        assert_eq!(results[0].result.tool_name, CALL_DEFERRED_TOOL_NAME);
        assert_eq!(results[0].result.result["received"]["city"], "Shanghai");
        assert_eq!(
            *received_arguments
                .lock()
                .expect("capturing tool argument lock"),
            Some(json!({ "city": "Shanghai" }))
        );

        let task = pipeline
            .state_manager
            .get_task("deferred_1")
            .expect("deferred tool task");
        assert_eq!(task.tool_call.tool_name, CALL_DEFERRED_TOOL_NAME);
        assert_eq!(task.effective_tool_name(), "get_weather");
        assert_eq!(task.effective_arguments(), &json!({ "city": "Shanghai" }));
        assert_eq!(
            task.invocation.wire_arguments,
            json!({
                "tool_name": "get_weather",
                "args": {
                    "city": "Shanghai"
                }
            })
        );
    }

    #[tokio::test]
    async fn deferred_gateway_rejects_registry_refresh_before_execution() {
        let pipeline = test_tool_pipeline();
        let old_received_arguments = Arc::new(Mutex::new(None));
        register_capturing_test_tool(
            &pipeline,
            "get_weather",
            Arc::clone(&old_received_arguments),
        )
        .await;
        let loaded_generation = current_registry_generation(&pipeline).await;

        let new_received_arguments = Arc::new(Mutex::new(None));
        register_capturing_test_tool(
            &pipeline,
            "get_weather",
            Arc::clone(&new_received_arguments),
        )
        .await;

        let mut context = test_tool_execution_context();
        context.allowed_tools = vec![
            CALL_DEFERRED_TOOL_NAME.to_string(),
            "get_weather".to_string(),
        ];
        context.deferred_tools = vec!["get_weather".to_string()];
        context.loaded_deferred_tool_specs = vec![loaded_spec("get_weather", loaded_generation)];

        let mut call = test_tool_call("deferred_stale", CALL_DEFERRED_TOOL_NAME);
        call.arguments = json!({
            "tool_name": "get_weather",
            "args": { "city": "Shanghai" }
        });

        let results = pipeline
            .execute_tools(vec![call], context, ToolExecutionOptions::default())
            .await
            .expect("stale deferred call should become a per-tool error result");

        assert_eq!(results.len(), 1);
        assert!(results[0].result.is_error);
        assert_eq!(
            results[0].result.effective_tool_name.as_deref(),
            Some("get_weather")
        );
        assert!(results[0]
            .result
            .result_for_assistant
            .as_deref()
            .unwrap_or_default()
            .contains("is stale"));
        assert_eq!(
            *old_received_arguments
                .lock()
                .expect("old capturing tool argument lock"),
            None
        );
        assert_eq!(
            *new_received_arguments
                .lock()
                .expect("new capturing tool argument lock"),
            None
        );
    }

    #[tokio::test]
    async fn deferred_gateway_requires_loaded_get_tool_spec_result() {
        let pipeline = test_tool_pipeline();
        register_capturing_test_tool(&pipeline, "get_weather", Arc::new(Mutex::new(None))).await;

        let mut context = test_tool_execution_context();
        context.allowed_tools = vec![
            CALL_DEFERRED_TOOL_NAME.to_string(),
            "get_weather".to_string(),
        ];
        context.deferred_tools = vec!["get_weather".to_string()];

        let mut call = test_tool_call("deferred_locked", CALL_DEFERRED_TOOL_NAME);
        call.arguments = json!({
            "tool_name": "get_weather",
            "args": { "city": "Shanghai" }
        });

        let results = pipeline
            .execute_tools(vec![call], context, ToolExecutionOptions::default())
            .await
            .expect("pipeline should return a per-tool error result");

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].tool_name, CALL_DEFERRED_TOOL_NAME);
        assert_eq!(results[0].effective_tool_name, "get_weather");
        assert!(results[0].result.is_error);
        assert!(results[0]
            .result
            .result_for_assistant
            .as_deref()
            .unwrap_or_default()
            .contains("Call GetToolSpec first"));
    }

    #[tokio::test]
    async fn deferred_gateway_does_not_dispatch_direct_tools() {
        let pipeline = test_tool_pipeline();
        let received_arguments = Arc::new(Mutex::new(None));
        register_capturing_test_tool(&pipeline, "get_weather", Arc::clone(&received_arguments))
            .await;

        let mut context = test_tool_execution_context();
        context.allowed_tools = vec![
            CALL_DEFERRED_TOOL_NAME.to_string(),
            "get_weather".to_string(),
        ];

        let mut call = test_tool_call("deferred_direct", CALL_DEFERRED_TOOL_NAME);
        call.arguments = json!({
            "tool_name": "get_weather",
            "args": { "city": "Shanghai" }
        });

        let results = pipeline
            .execute_tools(vec![call], context, ToolExecutionOptions::default())
            .await
            .expect("pipeline should return a per-tool error result");

        assert_eq!(results.len(), 1);
        assert!(results[0].result.is_error);
        assert!(results[0]
            .result
            .result_for_assistant
            .as_deref()
            .unwrap_or_default()
            .contains("not an available deferred tool"));
        assert_eq!(
            *received_arguments
                .lock()
                .expect("capturing tool argument lock"),
            None
        );
    }

    fn test_round_injection(
        kind: RoundInjectionKind,
        tool_preemption: RoundInjectionToolPreemption,
    ) -> RoundInjection {
        RoundInjection {
            id: format!("injection-{:?}-{:?}", kind, tool_preemption),
            kind,
            execution_policy: RoundInjectionExecutionPolicy::new(tool_preemption),
            target: RoundInjectionTarget::CurrentRunningTurn,
            content: "test injection".to_string(),
            display_content: "test injection".to_string(),
            attachments: Vec::new(),
            metadata: serde_json::Map::new(),
            created_at: SystemTime::now(),
            prepended_reminders: Vec::new(),
        }
    }

    fn assert_failed_task_contains(pipeline: &ToolPipeline, tool_id: &str, expected: &str) {
        let task = pipeline
            .state_manager
            .get_task(tool_id)
            .unwrap_or_else(|| panic!("{tool_id} task should be retained"));
        match task.state {
            ToolExecutionState::Failed { error, .. } => assert!(
                error.contains(expected),
                "failed task error should contain '{expected}', got '{error}'"
            ),
            state => panic!("expected failed task state, got {state:?}"),
        }
    }

    #[test]
    fn steering_interrupted_result_preserves_tool_call_identity() {
        let task = test_tool_task("tool_1", "Read");
        let result = build_user_steering_interrupted_result("tool_1", Some(task));

        assert_eq!(result.tool_id, "tool_1");
        assert_eq!(result.tool_name, "Read");
        // Skipped-by-steering must not surface as a tool failure: the tool
        // never ran, and `is_error: true` would make the model retry / detour
        // around a fake error (see build_user_steering_interrupted_result).
        assert!(!result.result.is_error);
        assert_eq!(
            result.result.result["category"],
            serde_json::Value::String("user_steering_interrupted".to_string())
        );
        assert_eq!(
            result.result.result["status"],
            serde_json::Value::String("skipped".to_string())
        );
        assert_eq!(
            result.result.result_for_assistant.as_deref(),
            Some(USER_STEERING_INTERRUPTED_MESSAGE)
        );
    }

    #[test]
    fn error_result_preserves_full_raw_arguments_for_unparseable_calls() {
        let mut task = test_tool_task("tool_1", "Git");
        task.tool_call.arguments = json!({});
        task.tool_call.is_error = true;
        let raw_arguments = format!("{{\"operation\":\"{}", "log".repeat(512));
        task.tool_call.raw_arguments = Some(raw_arguments.clone());

        let result = build_error_execution_result(
            "tool_1",
            Some(task),
            &BitFunError::Validation("Arguments are invalid JSON.".to_string()),
        );

        assert_eq!(
            result.result.result["provided_arguments"],
            serde_json::Value::String(raw_arguments.clone())
        );
        assert!(result
            .result
            .result_for_assistant
            .as_deref()
            .unwrap_or_default()
            .ends_with(&raw_arguments));
        assert!(!result
            .result
            .result_for_assistant
            .as_deref()
            .unwrap_or_default()
            .contains("[truncated"));
    }

    #[test]
    fn error_result_omits_arguments_for_parsed_validation_errors() {
        let mut task = test_tool_task("tool_1", "Git");
        task.tool_call.raw_arguments = Some(r#"{\"operation\":\"log\"}"#.to_string());

        let result = build_error_execution_result(
            "tool_1",
            Some(task),
            &BitFunError::Validation("operation is not supported".to_string()),
        );

        assert!(result.result.result["provided_arguments"].is_null());
        assert_eq!(
            result.result.result_for_assistant.as_deref(),
            Some("Tool 'Git' failed (invalid_arguments): Validation error: operation is not supported")
        );
    }

    #[tokio::test]
    async fn pipeline_admission_allowed_list_rejection_updates_failed_state_before_registry_lookup()
    {
        let pipeline = test_tool_pipeline();
        let mut context = test_tool_execution_context();
        context.allowed_tools = vec!["Read".to_string()];

        let results = pipeline
            .execute_tools(
                vec![test_tool_call("tool_1", "UnregisteredBlockedTool")],
                context,
                ToolExecutionOptions::default(),
            )
            .await
            .expect("admission rejection should be returned as an error result");

        assert_eq!(results.len(), 1);
        assert!(results[0].result.is_error);
        assert_failed_task_contains(
            &pipeline,
            "tool_1",
            "Tool 'UnregisteredBlockedTool' is not in the allowed list",
        );
        assert!(
            results[0]
                .result
                .result_for_assistant
                .as_deref()
                .unwrap_or_default()
                .contains("UnregisteredBlockedTool"),
            "error result should preserve rejected tool identity"
        );
    }

    #[tokio::test]
    async fn pipeline_admission_runtime_restriction_rejection_updates_failed_state() {
        let pipeline = test_tool_pipeline();
        let mut context = test_tool_execution_context();
        context
            .runtime_tool_restrictions
            .denied_tool_names
            .insert("Read".to_string());

        let results = pipeline
            .execute_tools(
                vec![test_tool_call("tool_1", "Read")],
                context,
                ToolExecutionOptions::default(),
            )
            .await
            .expect("admission rejection should be returned as an error result");

        assert_eq!(results.len(), 1);
        assert!(results[0].result.is_error);
        assert_failed_task_contains(
            &pipeline,
            "tool_1",
            "Tool 'Read' is denied by runtime restrictions",
        );
    }

    #[tokio::test]
    async fn pipeline_admission_direct_deferred_tool_requires_get_tool_spec() {
        let pipeline = test_tool_pipeline();
        let mut context = test_tool_execution_context();
        context.deferred_tools = vec!["WebFetch".to_string()];

        let results = pipeline
            .execute_tools(
                vec![test_tool_call("tool_1", "WebFetch")],
                context,
                ToolExecutionOptions::default(),
            )
            .await
            .expect("admission rejection should be returned as an error result");

        assert_eq!(results.len(), 1);
        assert!(results[0].result.is_error);
        assert_failed_task_contains(
            &pipeline,
            "tool_1",
            "Call GetToolSpec first with {\"tool_name\":\"WebFetch\"}",
        );
    }

    #[tokio::test]
    async fn pipeline_admission_allows_direct_deferred_tool_after_get_tool_spec() {
        let pipeline = test_tool_pipeline();
        let received_arguments = Arc::new(Mutex::new(None));
        register_capturing_test_tool(&pipeline, "get_weather", Arc::clone(&received_arguments))
            .await;

        let mut context = test_tool_execution_context();
        context.allowed_tools = vec!["get_weather".to_string()];
        context.deferred_tools = vec!["get_weather".to_string()];
        context.loaded_deferred_tool_specs = vec![loaded_spec(
            "get_weather",
            current_registry_generation(&pipeline).await,
        )];

        let mut call = test_tool_call("direct_deferred", "get_weather");
        call.arguments = json!({ "city": "Shanghai" });

        let results = pipeline
            .execute_tools(vec![call], context, ToolExecutionOptions::default())
            .await
            .expect("loaded deferred tool should support direct invocation");

        assert_eq!(results.len(), 1);
        assert!(!results[0].result.is_error);
        assert_eq!(
            received_arguments
                .lock()
                .expect("captured arguments lock")
                .as_ref(),
            Some(&json!({ "city": "Shanghai" }))
        );
    }

    #[tokio::test]
    async fn background_result_pending_does_not_skip_tool_execution() {
        let pipeline = test_tool_pipeline();
        register_static_test_tool(&pipeline, "Read", json!({ "ok": true }), 0).await;

        let buffer = Arc::new(SessionRoundInjectionBuffer::default());
        buffer.push(
            "session_1",
            test_round_injection(
                RoundInjectionKind::BackgroundResult,
                RoundInjectionKind::BackgroundResult
                    .default_execution_policy()
                    .tool_preemption,
            ),
        );

        let mut context = test_tool_execution_context();
        context.steering_interrupt = Some(DialogRoundInjectionInterrupt::new(
            "session_1".to_string(),
            "turn_1".to_string(),
            buffer,
        ));

        let results = pipeline
            .execute_tools(
                vec![test_tool_call("tool_1", "Read")],
                context,
                ToolExecutionOptions::default(),
            )
            .await
            .expect("background result should not skip tool execution");

        assert_eq!(results.len(), 1);
        assert!(!results[0].result.is_error);
        assert_eq!(results[0].result.result["ok"], json!(true));
    }

    #[tokio::test]
    async fn user_steering_pending_still_skips_remaining_tool_plan() {
        let pipeline = test_tool_pipeline();
        let buffer = Arc::new(SessionRoundInjectionBuffer::default());
        buffer.push(
            "session_1",
            test_round_injection(
                RoundInjectionKind::UserSteering,
                RoundInjectionKind::UserSteering
                    .default_execution_policy()
                    .tool_preemption,
            ),
        );

        let mut context = test_tool_execution_context();
        context.steering_interrupt = Some(DialogRoundInjectionInterrupt::new(
            "session_1".to_string(),
            "turn_1".to_string(),
            buffer,
        ));

        let results = pipeline
            .execute_tools(
                vec![
                    test_tool_call("tool_1", "Read"),
                    test_tool_call("tool_2", "Write"),
                ],
                context,
                ToolExecutionOptions::default(),
            )
            .await
            .expect("user steering skip should be surfaced as tool results");

        assert_eq!(results.len(), 2);
        assert_eq!(
            results[0].result.result["category"],
            json!("user_steering_interrupted")
        );
        assert_eq!(
            results[1].result.result["category"],
            json!("user_steering_interrupted")
        );
        // Skipped tools must not surface as failures (no retry / detour bait).
        assert!(!results[0].result.is_error);
        assert!(!results[1].result.is_error);
    }

    #[tokio::test]
    async fn custom_round_injection_can_cancel_running_tool_cooperatively() {
        let pipeline = test_tool_pipeline();
        register_static_test_tool(&pipeline, "Read", json!({ "ok": true }), 30_000).await;

        let buffer = Arc::new(SessionRoundInjectionBuffer::default());
        let buffer_for_injection = buffer.clone();
        tokio::spawn(async move {
            sleep(Duration::from_millis(50)).await;
            buffer_for_injection.push(
                "session_1",
                test_round_injection(
                    RoundInjectionKind::UserSteering,
                    RoundInjectionToolPreemption::CancelRunningCooperatively,
                ),
            );
        });

        let mut context = test_tool_execution_context();
        context.steering_interrupt = Some(DialogRoundInjectionInterrupt::new(
            "session_1".to_string(),
            "turn_1".to_string(),
            buffer,
        ));
        let options = ToolExecutionOptions {
            allow_parallel: false,
            ..Default::default()
        };

        let results = pipeline
            .execute_tools(vec![test_tool_call("tool_1", "Read")], context, options)
            .await
            .expect("cooperative cancel should still return a tool result");

        assert_eq!(results.len(), 1);
        assert!(results[0].result.is_error);
        assert_eq!(results[0].result.result["category"], json!("cancelled"));
    }

    #[tokio::test]
    async fn write_like_tool_in_flight_defers_round_injection_cancel_until_complete() {
        let pipeline = test_tool_pipeline();
        // Use a long-running write tool to simulate an in-flight atomic unit.
        register_static_test_tool(&pipeline, "Write", json!({ "ok": true }), 500).await;

        let buffer = Arc::new(SessionRoundInjectionBuffer::default());
        let buffer_for_injection = buffer.clone();
        tokio::spawn(async move {
            sleep(Duration::from_millis(50)).await;
            buffer_for_injection.push(
                "session_1",
                test_round_injection(
                    RoundInjectionKind::UserSteering,
                    RoundInjectionToolPreemption::CancelRunningCooperatively,
                ),
            );
        });

        let mut context = test_tool_execution_context();
        context.steering_interrupt = Some(DialogRoundInjectionInterrupt::new(
            "session_1".to_string(),
            "turn_1".to_string(),
            buffer,
        ));
        let options = ToolExecutionOptions {
            allow_parallel: false,
            ..Default::default()
        };

        let results = pipeline
            .execute_tools(vec![test_tool_call("tool_1", "Write")], context, options)
            .await
            .expect("write tool should complete despite cooperative cancel");

        // The write-like atomic unit must complete fully (no forced cancel /
        // no half-written file).
        assert_eq!(results.len(), 1);
        assert!(!results[0].result.is_error);
        assert_eq!(results[0].result.result["ok"], json!(true));
        assert_ne!(results[0].result.result["category"], json!("cancelled"));
    }

    #[tokio::test]
    async fn read_like_tool_in_flight_is_cancelled_immediately_by_round_injection() {
        let pipeline = test_tool_pipeline();
        // Long-running read tool: injection should cancel it immediately
        // (it is not protected by the write guard).
        register_static_test_tool(&pipeline, "Read", json!({ "ok": true }), 30_000).await;

        let buffer = Arc::new(SessionRoundInjectionBuffer::default());
        let buffer_for_injection = buffer.clone();
        tokio::spawn(async move {
            sleep(Duration::from_millis(50)).await;
            buffer_for_injection.push(
                "session_1",
                test_round_injection(
                    RoundInjectionKind::UserSteering,
                    RoundInjectionToolPreemption::CancelRunningCooperatively,
                ),
            );
        });

        let mut context = test_tool_execution_context();
        context.steering_interrupt = Some(DialogRoundInjectionInterrupt::new(
            "session_1".to_string(),
            "turn_1".to_string(),
            buffer,
        ));
        let options = ToolExecutionOptions {
            allow_parallel: false,
            ..Default::default()
        };

        let results = pipeline
            .execute_tools(vec![test_tool_call("tool_1", "Read")], context, options)
            .await
            .expect("read tool cancellation should surface as a tool result");

        // Read-like tools are not protected by the write guard: the
        // injection takes effect immediately (cancelled).
        assert_eq!(results.len(), 1);
        assert!(results[0].result.is_error);
        assert_eq!(results[0].result.result["category"], json!("cancelled"));
    }

    #[tokio::test]
    async fn write_like_tool_in_flight_defers_forceful_cancel_until_complete() {
        // P2: CancelRunningForcefully variant — the write guard defers the
        // forceful cancel until the atomic unit completes too.
        let pipeline = test_tool_pipeline();
        register_static_test_tool(&pipeline, "Write", json!({ "ok": true }), 500).await;

        let buffer = Arc::new(SessionRoundInjectionBuffer::default());
        let buffer_for_injection = buffer.clone();
        tokio::spawn(async move {
            sleep(Duration::from_millis(50)).await;
            buffer_for_injection.push(
                "session_1",
                test_round_injection(
                    RoundInjectionKind::UserSteering,
                    RoundInjectionToolPreemption::CancelRunningForcefully,
                ),
            );
        });

        let mut context = test_tool_execution_context();
        context.steering_interrupt = Some(DialogRoundInjectionInterrupt::new(
            "session_1".to_string(),
            "turn_1".to_string(),
            buffer,
        ));
        let options = ToolExecutionOptions {
            allow_parallel: false,
            ..Default::default()
        };

        let results = pipeline
            .execute_tools(vec![test_tool_call("tool_1", "Write")], context, options)
            .await
            .expect("write tool should complete despite forceful cancel");

        // The write-like atomic unit must still complete fully (no forced
        // cancel / no half-written file) under the forceful preemption.
        assert_eq!(results.len(), 1);
        assert!(!results[0].result.is_error);
        assert_eq!(results[0].result.result["ok"], json!(true));
        assert_ne!(results[0].result.result["category"], json!("cancelled"));
    }

    #[test]
    fn fallback_assistant_text_preserves_full_structured_result() {
        let result = convert_tool_result(
            FrameworkToolResult::Result {
                data: json!({
                    "success": false,
                    "exit_code": 1,
                    "working_directory": "/private/tmp",
                    "output": "ERR_PNPM_NO_PKG_MANIFEST"
                }),
                result_for_assistant: None,
                image_attachments: None,
            },
            "tool_1",
            "Bash",
            "Bash",
        );

        let assistant_text = result.result_for_assistant.unwrap_or_default();
        assert!(assistant_text.contains("\"success\": false"));
        assert!(assistant_text.contains("\"exit_code\": 1"));
        assert!(assistant_text.contains("\"working_directory\": \"/private/tmp\""));
        assert!(!assistant_text.contains("completed with error"));
    }

    #[test]
    fn normal_json_repair_notice_for_interactive_tools_does_not_claim_file_write() {
        let notice = build_normal_tool_json_repair_notice("AskUserQuestion");

        assert!(notice.contains("AskUserQuestion call contained malformed JSON"));
        assert!(notice.contains("fresh complete AskUserQuestion call"));
        assert!(!notice.contains("file was written"));
        assert!(!notice.contains("max_tokens"));
    }

    #[test]
    fn write_tail_closure_notice_keeps_write_continuation_guidance() {
        let notice = build_write_tail_closure_notice("Write");

        assert!(notice.contains("file may have been written with partial content"));
        assert!(notice.contains("latest Read result"));
        assert!(notice.contains("use Edit to add only the missing continuation"));
        assert!(!notice.contains("max_tokens"));
    }

    #[test]
    fn pipeline_preserves_core_owned_tool_context_without_portable_runtime_leak() {
        let pipeline = test_tool_pipeline();
        let mut task = test_tool_task("tool_context_1", "WebFetch");
        task.context
            .context_vars
            .insert("turn_index".to_string(), "7".to_string());
        task.context
            .context_vars
            .insert("acp_transport".to_string(), "true".to_string());
        task.context.deferred_tools = vec!["WebFetch".to_string()];
        task.context.loaded_deferred_tool_specs = vec![loaded_spec("WebFetch", 0)];
        task.context.runtime_tool_restrictions = ToolRuntimeRestrictions {
            allowed_tool_names: ["WebFetch"].into_iter().map(str::to_string).collect(),
            denied_tool_names: ["Bash"].into_iter().map(str::to_string).collect(),
            denied_tool_messages: Default::default(),
            path_policy: Default::default(),
            allowed_operation_classes: Default::default(),
            denied_operation_classes: Default::default(),
        };

        let context = pipeline.build_tool_use_context(&task, CancellationToken::new());

        assert_eq!(context.tool_call_id.as_deref(), Some("tool_context_1"));
        assert_eq!(context.agent_type.as_deref(), Some("agent"));
        assert_eq!(context.session_id.as_deref(), Some("session_1"));
        assert_eq!(context.dialog_turn_id.as_deref(), Some("turn_1"));
        assert_eq!(
            context.loaded_deferred_tool_specs,
            vec![loaded_spec("WebFetch", 0)]
        );
        assert!(context.cancellation_token().is_some());
        assert!(context
            .runtime_tool_restrictions
            .is_tool_allowed("WebFetch"));
        assert!(!context.runtime_tool_restrictions.is_tool_allowed("Bash"));
        assert_eq!(context.custom_data["turn_index"], json!(7));
        assert!(!context.custom_data.contains_key("primary_model_provider"));
        assert!(!context
            .custom_data
            .contains_key("primary_model_supports_image_understanding"));
        assert_eq!(context.custom_data["acp_transport"], json!(true));

        let facts = context.to_tool_context_facts();
        let value = serde_json::to_value(&facts).expect("serialize context facts");
        assert_eq!(value["toolCallId"], "tool_context_1");
        assert_eq!(value["sessionId"], "session_1");
        assert!(value.get("unlockedCollapsedTools").is_none());
        assert!(value.get("customData").is_none());
        assert!(value.get("cancellationToken").is_none());
        assert!(value.get("workspaceServices").is_none());
    }

    #[test]
    fn deferred_tool_requires_loaded_catalog_spec() {
        let mut task = test_tool_task("tool_1", "WebFetch");
        task.context.deferred_tools = vec!["WebFetch".to_string()];

        let err = validate_tool_execution_admission(ToolExecutionAdmissionRequest {
            tool_name: &task.tool_call.tool_name,
            allowed_tools: &task.context.allowed_tools,
            runtime_tool_restrictions: &task.context.runtime_tool_restrictions,
            user_enabled_tools: &task.context.user_enabled_tools,
            tool_arguments: &task.tool_call.arguments,
            deferred_tools: &task.context.deferred_tools,
            loaded_deferred_tool_specs: &task.context.loaded_deferred_tool_specs,
            current_catalog_generation: 0,
            get_tool_spec_tool_name: GET_TOOL_SPEC_TOOL_NAME,
        })
        .expect_err("deferred tool should require a loaded GetToolSpec result");

        assert!(err
            .to_string()
            .contains("Call GetToolSpec first with {\"tool_name\":\"WebFetch\"}"));
    }

    #[test]
    fn tool_catalog_rejects_reloading_already_loaded_tool() {
        let mut task = test_tool_task("tool_1", "GetToolSpec");
        task.tool_call.arguments = json!({ "tool_name": "WebFetch" });
        task.context.loaded_deferred_tool_specs = vec![loaded_spec("WebFetch", 0)];

        let result = validate_tool_execution_admission(ToolExecutionAdmissionRequest {
            tool_name: &task.tool_call.tool_name,
            allowed_tools: &task.context.allowed_tools,
            runtime_tool_restrictions: &task.context.runtime_tool_restrictions,
            user_enabled_tools: &task.context.user_enabled_tools,
            tool_arguments: &task.tool_call.arguments,
            deferred_tools: &task.context.deferred_tools,
            loaded_deferred_tool_specs: &task.context.loaded_deferred_tool_specs,
            current_catalog_generation: 0,
            get_tool_spec_tool_name: GET_TOOL_SPEC_TOOL_NAME,
        });

        assert!(
            result.is_ok(),
            "GetToolSpec duplicate-load validation moved into GetToolSpec itself"
        );
    }

    #[test]
    fn task_tool_manages_its_own_execution_timeout() {
        let task_tool = TaskTool::new();
        assert!(task_tool.manages_own_execution_timeout());
    }

    fn test_pipeline_with_global_registry() -> ToolPipeline {
        let registry = crate::agentic::tools::registry::get_global_tool_registry();
        let event_queue = Arc::new(EventQueue::new(EventQueueConfig::default()));
        let state_manager = Arc::new(ToolStateManager::new(event_queue));
        ToolPipeline::new(registry, state_manager, None)
    }

    fn test_deferred_list_models_invocation() -> ResolvedToolInvocation {
        ResolvedToolInvocation::from_wire_call(
            CALL_DEFERRED_TOOL_NAME,
            json!({
                "tool_name": "ListModels",
                "args": {},
            }),
        )
        .expect("valid deferred ListModels invocation")
    }

    fn test_deferred_list_models_task(tool_id: &str, stale_generation: u64) -> ToolTask {
        let mut context = test_tool_execution_context();
        context.agent_type = "agentic".to_string();
        context.deferred_tools = vec!["ListModels".to_string()];
        context.loaded_deferred_tool_specs = vec![loaded_spec("ListModels", stale_generation)];
        ToolTask::new_resolved(
            ToolCall {
                tool_id: tool_id.to_string(),
                tool_name: CALL_DEFERRED_TOOL_NAME.to_string(),
                arguments: json!({
                    "tool_name": "ListModels",
                    "args": {},
                }),
                raw_arguments: None,
                is_error: false,
                parse_error: None,
                recovered_from_truncation: false,
                repair_kind: Default::default(),
            },
            test_deferred_list_models_invocation(),
            None,
            context,
            ToolExecutionOptions::default(),
        )
    }

    #[test]
    fn merge_loaded_deferred_tool_specs_upserts_by_tool_name() {
        let existing = vec![loaded_spec("WebFetch", 41), loaded_spec("Git", 42)];
        let fresh = vec![loaded_spec("WebFetch", 42)];

        let merged = merge_loaded_deferred_tool_specs(&existing, &fresh);

        assert_eq!(
            merged,
            vec![loaded_spec("Git", 42), loaded_spec("WebFetch", 42)]
        );
    }

    #[tokio::test]
    async fn stale_deferred_spec_auto_reloads_and_continues_execution() {
        let pipeline = test_pipeline_with_global_registry();
        let current_generation = {
            let registry = crate::agentic::tools::registry::get_global_tool_registry();
            let guard = registry.read().await;
            assert!(
                guard.get_tool("ListModels").is_some(),
                "F2 test requires the product-full global registry with ListModels"
            );
            guard.current_snapshot_generation()
        };

        let tool_id = "f2-stale-reload";
        let task = test_deferred_list_models_task(tool_id, current_generation.saturating_sub(1));
        pipeline.insert_tool_task_for_test(task).await;

        let result = tokio::time::timeout(
            Duration::from_secs(20),
            pipeline.execute_single_tool(tool_id.to_string()),
        )
        .await
        .expect("stale auto-reload path must not hang");

        // The admission gate must auto-reload the stale spec and let the call
        // through; whatever happens afterwards is execution-layer behavior.
        // In this test environment ListModels fails to load model config, so
        // the observable contract is: no stale-spec / GetToolSpec admission
        // error may surface.
        match result {
            Ok(execution_result) => {
                assert_eq!(execution_result.effective_tool_name, "ListModels");
            }
            Err(error) => {
                let message = error.to_string();
                assert!(
                    !message.contains("stale"),
                    "stale spec must be auto-reloaded before admission, got: {message}"
                );
                assert!(
                    !message.contains("Call GetToolSpec first"),
                    "auto-reloaded admission must not fall back to RequiresGetToolSpec, got: {message}"
                );
            }
        }
    }

    #[tokio::test]
    async fn reload_stale_deferred_tool_spec_observes_fresh_generation() {
        let pipeline = test_pipeline_with_global_registry();
        let current_generation = {
            let registry = crate::agentic::tools::registry::get_global_tool_registry();
            let guard = registry.read().await;
            assert!(
                guard.get_tool("ListModels").is_some(),
                "F2 test requires the product-full global registry with ListModels"
            );
            guard.current_snapshot_generation()
        };

        let task =
            test_deferred_list_models_task("f2-reload-unit", current_generation.saturating_sub(1));
        let outcome = pipeline
            .reload_stale_deferred_tool_spec(&task, "ListModels")
            .await;
        let StaleSpecReloadOutcome::Reloaded(updated) = outcome else {
            panic!("reload must observe a fresh spec");
        };
        let refreshed = updated
            .iter()
            .find(|spec| spec.tool_name == "ListModels")
            .expect("refreshed spec must contain ListModels");
        assert_eq!(
            refreshed.catalog_generation,
            crate::agentic::tools::registry::get_global_tool_registry()
                .read()
                .await
                .current_snapshot_generation(),
            "reloaded spec generation must match the current catalog generation"
        );
    }

    #[tokio::test]
    async fn stale_reload_records_session_cache_for_later_rounds() {
        // F2 round 1: the stale task triggers the auto-reload and the
        // refreshed spec must land in the session-scoped cache so a later
        // round does not re-trigger the recovery.
        let pipeline = test_pipeline_with_global_registry();
        let current_generation = {
            let registry = crate::agentic::tools::registry::get_global_tool_registry();
            let guard = registry.read().await;
            assert!(
                guard.get_tool("ListModels").is_some(),
                "F2 test requires the product-full global registry with ListModels"
            );
            guard.current_snapshot_generation()
        };
        let stale_generation = current_generation.saturating_sub(1);

        let tool_id = "f2-cache-round-1";
        let task = test_deferred_list_models_task(tool_id, stale_generation);
        pipeline.insert_tool_task_for_test(task).await;
        let result = tokio::time::timeout(
            Duration::from_secs(20),
            pipeline.execute_single_tool(tool_id.to_string()),
        )
        .await
        .expect("round-1 stale auto-reload path must not hang");
        match &result {
            Ok(execution_result) => assert_eq!(execution_result.effective_tool_name, "ListModels"),
            Err(error) => {
                let message = error.to_string();
                assert!(
                    !message.contains("stale"),
                    "round-1 must auto-reload before admission, got: {message}"
                );
                assert!(
                    !message.contains("Call GetToolSpec first"),
                    "round-1 must not fall back to RequiresGetToolSpec, got: {message}"
                );
            }
        }

        let cached = pipeline.session_loaded_specs_for_test("session_1").await;
        let cached_list_models = cached
            .iter()
            .find(|spec| spec.tool_name == "ListModels")
            .expect("the auto-reloaded spec must be cached for the session");
        assert_eq!(
            cached_list_models.catalog_generation, current_generation,
            "the cached spec must carry the refreshed catalog generation"
        );
    }

    #[tokio::test]
    async fn second_round_rebuilds_loaded_specs_from_cache_without_recovery() {
        // F2 round 2: the next round rebuilds loaded specs from the message
        // history, which still carries only the stale generation (the
        // synthesized GetToolSpec result never becomes part of the
        // conversation). execute_tools must merge the session cache at its
        // entry so the invocation passes admission directly — no recovery
        // action and no reload.
        let pipeline = test_pipeline_with_global_registry();
        let current_generation = {
            let registry = crate::agentic::tools::registry::get_global_tool_registry();
            let guard = registry.read().await;
            assert!(
                guard.get_tool("ListModels").is_some(),
                "F2 test requires the product-full global registry with ListModels"
            );
            guard.current_snapshot_generation()
        };
        let stale_generation = current_generation.saturating_sub(1);

        // Seed the session cache exactly like round 1's auto-reload would.
        pipeline
            .record_session_loaded_deferred_specs(
                "session_1",
                &[loaded_spec("ListModels", current_generation)],
            )
            .await;

        let mut context = test_tool_execution_context();
        context.agent_type = "agentic".to_string();
        context.deferred_tools = vec!["ListModels".to_string()];
        context.loaded_deferred_tool_specs = vec![loaded_spec("ListModels", stale_generation)];
        let results = tokio::time::timeout(
            Duration::from_secs(20),
            pipeline.execute_tools(
                vec![ToolCall {
                    tool_id: "f2-cache-round-2".to_string(),
                    tool_name: CALL_DEFERRED_TOOL_NAME.to_string(),
                    arguments: json!({
                        "tool_name": "ListModels",
                        "args": {},
                    }),
                    raw_arguments: None,
                    is_error: false,
                    parse_error: None,
                    recovered_from_truncation: false,
                    repair_kind: Default::default(),
                }],
                context,
                ToolExecutionOptions::default(),
            ),
        )
        .await
        .expect("round-2 execute_tools must not hang")
        .expect("round-2 execute_tools must not fail at the pipeline level");

        // The created task must observe the merged fresh generation from the
        // start — an auto-reload only mutates the local clone inside
        // execute_single_tool, so a cache miss here would leave the stored
        // task stale and prove the round still needed recovery.
        let task = pipeline
            .state_manager
            .get_task("f2-cache-round-2")
            .expect("round-2 task must exist");
        let task_loaded = task
            .context
            .loaded_deferred_tool_specs
            .iter()
            .find(|spec| spec.tool_name == "ListModels")
            .expect("round-2 task must carry the ListModels loaded spec");
        assert_eq!(
            task_loaded.catalog_generation, current_generation,
            "round-2 task must see the cached generation merged over the rebuilt stale one"
        );

        // No stale-spec admission error may surface to the model.
        let execution = results
            .first()
            .expect("round-2 must produce one execution result");
        let visible = execution
            .result
            .result_for_assistant
            .as_deref()
            .unwrap_or_default();
        assert!(
            !visible.contains("stale"),
            "round-2 must pass admission without a stale-spec error, got: {visible}"
        );
    }

    struct RefreshProbeTool(String);

    #[async_trait]
    impl Tool for RefreshProbeTool {
        fn name(&self) -> &str {
            &self.0
        }

        async fn description(&self) -> BitFunResult<String> {
            Ok(format!("Refresh probe {}", self.0))
        }

        fn short_description(&self) -> String {
            format!("Refresh probe {}", self.0)
        }

        fn input_schema(&self) -> serde_json::Value {
            json!({ "type": "object" })
        }

        async fn call_impl(
            &self,
            _input: &serde_json::Value,
            _context: &ToolUseContext,
        ) -> BitFunResult<Vec<ToolResult>> {
            Ok(vec![ToolResult::Result {
                data: json!({ "ok": true }),
                result_for_assistant: Some("refresh probe executed".to_string()),
                image_attachments: None,
            }])
        }
    }

    #[tokio::test]
    async fn stale_reload_retries_when_registry_generation_advances_during_reload() {
        // F2 loop retry: a registry refresh racing the reload bumps the
        // catalog generation again after the first reload observed it; the
        // loop must reload again instead of surfacing the stale-spec
        // rejection.
        let pipeline = test_pipeline_with_global_registry();
        let registry = crate::agentic::tools::registry::get_global_tool_registry();
        let stale_generation = {
            let guard = registry.read().await;
            assert!(
                guard.get_tool("ListModels").is_some(),
                "F2 test requires the product-full global registry with ListModels"
            );
            guard.current_snapshot_generation()
        };

        let tool_id = "f2-retry-loop";
        let task = test_deferred_list_models_task(tool_id, stale_generation.saturating_sub(1));
        pipeline.insert_tool_task_for_test(task).await;

        let pipeline_runner = pipeline.clone();
        let handle = tokio::spawn(async move {
            pipeline_runner
                .execute_single_tool(tool_id.to_string())
                .await
        });

        // Wait until the first reload has landed in the session cache, then
        // advance the catalog generation twice (registering probe tools) to
        // simulate a refresh racing the reload. The first reload observes the
        // generation the test read above, so the poll is satisfied by any
        // entry at or above that baseline.
        let first_reloaded = async {
            loop {
                let cached = pipeline.session_loaded_specs_for_test("session_1").await;
                if cached.iter().any(|spec| {
                    spec.tool_name == "ListModels" && spec.catalog_generation >= stale_generation
                }) {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(2)).await;
            }
        };
        tokio::time::timeout(Duration::from_secs(10), first_reloaded)
            .await
            .expect("the first reload must land in the session cache");
        for probe_index in 0..2 {
            registry
                .write()
                .await
                .register_tool(Arc::new(RefreshProbeTool(format!(
                    "F2RefreshProbe{probe_index}"
                ))));
        }

        let result = tokio::time::timeout(Duration::from_secs(20), handle)
            .await
            .expect("stale reload retry loop must not hang")
            .expect("tool execution join must not fail");

        // Cleanup: remove the probe tools so other tests keep a stable catalog.
        for probe_index in 0..2 {
            registry
                .write()
                .await
                .unregister_tool(&format!("F2RefreshProbe{probe_index}"));
        }

        match result {
            Ok(execution_result) => {
                assert_eq!(execution_result.effective_tool_name, "ListModels");
            }
            Err(error) => {
                let message = error.to_string();
                assert!(
                    !message.contains("stale"),
                    "registry refresh racing the reload must be absorbed by the retry loop, got: {message}"
                );
                assert!(
                    !message.contains("Call GetToolSpec first"),
                    "the retry loop must not fall back to RequiresGetToolSpec, got: {message}"
                );
            }
        }
    }

    #[tokio::test]
    async fn stale_spec_reload_reports_not_reloadable_when_tool_leaves_deferred_catalog() {
        // F2 failure classification: the tool is tracked as loaded by the task
        // but no longer part of the deferred catalog. The reload cannot
        // observe a fresh spec and must be classified as not reloadable with a
        // semantic reason; the original stale-spec rejection stays visible.
        let pipeline = test_pipeline_with_global_registry();
        let current_generation = {
            let registry = crate::agentic::tools::registry::get_global_tool_registry();
            let guard = registry.read().await;
            assert!(
                guard.get_tool("ListModels").is_some(),
                "F2 test requires the product-full global registry with ListModels"
            );
            guard.current_snapshot_generation()
        };

        let mut context = test_tool_execution_context();
        context.agent_type = "agentic".to_string();
        context.deferred_tools = vec!["MissingDeferredTool".to_string()];
        context.loaded_deferred_tool_specs = vec![loaded_spec(
            "MissingDeferredTool",
            current_generation.saturating_sub(1),
        )];
        let invocation = ResolvedToolInvocation::from_wire_call(
            CALL_DEFERRED_TOOL_NAME,
            json!({
                "tool_name": "MissingDeferredTool",
                "args": {},
            }),
        )
        .expect("valid deferred MissingDeferredTool invocation");
        let task = ToolTask::new_resolved(
            ToolCall {
                tool_id: "f2-not-reloadable".to_string(),
                tool_name: CALL_DEFERRED_TOOL_NAME.to_string(),
                arguments: json!({
                    "tool_name": "MissingDeferredTool",
                    "args": {},
                }),
                raw_arguments: None,
                is_error: false,
                parse_error: None,
                recovered_from_truncation: false,
                repair_kind: Default::default(),
            },
            invocation,
            None,
            context,
            ToolExecutionOptions::default(),
        );

        let outcome = pipeline
            .reload_stale_deferred_tool_spec(&task, "MissingDeferredTool")
            .await;
        let StaleSpecReloadOutcome::NotReloadable(reason) = outcome else {
            panic!("a tool outside the deferred catalog must be classified as not reloadable");
        };
        assert!(
            reason.contains("not in the deferred catalog"),
            "unexpected not-reloadable reason: {reason}"
        );

        // End-to-end: the admission rejection keeps its original stale-spec
        // semantics instead of being silently swallowed.
        pipeline.insert_tool_task_for_test(task).await;
        let err = pipeline
            .execute_single_tool("f2-not-reloadable".to_string())
            .await
            .expect_err("the stale-spec rejection must be preserved");
        let message = err.to_string();
        assert!(
            message.contains("stale"),
            "original stale-spec rejection must surface, got: {message}"
        );
    }

    #[tokio::test]
    async fn missing_deferred_spec_still_requires_explicit_get_tool_spec() {
        let pipeline = test_pipeline_with_global_registry();
        let mut task = test_deferred_list_models_task("f2-require-spec", 0);
        task.context.loaded_deferred_tool_specs = Vec::new();
        pipeline.insert_tool_task_for_test(task).await;

        let result = pipeline
            .execute_single_tool("f2-require-spec".to_string())
            .await;
        let err = result.expect_err("unloaded deferred tools must still require GetToolSpec");
        let message = err.to_string();
        assert!(
            message.contains("Call GetToolSpec first"),
            "unexpected error: {message}"
        );
    }

    #[tokio::test]
    async fn direct_deferred_invocation_still_requires_gateway() {
        let pipeline = test_pipeline_with_global_registry();
        let mut context = test_tool_execution_context();
        context.agent_type = "agentic".to_string();
        context.deferred_tools = vec!["ListModels".to_string()];
        let task = ToolTask::new(
            ToolCall {
                tool_id: "f2-direct-gateway".to_string(),
                tool_name: "ListModels".to_string(),
                arguments: json!({}),
                raw_arguments: None,
                is_error: false,
                parse_error: None,
                recovered_from_truncation: false,
                repair_kind: Default::default(),
            },
            context,
            ToolExecutionOptions::default(),
        );
        pipeline.insert_tool_task_for_test(task).await;

        let result = pipeline
            .execute_single_tool("f2-direct-gateway".to_string())
            .await;
        let err = result.expect_err("direct deferred invocation must be rejected");
        let message = err.to_string();
        assert!(
            message.contains("Call GetToolSpec first"),
            "unexpected error: {message}"
        );
    }

    // ---- R-MR-11 读取/搜索重复拦截测试 ----

    /// 构造一个 Read 工具调用（同文件不同 offset = 同目标指纹）。
    fn repeated_read_call(tool_id: &str, file_path: &str, offset: u64) -> ToolCall {
        ToolCall {
            tool_id: tool_id.to_string(),
            tool_name: "Read".to_string(),
            arguments: json!({ "file_path": file_path, "offset": offset, "limit": 10 }),
            raw_arguments: None,
            is_error: false,
            parse_error: None,
            recovered_from_truncation: false,
            repair_kind: Default::default(),
        }
    }

    fn repeated_read_task(tool_id: &str, file_path: &str, offset: u64) -> ToolTask {
        ToolTask::new(
            repeated_read_call(tool_id, file_path, offset),
            test_tool_execution_context(),
            ToolExecutionOptions::default(),
        )
    }

    #[tokio::test]
    async fn repeated_read_same_file_three_offsets_blocks_third() {
        // type-contract §四.1：连续 3 次读同文件（不同 offset 分段）→ 第 3 次拦截。
        let pipeline = test_tool_pipeline();
        register_static_test_tool(&pipeline, "Read", json!({ "ok": true }), 0).await;
        let file_path = std::env::temp_dir()
            .join("r-mr-11-same-file.txt")
            .to_string_lossy()
            .to_string();
        std::fs::write(&file_path, "line1\nline2\nline3\n").expect("write test file");

        for (index, offset) in [0u64, 10u64, 20u64].into_iter().enumerate() {
            let tool_id = format!("same-file-{index}");
            let task = repeated_read_task(&tool_id, &file_path, offset);
            pipeline.insert_tool_task_for_test(task).await;
            let result = pipeline
                .execute_single_tool(tool_id.clone())
                .await
                .expect("execute single tool must not fail at pipeline level");

            if index < 2 {
                // 前 2 次：正常执行（工具返回 ok）。
                assert_eq!(
                    result.result.result["ok"],
                    json!(true),
                    "call {index} must execute normally"
                );
            } else {
                // 第 3 次：拦截，返回提示，零请求
                assert_eq!(
                    result.result.result["category"],
                    json!("repeated_read_blocked"),
                    "third consecutive same-file read must be blocked"
                );
                assert_eq!(result.result.result["status"], json!("skipped"));
                let message = result
                    .result
                    .result_for_assistant
                    .as_deref()
                    .expect("block message must be present");
                assert!(message.contains("已连续调用 3 次"));
                assert!(message.contains("检测到碎片化读取"));
                assert!(!result.result.is_error, "block is not an execution failure");
            }
        }
        std::fs::remove_file(&file_path).ok();
    }

    #[tokio::test]
    async fn repeated_grep_same_keyword_blocks_third() {
        // type-contract §四.2：连续 3 次 grep 同关键词 → 第 3 次拦截。
        let mut state = RepeatedReadSessionState::default();
        let thresholds = ExecutionThresholds::default();

        // 连续 3 次同一关键词：前 2 次放行，第 3 次拦截。
        let grep_args = json!({ "pattern": "log.*Error", "path": "src" });
        for index in 0..3 {
            let block = repeated_read_decide(
                &thresholds,
                "Grep",
                "log.*Error@src",
                &grep_args,
                &mut state,
            );
            if index < 2 {
                assert!(block.is_none(), "grep call {index} must pass");
            } else {
                let message = block.expect("third grep must be blocked");
                assert!(message.contains("已连续调用 3 次"));
                assert!(message.contains("请基于已有内容继续"));
            }
        }

        // 目标变化 → 重置：A→B→A 不误伤。
        let mut state = RepeatedReadSessionState::default();
        assert!(repeated_read_decide(
            &thresholds,
            "Grep",
            "log.*Error@src",
            &json!({ "pattern": "log.*Error", "path": "src" }),
            &mut state,
        )
        .is_none());
        assert!(repeated_read_decide(
            &thresholds,
            "Grep",
            "other@src",
            &json!({ "pattern": "other", "path": "src" }),
            &mut state,
        )
        .is_none());
        // A→B→A：目标回到 A，重新计数为 1，不拦。
        assert!(repeated_read_decide(
            &thresholds,
            "Grep",
            "log.*Error@src",
            &json!({ "pattern": "log.*Error", "path": "src" }),
            &mut state,
        )
        .is_none());
    }

    #[tokio::test]
    async fn repeated_read_cross_reference_a_b_a_not_blocked() {
        // type-contract §四.3：读 A → 读 B → 读 A（交叉引用）→ 不拦。
        let pipeline = test_tool_pipeline();
        register_static_test_tool(&pipeline, "Read", json!({ "ok": true }), 0).await;
        let file_a = std::env::temp_dir()
            .join("r-mr-11-cross-a.txt")
            .to_string_lossy()
            .to_string();
        let file_b = std::env::temp_dir()
            .join("r-mr-11-cross-b.txt")
            .to_string_lossy()
            .to_string();
        std::fs::write(&file_a, "a\n").ok();
        std::fs::write(&file_b, "b\n").ok();

        for (tool_id, path) in [
            ("cross-a1", file_a.as_str()),
            ("cross-b", file_b.as_str()),
            ("cross-a2", file_a.as_str()),
        ] {
            let task = repeated_read_task(tool_id, path, 0);
            pipeline.insert_tool_task_for_test(task).await;
            let result = pipeline
                .execute_single_tool(tool_id.to_string())
                .await
                .expect("execute single tool");
            assert_ne!(
                result.result.result["category"],
                json!("repeated_read_blocked"),
                "cross-reference {tool_id} must not be blocked"
            );
        }
        std::fs::remove_file(&file_a).ok();
        std::fs::remove_file(&file_b).ok();
    }

    #[tokio::test]
    async fn repeated_read_interleaved_production_resets() {
        // type-contract §四.4：读 A → 读 A(offset 10) → 写文件 → 读 A → 不拦
        // （中间有产出重置）。
        let pipeline = test_tool_pipeline();
        register_static_test_tool(&pipeline, "Read", json!({ "ok": true }), 0).await;
        register_static_test_tool(&pipeline, "Write", json!({ "written": true }), 0).await;
        let file_path = std::env::temp_dir()
            .join("r-mr-11-reset.txt")
            .to_string_lossy()
            .to_string();
        std::fs::write(&file_path, "x\n").ok();

        // 前 2 次连续读 A（不同 offset）。
        for (index, offset) in [0u64, 10u64].into_iter().enumerate() {
            let tool_id = format!("reset-read-{index}");
            let task = repeated_read_task(&tool_id, &file_path, offset);
            pipeline.insert_tool_task_for_test(task).await;
            let result = pipeline
                .execute_single_tool(tool_id)
                .await
                .expect("execute single tool");
            assert_ne!(
                result.result.result["category"],
                json!("repeated_read_blocked"),
                "pre-write read {index} must not be blocked"
            );
        }

        // 中间写文件（非读取类工具 → 重置计数）。
        let mut write_call = test_tool_call("reset-write", "Write");
        write_call.arguments = json!({ "payload": "+++ /tmp/reset.txt\nnew" });
        let write_task = ToolTask::new(
            write_call,
            test_tool_execution_context(),
            ToolExecutionOptions::default(),
        );
        pipeline.insert_tool_task_for_test(write_task).await;
        let result = pipeline
            .execute_single_tool("reset-write".to_string())
            .await
            .expect("write tool executes");
        assert_ne!(
            result.result.result["category"],
            json!("repeated_read_blocked"),
            "write must not be blocked"
        );

        // 再读 A：重置后重新计数为 1，不拦。
        let task = repeated_read_task("reset-read-after", &file_path, 20);
        pipeline.insert_tool_task_for_test(task).await;
        let result = pipeline
            .execute_single_tool("reset-read-after".to_string())
            .await
            .expect("execute single tool");
        assert_ne!(
            result.result.result["category"],
            json!("repeated_read_blocked"),
            "post-write read must not be blocked"
        );
        std::fs::remove_file(&file_path).ok();
    }

    #[tokio::test]
    async fn repeated_read_disabled_via_thresholds_config() {
        // type-contract §四.5：配置开关 enabled=false 不拦。
        let file_path = std::env::temp_dir()
            .join("r-mr-11-disabled.txt")
            .to_string_lossy()
            .to_string();
        std::fs::write(&file_path, "d\n").ok();

        // 直接构造纯函数判定验证开关：enabled=false → 永不拦截。
        let disabled = ExecutionThresholds {
            repeated_read_enabled: false,
            ..ExecutionThresholds::default()
        };
        let mut state = RepeatedReadSessionState::default();
        for _ in 0..5 {
            assert!(
                repeated_read_decide(
                    &disabled,
                    "Read",
                    &file_path,
                    &json!({ "file_path": file_path, "offset": 0 }),
                    &mut state,
                )
                .is_none(),
                "disabled threshold must never block"
            );
        }

        // 端到端：通过 pipeline 连读 3 次同一文件（enabled 默认 true 会拦第 3 次，
        // 但这里验证的是阈值配置关闭时的纯函数语义，故仅验证 pipeline 端到端拦截
        // 在开启时生效已在 repeated_read_same_file_three_offsets_blocks_third 覆盖）。
        // 本测试仅覆盖开关语义（纯函数层面，避免依赖全局配置注入）。
        std::fs::remove_file(&file_path).ok();
    }

    #[tokio::test]
    async fn repeated_read_offset_increment_fragment_blocks_with_guidance() {
        // 强化：连续分段读同文件（offset 递增十行读）→ 按同目标计数，3 次即拦；
        // 拦截提示含「碎片化读取」引导；小文件（<200 行）→ 提示一次读全文。
        // 小文件：<200 行。
        let small_path = std::env::temp_dir()
            .join("r-mr-11-small.txt")
            .to_string_lossy()
            .to_string();
        std::fs::write(&small_path, "small\n").ok();

        let mut state = RepeatedReadSessionState::default();
        let thresholds = ExecutionThresholds::default();

        // offset 递增的十行读同一小文件：第 3 次拦截 + 碎片化 + 小文件提示。
        let mut block_message = None;
        for (index, offset) in [0u64, 10u64, 20u64].into_iter().enumerate() {
            let arguments = json!({ "file_path": small_path, "offset": offset, "limit": 10 });
            let block =
                repeated_read_decide(&thresholds, "Read", &small_path, &arguments, &mut state);
            if index == 2 {
                block_message = block;
            }
        }
        let message = block_message.expect("third fragmented read must be blocked");
        assert!(
            message.contains("碎片化读取"),
            "fragment guidance must mention 碎片化读取, got: {message}"
        );
        assert!(
            message.contains("文件较小（<200 行），建议一次读全文"),
            "small-file hint must be present, got: {message}"
        );
        assert!(
            message.contains("正确做法：读全文（小文件）或搜索关键词定位（大文件）"),
            "correct-practice guidance must be present, got: {message}"
        );

        // 大文件（>=200 行）：有碎片化引导但无小文件提示。
        let big_path = std::env::temp_dir()
            .join("r-mr-11-big.txt")
            .to_string_lossy()
            .to_string();
        std::fs::write(&big_path, "line\n".repeat(300)).ok();
        let mut state = RepeatedReadSessionState::default();
        let mut big_block_message = None;
        for (index, offset) in [0u64, 10u64, 20u64].into_iter().enumerate() {
            let arguments = json!({ "file_path": big_path, "offset": offset, "limit": 10 });
            let block =
                repeated_read_decide(&thresholds, "Read", &big_path, &arguments, &mut state);
            if index == 2 {
                big_block_message = block;
            }
        }
        let message = big_block_message.expect("third fragmented big-file read must be blocked");
        assert!(message.contains("碎片化读取"));
        assert!(
            !message.contains("建议一次读全文"),
            "big file must not get the small-file hint, got: {message}"
        );

        std::fs::remove_file(&small_path).ok();
        std::fs::remove_file(&big_path).ok();
    }

    #[test]
    fn repeated_read_target_fingerprint_normalization() {
        // 目标指纹归一化：Read 忽略 offset/limit 分段；Grep 关键词+路径；Glob pattern；
        // WebSearch query；WebFetch url；LS path；非读取类工具 None。
        assert_eq!(
            repeated_read_target_fingerprint(
                "Read",
                &json!({ "file_path": "src/main.rs", "offset": 10, "limit": 10 }),
            )
            .as_deref(),
            Some("src/main.rs")
        );
        assert_eq!(
            repeated_read_target_fingerprint("Grep", &json!({ "pattern": "foo", "path": "src" }))
                .as_deref(),
            Some("foo@src")
        );
        assert_eq!(
            repeated_read_target_fingerprint("Grep", &json!({ "pattern": "foo" })).as_deref(),
            Some("foo@.")
        );
        assert_eq!(
            repeated_read_target_fingerprint("Glob", &json!({ "pattern": "**/*.ts" })).as_deref(),
            Some("**/*.ts")
        );
        assert_eq!(
            repeated_read_target_fingerprint("WebSearch", &json!({ "query": "rust async" }))
                .as_deref(),
            Some("rust async")
        );
        assert_eq!(
            repeated_read_target_fingerprint("WebFetch", &json!({ "url": "https://example.com" }),)
                .as_deref(),
            Some("https://example.com")
        );
        assert_eq!(
            repeated_read_target_fingerprint("LS", &json!({ "path": "src" })).as_deref(),
            Some("src")
        );
        assert_eq!(
            repeated_read_target_fingerprint("LS", &json!({})).as_deref(),
            Some(".")
        );
        assert!(repeated_read_target_fingerprint("Write", &json!({})).is_none());
        assert!(repeated_read_target_fingerprint("Read", &json!({})).is_none());
    }
}
