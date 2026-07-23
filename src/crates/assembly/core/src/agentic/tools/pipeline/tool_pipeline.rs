//! Tool pipeline
//!
//! Manages the complete lifecycle of tools:
//! permission authorization, execution, caching, retries, etc.

use super::state_manager::{tool_task_state_kind, ToolStateManager};
use super::types::*;
use crate::agentic::core::{ToolCall, ToolExecutionState, ToolResult as ModelToolResult};
use crate::agentic::events::types::ToolEventData;
use crate::agentic::tools::computer_use_host::ComputerUseHostRef;
use crate::agentic::tools::framework::ToolResult as FrameworkToolResult;
use crate::agentic::tools::registry::ToolRegistry;
use crate::agentic::tools::tool_context_runtime;
use crate::agentic::tools::tool_context_runtime::ToolUseContext;
use crate::agentic::tools::tool_result_storage;
use crate::util::elapsed_ms_u64;
use crate::util::errors::{BitFunError, BitFunResult};
use bitfun_agent_runtime::permission::{
    PendingPermissionReceiver, PermissionRequestManager, PermissionWaitOutcome,
};
use bitfun_agent_stream::ToolArgumentRepairKind;
use bitfun_agent_tools::{
    build_invalid_tool_call_error_message, build_normal_tool_json_repair_notice,
    build_permission_denied_tool_presentation, build_tool_execution_error_presentation,
    build_tool_execution_timeout_presentation,
    build_user_rejected_tool_presentation_with_instruction,
    build_user_steering_interrupted_presentation, build_write_tail_closure_notice,
    render_tool_result_for_assistant, truncate_raw_tool_arguments_preview,
    truncate_tool_arguments_preview, validate_tool_execution_admission, PermissionIntent,
    ResolvedToolInvocation, ToolExecutionAdmissionRejection, ToolExecutionAdmissionRequest,
    ToolExecutionErrorPresentation, GET_TOOL_SPEC_TOOL_NAME, USER_STEERING_INTERRUPTED_MESSAGE,
};
use bitfun_runtime_ports::{
    wildcard_matches, PermissionEffect, PermissionGrant, PermissionReply, PermissionRequest,
    PermissionRequestSource, PermissionRequestSourceKind, PermissionResourceCaseSensitivity,
    PermissionRule, RoundInjectionToolPreemption,
};
use futures::future::join_all;
use log::{debug, error, info, warn};
use std::collections::HashMap;
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

fn persisted_effective_tool_name(
    wire_tool_name: &str,
    effective_tool_name: &str,
) -> Option<String> {
    (wire_tool_name != effective_tool_name).then(|| effective_tool_name.to_string())
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
    let (tool_id, wire_tool_name, effective_tool_name, execution_time_ms, provided_arguments) =
        if let Some(task) = task {
            let preview = if task.invocation.is_deferred() {
                truncate_tool_arguments_preview(task.effective_arguments())
            } else {
                task.tool_call
                    .raw_arguments
                    .as_deref()
                    .map(truncate_raw_tool_arguments_preview)
                    .unwrap_or_else(|| truncate_tool_arguments_preview(task.effective_arguments()))
            };
            (
                task.tool_call.tool_id,
                task.tool_call.tool_name,
                task.invocation.effective_tool_name,
                elapsed_ms_since(task.created_at),
                Some(preview),
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
    let error_message = error.to_string();
    let category = classify_tool_error(error);
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
            is_error: true,
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
    identity: &crate::service::remote_ssh::workspace_state::WorkspaceSessionIdentity,
    is_remote: bool,
) -> BitFunResult<String> {
    if !is_remote {
        return Ok(
            bitfun_services_integrations::remote_ssh::paths::local_workspace_stable_storage_id(
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
        let key =
            bitfun_services_integrations::remote_ssh::paths::unresolved_remote_session_storage_key(
                connection_id,
                identity.logical_workspace_path(),
            );
        return Ok(format!("remote_unresolved_{key}"));
    }

    Ok(
        bitfun_services_integrations::remote_ssh::paths::remote_workspace_stable_id(
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

fn permission_intent_effect(
    intent: &PermissionIntent,
    rules: &[PermissionRule],
    grants: &[PermissionGrant],
    case_sensitivity: PermissionResourceCaseSensitivity,
) -> PermissionEffect {
    let evaluator = bitfun_runtime_ports::PermissionEvaluator::new(case_sensitivity);
    let mut aggregate = PermissionEffect::Allow;

    for resource in &intent.resources {
        let configured_effect = if intent.action == "bash" {
            rules
                .iter()
                .rev()
                .find(|rule| {
                    wildcard_matches(
                        &intent.action,
                        &rule.action,
                        PermissionResourceCaseSensitivity::Sensitive,
                    ) && match rule.effect {
                        PermissionEffect::Allow => rule.resource == *resource,
                        PermissionEffect::Ask | PermissionEffect::Deny => {
                            wildcard_matches(resource, &rule.resource, case_sensitivity)
                        }
                    }
                })
                .map(|rule| rule.effect)
                .unwrap_or(PermissionEffect::Ask)
        } else {
            evaluator.evaluate_resource(&intent.action, resource, rules)
        };

        match configured_effect {
            PermissionEffect::Deny => return PermissionEffect::Deny,
            PermissionEffect::Allow => {}
            PermissionEffect::Ask => {
                let remembered = grants.iter().any(|grant| {
                    if intent.action == "bash" {
                        grant.action == intent.action && grant.resource == *resource
                    } else {
                        wildcard_matches(
                            &intent.action,
                            &grant.action,
                            PermissionResourceCaseSensitivity::Sensitive,
                        ) && wildcard_matches(resource, &grant.resource, case_sensitivity)
                    }
                });
                if !remembered {
                    aggregate = PermissionEffect::Ask;
                }
            }
        }
    }

    let effect = if intent.resources.is_empty() {
        PermissionEffect::Ask
    } else {
        aggregate
    };
    if effect != PermissionEffect::Deny
        && intent
            .display_metadata
            .get("requiresFreshApproval")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false)
    {
        PermissionEffect::Ask
    } else {
        effect
    }
}

const SUBAGENT_LAUNCH_TOOL_NAME: &str = "Task";

/// Tool pipeline
#[derive(Clone)]
pub struct ToolPipeline {
    tool_registry: Arc<TokioRwLock<ToolRegistry>>,
    state_manager: Arc<ToolStateManager>,
    cancellation_tokens: ToolCancellationTokenStore,
    computer_use_host: Option<ComputerUseHostRef>,
    permission_request_manager: Option<Arc<PermissionRequestManager>>,
    permission_plans: Arc<TokioMutex<HashMap<String, PermissionExecutionPlan>>>,
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
        let permission_rules = task.options.permission_rules.clone();
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
        let mut asks = Vec::new();

        for intent in intents {
            match permission_intent_effect(&intent, &permission_rules, &grants, case_sensitivity) {
                PermissionEffect::Allow => {}
                PermissionEffect::Ask => asks.push(intent),
                PermissionEffect::Deny => {
                    return Ok(PermissionPlanDraft::Rejected {
                        reason: format!(
                            "Permission policy denied '{}' for {}",
                            intent.action,
                            intent.resources.join(", ")
                        ),
                    });
                }
            }
        }

        if asks.is_empty() {
            return Ok(PermissionPlanDraft::Allowed);
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
        auto_approve: bool,
    ) -> BitFunResult<Vec<PendingPermissionReceiver>> {
        let manager = self.permission_request_manager.as_ref().ok_or_else(|| {
            BitFunError::service(
                "Permission request manager is unavailable for a file tool request".to_string(),
            )
        })?;

        let receivers = if auto_approve {
            manager
                .register_batch_non_interactive(requests.clone())
                .await
        } else {
            manager.register_batch(requests.clone()).await
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

    async fn prepare_permission_plans(&self, task_ids: &[String]) -> BitFunResult<()> {
        let mut drafts = Vec::with_capacity(task_ids.len());
        let mut ordered_requests = Vec::new();

        for task_id in task_ids {
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
                if validate_tool_execution_admission(ToolExecutionAdmissionRequest {
                    tool_name: &tool_name,
                    allowed_tools: &task.context.allowed_tools,
                    runtime_tool_restrictions: &task.context.runtime_tool_restrictions,
                    invocation_is_deferred: task.invocation.is_deferred(),
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
            let receivers = self
                .register_permission_requests(batch_requests, auto_approve)
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
                self.register_permission_requests(requests, task.options.auto_approve_ask)
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

    fn should_interrupt_for_round_injection(&self, context: &ToolExecutionContext) -> bool {
        self.pending_round_injection_tool_preemption(context)
            .should_interrupt_after_current_atomic_unit()
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

    fn append_execution_result(
        &self,
        task_id: &str,
        result: BitFunResult<ToolExecutionResult>,
        all_results: &mut Vec<ToolExecutionResult>,
    ) {
        match result {
            Ok(execution_result) => all_results.push(execution_result),
            Err(error) => {
                error!("Tool execution failed: error={}", error);
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
                    let route_root = crate::external_tools::external_tool_route_root(
                        context
                            .workspace
                            .as_ref()
                            .map(|workspace| workspace.root_path()),
                        context
                            .workspace
                            .as_ref()
                            .is_some_and(|workspace| workspace.is_remote()),
                    );
                    let tool_is_concurrency_safe = registry
                        .get_tool(&invocation.effective_tool_name)
                        .and_then(|tool| {
                            crate::external_tools::resolve_external_tool_for_workspace(
                                tool, route_root,
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
            if batch_context
                .as_ref()
                .is_some_and(|context| self.should_interrupt_for_round_injection(context))
            {
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
            self.append_execution_result(task_id, result, &mut all_results);
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
            if task
                .as_ref()
                .is_some_and(|task| self.should_interrupt_for_round_injection(&task.context))
            {
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
            self.append_execution_result(&task_id, result, &mut results);
        }

        Ok(results)
    }

    /// Execute single tool
    async fn execute_single_tool(&self, tool_id: String) -> BitFunResult<ToolExecutionResult> {
        let start_time = Instant::now();

        debug!("Starting tool execution: tool_id={}", tool_id);

        // Get task
        let task = self
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
            let raw_arguments_preview = task
                .tool_call
                .raw_arguments
                .as_deref()
                .map(truncate_raw_tool_arguments_preview);
            Some(build_invalid_tool_call_error_message(
                &wire_tool_name,
                tool_is_error,
                recovered_from_truncation,
                raw_arguments_preview,
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

        // Repetition alone is not execution failure: polling and status checks
        // may legitimately reuse identical arguments. The execution engine
        // evaluates repeated patterns only after observing actual tool results.
        let (admission, tool) = {
            let registry = self.tool_registry.read().await;
            let admission = validate_tool_execution_admission(ToolExecutionAdmissionRequest {
                tool_name: &tool_name,
                allowed_tools: &task.context.allowed_tools,
                runtime_tool_restrictions: &task.context.runtime_tool_restrictions,
                invocation_is_deferred: task.invocation.is_deferred(),
                deferred_tools: &task.context.deferred_tools,
                loaded_deferred_tool_specs: &task.context.loaded_deferred_tool_specs,
                current_catalog_generation: registry.current_snapshot_generation(),
                get_tool_spec_tool_name: GET_TOOL_SPEC_TOOL_NAME,
            });
            (admission, registry.get_tool(&tool_name))
        };

        if let Err(err) = admission {
            let error_msg = err.to_string();
            if task.invocation.is_deferred() {
                warn!("Deferred tool gateway admission rejected: {}", error_msg);
            } else {
                warn!("Tool execution admission rejected: {}", error_msg);
            }

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

        let cancellation_token = CancellationToken::new();
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
            .insert(tool_id.clone(), cancellation_token.clone());

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

        let timeout_owner = crate::external_tools::resolve_external_tool_for_workspace(
            Arc::clone(&tool),
            crate::external_tools::external_tool_route_root(
                tool_context.workspace_root(),
                tool_context.is_remote(),
            ),
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
}

#[cfg(test)]
mod tests {
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
        PermissionGrant, PermissionGrantKey, PermissionGrantStorePort, PermissionReplyStorePort,
        PortResult, RoundInjection, RoundInjectionExecutionPolicy, RoundInjectionKind,
        RoundInjectionTarget, RoundInjectionToolPreemption, RuntimeServiceCapability,
        RuntimeServicePort,
    };
    use serde_json::json;
    use std::collections::HashMap;
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
    fn bash_permission_allows_only_exact_command_grants() {
        let intent = PermissionIntent::new("bash", vec!["git status && rm -rf build".to_string()]);
        let wildcard_allow = vec![PermissionRule::new(
            "bash",
            "git *",
            PermissionEffect::Allow,
        )];
        assert_eq!(
            permission_intent_effect(
                &intent,
                &wildcard_allow,
                &[],
                PermissionResourceCaseSensitivity::Sensitive,
            ),
            PermissionEffect::Ask
        );

        let exact_allow = vec![PermissionRule::new(
            "bash",
            "git status && rm -rf build",
            PermissionEffect::Allow,
        )];
        assert_eq!(
            permission_intent_effect(
                &intent,
                &exact_allow,
                &[],
                PermissionResourceCaseSensitivity::Sensitive,
            ),
            PermissionEffect::Allow
        );

        let wildcard_deny = vec![PermissionRule::new("bash", "*", PermissionEffect::Deny)];
        assert_eq!(
            permission_intent_effect(
                &intent,
                &wildcard_deny,
                &[],
                PermissionResourceCaseSensitivity::Sensitive,
            ),
            PermissionEffect::Deny
        );
    }

    #[test]
    fn account_scoped_fresh_approval_works_without_a_workspace_and_ignores_allow_rules() {
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

        let allow = vec![PermissionRule::new(
            "page_publish",
            "*",
            PermissionEffect::Allow,
        )];
        assert_eq!(
            permission_intent_effect(
                &intent,
                &allow,
                &[],
                PermissionResourceCaseSensitivity::Sensitive,
            ),
            PermissionEffect::Ask
        );
        let deny = vec![PermissionRule::new(
            "page_publish",
            "*",
            PermissionEffect::Deny,
        )];
        assert_eq!(
            permission_intent_effect(
                &intent,
                &deny,
                &[],
                PermissionResourceCaseSensitivity::Sensitive,
            ),
            PermissionEffect::Deny
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
        options.permission_rules = vec![PermissionRule::new(
            "custom_tool",
            "UnclassifiedMutation",
            PermissionEffect::Deny,
        )];

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
        allow_options.permission_rules = vec![PermissionRule::new(
            "edit",
            "src/*",
            PermissionEffect::Allow,
        )];
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
        deny_options.permission_rules = vec![
            PermissionRule::new("edit", "src/*", PermissionEffect::Allow),
            PermissionRule::new("edit", "src/private/*", PermissionEffect::Deny),
        ];
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
        deny_options.permission_rules = vec![
            PermissionRule::new("edit", "src/*", PermissionEffect::Allow),
            PermissionRule::new("edit", "src/private/*", PermissionEffect::Deny),
        ];
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
    async fn deferred_gateway_executes_effective_target_and_preserves_wire_identity() {
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
            "args": { "city": "Shanghai" }
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
            created_at: SystemTime::now(),
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
        assert!(result.result.is_error);
        assert_eq!(
            result.result.result["category"],
            serde_json::Value::String("user_steering_interrupted".to_string())
        );
        assert_eq!(
            result.result.result_for_assistant.as_deref(),
            Some(USER_STEERING_INTERRUPTED_MESSAGE)
        );
    }

    #[test]
    fn error_result_prefers_raw_arguments_preview_when_available() {
        let mut task = test_tool_task("tool_1", "Git");
        task.tool_call.arguments = json!({});
        task.tool_call.raw_arguments = Some("{\"operation\":\"log\"".to_string());

        let result = build_error_execution_result(
            "tool_1",
            Some(task),
            &BitFunError::Validation("Arguments are invalid JSON.".to_string()),
        );

        assert_eq!(
            result.result.result["provided_arguments"],
            serde_json::Value::String("{\"operation\":\"log\"".to_string())
        );
        assert!(result
            .result
            .result_for_assistant
            .as_deref()
            .unwrap_or_default()
            .contains("Provided arguments: {\"operation\":\"log\""));
        assert!(!result
            .result
            .result_for_assistant
            .as_deref()
            .unwrap_or_default()
            .contains("Raw arguments:"));
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
    async fn pipeline_admission_deferred_tool_rejection_updates_failed_state_before_validation() {
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
            "Tool 'WebFetch' is deferred and cannot be called directly",
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
            invocation_is_deferred: true,
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
            invocation_is_deferred: false,
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
}
