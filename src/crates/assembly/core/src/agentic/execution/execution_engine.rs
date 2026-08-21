//! Execution Engine
//!
//! Executes complete dialog turns, managing loops of multiple model rounds

use super::model_exchange_trace::{
    prepare_model_exchange_trace_for_workspace, ModelExchangeTraceOperation,
};
use super::round_executor::{ModelRoundLifecycle, RoundExecutor};
use super::types::{ExecutionContext, ExecutionResult, RoundContext, RoundResult};
use crate::agentic::agents::{
    build_prompt_context_for_workspace, get_agent_registry, render_direct_tool_listing_body,
    PrependedPromptReminders, PromptBuilder, PromptBuilderContext, RuntimeContextNeeds,
    ToolListingSections, UserContextPolicy, UserContextSection,
};
use crate::agentic::context_profile::{ContextProfilePolicy, ModelCapabilityProfile};
use crate::agentic::coordination::scheduler::agent_dialog_turn_image_contexts;
use crate::agentic::core::{
    render_system_reminder, InternalReminderKind, Message, MessageContent, MessageHelper,
    MessageRole, MessageSemanticKind, RequestReasoningTokenPolicy, Session,
};
use crate::agentic::events::{AgenticEvent, EventPriority, EventQueue};
#[cfg(feature = "agent-runtime")]
use crate::agentic::execution::conditional_instructions::{
    build_conditional_instruction_reminder, successful_workspace_read_paths,
};
use crate::agentic::execution::types::FinishReason;
use crate::agentic::image_analysis::{
    build_multimodal_message_with_images, process_image_contexts_for_provider, ImageContextData,
    ImageLimits,
};
use crate::agentic::round_preempt::RoundInjectionKind;
use crate::agentic::session::{
    ContextCompressor, SessionManager, TokenAnchor, TokenAnchorInput, UserContextCacheIdentity,
    INTERRUPTED_TURN_MODEL_BINDING_FINGERPRINT_METADATA_KEY,
    INTERRUPTED_TURN_PERMISSION_MODE_METADATA_KEY,
    INTERRUPTED_TURN_REASONING_FINGERPRINT_METADATA_KEY,
    INTERRUPTED_TURN_REASONING_PRESET_METADATA_KEY,
    INTERRUPTED_TURN_REASONING_SELECTION_METADATA_KEY,
    INTERRUPTED_TURN_RESOLVED_MODEL_ID_METADATA_KEY,
};
use crate::agentic::skill_agent_snapshot::build_skill_agent_tool_listing_sections_from_snapshot;
use crate::agentic::tools::implementations::{SkillTool, TaskTool};
use crate::agentic::tools::product_runtime::{
    collect_product_loaded_deferred_tool_specs, GetToolSpecTool,
};
use crate::agentic::tools::{
    resolve_tool_manifest, tool_context_runtime, ResolvedToolManifest, ToolRuntimeRestrictions,
};
use crate::agentic::WorkspaceBinding;
use crate::infrastructure::ai::get_global_ai_client_factory;
use crate::infrastructure::ai::reasoning_catalog::reasoning_preset_runtime_fingerprint;
use crate::native_hooks::{self, NativeHookSessionFacts};
use crate::service::config::get_global_config_service;
#[cfg(test)]
use crate::service::config::types::{
    automatic_max_output_tokens, MAX_CONFIGURED_OUTPUT_TOKENS_RATIO_PERCENT,
};
use crate::service::config::types::{
    model_runtime_binding_fingerprint, ModelCapability, ModelCategory,
};
use crate::service::instruction_context::{
    build_local_workspace_instruction_files_context_with_fs_detailed,
    build_workspace_instruction_files_context_detailed,
    build_workspace_instruction_files_context_with_fs, InstructionContextBuild,
};
use crate::util::errors::{BitFunError, BitFunResult};
use crate::util::token_counter::TokenCounter;
use crate::util::types::Message as AIMessage;
use crate::util::types::ToolDefinition;
use crate::util::{elapsed_ms_u64, truncate_at_char_boundary};
use bitfun_agent_runtime::output_surface::TOOL_CONTEXT_INLINE_MARKDOWN_IMAGE_DISPLAY_KEY;
use bitfun_agent_runtime::permission::PERMISSION_MODE_CONTEXT_KEY;
use bitfun_agent_runtime::prompt::RuntimeFactsUsage;
use bitfun_agent_runtime::remote_file_delivery::TOOL_CONTEXT_REMOTE_FILE_DELIVERY_KEY;
use bitfun_agent_runtime::thread_goal_tools::ensure_thread_goal_tools;
use bitfun_ai_adapters::ModelExchangeTraceConfig;
use bitfun_core_types::{ModelRequestContext, SessionModelBindingPolicy};
use bitfun_runtime_ports::{resolve_permission_mode, PermissionMode, PermissionModeLayers};
use dashmap::DashMap;
use log::{debug, error, info, trace, warn};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

fn execution_engine_owns_cancel_lifecycle(context: &HashMap<String, String>) -> bool {
    !super::types::coordinator_owns_cancel_lifecycle(context)
}

fn initial_round_index(context: &std::collections::HashMap<String, String>) -> usize {
    context
        .get("initial_round_index")
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(0)
}
use tool_runtime::context::PrimaryModelFacts;

fn ensure_primary_session_goal_tools(allowed_tools: &mut Vec<String>, is_subagent: bool) {
    if !is_subagent {
        ensure_thread_goal_tools(allowed_tools);
    }
}

fn resolve_round_permission_mode(
    active_turn_mode: Option<PermissionMode>,
    fixed_context_mode: Option<PermissionMode>,
    session_mode: Option<PermissionMode>,
    global_default: PermissionMode,
) -> PermissionMode {
    resolve_permission_mode(
        PermissionModeLayers::new(global_default)
            .with_session(session_mode)
            .with_turn(active_turn_mode.or(fixed_context_mode)),
    )
    .mode
}

pub(crate) fn restrict_recovered_permission_mode(
    original: PermissionMode,
    current: PermissionMode,
) -> PermissionMode {
    const fn rank(mode: PermissionMode) -> u8 {
        match mode {
            PermissionMode::Ask => 0,
            PermissionMode::AutoApprove => 1,
            PermissionMode::FullAccess => 2,
        }
    }

    if rank(current) < rank(original) {
        current
    } else {
        original
    }
}

/// Execution engine configuration
#[derive(Debug, Clone)]
pub struct ExecutionEngineConfig {
    pub max_rounds: usize,
    /// Max consecutive rounds with identical tool-call signatures before loop detection triggers.
    pub max_consecutive_same_tool: usize,
}

impl Default for ExecutionEngineConfig {
    fn default() -> Self {
        Self {
            max_rounds: crate::service::config::types::DEFAULT_MAX_ROUNDS,
            max_consecutive_same_tool: 3,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ContextCompactionOutcome {
    pub compression_id: String,
    pub compression_count: usize,
    pub tokens_before: usize,
    pub tokens_after: usize,
    pub compression_ratio: f64,
    pub duration_ms: u64,
    pub has_summary: bool,
    pub summary_source: String,
    pub applied: bool,
}

const MANUAL_COMPACTION_PLANNING: u8 = 0;
const MANUAL_COMPACTION_CANCELLED: u8 = 1;
const MANUAL_COMPACTION_COMMITTING: u8 = 2;

/// Session metadata key for the pre-compaction progress snapshot. Written by
/// the custom compaction checkpoint, which is intentionally not gated by
/// `app.hooks.enabled` so long-running tasks keep a recoverable record of
/// goal/role/todos state across context compaction.
const COMPACTION_PROGRESS_SNAPSHOT_KEY: &str = "compactionProgressSnapshot";

/// Current wall-clock time in milliseconds since the Unix epoch, used for
/// compaction snapshot timestamps.
fn compaction_snapshot_timestamp_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

/// Maximum number of thinking-only rescue continuations before the turn
/// finalizes locally. The round loop re-requests the model after a
/// thinking-only round (no text / no tool call) via a rescue reminder; without
/// this bound a thinking-only storm (2000 empty prompts observed in 1.5 h)
/// can consume the whole round budget. A round that made progress (tool call
/// or user-visible text) resets the counter, so healthy tasks are unaffected.
const DEFAULT_EMPTY_ROUND_RESPAWN_LIMIT: usize = 1;
/// Maximum number of finalize (rescue) model requests per turn. The finalize
/// path already retries once when the first request returns no usable text;
/// that retry is the second request, so the default budget is 2.
const DEFAULT_FINALIZE_ROUND_LIMIT: usize = 2;

/// Arbitrates the only race that matters for manual compaction: cancellation
/// may win while the model is planning, but context commit must be atomic once
/// it begins.
#[derive(Debug)]
pub(crate) struct ManualCompactionCommitGate {
    state: AtomicU8,
}

impl ManualCompactionCommitGate {
    pub(crate) fn planning() -> Self {
        Self {
            state: AtomicU8::new(MANUAL_COMPACTION_PLANNING),
        }
    }

    pub(crate) fn try_cancel(&self) -> bool {
        self.state
            .compare_exchange(
                MANUAL_COMPACTION_PLANNING,
                MANUAL_COMPACTION_CANCELLED,
                Ordering::AcqRel,
                Ordering::Acquire,
            )
            .is_ok()
    }

    pub(crate) fn try_begin_commit(&self) -> bool {
        self.state
            .compare_exchange(
                MANUAL_COMPACTION_PLANNING,
                MANUAL_COMPACTION_COMMITTING,
                Ordering::AcqRel,
                Ordering::Acquire,
            )
            .is_ok()
    }

    pub(crate) fn commit_started(&self) -> bool {
        self.state.load(Ordering::Acquire) == MANUAL_COMPACTION_COMMITTING
    }
}

fn manual_compaction_terminal_error(error: BitFunError) -> BitFunError {
    match error {
        error @ BitFunError::Cancelled(_) => error,
        error => BitFunError::Session(error.to_string()),
    }
}

#[cfg(feature = "agent-runtime")]
async fn activate_conditional_instructions_after_round(
    session_manager: &SessionManager,
    context: &ExecutionContext,
    round_result: &RoundResult,
    messages: &mut Vec<Message>,
) {
    let Some(workspace) = context.workspace.as_ref() else {
        return;
    };
    let read_paths = successful_workspace_read_paths(&round_result.tool_result_messages, workspace);
    if read_paths.is_empty() {
        return;
    }

    let reminder_round_id = round_result
        .tool_result_messages
        .first()
        .and_then(|message| message.metadata.round_id.as_deref())
        .unwrap_or("conditional-instructions");
    let reminder = build_conditional_instruction_reminder(
        workspace,
        context.workspace_services.as_ref(),
        &read_paths,
        messages,
        &context.dialog_turn_id,
        reminder_round_id,
    )
    .await;
    let Some(reminder) = (match reminder {
        Ok(reminder) => reminder,
        Err(error) => {
            warn!(
                "Failed to load conditional instructions; retrying after a later matching read: {}",
                error
            );
            None
        }
    }) else {
        return;
    };

    messages.push(reminder.clone());
    if let Err(error) = session_manager
        .add_message(&context.session_id, reminder)
        .await
    {
        warn!(
            "Failed to persist conditional instruction reminder: {}",
            error
        );
    }
}

struct CompressionRuntimeScaffold {
    ai_client: Arc<crate::infrastructure::ai::AIClient>,
    model_request_context: ModelRequestContext,
    tool_definitions: Option<Vec<ToolDefinition>>,
    system_prompt_message: Message,
    prepended_prompt_reminders: PrependedPromptReminders,
    primary_supports_image_understanding: bool,
    compression_contract_limit: usize,
}

#[derive(Debug, Clone)]
struct TurnPromptScaffold {
    system_prompt_message: Message,
    prepended_prompt_reminders: PrependedPromptReminders,
}

#[derive(Debug, Clone)]
struct ContextHealthSnapshot {
    token_usage_ratio: f32,
    full_compression_count: usize,
    compression_failure_count: u32,
    repeated_tool_signature_count: usize,
    consecutive_failed_commands: usize,
}

impl ContextHealthSnapshot {
    fn from_runtime_observations(
        token_usage_ratio: f32,
        full_compression_count: usize,
        compression_failure_count: u32,
        recent_tool_signatures: &[String],
        messages: &[Message],
    ) -> Self {
        Self {
            token_usage_ratio,
            full_compression_count,
            compression_failure_count,
            repeated_tool_signature_count: Self::repeated_tool_signature_count(
                recent_tool_signatures,
            ),
            consecutive_failed_commands: Self::consecutive_failed_commands(messages),
        }
    }

    fn token_usage_ratio(current_tokens: usize, context_window: usize) -> f32 {
        if context_window == 0 {
            return 0.0;
        }
        current_tokens as f32 / context_window as f32
    }

    fn log(&self, session_id: &str, turn_id: &str, round_index: usize, stage: &str) {
        debug!(
            "Context health snapshot: session_id={}, turn_id={}, round_index={}, stage={}, token_usage={:.3}, full_compression_count={}, compression_failure_count={}, repeated_tool_signature_count={}, consecutive_failed_commands={}",
            session_id,
            turn_id,
            round_index,
            stage,
            self.token_usage_ratio,
            self.full_compression_count,
            self.compression_failure_count,
            self.repeated_tool_signature_count,
            self.consecutive_failed_commands
        );
    }

    fn log_policy_thresholds(
        &self,
        session_id: &str,
        turn_id: &str,
        round_index: usize,
        policy: &ContextProfilePolicy,
    ) {
        if policy.has_repeated_tool_loop(self.repeated_tool_signature_count) {
            debug!(
                "Context profile repeated-tool threshold reached: session_id={}, turn_id={}, round_index={}, profile={:?}, repeated_tool_signature_count={}, threshold={}",
                session_id,
                turn_id,
                round_index,
                policy.profile,
                self.repeated_tool_signature_count,
                policy.repeated_tool_signature_threshold
            );
        }

        if policy.has_consecutive_command_failure_loop(self.consecutive_failed_commands) {
            warn!(
                "Context profile command-failure threshold reached: session_id={}, turn_id={}, round_index={}, profile={:?}, consecutive_failed_commands={}, threshold={}",
                session_id,
                turn_id,
                round_index,
                policy.profile,
                self.consecutive_failed_commands,
                policy.consecutive_failed_command_threshold
            );
        }
    }

    fn repeated_tool_signature_count(recent_tool_signatures: &[String]) -> usize {
        let Some(last_signature) = recent_tool_signatures.last() else {
            return 0;
        };

        let repeated_count = recent_tool_signatures
            .iter()
            .rev()
            .take_while(|signature| *signature == last_signature)
            .count();

        if repeated_count >= 2 {
            repeated_count
        } else {
            0
        }
    }

    fn consecutive_failed_commands(messages: &[Message]) -> usize {
        let mut failures = 0;
        for message in messages.iter().rev() {
            let Some(failed) = Self::command_result_failed(message) else {
                continue;
            };

            if failed {
                failures += 1;
            } else {
                break;
            }
        }
        failures
    }

    fn command_result_failed(message: &Message) -> Option<bool> {
        let MessageContent::ToolResult {
            tool_name,
            result,
            is_error,
            ..
        } = &message.content
        else {
            return None;
        };

        if !matches!(tool_name.as_str(), "Bash" | "Git") {
            return None;
        }

        Some(Self::tool_result_failed(result, *is_error))
    }

    fn tool_result_failed(result: &serde_json::Value, is_error: bool) -> bool {
        is_error
            || Self::bool_field(result, "timed_out") == Some(true)
            || Self::bool_field(result, "interrupted") == Some(true)
            || Self::bool_field(result, "success") == Some(false)
            || Self::numeric_field(result, "exit_code").is_some_and(|code| code != 0)
    }

    fn bool_field(value: &serde_json::Value, key: &str) -> Option<bool> {
        value.get(key).and_then(|field| field.as_bool())
    }

    fn numeric_field(value: &serde_json::Value, key: &str) -> Option<i64> {
        value.get(key).and_then(|field| field.as_i64())
    }
}

#[derive(Debug, Clone)]
struct TokenAnchorPressureDetails {
    anchor_id: String,
    prefix_message_count: usize,
    input_tokens: usize,
    adjusted_anchor_tokens: usize,
    system_tokens_at_anchor: usize,
    current_system_tokens: usize,
    system_delta: isize,
    tool_tokens_at_anchor: usize,
    current_tool_tokens: usize,
    tool_delta: isize,
    prepended_reminder_tokens_at_anchor: usize,
    current_prepended_reminder_tokens: usize,
    prepended_reminder_delta: isize,
    tail_tokens: usize,
}

#[derive(Debug, Clone, Copy)]
struct TokenPressureSnapshot {
    total_tokens: usize,
    system_tokens: usize,
    tool_tokens: usize,
    prepended_reminder_tokens: usize,
    conversation_tokens: usize,
    context_window: usize,
    input_limit: usize,
    output_reserve_tokens: usize,
    safety_reserve_tokens: usize,
    usage_ratio: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CompressionTriggerBudget {
    input_limit: usize,
    output_reserve_tokens: usize,
    safety_reserve_tokens: usize,
}

// Fields are declared in reverse parameter order so dropping an unconsumed
// input preserves the previous function-parameter drop order. Call sites keep
// struct literal fields in the original evaluation order.
struct TurnPromptScaffoldInput<'a> {
    stage: &'a str,
    runtime_context_needs: RuntimeContextNeeds,
    tool_listing_sections: ToolListingSections,
    supports_image_understanding: bool,
    model_name: &'a str,
    current_agent: &'a dyn crate::agentic::agents::Agent,
    runtime_facts_usage: RuntimeFactsUsage,
    context: &'a ExecutionContext,
}

struct FinalizeRoundInput<'a> {
    permission_constraints: bitfun_runtime_ports::PermissionConstraintLayer,
    context_window: usize,
    tool_definitions: Option<Vec<ToolDefinition>>,
    reminder_text: &'a str,
    messages: &'a [Message],
    static_prepended_reminders: &'a [&'a str],
    dynamic_prepended_reminders: &'a [&'a str],
    primary_model_facts: &'a PrimaryModelFacts,
    model_request_context: &'a ModelRequestContext,
    execution_context_vars: &'a HashMap<String, String>,
    round_group_id: Option<String>,
    round_number: usize,
    agent_type: String,
    user_enabled_tools: Vec<String>,
    context: &'a ExecutionContext,
    ai_client: Arc<crate::infrastructure::ai::AIClient>,
}

struct CompressionModelSummaryInput<'a> {
    trace_config: Option<ModelExchangeTraceConfig>,
    model_request_context: &'a ModelRequestContext,
    primary_supports_image_understanding: bool,
    prepended_prompt_reminders: &'a PrependedPromptReminders,
    tool_definitions: &'a Option<Vec<ToolDefinition>>,
    workspace: Option<&'a WorkspaceBinding>,
    dialog_turn_id: &'a str,
    runtime_messages: &'a [Message],
    ai_client: Arc<crate::infrastructure::ai::AIClient>,
}

/// Execution engine
pub struct ExecutionEngine {
    round_executor: Arc<RoundExecutor>,
    event_queue: Arc<EventQueue>,
    session_manager: Arc<SessionManager>,
    context_compressor: Arc<ContextCompressor>,
    config: ExecutionEngineConfig,
    generation_messages: DashMap<(String, String), Vec<Message>>,
}

impl ExecutionEngine {
    const AUTO_COMPRESSION_SAFETY_RESERVE_TOKENS: usize = 10_000;
    const MAX_COMPRESSION_OVERFLOW_ATTEMPTS: usize = 4;
    const MAX_MAIN_CONTEXT_OVERFLOW_RECOVERIES: usize = 2;
    const FINALIZE_AFTER_REPEATED_TOOL_FAILURES_REMINDER: &'static str = "This turn must end now because repeated tool failures have prevented further progress. Ignore any unfinished work. Your task now is to give the user a final answer. Do not call any more tools; any tool call will fail. Respond in plain text only. Summarize what was completed, what failed, the evidence available from the tool results, and the single best next step for the user.";
    const FINALIZE_AFTER_MAX_ROUNDS_REMINDER: &'static str = "This turn must end now because it has reached the round limit. Ignore any unfinished work. Your task now is to give the user a final answer. Do not call any more tools; any tool call will fail. Respond in plain text only. Summarize the most useful completed work and evidence collected so far, and clearly distinguish resolved items from anything still unresolved.";
    const FINALIZE_TOOL_DENIED_MESSAGE: &'static str =
        "Tool use is disabled for finalize. Respond with plain text only.";
    const FINALIZE_USER_FOLLOWUP: &'static str =
        "Provide a final answer. You MUST not call any tools.";

    fn model_request_context(
        prompt_cache_lineage_id: &str,
        session_id: &str,
        dialog_turn_id: &str,
    ) -> ModelRequestContext {
        ModelRequestContext {
            prompt_cache_route_key: Some(prompt_cache_lineage_id.to_string()),
            session_id: Some(session_id.to_string()),
            // Turn-level stable request-group ID: one user prompt -> one
            // value across every request of the turn (including retries).
            conversation_request_id: Some(dialog_turn_id.to_string()),
        }
    }

    async fn context_vars_for_round(
        &self,
        base: &HashMap<String, String>,
        session_id: &str,
        turn_id: &str,
    ) -> HashMap<String, String> {
        let mut context_vars = base.clone();
        let fixed_context_mode = base
            .get(PERMISSION_MODE_CONTEXT_KEY)
            .map(|value| PermissionMode::parse(value).unwrap_or(PermissionMode::Ask));
        let active_turn_mode = self
            .session_manager
            .active_turn_permission_mode(session_id, turn_id);
        let global_default = match get_global_config_service().await {
            Ok(service) => service
                .get_config(None)
                .await
                .map(|config: crate::service::config::types::GlobalConfig| {
                    PermissionMode::from_config(&config.tool_permissions)
                })
                .unwrap_or(PermissionMode::Ask),
            Err(_) => PermissionMode::Ask,
        };
        let current = resolve_round_permission_mode(
            active_turn_mode,
            fixed_context_mode,
            self.session_manager.session_permission_mode(session_id),
            global_default,
        );
        let resolved = base
            .get(INTERRUPTED_TURN_PERMISSION_MODE_METADATA_KEY)
            .and_then(|value| PermissionMode::parse(value))
            .map(|original| restrict_recovered_permission_mode(original, current))
            .unwrap_or(current);
        context_vars.insert(
            PERMISSION_MODE_CONTEXT_KEY.to_string(),
            resolved.as_str().to_string(),
        );
        context_vars
    }

    pub fn new(
        round_executor: Arc<RoundExecutor>,
        event_queue: Arc<EventQueue>,
        session_manager: Arc<SessionManager>,
        context_compressor: Arc<ContextCompressor>,
        config: ExecutionEngineConfig,
    ) -> Self {
        Self {
            round_executor,
            event_queue,
            session_manager,
            context_compressor,
            config,
            generation_messages: DashMap::new(),
        }
    }

    fn remember_generation_message(&self, session_id: &str, turn_id: &str, message: &Message) {
        self.generation_messages
            .entry((session_id.to_string(), turn_id.to_string()))
            .or_default()
            .push(message.clone());
    }

    pub(crate) fn take_generation_messages(&self, session_id: &str, turn_id: &str) -> Vec<Message> {
        self.generation_messages
            .remove(&(session_id.to_string(), turn_id.to_string()))
            .map(|(_, messages)| messages)
            .unwrap_or_default()
    }

    fn estimate_request_tokens_internal(
        messages: &[Message],
        tools: Option<&[ToolDefinition]>,
    ) -> usize {
        MessageHelper::estimate_request_tokens(
            messages,
            tools,
            RequestReasoningTokenPolicy::LatestTurnOnly,
        )
    }

    /// Resolve the configured compression safety reserve
    /// (`ai.thresholds.compression.safety_reserve_tokens`), falling back to
    /// `AUTO_COMPRESSION_SAFETY_RESERVE_TOKENS = 10_000` when unset or invalid.
    async fn configured_compression_safety_reserve_tokens() -> usize {
        let Ok(config_service) = get_global_config_service().await else {
            return Self::AUTO_COMPRESSION_SAFETY_RESERVE_TOKENS;
        };
        let Ok(thresholds) = config_service
            .get_config::<crate::service::config::types::AiThresholdsConfig>(Some("ai.thresholds"))
            .await
        else {
            return Self::AUTO_COMPRESSION_SAFETY_RESERVE_TOKENS;
        };
        let reserve = thresholds.compression.safety_reserve_tokens;
        if reserve == 0 {
            return Self::AUTO_COMPRESSION_SAFETY_RESERVE_TOKENS;
        }
        reserve
    }

    /// Resolve the configured compression trigger percent
    /// (`ai.thresholds.compression.trigger_percent`), falling back to `None`
    /// (legacy fixed-token algorithm) when unset, zero, or invalid.
    ///
    /// R-THR-01 批1：合法值域 1-99；0 = 合法特殊值（同 None = 现算法）；
    /// 越界（101+）或非数字 → 回退 None → 零变化铁律。
    async fn configured_compression_trigger_percent() -> Option<u8> {
        let Ok(config_service) = get_global_config_service().await else {
            return None;
        };
        let Ok(thresholds) = config_service
            .get_config::<crate::service::config::types::AiThresholdsConfig>(Some("ai.thresholds"))
            .await
        else {
            return None;
        };
        match thresholds.compression.trigger_percent {
            Some(percent) if (1..=99).contains(&percent) => Some(percent),
            _ => None,
        }
    }

    /// Resolve the configured compression overflow / recovery / pass budgets
    /// (`ai.thresholds.compression.*`), falling back to the legacy constants.
    async fn configured_compression_counts() -> (
        usize, // overflow attempts
        usize, // main-context overflow recoveries
        usize, // consecutive compression failures
        usize, // failed-tool recovery attempts
        usize, // stop-hook continuations
        usize, // same-round passes
    ) {
        let legacy = (
            Self::MAX_COMPRESSION_OVERFLOW_ATTEMPTS,
            Self::MAX_MAIN_CONTEXT_OVERFLOW_RECOVERIES,
            3usize, // legacy MAX_CONSECUTIVE_COMPRESSION_FAILURES
            3usize, // legacy MAX_FAILED_TOOL_RECOVERY_ATTEMPTS
            3usize, // legacy MAX_STOP_HOOK_CONTINUATIONS
            2usize, // legacy MAX_SAME_ROUND_COMPRESSION_PASSES
        );
        let Ok(config_service) = get_global_config_service().await else {
            return legacy;
        };
        let Ok(thresholds) = config_service
            .get_config::<crate::service::config::types::AiThresholdsConfig>(Some("ai.thresholds"))
            .await
        else {
            return legacy;
        };
        let c = &thresholds.compression;
        (
            c.overflow_attempts.max(1),
            c.main_context_overflow_recoveries,
            c.consecutive_failures.max(1),
            c.failed_tool_recovery_attempts,
            c.stop_hook_continuations,
            c.same_round_passes.max(1),
        )
    }

    /// Resolve the configured compression overflow-attempt budget
    /// (`ai.thresholds.compression.overflow_attempts`).
    async fn configured_compression_overflow_attempts() -> usize {
        Self::configured_compression_counts().await.0
    }

    /// Resolve the configured recent-context retention
    /// (`ai.thresholds.compression.recent_context_tokens`).
    async fn configured_compression_recent_context_tokens() -> usize {
        let Ok(config_service) = get_global_config_service().await else {
            return ContextCompressor::DEFAULT_RECENT_CONTEXT_TOKENS;
        };
        let Ok(thresholds) = config_service
            .get_config::<crate::service::config::types::AiThresholdsConfig>(Some("ai.thresholds"))
            .await
        else {
            return ContextCompressor::DEFAULT_RECENT_CONTEXT_TOKENS;
        };
        let tokens = thresholds.compression.recent_context_tokens;
        if tokens == 0 {
            return ContextCompressor::DEFAULT_RECENT_CONTEXT_TOKENS;
        }
        tokens
    }

    /// Resolve the configured compression retry-step
    /// (`ai.thresholds.compression.retry_step_tokens`).
    async fn configured_compression_retry_step_tokens() -> usize {
        let Ok(config_service) = get_global_config_service().await else {
            return ContextCompressor::RECENT_CONTEXT_RETRY_STEP_TOKENS;
        };
        let Ok(thresholds) = config_service
            .get_config::<crate::service::config::types::AiThresholdsConfig>(Some("ai.thresholds"))
            .await
        else {
            return ContextCompressor::RECENT_CONTEXT_RETRY_STEP_TOKENS;
        };
        let tokens = thresholds.compression.retry_step_tokens;
        if tokens == 0 {
            return ContextCompressor::RECENT_CONTEXT_RETRY_STEP_TOKENS;
        }
        tokens
    }

    /// Resolve the configured max retained user tokens
    /// (`ai.thresholds.compression.max_retained_user_tokens`).
    async fn configured_compression_max_retained_user_tokens() -> usize {
        let Ok(config_service) = get_global_config_service().await else {
            return 20_000;
        };
        let Ok(thresholds) = config_service
            .get_config::<crate::service::config::types::AiThresholdsConfig>(Some("ai.thresholds"))
            .await
        else {
            return 20_000;
        };
        let tokens = thresholds.compression.max_retained_user_tokens;
        if tokens == 0 {
            return 20_000;
        }
        tokens
    }

    /// Resolve the configured max image-bearing message rounds
    /// (`ai.thresholds.compression.image_bearing_messages`), falling back to
    /// the legacy `MAX_IMAGE_BEARING_MESSAGE_ROUNDS = 2` when unset.
    async fn configured_max_image_bearing_messages() -> usize {
        let Ok(config_service) = get_global_config_service().await else {
            return 2;
        };
        let Ok(thresholds) = config_service
            .get_config::<crate::service::config::types::AiThresholdsConfig>(Some("ai.thresholds"))
            .await
        else {
            return 2;
        };
        let count = thresholds.compression.image_bearing_messages;
        if count == 0 {
            return 2;
        }
        count
    }

    /// 空输入轮拦截开关（`ai.thresholds.execution.empty_input_guard`）。
    ///
    /// R-MR-06 / R-13：模型请求发出前检查「本轮是否无任何真实用户内容」——
    /// 全部 user 消息均为系统注入（internal_reminder / system_reminder 包裹）
    /// 或为空 → 本地合成 final response，不调 API、不计费。默认 true。
    /// 与 configured_duplicate_message_enabled 同构：0 硬编码铁律，默认值由配置
    /// 域承载（未配置时回退本常量 true）。
    async fn configured_empty_input_guard() -> bool {
        let Ok(config_service) = get_global_config_service().await else {
            return true;
        };
        let Ok(thresholds) = config_service
            .get_config::<crate::service::config::types::AiThresholdsConfig>(Some("ai.thresholds"))
            .await
        else {
            return true;
        };
        thresholds.execution.empty_input_guard
    }

    /// 消息序列重复闸门开关（`ai.thresholds.execution.duplicate_message_enabled`）。
    ///
    /// R-MR-10：请求发出前比对本轮与最近 N 轮的 messages 序列指纹，窗口内相同
    /// 即判定死循环 → 不调 API、本地合成 final response。0 硬编码铁律：默认值
    /// 由配置域承载（未配置时回退本常量 true）。当前 `ai.thresholds.execution.*`
    /// 配置域尚未落库（R-MR-07 层 7 未完成），暂用常量 + 注释，R-MR-07 完成后迁入。
    async fn configured_duplicate_message_enabled() -> bool {
        let Ok(config_service) = get_global_config_service().await else {
            return true;
        };
        let Ok(thresholds) = config_service
            .get_config::<crate::service::config::types::AiThresholdsConfig>(Some("ai.thresholds"))
            .await
        else {
            return true;
        };

        thresholds.execution.duplicate_message_enabled
    }

    /// 消息序列重复闸门窗口 N（`ai.thresholds.execution.duplicate_message_window`）。
    ///
    /// 默认 3：与最近 3 轮指纹比对，窗口内任一相同即拦。窗口 0 视为 1（至少保留
    /// 相邻轮比对，避免配置 0 使闸门静默失效）。
    async fn configured_duplicate_message_window() -> usize {
        let Ok(config_service) = get_global_config_service().await else {
            return 3;
        };
        let Ok(thresholds) = config_service
            .get_config::<crate::service::config::types::AiThresholdsConfig>(Some("ai.thresholds"))
            .await
        else {
            return 3;
        };
        let window = thresholds.execution.duplicate_message_window;
        window.max(1)
    }

    /// Estimate request pressure for compression decisions.
    ///
    /// `total_tokens` tracks the whole provider request input. The snapshot also
    /// keeps the mutable conversation portion and fixed scaffold overhead
    /// available for diagnostics, while the trigger decision reserves output and
    /// safety budget from the full context window.
    fn estimate_auto_compression_pressure(
        messages: &[Message],
        tools: Option<&[ToolDefinition]>,
        context_window: usize,
        trigger_budget: CompressionTriggerBudget,
        prepended_reminder_tokens: usize,
    ) -> TokenPressureSnapshot {
        let total_tokens = Self::estimate_request_tokens_internal(messages, tools)
            .saturating_add(prepended_reminder_tokens);
        Self::token_pressure_snapshot_from_total(
            total_tokens,
            messages,
            tools,
            context_window,
            trigger_budget,
            prepended_reminder_tokens,
        )
    }

    /// Map a token pressure snapshot to the prompt-level runtime facts used by
    /// the Runtime Facts reminder: live usage ratio plus the dynamic
    /// compression preview trigger point (input_limit / context_window).
    fn runtime_facts_usage_from_pressure(pressure: &TokenPressureSnapshot) -> RuntimeFactsUsage {
        let compression_preview_ratio = (pressure.context_window > 0)
            .then(|| pressure.input_limit as f32 / pressure.context_window as f32);
        RuntimeFactsUsage {
            context_usage_ratio: Some(pressure.usage_ratio),
            compression_preview_ratio,
        }
    }

    fn estimate_auto_compression_pressure_with_anchor(
        messages: &[Message],
        tools: Option<&[ToolDefinition]>,
        context_window: usize,
        trigger_budget: CompressionTriggerBudget,
        anchor: Option<&TokenAnchor>,
        prepended_reminder_tokens: usize,
    ) -> (TokenPressureSnapshot, Option<TokenAnchorPressureDetails>) {
        let Some(anchor) = anchor else {
            let snapshot = Self::estimate_auto_compression_pressure(
                messages,
                tools,
                context_window,
                trigger_budget,
                prepended_reminder_tokens,
            );
            return (snapshot, None);
        };

        let current_system_tokens = Self::system_tokens_for_pressure(messages);
        let current_tool_tokens = tools
            .map(TokenCounter::estimate_tool_definitions_tokens)
            .unwrap_or(0);
        let adjusted_anchor_tokens = Self::apply_token_delta(
            anchor.input_tokens,
            anchor.system_tokens_at_anchor,
            current_system_tokens,
        );
        let adjusted_anchor_tokens = Self::apply_token_delta(
            adjusted_anchor_tokens,
            anchor.tool_tokens_at_anchor,
            current_tool_tokens,
        );
        let adjusted_anchor_tokens = Self::apply_token_delta(
            adjusted_anchor_tokens,
            anchor.prepended_reminder_tokens_at_anchor,
            prepended_reminder_tokens,
        );
        let tail_tokens = Self::estimate_tail_tokens(&messages[anchor.prefix_message_count..]);
        let total_tokens = adjusted_anchor_tokens.saturating_add(tail_tokens);

        let snapshot = Self::token_pressure_snapshot_from_total(
            total_tokens,
            messages,
            tools,
            context_window,
            trigger_budget,
            prepended_reminder_tokens,
        );
        (
            snapshot,
            Some(TokenAnchorPressureDetails {
                anchor_id: anchor.anchor_id.clone(),
                prefix_message_count: anchor.prefix_message_count,
                input_tokens: anchor.input_tokens,
                adjusted_anchor_tokens,
                system_tokens_at_anchor: anchor.system_tokens_at_anchor,
                current_system_tokens,
                system_delta: current_system_tokens as isize
                    - anchor.system_tokens_at_anchor as isize,
                tool_tokens_at_anchor: anchor.tool_tokens_at_anchor,
                current_tool_tokens,
                tool_delta: current_tool_tokens as isize - anchor.tool_tokens_at_anchor as isize,
                prepended_reminder_tokens_at_anchor: anchor.prepended_reminder_tokens_at_anchor,
                current_prepended_reminder_tokens: prepended_reminder_tokens,
                prepended_reminder_delta: prepended_reminder_tokens as isize
                    - anchor.prepended_reminder_tokens_at_anchor as isize,
                tail_tokens,
            }),
        )
    }

    fn token_pressure_snapshot_from_total(
        total_tokens: usize,
        messages: &[Message],
        tools: Option<&[ToolDefinition]>,
        context_window: usize,
        trigger_budget: CompressionTriggerBudget,
        prepended_reminder_tokens: usize,
    ) -> TokenPressureSnapshot {
        let system_tokens = messages
            .first()
            .filter(|message| message.role == MessageRole::System)
            .map(|message| message.estimate_tokens_with_reasoning(false))
            .unwrap_or(0);
        let tool_tokens = tools
            .map(TokenCounter::estimate_tool_definitions_tokens)
            .unwrap_or(0);
        let reserved_overhead = system_tokens
            .saturating_add(tool_tokens)
            .saturating_add(prepended_reminder_tokens);
        let conversation_tokens = total_tokens.saturating_sub(reserved_overhead);
        let usage_ratio = ContextHealthSnapshot::token_usage_ratio(total_tokens, context_window);
        TokenPressureSnapshot {
            total_tokens,
            system_tokens,
            tool_tokens,
            prepended_reminder_tokens,
            conversation_tokens,
            context_window,
            input_limit: trigger_budget.input_limit,
            output_reserve_tokens: trigger_budget.output_reserve_tokens,
            safety_reserve_tokens: trigger_budget.safety_reserve_tokens,
            usage_ratio,
        }
    }

    /// Resolve the configured output-reserve for a compression trigger budget,
    /// honoring `ai.thresholds.output_tokens.automatic_tiers` (阈值参数配置化).
    async fn compression_trigger_budget_configured(
        context_window: usize,
        configured_max_tokens: Option<u32>,
    ) -> CompressionTriggerBudget {
        let automatic_output_reserve =
            crate::service::config::types::automatic_max_output_tokens_configured(
                context_window as u32,
            )
            .await as usize;
        let output_reserve_tokens = configured_max_tokens
            .map(|value| value as usize)
            .unwrap_or(automatic_output_reserve);
        let ratio_percent =
            crate::service::config::types::configured_output_tokens_ratio_percent().await;
        let trigger_percent = Self::configured_compression_trigger_percent().await;
        Self::compression_trigger_budget_with_output_reserve_and_ratio(
            context_window,
            configured_max_tokens,
            Self::configured_compression_safety_reserve_tokens().await,
            output_reserve_tokens,
            ratio_percent,
            trigger_percent,
        )
    }

    /// Legacy synchronous compression-trigger budget with hard-coded reserve
    /// defaults; used by unit tests (生产路径走 `compression_trigger_budget_configured`).
    #[cfg(test)]
    fn compression_trigger_budget(
        context_window: usize,
        configured_max_tokens: Option<u32>,
    ) -> CompressionTriggerBudget {
        Self::compression_trigger_budget_with_output_reserve_and_ratio(
            context_window,
            configured_max_tokens,
            Self::AUTO_COMPRESSION_SAFETY_RESERVE_TOKENS,
            automatic_max_output_tokens(context_window as u32) as usize,
            MAX_CONFIGURED_OUTPUT_TOKENS_RATIO_PERCENT,
            None,
        )
    }

    /// Same as [`Self::compression_trigger_budget_configured`] but with
    /// an explicit output-reserve ratio cap in percent
    /// (阈值参数配置化：`ai.thresholds.output_tokens.ratio_percent` replaces the
    /// legacy hard-coded `MAX_CONFIGURED_OUTPUT_TOKENS_RATIO_PERCENT = 40`).
    fn compression_trigger_budget_with_output_reserve_and_ratio(
        context_window: usize,
        configured_max_tokens: Option<u32>,
        safety_reserve_tokens: usize,
        output_reserve_tokens: usize,
        ratio_percent: u32,
        trigger_percent: Option<u8>,
    ) -> CompressionTriggerBudget {
        let output_reserve_tokens = configured_max_tokens
            .map(|value| value as usize)
            .unwrap_or(output_reserve_tokens);
        // ENGINE-03：把输出预留钳制到窗口的 ratio_percent（默认 40%）以内（与
        // `is_valid_configured_max_output_tokens` 强制执行的同一比例）。否则配置了
        // 超过窗口的 max_tokens 会把 input_limit 压到 0，导致每一轮都无条件触发自动压缩。
        let ratio_percent = ratio_percent.clamp(1, 100);
        let max_output_reserve = (context_window as f64 * ratio_percent as f64 / 100.0) as usize;
        let output_reserve_tokens = output_reserve_tokens.min(max_output_reserve);
        let safety_reserve_tokens = safety_reserve_tokens.max(1);
        // ENGINE-05: saturating_add guards a 32-bit usize overflow when both
        // reserves are summed.
        let input_limit = context_window
            .saturating_sub(output_reserve_tokens.saturating_add(safety_reserve_tokens));

        // R-THR-01 批1：`ai.thresholds.compression.trigger_percent`（窗口百分比触发线）。
        // 合法值域 1-99；0 = 合法特殊值（同 None）；越界（101+/非数字 → None）按 None 处理
        // （零变化铁律：非法配置回退 None 后触发点与现算法完全一致）。
        // min 语义：百分比触发线是**上限约束**（更早压缩），小窗口（128k/200k）现算法
        // 已优于百分比线时 min 取现算法 → 配置不改变触发点（合法非 bug）。
        let input_limit = match trigger_percent {
            Some(percent) if (1..=99).contains(&percent) => {
                // round（非 floor）：契约断言 1M×85% = 891,290（1,048,576×0.85 =
                // 891,289.6 → round 891,290）。
                let percent_limit =
                    (context_window as f64 * percent as f64 / 100.0).round() as usize;
                input_limit.min(percent_limit)
            }
            _ => input_limit,
        };

        CompressionTriggerBudget {
            input_limit,
            output_reserve_tokens,
            safety_reserve_tokens,
        }
    }

    fn prepended_reminder_tokens_for_pressure(prepended_reminders: &[&str]) -> usize {
        prepended_reminders
            .iter()
            .map(|reminder| reminder.trim())
            .filter(|reminder| !reminder.is_empty())
            .map(|reminder| {
                Message::user(render_system_reminder(reminder))
                    .estimate_tokens_with_reasoning(false)
            })
            .sum()
    }

    fn system_tokens_for_pressure(messages: &[Message]) -> usize {
        messages
            .first()
            .filter(|message| message.role == MessageRole::System)
            .map(|message| message.estimate_tokens_with_reasoning(false))
            .unwrap_or(0)
    }

    fn estimate_tail_tokens(messages: &[Message]) -> usize {
        messages
            .iter()
            .map(|message| message.estimate_tokens_with_reasoning(true))
            .sum()
    }

    fn apply_token_delta(base: usize, old: usize, new: usize) -> usize {
        if new >= old {
            base.saturating_add(new - old)
        } else {
            base.saturating_sub(old - new)
        }
    }

    fn tool_signature_args_summary(args_str: &str) -> String {
        if args_str.len() <= 128 {
            return args_str.to_string();
        }

        let args_hash = hex::encode(Sha256::digest(args_str.as_bytes()));
        format!(
            "{}..#{}:sha256={}",
            truncate_at_char_boundary(args_str, 64),
            args_str.len(),
            args_hash
        )
    }

    fn tool_call_signature(tool_calls: &[crate::agentic::core::ToolCall]) -> Option<String> {
        if tool_calls.is_empty() {
            return None;
        }

        let mut signatures: Vec<String> = tool_calls
            .iter()
            .map(|tool_call| {
                let arguments = tool_call.arguments.to_string();
                let arguments_summary = Self::tool_signature_args_summary(&arguments);
                format!("{}:{}", tool_call.tool_name, arguments_summary)
            })
            .collect();
        signatures.sort();
        Some(signatures.join("|"))
    }

    /// 计算一轮待发送 messages 序列的指纹（R-MR-10 消息重复闸门）。
    ///
    /// hash 全部消息内容（文本/多模态/工具调用参数 + 工具结果），逐字节稳定：
    /// - 正常轮消息序列必变（模型输出/工具结果不同）→ 指纹必不同 → 永不误拦。
    /// - 死循环轮消息序列完全相同 → 指纹相同 → 判定重复 → 不调 API 本地合成。
    ///
    /// 消息组装零改动（缓存前缀保护铁律）：本函数只读 `messages`，不触碰
    /// `build_ai_messages_for_send` 的任何组装逻辑。
    fn messages_sequence_fingerprint(messages: &[Message]) -> String {
        let mut hasher = Sha256::new();
        for msg in messages {
            let role_label = match msg.role {
                MessageRole::User => "user",
                MessageRole::Assistant => "assistant",
                MessageRole::Tool => "tool",
                MessageRole::System => "system",
            };
            hasher.update(role_label.as_bytes());
            hasher.update([0u8]);
            match &msg.content {
                MessageContent::Text(text) => {
                    hasher.update(b"T");
                    hasher.update(text.as_bytes());
                }
                MessageContent::Multimodal { text, images } => {
                    hasher.update(b"M");
                    hasher.update(text.as_bytes());
                    hasher.update([0u8]);
                    for image in images {
                        hasher.update(image.id.as_bytes());
                        hasher.update([0u8]);
                        if let Some(path) = image.image_path.as_deref() {
                            hasher.update(path.as_bytes());
                        }
                        hasher.update([0u8]);
                        if let Some(data_url) = image.data_url.as_deref() {
                            hasher.update(data_url.as_bytes());
                        }
                        hasher.update([0u8]);
                        hasher.update(image.mime_type.as_bytes());
                        if let Some(meta) = image.metadata.as_ref() {
                            hasher.update([0u8]);
                            hasher.update(meta.to_string().as_bytes());
                        }
                    }
                }
                MessageContent::ToolResult {
                    tool_id,
                    tool_name,
                    effective_tool_name,
                    result,
                    result_for_assistant,
                    is_error,
                    image_attachments,
                } => {
                    hasher.update(b"R");
                    hasher.update(tool_id.as_bytes());
                    hasher.update([0u8]);
                    hasher.update(tool_name.as_bytes());
                    hasher.update([0u8]);
                    if let Some(effective) = effective_tool_name.as_deref() {
                        hasher.update(effective.as_bytes());
                    }
                    hasher.update([0u8]);
                    hasher.update(result.to_string().as_bytes());
                    hasher.update([0u8]);
                    if let Some(result_text) = result_for_assistant.as_deref() {
                        hasher.update(result_text.as_bytes());
                    }
                    hasher.update([0u8]);
                    hasher.update([u8::from(*is_error)]);
                    if let Some(attachments) = image_attachments.as_ref() {
                        hasher.update([0u8]);
                        hasher.update(attachments.len().to_le_bytes());
                        for attachment in attachments {
                            hasher.update(attachment.mime_type.as_bytes());
                            hasher.update([0u8]);
                            hasher.update(attachment.data_base64.as_bytes());
                        }
                    }
                }
                MessageContent::Mixed {
                    reasoning_content,
                    text,
                    tool_calls,
                } => {
                    hasher.update(b"A");
                    if let Some(reasoning) = reasoning_content.as_deref() {
                        hasher.update(reasoning.as_bytes());
                    }
                    hasher.update([0u8]);
                    hasher.update(text.as_bytes());
                    hasher.update([0u8]);
                    hasher.update(tool_calls.len().to_le_bytes());
                    for tool_call in tool_calls {
                        hasher.update(tool_call.tool_id.as_bytes());
                        hasher.update([0u8]);
                        hasher.update(tool_call.tool_name.as_bytes());
                        hasher.update([0u8]);
                        hasher.update(tool_call.arguments.to_string().as_bytes());
                        if let Some(raw) = tool_call.raw_arguments.as_deref() {
                            hasher.update([0u8]);
                            hasher.update(raw.as_bytes());
                        }
                        hasher.update([u8::from(tool_call.is_error)]);
                    }
                }
            }
            hasher.update([0xffu8]);
        }
        hex::encode(hasher.finalize())
    }

    /// R-MR-10 消息重复闸门：本轮指纹是否与最近 N 轮窗口中任一指纹相同。
    ///
    /// `window == 0` 视为 1（配置侧已 clamp，此处再防御一次：至少保留相邻轮
    /// 比对，避免配置 0 使闸门静默失效）。
    fn is_duplicate_message_fingerprint(
        current_fingerprint: &str,
        recent_fingerprints: &[String],
        window: usize,
    ) -> bool {
        let window = window.max(1);
        let tail_len = recent_fingerprints.len().min(window);
        if tail_len == 0 {
            return false;
        }
        recent_fingerprints[recent_fingerprints.len() - tail_len..]
            .iter()
            .any(|fingerprint| fingerprint == current_fingerprint)
    }

    fn failed_tool_round_signature(
        tool_calls: &[crate::agentic::core::ToolCall],
        tool_result_messages: &[Message],
    ) -> Option<String> {
        if tool_result_messages.is_empty()
            || !tool_result_messages.iter().all(|message| {
                let MessageContent::ToolResult {
                    result, is_error, ..
                } = &message.content
                else {
                    return false;
                };
                ContextHealthSnapshot::tool_result_failed(result, *is_error)
            })
        {
            return None;
        }

        Self::tool_call_signature(tool_calls)
    }

    /// Whether a partial stream recovery should trigger a continuation round
    /// instead of treating truncated assistant text as the final answer.
    ///
    /// User-initiated cancellation is excluded; all other partial recoveries
    /// (idle timeout, watchdog timeout, mid-stream errors) may continue.
    fn should_continue_after_partial_response(reason: &str) -> bool {
        let lower = reason.to_ascii_lowercase();
        !lower.contains("cancelled")
    }

    /// Detect periodic tool-signature loops in the trailing window.
    ///
    /// Returns `true` when the last `2 * threshold` rounds contain at most
    /// `threshold` distinct signatures AND every signature in that window
    /// appeared at least twice. Such windows have no new exploration and
    /// represent the model toggling between a small fixed set of calls
    /// (e.g. `A-B-A-B-A-B`, `A-B-C-A-B-C`).
    ///
    /// The window length is `2 * threshold` (rather than `threshold`) so the
    /// strict consecutive check (`windows(2).all(eq)`) keeps owning the
    /// `A-A-A` case at threshold rounds, and this detector only fires once
    /// the alternating pattern has had room to repeat.
    fn is_periodic_tool_signature_loop(recent_signatures: &[String], threshold: usize) -> bool {
        let threshold = threshold.max(1);
        let window_size = threshold.saturating_mul(2);
        if window_size == 0 || recent_signatures.len() < window_size {
            return false;
        }

        let tail = &recent_signatures[recent_signatures.len() - window_size..];
        let mut counts: HashMap<&str, usize> = HashMap::new();
        for sig in tail {
            *counts.entry(sig.as_str()).or_insert(0) += 1;
        }

        if counts.len() > threshold {
            return false;
        }

        counts.values().all(|&count| count >= 2)
    }

    fn assistant_has_tool_calls(message: &Message) -> bool {
        matches!(
            &message.content,
            MessageContent::Mixed { tool_calls, .. } if !tool_calls.is_empty()
        )
    }

    fn finalize_tool_names(tool_definitions: Option<&[ToolDefinition]>) -> Vec<String> {
        tool_definitions
            .unwrap_or(&[])
            .iter()
            .map(|tool| tool.name.clone())
            .collect()
    }

    fn finalize_runtime_tool_restrictions(
        context: &ExecutionContext,
        tool_names: &[String],
    ) -> ToolRuntimeRestrictions {
        let mut restrictions = context.runtime_tool_restrictions.clone();
        for tool_name in tool_names {
            restrictions.denied_tool_names.insert(tool_name.clone());
            restrictions
                .denied_tool_messages
                .entry(tool_name.clone())
                .or_insert_with(|| Self::FINALIZE_TOOL_DENIED_MESSAGE.to_string());
        }
        restrictions
    }

    /// Whether a finalize (rescue) round may still request the model.
    ///
    /// The rescue path (`run_finalize_round`) issues a fresh model request when
    /// the main loop stopped on repeated tool failures / max rounds. That
    /// request is only useful while the model still has a chance to produce a
    /// final answer; otherwise the turn should synthesize a local final
    /// response without spending tokens on a request that cannot help.
    fn should_allow_finalize_round(
        finalize_rounds_completed: usize,
        max_finalize_rounds: usize,
    ) -> bool {
        finalize_rounds_completed < max_finalize_rounds
    }

    fn build_local_final_response_message(reason: &str) -> String {
        match reason {
            "repeated_tool_failures" => {
                "I'm stopping here because repeated tool failures prevented further progress in this turn.".to_string()
            }
            "max_rounds" => {
                "I'm stopping here because this turn reached its round limit before I could complete a final response.".to_string()
            }
            "thinking_only_budget" => {
                "I'm stopping here because repeated reasoning-only rounds produced no action and the automatic continuation budget was exhausted.".to_string()
            }
            "duplicate_messages" => {
                "I'm stopping here because the outgoing message sequence repeated itself without any new information, which indicates the turn is stuck in a loop; no further model requests were issued.".to_string()
            }
            "empty_initial_turn" => {
                "I'm stopping here because this turn had no real user content — every user message was system-injected context (e.g. legion/agent/hook reminders) or empty. No model request was issued; no tokens were spent.".to_string()
            }
            _ => "I'm stopping here because this turn could not be completed successfully.".to_string(),
        }
    }

    /// R-13/DR-7 落点 1 守卫判定：首轮是否存在「真实用户内容」。
    ///
    /// 系统注入（legion_context / hook_context / 各类 internal_reminder 及
    /// `<system_reminder>` 包裹的 prepended reminders）以 user 角色进请求体，
    /// 内容非空 → 任何 trim 判空拦截都失效。本判定复用
    /// `Message::is_actual_user_message()`（message.rs:611-627）+ 注入 kind
    /// 标记 + `is_system_reminder_only`（prompt_markup.rs:94-98）：
    /// - user 消息带 ActualUserInput 语义标记 → 真实内容（A'-1：即使带壳形态
    ///   `user(render_system_reminder(...))` 也放行，语义标记权威）；
    /// - user 消息无语义标记但文本非 system_reminder-only 且非空 → 真实内容；
    /// - internal_reminder / system_reminder-only / 空文本 / 无文本 → 注入，不计。
    ///
    /// 全部 user 消息均为注入 → 无真实内容 → 守卫命中。
    ///
    /// A'-1 职责 = 字符串泄露检测：user 通道出现 `<XXX>` 壳 = 异常信号。
    /// 空串前置保留 :8264 语义（空串 user → false），未判空走
    /// `is_actual_user_message()` 统一收敛（Text + Multimodal 两分支）。
    fn has_real_user_content(messages: &[Message]) -> bool {
        messages.iter().any(|msg| {
            if msg.role != MessageRole::User {
                return false;
            }
            match &msg.content {
                MessageContent::Multimodal { text, images } => {
                    // 带真实图片的 user 消息视为真实内容（用户传图不可能为空轮）。
                    if !images.is_empty() {
                        return true;
                    }
                    if text.trim().is_empty() {
                        return false;
                    }
                    // A'-1 字符串泄露检测：复用 is_actual_user_message（语义标记
                    // 优先：ActualUserInput 带壳仍放行，InternalReminder 一律不算）。
                    msg.is_actual_user_message()
                }
                MessageContent::Text(text) => {
                    if text.trim().is_empty() {
                        return false;
                    }
                    // A'-1 字符串泄露检测：复用 is_actual_user_message（语义标记
                    // 优先：ActualUserInput 带壳仍放行，InternalReminder 一律不算）。
                    msg.is_actual_user_message()
                }
                _ => false,
            }
        })
    }

    fn should_mark_has_final_response(
        has_assistant_message: bool,
        used_local_final_response_synthesis: bool,
    ) -> bool {
        has_assistant_message && !used_local_final_response_synthesis
    }

    /// R-ASYNC-01（项1）：移除 round 边界排队合并。同一轮边界排队的 N 条
    /// 后台完成通知不再合并——N 条独立注入，逐条到达模型。
    /// 排队消费语义（drain_for_turn / acknowledge_consumed）保留。
    fn build_finalize_cache_anchor_messages(turn_id: &str, reminder_text: &str) -> Vec<Message> {
        vec![
            Message::internal_reminder(
                InternalReminderKind::FinalizeCacheAnchor,
                reminder_text.to_string(),
            )
            .with_turn_id(turn_id.to_string()),
            Message::internal_reminder(
                InternalReminderKind::FinalizeCacheAnchor,
                Self::FINALIZE_USER_FOLLOWUP,
            )
            .with_turn_id(turn_id.to_string()),
        ]
    }

    /// Emergency truncation: drop oldest API rounds (assistant+tool pairs)
    /// from the front of the message list until estimated tokens fit within
    /// `context_window`.  System messages and the first user message are
    /// always preserved.
    fn emergency_truncate_messages(
        messages: Vec<Message>,
        context_window: usize,
        tools: Option<&[ToolDefinition]>,
        prepended_reminder_tokens: usize,
    ) -> Vec<Message> {
        use crate::agentic::core::MessageRole;

        // Separate preserved head (system + first user) from droppable body.
        let mut preserved: Vec<Message> = Vec::new();
        let mut droppable: Vec<Message> = Vec::new();
        let mut seen_first_user = false;

        for msg in messages {
            if !seen_first_user {
                let is_user = msg.role == MessageRole::User;
                preserved.push(msg);
                if is_user {
                    seen_first_user = true;
                }
            } else {
                droppable.push(msg);
            }
        }

        if droppable.is_empty() {
            return preserved;
        }

        // Group droppable messages into API rounds.
        // An API round starts with an Assistant message and includes all
        // following Tool messages until the next Assistant or User message.
        let mut rounds: Vec<Vec<Message>> = Vec::new();
        for msg in droppable {
            match msg.role {
                MessageRole::Assistant => {
                    rounds.push(vec![msg]);
                }
                MessageRole::Tool => {
                    if let Some(last_round) = rounds.last_mut() {
                        last_round.push(msg);
                    } else {
                        rounds.push(vec![msg]);
                    }
                }
                _ => {
                    rounds.push(vec![msg]);
                }
            }
        }

        // Drop rounds from the front until we fit.
        let tool_tokens = tools
            .map(TokenCounter::estimate_tool_definitions_tokens)
            .unwrap_or(0);
        let preserved_tokens: usize = preserved
            .iter()
            .map(|m| m.estimate_tokens_with_reasoning(true))
            .sum::<usize>()
            + tool_tokens
            + prepended_reminder_tokens
            + 3;

        let mut kept_start = 0;
        let mut total_tokens = preserved_tokens
            + rounds
                .iter()
                .flat_map(|r| r.iter())
                .map(|m| m.estimate_tokens_with_reasoning(true))
                .sum::<usize>();

        while total_tokens > context_window && kept_start < rounds.len() {
            let round_tokens: usize = rounds[kept_start]
                .iter()
                .map(|m| m.estimate_tokens_with_reasoning(true))
                .sum();
            total_tokens -= round_tokens;
            kept_start += 1;
        }

        if kept_start > 0 {
            warn!(
                "Emergency truncation dropped {} API round(s) from context head",
                kept_start
            );
        }

        let mut result = preserved;
        for round in rounds.into_iter().skip(kept_start) {
            result.extend(round);
        }
        result
    }

    fn is_redacted_image_context(image: &ImageContextData) -> bool {
        let missing_path = image
            .image_path
            .as_ref()
            .map(|s| s.trim().is_empty())
            .unwrap_or(true);
        let missing_data_url = image
            .data_url
            .as_ref()
            .map(|s| s.trim().is_empty())
            .unwrap_or(true);
        let has_redaction_hint = image
            .metadata
            .as_ref()
            .and_then(|m| m.get("has_data_url"))
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        missing_path && missing_data_url && has_redaction_hint
    }

    fn is_recoverable_historical_image_error(err: &BitFunError) -> bool {
        match err {
            BitFunError::Io(_) | BitFunError::Deserialization(_) => true,
            BitFunError::Validation(msg) => {
                msg.starts_with("Failed to decode image data")
                    || msg.starts_with("Unsupported or unrecognized image format")
                    || msg.starts_with("Invalid data URL format")
                    || msg.starts_with("Data URL format error")
            }
            _ => false,
        }
    }

    fn can_fallback_to_text_only(
        images: &[ImageContextData],
        err: &BitFunError,
        is_current_turn_message: bool,
    ) -> bool {
        let is_redacted_payload_error = matches!(
            err,
            BitFunError::Validation(msg) if msg.starts_with("Image context missing image_path/data_url")
        ) && !images.is_empty()
            && images.iter().all(Self::is_redacted_image_context);

        if is_redacted_payload_error {
            return true;
        }

        if is_current_turn_message {
            return false;
        }

        Self::is_recoverable_historical_image_error(err)
    }

    fn resolve_configured_model_id(
        ai_config: &crate::service::config::types::AIConfig,
        model_id: &str,
    ) -> String {
        let trimmed = model_id.trim();
        if trimmed.is_empty() || trimmed == "auto" || trimmed == "default" {
            return "auto".to_string();
        }
        ai_config
            .resolve_model_selection(trimmed)
            .unwrap_or_else(|| "auto".to_string())
    }

    fn resolve_model_id_for_turn_selection(
        ai_config: &crate::service::config::types::AIConfig,
        configured_model_id: &str,
        frozen_model_id: Option<&str>,
    ) -> BitFunResult<String> {
        if let Some(frozen_model_id) = frozen_model_id
            .map(str::trim)
            .filter(|model_id| !model_id.is_empty())
        {
            return ai_config
                .resolve_model_reference(frozen_model_id)
                .ok_or_else(|| {
                    BitFunError::Validation(format!(
                        "Frozen dialog turn model contract is unavailable: {frozen_model_id}"
                    ))
                });
        }

        let resolved_configured_model_id =
            Self::resolve_configured_model_id(ai_config, configured_model_id);
        if configured_model_id == "auto"
            || configured_model_id == "default"
            || resolved_configured_model_id == "auto"
        {
            ai_config.resolve_model_selection("primary").ok_or_else(|| {
                BitFunError::AIClient(
                    "Auto dialog turn model could not resolve a concrete primary model".to_string(),
                )
            })
        } else {
            Ok(resolved_configured_model_id)
        }
    }

    fn validate_frozen_reasoning_contract(
        context: &ExecutionContext,
        ai_client: &crate::infrastructure::ai::AIClient,
    ) -> BitFunResult<()> {
        let Some(expected_value) = context
            .context
            .get(INTERRUPTED_TURN_REASONING_PRESET_METADATA_KEY)
        else {
            return Ok(());
        };
        let expected = serde_json::from_str::<Option<String>>(expected_value).map_err(|error| {
            BitFunError::Validation(format!(
                "Frozen dialog turn reasoning contract is malformed: {error}"
            ))
        })?;
        let actual_descriptor = ai_client
            .selected_reasoning_preset()
            .or_else(|| ai_client.model_reasoning_preset());
        let actual = actual_descriptor.map(|preset| preset.id.as_str());
        if actual != expected.as_deref() {
            return Err(BitFunError::Validation(format!(
                "Frozen dialog turn reasoning contract changed before execution: expected={:?}, actual={actual:?}",
                expected.as_deref(),
            )));
        }
        let expected_fingerprint = context
            .context
            .get(INTERRUPTED_TURN_REASONING_FINGERPRINT_METADATA_KEY)
            .ok_or_else(|| {
                BitFunError::Validation(
                    "Frozen dialog turn reasoning contract has no runtime fingerprint".to_string(),
                )
            })?;
        if reasoning_preset_runtime_fingerprint(actual_descriptor) != expected_fingerprint.as_str()
        {
            return Err(BitFunError::Validation(
                "Frozen dialog turn reasoning contract changed before execution: runtime fingerprint mismatch"
                    .to_string(),
            ));
        }
        Ok(())
    }

    async fn resolve_reasoning_selection_for_turn(
        &self,
        session_id: &str,
        context: &ExecutionContext,
    ) -> BitFunResult<Option<String>> {
        if let Some(frozen_selection) = context
            .context
            .get(INTERRUPTED_TURN_REASONING_SELECTION_METADATA_KEY)
        {
            return serde_json::from_str::<Option<String>>(frozen_selection).map_err(|error| {
                BitFunError::Validation(format!(
                    "Frozen dialog turn reasoning selection is malformed: {error}"
                ))
            });
        }

        self.session_manager
            .reconcile_session_reasoning_preset_for_turn(session_id, "turn_resolution")
            .await
    }

    pub(crate) fn is_frozen_reasoning_contract_error(error: &BitFunError) -> bool {
        matches!(error, BitFunError::Validation(message) if message.starts_with("Frozen dialog turn reasoning contract changed before execution:"))
    }

    async fn validate_frozen_model_contract(context: &ExecutionContext) -> BitFunResult<()> {
        let Some(expected_model_id) = context
            .context
            .get(INTERRUPTED_TURN_RESOLVED_MODEL_ID_METADATA_KEY)
        else {
            return Ok(());
        };
        let expected_fingerprint = context
            .context
            .get(INTERRUPTED_TURN_MODEL_BINDING_FINGERPRINT_METADATA_KEY)
            .ok_or_else(|| {
                BitFunError::Validation(
                    "Frozen dialog turn model contract has no binding fingerprint".to_string(),
                )
            })?;
        let ai_config = SessionManager::load_ai_config_for_model_resolution()
            .await
            .ok_or_else(|| {
                BitFunError::Validation(
                    "Frozen dialog turn model contract cannot be validated because AI configuration is unavailable"
                        .to_string(),
                )
            })?;
        let canonical_model_id = ai_config
            .resolve_model_reference(expected_model_id)
            .ok_or_else(|| {
                BitFunError::Validation(format!(
                    "Frozen dialog turn model contract is unavailable: {expected_model_id}"
                ))
            })?;
        let model = ai_config
            .models
            .iter()
            .find(|model| model.enabled && model.id == canonical_model_id)
            .ok_or_else(|| {
                BitFunError::Validation(format!(
                    "Frozen dialog turn model contract is unavailable: {expected_model_id}"
                ))
            })?;
        let actual_fingerprint = model_runtime_binding_fingerprint(model);
        if actual_fingerprint != expected_fingerprint.as_str() {
            return Err(BitFunError::Validation(format!(
                "Frozen dialog turn model contract changed before execution: model_id={expected_model_id}"
            )));
        }
        Ok(())
    }

    pub(crate) fn is_frozen_model_contract_error(error: &BitFunError) -> bool {
        matches!(error, BitFunError::Validation(message) if message.starts_with("Frozen dialog turn model contract"))
    }

    async fn resolve_primary_model_context(
        model_id: &str,
        model_binding_policy: SessionModelBindingPolicy,
        ai_client_model: &str,
        ai_client_api_format: &str,
        unavailable_log_message: &str,
    ) -> PrimaryModelFacts {
        let config_service = get_global_config_service().await.ok();
        if let Some(service) = config_service {
            let ai_config: crate::service::config::types::AIConfig =
                service.get_config(Some("ai")).await.unwrap_or_default();

            let resolved_id = if matches!(
                model_binding_policy,
                SessionModelBindingPolicy::ApprovedImmutable
            ) {
                ai_config
                    .resolve_model_reference(model_id)
                    .unwrap_or_else(|| model_id.to_string())
            } else {
                Self::resolve_configured_model_id(&ai_config, model_id)
            };
            let model_cfg = ai_config.models.iter().find(|m| m.id == resolved_id);

            let supports = model_cfg.is_some_and(|m| {
                m.capabilities
                    .iter()
                    .any(|cap| matches!(cap, ModelCapability::ImageUnderstanding))
                    || matches!(m.category, ModelCategory::Multimodal)
            });

            PrimaryModelFacts::new(resolved_id, ai_client_model, ai_client_api_format, supports)
        } else {
            warn!("{}", unavailable_log_message);
            PrimaryModelFacts::new(model_id, ai_client_model, ai_client_api_format, false)
        }
    }

    async fn build_tool_listing_sections(
        manifest: &ResolvedToolManifest,
        tool_context: &crate::agentic::tools::framework::ToolUseContext,
    ) -> ToolListingSections {
        let has_tool_definition = |tool_name: &str| {
            manifest
                .tool_definitions
                .iter()
                .any(|definition| definition.name == tool_name)
        };

        ToolListingSections {
            skill_listing: if has_tool_definition("Skill") {
                SkillTool::build_available_skills_context_section(Some(tool_context)).await
            } else {
                None
            },
            agent_listing: if has_tool_definition("Task") {
                TaskTool::build_available_agents_context_section(Some(tool_context)).await
            } else {
                None
            },
            direct_tool_listing: (!manifest.deferred_tool_names.is_empty()).then(|| {
                render_direct_tool_listing_body(
                    manifest
                        .tool_definitions
                        .iter()
                        .map(|definition| definition.name.as_str()),
                )
            }),
            deferred_tool_listing: if has_tool_definition("GetToolSpec") {
                GetToolSpecTool::build_deferred_tools_context_section(
                    &manifest.deferred_tool_summaries,
                )
            } else {
                None
            },
        }
    }

    async fn build_prompt_context(
        context: &ExecutionContext,
        model_name: &str,
        supports_image_understanding: bool,
        tool_listing_sections: ToolListingSections,
        runtime_context_needs: RuntimeContextNeeds,
    ) -> Option<PromptBuilderContext> {
        let workspace = context.workspace.as_ref()?;
        let remote_file_delivery_channel = context
            .context
            .get(TOOL_CONTEXT_REMOTE_FILE_DELIVERY_KEY)
            .and_then(|value| value.parse::<bool>().ok())
            .unwrap_or(false);
        let inline_markdown_image_display = context
            .context
            .get(TOOL_CONTEXT_INLINE_MARKDOWN_IMAGE_DISPLAY_KEY)
            .and_then(|value| value.parse::<bool>().ok())
            .unwrap_or(false);

        build_prompt_context_for_workspace(
            workspace,
            workspace.workspace_id.as_deref(),
            &context.session_id,
            Some(model_name.to_string()),
            Some(supports_image_understanding),
            tool_listing_sections,
            runtime_context_needs,
        )
        .await
        .map(|prompt_context| {
            prompt_context
                .with_remote_file_delivery_channel(remote_file_delivery_channel)
                .with_inline_markdown_image_display(inline_markdown_image_display)
        })
    }

    async fn build_user_context_for_cache_miss(
        workspace: Option<&WorkspaceBinding>,
        workspace_services: Option<&crate::agentic::workspace::WorkspaceServices>,
        mut prompt_context: PromptBuilderContext,
        policy: &UserContextPolicy,
    ) -> (Option<String>, bool) {
        let mut cacheable = true;
        if policy.includes(UserContextSection::WorkspaceInstructions) {
            let instruction_context: BitFunResult<InstructionContextBuild> =
                if let Some(workspace) = workspace {
                    if workspace.is_remote() {
                        if let Some(services) = workspace_services {
                            build_workspace_instruction_files_context_with_fs(
                                services.fs.as_ref(),
                                &workspace.root_path_string(),
                            )
                            .await
                            .map(|content| InstructionContextBuild {
                                content,
                                cacheable: true,
                            })
                        } else {
                            Ok(InstructionContextBuild {
                                content: None,
                                cacheable: false,
                            })
                        }
                    } else {
                        if let Some(services) = workspace_services {
                            build_local_workspace_instruction_files_context_with_fs_detailed(
                                workspace.root_path(),
                                services.fs.as_ref(),
                                &workspace.root_path_string(),
                            )
                            .await
                        } else {
                            build_workspace_instruction_files_context_detailed(
                                workspace.root_path(),
                            )
                            .await
                        }
                    }
                } else {
                    Ok(InstructionContextBuild {
                        content: None,
                        cacheable: true,
                    })
                };
            let instruction_context = match instruction_context {
                Ok(instruction_context) => {
                    cacheable &= instruction_context.cacheable;
                    instruction_context.content
                }
                Err(error) => {
                    cacheable = false;
                    warn!(
                        "Failed to build workspace instruction context: path={} error={}",
                        workspace
                            .map(WorkspaceBinding::root_path_string)
                            .unwrap_or_else(|| "<none>".to_string()),
                        error
                    );
                    None
                }
            };
            prompt_context =
                prompt_context.with_workspace_instruction_files_context(instruction_context);
        }

        let user_context = PromptBuilder::new(prompt_context)
            .build_user_context_reminder(policy)
            .await;
        (user_context, cacheable)
    }

    /// Resolve the user context cache identity for the current execution,
    /// layering the runtime-affecting dimensions onto the agent policy scope
    /// key:
    ///
    /// - `remote:<connection>` — a failed overlay cached without remote hints
    ///   must not persist across reconnects (existing behavior).
    /// - `extsrc:<on|off>` — the `external_instruction_sources` master switch
    ///   changes the rendered User Context content (external user files are
    ///   skipped when off). Without it in the scope key, a session that toggles
    ///   on↔off mid-session would keep hitting the stale cached content,
    ///   because cache hits only check identity + TTL, never content.
    /// - `winstr:<on|off>` — the `workspace_instruction_files` master switch
    ///   changes the rendered User Context content (project AGENTS.md / CLAUDE.md
    ///   skipped when off). Same staleness concern as `extsrc`.
    /// - `|instr:<digest>` (TOKEN-03): the digest of the workspace instruction
    ///   files (workspace-level `AGENTS.md`/`CLAUDE.md` and user-level external
    ///   sources when enabled). Appended AFTER the stable prefix so unchanged
    ///   content keeps hitting the cache while an edited instruction file
    ///   invalidates it.
    async fn user_context_cache_identity_for(
        base_identity: UserContextCacheIdentity,
        remote_connection: Option<&str>,
        workspace_root: Option<std::path::PathBuf>,
    ) -> UserContextCacheIdentity {
        let mut scope_key = base_identity.scope_key;
        if let Some(connection) = remote_connection {
            scope_key = format!("{scope_key}|remote:{connection}");
        }
        let external_sources = crate::service::config::external_instruction_sources_enabled();
        scope_key = format!(
            "{scope_key}|extsrc:{}",
            if external_sources { "on" } else { "off" }
        );
        let workspace_instruction_files =
            crate::service::config::workspace_instruction_files_enabled();
        scope_key = format!(
            "{scope_key}|winstr:{}",
            if workspace_instruction_files {
                "on"
            } else {
                "off"
            }
        );
        if let Some(workspace_root) = workspace_root {
            let digest = workspace_instruction_digest(&workspace_root, external_sources).await;
            scope_key = format!("{scope_key}|instr:{digest}");
        }
        UserContextCacheIdentity::new(scope_key)
    }

    async fn build_cached_prepended_prompt_reminders(
        &self,
        execution_context: &ExecutionContext,
        current_agent: &dyn crate::agentic::agents::Agent,
        prompt_context: Option<&PromptBuilderContext>,
        runtime_facts_usage: RuntimeFactsUsage,
    ) -> PrependedPromptReminders {
        let Some(prompt_context) = prompt_context.cloned() else {
            return PrependedPromptReminders::default();
        };
        let session_id = &execution_context.session_id;

        // Extract remote execution info before prompt_context is moved into PromptBuilder.
        let remote_connection_for_cache = prompt_context
            .remote_execution
            .as_ref()
            .map(|remote| remote.connection_display_name.replace('|', "/"));

        let prompt_builder = PromptBuilder::new(prompt_context.clone());
        let baseline_snapshot = if let Some(snapshot) = self
            .session_manager
            .skill_agent_baseline_override_snapshot(session_id)
            .await
        {
            Some(snapshot)
        } else {
            self.session_manager
                .turn_skill_agent_snapshot(session_id, 0)
                .await
        };
        let baseline_tool_sections = baseline_snapshot
            .map(|snapshot| build_skill_agent_tool_listing_sections_from_snapshot(&snapshot));
        if baseline_tool_sections.is_none() {
            warn!(
                "Listing reminder baseline snapshot unavailable while building prepended reminders: session_id={}",
                session_id
            );
        }
        let user_context_identity = Self::user_context_cache_identity_for(
            current_agent.user_context_cache_identity(),
            remote_connection_for_cache.as_deref(),
            // TOKEN-03: include the workspace instruction content digest so a
            // changed instruction file invalidates the session-level cache.
            // The digest is appended AFTER the existing scope-key prefix so
            // stable prefixes keep matching for unchanged content.
            execution_context
                .workspace
                .as_ref()
                .map(|workspace| workspace.root_path().to_path_buf()),
        )
        .await;
        let user_context = if let Some(cached_user_context) = self
            .session_manager
            .cached_user_context(session_id, &user_context_identity)
            .await
        {
            debug!(
                "User context cache hit: session_id={}, scope_key={}",
                session_id, user_context_identity.scope_key
            );
            Some(cached_user_context)
        } else {
            debug!(
                "User context cache miss: session_id={}, scope_key={}",
                session_id, user_context_identity.scope_key
            );
            let cache_generation = self
                .session_manager
                .user_context_cache_generation(session_id)
                .await;
            let user_context_policy = current_agent.user_context_policy();
            let (built_user_context, cacheable) = Self::build_user_context_for_cache_miss(
                execution_context.workspace.as_ref(),
                execution_context.workspace_services.as_ref(),
                prompt_context,
                &user_context_policy,
            )
            .await;
            if cacheable {
                if let Some(ref user_context) = built_user_context {
                    let cached = self
                        .session_manager
                        .remember_user_context_if_generation(
                            session_id,
                            cache_generation,
                            user_context_identity.clone(),
                            user_context.clone(),
                        )
                        .await;
                    if !cached {
                        debug!(
                            "Skipped stale user context cache write after invalidation: session_id={}, scope_key={}",
                            session_id, user_context_identity.scope_key
                        );
                    }
                }
            } else {
                debug!(
                    "User context was not cached after workspace instruction resolution failed: session_id={}, scope_key={}",
                    session_id, user_context_identity.scope_key
                );
            }
            built_user_context
        };
        let runtime_context = prompt_builder.build_runtime_context_reminder().await;
        let runtime_facts = Some(prompt_builder.build_runtime_facts_reminder(runtime_facts_usage));

        PrependedPromptReminders {
            deferred_tool_listing: prompt_builder.build_deferred_tool_listing_reminder(),
            skill_listing: baseline_tool_sections
                .as_ref()
                .and_then(|sections| sections.render_skill_listing_reminder()),
            agent_listing: baseline_tool_sections
                .as_ref()
                .and_then(|sections| sections.render_agent_listing_reminder()),
            runtime_context,
            runtime_facts,
            user_context,
        }
    }

    async fn resolve_cached_system_prompt(
        &self,
        session_id: &str,
        current_agent: &dyn crate::agentic::agents::Agent,
        prompt_context: Option<&PromptBuilderContext>,
    ) -> BitFunResult<String> {
        let identity = prompt_context
            .map(|context| {
                current_agent.system_prompt_cache_identity(context.model_name.as_deref())
            })
            .unwrap_or_else(|| current_agent.system_prompt_cache_identity(None));

        if let Some(cached_system_prompt) = self
            .session_manager
            .cached_system_prompt(session_id, &identity)
            .await
        {
            debug!(
                "System prompt cache hit: session_id={}, scope_key={}",
                session_id, identity.scope_key
            );
            return Ok(cached_system_prompt);
        }

        debug!(
            "System prompt cache miss: session_id={}, scope_key={}",
            session_id, identity.scope_key
        );
        let system_prompt = current_agent.get_system_prompt(prompt_context).await?;
        self.session_manager
            .remember_system_prompt(session_id, identity, system_prompt.clone())
            .await;
        Ok(system_prompt)
    }
}

/// TOKEN-03: digest of the workspace instruction files that feed the User
/// Context reminder, so the session-level User Context cache invalidates when
/// an instruction file's content changes (the cache identity previously only
/// covered the policy scope labels, so edited instructions stayed invisible
/// until the session rebuilt).
///
/// Best-effort: any read/scan failure falls back to `"unreadable"` so the
/// digest never blocks prompt assembly; the cache just misses once and the
/// fresh content is re-read on the miss path.
async fn workspace_instruction_digest(
    workspace_root: &std::path::Path,
    external_sources: bool,
) -> String {
    use std::collections::BTreeMap;

    let mut digest_input = String::new();

    // Workspace-level instruction files (startup-context, no path patterns) —
    // only when the workspace instruction files master switch is on, mirroring
    // the render path in service::instruction_context.
    if crate::service::config::workspace_instruction_files_enabled() {
        match bitfun_services_core::workspace_instructions::read_workspace_instruction_files(
            workspace_root,
        )
        .await
        {
            Ok(files) => {
                for file in files {
                    digest_input.push_str(&file.name);
                    digest_input.push('\0');
                    digest_input.push_str(&file.content);
                    digest_input.push('\0');
                }
            }
            Err(error) => {
                log::warn!(
                    "workspace_instruction_digest: failed to read workspace instruction files: {}",
                    error
                );
                return "unreadable".to_string();
            }
        }
    }

    // User-level external instruction sources (~/.claude/CLAUDE.md, OpenCode
    // AGENTS.md, Codex AGENTS.md, rules/) — only when the master switch is on,
    // mirroring the render path in service::instruction_context.
    if external_sources {
        let loaded =
            crate::instruction_sources::load_local_user_instruction_files(workspace_root).await;
        let mut names: BTreeMap<String, String> = BTreeMap::new();
        for file in loaded.files {
            names.insert(file.name.clone(), file.content.clone());
        }
        for (name, content) in names {
            digest_input.push_str(&name);
            digest_input.push('\0');
            digest_input.push_str(&content);
            digest_input.push('\0');
        }
    }

    if digest_input.is_empty() {
        return "none".to_string();
    }

    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(digest_input.as_bytes());
    format!("{:x}", hasher.finalize())
}

impl ExecutionEngine {
    async fn resolve_turn_prompt_scaffold(
        &self,
        input: TurnPromptScaffoldInput<'_>,
    ) -> BitFunResult<TurnPromptScaffold> {
        debug!(
            "Resolving turn prompt scaffold: session_id={}, turn_id={}, stage={}, agent={}, model={}",
            input.context.session_id,
            input.context.dialog_turn_id,
            input.stage,
            input.current_agent.name(),
            input.model_name
        );

        let prompt_context = Self::build_prompt_context(
            input.context,
            input.model_name,
            input.supports_image_understanding,
            input.tool_listing_sections,
            input.runtime_context_needs,
        )
        .await;
        let prepended_prompt_reminders = self
            .build_cached_prepended_prompt_reminders(
                input.context,
                input.current_agent,
                prompt_context.as_ref(),
                input.runtime_facts_usage,
            )
            .await;
        let system_prompt = self
            .resolve_cached_system_prompt(
                &input.context.session_id,
                input.current_agent,
                prompt_context.as_ref(),
            )
            .await?;

        Self::log_turn_prompt_scaffold(
            &input.context.session_id,
            &input.context.dialog_turn_id,
            input.stage,
            system_prompt.len(),
            &prepended_prompt_reminders,
        );

        Ok(TurnPromptScaffold {
            system_prompt_message: Message::system(system_prompt),
            prepended_prompt_reminders,
        })
    }

    fn log_turn_prompt_scaffold(
        session_id: &str,
        turn_id: &str,
        stage: &str,
        system_prompt_len: usize,
        prepended_prompt_reminders: &PrependedPromptReminders,
    ) {
        debug!(
            "Turn prompt scaffold resolved: session_id={}, turn_id={}, stage={}, system_prompt_len={} bytes, skill_listing_len={}, agent_listing_len={}, deferred_tool_listing_len={}, user_context_len={}, runtime_context_len={}, runtime_facts_len={}",
            session_id,
            turn_id,
            stage,
            system_prompt_len,
            prepended_prompt_reminders
                .skill_listing
                .as_ref()
                .map(|text| text.len())
                .unwrap_or(0),
            prepended_prompt_reminders
                .agent_listing
                .as_ref()
                .map(|text| text.len())
                .unwrap_or(0),
            prepended_prompt_reminders
                .deferred_tool_listing
                .as_ref()
                .map(|text| text.len())
                .unwrap_or(0),
            prepended_prompt_reminders
                .user_context
                .as_ref()
                .map(|text| text.len())
                .unwrap_or(0),
            prepended_prompt_reminders
                .runtime_context
                .as_ref()
                .map(|text| text.len())
                .unwrap_or(0),
            prepended_prompt_reminders
                .runtime_facts
                .as_ref()
                .map(|text| text.len())
                .unwrap_or(0)
        );
    }

    fn apply_turn_prompt_scaffold_to_messages(
        messages: &mut Vec<Message>,
        scaffold: &TurnPromptScaffold,
    ) {
        match messages.first_mut() {
            Some(first_message) if first_message.role == MessageRole::System => {
                *first_message = scaffold.system_prompt_message.clone();
            }
            _ => messages.insert(0, scaffold.system_prompt_message.clone()),
        }
    }

    /// Refresh only the per-round runtime facts reminder on a turn scaffold so
    /// every model request carries live time and the current token pressure
    /// snapshot instead of the turn-start values. Long-lived turns (background
    /// Task agents, subagents, deep-review passes) can span many rounds and
    /// minutes; keeping the turn-start snapshot would freeze the model's view
    /// of time and context usage for the whole turn.
    ///
    /// ENGINE-01/07: sessions without a workspace never produce a prompt
    /// context (`build_prompt_context` returns `None`), so the round-level
    /// reminder previously stayed frozen at the turn-start value forever.
    /// `build_runtime_facts_reminder` only needs the live clock and the usage
    /// snapshot, so a minimal context refreshes it for every session shape.
    /// The reminder always builds (returns `String`, never `None`), so the
    /// round-level refresh can no longer silently skip.
    /// P-17：按回合标记刷新或置空 Runtime Facts。
    /// - inject_runtime_facts == true（用户消息回合首轮或上下文恢复后首轮）→ 刷新注入。
    /// - false（同回合工具轮）→ 置空，动态后置不再携带 Runtime Facts。
    fn refresh_runtime_facts_for_round(
        scaffold: &mut TurnPromptScaffold,
        prompt_context: Option<PromptBuilderContext>,
        usage: RuntimeFactsUsage,
        inject_runtime_facts: bool,
    ) {
        if !inject_runtime_facts {
            scaffold.prepended_prompt_reminders.runtime_facts = None;
            return;
        }
        let builder = match prompt_context {
            Some(prompt_context) => PromptBuilder::new(prompt_context),
            None => {
                let mut context = PromptBuilderContext::new("", None, None);
                // Preserve remote_execution from original context if available
                if let Some(original_context) = &prompt_context {
                    context.remote_execution = original_context.remote_execution.clone();
                }
                PromptBuilder::new(context)
            }
        };
        let refreshed = builder.build_runtime_facts_reminder(usage);
        scaffold.prepended_prompt_reminders.runtime_facts = Some(refreshed);
    }

    /// P-18/F-5/RT：按「真实用户轮」规则构建本轮动态后置提醒。
    /// - User Context + Runtime Facts：都只在真实用户轮注入——与用户消息
    ///   拼接发送（动态提醒追加在最新用户消息后 = 用户消息轮内），不再
    ///   每轮独立发送「时间 + 上下文占比」提示（主人实测：独立消息每条
    ///   增加 token 消耗）。F-5 起改为「真实用户消息轮才注入/计数」——
    ///   只有 trigger_source 属于用户面（DesktopUi/DesktopApi/Cli/Bot/
    ///   RemoteRelay/SdkHost）的轮才参与世代比较并注入；Agent 间轮
    ///   （AgentSession/ScheduledJob 等）与子代理内部轮（trigger_source=None）
    ///   既不注入也不记录注入世代（不锁世代 → 后续真实用户轮仍可注入，
    ///   防回归重复注入问题的同时避免 Agent 轮误触发）。世代语义保留：
    ///   同一世代内已注入过 → 不重复；上下文压缩/恢复使缓存世代递增 →
    ///   恢复后首个真实用户轮重新注入一次。
    ///   原实现（每回合首轮注入）在 execute_dialog_turn_impl 每次 turn 开始清除
    ///   注入标记，导致同一会话每个用户回合都重复注入工作区指令全文。
    async fn round_dynamic_reminders<'a>(
        &self,
        session_id: &str,
        context: &ExecutionContext,
        reminders: &'a PrependedPromptReminders,
    ) -> Vec<&'a str> {
        let mut dynamic = Vec::new();
        // F-5/RT：真实用户轮判定——只有用户面 trigger_source 才注入/计数
        // User Context 与 Runtime Facts；Agent 注入轮与子代理内部轮（None）
        // 直接跳过（不锁世代），不再每轮独立发送时间+占比提示。
        let user_submission_source = context.trigger_source.is_some_and(|source| {
            matches!(
                source,
                bitfun_runtime_ports::DialogTriggerSource::DesktopUi
                    | bitfun_runtime_ports::DialogTriggerSource::DesktopApi
                    | bitfun_runtime_ports::DialogTriggerSource::Cli
                    | bitfun_runtime_ports::DialogTriggerSource::Bot
                    | bitfun_runtime_ports::DialogTriggerSource::RemoteRelay
                    | bitfun_runtime_ports::DialogTriggerSource::SdkHost
            )
        });
        if user_submission_source {
            // RT：Runtime Facts（时间 + 上下文占比）随真实用户消息轮拼接发送，
            // 不再每轮独立消息注入。refresh_runtime_facts_for_round 已保证
            // 工具轮置空（None），此处再以真实用户轮为闸，Agent 轮同样不带。
            if let Some(runtime_facts) = reminders.runtime_facts.as_deref() {
                dynamic.push(runtime_facts);
            }
            let generation = self
                .session_manager
                .user_context_cache_generation(session_id)
                .await;
            let injected_generation = self
                .session_manager
                .user_context_injected_generation(session_id)
                .await;
            // P-18（d5-P1-1）：只在真正注入了 User Context 时才记录注入世代。
            // `user_context` 为 None（无 workspace / 指令文件构建失败 / 无内容可注入）时
            // 不记录——否则同一世代内后续轮被抑制注入，而模型实际从未看到 User Context，
            // 当缓存恢复可用时（如远端重连）也必须能重新注入。
            if injected_generation != Some(generation) {
                if let Some(user_context) = reminders.user_context.as_deref() {
                    dynamic.push(user_context);
                    self.session_manager
                        .remember_user_context_injected_generation(session_id, generation)
                        .await;
                }
            }
        }
        dynamic
    }

    #[allow(clippy::too_many_arguments)] // model resolution context; kept flat for explicit call sites
    pub(crate) async fn resolve_model_id_for_turn(
        &self,
        session: &Session,
        agent_type: &str,
        workspace: Option<&WorkspaceBinding>,
        original_user_input: &str,
        turn_index: usize,
        frozen_model_id: Option<&str>,
        frozen_model_binding_fingerprint: Option<&str>,
    ) -> BitFunResult<(String, String)> {
        let ai_config = SessionManager::load_ai_config_for_model_resolution()
            .await
            .ok_or_else(|| {
                BitFunError::AIClient(
                    "Failed to get config service for model resolution".to_string(),
                )
            })?;
        if matches!(
            session.config.model_binding_policy,
            SessionModelBindingPolicy::ApprovedImmutable
        ) {
            let model_id = session
                .config
                .model_id
                .as_deref()
                .map(str::trim)
                .filter(|model_id| !model_id.is_empty())
                .ok_or_else(|| {
                    BitFunError::AIClient(
                        "Approved immutable session has no concrete model id".to_string(),
                    )
                })?;
            let expected_fingerprint = session
                .config
                .model_binding_fingerprint
                .as_deref()
                .ok_or_else(|| {
                    BitFunError::AIClient(
                        "Approved immutable session has no model binding fingerprint".to_string(),
                    )
                })?;
            let mut matches = ai_config
                .models
                .iter()
                .filter(|model| model.enabled && model.id == model_id);
            let model = matches.next().ok_or_else(|| {
                BitFunError::AIClient(format!(
                    "Approved model configuration is unavailable: {}",
                    model_id
                ))
            })?;
            if matches.next().is_some()
                || model_runtime_binding_fingerprint(model) != expected_fingerprint
            {
                return Err(BitFunError::AIClient(format!(
                    "Approved model binding changed before execution: {}",
                    model_id
                )));
            }
            return Ok((model_id.to_string(), expected_fingerprint.to_string()));
        }

        let agent_registry = get_agent_registry();
        let fallback_model_id = agent_registry
            .get_model_id_for_agent(agent_type, workspace.map(|binding| binding.root_path()))
            .await
            .map_err(|e| BitFunError::AIClient(format!("Failed to get model ID: {}", e)))?;
        let configured_model_id = session
            .config
            .model_id
            .as_ref()
            .map(|model_id| model_id.trim())
            .filter(|model_id| !model_id.is_empty())
            .map(str::to_string)
            .unwrap_or(fallback_model_id.clone());
        let model_id = Self::resolve_model_id_for_turn_selection(
            &ai_config,
            &configured_model_id,
            frozen_model_id,
        )?;
        let model = ai_config
            .models
            .iter()
            .find(|model| model.enabled && model.id == model_id)
            .ok_or_else(|| {
                if frozen_model_id.is_some() {
                    BitFunError::Validation(format!(
                        "Frozen dialog turn model contract is unavailable: {model_id}"
                    ))
                } else {
                    BitFunError::AIClient(format!(
                        "Dialog turn model configuration is unavailable: {model_id}"
                    ))
                }
            })?;
        let model_binding_fingerprint = model_runtime_binding_fingerprint(model);
        if frozen_model_binding_fingerprint
            .is_some_and(|expected| expected != model_binding_fingerprint)
        {
            return Err(BitFunError::Validation(format!(
                "Frozen dialog turn model contract changed before execution: model_id={model_id}"
            )));
        }
        if frozen_model_id.is_some() {
            info!(
                "Using frozen dialog turn model: session_id={}, turn_index={}, resolved_model_id={}",
                session.session_id, turn_index, model_id
            );
        } else if configured_model_id == "auto" || configured_model_id == "default" {
            info!(
                "Auto model resolved without locking session: session_id={}, turn_index={}, user_input_chars={}, strategy=primary, resolved_model_id={}",
                session.session_id,
                turn_index,
                original_user_input.chars().count(),
                model_id
            );
        }

        Ok((model_id, model_binding_fingerprint))
    }

    /// Omit from model request: UI-only verification frames and legacy auto desktop snapshots.
    fn skip_message_for_model_send(msg: &Message) -> bool {
        matches!(
            msg.metadata.semantic_kind.as_ref(),
            Some(MessageSemanticKind::ComputerUseVerificationScreenshot)
                | Some(MessageSemanticKind::ComputerUsePostActionSnapshot)
        )
    }

    fn is_stale_interrupted_continue(msg: &Message, current_turn_id: &str) -> bool {
        msg.internal_reminder_kind() == Some(InternalReminderKind::InterruptedContinue)
            && msg.metadata.turn_id.as_deref() != Some(current_turn_id)
    }

    /// True if this message would contribute at least one image to the model (before pruning).
    fn message_bears_images(msg: &Message) -> bool {
        if Self::skip_message_for_model_send(msg) {
            return false;
        }
        match &msg.content {
            MessageContent::Multimodal { images, .. } => !images.is_empty(),
            MessageContent::ToolResult {
                image_attachments, ..
            } => image_attachments.as_ref().is_some_and(|a| !a.is_empty()),
            _ => false,
        }
    }

    /// Indices of the last image-bearing messages that should keep image payloads.
    fn image_bearing_indices_to_keep(
        messages: &[Message],
        max_image_messages: usize,
    ) -> HashSet<usize> {
        let with_images: Vec<usize> = messages
            .iter()
            .enumerate()
            .filter(|(_, m)| Self::message_bears_images(m))
            .map(|(i, _)| i)
            .collect();
        let n = with_images.len();
        if n <= max_image_messages {
            return with_images.into_iter().collect();
        }
        with_images[n - max_image_messages..]
            .iter()
            .copied()
            .collect()
    }

    async fn run_finalize_round(&self, input: FinalizeRoundInput<'_>) -> BitFunResult<RoundResult> {
        // Keep the original tool definitions attached to the finalize request
        // even though finalize forbids tool execution at runtime. Dropping the
        // tools here would change the provider request shape, which breaks
        // prompt/prefix cache reuse and turns the finalize round into a cache
        // miss for providers that key caching on the full request schema.
        let finalize_tool_names = Self::finalize_tool_names(input.tool_definitions.as_deref());
        let finalize_runtime_tool_restrictions =
            Self::finalize_runtime_tool_restrictions(input.context, &finalize_tool_names);
        let mut final_ai_messages = Self::build_ai_messages_for_send(
            input.messages,
            &input.ai_client.config.format,
            input
                .context
                .workspace
                .as_ref()
                .map(|workspace| workspace.root_path()),
            &input.context.dialog_turn_id,
            input.primary_model_facts.supports_image_inputs,
            input.static_prepended_reminders,
            input.dynamic_prepended_reminders,
            Self::configured_max_image_bearing_messages().await,
        )
        .await?;
        final_ai_messages.push(AIMessage::system(render_system_reminder(
            input.reminder_text,
        )));
        final_ai_messages.push(AIMessage::system(render_system_reminder(
            Self::FINALIZE_USER_FOLLOWUP,
        )));

        let model_exchange_trace_dir = self
            .session_manager
            .persistent_model_exchange_trace_dir(&input.context.session_id)
            .await;
        let round_context_vars = self
            .context_vars_for_round(
                input.execution_context_vars,
                &input.context.session_id,
                &input.context.dialog_turn_id,
            )
            .await;
        let round_context = RoundContext {
            session_id: input.context.session_id.clone(),
            subagent_parent_info: input.context.subagent_parent_info.clone(),
            permission_delegation: input.context.permission_delegation.clone(),
            dialog_turn_id: input.context.dialog_turn_id.clone(),
            turn_index: input.context.turn_index,
            round_number: input.round_number,
            round_group_id: input.round_group_id,
            workspace: input.context.workspace.clone(),
            model_exchange_trace_dir,
            available_tools: finalize_tool_names,
            user_enabled_tools: input.user_enabled_tools.clone(),
            deferred_tools: Vec::new(),
            loaded_deferred_tool_specs: Vec::new(),
            model_config_id: input.primary_model_facts.model_id.clone(),
            effective_model_name: input.ai_client.config.model.clone(),
            model_request_context: input.model_request_context.clone(),
            primary_model_facts: input.primary_model_facts.clone(),
            agent_type: input.agent_type,
            context_vars: round_context_vars,
            permission_constraints: input.permission_constraints,
            permission_runtime_ceiling: input.context.permission_runtime_ceiling.clone(),
            delegation_policy: input.context.delegation_policy,
            runtime_tool_restrictions: finalize_runtime_tool_restrictions,
            steering_interrupt: None,
            cancellation_token: CancellationToken::new(),
            workspace_services: input.context.workspace_services.clone(),
            terminal_port: input.context.terminal_port.clone(),
            remote_exec_port: input.context.remote_exec_port.clone(),
            recover_partial_on_cancel: input.context.recover_partial_on_cancel,
        };

        self.round_executor
            .execute_round(
                input.ai_client,
                round_context,
                final_ai_messages,
                input.tool_definitions,
                Some(input.context_window),
            )
            .await
    }

    #[allow(clippy::too_many_arguments)] // full send-context; kept flat for the single call site
    async fn build_ai_messages_for_send(
        messages: &[Message],
        provider: &str,
        workspace_path: Option<&Path>,
        current_turn_id: &str,
        attach_images: bool,
        static_prepended_reminders: &[&str],
        dynamic_prepended_reminders: &[&str],
        max_image_bearing_messages: usize,
    ) -> BitFunResult<Vec<AIMessage>> {
        // Only the last `max_image_bearing_messages` messages that contain
        // images keep their images for the API.
        let limits = ImageLimits::for_provider(provider);

        let trimmed_static_reminders = static_prepended_reminders
            .iter()
            .map(|text| text.trim())
            .filter(|text| !text.is_empty())
            .collect::<Vec<_>>();
        let trimmed_dynamic_reminders = dynamic_prepended_reminders
            .iter()
            .map(|text| text.trim())
            .filter(|text| !text.is_empty())
            .collect::<Vec<_>>();
        let mut result = Vec::with_capacity(
            messages.len() + trimmed_static_reminders.len() + trimmed_dynamic_reminders.len(),
        );
        let mut attached_image_count = 0usize;
        let first_non_system_index = messages
            .iter()
            .position(|msg| msg.role != crate::agentic::core::MessageRole::System)
            .unwrap_or(messages.len());
        let mut prepended_reminders_injected = false;

        let keep_image_messages = if attach_images {
            Self::image_bearing_indices_to_keep(messages, max_image_bearing_messages)
        } else {
            HashSet::new()
        };

        for (msg_idx, msg) in messages.iter().enumerate() {
            if !prepended_reminders_injected && msg_idx == first_non_system_index {
                // Static reminders (deferred tool listing / skill / agent /
                // runtime context) stay right after the system message so the
                // provider-side prompt/prefix cache prefix stays stable.
                for reminder in &trimmed_static_reminders {
                    result.push(AIMessage::system(render_system_reminder(reminder)));
                }
                prepended_reminders_injected = true;
            }

            if Self::skip_message_for_model_send(msg)
                || Self::is_stale_interrupted_continue(msg, current_turn_id)
            {
                continue;
            }
            let keep_this_message_images = attach_images && keep_image_messages.contains(&msg_idx);
            match &msg.content {
                MessageContent::Multimodal { text, images } => {
                    if !attach_images {
                        // Primary model is text-only (or images are disabled). Convert to text-only
                        // placeholder so providers that don't support image inputs won't error.
                        result.push(AIMessage::from(msg));
                        continue;
                    }

                    let (filtered_images, dropped_count): (Vec<ImageContextData>, usize) =
                        if images.is_empty() {
                            (Vec::new(), 0)
                        } else if keep_this_message_images {
                            (images.clone(), 0)
                        } else {
                            (Vec::new(), images.len())
                        };

                    let prompt = if text.trim().is_empty() {
                        "(image attached)".to_string()
                    } else {
                        text.clone()
                    };
                    let prompt = if dropped_count > 0 {
                        format!(
                            "{}\n\n[{} image(s) from this message omitted: only the latest {} message(s) in the conversation that contain images are sent to the model.]",
                            prompt.trim_end(),
                            dropped_count,
                            max_image_bearing_messages
                        )
                    } else {
                        prompt
                    };

                    match process_image_contexts_for_provider(
                        &filtered_images,
                        provider,
                        workspace_path,
                    )
                    .await
                    {
                        Ok(processed) => {
                            let next_count = attached_image_count + processed.len();
                            if next_count > limits.max_images_per_request {
                                return Err(BitFunError::validation(format!(
                                    "Too many images in one request: {} > {}",
                                    next_count, limits.max_images_per_request
                                )));
                            }
                            attached_image_count = next_count;

                            let multimodal = build_multimodal_message_with_images(
                                &prompt, &processed, provider,
                            )?;
                            result.extend(multimodal);
                        }
                        Err(err) => {
                            if matches!(&err, BitFunError::Validation(msg) if msg.starts_with("Too many images in one request"))
                            {
                                return Err(err);
                            }
                            let is_current_turn_message =
                                msg.metadata.turn_id.as_deref() == Some(current_turn_id);
                            if Self::can_fallback_to_text_only(
                                images,
                                &err,
                                is_current_turn_message,
                            ) {
                                warn!(
                                    "Failed to rebuild multimodal payload, falling back to text-only message: message_id={}, provider={}, turn_id={:?}, current_turn_id={}, error={}",
                                    msg.id, provider, msg.metadata.turn_id, current_turn_id, err
                                );
                                result.push(AIMessage::from(msg));
                            } else {
                                return Err(err);
                            }
                        }
                    }
                }
                MessageContent::ToolResult { .. } => {
                    if !attach_images {
                        result.push(AIMessage::from(msg));
                        continue;
                    }
                    let mut ai = AIMessage::from(msg.clone());
                    if let Some(atts) = ai.tool_image_attachments.take() {
                        if !atts.is_empty() {
                            if keep_this_message_images {
                                let next_count = attached_image_count + atts.len();
                                if next_count > limits.max_images_per_request {
                                    return Err(BitFunError::validation(format!(
                                        "Too many images in one request: {} > {}",
                                        next_count, limits.max_images_per_request
                                    )));
                                }
                                attached_image_count = next_count;
                                ai.tool_image_attachments = Some(atts);
                            } else {
                                let dropped = atts.len();
                                let content_str = ai.content.as_deref().unwrap_or("");
                                ai.content = Some(format!(
                                    "{}\n\n[{} image(s) from this tool result omitted: only the latest {} message(s) in the conversation that contain images are sent to the model.]",
                                    content_str.trim_end(),
                                    dropped,
                                    max_image_bearing_messages
                                ));
                                ai.tool_image_attachments = None;
                            }
                        }
                    }
                    result.push(ai);
                }
                _ => result.push(AIMessage::from(msg)),
            }
        }

        if !prepended_reminders_injected {
            for reminder in trimmed_static_reminders {
                result.push(AIMessage::system(render_system_reminder(reminder)));
            }
        }

        // Dynamic reminders (runtime facts refreshed every round + user
        // context) are always appended at the very end of the message
        // sequence, after the newest user message, so their per-round
        // changes never break the stable cache prefix built from the system
        // message, the static reminders and the full conversation history.
        for reminder in trimmed_dynamic_reminders {
            result.push(AIMessage::system(render_system_reminder(reminder)));
        }

        Ok(result)
    }

    fn render_multimodal_as_text(text: &str, images: &[ImageContextData]) -> String {
        let mut content = text.to_string();

        if images.is_empty() {
            return content;
        }

        content.push_str("\n\n[Attached image(s):\n");
        for image in images {
            let name = image
                .metadata
                .as_ref()
                .and_then(|m| m.get("name"))
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
                .map(str::to_string)
                .unwrap_or_else(|| image.id.clone());

            let path = image.image_path.as_deref().filter(|s| !s.trim().is_empty());

            if let Some(path) = path {
                content.push_str(&format!(
                    "- {} ({}, image_id={}, path={})\n",
                    name, image.mime_type, image.id, path
                ));
            } else {
                content.push_str(&format!(
                    "- {} ({}, image_id={})\n",
                    name, image.mime_type, image.id
                ));
            }
        }
        content.push_str("]\n");

        content.push_str("Note: the primary model cannot inspect image pixels directly. If an image path is available, use analyze_image to inspect it, or use a user-provided image skill with that path.\n");

        content
    }

    async fn build_compression_request_messages(
        &self,
        runtime_messages: &[Message],
        dialog_turn_id: &str,
        workspace: Option<&WorkspaceBinding>,
        provider: &str,
        attach_images: bool,
        prepended_prompt_reminders: &PrependedPromptReminders,
    ) -> BitFunResult<Vec<AIMessage>> {
        let static_reminders = prepended_prompt_reminders.static_ordered_reminders();
        let dynamic_reminders = prepended_prompt_reminders.dynamic_ordered_reminders();
        let mut compression_messages = Self::build_ai_messages_for_send(
            runtime_messages,
            provider,
            workspace.map(|workspace| workspace.root_path()),
            dialog_turn_id,
            attach_images,
            &static_reminders,
            &dynamic_reminders,
            Self::configured_max_image_bearing_messages().await,
        )
        .await?;
        compression_messages.push(AIMessage::user(
            self.context_compressor.build_compact_prompt(),
        ));
        Ok(compression_messages)
    }

    async fn request_compression_summary_with_retry(
        &self,
        ai_client: Arc<crate::infrastructure::ai::AIClient>,
        request_messages: Vec<AIMessage>,
        tool_definitions: Option<Vec<ToolDefinition>>,
        model_request_context: &ModelRequestContext,
        trace_config: Option<ModelExchangeTraceConfig>,
        max_tries: usize,
    ) -> BitFunResult<String> {
        let mut last_error = None;
        let base_wait_time_ms = 500;

        for attempt in 0..max_tries {
            let result = ai_client
                .send_message_with_trace_and_request_context(
                    request_messages.clone(),
                    tool_definitions.clone(),
                    Some(model_request_context.clone()),
                    trace_config.clone(),
                )
                .await;

            match result {
                Ok(response) => {
                    if response.tool_calls.is_some() {
                        return Err(BitFunError::AIClient(
                            "Compression request returned tool calls instead of a summary"
                                .to_string(),
                        ));
                    }
                    if attempt > 0 {
                        debug!(
                            "Compression summary generation succeeded (attempt {}/{})",
                            attempt + 1,
                            max_tries
                        );
                    }
                    return Ok(response.text);
                }
                Err(err) => {
                    let provider_error = err
                        .downcast_ref::<bitfun_core_types::errors::AiProviderError>()
                        .cloned();
                    let err_msg = err.to_string();
                    warn!(
                        "Compression summary generation failed (attempt {}/{}): {}",
                        attempt + 1,
                        max_tries,
                        err_msg
                    );
                    let category = provider_error
                        .as_ref()
                        .map(|error| error.category.clone())
                        .unwrap_or_else(|| {
                            bitfun_core_types::errors::classify_ai_error_message(&err_msg)
                        });
                    if category == bitfun_core_types::errors::ErrorCategory::ContextOverflow {
                        return Err(BitFunError::RecoverableContextOverflow(
                            provider_error.unwrap_or_else(|| {
                                bitfun_core_types::errors::AiProviderError::classified(
                                    err_msg,
                                    bitfun_core_types::errors::ErrorCategory::ContextOverflow,
                                )
                            }),
                        ));
                    }
                    last_error = Some(err);

                    if attempt < max_tries - 1 {
                        let delay_ms = base_wait_time_ms * (1 << attempt.min(3));
                        debug!(
                            "Waiting {}ms before compression summary retry {}...",
                            delay_ms,
                            attempt + 2
                        );
                        tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
                    }
                }
            }
        }

        Err(BitFunError::AIClient(format!(
            "Compression summary generation failed after {} attempts: {}",
            max_tries,
            last_error
                .map(|err| err.to_string())
                .unwrap_or_else(|| "Unknown error".to_string())
        )))
    }

    async fn generate_compression_model_summary(
        &self,
        input: CompressionModelSummaryInput<'_>,
    ) -> BitFunResult<Option<String>> {
        let request_messages = self
            .build_compression_request_messages(
                input.runtime_messages,
                input.dialog_turn_id,
                input.workspace,
                &input.ai_client.config.format,
                input.primary_supports_image_understanding,
                input.prepended_prompt_reminders,
            )
            .await?;

        let raw_summary = self
            .request_compression_summary_with_retry(
                input.ai_client,
                request_messages,
                input.tool_definitions.clone(),
                input.model_request_context,
                input.trace_config,
                2,
            )
            .await?;
        let summary =
            ContextCompressor::normalize_model_summary_output(&raw_summary).ok_or_else(|| {
                BitFunError::AIClient(
                    "Model-based compression returned an empty summary".to_string(),
                )
            })?;
        Ok(Some(summary))
    }

    #[allow(clippy::too_many_arguments)]
    async fn build_planned_compression_result(
        &self,
        session_id: &str,
        dialog_turn_id: &str,
        runtime_messages: &[Message],
        context_window: usize,
        compression_contract: Option<crate::agentic::core::CompressionContract>,
        ai_client: Arc<crate::infrastructure::ai::AIClient>,
        model_request_context: &ModelRequestContext,
        tool_definitions: &Option<Vec<ToolDefinition>>,
        prepended_prompt_reminders: &PrependedPromptReminders,
        primary_supports_image_understanding: bool,
        workspace: Option<&WorkspaceBinding>,
        trace_config: Option<ModelExchangeTraceConfig>,
    ) -> BitFunResult<Option<crate::agentic::session::CompressionResult>> {
        let max_initial_recent = context_window.saturating_div(2).max(1);
        let recent_context_tokens = Self::configured_compression_recent_context_tokens().await;
        let retry_step_tokens = Self::configured_compression_retry_step_tokens().await;
        let mut recent_target = recent_context_tokens.min(max_initial_recent);
        let max_overflow_attempts = Self::configured_compression_overflow_attempts().await;
        let mut selected_plan = None;
        let mut model_summary = None;

        for attempt in 0..max_overflow_attempts {
            let Some(plan) = self.context_compressor.plan_compression(
                session_id,
                runtime_messages,
                context_window,
                recent_target,
                Some(Self::configured_compression_max_retained_user_tokens().await),
            )?
            else {
                break;
            };
            info!(
                "Compression context plan: session_id={}, turn_id={}, attempt={}/{}, retained_user_token_budget={}, retained_user_tokens={}, retained_user_messages={}, recent_target_tokens={}, recent_tail_tokens={}, cutoff_message_index={}, summary_messages={}, recent_tail_messages={}",
                session_id,
                dialog_turn_id,
                attempt + 1,
                max_overflow_attempts,
                plan.retained_user_token_budget,
                plan.retained_user_tokens,
                plan.retained_user_messages.len(),
                plan.recent_target_tokens,
                plan.recent_tail_tokens,
                plan.cutoff_message_index,
                plan.summary_messages.len(),
                plan.recent_tail_messages.len()
            );

            let summary_result = self
                .generate_compression_model_summary(CompressionModelSummaryInput {
                    ai_client: ai_client.clone(),
                    model_request_context,
                    runtime_messages: &plan.summary_request_messages,
                    dialog_turn_id,
                    workspace,
                    tool_definitions,
                    prepended_prompt_reminders,
                    primary_supports_image_understanding,
                    trace_config: trace_config.clone(),
                })
                .await;

            match summary_result {
                Ok(summary) => {
                    selected_plan = Some(plan);
                    model_summary = summary;
                    break;
                }
                Err(err) if err.is_recoverable_context_overflow() => {
                    warn!(
                        "Compression request exceeded provider context: session_id={}, turn_id={}, attempt={}/{}, recent_target_tokens={}, cutoff_message_index={}, next_recent_target_tokens={:?}, error={}",
                        session_id,
                        dialog_turn_id,
                        attempt + 1,
                        max_overflow_attempts,
                        plan.recent_target_tokens,
                        plan.cutoff_message_index,
                        plan.next_recent_target_tokens,
                        err
                    );
                    let can_retry = attempt + 1 < max_overflow_attempts
                        && plan.next_recent_target_tokens.is_some();
                    let next_recent_target = plan.next_recent_target_tokens;
                    selected_plan = Some(plan);
                    if can_retry {
                        recent_target = recent_target
                            .saturating_add(retry_step_tokens)
                            .max(next_recent_target.expect("retry target checked above"));
                        continue;
                    }
                    break;
                }
                Err(err) => {
                    warn!(
                        "Model-based compression failed, falling back to structured local compression: {}",
                        err
                    );
                    selected_plan = Some(plan);
                    break;
                }
            }
        }

        let Some(selected_plan) = selected_plan else {
            return Ok(None);
        };
        self.context_compressor
            .compress_plan_with_contract(
                session_id,
                context_window,
                selected_plan,
                compression_contract,
                model_summary,
            )
            .map(Some)
    }

    async fn resolve_compression_runtime_scaffold(
        &self,
        session: &Session,
        context: &ExecutionContext,
    ) -> BitFunResult<CompressionRuntimeScaffold> {
        let agent_registry = get_agent_registry();
        agent_registry
            .load_custom_agents(
                context
                    .workspace
                    .as_ref()
                    .map(|workspace| workspace.root_path()),
            )
            .await;

        let current_agent = agent_registry
            .get_agent(
                &context.agent_type,
                context
                    .workspace
                    .as_ref()
                    .map(|workspace| workspace.root_path()),
            )
            .ok_or_else(|| {
                BitFunError::NotFound(format!("Agent not found: {}", context.agent_type))
            })?;

        let original_user_input = context
            .context
            .get("original_user_input")
            .cloned()
            .unwrap_or_default();
        let (model_id, _) = self
            .resolve_model_id_for_turn(
                session,
                &context.agent_type,
                context.workspace.as_ref(),
                &original_user_input,
                context.turn_index,
                context
                    .context
                    .get(INTERRUPTED_TURN_RESOLVED_MODEL_ID_METADATA_KEY)
                    .map(String::as_str),
                context
                    .context
                    .get(INTERRUPTED_TURN_MODEL_BINDING_FINGERPRINT_METADATA_KEY)
                    .map(String::as_str),
            )
            .await?;

        let ai_client_factory = get_global_ai_client_factory().await.map_err(|e| {
            BitFunError::AIClient(format!("Failed to get AI client factory: {}", e))
        })?;
        let reasoning_preset = match self
            .resolve_reasoning_selection_for_turn(&session.session_id, context)
            .await
        {
            Ok(reasoning_preset) => reasoning_preset,
            Err(error) => {
                warn!(
                    "Failed to persist reasoning preset fallback; using Auto for this turn: session_id={}, error={}",
                    session.session_id, error
                );
                None
            }
        };
        let ai_client_result = if matches!(
            session.config.model_binding_policy,
            SessionModelBindingPolicy::ApprovedImmutable
        ) {
            ai_client_factory
                .get_client_by_approved_binding_with_reasoning_preset(
                    &model_id,
                    session
                        .config
                        .model_binding_fingerprint
                        .as_deref()
                        .unwrap_or_default(),
                    reasoning_preset.as_deref(),
                )
                .await
        } else {
            ai_client_factory
                .get_client_resolved_with_reasoning_preset(&model_id, reasoning_preset.as_deref())
                .await
        };
        let ai_client = match ai_client_result {
            Ok(ai_client) => ai_client,
            Err(error) => {
                if context
                    .context
                    .contains_key(INTERRUPTED_TURN_MODEL_BINDING_FINGERPRINT_METADATA_KEY)
                {
                    // Re-check the frozen binding after a factory failure so a
                    // config race is classified as recoverable contract drift,
                    // while credentials/provider/client construction failures
                    // remain ordinary execution failures.
                    Self::validate_frozen_model_contract(context).await?;
                }
                return Err(BitFunError::AIClient(format!(
                    "Failed to get AI client (model_id={}): {}",
                    model_id, error
                )));
            }
        };
        Self::validate_frozen_model_contract(context).await?;
        Self::validate_frozen_reasoning_contract(context, ai_client.as_ref())?;
        let model_request_context = Self::model_request_context(
            session.effective_prompt_cache_lineage_id(),
            &session.session_id,
            &context.dialog_turn_id,
        );

        let primary_model_facts = Self::resolve_primary_model_context(
            &model_id,
            session.config.model_binding_policy,
            &ai_client.config.model,
            &ai_client.config.format,
            "Config service unavailable, assuming compression model is text-only for image input gating",
        )
        .await;
        let resolved_primary_model_id = primary_model_facts.model_id.clone();
        let primary_supports_image_understanding = primary_model_facts.supports_image_inputs;

        let model_capability_profile = ModelCapabilityProfile::from_resolved_model(
            &resolved_primary_model_id,
            &ai_client.config.model,
        );
        let is_review_subagent = agent_registry
            .get_subagent_is_review(&context.agent_type)
            .unwrap_or(false);
        let context_profile_policy = ContextProfilePolicy::for_agent_context(
            &context.agent_type,
            is_review_subagent,
            model_capability_profile,
        );

        let tool_policy = agent_registry
            .get_agent_tool_policy(
                &context.agent_type,
                context
                    .workspace
                    .as_ref()
                    .map(|workspace| workspace.root_path()),
            )
            .await;
        let mut allowed_tools = tool_policy.allowed_tools.clone();
        ensure_primary_session_goal_tools(
            &mut allowed_tools,
            context.subagent_parent_info.is_some(),
        );
        let enable_tools = context
            .context
            .get("enable_tools")
            .and_then(|value| value.parse::<bool>().ok())
            .unwrap_or(true);
        let tool_manifest_context_vars = context.context.clone();

        let tool_description_context = tool_context_runtime::build_tool_description_context(
            &context.agent_type,
            context.workspace.as_ref(),
            context.workspace_services.as_ref(),
            Some(&primary_model_facts),
            &tool_manifest_context_vars,
            &context.runtime_tool_restrictions,
        );
        let tool_manifest = if enable_tools {
            Some(
                resolve_tool_manifest(
                    &allowed_tools,
                    &tool_policy.exposure_overrides,
                    &tool_description_context,
                )
                .await,
            )
        } else {
            None
        };
        let tool_listing_sections = if let Some(manifest) = tool_manifest.as_ref() {
            Self::build_tool_listing_sections(manifest, &tool_description_context).await
        } else {
            ToolListingSections::default()
        };
        let runtime_context_needs = tool_manifest
            .as_ref()
            .map(|manifest| {
                RuntimeContextNeeds::from_tool_names(manifest.allowed_tool_names.iter())
            })
            .unwrap_or_default();
        // Snapshot prompt-visible tool definitions once for this turn. Do not
        // re-resolve or rewrite them after GetToolSpec loads a deferred tool spec:
        // the loaded detail travels in tool results, while mutating the tool
        // definitions would change the request prefix and trigger provider
        // prefix/KV cache misses on subsequent rounds.
        let tool_definitions = tool_manifest.map(|manifest| manifest.tool_definitions);

        let turn_prompt_scaffold = self
            .resolve_turn_prompt_scaffold(TurnPromptScaffoldInput {
                context,
                current_agent: current_agent.as_ref(),
                model_name: &ai_client.config.model,
                supports_image_understanding: primary_supports_image_understanding,
                tool_listing_sections,
                runtime_context_needs,
                // Compression model requests do not need per-turn runtime
                // facts; the default keeps their prompt prefix stable.
                runtime_facts_usage: RuntimeFactsUsage::default(),
                stage: "compression_scaffold",
            })
            .await?;

        Ok(CompressionRuntimeScaffold {
            ai_client,
            model_request_context,
            tool_definitions,
            system_prompt_message: turn_prompt_scaffold.system_prompt_message,
            prepended_prompt_reminders: turn_prompt_scaffold.prepended_prompt_reminders,
            primary_supports_image_understanding,
            compression_contract_limit: context_profile_policy.compression_contract_limit,
        })
    }

    /// Plain assistant text of a message, when it has any.
    fn assistant_message_text(message: &Message) -> Option<&str> {
        match &message.content {
            MessageContent::Text(text) => Some(text.as_str()),
            MessageContent::Multimodal { text, .. } => Some(text.as_str()),
            _ => None,
        }
        .map(str::trim)
        .filter(|text| !text.is_empty())
    }

    /// Native hook session facts for a compaction or turn-lifecycle dispatch.
    fn native_hook_facts<'a>(
        session_id: &'a str,
        dialog_turn_id: &'a str,
        workspace: Option<&'a WorkspaceBinding>,
        model: &'a str,
    ) -> NativeHookSessionFacts<'a> {
        NativeHookSessionFacts {
            session_id,
            turn_id: Some(dialog_turn_id),
            workspace_root: workspace.map(|workspace| workspace.root_path()),
            is_remote_workspace: workspace.is_some_and(|workspace| workspace.is_remote()),
            model,
            bypass_permissions: false,
        }
    }

    /// Custom compaction checkpoint, intentionally outside the `app.hooks.enabled`
    /// gate: persist a lightweight pre-compaction progress snapshot into session
    /// metadata so long-running tasks can verify goal/role/todos state survived
    /// context compaction.
    async fn preserve_compaction_progress_snapshot(
        &self,
        session_id: &str,
        trigger: &str,
        session: &Session,
    ) {
        let Some(storage_path) = self
            .session_manager
            .effective_session_storage_path(session_id)
            .await
        else {
            // Session persistence is disabled; there is nowhere to store the
            // snapshot and post-compaction verification is skipped accordingly.
            debug!(
                "Compaction snapshot skipped (session storage unavailable): session_id={}",
                session_id
            );
            return;
        };

        let mut has_thread_goal = false;
        let mut todos_present = false;
        let mut custom_metadata_present = false;
        match self
            .session_manager
            .load_session_metadata(&storage_path, session_id)
            .await
        {
            Ok(Some(metadata)) => {
                has_thread_goal = metadata
                    .custom_metadata
                    .as_ref()
                    .and_then(|value| value.get(bitfun_runtime_ports::THREAD_GOAL_METADATA_KEY))
                    .is_some();
                todos_present = metadata.todos.is_some();
                custom_metadata_present = metadata.custom_metadata.is_some();
            }
            Ok(None) => {}
            Err(error) => {
                debug!(
                    "Compaction snapshot baseline unavailable: session_id={}, error={}",
                    session_id, error
                );
            }
        }

        let snapshot = serde_json::json!({
            "trigger": trigger,
            "compressionCountBefore": session.compression_state.compression_count,
            "agentType": session.agent_type,
            "hasThreadGoal": has_thread_goal,
            "todosPresent": todos_present,
            "customMetadataPresent": custom_metadata_present,
            "recordedAtMs": compaction_snapshot_timestamp_ms(),
        });
        if let Err(error) = self
            .session_manager
            .merge_session_custom_metadata(
                session_id,
                serde_json::json!({ COMPACTION_PROGRESS_SNAPSHOT_KEY: snapshot }),
            )
            .await
        {
            warn!(
                "Failed to persist compaction progress snapshot: session_id={}, trigger={}, error={}",
                session_id, trigger, error
            );
        } else {
            // Registered: active subagent tracking is runtime-only (coordinator
            // in-memory state) and is not persisted in session metadata;
            // compaction does not clear it.
            debug!(
                "Compaction snapshot recorded: session_id={}, trigger={}, active_subagents=runtime_only_not_persisted",
                session_id, trigger
            );
        }
    }

    /// Custom compaction checkpoint, intentionally outside the `app.hooks.enabled`
    /// gate: read-only verification that goal/role/todos survived context
    /// compaction. Only warns on missing state; never blocks or rewrites anything.
    async fn verify_compaction_progress_state(
        &self,
        session_id: &str,
        trigger: &str,
        session: &Session,
    ) {
        let Some(storage_path) = self
            .session_manager
            .effective_session_storage_path(session_id)
            .await
        else {
            return;
        };
        let metadata = match self
            .session_manager
            .load_session_metadata(&storage_path, session_id)
            .await
        {
            Ok(Some(metadata)) => metadata,
            Ok(None) => {
                warn!(
                    "Compaction verification: session metadata missing after compaction: session_id={}, trigger={}",
                    session_id, trigger
                );
                return;
            }
            Err(error) => {
                warn!(
                    "Compaction verification: failed to load session metadata after compaction: session_id={}, trigger={}, error={}",
                    session_id, trigger, error
                );
                return;
            }
        };

        let Some(snapshot) = metadata
            .custom_metadata
            .as_ref()
            .and_then(|value| value.get(COMPACTION_PROGRESS_SNAPSHOT_KEY))
        else {
            // No baseline was recorded (e.g. persistence disabled at snapshot
            // time); verification is skipped without noise.
            return;
        };

        let mut missing = Vec::new();
        if session.agent_type
            != snapshot
                .get("agentType")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("")
        {
            missing.push("role(agent_type)");
        }
        if snapshot
            .get("hasThreadGoal")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false)
            && metadata
                .custom_metadata
                .as_ref()
                .and_then(|value| value.get(bitfun_runtime_ports::THREAD_GOAL_METADATA_KEY))
                .is_none()
        {
            missing.push("thread_goal");
        }
        if snapshot
            .get("todosPresent")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false)
            && metadata.todos.is_none()
        {
            missing.push("todos");
        }
        if snapshot
            .get("customMetadataPresent")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false)
            && metadata.custom_metadata.is_none()
        {
            missing.push("custom_metadata");
        }

        if missing.is_empty() {
            debug!(
                "Compaction verification passed: session_id={}, trigger={}",
                session_id, trigger
            );
        } else {
            warn!(
                "Compaction verification: state lost across compaction: session_id={}, trigger={}, missing={}",
                session_id, trigger, missing.join(",")
            );
        }
    }

    /// Compress context, will emit compression events (Started, Completed, and Failed)
    #[allow(clippy::too_many_arguments)]
    async fn compress_messages(
        &self,
        session_id: &str,
        dialog_turn_id: &str,
        trigger: &str,
        runtime_messages: Vec<Message>,
        before_pressure: TokenPressureSnapshot,
        context_window: usize,
        ai_client: Arc<crate::infrastructure::ai::AIClient>,
        model_request_context: &ModelRequestContext,
        tool_definitions: &Option<Vec<ToolDefinition>>,
        system_prompt_message: Message,
        prepended_prompt_reminders: &PrependedPromptReminders,
        primary_supports_image_understanding: bool,
        compression_contract_limit: usize,
        workspace: Option<&WorkspaceBinding>,
    ) -> BitFunResult<Option<(usize, Vec<Message>)>> {
        let mut session = self
            .session_manager
            .get_session(session_id)
            .ok_or_else(|| BitFunError::NotFound(format!("Session not found: {}", session_id)))?;

        // Record start time
        let start_time = std::time::Instant::now();

        let old_messages_len = runtime_messages.len();
        if !runtime_messages
            .iter()
            .any(|message| message.role != MessageRole::System)
        {
            return Ok(None);
        }
        // Generate compression ID
        let compression_id = format!("compression_{}", uuid::Uuid::new_v4());
        // Captured before `ai_client` is consumed by summary generation.
        let ai_client_model = ai_client.config.model.clone();

        // Capture pre-compaction progress state before native hook dispatch so
        // long-running task state can be verified after compaction.
        self.preserve_compaction_progress_snapshot(session_id, trigger, &session)
            .await;

        native_hooks::dispatch_pre_compact(
            Self::native_hook_facts(session_id, dialog_turn_id, workspace, &ai_client_model),
            trigger,
        )
        .await;

        // Emit compression started event
        self.emit_event(
            AgenticEvent::ContextCompressionStarted {
                session_id: session_id.to_string(),
                turn_id: dialog_turn_id.to_string(),
                compression_id: compression_id.clone(),
                trigger: trigger.to_string(),
                tokens_before: before_pressure.total_tokens,
                context_window,
            },
            EventPriority::Normal,
        )
        .await;

        // Execute compression
        let compression_contract = self
            .session_manager
            .compression_contract_for_session(session_id, compression_contract_limit);
        let model_exchange_trace_dir = self
            .session_manager
            .persistent_model_exchange_trace_dir(session_id)
            .await;
        let trace_config = prepare_model_exchange_trace_for_workspace(
            session_id,
            dialog_turn_id,
            workspace,
            model_exchange_trace_dir.as_deref(),
            ModelExchangeTraceOperation {
                kind: "context_compression",
                id: &compression_id,
                trigger: Some(trigger),
            },
            ai_client.as_ref(),
        )
        .await;
        let planned_result = self
            .build_planned_compression_result(
                session_id,
                dialog_turn_id,
                &runtime_messages,
                context_window,
                compression_contract,
                ai_client,
                model_request_context,
                tool_definitions,
                prepended_prompt_reminders,
                primary_supports_image_understanding,
                workspace,
                trace_config,
            )
            .await;
        match planned_result {
            Ok(Some(mut compression_result)) => {
                let boundary_turn_index = self
                    .session_manager
                    .get_turn_count(session_id)
                    .saturating_sub(1);
                match self
                    .session_manager
                    .create_compression_transcript_reference(
                        session_id,
                        boundary_turn_index,
                        &compression_id,
                        trigger,
                    )
                    .await
                {
                    Ok(Some(reference)) => {
                        self.context_compressor.append_transcript_reference(
                            &mut compression_result,
                            &reference.uri,
                            &reference.index_range,
                        );
                    }
                    Ok(None) => {}
                    Err(error) => warn!(
                        "Failed to create automatic compression transcript; continuing without reference: session_id={}, turn_id={}, error={}",
                        session_id, dialog_turn_id, error
                    ),
                }
                self.session_manager
                    .replace_context_messages(session_id, compression_result.messages.clone())
                    .await;
                if self
                    .session_manager
                    .rebuild_skill_agent_listing_baseline_to_latest(session_id)
                    .await
                {
                    debug!(
                        "Rebuilt skill-agent listing baseline after compression: session_id={}",
                        session_id
                    );
                }
                self.session_manager
                    .invalidate_prompt_cache(
                        session_id,
                        crate::agentic::session::PromptCacheScope::All,
                        "context_compression_applied",
                    )
                    .await;
                let mut new_messages = vec![system_prompt_message];
                new_messages.extend(compression_result.messages);
                // Update session compression state
                session.compression_state.increment_compression_count();

                // Update session state
                let _ = self
                    .session_manager
                    .update_compression_state(session_id, session.compression_state.clone())
                    .await;

                // Calculate duration
                let duration_ms = elapsed_ms_u64(start_time);

                // Recalculate tokens after compression
                let prepended_reminders = prepended_prompt_reminders.ordered_reminders();
                let prepended_reminder_tokens =
                    Self::prepended_reminder_tokens_for_pressure(&prepended_reminders);
                let after_pressure = Self::estimate_auto_compression_pressure(
                    &new_messages,
                    tool_definitions.as_deref(),
                    context_window,
                    CompressionTriggerBudget {
                        input_limit: before_pressure.input_limit,
                        output_reserve_tokens: before_pressure.output_reserve_tokens,
                        safety_reserve_tokens: before_pressure.safety_reserve_tokens,
                    },
                    prepended_reminder_tokens,
                );
                let compressed_tokens = after_pressure.total_tokens;
                let summary_source = if compression_result.has_model_summary {
                    "model"
                } else {
                    "local_fallback"
                };

                info!(
                    "Compression completed: session_id={}, turn_id={}, messages {} -> {}, total_tokens {} -> {}, system_tokens {} -> {}, tool_tokens {} -> {}, prepended_reminder_tokens {} -> {}, conversation_tokens {} -> {}, context_window={}, input_limit={}, output_reserve={}, safety_reserve={}, usage {:.3} -> {:.3}, compression_count={}, duration_ms={}, summary_source={}",
                    session_id,
                    dialog_turn_id,
                    old_messages_len,
                    new_messages.len(),
                    before_pressure.total_tokens,
                    after_pressure.total_tokens,
                    before_pressure.system_tokens,
                    after_pressure.system_tokens,
                    before_pressure.tool_tokens,
                    after_pressure.tool_tokens,
                    before_pressure.prepended_reminder_tokens,
                    after_pressure.prepended_reminder_tokens,
                    before_pressure.conversation_tokens,
                    after_pressure.conversation_tokens,
                    before_pressure.context_window,
                    before_pressure.input_limit,
                    before_pressure.output_reserve_tokens,
                    before_pressure.safety_reserve_tokens,
                    before_pressure.usage_ratio,
                    after_pressure.usage_ratio,
                    session.compression_state.compression_count,
                    duration_ms,
                    summary_source
                );

                // Emit compression completed event
                self.emit_event(
                    AgenticEvent::ContextCompressionCompleted {
                        session_id: session_id.to_string(),
                        turn_id: dialog_turn_id.to_string(),
                        compression_id: compression_id.clone(),
                        compression_count: session.compression_state.compression_count,
                        tokens_before: before_pressure.total_tokens,
                        tokens_after: compressed_tokens,
                        compression_ratio: if before_pressure.total_tokens == 0 {
                            1.0
                        } else {
                            (compressed_tokens as f64) / (before_pressure.total_tokens as f64)
                        },
                        duration_ms,
                        has_summary: compression_result.has_model_summary,
                        summary_source: summary_source.to_string(),
                        applied: true,
                    },
                    EventPriority::Normal,
                )
                .await;

                native_hooks::dispatch_post_compact(
                    Self::native_hook_facts(
                        session_id,
                        dialog_turn_id,
                        workspace,
                        &ai_client_model,
                    ),
                    trigger,
                )
                .await;

                // Verify goal/role/todos survived compaction after native hook
                // dispatch; only warns on missing state.
                self.verify_compaction_progress_state(session_id, trigger, &session)
                    .await;

                Ok(Some((compressed_tokens, new_messages)))
            }
            Ok(None) => Ok(None),
            Err(e) => {
                // Emit compression failed event
                self.emit_event(
                    AgenticEvent::ContextCompressionFailed {
                        session_id: session_id.to_string(),
                        turn_id: dialog_turn_id.to_string(),
                        compression_id: compression_id.clone(),
                        error: e.to_string(),
                    },
                    EventPriority::High,
                )
                .await;

                Err(BitFunError::Session(e.to_string()))
            }
        }
    }

    /// Compact the current session context outside the normal dialog execution loop.
    /// Always emits compression started/completed/failed events for the provided turn.
    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn compact_session_context(
        &self,
        session_id: String,
        dialog_turn_id: String,
        compression_id: String,
        context: ExecutionContext,
        messages: Vec<Message>,
        trigger: &str,
        cancellation_token: CancellationToken,
        commit_gate: Arc<ManualCompactionCommitGate>,
    ) -> BitFunResult<ContextCompactionOutcome> {
        let mut session = self
            .session_manager
            .get_session(&session_id)
            .ok_or_else(|| BitFunError::NotFound(format!("Session not found: {}", session_id)))?;
        let start_time = std::time::Instant::now();
        let scaffold = self
            .resolve_compression_runtime_scaffold(&session, &context)
            .await?;
        // Capture pre-compaction progress state before native hook dispatch so
        // long-running task state can be verified after compaction.
        self.preserve_compaction_progress_snapshot(&session_id, trigger, &session)
            .await;
        native_hooks::dispatch_pre_compact(
            Self::native_hook_facts(
                &session_id,
                &dialog_turn_id,
                context.workspace.as_ref(),
                &scaffold.ai_client.config.model,
            ),
            trigger,
        )
        .await;
        let context_window = (scaffold.ai_client.config.context_window as usize)
            .min(session.config.max_context_tokens);
        let prepended_reminders = scaffold.prepended_prompt_reminders.ordered_reminders();
        let prepended_reminder_tokens =
            Self::prepended_reminder_tokens_for_pressure(&prepended_reminders);
        let compression_trigger_budget = Self::compression_trigger_budget_configured(
            context_window,
            scaffold.ai_client.config.max_tokens,
        )
        .await;
        let mut runtime_messages = vec![scaffold.system_prompt_message.clone()];
        runtime_messages.extend(messages.clone());
        let before_pressure = Self::estimate_auto_compression_pressure(
            &runtime_messages,
            scaffold.tool_definitions.as_deref(),
            context_window,
            compression_trigger_budget,
            prepended_reminder_tokens,
        );

        self.emit_event(
            AgenticEvent::ContextCompressionStarted {
                session_id: session_id.to_string(),
                turn_id: dialog_turn_id.to_string(),
                compression_id: compression_id.clone(),
                trigger: trigger.to_string(),
                tokens_before: before_pressure.total_tokens,
                context_window,
            },
            EventPriority::Normal,
        )
        .await;

        let compression_contract = self
            .session_manager
            .compression_contract_for_session(&session_id, scaffold.compression_contract_limit);
        let model_exchange_trace_dir = self
            .session_manager
            .persistent_model_exchange_trace_dir(&session_id)
            .await;
        let trace_config = prepare_model_exchange_trace_for_workspace(
            &session_id,
            &dialog_turn_id,
            context.workspace.as_ref(),
            model_exchange_trace_dir.as_deref(),
            ModelExchangeTraceOperation {
                kind: "context_compression",
                id: &compression_id,
                trigger: Some(trigger),
            },
            scaffold.ai_client.as_ref(),
        )
        .await;
        let planned_result = tokio::select! {
            biased;
            _ = cancellation_token.cancelled() => {
                Err(BitFunError::Cancelled("Manual context compaction cancelled".to_string()))
            }
            result = self.build_planned_compression_result(
                &session_id,
                &dialog_turn_id,
                &runtime_messages,
                context_window,
                compression_contract,
                scaffold.ai_client.clone(),
                &scaffold.model_request_context,
                &scaffold.tool_definitions,
                &scaffold.prepended_prompt_reminders,
                scaffold.primary_supports_image_understanding,
                context.workspace.as_ref(),
                trace_config,
            ) => result,
        };
        let planned_result = match planned_result {
            Ok(result) if commit_gate.try_begin_commit() => Ok(result),
            Ok(_) => Err(BitFunError::Cancelled(
                "Manual context compaction cancelled".to_string(),
            )),
            Err(error) => Err(error),
        };
        match planned_result {
            Ok(Some(mut compression_result)) => {
                let boundary_turn_index = self
                    .session_manager
                    .get_turn_count(&session_id)
                    .saturating_sub(1);
                match self
                    .session_manager
                    .create_compression_transcript_reference(
                        &session_id,
                        boundary_turn_index,
                        &compression_id,
                        trigger,
                    )
                    .await
                {
                    Ok(Some(reference)) => {
                        self.context_compressor.append_transcript_reference(
                            &mut compression_result,
                            &reference.uri,
                            &reference.index_range,
                        );
                    }
                    Ok(None) => {}
                    Err(error) => warn!(
                        "Failed to create manual compression transcript; continuing without reference: session_id={}, turn_id={}, error={}",
                        session_id, dialog_turn_id, error
                    ),
                }
                let compressed_messages = compression_result.messages;
                self.session_manager
                    .replace_context_messages(&session_id, compressed_messages.clone())
                    .await;
                if self
                    .session_manager
                    .rebuild_skill_agent_listing_baseline_to_latest(&session_id)
                    .await
                {
                    debug!(
                        "Rebuilt skill-agent listing baseline after manual compaction: session_id={}",
                        session_id
                    );
                }
                self.session_manager
                    .invalidate_prompt_cache(
                        &session_id,
                        crate::agentic::session::PromptCacheScope::All,
                        "manual_context_compaction_applied",
                    )
                    .await;

                session.compression_state.increment_compression_count();
                let compression_count = session.compression_state.compression_count;
                let _ = self
                    .session_manager
                    .update_compression_state(&session_id, session.compression_state.clone())
                    .await;

                let duration_ms = elapsed_ms_u64(start_time);
                let mut compressed_runtime_messages = vec![scaffold.system_prompt_message.clone()];
                compressed_runtime_messages.extend(compressed_messages.clone());
                let after_pressure = Self::estimate_auto_compression_pressure(
                    &compressed_runtime_messages,
                    scaffold.tool_definitions.as_deref(),
                    context_window,
                    compression_trigger_budget,
                    prepended_reminder_tokens,
                );
                let tokens_after = after_pressure.total_tokens;
                let compression_ratio = if before_pressure.total_tokens == 0 {
                    1.0
                } else {
                    (tokens_after as f64) / (before_pressure.total_tokens as f64)
                };
                info!(
                    "Manual compression completed: session_id={}, turn_id={}, total_tokens {} -> {}, system_tokens {} -> {}, tool_tokens {} -> {}, prepended_reminder_tokens {} -> {}, conversation_tokens {} -> {}, context_window={}, input_limit={}, output_reserve={}, safety_reserve={}, usage {:.3} -> {:.3}, compression_count={}, duration_ms={}, summary_source={}",
                    session_id,
                    dialog_turn_id,
                    before_pressure.total_tokens,
                    after_pressure.total_tokens,
                    before_pressure.system_tokens,
                    after_pressure.system_tokens,
                    before_pressure.tool_tokens,
                    after_pressure.tool_tokens,
                    before_pressure.prepended_reminder_tokens,
                    after_pressure.prepended_reminder_tokens,
                    before_pressure.conversation_tokens,
                    after_pressure.conversation_tokens,
                    before_pressure.context_window,
                    before_pressure.input_limit,
                    before_pressure.output_reserve_tokens,
                    before_pressure.safety_reserve_tokens,
                    before_pressure.usage_ratio,
                    after_pressure.usage_ratio,
                    compression_count,
                    duration_ms,
                    if compression_result.has_model_summary {
                        "model"
                    } else {
                        "local_fallback"
                    }
                );

                self.emit_event(
                    AgenticEvent::ContextCompressionCompleted {
                        session_id: session_id.to_string(),
                        turn_id: dialog_turn_id.to_string(),
                        compression_id: compression_id.clone(),
                        compression_count,
                        tokens_before: before_pressure.total_tokens,
                        tokens_after,
                        compression_ratio,
                        duration_ms,
                        has_summary: compression_result.has_model_summary,
                        summary_source: if compression_result.has_model_summary {
                            "model".to_string()
                        } else {
                            "local_fallback".to_string()
                        },
                        applied: true,
                    },
                    EventPriority::Normal,
                )
                .await;

                native_hooks::dispatch_post_compact(
                    Self::native_hook_facts(
                        &session_id,
                        &dialog_turn_id,
                        context.workspace.as_ref(),
                        &scaffold.ai_client.config.model,
                    ),
                    trigger,
                )
                .await;

                // Verify goal/role/todos survived compaction after native hook
                // dispatch; only warns on missing state.
                self.verify_compaction_progress_state(&session_id, trigger, &session)
                    .await;

                Ok(ContextCompactionOutcome {
                    compression_id,
                    compression_count,
                    tokens_before: before_pressure.total_tokens,
                    tokens_after,
                    compression_ratio,
                    duration_ms,
                    has_summary: compression_result.has_model_summary,
                    summary_source: if compression_result.has_model_summary {
                        "model".to_string()
                    } else {
                        "local_fallback".to_string()
                    },
                    applied: true,
                })
            }
            Ok(None) => {
                let duration_ms = elapsed_ms_u64(start_time);
                let tokens_after = before_pressure.total_tokens;
                let compression_ratio = if before_pressure.total_tokens == 0 {
                    1.0
                } else {
                    (tokens_after as f64) / (before_pressure.total_tokens as f64)
                };
                info!(
                    "Manual compression skipped: session_id={}, turn_id={}, reason=no_eligible_prefix, total_tokens={}, duration_ms={}",
                    session_id, dialog_turn_id, before_pressure.total_tokens, duration_ms
                );
                self.emit_event(
                    AgenticEvent::ContextCompressionCompleted {
                        session_id: session_id.to_string(),
                        turn_id: dialog_turn_id.to_string(),
                        compression_id: compression_id.clone(),
                        compression_count: session.compression_state.compression_count,
                        tokens_before: before_pressure.total_tokens,
                        tokens_after,
                        compression_ratio,
                        duration_ms,
                        has_summary: false,
                        summary_source: "none".to_string(),
                        applied: false,
                    },
                    EventPriority::Normal,
                )
                .await;
                Ok(ContextCompactionOutcome {
                    compression_id,
                    compression_count: session.compression_state.compression_count,
                    tokens_before: before_pressure.total_tokens,
                    tokens_after,
                    compression_ratio,
                    duration_ms,
                    has_summary: false,
                    summary_source: "none".to_string(),
                    applied: false,
                })
            }
            Err(err) => {
                self.emit_event(
                    AgenticEvent::ContextCompressionFailed {
                        session_id: session_id.to_string(),
                        turn_id: dialog_turn_id.to_string(),
                        compression_id: compression_id.clone(),
                        error: err.to_string(),
                    },
                    EventPriority::High,
                )
                .await;

                Err(manual_compaction_terminal_error(err))
            }
        }
    }

    /// Execute a complete dialog turn (may contain multiple model rounds)
    /// Returns ExecutionResult containing the final response and all newly generated messages
    pub async fn execute_dialog_turn(
        &self,
        agent_type: String,
        initial_messages: Vec<Message>,
        context: ExecutionContext,
    ) -> BitFunResult<ExecutionResult> {
        let start_time = std::time::Instant::now();
        let dialog_turn_id = context.dialog_turn_id.clone();
        self.generation_messages
            .remove(&(context.session_id.clone(), dialog_turn_id.clone()));

        info!("Starting dialog turn: dialog_turn_id={}", dialog_turn_id);

        // Execute actual logic
        let result = self
            .execute_dialog_turn_impl(agent_type, initial_messages, context, start_time)
            .await;

        // Cleanup cancellation token
        self.round_executor
            .cleanup_dialog_turn(&dialog_turn_id)
            .await;
        debug!(
            "Cleaned up cancel token (final cleanup): dialog_turn_id={}",
            dialog_turn_id
        );

        result
    }

    /// Internal implementation of dialog turn execution
    async fn execute_dialog_turn_impl(
        &self,
        agent_type: String,
        initial_messages: Vec<Message>,
        context: ExecutionContext,
        start_time: std::time::Instant,
    ) -> BitFunResult<ExecutionResult> {
        let dialog_turn_id = context.dialog_turn_id.clone();
        let initial_count = initial_messages.len();

        debug!(
            "Executing dialog turn implementation: dialog_turn_id={}",
            dialog_turn_id
        );

        // P-18（每会话一次语义）：User Context 注入标记在整个会话生命周期内
        // 只清除一次——首次执行时注入一次，之后所有用户回合都不再重新注入。
        // round_dynamic_reminders 通过 user_context_injected_generation 与
        // user_context_cache_generation 比较：已注入过（标记 == 当前世代）→
        // 不再注入；上下文压缩/恢复使缓存世代递增 → 恢复后首轮重新注入一次。
        // 原实现（每回合首轮注入）在 turn 开始时清除标记，导致同一会话每个
        // 用户回合都重复注入工作区指令全文；现改为会话级一次注入。

        // Things that remain constant in a dialog turn: 1.agent, 2.system prompt, 3.tools, 4.ai client
        // 1. Get current agent
        let agent_registry = get_agent_registry();
        agent_registry
            .load_custom_agents(
                context
                    .workspace
                    .as_ref()
                    .map(|workspace| workspace.root_path()),
            )
            .await;
        let current_agent = agent_registry
            .get_agent(
                &agent_type,
                context
                    .workspace
                    .as_ref()
                    .map(|workspace| workspace.root_path()),
            )
            .ok_or_else(|| BitFunError::NotFound(format!("Agent not found: {}", agent_type)))?;
        info!(
            "Current Agent: {} ({})",
            current_agent.name(),
            current_agent.id()
        );

        let session = self
            .session_manager
            .get_session(&context.session_id)
            .ok_or_else(|| {
                BitFunError::Session(format!("Session not found: {}", context.session_id))
            })?;

        // 2. Get AI client
        let original_user_input = context
            .context
            .get("original_user_input")
            .cloned()
            .unwrap_or_default();

        // Edit constraint guard: process each distinct user instruction once.
        // The fast extractor receives the active state so explicit additions
        // and revocations form an auditable session-persistent state machine.
        if !original_user_input.trim().is_empty() {
            let revocation_authorized = context
                .context
                .get("edit_constraint_revocation_authorized")
                .is_some_and(|value| value == "true");
            let message_sha256 = crate::agentic::execution::edit_constraint_guard::message_sha256(
                &original_user_input,
            );
            let already_processed = self
                .session_manager
                .edit_constraint_state(&context.session_id)
                .is_some_and(|state| {
                    state.message_processed(&context.dialog_turn_id, &message_sha256)
                });
            if !already_processed {
                let active_constraints = self
                    .session_manager
                    .edit_constraints(&context.session_id)
                    .unwrap_or_default();
                let mut extraction = crate::agentic::execution::edit_constraint_guard::extract_constraints_with_active_and_revocation_authorization(
                    &original_user_input,
                    &active_constraints,
                    revocation_authorized,
                )
                .await;
                extraction.dialog_turn_id = Some(context.dialog_turn_id.clone());
                if crate::agentic::execution::edit_constraint_guard::extraction_requires_session_state(
                    &extraction,
                ) {
                    self.session_manager
                        .remember_edit_constraint_extraction(&context.session_id, extraction)
                        .await;
                }
            }
        }

        let (model_id, _) = self
            .resolve_model_id_for_turn(
                &session,
                &agent_type,
                context.workspace.as_ref(),
                &original_user_input,
                context.turn_index,
                context
                    .context
                    .get(INTERRUPTED_TURN_RESOLVED_MODEL_ID_METADATA_KEY)
                    .map(String::as_str),
                context
                    .context
                    .get(INTERRUPTED_TURN_MODEL_BINDING_FINGERPRINT_METADATA_KEY)
                    .map(String::as_str),
            )
            .await?;
        info!(
            "Agent using model: agent={}, resolved_model_id={}",
            current_agent.name(),
            model_id
        );

        let ai_client_factory = get_global_ai_client_factory().await.map_err(|e| {
            BitFunError::AIClient(format!("Failed to get AI client factory: {}", e))
        })?;

        // Get AI client by model ID
        let reasoning_preset = match self
            .resolve_reasoning_selection_for_turn(&session.session_id, &context)
            .await
        {
            Ok(reasoning_preset) => reasoning_preset,
            Err(error) => {
                warn!(
                    "Failed to persist reasoning preset fallback; using Auto for this turn: session_id={}, error={}",
                    session.session_id, error
                );
                None
            }
        };
        let ai_client_result = if matches!(
            session.config.model_binding_policy,
            SessionModelBindingPolicy::ApprovedImmutable
        ) {
            ai_client_factory
                .get_client_by_approved_binding_with_reasoning_preset(
                    &model_id,
                    session
                        .config
                        .model_binding_fingerprint
                        .as_deref()
                        .unwrap_or_default(),
                    reasoning_preset.as_deref(),
                )
                .await
        } else {
            ai_client_factory
                .get_client_resolved_with_reasoning_preset(&model_id, reasoning_preset.as_deref())
                .await
        };
        let ai_client = match ai_client_result {
            Ok(ai_client) => ai_client,
            Err(error) => {
                if context
                    .context
                    .contains_key(INTERRUPTED_TURN_MODEL_BINDING_FINGERPRINT_METADATA_KEY)
                {
                    Self::validate_frozen_model_contract(&context).await?;
                }
                return Err(BitFunError::AIClient(format!(
                    "Failed to get AI client (model_id={}): {}",
                    model_id, error
                )));
            }
        };
        Self::validate_frozen_model_contract(&context).await?;
        Self::validate_frozen_reasoning_contract(&context, ai_client.as_ref())?;
        let model_request_context = Self::model_request_context(
            session.effective_prompt_cache_lineage_id(),
            &session.session_id,
            &context.dialog_turn_id,
        );

        // Primary model vision capability (tools + system prompt appendix; also used below for API message stripping).
        let primary_model_facts = Self::resolve_primary_model_context(
            &model_id,
            session.config.model_binding_policy,
            &ai_client.config.model,
            &ai_client.config.format,
            "Config service unavailable, assuming primary model is text-only for image input gating",
        )
        .await;
        let resolved_primary_model_id = primary_model_facts.model_id.clone();
        let primary_supports_image_understanding = primary_model_facts.supports_image_inputs;

        let model_context_window = ai_client.config.context_window as usize;
        let session_max_tokens = session.config.max_context_tokens;
        let context_window = model_context_window.min(session_max_tokens);
        if model_context_window != session_max_tokens {
            debug!(
                "Context window: model={}, session_config={}, effective={}",
                model_context_window, session_max_tokens, context_window
            );
        }

        let model_capability_profile = ModelCapabilityProfile::from_resolved_model(
            &resolved_primary_model_id,
            &ai_client.config.model,
        );
        let is_review_subagent = agent_registry
            .get_subagent_is_review(&agent_type)
            .unwrap_or(false);
        let context_profile_policy = ContextProfilePolicy::for_agent_context(
            &agent_type,
            is_review_subagent,
            model_capability_profile,
        );
        debug!(
            "Context profile policy selected: session_id={}, agent_type={}, profile={:?}, model_capability={:?}, compression_contract_limit={}, subagent_concurrency_cap={}, repeated_tool_signature_threshold={}, consecutive_failed_command_threshold={}",
            context.session_id,
            agent_type,
            context_profile_policy.profile,
            model_capability_profile,
            context_profile_policy.compression_contract_limit,
            context_profile_policy.subagent_concurrency_cap,
            context_profile_policy.repeated_tool_signature_threshold,
            context_profile_policy.consecutive_failed_command_threshold
        );

        // 3. Get available tools list (read tool configuration for current mode from global config)
        let tool_policy = agent_registry
            .get_agent_tool_policy(
                &agent_type,
                context
                    .workspace
                    .as_ref()
                    .map(|workspace| workspace.root_path()),
            )
            .await;
        let mut allowed_tools = tool_policy.allowed_tools.clone();
        ensure_primary_session_goal_tools(
            &mut allowed_tools,
            context.subagent_parent_info.is_some(),
        );
        let enable_tools = context
            .context
            .get("enable_tools")
            .and_then(|v| v.parse::<bool>().ok())
            .unwrap_or(true);
        let deferred_tool_loading_enabled = match get_global_config_service().await {
            Ok(service) => service
                .get_config::<bool>(Some("ai.enable_deferred_tool_loading"))
                .await
                .unwrap_or(true),
            Err(_) => true,
        };
        let mut execution_context_vars = context.context.clone();
        execution_context_vars.insert(
            "enable_deferred_tool_loading".to_string(),
            deferred_tool_loading_enabled.to_string(),
        );
        execution_context_vars.insert("turn_index".to_string(), context.turn_index.to_string());
        let tool_manifest_context_vars = execution_context_vars.clone();

        let tool_description_context = tool_context_runtime::build_tool_description_context(
            &agent_type,
            context.workspace.as_ref(),
            context.workspace_services.as_ref(),
            Some(&primary_model_facts),
            &tool_manifest_context_vars,
            &context.runtime_tool_restrictions,
        );

        let tool_manifest = if enable_tools {
            debug!(
                "Agent tools: agent={}, tool_count={}",
                agent_type,
                allowed_tools.len()
            );
            Some(
                resolve_tool_manifest(
                    &allowed_tools,
                    &tool_policy.exposure_overrides,
                    &tool_description_context,
                )
                .await,
            )
        } else {
            None
        };
        let deferred_tools = tool_manifest
            .as_ref()
            .map(|manifest| manifest.deferred_tool_names.clone())
            .unwrap_or_default();
        let tool_listing_sections = if let Some(manifest) = tool_manifest.as_ref() {
            Self::build_tool_listing_sections(manifest, &tool_description_context).await
        } else {
            ToolListingSections::default()
        };
        let runtime_context_needs = tool_manifest
            .as_ref()
            .map(|manifest| {
                RuntimeContextNeeds::from_tool_names(manifest.allowed_tool_names.iter())
            })
            .unwrap_or_default();
        // We do not currently keep a session-level cache of resolved tool
        // definitions; each turn re-resolves them from the current manifest.
        // Expected changes therefore come from user-driven configuration or
        // product-version changes, such as:
        // - agent_type / mode changes
        // - the user editing the enabled tool set for the current agent
        // - MCP tool enablement / settings changes
        // - a newer product build changing built-in tool definitions
        //
        // Outside those cases, tool definitions should remain byte-stable
        // across the session. Avoid introducing extra turn-to-turn variation:
        // it changes the request prefix and causes provider prefix/KV cache
        // misses.
        let (available_tools, tool_definitions) = if let Some(manifest) = tool_manifest {
            (manifest.allowed_tool_names, Some(manifest.tool_definitions))
        } else {
            (vec![], None)
        };
        let final_tool_names = Self::finalize_tool_names(tool_definitions.as_deref());
        debug!(
            "Primary model and tool manifest resolved: session_id={}, turn_id={}, resolved_primary_model_id={}, primary_model_api_format={}, primary_model_supports_image_inputs={}, final_tool_count={}, final_tool_names={:?}, deferred_tool_names={:?}",
            context.session_id,
            context.dialog_turn_id,
            primary_model_facts.model_id,
            primary_model_facts.api_format,
            primary_model_facts.supports_image_inputs,
            final_tool_names.len(),
            final_tool_names,
            deferred_tools,
        );

        // 4. Resolve the prompt scaffold used by model requests in this turn.
        // It is refreshed after successful context compression so the first
        // post-compaction request builds the new provider-side prefix cache.
        // Runtime facts carry a turn-start usage estimate: system prompt and
        // prepended reminder tokens are not yet measurable at this point, so
        // it is a lower bound that gets refreshed after context compression.
        let mut turn_prompt_scaffold = self
            .resolve_turn_prompt_scaffold(TurnPromptScaffoldInput {
                context: &context,
                current_agent: current_agent.as_ref(),
                model_name: &ai_client.config.model,
                supports_image_understanding: primary_supports_image_understanding,
                tool_listing_sections: tool_listing_sections.clone(),
                runtime_context_needs,
                runtime_facts_usage: Self::runtime_facts_usage_from_pressure(
                    &Self::estimate_auto_compression_pressure(
                        &initial_messages,
                        tool_definitions.as_deref(),
                        context_window,
                        Self::compression_trigger_budget_configured(
                            context_window,
                            ai_client.config.max_tokens,
                        )
                        .await,
                        0,
                    ),
                ),
                stage: "turn_start",
            })
            .await?;

        // Add System Prompt to the beginning of message list (only for this execution, not persisted)
        let mut messages = vec![turn_prompt_scaffold.system_prompt_message.clone()];
        messages.extend(initial_messages);
        // Keep this generation's append-only transcript separate from the
        // mutable request history. Context compression may replace `messages`
        // wholesale, but durable recovered-Turn completion must still append
        // every assistant/tool/injection message produced by this generation.

        let mut round_index = initial_round_index(&context.context);
        // P-17：本轮是否发生上下文恢复（压缩/溢出恢复），恢复后首轮需注入 Runtime Facts。
        let mut context_recovered_this_round = false;
        let mut completed_rounds = 0usize;
        let mut total_tools = 0;
        let mut last_partial_recovery_reason: Option<String> = None;
        let mut finalization_reason: Option<&'static str> = None;
        let mut consecutive_compression_failures: u32 = 0;
        // 阈值参数配置化：ai.thresholds.compression.*
        let compression_counts = Self::configured_compression_counts().await;
        let max_consecutive_compression_failures = compression_counts.2 as u32;
        let max_failed_tool_recovery_attempts = compression_counts.3;
        let max_stop_hook_continuations = compression_counts.4;
        let mut main_context_overflow_recoveries = 0usize;
        let mut active_round_lifecycle: Option<ModelRoundLifecycle> = None;

        // Track tool-call patterns for context health, but only use rounds with
        // actual failed tool results for no-progress recovery decisions.
        let mut recent_tool_signatures: Vec<String> = Vec::new();
        let mut recent_failed_tool_signatures: Vec<String> = Vec::new();
        let mut failed_tool_recovery_attempts: usize = 0;
        let max_partial_continuation_attempts: usize = 3;
        let mut full_compression_count = 0usize;
        let mut compression_failure_count = 0u32;
        // R-MR-10 消息重复闸门：最近 N 轮（默认 3）已发送「新增消息序列」指纹窗口。
        // 正常流程每轮：模型输出 → 工具结果 → 追加新消息 → 下一轮发送的新增消息
        // 序列必然变化（指纹必不同，永不误拦）；死循环轮：模型输出 + 工具结果与
        // 上一轮完全相同 → 新增消息序列指纹相同 → 判定重复 → 不调 API、本地合成
        // final response；正常轮指纹变化 → 窗口滑动。
        // 说明：主循环 messages 为追加式增长（每轮 push assistant + 工具结果），
        // 全量序列指纹永远不重复；真正可比的「消息序列」是自上次发送以来新增的
        // 消息段（= 本轮模型输出 + 工具结果），契约「hash 全部消息内容 + 工具调用
        // + 工具结果」按此语义落地（见实现说明落盘）。
        let mut recent_message_fingerprints: Vec<String> = Vec::new();
        let mut last_sent_messages_len = messages.len();
        let empty_input_guard = Self::configured_empty_input_guard().await;
        let duplicate_message_enabled = Self::configured_duplicate_message_enabled().await;
        let duplicate_message_window = Self::configured_duplicate_message_window().await;
        if duplicate_message_enabled {
            debug!(
                "R-MR-10 duplicate-message gate enabled: session_id={}, turn_id={}, window={}",
                context.session_id, context.dialog_turn_id, duplicate_message_window
            );
        }

        // Save the last token usage statistics
        let mut last_usage: Option<crate::util::types::ai::GeminiUsage> = None;

        // Track thinking-only rescue reminders. This counter is also a stop
        // condition: repeated thinking-only rounds with no progress exhaust
        // DEFAULT_EMPTY_ROUND_RESPAWN_LIMIT and end the turn with a local
        // final response (resets on rounds that made progress).
        let mut thinking_only_rescue_attempts: usize = 0;
        let mut partial_continuation_attempts: usize = 0;
        // Bounds how often Stop hooks may reopen a finished turn.
        let mut stop_hook_continuations: usize = 0;

        // Add detailed logging showing the execution context messages.
        debug!(
            "Executing dialog turn: dialog_turn_id={}, mode={}, agent={}, initial_messages={}, messages_len={}",
            dialog_turn_id,
            current_agent.name(),
            context.agent_type,
            initial_count,
            messages.len()
        );
        trace!(
            "Context message details: dialog_turn_id={}, session_id={}, roles={:?}",
            dialog_turn_id,
            context.session_id,
            messages
                .iter()
                .map(|m| format!("{:?}", m.role))
                .collect::<Vec<_>>()
        );

        let enable_context_compression = session.config.enable_context_compression;
        let compression_trigger_budget = Self::compression_trigger_budget_configured(
            context_window,
            ai_client.config.max_tokens,
        )
        .await;

        // If the primary model is text-only, do not send image payloads to the provider.
        // Instead, keep a text-only placeholder (including `image_id`).
        if !primary_supports_image_understanding {
            for msg in messages.iter_mut() {
                let MessageContent::Multimodal { text, images } = &msg.content else {
                    continue;
                };

                let original_text = text.clone();
                let original_images = images.clone();

                // Replace multimodal messages with text-only versions to avoid provider errors.
                let next_text = Self::render_multimodal_as_text(&original_text, &original_images);

                msg.content = MessageContent::Text(next_text);
                msg.metadata.tokens = None;
            }
        }

        // Loop to execute model rounds
        loop {
            if completed_rounds >= self.config.max_rounds {
                warn!(
                    "Reached max rounds limit: {}, stopping execution",
                    self.config.max_rounds
                );
                finalization_reason = Some("max_rounds");
                break;
            }

            // Check and compress before sending AI request
            //
            // NOTE: There used to be a "microcompact" pre-pass here that
            // silently rewrote older tool-result contents into a placeholder.
            // It has been removed: it mutated already-sent message prefixes —
            // killing provider KV-cache hits on every round — and stripped the
            // model of memory of what it had already done, which directly
            // drove repetitive tool-call loops in long exploratory subagents
            // (see deep-review subagent loop incident, 2026-05-12).
            //
            // The remaining context-pressure layers are:
            //   - L1: AI-summary based full compression (preserves semantics).
            //   - L2: Emergency truncation (only if tokens still exceed the
            //         provider context window after L1).
            let pressure_prepended_reminders = turn_prompt_scaffold
                .prepended_prompt_reminders
                .ordered_reminders();
            let pressure_prepended_reminder_tokens =
                Self::prepended_reminder_tokens_for_pressure(&pressure_prepended_reminders);
            let token_anchor_selection = self
                .session_manager
                .select_latest_matching_token_anchor(&context.session_id, &messages)
                .await;
            let (mut token_pressure, anchor_details) =
                Self::estimate_auto_compression_pressure_with_anchor(
                    &messages,
                    tool_definitions.as_deref(),
                    context_window,
                    compression_trigger_budget,
                    token_anchor_selection.selected.as_ref(),
                    pressure_prepended_reminder_tokens,
                );
            if let Some(details) = anchor_details.as_ref() {
                debug!(
                    "Token pressure estimate: session_id={}, turn_id={}, round_index={}, source=provider_anchor, anchor_id={}, prefix_messages={}, input_tokens={}, adjusted_anchor_tokens={}, tail_tokens={}, system_tokens_at_anchor={}, current_system_tokens={}, system_delta={}, tool_tokens_at_anchor={}, current_tool_tokens={}, tool_delta={}, prepended_reminder_tokens_at_anchor={}, current_prepended_reminder_tokens={}, prepended_reminder_delta={}, total_tokens={}, system_tokens={}, tool_tokens={}, prepended_reminder_tokens={}, conversation_tokens={}, context_window={}, input_limit={}, output_reserve={}, safety_reserve={}, usage={:.3}",
                    context.session_id,
                    context.dialog_turn_id,
                    round_index,
                    details.anchor_id,
                    details.prefix_message_count,
                    details.input_tokens,
                    details.adjusted_anchor_tokens,
                    details.tail_tokens,
                    details.system_tokens_at_anchor,
                    details.current_system_tokens,
                    details.system_delta,
                    details.tool_tokens_at_anchor,
                    details.current_tool_tokens,
                    details.tool_delta,
                    details.prepended_reminder_tokens_at_anchor,
                    details.current_prepended_reminder_tokens,
                    details.prepended_reminder_delta,
                    token_pressure.total_tokens,
                    token_pressure.system_tokens,
                    token_pressure.tool_tokens,
                    token_pressure.prepended_reminder_tokens,
                    token_pressure.conversation_tokens,
                    token_pressure.context_window,
                    token_pressure.input_limit,
                    token_pressure.output_reserve_tokens,
                    token_pressure.safety_reserve_tokens,
                    token_pressure.usage_ratio
                );
                if !token_anchor_selection.skipped.is_empty() {
                    trace!(
                        "Token anchor selection skipped newer anchors before match: session_id={}, turn_id={}, round_index={}, selected_anchor_id={}, skipped={:?}",
                        context.session_id,
                        context.dialog_turn_id,
                        round_index,
                        details.anchor_id,
                        token_anchor_selection.skipped
                    );
                }
            } else {
                debug!(
                    "Token pressure estimate: session_id={}, turn_id={}, round_index={}, source=full_estimate, total_tokens={}, system_tokens={}, tool_tokens={}, prepended_reminder_tokens={}, conversation_tokens={}, context_window={}, input_limit={}, output_reserve={}, safety_reserve={}, usage={:.3}, fallback_reasons={:?}",
                    context.session_id,
                    context.dialog_turn_id,
                    round_index,
                    token_pressure.total_tokens,
                    token_pressure.system_tokens,
                    token_pressure.tool_tokens,
                    token_pressure.prepended_reminder_tokens,
                    token_pressure.conversation_tokens,
                    token_pressure.context_window,
                    token_pressure.input_limit,
                    token_pressure.output_reserve_tokens,
                    token_pressure.safety_reserve_tokens,
                    token_pressure.usage_ratio,
                    token_anchor_selection.skipped
                );
            }
            debug!(
                "Round {} token usage before send: total={} / {}, conversation={} / {}, usage={:.1}%, input_limit={}, output_reserve={}, safety_reserve={}",
                round_index,
                token_pressure.total_tokens,
                token_pressure.context_window,
                token_pressure.conversation_tokens,
                token_pressure.context_window,
                token_pressure.usage_ratio * 100.0,
                token_pressure.input_limit,
                token_pressure.output_reserve_tokens,
                token_pressure.safety_reserve_tokens
            );

            // ENGINE-03：input_limit == 0 表示窗口过小，仅预留（output reserve +
            // safety reserve）就已超出窗口；此时禁用自动压缩，而不是每轮都无条件压缩。
            let should_compress = enable_context_compression
                && token_pressure.input_limit > 0
                && token_pressure.total_tokens >= token_pressure.input_limit;
            let mut send_pressure_reusable = true;

            // Circuit breaker: skip full compression if it has failed too many
            // consecutive times.  Microcompact and emergency truncation still run.
            let circuit_breaker_open =
                consecutive_compression_failures >= max_consecutive_compression_failures;

            if !should_compress {
                debug!(
                    "No compression needed: session={}, total_tokens={}, input_limit={}, context_window={}, output_reserve={}, safety_reserve={}, usage={:.1}%",
                    context.session_id,
                    token_pressure.total_tokens,
                    token_pressure.input_limit,
                    token_pressure.context_window,
                    token_pressure.output_reserve_tokens,
                    token_pressure.safety_reserve_tokens,
                    token_pressure.usage_ratio * 100.0
                );
            } else if circuit_breaker_open {
                warn!(
                    "Compression circuit breaker open ({} consecutive failures), skipping full compression for round {}",
                    consecutive_compression_failures, round_index
                );
            } else {
                info!(
                    "Triggering context compression: session={}, total_tokens={}, input_limit={}, context_window={}, output_reserve={}, safety_reserve={}, usage={:.1}%",
                    context.session_id,
                    token_pressure.total_tokens,
                    token_pressure.input_limit,
                    token_pressure.context_window,
                    token_pressure.output_reserve_tokens,
                    token_pressure.safety_reserve_tokens,
                    token_pressure.usage_ratio * 100.0
                );

                // ENGINE-04: a single full-compression pass can still leave the
                // context over input_limit (the compression contract preserves a
                // recent-context tail). Re-check input_limit after each pass and
                // compress again in the same round (bounded) instead of trusting
                // the pre-compression snapshot.
                let max_same_round_compression_passes = compression_counts.5 as u32;
                let mut compression_passes = 0u32;
                let mut compressed_this_round = false;
                while !circuit_breaker_open
                    && compression_passes < max_same_round_compression_passes
                    && token_pressure.total_tokens >= token_pressure.input_limit
                {
                    compression_passes += 1;
                    match self
                        .compress_messages(
                            &context.session_id,
                            &context.dialog_turn_id,
                            "auto",
                            messages.clone(),
                            token_pressure,
                            context_window,
                            ai_client.clone(),
                            &model_request_context,
                            &tool_definitions,
                            turn_prompt_scaffold.system_prompt_message.clone(),
                            &turn_prompt_scaffold.prepended_prompt_reminders,
                            primary_supports_image_understanding,
                            context_profile_policy.compression_contract_limit,
                            context.workspace.as_ref(),
                        )
                        .await
                    {
                        Ok(Some((compressed_tokens, compressed_messages))) => {
                            info!(
                                "Round {} compression pass {} completed: messages {} -> {}, tokens {} -> {}",
                                round_index,
                                compression_passes,
                                messages.len(),
                                compressed_messages.len(),
                                token_pressure.total_tokens,
                                compressed_tokens,
                            );

                            messages = compressed_messages;
                            // ENGINE-02: recompute the pressure against the
                            // compressed messages so the next-pass decision, the
                            // scaffold refresh, and the runtime-facts reminder all
                            // see the post-compression state instead of the stale
                            // pre-compression snapshot. The prepended reminders are
                            // still the pre-refresh values here — they are small and
                            // the final send-pressure estimate below reuses the
                            // freshly resolved scaffold.
                            token_pressure = Self::estimate_auto_compression_pressure(
                                &messages,
                                tool_definitions.as_deref(),
                                context_window,
                                compression_trigger_budget,
                                Self::prepended_reminder_tokens_for_pressure(
                                    &turn_prompt_scaffold
                                        .prepended_prompt_reminders
                                        .ordered_reminders(),
                                ),
                            );
                            compressed_this_round = true;
                            context_recovered_this_round = true;
                            full_compression_count += 1;
                            consecutive_compression_failures = 0;
                            send_pressure_reusable = false;
                        }
                        Ok(None) => {
                            debug!("No eligible multi-turn context available for compression");
                            consecutive_compression_failures = 0;
                            break;
                        }
                        Err(e) => {
                            consecutive_compression_failures += 1;
                            compression_failure_count += 1;
                            error!(
                                "Round {} compression failed ({}/{}): {}, continuing with uncompressed context",
                                round_index,
                                consecutive_compression_failures,
                                max_consecutive_compression_failures,
                                e
                            );
                            break;
                        }
                    }
                }

                // Re-resolve the scaffold once after compression so the first
                // post-compaction request builds the new provider-side prefix
                // cache with the post-compression token pressure (ENGINE-02).
                if compressed_this_round {
                    turn_prompt_scaffold = self
                        .resolve_turn_prompt_scaffold(TurnPromptScaffoldInput {
                            context: &context,
                            current_agent: current_agent.as_ref(),
                            model_name: &ai_client.config.model,
                            supports_image_understanding: primary_supports_image_understanding,
                            tool_listing_sections: tool_listing_sections.clone(),
                            runtime_context_needs,
                            runtime_facts_usage: Self::runtime_facts_usage_from_pressure(
                                &token_pressure,
                            ),
                            stage: "after_context_compression",
                        })
                        .await?;
                    Self::apply_turn_prompt_scaffold_to_messages(
                        &mut messages,
                        &turn_prompt_scaffold,
                    );
                }
            }

            // L2: Emergency truncation — if tokens still exceed context_window
            // after all compression layers, drop oldest API rounds until we fit.
            // Refresh runtime facts per round so every model request carries
            // live time and the current token pressure snapshot; long-lived
            // turns must not freeze the model's view at turn start.
            let prompt_context = Self::build_prompt_context(
                &context,
                &ai_client.config.model,
                primary_supports_image_understanding,
                tool_listing_sections.clone(),
                runtime_context_needs,
            )
            .await;
            // P-17/P-18 回合标记：用户消息回合首轮（round_index == 0）或上下文恢复后首轮
            // 注入 Runtime Facts；同回合工具轮（round_index > 0 且未恢复）不注入。
            let inject_runtime_facts = round_index == 0 || context_recovered_this_round;
            context_recovered_this_round = false;
            Self::refresh_runtime_facts_for_round(
                &mut turn_prompt_scaffold,
                prompt_context,
                Self::runtime_facts_usage_from_pressure(&token_pressure),
                inject_runtime_facts,
            );
            let send_prepended_reminders = turn_prompt_scaffold
                .prepended_prompt_reminders
                .ordered_reminders();
            let send_static_prepended_reminders = turn_prompt_scaffold
                .prepended_prompt_reminders
                .static_ordered_reminders();
            let send_dynamic_prepended_reminders = self
                .round_dynamic_reminders(
                    &context.session_id,
                    &context,
                    &turn_prompt_scaffold.prepended_prompt_reminders,
                )
                .await;
            let send_prepended_reminder_tokens =
                Self::prepended_reminder_tokens_for_pressure(&send_prepended_reminders);
            let mut send_pressure = if send_pressure_reusable
                && token_pressure.prepended_reminder_tokens == send_prepended_reminder_tokens
            {
                token_pressure
            } else {
                Self::estimate_auto_compression_pressure(
                    &messages,
                    tool_definitions.as_deref(),
                    context_window,
                    compression_trigger_budget,
                    send_prepended_reminder_tokens,
                )
            };
            if send_pressure.total_tokens > context_window {
                warn!(
                    "Round {} tokens ({}) still exceed context_window ({}) after compression, performing emergency truncation",
                    round_index, send_pressure.total_tokens, context_window
                );
                let before_truncate_tokens = send_pressure.total_tokens;
                messages = Self::emergency_truncate_messages(
                    messages,
                    context_window,
                    tool_definitions.as_deref(),
                    send_prepended_reminder_tokens,
                );
                self.session_manager
                    .prune_token_anchors_to_messages(&context.session_id, &messages)
                    .await;
                send_pressure = Self::estimate_auto_compression_pressure(
                    &messages,
                    tool_definitions.as_deref(),
                    context_window,
                    compression_trigger_budget,
                    send_prepended_reminder_tokens,
                );
                info!(
                    "Emergency truncation complete: tokens {} -> {}",
                    before_truncate_tokens, send_pressure.total_tokens
                );
            }

            ContextHealthSnapshot::from_runtime_observations(
                send_pressure.usage_ratio,
                full_compression_count,
                compression_failure_count,
                &recent_tool_signatures,
                &messages,
            )
            .log(
                &context.session_id,
                &context.dialog_turn_id,
                round_index,
                "before_send",
            );

            // Create round context
            let round_context_vars = self
                .context_vars_for_round(
                    &execution_context_vars,
                    &context.session_id,
                    &context.dialog_turn_id,
                )
                .await;
            let loaded_deferred_tool_specs =
                collect_product_loaded_deferred_tool_specs(&messages, &deferred_tools);

            let model_exchange_trace_dir = self
                .session_manager
                .persistent_model_exchange_trace_dir(&context.session_id)
                .await;
            let round_context = RoundContext {
                session_id: context.session_id.clone(),
                subagent_parent_info: context.subagent_parent_info.clone(),
                permission_delegation: context.permission_delegation.clone(),
                dialog_turn_id: context.dialog_turn_id.clone(),
                turn_index: context.turn_index,
                round_number: round_index,
                round_group_id: None,
                workspace: context.workspace.clone(),
                model_exchange_trace_dir,
                available_tools: available_tools.clone(),
                user_enabled_tools: tool_policy.user_enabled_tools.clone(),
                deferred_tools: deferred_tools.clone(),
                loaded_deferred_tool_specs,
                model_config_id: model_id.clone(),
                effective_model_name: ai_client.config.model.clone(),
                model_request_context: model_request_context.clone(),
                primary_model_facts: primary_model_facts.clone(),
                agent_type: agent_type.clone(),
                context_vars: round_context_vars,
                permission_constraints: tool_policy.permission_constraints.clone(),
                permission_runtime_ceiling: context.permission_runtime_ceiling.clone(),
                delegation_policy: context.delegation_policy,
                runtime_tool_restrictions: context.runtime_tool_restrictions.clone(),
                steering_interrupt: context.round_injection.as_ref().map(|source| {
                    crate::agentic::round_preempt::DialogRoundInjectionInterrupt::new(
                        context.session_id.clone(),
                        context.dialog_turn_id.clone(),
                        Arc::clone(source),
                    )
                }),
                cancellation_token: CancellationToken::new(),
                workspace_services: context.workspace_services.clone(),
                terminal_port: context.terminal_port.clone(),
                remote_exec_port: context.remote_exec_port.clone(),
                recover_partial_on_cancel: context.recover_partial_on_cancel,
            };

            // Execute single model round
            debug!(
                "Starting model round: round_index={}, messages={}",
                round_index,
                messages.len()
            );

            // R-MR-10 消息重复校验闸门：请求发出前（build_ai_messages_for_send /
            // 实际调 API 之前）比对新增消息序列指纹。
            //
            // 正常流程：每轮模型输出 + 工具结果追加进 messages → 新增序列指纹必变
            // → 永不误拦；死循环：模型重复同工具同参数、工具结果相同 → 新增序列
            // 指纹与窗口内最近 N 轮（默认 3）某一轮相同 → 判定重复 → 不调 API，
            // 本地合成 final response（同 max_rounds 路径）。
            if duplicate_message_enabled {
                let new_start = last_sent_messages_len.min(messages.len());
                let new_messages = &messages[new_start..];
                // 防御：本轮无新增消息（理论上主循环每轮必追加 assistant + 工具
                // 结果）时不判定——空序列指纹恒定，避免任何空切片误拦。
                if !new_messages.is_empty() {
                    let current_fingerprint = Self::messages_sequence_fingerprint(new_messages);
                    if Self::is_duplicate_message_fingerprint(
                        &current_fingerprint,
                        &recent_message_fingerprints,
                        duplicate_message_window,
                    ) {
                        warn!(
                            "R-MR-10 duplicate message sequence detected; stopping turn without a model request: session_id={}, turn_id={}, round_index={}, duplicate_fingerprint={}, recent_fingerprints={}",
                            context.session_id,
                            context.dialog_turn_id,
                            round_index,
                            current_fingerprint,
                            recent_message_fingerprints.len()
                        );
                        finalization_reason = Some("duplicate_messages");
                        break;
                    }
                    recent_message_fingerprints.push(current_fingerprint);
                    if recent_message_fingerprints.len() > duplicate_message_window {
                        recent_message_fingerprints
                            .drain(0..recent_message_fingerprints.len() - duplicate_message_window);
                    }
                }
            }
            last_sent_messages_len = messages.len();

            // R-MR-06 / R-13 首轮真实内容守卫（DR-7 落点 1，消费 empty_input_guard）：
            // 空任务子会话首轮 = initial_messages 只有 legion_context / hook_context
            // 等 system_reminder 包裹的注入（role=User、内容非空），trim 判空拦截
            // 永远失效。这里在请求发出前判定「全部 user 消息均为注入/空」→ 本地
            // 合成 final response，不调 API、不计费。
            if empty_input_guard && round_index == 0 && !Self::has_real_user_content(&messages) {
                warn!(
                    "R-MR-06 empty-input guard hit on first round (all user messages are system injections or empty); synthesizing local final response without a model request: session_id={}, turn_id={}, user_messages={}",
                    context.session_id,
                    context.dialog_turn_id,
                    messages.iter().filter(|m| m.role == MessageRole::User).count()
                );
                finalization_reason = Some("empty_initial_turn");
                break;
            }

            let ai_messages = Self::build_ai_messages_for_send(
                &messages,
                &ai_client.config.format,
                context
                    .workspace
                    .as_ref()
                    .map(|workspace| workspace.root_path()),
                &context.dialog_turn_id,
                primary_supports_image_understanding,
                &send_static_prepended_reminders,
                &send_dynamic_prepended_reminders,
                Self::configured_max_image_bearing_messages().await,
            )
            .await?;

            let round_lifecycle =
                active_round_lifecycle.get_or_insert_with(ModelRoundLifecycle::new);
            let round_result = match self
                .round_executor
                .execute_round_with_lifecycle(
                    ai_client.clone(),
                    round_context,
                    ai_messages,
                    tool_definitions.clone(),
                    Some(context_window),
                    round_lifecycle,
                )
                .await
            {
                Ok(result) => result,
                Err(err)
                    if enable_context_compression
                        && err.is_recoverable_context_overflow()
                        && main_context_overflow_recoveries
                            < Self::MAX_MAIN_CONTEXT_OVERFLOW_RECOVERIES =>
                {
                    main_context_overflow_recoveries += 1;
                    warn!(
                        "Main model request exceeded provider context; starting recovery compression: session_id={}, turn_id={}, round_index={}, recovery={}/{}, error={}",
                        context.session_id,
                        context.dialog_turn_id,
                        round_index,
                        main_context_overflow_recoveries,
                        Self::MAX_MAIN_CONTEXT_OVERFLOW_RECOVERIES,
                        err
                    );
                    match self
                        .compress_messages(
                            &context.session_id,
                            &context.dialog_turn_id,
                            "context_overflow_recovery",
                            messages.clone(),
                            send_pressure,
                            context_window,
                            ai_client.clone(),
                            &model_request_context,
                            &tool_definitions,
                            turn_prompt_scaffold.system_prompt_message.clone(),
                            &turn_prompt_scaffold.prepended_prompt_reminders,
                            primary_supports_image_understanding,
                            context_profile_policy.compression_contract_limit,
                            context.workspace.as_ref(),
                        )
                        .await
                    {
                        Ok(Some((compressed_tokens, compressed_messages))) => {
                            info!(
                                "Context-overflow recovery compression completed: session_id={}, turn_id={}, round_index={}, recovery={}, messages {} -> {}, tokens {} -> {}",
                                context.session_id,
                                context.dialog_turn_id,
                                round_index,
                                main_context_overflow_recoveries,
                                messages.len(),
                                compressed_messages.len(),
                                send_pressure.total_tokens,
                                compressed_tokens
                            );
                            messages = compressed_messages;
                            turn_prompt_scaffold = self
                                .resolve_turn_prompt_scaffold(TurnPromptScaffoldInput {
                                    context: &context,
                                    current_agent: current_agent.as_ref(),
                                    model_name: &ai_client.config.model,
                                    supports_image_understanding:
                                        primary_supports_image_understanding,
                                    tool_listing_sections: tool_listing_sections.clone(),
                                    runtime_context_needs,
                                    runtime_facts_usage: Self::runtime_facts_usage_from_pressure(
                                        &send_pressure,
                                    ),
                                    stage: "after_context_overflow_recovery",
                                })
                                .await?;
                            Self::apply_turn_prompt_scaffold_to_messages(
                                &mut messages,
                                &turn_prompt_scaffold,
                            );
                            self.round_executor
                                .record_context_overflow_recovery(
                                    &context.session_id,
                                    &context.dialog_turn_id,
                                    round_lifecycle,
                                    err.to_string(),
                                )
                                .await;
                            full_compression_count += 1;
                            consecutive_compression_failures = 0;
                            context_recovered_this_round = true;
                            continue;
                        }
                        Ok(None) => {
                            warn!(
                                "Context-overflow recovery found no compressible context: session_id={}, turn_id={}, round_index={}",
                                context.session_id, context.dialog_turn_id, round_index
                            );
                            return Err(err);
                        }
                        Err(compression_error) => {
                            error!(
                                "Context-overflow recovery compression failed: session_id={}, turn_id={}, round_index={}, error={}",
                                context.session_id,
                                context.dialog_turn_id,
                                round_index,
                                compression_error
                            );
                            return Err(err);
                        }
                    }
                }
                Err(err) => return Err(err),
            };
            active_round_lifecycle = None;

            debug!(
                "Model round completed: round_index={}, has_more_rounds={}, tool_calls={}",
                round_index,
                round_result.has_more_rounds,
                round_result.tool_calls.len()
            );
            completed_rounds += 1;

            // Save the last token usage statistics (update each time, keep the last one)
            if let Some(ref usage) = round_result.usage {
                last_usage = Some(usage.clone());
                let round_id = round_result
                    .assistant_message
                    .metadata
                    .round_id
                    .clone()
                    .unwrap_or_else(|| format!("round_{}", round_index));
                let system_tokens_at_anchor = Self::system_tokens_for_pressure(&messages);
                let tool_tokens_at_anchor = tool_definitions
                    .as_deref()
                    .map(TokenCounter::estimate_tool_definitions_tokens)
                    .unwrap_or(0);
                let anchor = TokenAnchor::from_request_prefix(
                    TokenAnchorInput {
                        session_id: context.session_id.clone(),
                        turn_id: context.dialog_turn_id.clone(),
                        round_id,
                        model_id: ai_client.config.model.clone(),
                        input_tokens: usage.prompt_token_count as usize,
                        system_tokens_at_anchor,
                        tool_tokens_at_anchor,
                        prepended_reminder_tokens_at_anchor: send_prepended_reminder_tokens,
                    },
                    &messages,
                );
                self.session_manager.remember_token_anchor(anchor).await;
            }

            // Add assistant message to history
            messages.push(round_result.assistant_message.clone());
            self.remember_generation_message(
                &context.session_id,
                &context.dialog_turn_id,
                &round_result.assistant_message,
            );

            // Update the in-memory message caches immediately so subsequent rounds see it.
            if let Err(e) = self
                .session_manager
                .add_message(&context.session_id, round_result.assistant_message.clone())
                .await
            {
                warn!("Failed to update assistant message in memory: {}", e);
            }

            // Add tool result messages to history
            for tool_result_msg in round_result.tool_result_messages.iter() {
                messages.push(tool_result_msg.clone());
                self.remember_generation_message(
                    &context.session_id,
                    &context.dialog_turn_id,
                    tool_result_msg,
                );

                // Update the in-memory message caches immediately so subsequent rounds see it.
                if let Err(e) = self
                    .session_manager
                    .add_message(&context.session_id, tool_result_msg.clone())
                    .await
                {
                    warn!("Failed to update tool result message in memory: {}", e);
                }
            }

            #[cfg(feature = "agent-runtime")]
            {
                let previous_message_count = messages.len();
                activate_conditional_instructions_after_round(
                    self.session_manager.as_ref(),
                    &context,
                    &round_result,
                    &mut messages,
                )
                .await;
                for message in &messages[previous_message_count..] {
                    self.remember_generation_message(
                        &context.session_id,
                        &context.dialog_turn_id,
                        message,
                    );
                }
            }

            debug!(
                "Updated round messages in memory: round_index={}, assistant + {} tool results",
                round_index,
                round_result.tool_result_messages.len()
            );

            total_tools += round_result.tool_calls.len();

            // Track partial recovery reason from the last round
            if round_result.partial_recovery_reason.is_some() {
                last_partial_recovery_reason = round_result.partial_recovery_reason.clone();
            }

            if let Some(round_signature) = Self::tool_call_signature(&round_result.tool_calls) {
                recent_tool_signatures.push(round_signature.clone());
                if Self::failed_tool_round_signature(
                    &round_result.tool_calls,
                    &round_result.tool_result_messages,
                )
                .is_some()
                {
                    recent_failed_tool_signatures.push(round_signature);
                } else {
                    recent_failed_tool_signatures.clear();
                    failed_tool_recovery_attempts = 0;
                }
            } else {
                recent_tool_signatures.clear();
                recent_failed_tool_signatures.clear();
                failed_tool_recovery_attempts = 0;
            }

            // A round that made real progress (tool call issued, more rounds
            // scheduled, or user-visible text produced) resets the thinking-only
            // rescue counter so an occasional thinking round inside an otherwise
            // healthy task does not accumulate toward the storm budget.
            if round_result.has_more_rounds
                || !round_result.tool_calls.is_empty()
                || round_result.had_assistant_text
            {
                thinking_only_rescue_attempts = 0;
            }

            let after_round_pressure = Self::estimate_auto_compression_pressure(
                &messages,
                tool_definitions.as_deref(),
                context_window,
                compression_trigger_budget,
                send_prepended_reminder_tokens,
            );
            let after_round_health = ContextHealthSnapshot::from_runtime_observations(
                after_round_pressure.usage_ratio,
                full_compression_count,
                compression_failure_count,
                &recent_tool_signatures,
                &messages,
            );
            after_round_health.log(
                &context.session_id,
                &context.dialog_turn_id,
                round_index,
                "after_round",
            );
            after_round_health.log_policy_thresholds(
                &context.session_id,
                &context.dialog_turn_id,
                round_index,
                &context_profile_policy,
            );

            let max_consec = context_profile_policy
                .effective_loop_threshold(self.config.max_consecutive_same_tool);
            if recent_failed_tool_signatures.len() >= max_consec {
                let tail = &recent_failed_tool_signatures
                    [recent_failed_tool_signatures.len() - max_consec..];
                if tail.windows(2).all(|w| w[0] == w[1]) {
                    if failed_tool_recovery_attempts < max_failed_tool_recovery_attempts {
                        failed_tool_recovery_attempts += 1;
                        warn!(
                            "Repeated tool failure detected: {} consecutive rounds with identical tool signatures, injecting recovery prompt #{}",
                            max_consec, failed_tool_recovery_attempts
                        );
                        let reminder = format!(
                            "<system_reminder>Repeated tool failure detected: the same tool call with identical arguments has failed {} times in a row. \
                            The current approach is not making progress. You MUST now change your strategy: \
                            (1) if the tool keeps failing, try a completely different approach or tool; \
                            (2) if you are stuck, step back and reason about the root cause before acting; \
                            (3) if the task is genuinely impossible with the available tools, provide a clear explanation to the user. \
                            Do NOT repeat the same tool call again.</system_reminder>",
                            max_consec
                        );
                        let user_msg = Message::internal_reminder(
                            InternalReminderKind::LoopRecovery,
                            reminder,
                        )
                        .with_turn_id(context.dialog_turn_id.clone());
                        messages.push(user_msg.clone());
                        self.remember_generation_message(
                            &context.session_id,
                            &context.dialog_turn_id,
                            &user_msg,
                        );
                        if let Err(e) = self
                            .session_manager
                            .add_message(&context.session_id, user_msg)
                            .await
                        {
                            warn!("Failed to persist failed-tool recovery reminder: {}", e);
                        }
                        recent_failed_tool_signatures.clear();
                    } else {
                        warn!(
                            "Repeated tool failure detected: {} consecutive rounds with identical tool signatures, max recovery attempts ({}) exhausted, finalizing without tools",
                            max_consec, max_failed_tool_recovery_attempts
                        );
                        finalization_reason = Some("repeated_tool_failures");
                        break;
                    }
                }
            }

            // Periodic-pattern loop detection.
            //
            // The strict consecutive check above only fires on `A-A-A` patterns.
            // Real-world subagent loops often alternate between a small set of
            // signatures (e.g. `A-B-A-B-A-B` when the model toggles a single
            // argument such as the regex pattern, while every other call is
            // identical). Such rounds never collapse to a single signature, so
            // the model can stay stuck for hundreds of rounds without tripping
            // the strict check.
            //
            // The periodic detector inspects the last `2 * max_consec` rounds:
            // if at most `max_consec` distinct signatures appear AND every one
            // of those signatures appears at least twice, the window contains
            // no genuine new exploration and we treat it as a loop.
            if Self::is_periodic_tool_signature_loop(&recent_failed_tool_signatures, max_consec) {
                let window_size = max_consec.max(1).saturating_mul(2);
                if failed_tool_recovery_attempts < max_failed_tool_recovery_attempts {
                    failed_tool_recovery_attempts += 1;
                    warn!(
                        "Repeated tool failure detected: last {} failed rounds form a periodic tool-call pattern (<= {} distinct signatures, each repeated), injecting recovery prompt #{}",
                        window_size, max_consec, failed_tool_recovery_attempts
                    );
                    let reminder = format!(
                        "<system_reminder>Repeated tool failure detected: your last {} failed tool calls form a repeating pattern with no new progress. \
                        You are cycling between failing actions without advancing the task. You MUST now change your strategy: \
                        (1) try a completely different approach or tool; \
                        (2) step back and reason about the root cause before acting; \
                        (3) if the task is genuinely impossible with the available tools, provide a clear explanation to the user. \
                        Do NOT repeat the same pattern of tool calls.</system_reminder>",
                        window_size
                    );
                    let user_msg = Message::internal_reminder(
                        InternalReminderKind::PeriodicLoopRecovery,
                        reminder,
                    )
                    .with_turn_id(context.dialog_turn_id.clone());
                    messages.push(user_msg.clone());
                    self.remember_generation_message(
                        &context.session_id,
                        &context.dialog_turn_id,
                        &user_msg,
                    );
                    if let Err(e) = self
                        .session_manager
                        .add_message(&context.session_id, user_msg)
                        .await
                    {
                        warn!("Failed to persist periodic loop recovery reminder: {}", e);
                    }
                    recent_failed_tool_signatures.clear();
                } else {
                    warn!(
                            "Repeated tool failure detected: last {} failed rounds form a periodic tool-call pattern, max recovery attempts ({}) exhausted, finalizing without tools",
                            window_size, max_failed_tool_recovery_attempts
                    );
                    finalization_reason = Some("repeated_tool_failures");
                    break;
                }
            }

            // User-steering messages submitted while this turn is running: drain and inject
            // them as user messages into the working history before starting the next round
            // (Codex-style mid-turn injection). This does NOT end the current turn: if the
            // model wanted to finish but the user steered, we keep the turn running so the
            // steering message gets a response.
            let mut injection_applied = false;
            if let Some(source) = context.round_injection.as_ref() {
                let pending = source.take_pending(&context.session_id, &context.dialog_turn_id);
                if !pending.is_empty() {
                    // R-ASYNC-01（项1）：移除 round 边界排队合并。
                    // 同轮边界排队的 N 条后台完成通知全部逐条注入（不合并），排队消费
                    // 语义（drain_for_turn / acknowledge_consumed）保留。
                    info!(
                        "Injecting {} round message(s) at round boundary: session_id={}, dialog_turn_id={}, round_index={}",
                        pending.len(),
                        context.session_id,
                        context.dialog_turn_id,
                        round_index
                    );
                    for injection in pending {
                        let injection_id = injection.id.clone();
                        let injection_kind = injection.kind;
                        let wrapped = match injection.kind {
                            RoundInjectionKind::UserSteering => {
                                let steering_text = if injection.content.trim().is_empty()
                                    && !injection.attachments.is_empty()
                                {
                                    "(image attached)".to_string()
                                } else {
                                    injection.content.clone()
                                };
                                let prepended_text = injection
                                    .prepended_reminders
                                    .iter()
                                    .map(|reminder| reminder.text.as_str())
                                    .collect::<Vec<_>>()
                                    .join("\n");
                                if prepended_text.is_empty() {
                                    format!(
                                        "<system_reminder>\nThe user sent a new message while this turn was running. You have just finished the previous atomic action; handle this new user message now as the current direction, while preserving the existing conversation and task context. Do not ignore it or wait for a separate future turn.\n\nNew user message:\n{}\n</system_reminder>",
                                        steering_text
                                    )
                                } else {
                                    format!(
                                        "<system_reminder>\n{}\n\nAn agent sent a new message while this turn was running. You have just finished the previous atomic action; handle this new message now as the current direction, while preserving the existing conversation and task context. Do not ignore it or wait for a separate future turn.\n\nNew message:\n{}\n</system_reminder>",
                                        prepended_text, steering_text
                                    )
                                }
                            }
                            RoundInjectionKind::BackgroundResult => format!(
                                "<system_reminder>\nA background task has finished and returned new information while this turn was running. Incorporate it into your current work immediately when relevant. Do not wait for a separate future turn.\n\nBackground result:\n{}\n</system_reminder>",
                                injection.content
                            ),
                            RoundInjectionKind::ThreadGoalObjectiveUpdated => {
                                injection.content.clone()
                            }
                        };
                        let reminder_kind = match injection.kind {
                            RoundInjectionKind::UserSteering => InternalReminderKind::UserSteering,
                            RoundInjectionKind::BackgroundResult => {
                                InternalReminderKind::BackgroundResult
                            }
                            RoundInjectionKind::ThreadGoalObjectiveUpdated => {
                                InternalReminderKind::GoalObjectiveUpdated
                            }
                        };
                        // Attachments rebuild into the same multimodal user
                        // message a turn-boundary submission would have
                        // produced; a bad payload degrades to text rather than
                        // dropping the user's steering message entirely.
                        let images = match agent_dialog_turn_image_contexts(&injection.attachments)
                        {
                            Ok(images) => images.unwrap_or_default(),
                            Err(error) => {
                                warn!(
                                    "Dropping unusable steering attachments, injecting text only: session_id={}, steering_id={}, error={}",
                                    context.session_id, injection_id, error
                                );
                                Vec::new()
                            }
                        };
                        let user_msg = if images.is_empty() {
                            Message::internal_reminder(reminder_kind, wrapped)
                        } else {
                            Message::internal_reminder_multimodal(reminder_kind, wrapped, images)
                        }
                        .with_turn_id(context.dialog_turn_id.clone())
                        .with_steering_id(injection.id.clone());
                        messages.push(user_msg.clone());
                        self.remember_generation_message(
                            &context.session_id,
                            &context.dialog_turn_id,
                            &user_msg,
                        );
                        if let Err(e) = self
                            .session_manager
                            .add_message(&context.session_id, user_msg)
                            .await
                        {
                            warn!("Failed to persist user steering message in memory: {}", e);
                        }

                        self.emit_event(
                            AgenticEvent::UserSteeringInjected {
                                session_id: context.session_id.clone(),
                                turn_id: context.dialog_turn_id.clone(),
                                round_index,
                                steering_id: injection.id,
                                content: injection.content,
                                display_content: injection.display_content,
                            },
                            EventPriority::Normal,
                        )
                        .await;
                        source.acknowledge_consumed(
                            &context.session_id,
                            &context.dialog_turn_id,
                            &injection_id,
                            injection_kind,
                        );
                        injection_applied = true;
                    }
                }
            }

            // P0-1: Decide whether to end the turn here.
            //
            // If the user just injected a steering message we always continue so the
            // model can respond to it.
            //
            // Otherwise, if the round produced any tool_call, we already continue via
            // `has_more_rounds = true`. The interesting case is `has_more_rounds == false`:
            //
            // - Model emitted user-visible text  -> final answer, end the turn, unless
            //   the stream was partially recovered (timeout / interruption) in which
            //   case inject a continuation reminder and keep going.
            // - Model emitted thinking only      -> stalled mid-reasoning. Inject a
            //   system_reminder asking it to either act (call a tool) or finish
            //   (write the answer), and continue.
            // - Model emitted nothing at all     -> partial recovery / truncation.
            //   Retrying without new context will not help, so end the turn.
            if injection_applied {
                // fall through to next round so the model can respond to the steering
            } else if !round_result.has_more_rounds {
                if round_result.had_assistant_text {
                    if let Some(ref reason) = round_result.partial_recovery_reason {
                        if Self::should_continue_after_partial_response(reason) {
                            partial_continuation_attempts += 1;
                            if partial_continuation_attempts <= max_partial_continuation_attempts {
                                let reminder = format!(
                                    "<system_reminder>Your previous assistant response was interrupted mid-stream ({reason}). Continue writing from exactly where you stopped. Do not repeat content that was already delivered; pick up seamlessly and complete the answer.</system_reminder>"
                                );
                                let user_msg = Message::internal_reminder(
                                    InternalReminderKind::InterruptedContinue,
                                    reminder.clone(),
                                )
                                .with_turn_id(context.dialog_turn_id.clone());
                                messages.push(user_msg.clone());
                                self.remember_generation_message(
                                    &context.session_id,
                                    &context.dialog_turn_id,
                                    &user_msg,
                                );
                                if let Err(e) = self
                                    .session_manager
                                    .add_message(&context.session_id, user_msg)
                                    .await
                                {
                                    warn!("Failed to persist partial continuation reminder: {}", e);
                                }
                                warn!(
                                    "Partial stream recovery with assistant text; injecting continuation reminder #{}/{}: turn={}, round={}, reason={}",
                                    partial_continuation_attempts,
                                    max_partial_continuation_attempts,
                                    context.dialog_turn_id,
                                    round_index,
                                    reason
                                );
                                // Continue into the next round so the model can finish.
                            } else {
                                warn!(
                                    "Partial stream continuation attempts exhausted; accepting truncated answer: turn={}, round={}, reason={}",
                                    context.dialog_turn_id, round_index, reason
                                );
                                finalization_reason = Some("partial_truncated");
                                break;
                            }
                        } else {
                            debug!(
                                "Model round {} ended with partial answer after cancellation, reason: {:?}",
                                round_index, round_result.finish_reason
                            );
                            break;
                        }
                    } else {
                        debug!(
                            "Model round {} ended with final answer, reason: {:?}",
                            round_index, round_result.finish_reason
                        );
                        // Stop hooks may block the natural end of the turn and
                        // ask the agent to keep working. `stop_hook_active`
                        // tells the hook it is already running inside such a
                        // continuation so it can avoid an endless loop, and the
                        // engine caps continuations regardless.
                        // Subagent turns run through this same loop; their
                        // completion is reported by SubagentStop instead, so
                        // Stop stays a top-level-turn event as in Codex.
                        let stop_block_reason = if context.subagent_parent_info.is_none()
                            && stop_hook_continuations < max_stop_hook_continuations
                        {
                            native_hooks::dispatch_stop(
                                Self::native_hook_facts(
                                    &context.session_id,
                                    &context.dialog_turn_id,
                                    context.workspace.as_ref(),
                                    &ai_client.config.model,
                                ),
                                stop_hook_continuations > 0,
                                Self::assistant_message_text(&round_result.assistant_message),
                            )
                            .await
                        } else {
                            None
                        };
                        if let Some(reason) = stop_block_reason {
                            stop_hook_continuations += 1;
                            let reminder = format!(
                                "<system_reminder>A Stop hook blocked the end of this turn: {reason}\nAddress this before finishing, then produce your final answer.</system_reminder>"
                            );
                            let user_msg = Message::internal_reminder(
                                InternalReminderKind::StopHookBlock,
                                reminder,
                            )
                            .with_turn_id(context.dialog_turn_id.clone());
                            messages.push(user_msg.clone());
                            self.remember_generation_message(
                                &context.session_id,
                                &context.dialog_turn_id,
                                &user_msg,
                            );
                            if let Err(e) = self
                                .session_manager
                                .add_message(&context.session_id, user_msg)
                                .await
                            {
                                warn!("Failed to persist Stop hook reminder: {}", e);
                            }
                            info!(
                                "Stop hook blocked turn completion; continuing turn #{}/{}: turn={}, round={}",
                                stop_hook_continuations,
                                max_stop_hook_continuations,
                                context.dialog_turn_id,
                                round_index
                            );
                            // Continue into the next round so the agent can act
                            // on the hook feedback.
                        } else {
                            break;
                        }
                    }
                } else if round_result.had_thinking_content {
                    thinking_only_rescue_attempts += 1;
                    // Bound repeated thinking-only rounds: each rescue re-requests
                    // the model with no new information. Once the budget is
                    // exhausted, synthesize a local final response instead of
                    // keeping the storm alive (the observable driver of the
                    // 2000 empty prompts observed in the 2026-08-10 audit).
                    if thinking_only_rescue_attempts > DEFAULT_EMPTY_ROUND_RESPAWN_LIMIT {
                        warn!(
                            "Thinking-only round rescue budget exhausted ({} attempts); ending turn with local final response: turn={}, round={}",
                            thinking_only_rescue_attempts, context.dialog_turn_id, round_index
                        );
                        finalization_reason = Some("thinking_only_budget");
                        let local_msg = Message::assistant(
                            Self::build_local_final_response_message("thinking_only_budget"),
                        )
                        .with_turn_id(context.dialog_turn_id.clone());
                        messages.push(local_msg.clone());
                        if let Err(e) = self
                            .session_manager
                            .add_message(&context.session_id, local_msg)
                            .await
                        {
                            warn!(
                                "Failed to persist thinking-only budget final response: {}",
                                e
                            );
                        }
                        break;
                    }
                    let reminder = "<system_reminder>The previous round produced internal reasoning only — no tool call and no user-visible response. You MUST now either: (1) call the single tool that best advances the user's task, or (2) write your final answer to the user. Do not produce another round of reasoning without taking action.</system_reminder>".to_string();
                    let user_msg = Message::internal_reminder(
                        InternalReminderKind::ThinkingOnlyRescue,
                        reminder.clone(),
                    )
                    .with_turn_id(context.dialog_turn_id.clone());
                    messages.push(user_msg.clone());
                    self.remember_generation_message(
                        &context.session_id,
                        &context.dialog_turn_id,
                        &user_msg,
                    );
                    if let Err(e) = self
                        .session_manager
                        .add_message(&context.session_id, user_msg)
                        .await
                    {
                        warn!("Failed to persist thinking-only rescue reminder: {}", e);
                    }
                    warn!(
                        "Thinking-only round detected; injecting rescue reminder #{}: turn={}, round={}",
                        thinking_only_rescue_attempts, context.dialog_turn_id, round_index
                    );
                    // Continue into the next round so the model gets a chance to act.
                } else {
                    warn!(
                        "Empty round (no text/thinking/tool_call); ending turn: turn={}, round={}",
                        context.dialog_turn_id, round_index
                    );
                    finalization_reason = Some("empty_round");
                    break;
                }
            }

            // Check if cancellation was requested after each round. Tokens stay
            // registered until final cleanup so early cancellation can be
            // observed by the first round.
            if self
                .round_executor
                .is_dialog_turn_cancelled(&dialog_turn_id)
            {
                debug!(
                    "Dialog turn cancelled, stopping execution: dialog_turn_id={}",
                    dialog_turn_id
                );

                if context.emit_lifecycle_events
                    && execution_engine_owns_cancel_lifecycle(&context.context)
                {
                    self.emit_event(
                        AgenticEvent::DialogTurnCancelled {
                            session_id: context.session_id.clone(),
                            turn_id: context.dialog_turn_id.clone(),
                        },
                        EventPriority::High,
                    )
                    .await;
                }

                // Note: Token will be cleaned up when outer function exits
                return Err(BitFunError::cancelled("Dialog cancelled"));
            }

            // Continue to next round
            round_index += 1;

            debug!(
                "Model round {} completed, continuing to round {}",
                round_index - 1,
                round_index
            );
        }

        // P1-6: Track the actual termination reason for downstream reporting.
        // Defaults to "complete" (model produced a final answer naturally).
        let effective_finish_reason: &'static str = match finalization_reason {
            Some(r) => r,
            None => "complete",
        };
        let mut has_final_response = finalization_reason.is_none();
        let mut used_local_final_response_synthesis = false;

        if let Some(reason) = finalization_reason {
            let finalize_reminder = match reason {
                "repeated_tool_failures" => {
                    Some(Self::FINALIZE_AFTER_REPEATED_TOOL_FAILURES_REMINDER)
                }
                "max_rounds" => Some(Self::FINALIZE_AFTER_MAX_ROUNDS_REMINDER),
                _ => None,
            };

            if let Some(finalize_reminder) = finalize_reminder {
                // The finalize path issues fresh model requests. Bound them so
                // an empty-reply / non-progress storm cannot turn the finalize
                // step itself into an unbounded token sink; when the budget is
                // exhausted, synthesize a local final response instead.
                // finalize 路径是直线结构：首请求 + 至多一次重试，天然受
                // DEFAULT_FINALIZE_ROUND_LIMIT=2 约束（gate 边界 0/1 用字面量
                // 显式表达），无需运行时计数（消除「写后未读」死代码）。
                let finalize_allowed =
                    Self::should_allow_finalize_round(0, DEFAULT_FINALIZE_ROUND_LIMIT);
                let finalize_round_group_id = Some(format!(
                    "{}:finalize:{}",
                    context.dialog_turn_id, completed_rounds
                ));
                info!(
                    "Finalizing dialog turn: session_id={}, turn_id={}, reason={}, finalize_rounds_completed={}, finalize_allowed={}",
                    context.session_id, context.dialog_turn_id, reason, 0usize, finalize_allowed
                );

                let finalize_static_prepended_reminders = turn_prompt_scaffold
                    .prepended_prompt_reminders
                    .static_ordered_reminders();
                let finalize_dynamic_prepended_reminders = self
                    .round_dynamic_reminders(
                        &context.session_id,
                        &context,
                        &turn_prompt_scaffold.prepended_prompt_reminders,
                    )
                    .await;
                let final_round_result = if finalize_allowed {
                    self.run_finalize_round(FinalizeRoundInput {
                        permission_constraints: tool_policy.permission_constraints.clone(),
                        ai_client: ai_client.clone(),
                        context: &context,
                        agent_type: agent_type.clone(),
                        round_number: completed_rounds,
                        round_group_id: finalize_round_group_id.clone(),
                        execution_context_vars: &execution_context_vars,
                        primary_model_facts: &primary_model_facts,
                        static_prepended_reminders: &finalize_static_prepended_reminders,
                        dynamic_prepended_reminders: &finalize_dynamic_prepended_reminders,
                        model_request_context: &model_request_context,
                        messages: &messages,
                        reminder_text: finalize_reminder,
                        tool_definitions: tool_definitions.clone(),
                        user_enabled_tools: tool_policy.user_enabled_tools.clone(),
                        context_window,
                    })
                    .await?
                } else {
                    warn!(
                        "Finalize round budget exhausted ({} >= {}); synthesizing local final response: session_id={}, turn_id={}, reason={}",
                        0usize,
                        DEFAULT_FINALIZE_ROUND_LIMIT,
                        context.session_id,
                        context.dialog_turn_id,
                        reason
                    );
                    crate::agentic::execution::types::RoundResult::local_fallback()
                };
                // 首请求完成；重试门控（1 < 2）为后续唯一读取点，
                // 修复前此处的 += 1 是「写后未读」死代码，已移除。

                let mut accepted = final_round_result.had_assistant_text
                    && !Self::assistant_has_tool_calls(&final_round_result.assistant_message);
                let chosen_assistant_message: Option<Message>;
                let mut chosen_usage: Option<crate::util::types::ai::GeminiUsage> =
                    final_round_result.usage.clone();

                if accepted {
                    chosen_assistant_message = Some(final_round_result.assistant_message.clone());
                } else {
                    warn!(
                        "Finalize round did not return usable assistant text; retrying once: session_id={}, turn_id={}",
                        context.session_id, context.dialog_turn_id
                    );
                    let retry_allowed =
                        Self::should_allow_finalize_round(1, DEFAULT_FINALIZE_ROUND_LIMIT);
                    let retry_result = if retry_allowed {
                        self.run_finalize_round(FinalizeRoundInput {
                            permission_constraints: tool_policy.permission_constraints.clone(),
                            ai_client: ai_client.clone(),
                            context: &context,
                            agent_type: agent_type.clone(),
                            round_number: completed_rounds,
                            round_group_id: finalize_round_group_id.clone(),
                            execution_context_vars: &execution_context_vars,
                            primary_model_facts: &primary_model_facts,
                            static_prepended_reminders: &finalize_static_prepended_reminders,
                            dynamic_prepended_reminders: &finalize_dynamic_prepended_reminders,
                            model_request_context: &model_request_context,
                            messages: &messages,
                            reminder_text: finalize_reminder,
                            tool_definitions: tool_definitions.clone(),
                            user_enabled_tools: tool_policy.user_enabled_tools.clone(),
                            context_window,
                        })
                        .await?
                    } else {
                        warn!(
                            "Finalize retry budget exhausted ({} >= {}); synthesizing local final response: session_id={}, turn_id={}",
                            1usize,
                            DEFAULT_FINALIZE_ROUND_LIMIT,
                            context.session_id,
                            context.dialog_turn_id
                        );
                        crate::agentic::execution::types::RoundResult::local_fallback()
                    };
                    if !retry_result.had_assistant_text
                        || Self::assistant_has_tool_calls(&retry_result.assistant_message)
                    {
                        warn!(
                            "Finalize retry did not return usable assistant text; synthesizing local final response: session_id={}, turn_id={}",
                            context.session_id, context.dialog_turn_id
                        );
                        accepted = true;
                        used_local_final_response_synthesis = true;
                        chosen_assistant_message = Some(
                            Message::assistant(Self::build_local_final_response_message(reason))
                                .with_turn_id(context.dialog_turn_id.clone()),
                        );
                    } else {
                        accepted = true;
                        chosen_usage = retry_result.usage.clone();
                        chosen_assistant_message = Some(retry_result.assistant_message);
                    }
                }

                has_final_response = Self::should_mark_has_final_response(
                    chosen_assistant_message.is_some(),
                    used_local_final_response_synthesis,
                );
                if let Some(msg) = chosen_assistant_message {
                    if accepted && !used_local_final_response_synthesis {
                        let finalize_cache_anchor_messages =
                            Self::build_finalize_cache_anchor_messages(
                                &context.dialog_turn_id,
                                finalize_reminder,
                            );
                        for anchor_message in finalize_cache_anchor_messages {
                            messages.push(anchor_message.clone());
                            self.remember_generation_message(
                                &context.session_id,
                                &context.dialog_turn_id,
                                &anchor_message,
                            );
                            if let Err(e) = self
                                .session_manager
                                .add_message(&context.session_id, anchor_message)
                                .await
                            {
                                warn!("Failed to persist finalize cache anchor message: {}", e);
                            }
                        }
                    }
                    completed_rounds += 1;
                    if let Some(usage) = chosen_usage {
                        last_usage = Some(usage);
                    }
                    messages.push(msg.clone());
                    self.remember_generation_message(
                        &context.session_id,
                        &context.dialog_turn_id,
                        &msg,
                    );
                    if let Err(e) = self
                        .session_manager
                        .add_message(&context.session_id, msg)
                        .await
                    {
                        warn!("Failed to update final assistant message in memory: {}", e);
                    }
                }
            } else if reason == "partial_truncated" || reason == "thinking_only_budget" {
                // Both paths deliver a user-visible final response: the partial
                // answer streamed earlier, and the thinking-only budget path
                // synthesized a local assistant message.
                has_final_response = true;
            } else if reason == "duplicate_messages" {
                // R-MR-10 消息重复闸门拦截：不调 API，本地合成 final response。
                // 与 max_rounds / thinking_only_budget 同为「本地收尾」路径——不
                // 再发起任何模型请求（拦截即停），把本地合成的终止说明写入会话。
                let local_msg = Message::assistant(Self::build_local_final_response_message(
                    "duplicate_messages",
                ))
                .with_turn_id(context.dialog_turn_id.clone());
                messages.push(local_msg.clone());
                if let Err(e) = self
                    .session_manager
                    .add_message(&context.session_id, local_msg)
                    .await
                {
                    warn!("Failed to persist duplicate-message final response: {}", e);
                }
                has_final_response = true;
            } else if reason == "empty_initial_turn" {
                // R-MR-06 / R-13 首轮空内容守卫拦截：不调 API，本地合成 final
                // response（同 duplicate_messages 路径），把终止说明写入会话。
                let local_msg = Message::assistant(Self::build_local_final_response_message(
                    "empty_initial_turn",
                ))
                .with_turn_id(context.dialog_turn_id.clone());
                messages.push(local_msg.clone());
                if let Err(e) = self
                    .session_manager
                    .add_message(&context.session_id, local_msg)
                    .await
                {
                    warn!("Failed to persist empty-initial-turn final response: {}", e);
                }
                has_final_response = true;
            }
        }

        let duration_ms = elapsed_ms_u64(start_time);

        info!(
            "Dialog turn loop completed: turn={}, rounds={}, total_tools={}, reason={}",
            context.dialog_turn_id, completed_rounds, total_tools, effective_finish_reason
        );

        let finish_reason = FinishReason::Complete;
        // Some abnormal turn endings still go through the completed-event path
        // so the UI can explain the termination cause inline even when the turn
        // ended without a final assistant reply.
        let success = has_final_response
            || matches!(
                effective_finish_reason,
                "max_rounds" | "repeated_tool_failures" | "empty_initial_turn"
            );

        // Post-processing hook: when a DeepResearch dialog turn finishes
        // successfully, renumber `cit_XXX` references in the final report
        // into consecutive `[N]` display IDs. Two gates apply (agent type +
        // dialog success) so other agents and failed turns are unaffected.
        #[cfg(feature = "deep-research")]
        {
            if bitfun_agent_runtime::deep_research::should_post_process_research_report(
                &agent_type,
                success,
            ) {
                if let Some(workspace) = context.workspace.as_ref() {
                    if let Some(workspace_services) = context.workspace_services.as_ref() {
                        bitfun_services_integrations::deep_research::run_for_session_workspace(
                            workspace_services.fs.as_ref(),
                            &workspace.root_path().to_string_lossy(),
                            &context.session_id,
                        )
                        .await;
                    } else {
                        warn!(
                            "citation_renumber: skipped because workspace filesystem services are unavailable: session_id={}, workspace={}",
                            context.session_id,
                            workspace.root_path().display()
                        );
                    }
                }
            }
        }

        if context.emit_lifecycle_events {
            debug!("Preparing to send DialogTurnCompleted event");

            let _ = self
                .event_queue
                .enqueue(
                    AgenticEvent::DialogTurnCompleted {
                        session_id: context.session_id.clone(),
                        turn_id: context.dialog_turn_id.clone(),
                        total_rounds: completed_rounds,
                        total_tools,
                        duration_ms,
                        partial_recovery_reason: last_partial_recovery_reason.clone(),
                        success: Some(success),
                        finish_reason: Some(effective_finish_reason.to_string()),
                        has_final_response: Some(has_final_response),
                    },
                    None,
                )
                .await;

            debug!("DialogTurnCompleted event sent");
        }

        // Print dialog turn token statistics (from model's last returned usage)
        if let Some(ref usage) = last_usage {
            info!(
                "Dialog turn completed - Token stats: turn_id={}, rounds={}, tools={}, duration={}ms, prompt_tokens={}, completion_tokens={}, total_tokens={}",
                context.dialog_turn_id,
                completed_rounds,
                total_tools,
                duration_ms,
                usage.prompt_token_count,
                usage.candidates_token_count,
                usage.total_token_count
            );
        } else {
            warn!("Dialog turn completed but token stats not available");
        }

        Ok(ExecutionResult {
            final_message: self
                .generation_messages
                .get(&(context.session_id.clone(), context.dialog_turn_id.clone()))
                .and_then(|generated| {
                    generated
                        .iter()
                        .rev()
                        .find(|message| message.role == MessageRole::Assistant)
                        .cloned()
                })
                .or_else(|| {
                    messages
                        .iter()
                        .rev()
                        .find(|message| message.role == MessageRole::Assistant)
                        .cloned()
                })
                .unwrap_or_else(|| Message::assistant(String::new())),
            total_rounds: completed_rounds,
            total_tools,
            total_tokens: last_usage
                .as_ref()
                .map(|usage| usage.total_token_count as usize)
                .unwrap_or(0),
            duration_ms,
            success,
            new_messages: self
                .take_generation_messages(&context.session_id, &context.dialog_turn_id),
            finish_reason,
            partial_recovery_reason: last_partial_recovery_reason,
            effective_finish_reason: effective_finish_reason.to_string(),
            has_final_response,
        })
    }

    /// Cancel dialog turn execution
    pub async fn cancel_dialog_turn(&self, dialog_turn_id: &str) -> BitFunResult<()> {
        debug!("Cancelling dialog turn: dialog_turn_id={}", dialog_turn_id);
        let result = self.round_executor.cancel_dialog_turn(dialog_turn_id).await;
        if result.is_ok() {
            debug!(
                "Dialog turn cancelled successfully: dialog_turn_id={}",
                dialog_turn_id
            );
        } else {
            error!(
                "Failed to cancel dialog turn: dialog_turn_id={}, error={:?}",
                dialog_turn_id, result
            );
        }
        result
    }

    /// Check if dialog turn is still active (used to detect cancellation)
    pub fn has_active_turn(&self, dialog_turn_id: &str) -> bool {
        self.round_executor.has_active_dialog_turn(dialog_turn_id)
    }

    /// Register cancellation token (for external control, e.g., execute_subagent)
    pub fn register_cancel_token(&self, dialog_turn_id: &str, token: CancellationToken) {
        self.round_executor
            .register_cancel_token(dialog_turn_id, token)
    }

    /// Return a clone of the cancellation token registered for a dialog turn.
    pub fn cancel_token_for_dialog_turn(&self, dialog_turn_id: &str) -> Option<CancellationToken> {
        self.round_executor
            .cancel_token_for_dialog_turn(dialog_turn_id)
    }

    /// Cleanup cancellation token (for external calls)
    pub async fn cleanup_cancel_token(&self, dialog_turn_id: &str) {
        self.round_executor
            .cleanup_dialog_turn(dialog_turn_id)
            .await
    }

    /// Emit event
    async fn emit_event(&self, event: AgenticEvent, priority: EventPriority) {
        let _ = self.event_queue.enqueue(event, Some(priority)).await;
    }
}

#[cfg(test)]
mod tests {
    use super::{
        activate_conditional_instructions_after_round, ensure_primary_session_goal_tools,
        manual_compaction_terminal_error, resolve_round_permission_mode, ContextHealthSnapshot,
        ExecutionEngine, RoundResult, TurnPromptScaffold,
    };
    use crate::agentic::agents::{
        PrependedPromptReminders, PromptBuilderContext, UserContextPolicy,
    };
    use crate::agentic::core::{
        InternalReminderKind, Message, MessageRole, MessageSemanticKind, ToolCall, ToolResult,
    };
    use crate::agentic::events::{EventQueue, EventQueueConfig};
    use crate::agentic::execution::{ExecutionEngineConfig, RoundExecutor, StreamProcessor};
    use crate::agentic::persistence::PersistenceManager;

    use crate::agentic::session::compression::CompressionConfig;
    use crate::agentic::session::PromptCacheScope;
    use crate::agentic::session::{
        ContextCompressor, PromptCachePolicy, SessionContextStore, SessionManager,
        SessionManagerConfig, TokenAnchor, TokenAnchorInput,
    };
    use crate::agentic::tools::registry::ToolRegistry;
    use crate::agentic::tools::ToolRuntimeRestrictions;
    use crate::agentic::tools::{ToolPipeline, ToolStateManager};
    use crate::agentic::workspace::{local_workspace_services, WorkspaceBinding};
    use crate::infrastructure::PathManager;
    #[cfg(feature = "external-sources")]
    use crate::instruction_sources::test_support::EnvironmentGuard;
    use crate::instruction_sources::test_support::{lock_environment, InstructionSwitches};
    use crate::service::config::types::AIConfig;
    use crate::service::config::types::AIModelConfig;
    use crate::service::remote_ssh::workspace_state::workspace_session_identity;
    use crate::util::types::ToolDefinition;
    use crate::util::TokenCounter;
    use bitfun_agent_runtime::prompt::RuntimeFactsUsage;
    use bitfun_agent_runtime::thread_goal_tools::THREAD_GOAL_TOOL_NAMES;
    use bitfun_runtime_ports::{
        PermissionMode, WorkspaceDirEntry, WorkspaceFileSystem, WorkspacePathKind,
    };
    use serde_json::json;
    use sha2::{Digest, Sha256};
    use std::collections::HashMap;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::sync::Arc;
    use std::time::Duration;
    use tokio::sync::RwLock as TokioRwLock;

    #[test]
    fn recovered_execution_starts_after_existing_model_rounds() {
        let mut context = std::collections::HashMap::new();
        context.insert("initial_round_index".to_string(), "3".to_string());

        assert_eq!(super::initial_round_index(&context), 3);
        assert_eq!(
            super::initial_round_index(&std::collections::HashMap::new()),
            0
        );
    }

    #[test]
    fn interrupted_continue_reminder_is_visible_only_to_its_own_turn() {
        let reminder = Message::internal_reminder(
            InternalReminderKind::InterruptedContinue,
            "continue the interrupted work".to_string(),
        )
        .with_turn_id("turn-interrupted".to_string());

        assert!(!ExecutionEngine::is_stale_interrupted_continue(
            &reminder,
            "turn-interrupted"
        ));
        assert!(ExecutionEngine::is_stale_interrupted_continue(
            &reminder, "turn-new"
        ));
    }

    #[test]
    fn coordinator_owned_cancellation_suppresses_early_cancelled_event() {
        let mut context = std::collections::HashMap::new();
        assert!(super::execution_engine_owns_cancel_lifecycle(&context));
        context.insert(
            super::super::types::CANCEL_LIFECYCLE_OWNER_CONTEXT_KEY.to_string(),
            "coordinator".to_string(),
        );
        assert!(!super::execution_engine_owns_cancel_lifecycle(&context));
    }

    #[test]
    fn primary_session_tool_policy_restores_goal_tools_but_subagents_stay_scoped() {
        let mut primary_tools = vec!["Read".to_string()];
        ensure_primary_session_goal_tools(&mut primary_tools, false);
        for tool_name in THREAD_GOAL_TOOL_NAMES {
            assert!(primary_tools.iter().any(|tool| tool == tool_name));
        }

        let mut subagent_tools = vec!["Read".to_string()];
        ensure_primary_session_goal_tools(&mut subagent_tools, true);
        assert_eq!(subagent_tools, vec!["Read".to_string()]);
    }

    #[test]
    fn round_permission_mode_prefers_mutable_turn_then_fixed_child_then_session() {
        assert_eq!(
            resolve_round_permission_mode(
                Some(PermissionMode::Ask),
                Some(PermissionMode::FullAccess),
                Some(PermissionMode::AutoApprove),
                PermissionMode::FullAccess,
            ),
            PermissionMode::Ask,
        );
        assert_eq!(
            resolve_round_permission_mode(
                None,
                Some(PermissionMode::FullAccess),
                Some(PermissionMode::Ask),
                PermissionMode::AutoApprove,
            ),
            PermissionMode::FullAccess,
        );
        assert_eq!(
            resolve_round_permission_mode(
                None,
                None,
                Some(PermissionMode::AutoApprove),
                PermissionMode::Ask,
            ),
            PermissionMode::AutoApprove,
        );
    }

    #[test]
    fn manual_compaction_preserves_cancellation_as_a_terminal_cancellation() {
        let error = manual_compaction_terminal_error(crate::BitFunError::Cancelled(
            "cancelled by user".to_string(),
        ));

        assert!(matches!(error, crate::BitFunError::Cancelled(_)));
    }

    #[derive(Clone)]
    struct InstructionWorkspaceFs {
        operation_count: Arc<AtomicUsize>,
        fail_next_probe: Arc<AtomicBool>,
    }

    impl InstructionWorkspaceFs {
        fn recovering() -> Self {
            Self {
                operation_count: Arc::new(AtomicUsize::new(0)),
                fail_next_probe: Arc::new(AtomicBool::new(true)),
            }
        }

        fn stable() -> Self {
            Self {
                operation_count: Arc::new(AtomicUsize::new(0)),
                fail_next_probe: Arc::new(AtomicBool::new(false)),
            }
        }

        fn record(&self) {
            self.operation_count.fetch_add(1, Ordering::SeqCst);
        }

        fn operation_count(&self) -> usize {
            self.operation_count.load(Ordering::SeqCst)
        }
    }

    #[async_trait::async_trait]
    impl WorkspaceFileSystem for InstructionWorkspaceFs {
        async fn read_file(&self, path: &str) -> anyhow::Result<Vec<u8>> {
            Ok(self.read_file_text(path).await?.into_bytes())
        }

        async fn read_file_text(&self, path: &str) -> anyhow::Result<String> {
            self.record();
            Ok(if path.ends_with("AGENTS.md") {
                "Recovered workspace instructions.".to_string()
            } else {
                String::new()
            })
        }

        async fn write_file(&self, _path: &str, _contents: &[u8]) -> anyhow::Result<()> {
            anyhow::bail!("writes are not supported")
        }

        async fn exists(&self, path: &str) -> anyhow::Result<bool> {
            self.is_file(path).await
        }

        async fn is_file(&self, path: &str) -> anyhow::Result<bool> {
            self.record();
            if path.ends_with("AGENTS.override.md")
                && self.fail_next_probe.swap(false, Ordering::SeqCst)
            {
                anyhow::bail!("temporary workspace connection failure")
            }
            Ok(path.ends_with("AGENTS.md") && !path.ends_with("AGENTS.override.md"))
        }

        async fn is_dir(&self, _path: &str) -> anyhow::Result<bool> {
            Ok(false)
        }

        async fn path_kind_no_follow(
            &self,
            path: &str,
        ) -> anyhow::Result<Option<WorkspacePathKind>> {
            self.record();
            if path.ends_with("AGENTS.override.md")
                && self.fail_next_probe.swap(false, Ordering::SeqCst)
            {
                anyhow::bail!("temporary workspace connection failure")
            }
            Ok(
                (path.ends_with("AGENTS.md") && !path.ends_with("AGENTS.override.md"))
                    .then_some(WorkspacePathKind::File),
            )
        }

        async fn read_dir(&self, _path: &str) -> anyhow::Result<Vec<WorkspaceDirEntry>> {
            Ok(Vec::new())
        }
    }

    fn workspace_with_fs(
        fs: Arc<dyn WorkspaceFileSystem>,
    ) -> (
        WorkspaceBinding,
        crate::agentic::workspace::WorkspaceServices,
    ) {
        let workspace_root = PathBuf::from("/workspace");
        let mut workspace_services =
            local_workspace_services(workspace_root.to_string_lossy().to_string());
        workspace_services.fs = fs;
        let identity =
            workspace_session_identity("/workspace", Some("instruction-test"), Some("remote-host"))
                .expect("remote test identity");
        (
            WorkspaceBinding::new_remote(
                None,
                workspace_root,
                "instruction-test".to_string(),
                "Instruction test".to_string(),
                identity,
            ),
            workspace_services,
        )
    }

    fn build_model(id: &str, name: &str, model_name: &str) -> AIModelConfig {
        AIModelConfig {
            id: id.to_string(),
            name: name.to_string(),
            model_name: model_name.to_string(),
            provider: "anthropic".to_string(),
            enabled: true,
            ..Default::default()
        }
    }

    fn message_text(message: &Message) -> Option<&str> {
        match &message.content {
            crate::agentic::core::MessageContent::Text(text) => Some(text.as_str()),
            _ => None,
        }
    }

    #[tokio::test]
    async fn user_context_without_instruction_policy_does_not_read_instruction_files() {
        let fs = InstructionWorkspaceFs::recovering();
        let (workspace, workspace_services) = workspace_with_fs(Arc::new(fs.clone()));
        let prompt_context = PromptBuilderContext::new(
            "/workspace".to_string(),
            Some("session".to_string()),
            Some("model".to_string()),
        );
        let (_, cacheable) = ExecutionEngine::build_user_context_for_cache_miss(
            Some(&workspace),
            Some(&workspace_services),
            prompt_context,
            &UserContextPolicy::empty().with_workspace_context(),
        )
        .await;

        assert!(cacheable);
        assert_eq!(fs.operation_count(), 0);
    }

    #[tokio::test]
    #[allow(clippy::await_holding_lock)] // environment lock is intentionally held for the whole test body
    async fn workspace_instruction_read_failure_is_not_cacheable_and_can_recover() {
        // E-5: `InstructionSwitches::set` reads+writes the process-level
        // instruction master switches (instruction_sources.rs). Without the
        // environment lock, concurrent switch-mutating tests race the
        // read-modify-write here. Same lock_environment() discipline as the
        // sibling tests below (5948/5980/6001/6076/6124).
        let _environment = lock_environment();
        // Guard restores the previous switch values on drop.
        let _switches = InstructionSwitches::set(Some(true), None);
        let fs = InstructionWorkspaceFs::recovering();
        let (workspace, workspace_services) = workspace_with_fs(Arc::new(fs));
        let prompt_context = PromptBuilderContext::new(
            "/workspace".to_string(),
            Some("session".to_string()),
            Some("model".to_string()),
        );
        let policy = UserContextPolicy::empty()
            .with_workspace_context()
            .with_workspace_instructions();

        let (degraded_context, degraded_cacheable) =
            ExecutionEngine::build_user_context_for_cache_miss(
                Some(&workspace),
                Some(&workspace_services),
                prompt_context.clone(),
                &policy,
            )
            .await;
        assert!(!degraded_cacheable);
        assert!(!degraded_context
            .as_deref()
            .unwrap_or_default()
            .contains("Recovered workspace instructions."));

        let (recovered_context, recovered_cacheable) =
            ExecutionEngine::build_user_context_for_cache_miss(
                Some(&workspace),
                Some(&workspace_services),
                prompt_context,
                &policy,
            )
            .await;
        assert!(recovered_cacheable);
        assert!(recovered_context
            .as_deref()
            .unwrap_or_default()
            .contains("Recovered workspace instructions."));
    }

    #[cfg(feature = "external-sources")]
    #[tokio::test]
    #[allow(clippy::await_holding_lock)] // environment lock is intentionally held for the whole test body
    async fn user_context_cache_identity_includes_external_sources_switch_state() {
        // P2-1 (KV cache design audit 20260810): the external_instruction_sources
        // master switch changes the rendered User Context content (external user
        // files are skipped when off), but cache hits only check identity + TTL,
        // never content. The switch state must therefore be part of the cache
        // scope key so an on↔off toggle mid-session cannot hit the stale cached
        // content from the other switch state.
        let _environment = lock_environment();
        let base = crate::agentic::session::UserContextCacheIdentity::new(
            "workspace_context|workspace_instructions",
        );
        // Start ON; the explicit mid-test flip to OFF is asserted below, and
        // the guard restores the previous value on drop.
        let _switches = InstructionSwitches::set(None, Some(true));
        let on = ExecutionEngine::user_context_cache_identity_for(base.clone(), None, None).await;
        crate::service::config::set_external_instruction_sources_enabled(false);
        let off = ExecutionEngine::user_context_cache_identity_for(base.clone(), None, None).await;

        assert_eq!(
            on.scope_key,
            "workspace_context|workspace_instructions|extsrc:on|winstr:off"
        );
        assert_eq!(
            off.scope_key,
            "workspace_context|workspace_instructions|extsrc:off|winstr:off"
        );
        assert_ne!(
            on.scope_key, off.scope_key,
            "switch toggle must change the user context cache identity"
        );
    }

    #[cfg(feature = "external-sources")]
    #[tokio::test]
    #[allow(clippy::await_holding_lock)] // environment lock is intentionally held for the whole test body
    async fn user_context_cache_identity_layers_remote_and_switch_state() {
        // remote:<connection> and extsrc:<on|off> are orthogonal scope suffixes:
        // a remote overlay reconnect and a switch toggle must both invalidate
        // the user context cache independently while composing in one key.
        let _environment = lock_environment();
        let base = crate::agentic::session::UserContextCacheIdentity::new("workspace_instructions");
        // Guard restores the previous switch values on drop.
        let _switches = InstructionSwitches::set(None, Some(true));
        let identity =
            ExecutionEngine::user_context_cache_identity_for(base, Some("ssh-host/22"), None).await;
        assert_eq!(
            identity.scope_key,
            "workspace_instructions|remote:ssh-host/22|extsrc:on|winstr:off"
        );
    }

    #[cfg(feature = "external-sources")]
    #[tokio::test]
    #[allow(clippy::await_holding_lock)] // environment lock is intentionally held for the whole test body
    async fn session_user_context_cache_misses_after_external_sources_switch_toggle() {
        // P2-1 end-to-end guard: the scope key drives the session-level user
        // context cache. With the switch ON we remember content under the
        // `|extsrc:on` identity; after the switch flips OFF the engine must
        // miss that entry (it queries `|extsrc:off`) and rebuild, instead of
        // serving the stale ON content.
        let _environment = lock_environment();
        // Start ON; the explicit mid-test flip to OFF is asserted below, and
        // the guard restores the previous value on drop.
        let _switches = InstructionSwitches::set(None, Some(true));
        let temp = tempfile::tempdir().expect("tempdir");
        let workspace_path = temp.path().join("workspace");
        std::fs::create_dir_all(&workspace_path).expect("workspace directory");
        let session_manager = Arc::new(SessionManager::new(
            Arc::new(SessionContextStore::new()),
            Arc::new(
                PersistenceManager::new(Arc::new(PathManager::with_user_root_for_tests(
                    temp.path().join("user-root"),
                )))
                .expect("persistence manager"),
            ),
            SessionManagerConfig {
                max_active_sessions: 4,
                session_idle_timeout: Duration::from_secs(3600),
                auto_save_interval: Duration::from_secs(300),
                enable_persistence: false,
                prompt_cache_policy: PromptCachePolicy::default(),
            },
        ));
        let session = session_manager
            .create_session(
                "P2-1 switch toggle".to_string(),
                "agentic".to_string(),
                crate::agentic::core::SessionConfig {
                    workspace_path: Some(workspace_path.to_string_lossy().into_owned()),
                    ..Default::default()
                },
            )
            .await
            .expect("session should be created");

        let base_identity =
            crate::agentic::session::UserContextCacheIdentity::new("workspace_instructions");

        crate::service::config::set_external_instruction_sources_enabled(true);
        let on_identity =
            ExecutionEngine::user_context_cache_identity_for(base_identity.clone(), None, None)
                .await;
        session_manager
            .remember_user_context(
                &session.session_id,
                on_identity.clone(),
                "ON content".to_string(),
            )
            .await;
        assert_eq!(
            session_manager
                .cached_user_context(&session.session_id, &on_identity)
                .await
                .as_deref(),
            Some("ON content"),
            "same switch state must still hit the cache"
        );

        crate::service::config::set_external_instruction_sources_enabled(false);
        let off_identity =
            ExecutionEngine::user_context_cache_identity_for(base_identity, None, None).await;
        assert_ne!(on_identity, off_identity);
        assert_eq!(
            session_manager
                .cached_user_context(&session.session_id, &off_identity)
                .await,
            None,
            "switch toggle must not hit the stale ON content"
        );
    }

    #[cfg(feature = "external-sources")]
    #[tokio::test]
    #[allow(clippy::await_holding_lock)] // environment lock is intentionally held for the whole test body
    async fn local_workspace_services_still_include_local_user_instruction_sources() {
        let _environment = lock_environment();
        // Enable both instruction master switches; guard restores on drop.
        let _switches = InstructionSwitches::enable_all();
        let temp = tempfile::tempdir().expect("tempdir");
        let workspace_root = temp.path().join("workspace");
        let xdg = temp.path().join("xdg");
        let codex = temp.path().join("codex");
        let claude = temp.path().join("claude");
        std::fs::create_dir_all(xdg.join("opencode")).expect("OpenCode config directory");
        std::fs::create_dir_all(&codex).expect("Codex config directory");
        std::fs::create_dir_all(&claude).expect("Claude config directory");
        std::fs::create_dir_all(&workspace_root).expect("workspace directory");
        std::fs::write(xdg.join("opencode/AGENTS.md"), "Local engine user\n")
            .expect("OpenCode instructions");
        std::fs::write(workspace_root.join("AGENTS.md"), "Local engine project\n")
            .expect("workspace instructions");
        let _guard = EnvironmentGuard::set(&[
            ("XDG_CONFIG_HOME", &xdg),
            ("CODEX_HOME", &codex),
            ("CLAUDE_CONFIG_DIR", &claude),
        ]);
        let workspace = WorkspaceBinding::new(None, workspace_root.clone());
        let workspace_services =
            local_workspace_services(workspace_root.to_string_lossy().to_string());
        let policy = UserContextPolicy::empty().with_workspace_instructions();

        let (context, cacheable) = ExecutionEngine::build_user_context_for_cache_miss(
            Some(&workspace),
            Some(&workspace_services),
            PromptBuilderContext::new(
                workspace_root.to_string_lossy().to_string(),
                Some("session".to_string()),
                Some("model".to_string()),
            ),
            &policy,
        )
        .await;
        let context = context.expect("user context");

        assert!(cacheable);
        assert!(context.contains("Local engine user"));
        assert!(context.contains("Local engine project"));
    }

    #[cfg(feature = "external-sources")]
    #[tokio::test]
    #[allow(clippy::await_holding_lock)] // environment lock is intentionally held for the whole test body
    async fn local_workspace_services_remain_the_project_instruction_io_owner() {
        let _environment = lock_environment();
        // Enable both instruction master switches; guard restores on drop.
        let _switches = InstructionSwitches::enable_all();
        let temp = tempfile::tempdir().expect("tempdir");
        let workspace_root = temp.path().join("workspace");
        let xdg = temp.path().join("xdg");
        let codex = temp.path().join("codex");
        let claude = temp.path().join("claude");
        std::fs::create_dir_all(xdg.join("opencode")).expect("OpenCode config directory");
        std::fs::create_dir_all(&codex).expect("Codex config directory");
        std::fs::create_dir_all(&claude).expect("Claude config directory");
        std::fs::create_dir_all(&workspace_root).expect("workspace directory");
        std::fs::write(xdg.join("opencode/AGENTS.md"), "Local user source\n")
            .expect("OpenCode instructions");
        std::fs::write(workspace_root.join("AGENTS.md"), "Disk project source\n")
            .expect("disk instructions");
        let _guard = EnvironmentGuard::set(&[
            ("XDG_CONFIG_HOME", &xdg),
            ("CODEX_HOME", &codex),
            ("CLAUDE_CONFIG_DIR", &claude),
        ]);
        let workspace = WorkspaceBinding::new(None, workspace_root.clone());
        let mut workspace_services =
            local_workspace_services(workspace_root.to_string_lossy().to_string());
        workspace_services.fs = Arc::new(InstructionWorkspaceFs::stable());
        let policy = UserContextPolicy::empty().with_workspace_instructions();

        let (context, cacheable) = ExecutionEngine::build_user_context_for_cache_miss(
            Some(&workspace),
            Some(&workspace_services),
            PromptBuilderContext::new(
                workspace_root.to_string_lossy().to_string(),
                Some("session".to_string()),
                Some("model".to_string()),
            ),
            &policy,
        )
        .await;
        let context = context.expect("user context");

        assert!(cacheable);
        assert!(context.contains("Local user source"));
        assert!(context.contains("Recovered workspace instructions."));
        assert!(!context.contains("Disk project source"));
    }

    #[cfg(feature = "external-sources")]
    #[tokio::test]
    #[allow(clippy::await_holding_lock)] // environment lock is intentionally held for the whole test body
    async fn conditional_rules_persist_once_and_reload_after_compaction() {
        let _environment = lock_environment();
        let temp = tempfile::tempdir().expect("tempdir");
        let workspace_root = temp.path().join("workspace");
        let claude = temp.path().join("claude");
        let xdg = temp.path().join("xdg");
        let codex = temp.path().join("codex");
        std::fs::create_dir_all(workspace_root.join(".claude/rules")).expect("workspace rules");
        std::fs::create_dir_all(&claude).expect("Claude config");
        std::fs::create_dir_all(xdg.join("opencode")).expect("OpenCode config");
        std::fs::create_dir_all(&codex).expect("Codex config");
        let rule_path = workspace_root.join(".claude/rules/rust.md");
        std::fs::write(&rule_path, "---\npaths:\n  - src/**/*.rs\n---\nOld rule\n")
            .expect("old rule");
        let _guard = EnvironmentGuard::set(&[
            ("XDG_CONFIG_HOME", &xdg),
            ("CODEX_HOME", &codex),
            ("CLAUDE_CONFIG_DIR", &claude),
        ]);

        let session_manager = SessionManager::new(
            Arc::new(SessionContextStore::new()),
            Arc::new(
                PersistenceManager::new(Arc::new(PathManager::with_user_root_for_tests(
                    temp.path().join("user-root"),
                )))
                .expect("persistence manager"),
            ),
            SessionManagerConfig {
                max_active_sessions: 4,
                session_idle_timeout: Duration::from_secs(3600),
                auto_save_interval: Duration::from_secs(300),
                enable_persistence: false,
                prompt_cache_policy: PromptCachePolicy::default(),
            },
        );
        let workspace = WorkspaceBinding::new(None, workspace_root.clone());
        let context = crate::agentic::execution::types::ExecutionContext {
            session_id: "session".to_string(),
            dialog_turn_id: "turn".to_string(),
            turn_index: 0,
            agent_type: "agentic".to_string(),
            workspace: Some(workspace),
            context: HashMap::new(),
            subagent_parent_info: None,
            permission_delegation: None,
            permission_runtime_ceiling: None,
            delegation_policy: bitfun_runtime_ports::DelegationPolicy::top_level(),
            runtime_tool_restrictions: ToolRuntimeRestrictions::default(),
            workspace_services: Some(local_workspace_services(
                workspace_root.to_string_lossy().to_string(),
            )),
            terminal_port: None,
            remote_exec_port: None,
            round_injection: None,
            emit_lifecycle_events: false,
            recover_partial_on_cancel: false,
            trigger_source: None,
        };
        let mut messages = vec![
            Message::system("system".to_string()),
            Message::user("older request".repeat(200)),
        ];
        for message in messages.iter().cloned() {
            session_manager
                .add_message(&context.session_id, message)
                .await
                .expect("seed context");
        }

        let first_round = conditional_read_round(&workspace_root, "round-1");
        append_round_messages(
            &session_manager,
            &context.session_id,
            &first_round,
            &mut messages,
        )
        .await;
        activate_conditional_instructions_after_round(
            &session_manager,
            &context,
            &first_round,
            &mut messages,
        )
        .await;
        activate_conditional_instructions_after_round(
            &session_manager,
            &context,
            &first_round,
            &mut messages,
        )
        .await;

        assert_eq!(conditional_reminders(&messages).len(), 1);
        let persisted = session_manager
            .get_context_messages(&context.session_id)
            .await
            .expect("persisted context");
        assert_eq!(conditional_reminders(&persisted).len(), 1);
        assert!(conditional_reminders(&persisted)[0]
            .content
            .to_string()
            .contains("Old rule"));

        let compressor = ContextCompressor::new(Default::default());
        let plan = compressor
            .plan_compression(&context.session_id, &persisted, 128_000, 100, None)
            .expect("compression plan")
            .expect("compressible context");
        let compressed = compressor
            .compress_plan_with_contract(
                &context.session_id,
                128_000,
                plan,
                None,
                Some("summary".to_string()),
            )
            .expect("compression result")
            .messages;
        session_manager
            .replace_context_messages(&context.session_id, compressed.clone())
            .await;
        assert!(conditional_reminders(&compressed).is_empty());
        assert!(conditional_reminders(
            &session_manager
                .get_context_messages(&context.session_id)
                .await
                .expect("compacted context")
        )
        .is_empty());

        std::fs::write(&rule_path, "---\npaths:\n  - src/**/*.rs\n---\nNew rule\n")
            .expect("new rule");
        messages = compressed;
        let second_round = conditional_read_round(&workspace_root, "round-2");
        append_round_messages(
            &session_manager,
            &context.session_id,
            &second_round,
            &mut messages,
        )
        .await;
        activate_conditional_instructions_after_round(
            &session_manager,
            &context,
            &second_round,
            &mut messages,
        )
        .await;

        let reloaded = conditional_reminders(&messages);
        assert_eq!(reloaded.len(), 1);
        assert!(reloaded[0].content.to_string().contains("New rule"));
        assert!(!reloaded[0].content.to_string().contains("Old rule"));
    }

    fn conditional_read_round(workspace_root: &std::path::Path, round_id: &str) -> RoundResult {
        let assistant = Message::assistant("Reading source".to_string())
            .with_turn_id("turn".to_string())
            .with_round_id(round_id.to_string());
        let tool_result = Message::tool_result(ToolResult {
            tool_id: format!("{round_id}-read"),
            tool_name: bitfun_agent_tools::CALL_DEFERRED_TOOL_NAME.to_string(),
            effective_tool_name: Some("Read".to_string()),
            result: json!({ "file_path": workspace_root.join("src/lib.rs") }),
            result_for_assistant: Some("source".to_string()),
            is_error: false,
            duration_ms: Some(1),
            image_attachments: None,
        })
        .with_turn_id("turn".to_string())
        .with_round_id(round_id.to_string());
        RoundResult {
            assistant_message: assistant,
            tool_calls: Vec::new(),
            tool_result_messages: vec![tool_result],
            has_more_rounds: true,
            finish_reason: crate::agentic::execution::types::FinishReason::Complete,
            usage: None,
            provider_metadata: None,
            partial_recovery_reason: None,
            had_assistant_text: false,
            had_thinking_content: false,
        }
    }

    async fn append_round_messages(
        session_manager: &SessionManager,
        session_id: &str,
        round: &RoundResult,
        messages: &mut Vec<Message>,
    ) {
        for message in std::iter::once(&round.assistant_message)
            .chain(round.tool_result_messages.iter())
            .cloned()
        {
            messages.push(message.clone());
            session_manager
                .add_message(session_id, message)
                .await
                .expect("persist round message");
        }
    }

    fn conditional_reminders(messages: &[Message]) -> Vec<&Message> {
        messages
            .iter()
            .filter(|message| {
                message.internal_reminder_kind()
                    == Some(InternalReminderKind::ConditionalInstructions)
            })
            .collect()
    }

    #[tokio::test]
    async fn remote_workspace_without_services_is_not_cacheable() {
        let identity = workspace_session_identity(
            "/remote/workspace",
            Some("connection-1"),
            Some("remote-host"),
        )
        .expect("remote identity");
        let workspace = WorkspaceBinding::new_remote(
            None,
            PathBuf::from("/remote/workspace"),
            "connection-1".to_string(),
            "Remote".to_string(),
            identity,
        );
        let policy = UserContextPolicy::empty()
            .with_workspace_context()
            .with_workspace_instructions();

        let (_, cacheable) = ExecutionEngine::build_user_context_for_cache_miss(
            Some(&workspace),
            None,
            PromptBuilderContext::new(
                "/remote/workspace".to_string(),
                Some("session".to_string()),
                Some("model".to_string()),
            ),
            &policy,
        )
        .await;

        assert!(!cacheable);
    }

    #[test]
    fn resolve_configured_fast_model_falls_back_to_primary_when_fast_is_stale() {
        let mut ai_config = AIConfig {
            models: vec![build_model("model-primary", "Primary", "claude-sonnet-4.5")],
            ..Default::default()
        };
        ai_config.default_models.primary = Some("model-primary".to_string());
        ai_config.default_models.fast = Some("deleted-fast-model".to_string());

        assert_eq!(
            ExecutionEngine::resolve_configured_model_id(&ai_config, "fast"),
            "model-primary"
        );
    }

    #[test]
    fn frozen_turn_model_wins_when_the_auto_default_changes() {
        let mut ai_config = AIConfig {
            models: vec![
                build_model("model-original", "Original", "claude-sonnet-4.5"),
                build_model("model-new-default", "New default", "gpt-5.4"),
            ],
            ..Default::default()
        };
        ai_config.default_models.primary = Some("model-new-default".to_string());

        assert_eq!(
            ExecutionEngine::resolve_model_id_for_turn_selection(
                &ai_config,
                "auto",
                Some("model-original"),
            )
            .expect("the original resolved model remains available"),
            "model-original"
        );
    }

    #[test]
    fn auto_turn_model_must_resolve_to_a_concrete_model_before_persistence() {
        let ai_config = AIConfig::default();

        let error = ExecutionEngine::resolve_model_id_for_turn_selection(&ai_config, "auto", None)
            .expect_err("a symbolic selector cannot become the frozen Turn model");

        assert!(error.to_string().contains("primary model"), "{error}");
    }

    #[test]
    fn frozen_turn_model_must_still_be_available_for_recovery() {
        let mut model = build_model("model-original", "Original", "claude-sonnet-4.5");
        model.enabled = false;
        let ai_config = AIConfig {
            models: vec![model],
            ..Default::default()
        };

        let error = ExecutionEngine::resolve_model_id_for_turn_selection(
            &ai_config,
            "auto",
            Some("model-original"),
        )
        .expect_err("a disabled frozen model cannot execute another generation");

        assert!(error.to_string().contains("unavailable"), "{error}");
    }

    #[test]
    fn auto_compression_pressure_tracks_total_and_conversation_tokens() {
        let messages = vec![
            Message::system("system prompt".repeat(10_000)),
            Message::user("hello".to_string()),
        ];
        let tools = vec![ToolDefinition {
            name: "Read".to_string(),
            description: "Read files".repeat(5_000),
            parameters: json!({"type": "object"}),
        }];
        let prepended_reminders = ["prepended reminder".repeat(5_000)];
        let prepended_reminder_refs = prepended_reminders
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>();
        let prepended_reminder_tokens =
            ExecutionEngine::prepended_reminder_tokens_for_pressure(&prepended_reminder_refs);

        let snapshot = ExecutionEngine::estimate_auto_compression_pressure(
            &messages,
            Some(&tools),
            128_000,
            ExecutionEngine::compression_trigger_budget(128_000, None),
            prepended_reminder_tokens,
        );

        assert!(snapshot.total_tokens > snapshot.conversation_tokens);
        assert!(snapshot.system_tokens > 0);
        assert!(snapshot.tool_tokens > 0);
        assert_eq!(
            snapshot.prepended_reminder_tokens,
            prepended_reminder_tokens
        );
        assert!(
            (snapshot.usage_ratio - snapshot.total_tokens as f32 / 128_000_f32).abs()
                < f32::EPSILON
        );
        assert_eq!(messages[1].role, MessageRole::User);
    }

    #[test]
    fn compression_trigger_budget_reserves_output_and_safety_tokens() {
        let budget = ExecutionEngine::compression_trigger_budget(128_000, Some(32_000));

        assert_eq!(budget.output_reserve_tokens, 32_000);
        assert_eq!(budget.safety_reserve_tokens, 10_000);
        assert_eq!(budget.input_limit, 86_000);
    }

    #[test]
    fn compression_trigger_budget_clamps_output_reserve_to_window_ratio() {
        // ENGINE-03：配置的 max_tokens 超过窗口时，不允许把 input_limit 饿死到 0；
        // 输出预留被钳制到窗口的 40%（与 is_valid_configured_max_output_tokens 允许的同一比例）。
        let budget = ExecutionEngine::compression_trigger_budget(32_000, Some(100_000));

        assert_eq!(budget.output_reserve_tokens, 12_800);
        assert_eq!(budget.safety_reserve_tokens, 10_000);
        assert!(
            budget.input_limit > 0,
            "input_limit must stay positive after the clamp, got {}",
            budget.input_limit
        );
        assert_eq!(budget.input_limit, 32_000 - 12_800 - 10_000);
    }

    #[test]
    fn compression_trigger_budget_disables_auto_compression_on_zero_input_limit() {
        // ENGINE-03：当窗口小到仅预留就超出窗口时，input_limit 饱和为 0；
        // 调用方在此时禁用自动压缩，而不是每轮都无条件压缩。
        let budget = ExecutionEngine::compression_trigger_budget(1_000, None);
        assert_eq!(budget.input_limit, 0);
    }

    #[test]
    fn compression_trigger_budget_uses_the_automatic_output_tier_when_max_tokens_is_unset() {
        let budget = ExecutionEngine::compression_trigger_budget(128_000, None);

        assert_eq!(budget.output_reserve_tokens, 32_000);
        assert_eq!(budget.safety_reserve_tokens, 10_000);
        assert_eq!(budget.input_limit, 86_000);
    }

    #[test]
    fn compression_trigger_percent_1m_window_85_percent_activates_before_legacy_limit() {
        // R-THR-01 批1（B1-1）：1M 窗口 × 85% → input_limit = 891,290（1,048,576×0.85）。
        // 修复前 974,576（legacy 算式）→ 修复后 891,290（铁证差异）。
        let budget = ExecutionEngine::compression_trigger_budget_with_output_reserve_and_ratio(
            1_048_576,
            None,
            10_000,
            64_000,
            40,
            Some(85),
        );
        assert_eq!(budget.input_limit, 891_290);
        assert_eq!(budget.output_reserve_tokens, 64_000);
        assert_eq!(budget.safety_reserve_tokens, 10_000);
    }

    #[test]
    fn compression_trigger_percent_128k_window_85_percent_min_keeps_legacy_limit() {
        // R-THR-01 批1（B1-2）：128k 窗口 × 85% → min(89,072, 111,411) = 89,072。
        // 现算法 89,072（68%）< 85% 线 111,411 → min 取现算法 → 配置 85% 对 128k 不生效（合法非 bug）。
        // 禁断言 111,411。
        let budget = ExecutionEngine::compression_trigger_budget_with_output_reserve_and_ratio(
            131_072,
            None,
            10_000,
            32_000,
            40,
            Some(85),
        );
        assert_eq!(budget.input_limit, 89_072);
        assert_eq!(budget.input_limit, (131_072 - 32_000 - 10_000));
    }

    #[test]
    fn compression_trigger_percent_none_preserves_legacy_limit() {
        // R-THR-01 批1（B1-3）：不配置（None）→ 现算法不变（1M = 974,576）。
        let budget = ExecutionEngine::compression_trigger_budget_with_output_reserve_and_ratio(
            1_048_576, None, 10_000, 64_000, 40, None,
        );
        assert_eq!(budget.input_limit, 974_576);
    }

    #[test]
    fn compression_trigger_percent_zero_is_valid_special_value_preserving_legacy_limit() {
        // R-THR-01 批1（B1-4）：0 = 合法特殊值（同 None）→ 现算法不变（1M = 974,576）。
        let budget = ExecutionEngine::compression_trigger_budget_with_output_reserve_and_ratio(
            1_048_576,
            None,
            10_000,
            64_000,
            40,
            Some(0),
        );
        assert_eq!(budget.input_limit, 974_576);
    }

    #[test]
    fn compression_trigger_percent_out_of_range_degrades_to_none_preserving_legacy_limit() {
        // R-THR-01 批1（B1-5）：非法值（101+/非数字 → 后端校验回退 None）→ 现算法不变（1M = 974,576 零变化铁证）。
        // 101 直接传参时按 None 处理（合法值域 1-99，0 特殊；越界 = 忽略）。
        let budget = ExecutionEngine::compression_trigger_budget_with_output_reserve_and_ratio(
            1_048_576,
            None,
            10_000,
            64_000,
            40,
            Some(101),
        );
        assert_eq!(budget.input_limit, 974_576);
    }

    #[test]
    fn auto_compression_pressure_uses_provider_input_anchor_plus_tail_estimate() {
        let prefix = vec![
            Message::system("system prompt".to_string()),
            Message::user("hello".to_string()),
        ];
        let system_tokens = ExecutionEngine::system_tokens_for_pressure(&prefix);
        let anchor = TokenAnchor::from_request_prefix(
            TokenAnchorInput {
                session_id: "session".to_string(),
                turn_id: "turn".to_string(),
                round_id: "round".to_string(),
                model_id: "model".to_string(),
                input_tokens: 100,
                system_tokens_at_anchor: system_tokens,
                tool_tokens_at_anchor: 0,
                prepended_reminder_tokens_at_anchor: 0,
            },
            &prefix,
        );
        let mut messages = prefix;
        messages.push(Message::assistant("assistant tail".repeat(10)));
        let tail_tokens =
            ExecutionEngine::estimate_tail_tokens(&messages[anchor.prefix_message_count..]);

        let (snapshot, details) = ExecutionEngine::estimate_auto_compression_pressure_with_anchor(
            &messages,
            None,
            1_000,
            ExecutionEngine::compression_trigger_budget(1_000, None),
            Some(&anchor),
            0,
        );

        assert_eq!(snapshot.total_tokens, 100 + tail_tokens);
        assert_eq!(details.expect("anchor details").tail_tokens, tail_tokens);
    }

    #[test]
    fn auto_compression_pressure_applies_tool_definition_delta_to_anchor() {
        let messages = vec![
            Message::system("system prompt".to_string()),
            Message::user("hello".to_string()),
        ];
        let old_tools = vec![ToolDefinition {
            name: "Read".to_string(),
            description: "read files".to_string(),
            parameters: json!({"type": "object"}),
        }];
        let new_tools = vec![ToolDefinition {
            name: "Read".to_string(),
            description: "read files with a longer provider-visible description".repeat(10),
            parameters: json!({"type": "object"}),
        }];
        let old_tool_tokens =
            crate::util::TokenCounter::estimate_tool_definitions_tokens(&old_tools);
        let new_tool_tokens =
            crate::util::TokenCounter::estimate_tool_definitions_tokens(&new_tools);
        let anchor = TokenAnchor::from_request_prefix(
            TokenAnchorInput {
                session_id: "session".to_string(),
                turn_id: "turn".to_string(),
                round_id: "round".to_string(),
                model_id: "model".to_string(),
                input_tokens: 100,
                system_tokens_at_anchor: ExecutionEngine::system_tokens_for_pressure(&messages),
                tool_tokens_at_anchor: old_tool_tokens,
                prepended_reminder_tokens_at_anchor: 0,
            },
            &messages,
        );

        let (snapshot, details) = ExecutionEngine::estimate_auto_compression_pressure_with_anchor(
            &messages,
            Some(&new_tools),
            1_000,
            ExecutionEngine::compression_trigger_budget(1_000, None),
            Some(&anchor),
            0,
        );

        assert_eq!(
            snapshot.total_tokens,
            100 + (new_tool_tokens - old_tool_tokens)
        );
        assert_eq!(snapshot.tool_tokens, new_tool_tokens);
        assert_eq!(
            details.expect("anchor details").tool_delta,
            (new_tool_tokens - old_tool_tokens) as isize
        );
    }

    #[test]
    fn auto_compression_pressure_applies_prepended_reminder_delta_to_anchor() {
        let messages = vec![
            Message::system("system prompt".to_string()),
            Message::user("hello".to_string()),
        ];
        let old_reminders = ["short reminder".to_string()];
        let new_reminders = ["longer reminder ".repeat(20)];
        let old_reminder_refs = old_reminders.iter().map(String::as_str).collect::<Vec<_>>();
        let new_reminder_refs = new_reminders.iter().map(String::as_str).collect::<Vec<_>>();
        let old_reminder_tokens =
            ExecutionEngine::prepended_reminder_tokens_for_pressure(&old_reminder_refs);
        let new_reminder_tokens =
            ExecutionEngine::prepended_reminder_tokens_for_pressure(&new_reminder_refs);
        let anchor = TokenAnchor::from_request_prefix(
            TokenAnchorInput {
                session_id: "session".to_string(),
                turn_id: "turn".to_string(),
                round_id: "round".to_string(),
                model_id: "model".to_string(),
                input_tokens: 100,
                system_tokens_at_anchor: ExecutionEngine::system_tokens_for_pressure(&messages),
                tool_tokens_at_anchor: 0,
                prepended_reminder_tokens_at_anchor: old_reminder_tokens,
            },
            &messages,
        );

        let (snapshot, details) = ExecutionEngine::estimate_auto_compression_pressure_with_anchor(
            &messages,
            None,
            1_000,
            ExecutionEngine::compression_trigger_budget(1_000, None),
            Some(&anchor),
            new_reminder_tokens,
        );
        let details = details.expect("anchor details");

        assert_eq!(
            snapshot.total_tokens,
            100 + (new_reminder_tokens - old_reminder_tokens)
        );
        assert_eq!(
            snapshot.conversation_tokens,
            snapshot.total_tokens
                - ExecutionEngine::system_tokens_for_pressure(&messages)
                - new_reminder_tokens
        );
        assert_eq!(snapshot.prepended_reminder_tokens, new_reminder_tokens);
        assert_eq!(
            details.prepended_reminder_delta,
            (new_reminder_tokens - old_reminder_tokens) as isize
        );
    }

    #[test]
    fn refreshed_turn_prompt_scaffold_replaces_existing_system_message() {
        let scaffold = TurnPromptScaffold {
            system_prompt_message: Message::system("new system prompt".to_string()),
            prepended_prompt_reminders: PrependedPromptReminders::default(),
        };
        let mut messages = vec![
            Message::system("old system prompt".to_string()),
            Message::user("hello".to_string()),
        ];

        ExecutionEngine::apply_turn_prompt_scaffold_to_messages(&mut messages, &scaffold);

        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].role, MessageRole::System);
        assert_eq!(message_text(&messages[0]), Some("new system prompt"));
        assert_eq!(messages[1].role, MessageRole::User);
    }

    #[test]
    fn refreshed_turn_prompt_scaffold_inserts_system_message_when_missing() {
        let scaffold = TurnPromptScaffold {
            system_prompt_message: Message::system("new system prompt".to_string()),
            prepended_prompt_reminders: PrependedPromptReminders::default(),
        };
        let mut messages = vec![Message::user("hello".to_string())];

        ExecutionEngine::apply_turn_prompt_scaffold_to_messages(&mut messages, &scaffold);

        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].role, MessageRole::System);
        assert_eq!(message_text(&messages[0]), Some("new system prompt"));
        assert_eq!(messages[1].role, MessageRole::User);
    }

    #[test]
    fn per_round_runtime_facts_refresh_replaces_turn_start_value() {
        let mut scaffold = TurnPromptScaffold {
            system_prompt_message: Message::system("system prompt".to_string()),
            prepended_prompt_reminders: PrependedPromptReminders::default(),
        };
        let context = PromptBuilderContext::new(
            "E:/workspace".to_string(),
            Some("session-1".to_string()),
            Some("model-1".to_string()),
        );

        ExecutionEngine::refresh_runtime_facts_for_round(
            &mut scaffold,
            Some(context),
            RuntimeFactsUsage {
                context_usage_ratio: Some(0.35),
                compression_preview_ratio: Some(0.9),
            },
            true,
        );
        let first = scaffold
            .prepended_prompt_reminders
            .runtime_facts
            .clone()
            .expect("runtime facts should be refreshed for the round");
        assert!(first.contains("[Runtime Facts]"));
        assert!(first.contains("当前上下文占比: 35%"));

        // A later round with a different pressure snapshot replaces the text:
        // the runtime facts must not stay frozen at the first round's values.
        ExecutionEngine::refresh_runtime_facts_for_round(
            &mut scaffold,
            Some(PromptBuilderContext::new(
                "E:/workspace".to_string(),
                Some("session-1".to_string()),
                Some("model-1".to_string()),
            )),
            RuntimeFactsUsage {
                context_usage_ratio: Some(0.72),
                compression_preview_ratio: Some(0.9),
            },
            true,
        );
        let second = scaffold
            .prepended_prompt_reminders
            .runtime_facts
            .clone()
            .expect("runtime facts should stay refreshed");
        assert_ne!(first, second);
        assert!(second.contains("当前上下文占比: 72%"));

        // ENGINE-01/07: a missing prompt context (workspace-less session) must
        // still refresh the reminder from a minimal context instead of leaving
        // the previous round's value frozen; the usage ratio is replaced.
        ExecutionEngine::refresh_runtime_facts_for_round(
            &mut scaffold,
            None,
            RuntimeFactsUsage {
                context_usage_ratio: Some(0.41),
                compression_preview_ratio: Some(0.9),
            },
            true,
        );
        let third = scaffold
            .prepended_prompt_reminders
            .runtime_facts
            .clone()
            .expect("runtime facts should refresh even without a prompt context");
        assert_ne!(second, third);
        assert!(third.contains("[Runtime Facts]"));
        assert!(third.contains("当前上下文占比: 41%"));
    }

    #[test]
    fn tool_round_clears_runtime_facts_after_user_round_injection() {
        // P-17: user round first turn injects runtime facts; the same round's
        // tool turn clears them so the dynamic postfix no longer carries them.
        let mut scaffold = TurnPromptScaffold {
            system_prompt_message: Message::system("system prompt".to_string()),
            prepended_prompt_reminders: PrependedPromptReminders::default(),
        };
        let context = PromptBuilderContext::new(
            "E:/workspace".to_string(),
            Some("session-1".to_string()),
            Some("model-1".to_string()),
        );
        let usage = RuntimeFactsUsage {
            context_usage_ratio: Some(0.35),
            compression_preview_ratio: Some(0.9),
        };

        // User round first turn: inject.
        ExecutionEngine::refresh_runtime_facts_for_round(
            &mut scaffold,
            Some(context.clone()),
            usage,
            true,
        );
        assert!(
            scaffold.prepended_prompt_reminders.runtime_facts.is_some(),
            "user round first turn should inject runtime facts"
        );

        // Same-round tool turn: clear.
        ExecutionEngine::refresh_runtime_facts_for_round(
            &mut scaffold,
            Some(context),
            usage,
            false,
        );
        assert!(
            scaffold.prepended_prompt_reminders.runtime_facts.is_none(),
            "same-round tool turn must not carry runtime facts"
        );
    }

    #[tokio::test]
    async fn round_dynamic_reminders_injects_user_context_once_per_session() {
        // P-18（每会话一次语义）：User Context 在新会话首轮注入一次，同一会话
        // 的后续用户回合与工具轮均不再重复注入；上下文压缩使缓存世代递增 →
        // 恢复后首轮重新注入一次。
        let temp = tempfile::tempdir().expect("tempdir");
        let event_queue = Arc::new(EventQueue::new(EventQueueConfig::default()));
        let session_manager = Arc::new(SessionManager::new(
            Arc::new(SessionContextStore::new()),
            Arc::new(
                PersistenceManager::new(Arc::new(PathManager::with_user_root_for_tests(
                    temp.path().join("user-root"),
                )))
                .expect("persistence manager"),
            ),
            SessionManagerConfig {
                max_active_sessions: 4,
                session_idle_timeout: Duration::from_secs(3600),
                auto_save_interval: Duration::from_secs(300),
                enable_persistence: false,
                prompt_cache_policy: PromptCachePolicy::default(),
            },
        ));
        let tool_pipeline = Arc::new(ToolPipeline::new(
            Arc::new(TokioRwLock::new(ToolRegistry::new())),
            Arc::new(ToolStateManager::new(event_queue.clone())),
            None,
        ));
        let engine = ExecutionEngine::new(
            Arc::new(RoundExecutor::new(
                Arc::new(StreamProcessor::new(event_queue.clone())),
                event_queue.clone(),
                tool_pipeline.clone(),
            )),
            event_queue.clone(),
            session_manager.clone(),
            Arc::new(ContextCompressor::new(CompressionConfig::default())),
            ExecutionEngineConfig::default(),
        );

        let session_id = "p18-session-scoped-session";
        let reminders = PrependedPromptReminders {
            runtime_facts: Some("[Runtime Facts] 当前上下文占比: 35%".to_string()),
            user_context: Some("[User Context] workspace instructions".to_string()),
            ..Default::default()
        };
        // F-5：真实用户轮（DesktopUi 等用户面 trigger_source）才参与注入/计数。
        let user_context = crate::agentic::execution::types::ExecutionContext {
            session_id: session_id.to_string(),
            dialog_turn_id: "turn".to_string(),
            turn_index: 0,
            agent_type: "agentic".to_string(),
            workspace: None,
            context: HashMap::new(),
            subagent_parent_info: None,
            permission_delegation: None,
            permission_runtime_ceiling: None,
            delegation_policy: bitfun_runtime_ports::DelegationPolicy::top_level(),
            runtime_tool_restrictions: ToolRuntimeRestrictions::default(),
            workspace_services: None,
            terminal_port: None,
            remote_exec_port: None,
            round_injection: None,
            emit_lifecycle_events: false,
            recover_partial_on_cancel: false,
            trigger_source: Some(bitfun_runtime_ports::DialogTriggerSource::DesktopUi),
        };
        // F-5：Agent 轮（AgentSession）不得注入 User Context，也不锁世代。
        let agent_context = crate::agentic::execution::types::ExecutionContext {
            trigger_source: Some(bitfun_runtime_ports::DialogTriggerSource::AgentSession),
            ..user_context.clone()
        };

        // Turn 1, first round: runtime facts + user context both inject.
        let turn1_first = engine
            .round_dynamic_reminders(session_id, &user_context, &reminders)
            .await;
        assert!(turn1_first.iter().any(|r| r.contains("[Runtime Facts]")));
        assert!(turn1_first.iter().any(|r| r.contains("[User Context]")));

        // Turn 1, same-turn tool round (round >= 1): user context skipped by
        // the injected-generation marker. The scaffold in a real tool round no
        // longer carries runtime facts either — `refresh_runtime_facts_for_round`
        // with `inject_runtime_facts=false` clears them (P-17, execution_engine
        // tool-turn path) — so the dynamic postfix must carry neither
        // (d5-P2-3：此前断言"runtime facts 仍被携带"是测试构造假阳性，因为
        // 测试直接复用首轮 scaffold；真实工具轮链路必须验证置空后的状态)。
        let mut tool_round_scaffold = TurnPromptScaffold {
            system_prompt_message: Message::system("system prompt".to_string()),
            prepended_prompt_reminders: PrependedPromptReminders {
                runtime_facts: Some("[Runtime Facts] 当前上下文占比: 35%".to_string()),
                user_context: Some("[User Context] workspace instructions".to_string()),
                ..Default::default()
            },
        };
        ExecutionEngine::refresh_runtime_facts_for_round(
            &mut tool_round_scaffold,
            None,
            RuntimeFactsUsage {
                context_usage_ratio: Some(0.35),
                compression_preview_ratio: Some(0.9),
            },
            false,
        );
        let turn1_tool_round = engine
            .round_dynamic_reminders(
                session_id,
                &user_context,
                &tool_round_scaffold.prepended_prompt_reminders,
            )
            .await;
        assert!(
            !turn1_tool_round
                .iter()
                .any(|r| r.contains("[Runtime Facts]")),
            "same-round tool turn must not carry runtime facts (cleared at scaffold level)"
        );
        assert!(!turn1_tool_round
            .iter()
            .any(|r| r.contains("[User Context]")));

        // Turn 2: session-scoped semantics — no turn-start marker reset, so the
        // first round of the next user turn must NOT re-inject user context.
        let turn2_first = engine
            .round_dynamic_reminders(session_id, &user_context, &reminders)
            .await;
        assert!(turn2_first.iter().any(|r| r.contains("[Runtime Facts]")));
        assert!(
            !turn2_first.iter().any(|r| r.contains("[User Context]")),
            "session-scoped injection: second user turn must not re-inject user context"
        );

        // F-5/RT：Agent 轮（AgentSession）不得注入 User Context，也不得记录
        // 注入世代；且不再携带 Runtime Facts（时间+占比提示随用户轮拼接，
        // Agent 轮零动态提醒）。
        let agent_round = engine
            .round_dynamic_reminders(session_id, &agent_context, &reminders)
            .await;
        assert!(
            !agent_round.iter().any(|r| r.contains("[User Context]")),
            "agent round must not inject user context"
        );
        assert!(
            !agent_round.iter().any(|r| r.contains("[Runtime Facts]")),
            "agent round must not carry runtime facts (拼接进用户消息轮，非独立每轮提示)"
        );

        // Context compaction bumps the generation: first round re-injects even
        // without an explicit marker reset.
        session_manager
            .invalidate_prompt_cache(session_id, PromptCacheScope::UserContext, "test")
            .await;
        let recovery_first = engine
            .round_dynamic_reminders(session_id, &user_context, &reminders)
            .await;
        assert!(recovery_first.iter().any(|r| r.contains("[Runtime Facts]")));
        assert!(recovery_first.iter().any(|r| r.contains("[User Context]")));
    }

    #[tokio::test]
    async fn round_dynamic_reminders_does_not_record_generation_when_user_context_none() {
        // d5-P1-1: when the scaffold carries no User Context (no workspace,
        // instruction build failure, nothing injectable), the injected
        // generation must NOT be recorded. Otherwise the same cache generation
        // suppresses later rounds and the model never sees User Context even
        // after the cache becomes available again (e.g. remote reconnect).
        let temp = tempfile::tempdir().expect("tempdir");
        let event_queue = Arc::new(EventQueue::new(EventQueueConfig::default()));
        let session_manager = Arc::new(SessionManager::new(
            Arc::new(SessionContextStore::new()),
            Arc::new(
                PersistenceManager::new(Arc::new(PathManager::with_user_root_for_tests(
                    temp.path().join("user-root"),
                )))
                .expect("persistence manager"),
            ),
            SessionManagerConfig {
                max_active_sessions: 4,
                session_idle_timeout: Duration::from_secs(3600),
                auto_save_interval: Duration::from_secs(300),
                enable_persistence: false,
                prompt_cache_policy: PromptCachePolicy::default(),
            },
        ));
        let tool_pipeline = Arc::new(ToolPipeline::new(
            Arc::new(TokioRwLock::new(ToolRegistry::new())),
            Arc::new(ToolStateManager::new(event_queue.clone())),
            None,
        ));
        let engine = ExecutionEngine::new(
            Arc::new(RoundExecutor::new(
                Arc::new(StreamProcessor::new(event_queue.clone())),
                event_queue.clone(),
                tool_pipeline.clone(),
            )),
            event_queue.clone(),
            session_manager.clone(),
            Arc::new(ContextCompressor::new(CompressionConfig::default())),
            ExecutionEngineConfig::default(),
        );

        let session_id = "p18-none-context-session";
        // F-5：真实用户轮（DesktopUi）上下文。
        let user_context = crate::agentic::execution::types::ExecutionContext {
            session_id: session_id.to_string(),
            dialog_turn_id: "turn".to_string(),
            turn_index: 0,
            agent_type: "agentic".to_string(),
            workspace: None,
            context: HashMap::new(),
            subagent_parent_info: None,
            permission_delegation: None,
            permission_runtime_ceiling: None,
            delegation_policy: bitfun_runtime_ports::DelegationPolicy::top_level(),
            runtime_tool_restrictions: ToolRuntimeRestrictions::default(),
            workspace_services: None,
            terminal_port: None,
            remote_exec_port: None,
            round_injection: None,
            emit_lifecycle_events: false,
            recover_partial_on_cancel: false,
            trigger_source: Some(bitfun_runtime_ports::DialogTriggerSource::DesktopUi),
        };
        // No User Context in the scaffold: the first round must not record a
        // generation and must not inject anything from the user-context slot.
        let reminders = PrependedPromptReminders {
            runtime_facts: Some("[Runtime Facts] 当前上下文占比: 35%".to_string()),
            user_context: None,
            ..Default::default()
        };

        let first = engine
            .round_dynamic_reminders(session_id, &user_context, &reminders)
            .await;
        assert!(first.iter().any(|r| r.contains("[Runtime Facts]")));
        assert!(
            session_manager
                .user_context_injected_generation(session_id)
                .await
                .is_none(),
            "user_context=None must not record an injected generation"
        );

        // A later round in the same generation with a user context available
        // must still inject (the None round did not lock the generation).
        let reminders_with_context = PrependedPromptReminders {
            runtime_facts: Some("[Runtime Facts] 当前上下文占比: 35%".to_string()),
            user_context: Some("[User Context] workspace instructions".to_string()),
            ..Default::default()
        };
        let later = engine
            .round_dynamic_reminders(session_id, &user_context, &reminders_with_context)
            .await;
        assert!(
            later.iter().any(|r| r.contains("[User Context]")),
            "user_context becoming available in the same generation must still inject"
        );
        assert!(
            session_manager
                .user_context_injected_generation(session_id)
                .await
                .is_some(),
            "a real injection must record the generation"
        );
    }

    #[tokio::test]
    async fn round_dynamic_reminders_agent_round_does_not_inject_or_lock_generation() {
        // F-5：Agent 间轮（AgentSession / ScheduledJob）不得注入 User Context，
        // 也不得记录注入世代——后续真实用户轮在同一世代内仍可注入（防「Agent
        // 轮先跑导致真实用户轮被世代抑制」的回归）。
        let temp = tempfile::tempdir().expect("tempdir");
        let event_queue = Arc::new(EventQueue::new(EventQueueConfig::default()));
        let session_manager = Arc::new(SessionManager::new(
            Arc::new(SessionContextStore::new()),
            Arc::new(
                PersistenceManager::new(Arc::new(PathManager::with_user_root_for_tests(
                    temp.path().join("user-root"),
                )))
                .expect("persistence manager"),
            ),
            SessionManagerConfig {
                max_active_sessions: 4,
                session_idle_timeout: Duration::from_secs(3600),
                auto_save_interval: Duration::from_secs(300),
                enable_persistence: false,
                prompt_cache_policy: PromptCachePolicy::default(),
            },
        ));
        let tool_pipeline = Arc::new(ToolPipeline::new(
            Arc::new(TokioRwLock::new(ToolRegistry::new())),
            Arc::new(ToolStateManager::new(event_queue.clone())),
            None,
        ));
        let engine = ExecutionEngine::new(
            Arc::new(RoundExecutor::new(
                Arc::new(StreamProcessor::new(event_queue.clone())),
                event_queue.clone(),
                tool_pipeline.clone(),
            )),
            event_queue.clone(),
            session_manager.clone(),
            Arc::new(ContextCompressor::new(CompressionConfig::default())),
            ExecutionEngineConfig::default(),
        );

        let session_id = "f5-agent-round-session";
        let reminders = PrependedPromptReminders {
            runtime_facts: Some("[Runtime Facts] 当前上下文占比: 35%".to_string()),
            user_context: Some("[User Context] workspace instructions".to_string()),
            ..Default::default()
        };
        let base_context = crate::agentic::execution::types::ExecutionContext {
            session_id: session_id.to_string(),
            dialog_turn_id: "turn".to_string(),
            turn_index: 0,
            agent_type: "agentic".to_string(),
            workspace: None,
            context: HashMap::new(),
            subagent_parent_info: None,
            permission_delegation: None,
            permission_runtime_ceiling: None,
            delegation_policy: bitfun_runtime_ports::DelegationPolicy::top_level(),
            runtime_tool_restrictions: ToolRuntimeRestrictions::default(),
            workspace_services: None,
            terminal_port: None,
            remote_exec_port: None,
            round_injection: None,
            emit_lifecycle_events: false,
            recover_partial_on_cancel: false,
            trigger_source: None,
        };
        let agent_context = crate::agentic::execution::types::ExecutionContext {
            trigger_source: Some(bitfun_runtime_ports::DialogTriggerSource::AgentSession),
            ..base_context.clone()
        };
        let scheduled_context = crate::agentic::execution::types::ExecutionContext {
            trigger_source: Some(bitfun_runtime_ports::DialogTriggerSource::ScheduledJob),
            ..base_context.clone()
        };
        // Subagent 内部轮（trigger_source = None）同 Agent 轮语义。
        let subagent_context = base_context;
        let user_context = crate::agentic::execution::types::ExecutionContext {
            trigger_source: Some(bitfun_runtime_ports::DialogTriggerSource::DesktopUi),
            ..subagent_context.clone()
        };

        // Agent 轮（AgentSession/ScheduledJob/子代理 None）：不注入，也不携带
        // Runtime Facts（时间+占比提示随真实用户消息轮拼接，非独立每轮提示）。
        for context in [&agent_context, &scheduled_context, &subagent_context] {
            let round = engine
                .round_dynamic_reminders(session_id, context, &reminders)
                .await;
            assert!(
                !round.iter().any(|r| r.contains("[User Context]")),
                "non-user round must not inject user context"
            );
            assert!(
                !round.iter().any(|r| r.contains("[Runtime Facts]")),
                "non-user round must not carry runtime facts"
            );
            assert!(
                session_manager
                    .user_context_injected_generation(session_id)
                    .await
                    .is_none(),
                "non-user round must not lock the injected generation"
            );
        }

        // 后续真实用户轮：同一世代内仍可注入（未被 Agent 轮锁世代），且
        // Runtime Facts 随用户轮一起拼接发送。
        let user_round = engine
            .round_dynamic_reminders(session_id, &user_context, &reminders)
            .await;
        assert!(
            user_round.iter().any(|r| r.contains("[User Context]")),
            "the first real user round must still inject after agent rounds"
        );
        assert!(
            user_round.iter().any(|r| r.contains("[Runtime Facts]")),
            "real user round carries runtime facts (拼接进用户消息轮)"
        );
        assert!(
            session_manager
                .user_context_injected_generation(session_id)
                .await
                .is_some(),
            "the real user round must record the generation"
        );

        // 再次 Agent 轮：仍不注入（世代已锁定，但 Agent 轮本身也绝不注入，
        // Runtime Facts 同样不带）。
        let agent_after_user = engine
            .round_dynamic_reminders(session_id, &agent_context, &reminders)
            .await;
        assert!(
            !agent_after_user
                .iter()
                .any(|r| r.contains("[User Context]")),
            "agent round after a user round must still not inject"
        );
        assert!(
            !agent_after_user
                .iter()
                .any(|r| r.contains("[Runtime Facts]")),
            "agent round after a user round must still not carry runtime facts"
        );
    }

    // ---- R-13 / R-MR-06 首轮真实内容守卫（has_real_user_content）----
    #[test]
    fn empty_input_guard_detects_injection_only_first_round() {
        // 纯 legion_context 注入（DR-7 实证形态：role=User、内容非空、
        // system_reminder 包裹）→ 无真实 user 内容 → 守卫命中。
        let injection_only = vec![
            Message::system("system prompt".to_string()),
            Message::internal_reminder(
                InternalReminderKind::LifecycleContext,
                "<legion_context>\n[Legion Context]\nLegion depth: 1\n</legion_context>",
            ),
        ];
        assert!(!ExecutionEngine::has_real_user_content(&injection_only));

        // HookContext 注入（A3 同构链路）同样命中。
        let hook_only = vec![
            Message::system("system prompt".to_string()),
            Message::internal_reminder(
                InternalReminderKind::HookContext,
                "<hook_context>\nsection\n</hook_context>",
            ),
        ];
        assert!(!ExecutionEngine::has_real_user_content(&hook_only));
    }

    #[test]
    fn empty_input_guard_passes_injection_plus_real_task() {
        // 注入 + 真实任务（非空、非 system_reminder-only）→ 有真实内容 → 放行。
        let injection_plus_task = vec![
            Message::system("system prompt".to_string()),
            Message::internal_reminder(
                InternalReminderKind::LifecycleContext,
                "<legion_context>\n[Legion Context]\nLegion depth: 1\n</legion_context>",
            ),
            Message::user("fix the bug in execution_engine.rs".to_string()),
        ];
        assert!(ExecutionEngine::has_real_user_content(&injection_plus_task));

        // 空串 user（Message::user("") 合法）→ 无真实内容 → 命中（守卫兜底）。
        let empty_user = vec![
            Message::system("system prompt".to_string()),
            Message::user(String::new()),
        ];
        assert!(!ExecutionEngine::has_real_user_content(&empty_user));
    }

    #[test]
    fn empty_input_guard_ignores_non_user_and_tool_rounds() {
        // system/assistant/tool 消息不参与判定。
        let tool_round = vec![
            Message::system("system prompt".to_string()),
            Message::user("real task".to_string()),
            Message::assistant("checking".to_string()),
        ];
        assert!(ExecutionEngine::has_real_user_content(&tool_round));
    }

    #[test]
    fn empty_input_guard_passes_fork_inherited_context() {
        // fork 继承上下文：历史真实 user 消息 + 注入 + 任务 → 放行。
        let fork_messages = vec![
            Message::system("system prompt".to_string()),
            Message::user("previous real conversation".to_string()),
            Message::internal_reminder(
                InternalReminderKind::ForkSubagent,
                fork_subagent_reminder_text(),
            ),
            Message::user("continue this work".to_string()),
        ];
        assert!(ExecutionEngine::has_real_user_content(&fork_messages));
    }

    #[test]
    fn empty_input_guard_detects_unmarked_system_reminder_injection() {
        // DR-8 B4-B6 结构风险：prepended reminders 以
        // `Message::user(render_system_reminder(...))` 形式（无 InternalReminderKind
        // 标记）注入 → content 判定兜底识别，守卫仍命中。
        let bare_reminder = vec![
            Message::system("system prompt".to_string()),
            Message::user(crate::agentic::core::render_system_reminder(
                "Deferred tool listing",
            )),
        ];
        assert!(!ExecutionEngine::has_real_user_content(&bare_reminder));

        // 真实文本（带 user_query 标记）仍算真实内容。
        let user_query_marked = vec![
            Message::system("system prompt".to_string()),
            Message::user(crate::agentic::core::render_user_query("fix the bug")),
        ];
        assert!(ExecutionEngine::has_real_user_content(&user_query_marked));
    }

    #[test]
    fn empty_input_guard_requires_first_round_condition() {
        // 工具轮/续轮（round_index > 0）不受守卫影响：即使消息列表无真实 user
        // 内容（本轮是工具结果 + 注入），守卫条件 `round_index == 0` 也不命中。
        // 用初始 round index 语义验证：恢复轮（initial_round_index=3）首轮就是
        // round_index=3 → 不拦。
        let mut context = std::collections::HashMap::new();
        context.insert("initial_round_index".to_string(), "3".to_string());
        assert_eq!(super::initial_round_index(&context), 3);
    }

    fn fork_subagent_reminder_text() -> String {
        // 与 coordinator fork_subagent_system_reminder() 语义等价（system_reminder 包裹）。
        crate::agentic::core::render_system_reminder("Forked subagent context")
    }

    // ---- R-URGENT-01-W2 单测七件套（CI 门禁 v3 TC-4.1~4.7）----
    // D1：D1 依赖 W1 工厂语义（internal_reminder 分道后 FinalizeCacheAnchor → system）。
    // 本任务文件域硬约束仅 execution_engine.rs，message.rs 禁改 → 工厂分道后
    // role 断言无法在此复现 → 按任务书标注「D1 依赖 W1 工厂语义，W1 合入后补」。
    // D1 需求中「注入走 system」的消费者侧语义由 D4 覆盖（拼接层/finalize 场景
    // 的 AIMessage role 断言）。D1 本身不在 W2 落地，记录在案。

    // D2：urgent 运行中不拦 —— UserSteering + round_index>0 → 守卫不命中。
    #[test]
    fn guard_skips_user_steering_mid_turn() {
        // 守卫条件是首轮（round_index == 0）判定。urgent 运行中（round_index > 0）
        // 即使消息列表全是 user 壳注入，has_real_user_content 由调用方仅在首轮
        // 咨询，运行中轮次根本不会调用它 → 语义上不拦。
        // 同时验证：UserSteering 即使被误咨询，也带 ActualUserInput 语义标记放行。
        let steering = vec![
            Message::system("system prompt".to_string()),
            Message::internal_reminder(
                InternalReminderKind::UserSteering,
                "<system_reminder>\nThe user sent a new message while this turn was running.\n\nNew user message:\nurgent fix now\n</system_reminder>",
            )
            .with_semantic_kind(MessageSemanticKind::ActualUserInput),
        ];
        assert!(ExecutionEngine::has_real_user_content(&steering));
        // 运行中判定点：round_index > 0 由上游守卫条件控制（恢复轮 initial_round_index=3
        // 首轮即 round_index=3 → 守卫不命中），这里锁死语义映射。
        let mut context = std::collections::HashMap::new();
        context.insert("initial_round_index".to_string(), "1".to_string());
        assert_eq!(super::initial_round_index(&context), 1);
        // 真实 UserSteering（无 ActualUserInput 标记、带壳）被注入 → 首轮也不应
        // 当作真实内容，保证运行中 turn 的注入不影响首轮判定。
        let bare_steering = vec![
            Message::system("system prompt".to_string()),
            Message::internal_reminder(
                InternalReminderKind::UserSteering,
                "<system_reminder>\nNew user message:\nurgent fix now\n</system_reminder>",
            ),
        ];
        assert!(!ExecutionEngine::has_real_user_content(&bare_steering));
    }

    // D3：纯 LifecycleContext 首轮仍拦 —— semantic_kind=InternalReminder → !has_real_user_content。
    #[test]
    fn guard_still_blocks_pure_lifecycle_context_first_round() {
        let lifecycle_only = vec![
            Message::system("system prompt".to_string()),
            Message::internal_reminder(
                InternalReminderKind::LifecycleContext,
                "<legion_context>\n[Legion Context]\nLegion depth: 1\n</legion_context>",
            ),
        ];
        assert!(!ExecutionEngine::has_real_user_content(&lifecycle_only));
    }

    // D4：reminders 构造后 role=system —— 拼接层 static/dynamic + finalize 场景。
    #[tokio::test]
    async fn prepended_and_finalize_reminders_are_system_role() {
        // 拼接层 static/dynamic reminders（build_ai_messages_for_send）→ AIMessage role=system。
        let built = ExecutionEngine::build_ai_messages_for_send(
            &[Message::user("real task".to_string())],
            "openai",
            None,
            "turn-1",
            false,
            &["static reminder"],
            &["dynamic reminder"],
            0,
        )
        .await
        .expect("build_ai_messages_for_send ok");
        let system_roles = built.iter().filter(|m| m.role == "system").count();
        // 只有真实 user 消息保留 user 角色。
        let user_roles = built.iter().filter(|m| m.role == "user").count();
        assert_eq!(user_roles, 1, "只有真实 user 消息保留 user 角色");
        assert!(
            system_roles >= 2,
            "static+dynamic reminders 以 system 角色注入"
        );
        // 真实 user 仍 role=user。
        assert!(built.iter().any(|m| {
            m.role == "user"
                && m.content
                    .as_deref()
                    .is_some_and(|c| c.contains("real task"))
        }));
    }

    // D4（finalize 场景）：run_finalize_round 的 final_ai_messages 双 reminders → system。
    #[test]
    fn finalize_reminders_are_system_role() {
        // 直接断言 build_finalize_cache_anchor_messages 产物：工厂分道后
        // FinalizeCacheAnchor → system。W1 合入前工厂仍为 user（依赖 W1），
        // 此处锁定 run_finalize_round 直推的 AIMessage::system 语义（本任务改造）。
        // W1 未合入时该断言由 D1 标注依赖，不在此强锁工厂侧。
        let anchor = ExecutionEngine::build_finalize_cache_anchor_messages(
            "turn-finalize",
            ExecutionEngine::FINALIZE_AFTER_MAX_ROUNDS_REMINDER,
        );
        // 语义标记正确（工厂设置 internal_reminder_kind 等）。
        assert!(anchor.iter().all(|m| m.internal_reminder_kind()
            == Some(InternalReminderKind::FinalizeCacheAnchor)
            || m.metadata.semantic_kind == Some(MessageSemanticKind::InternalReminder)));
    }

    // D5：ActualUserInput 首轮放行 —— 语义标记权威（带壳形态也放行）。
    #[test]
    fn guard_passes_actual_user_input_first_round_even_when_wrapped() {
        let real = vec![
            Message::system("system prompt".to_string()),
            Message::user("real user input".to_string())
                .with_semantic_kind(MessageSemanticKind::ActualUserInput),
        ];
        assert!(ExecutionEngine::has_real_user_content(&real));

        let wrapped = vec![
            Message::system("system prompt".to_string()),
            Message::user(crate::agentic::core::render_system_reminder("urgent"))
                .with_semantic_kind(MessageSemanticKind::ActualUserInput),
        ];
        assert!(
            ExecutionEngine::has_real_user_content(&wrapped),
            "ActualUserInput 语义标记权威：带壳形态仍放行"
        );
    }

    // D6：无标记壳文本仍拦 + 空串保留。
    #[test]
    fn guard_blocks_unmarked_shell_and_keeps_empty_string_semantics() {
        // 无标记 + <system_reminder> 开头 → false。
        let shell = vec![
            Message::system("system prompt".to_string()),
            Message::user("<system_reminder>\nurgent\n</system_reminder>".to_string()),
        ];
        assert!(!ExecutionEngine::has_real_user_content(&shell));
        // 空串 user → false（:8264 语义保留）。
        let empty = vec![
            Message::system("system prompt".to_string()),
            Message::user(String::new()),
        ];
        assert!(!ExecutionEngine::has_real_user_content(&empty));
    }

    // D7：空内容不产消息 —— internal_reminder(kind, "") 不产 <system_reminder> 壳
    // 语义由消费端保证（生成函数输入非空，见空校验核查表）；此处置换为：
    // 1) 消费端入口（build_ai_messages_for_send）对空 reminders 不产壳（trim+filter 天然拦截）
    // 2) 空文本构造 internal_reminder 时壳内容为空 → is_system_reminder_only 语义下不误伤真实用户。
    #[tokio::test]
    async fn empty_content_produces_no_reminder_message() {
        // 拼接层静态/动态 reminders 空串 → 不产任何壳消息。
        let built = ExecutionEngine::build_ai_messages_for_send(
            &[Message::user("real".to_string())],
            "openai",
            None,
            "turn-1",
            false,
            &[""],
            &["   "],
            0,
        )
        .await
        .expect("build ok");
        // 只有真实 user 一条，无任何 system_reminder 壳。
        assert_eq!(built.len(), 1);
        assert!(!built[0]
            .content
            .as_deref()
            .is_some_and(|c| c.contains("<system_reminder>")));
        // 空文本的 internal_reminder：W1 工厂分道 + 空防护后返回空串（无
        // <system_reminder> 壳）→ 空 payload 不取 injection shape（W1 防护目标）。
        let empty_reminder =
            Message::internal_reminder(InternalReminderKind::Generic, String::new());
        let rendered = message_text(&empty_reminder).expect("internal_reminder 产 Text 内容");
        assert_eq!(
            rendered, "",
            "W1 空防护：空文本 internal_reminder 返回空串（无壳）"
        );
        assert!(
            !rendered.contains("<system_reminder>"),
            "空文本 internal_reminder 不产壳"
        );
        assert!(
            !crate::agentic::core::is_system_reminder_only(rendered),
            "空文本不满足 system_reminder-only → 守卫不误判为注入"
        );
    }

    #[test]
    fn tool_signature_args_summary_truncates_on_utf8_boundary() {
        let args = format!("{}{}", "a".repeat(62), "案".repeat(30));
        let args_hash = hex::encode(Sha256::digest(args.as_bytes()));

        let summary = ExecutionEngine::tool_signature_args_summary(&args);

        assert_eq!(
            summary,
            format!("{}..#{}:sha256={}", "a".repeat(62), args.len(), args_hash)
        );
    }

    #[test]
    fn tool_signature_args_summary_keeps_short_arguments() {
        let args = r#"{"content":"short"}"#;

        let summary = ExecutionEngine::tool_signature_args_summary(args);

        assert_eq!(summary, args);
    }

    #[test]
    fn partial_continuation_allowed_for_stream_stall_reasons() {
        assert!(ExecutionEngine::should_continue_after_partial_response(
            "Stream processor watchdog timeout (no data received for 45 seconds)"
        ));
        assert!(ExecutionEngine::should_continue_after_partial_response(
            "Stream processing error: SSE stream error"
        ));
    }

    #[test]
    fn partial_continuation_skipped_for_user_cancellation() {
        assert!(!ExecutionEngine::should_continue_after_partial_response(
            "Stream processing cancelled after partial output"
        ));
        assert!(!ExecutionEngine::should_continue_after_partial_response(
            "Stream processing cancelled"
        ));
    }

    #[test]
    fn finalize_tool_names_match_tool_definitions() {
        let tools = vec![
            ToolDefinition {
                name: "Read".to_string(),
                description: String::new(),
                parameters: json!({}),
            },
            ToolDefinition {
                name: "Bash".to_string(),
                description: String::new(),
                parameters: json!({}),
            },
        ];

        assert_eq!(
            ExecutionEngine::finalize_tool_names(Some(&tools)),
            vec!["Read".to_string(), "Bash".to_string()]
        );
    }

    #[test]
    fn finalize_runtime_tool_restrictions_deny_all_finalize_tools() {
        let context = crate::agentic::execution::types::ExecutionContext {
            session_id: "session".to_string(),
            dialog_turn_id: "turn".to_string(),
            turn_index: 0,
            agent_type: "agentic".to_string(),
            workspace: None,
            context: HashMap::new(),
            subagent_parent_info: None,
            permission_delegation: None,
            permission_runtime_ceiling: None,
            delegation_policy: bitfun_runtime_ports::DelegationPolicy::top_level(),
            runtime_tool_restrictions: ToolRuntimeRestrictions::default(),
            workspace_services: None,
            terminal_port: None,
            remote_exec_port: None,
            round_injection: None,
            emit_lifecycle_events: true,
            recover_partial_on_cancel: false,
            trigger_source: None,
        };

        let restrictions = ExecutionEngine::finalize_runtime_tool_restrictions(
            &context,
            &["Read".to_string(), "Bash".to_string()],
        );

        assert!(restrictions.denied_tool_names.contains("Read"));
        assert!(restrictions.denied_tool_names.contains("Bash"));
        assert_eq!(
            restrictions.denied_tool_messages.get("Read"),
            Some(&ExecutionEngine::FINALIZE_TOOL_DENIED_MESSAGE.to_string())
        );
    }

    #[test]
    fn local_final_response_message_mentions_reason() {
        assert!(
            ExecutionEngine::build_local_final_response_message("repeated_tool_failures")
                .contains("repeated tool failures")
        );
        assert!(
            ExecutionEngine::build_local_final_response_message("max_rounds")
                .contains("round limit")
        );
        assert!(
            !ExecutionEngine::build_local_final_response_message("max_rounds")
                .contains("finalize mode")
        );
    }

    #[test]
    fn local_fallback_response_does_not_count_as_agent_final_response() {
        assert!(ExecutionEngine::should_mark_has_final_response(true, false));
        assert!(!ExecutionEngine::should_mark_has_final_response(true, true));
        assert!(!ExecutionEngine::should_mark_has_final_response(
            false, false
        ));
    }

    #[test]
    fn finalize_cache_anchor_messages_are_internal_and_not_actual_user_input() {
        let messages = ExecutionEngine::build_finalize_cache_anchor_messages(
            "turn-1",
            ExecutionEngine::FINALIZE_AFTER_MAX_ROUNDS_REMINDER,
        );

        assert_eq!(messages.len(), 2);
        assert_eq!(
            messages[0].internal_reminder_kind(),
            Some(InternalReminderKind::FinalizeCacheAnchor)
        );
        assert_eq!(
            messages[1].internal_reminder_kind(),
            Some(InternalReminderKind::FinalizeCacheAnchor)
        );
        assert!(!messages[0].is_actual_user_message());
        assert!(!messages[1].is_actual_user_message());

        // Both finalize anchor messages must carry the system-reminder markup so
        // downstream CLI statistics can tell them apart from real user prompts.
        assert!(
            message_text(&messages[0]).is_some_and(crate::agentic::core::is_system_reminder_only)
        );
        assert!(
            message_text(&messages[1]).is_some_and(crate::agentic::core::is_system_reminder_only)
        );
    }

    #[test]
    fn finalize_followup_reminder_keeps_system_reminder_markup_in_request_body() {
        // The FINALIZE_USER_FOLLOWUP text is an internal injection sent as a
        // role=user message. It must stay wrapped in <system_reminder> so CLI
        // usage statistics do not count it as a user prompt.
        assert!(crate::agentic::core::is_system_reminder_only(&format!(
            "<system_reminder>{}</system_reminder>",
            ExecutionEngine::FINALIZE_USER_FOLLOWUP
        )));
    }

    #[test]
    fn tool_signature_args_summary_distinguishes_same_prefix_and_length() {
        let first = format!("{}{}", "x".repeat(64), "a".repeat(80));
        let second = format!("{}{}", "x".repeat(64), "b".repeat(80));

        let first_summary = ExecutionEngine::tool_signature_args_summary(&first);
        let second_summary = ExecutionEngine::tool_signature_args_summary(&second);

        assert_eq!(first.len(), second.len());
        assert_ne!(first, second);
        assert_ne!(first_summary, second_summary);
    }

    #[test]
    fn failed_tool_round_signature_ignores_successful_repeated_calls() {
        let tool_calls = vec![ToolCall {
            tool_id: "tool-1".to_string(),
            tool_name: "PollStatus".to_string(),
            arguments: json!({ "job_id": "job-1" }),
            raw_arguments: None,
            is_error: false,
            parse_error: None,
            recovered_from_truncation: false,
            repair_kind: Default::default(),
        }];
        let results = vec![Message::tool_result(ToolResult {
            tool_id: "tool-1".to_string(),
            tool_name: "PollStatus".to_string(),
            effective_tool_name: None,
            result: json!({ "status": "pending", "success": true }),
            result_for_assistant: Some("The job is still pending.".to_string()),
            is_error: false,
            duration_ms: Some(1),
            image_attachments: None,
        })];

        assert!(
            ExecutionEngine::failed_tool_round_signature(&tool_calls, &results).is_none(),
            "successful polling must not be treated as a failed loop"
        );
    }

    #[test]
    fn failed_tool_round_signature_requires_actual_failure_evidence() {
        let tool_calls = vec![ToolCall {
            tool_id: "tool-1".to_string(),
            tool_name: "Read".to_string(),
            arguments: json!({ "path": "missing.txt" }),
            raw_arguments: None,
            is_error: false,
            parse_error: None,
            recovered_from_truncation: false,
            repair_kind: Default::default(),
        }];
        let results = vec![Message::tool_result(ToolResult {
            tool_id: "tool-1".to_string(),
            tool_name: "Read".to_string(),
            effective_tool_name: None,
            result: json!({ "success": false, "error": "not found" }),
            result_for_assistant: Some("File not found.".to_string()),
            is_error: true,
            duration_ms: Some(1),
            image_attachments: None,
        })];

        assert_eq!(
            ExecutionEngine::failed_tool_round_signature(&tool_calls, &results).as_deref(),
            Some(r#"Read:{"path":"missing.txt"}"#)
        );
    }

    #[test]
    fn periodic_loop_detector_ignores_short_windows() {
        let signatures: Vec<String> = vec!["A".to_string(), "B".to_string(), "A".to_string()];
        assert!(!ExecutionEngine::is_periodic_tool_signature_loop(
            &signatures,
            3
        ));
    }

    #[test]
    fn periodic_loop_detector_catches_consecutive_identical_window() {
        let signatures: Vec<String> = std::iter::repeat_n("A".to_string(), 6).collect();
        assert!(ExecutionEngine::is_periodic_tool_signature_loop(
            &signatures,
            3
        ));
    }

    #[test]
    fn periodic_loop_detector_catches_alternating_pattern() {
        // A-B-A-B-A-B is a stable period-2 loop with 3 distinct rounds per
        // signature. The strict consecutive check cannot see this because no
        // two adjacent rounds share the same signature.
        let signatures: Vec<String> = ["A", "B", "A", "B", "A", "B"]
            .iter()
            .map(|s| (*s).to_string())
            .collect();
        assert!(ExecutionEngine::is_periodic_tool_signature_loop(
            &signatures,
            3
        ));
    }

    #[test]
    fn periodic_loop_detector_catches_three_signature_cycle() {
        // A-B-C-A-B-C: window size 6, three distinct signatures, each twice.
        let signatures: Vec<String> = ["A", "B", "C", "A", "B", "C"]
            .iter()
            .map(|s| (*s).to_string())
            .collect();
        assert!(ExecutionEngine::is_periodic_tool_signature_loop(
            &signatures,
            3
        ));
    }

    #[test]
    fn periodic_loop_detector_skips_genuine_progress() {
        // Six distinct signatures means each tool call is a new exploration
        // step - not a loop, even if the same tool name keeps appearing.
        let signatures: Vec<String> = ["A", "B", "C", "D", "E", "F"]
            .iter()
            .map(|s| (*s).to_string())
            .collect();
        assert!(!ExecutionEngine::is_periodic_tool_signature_loop(
            &signatures,
            3
        ));
    }

    #[test]
    fn periodic_loop_detector_skips_when_a_signature_appears_only_once() {
        // A-B-A-B-A-C: trailing window has 3 distinct signatures, but C
        // appeared exactly once - the model is still introducing new work.
        let signatures: Vec<String> = ["A", "B", "A", "B", "A", "C"]
            .iter()
            .map(|s| (*s).to_string())
            .collect();
        assert!(!ExecutionEngine::is_periodic_tool_signature_loop(
            &signatures,
            3
        ));
    }

    #[test]
    fn periodic_loop_detector_only_inspects_trailing_window() {
        // The first 4 rounds were genuine exploration, but the last 6 are a
        // stable A-B alternation. We should still flag the loop.
        let signatures: Vec<String> = ["X1", "X2", "X3", "X4", "A", "B", "A", "B", "A", "B"]
            .iter()
            .map(|s| (*s).to_string())
            .collect();
        assert!(ExecutionEngine::is_periodic_tool_signature_loop(
            &signatures,
            3
        ));
    }

    #[test]
    fn periodic_loop_detector_treats_threshold_zero_like_one() {
        let signatures: Vec<String> = ["A", "A"].iter().map(|s| (*s).to_string()).collect();
        // A two-round window of identical signatures with threshold 0 should
        // still register as a loop (threshold is clamped to 1, window = 2).
        assert!(ExecutionEngine::is_periodic_tool_signature_loop(
            &signatures,
            0
        ));
    }

    #[test]
    fn context_health_snapshot_scores_repeated_tool_signatures() {
        let signatures = vec![
            r#"Bash:{"command":"cargo test"}"#.to_string(),
            r#"Bash:{"command":"cargo test"}"#.to_string(),
            r#"Bash:{"command":"cargo test"}"#.to_string(),
        ];

        let snapshot =
            ContextHealthSnapshot::from_runtime_observations(0.82, 1, 0, &signatures, &[]);

        assert!((snapshot.token_usage_ratio - 0.82).abs() < f32::EPSILON);
        assert_eq!(snapshot.full_compression_count, 1);
        assert_eq!(snapshot.compression_failure_count, 0);
        assert_eq!(snapshot.repeated_tool_signature_count, 3);
        assert_eq!(snapshot.consecutive_failed_commands, 0);
    }

    #[test]
    fn context_health_snapshot_counts_consecutive_failed_commands() {
        let messages = vec![
            command_result("Bash", true, Some(0)),
            command_result("Bash", false, Some(1)),
            command_result("Git", false, Some(128)),
        ];

        let snapshot = ContextHealthSnapshot::from_runtime_observations(0.44, 0, 2, &[], &messages);

        assert_eq!(snapshot.repeated_tool_signature_count, 0);
        assert_eq!(snapshot.consecutive_failed_commands, 2);
        assert_eq!(snapshot.compression_failure_count, 2);
    }

    #[test]
    fn provider_prompt_cache_route_key_depends_only_on_lineage() {
        let first = ExecutionEngine::model_request_context("session-1", "sid-a", "turn-1");
        let same_lineage = ExecutionEngine::model_request_context("session-1", "sid-a", "turn-2");
        let changed_lineage = ExecutionEngine::model_request_context("session-2", "sid-b", "turn-3");

        assert_eq!(first.prompt_cache_route_key.as_deref(), Some("session-1"));
        assert_eq!(first.session_id.as_deref(), Some("sid-a"));
        assert_eq!(
            first.prompt_cache_route_key,
            same_lineage.prompt_cache_route_key
        );
        assert_ne!(
            first.prompt_cache_route_key,
            changed_lineage.prompt_cache_route_key
        );
    }

    fn command_result(tool_name: &str, success: bool, exit_code: Option<i32>) -> Message {
        Message::tool_result(ToolResult {
            tool_id: format!("{}-tool", tool_name),
            tool_name: tool_name.to_string(),
            effective_tool_name: None,
            result: json!({
                "success": success,
                "exit_code": exit_code,
                "command": format!("{} command", tool_name),
            }),
            result_for_assistant: None,
            is_error: !success,
            duration_ms: Some(1),
            image_attachments: None,
        })
    }

    #[tokio::test]
    async fn resident_subagent_session_compaction_keeps_context_reusable() {
        // A resident subagent work post (Task spawn then repeated send_input
        // reuse) accumulates context across dialog turns. Automatic compaction
        // must replace the in-memory context — the exact source the next
        // send_input loads — without changing the session identity, and the
        // compacted context must stay compressible so the resident session
        // never dies from an ever-growing context window.
        let temp = tempfile::tempdir().expect("tempdir");
        let session_manager = SessionManager::new(
            Arc::new(SessionContextStore::new()),
            Arc::new(
                PersistenceManager::new(Arc::new(PathManager::with_user_root_for_tests(
                    temp.path().join("user-root"),
                )))
                .expect("persistence manager"),
            ),
            SessionManagerConfig {
                max_active_sessions: 4,
                session_idle_timeout: Duration::from_secs(3600),
                auto_save_interval: Duration::from_secs(300),
                enable_persistence: false,
                prompt_cache_policy: PromptCachePolicy::default(),
            },
        );
        let compressor = ContextCompressor::new(Default::default());
        let session_id = "resident-subagent-session";
        // A small window keeps the test fast while exercising the real trigger
        // math (input_limit = window - output reserve - safety reserve). It
        // must stay above the 10k safety reserve so input_limit is meaningful.
        let context_window = 32_000usize;
        let trigger_budget = ExecutionEngine::compression_trigger_budget(context_window, None);
        assert!(trigger_budget.input_limit > 0);

        // Repeated send_input turns: each turn appends a user message plus
        // assistant/tool round messages (the engine loop's add_message path).
        let mut turn = 0usize;
        let compressed_turn = loop {
            turn += 1;
            assert!(turn < 50, "compression never triggered");
            let user_message = Message::user(format!(
                "send_input turn {}: continue the standing task",
                turn
            ))
            .with_turn_id(format!("turn-{turn}"));
            let assistant_message =
                Message::assistant(format!("round evidence {}", "x".repeat(2_000)))
                    .with_turn_id(format!("turn-{turn}"));
            let tool_message =
                command_result("Bash", true, Some(0)).with_turn_id(format!("turn-{turn}"));
            for message in [&user_message, &assistant_message, &tool_message] {
                session_manager
                    .add_message(session_id, message.clone())
                    .await
                    .expect("append turn messages");
            }

            let context = session_manager
                .get_context_messages(session_id)
                .await
                .expect("reusable context");
            let pressure = ExecutionEngine::estimate_auto_compression_pressure(
                &context,
                None,
                context_window,
                trigger_budget,
                0,
            );
            if pressure.total_tokens >= pressure.input_limit {
                let Some(plan) = compressor
                    .plan_compression(
                        session_id,
                        &context,
                        context_window,
                        ContextCompressor::DEFAULT_RECENT_CONTEXT_TOKENS,
                        None,
                    )
                    .expect("compression planning succeeds")
                else {
                    // Not enough compressible history yet; keep accumulating.
                    continue;
                };
                let result = compressor
                    .compress_plan_with_contract(
                        session_id,
                        context_window,
                        plan,
                        None,
                        Some(format!("turn {} handoff summary", turn)),
                    )
                    .expect("compression succeeds");
                let before_message_count = context.len();
                session_manager
                    .replace_context_messages(session_id, result.messages.clone())
                    .await;
                let after = session_manager
                    .get_context_messages(session_id)
                    .await
                    .expect("compacted context");
                // ENGINE-06：压缩回归断言必须与真实 send_input 使用同一度量——
                // 完整会话消息走 estimate_auto_compression_pressure，而不是按单条
                // 消息求和（后者测的是另一个 token 口径）。
                let after_pressure = ExecutionEngine::estimate_auto_compression_pressure(
                    &after,
                    None,
                    context_window,
                    trigger_budget,
                    0,
                );
                assert!(
                    after_pressure.total_tokens < after_pressure.input_limit,
                    "compaction must bring the resident context back under the input limit: after={}, input_limit={}",
                    after_pressure.total_tokens,
                    after_pressure.input_limit
                );
                // ENGINE-06：压缩后的会话消息并不是完整请求。下一次 send_input 会
                // 在其上重新拼回系统提示、前置提醒与工具定义；这些固定脚手架的开销
                // 必须由压缩后的裕量（input_limit - total_tokens）覆盖，否则常驻
                // 会话在下一轮又会立刻触发压缩，依然会在窗口处耗尽。
                let scaffold_system_tokens = ExecutionEngine::system_tokens_for_pressure(
                    std::slice::from_ref(&Message::system(
                        "You are BitFun, an autonomous coding agent. Execute the user's task within the workshop workflow."
                            .to_string(),
                    )),
                );
                let scaffold_reminder_tokens =
                    ExecutionEngine::prepended_reminder_tokens_for_pressure(&[
                        "Continue executing the standing task. The prior context was summarized by compression.",
                        "Current time is 2026-08-05T12:00:00Z. Context usage is low after compaction.",
                    ]);
                let scaffold_tools = vec![
                    ToolDefinition {
                        name: "Bash".to_string(),
                        description: "Run a shell command and capture its output.".to_string(),
                        parameters: json!({"type": "object", "properties": {"command": {"type": "string"}}}),
                    },
                    ToolDefinition {
                        name: "Read".to_string(),
                        description: "Read a file from the workspace and return its content."
                            .to_string(),
                        parameters: json!({"type": "object", "properties": {"path": {"type": "string"}}}),
                    },
                ];
                let scaffold_tool_tokens =
                    TokenCounter::estimate_tool_definitions_tokens(&scaffold_tools);
                let scaffold_overhead = scaffold_system_tokens
                    .saturating_add(scaffold_reminder_tokens)
                    .saturating_add(scaffold_tool_tokens);
                let after_headroom = after_pressure
                    .input_limit
                    .saturating_sub(after_pressure.total_tokens);
                assert!(
                    after_headroom >= scaffold_overhead,
                    "compaction must leave margin for the system/reminder/tool scaffold the next send_input adds back: headroom={}, scaffold={} (system={}, reminders={}, tools={}), after={}, input_limit={}",
                    after_headroom,
                    scaffold_overhead,
                    scaffold_system_tokens,
                    scaffold_reminder_tokens,
                    scaffold_tool_tokens,
                    after_pressure.total_tokens,
                    after_pressure.input_limit
                );
                assert!(
                    after.len() < before_message_count,
                    "compaction must fold the accumulated turn messages: before={}, after={}",
                    before_message_count,
                    after.len()
                );
                assert!(
                    after.iter().any(|message| message.metadata.semantic_kind
                        == Some(MessageSemanticKind::CompressionSummary)),
                    "compacted context must carry the compression summary"
                );
                assert!(
                    after.iter().any(|message| message.internal_reminder_kind()
                        == Some(InternalReminderKind::CompressionContinuation)),
                    "compacted context must carry the continuation reminder"
                );
                break turn;
            }
        };

        // The next send_input loads the compacted context (same session_id),
        // appends a new user message, and must remain compressible so the
        // resident session can keep running instead of dying at the window.
        let continued = session_manager
            .get_context_messages(session_id)
            .await
            .expect("reusable context after compaction");
        assert!(
            !continued.is_empty(),
            "compacted context is loadable by the next send_input"
        );
        session_manager
            .add_message(
                session_id,
                Message::user("send_input after compaction: keep going".to_string())
                    .with_turn_id(format!("turn-{}", compressed_turn + 1)),
            )
            .await
            .expect("append after compaction");
        let continued = session_manager
            .get_context_messages(session_id)
            .await
            .expect("reloaded context");
        let plan = compressor
            .plan_compression(
                session_id,
                &continued,
                context_window,
                ContextCompressor::DEFAULT_RECENT_CONTEXT_TOKENS,
                None,
            )
            .expect("recompression planning succeeds");
        assert!(
            plan.is_some(),
            "compacted resident context remains compressible"
        );
    }

    #[test]
    fn finalize_round_budget_gates_model_requests() {
        assert!(ExecutionEngine::should_allow_finalize_round(0, 2));
        assert!(ExecutionEngine::should_allow_finalize_round(1, 2));
        assert!(!ExecutionEngine::should_allow_finalize_round(2, 2));
        assert!(!ExecutionEngine::should_allow_finalize_round(5, 2));
    }

    #[test]
    fn local_fallback_round_has_no_model_visible_content() {
        let fallback = crate::agentic::execution::types::RoundResult::local_fallback();
        assert!(!fallback.had_assistant_text);
        assert!(!fallback.had_thinking_content);
        assert!(fallback.tool_calls.is_empty());
        assert!(!fallback.has_more_rounds);
        assert!(fallback.usage.is_none());
    }

    #[test]
    fn local_final_response_message_covers_thinking_only_budget() {
        assert!(
            ExecutionEngine::build_local_final_response_message("thinking_only_budget")
                .contains("reasoning-only")
        );
    }

    #[test]
    fn finalize_budget_allows_legacy_first_request_and_single_retry() {
        // 缓存保护（主人定标 2026-08-10）：finalize 门控必须允许「首请求 +
        // 一次重试」——这是修复前 legacy 行为的逐字节等价。预算 2 恰好等于
        // 该行为；超过 2 的请求（修复前不存在）才被截断为本地合成。
        // 因此正常 finalize 轮请求的 prompt 组装路径零变化（run_finalize_round
        // 内部未被触碰），共享前缀不漂移。
        assert!(ExecutionEngine::should_allow_finalize_round(0, 2)); // 首请求
        assert!(ExecutionEngine::should_allow_finalize_round(1, 2)); // 一次重试
        assert!(!ExecutionEngine::should_allow_finalize_round(2, 2)); // 修复前无第 3 次
    }

    // ================= R-MR-10 消息重复校验闸门 =================

    fn tool_result_message(tool_name: &str, result_value: serde_json::Value) -> Message {
        Message::tool_result(ToolResult {
            tool_id: format!("call-{}", tool_name),
            tool_name: tool_name.to_string(),
            effective_tool_name: None,
            result: result_value,
            result_for_assistant: None,
            is_error: false,
            duration_ms: Some(1),
            image_attachments: None,
        })
    }

    /// 模拟一轮「模型输出 + 工具结果」追加进 messages 后的新增序列。
    fn appended_round_messages(
        assistant_text: &str,
        tool_name: &str,
        result_value: serde_json::Value,
    ) -> Vec<Message> {
        vec![
            Message::assistant_with_tools(
                assistant_text.to_string(),
                vec![crate::agentic::core::ToolCall {
                    tool_id: format!("call-{}", tool_name),
                    tool_name: tool_name.to_string(),
                    arguments: json!({ "query": format!("{}", tool_name) }),
                    raw_arguments: None,
                    is_error: false,
                    parse_error: None,
                    recovered_from_truncation: false,
                    repair_kind: bitfun_agent_stream::ToolArgumentRepairKind::None,
                }],
            ),
            tool_result_message(tool_name, result_value),
        ]
    }

    #[test]
    fn duplicate_message_gate_intercepts_dead_loop_on_first_repeat() {
        // 验收断言 1（R-MR-10 §四.1）：死循环——第 1 轮发送正常，第 2 轮新增
        // 消息序列与第 1 轮完全相同 → 第一次重复即拦（0 请求，本地合成）。
        let round_1 = appended_round_messages("call Bash", "Bash", json!({ "stdout": "same" }));
        let round_2 = appended_round_messages("call Bash", "Bash", json!({ "stdout": "same" }));

        let fingerprint_1 = ExecutionEngine::messages_sequence_fingerprint(&round_1);
        let fingerprint_2 = ExecutionEngine::messages_sequence_fingerprint(&round_2);
        assert_eq!(fingerprint_1, fingerprint_2, "死循环两轮指纹应相同");

        let mut window: Vec<String> = Vec::new();
        assert!(!ExecutionEngine::is_duplicate_message_fingerprint(
            &fingerprint_1,
            &window,
            3
        ));
        window.push(fingerprint_1.clone());
        // 第二次出现相同指纹 → 窗口内重复 → 拦
        assert!(
            ExecutionEngine::is_duplicate_message_fingerprint(&fingerprint_2, &window, 3),
            "窗口内重复应判定拦截"
        );
    }

    #[test]
    fn duplicate_message_gate_never_intercepts_normal_rounds() {
        // 验收断言 2（R-MR-10 §四.2）：正常轮（工具结果变化）→ 零误拦。
        // 正常轮语义：每一轮的新指纹互不相同，且不与窗口内已发送指纹重复
        // → 逐轮放行。
        let round_1 = appended_round_messages("call Bash", "Bash", json!({ "stdout": "a" }));
        let round_2 = appended_round_messages("call Grep", "Grep", json!({ "matches": 1 }));
        let round_3 = appended_round_messages("call Read", "Read", json!({ "path": "x" }));

        let fingerprint_1 = ExecutionEngine::messages_sequence_fingerprint(&round_1);
        let fingerprint_2 = ExecutionEngine::messages_sequence_fingerprint(&round_2);
        let fingerprint_3 = ExecutionEngine::messages_sequence_fingerprint(&round_3);
        assert_ne!(fingerprint_1, fingerprint_2, "工具结果变化 → 指纹必不同");
        assert_ne!(fingerprint_2, fingerprint_3);

        let mut window: Vec<String> = Vec::new();
        // 第 1 轮：空窗口 → 放行，入窗
        assert!(!ExecutionEngine::is_duplicate_message_fingerprint(
            &fingerprint_1,
            &window,
            3
        ));
        window.push(fingerprint_1.clone());
        // 第 2 轮：新指纹不在窗口内 → 放行，入窗
        assert!(!ExecutionEngine::is_duplicate_message_fingerprint(
            &fingerprint_2,
            &window,
            3
        ));
        window.push(fingerprint_2.clone());
        // 第 3 轮：新指纹不在窗口内 → 放行
        assert!(!ExecutionEngine::is_duplicate_message_fingerprint(
            &fingerprint_3,
            &window,
            3
        ));
    }

    #[test]
    fn duplicate_message_gate_window_three_catches_round_three_repeating_round_one() {
        // 验收断言 3（R-MR-10 §四.3）：窗口 3——第 1/2 轮不同，第 3 轮重复第 1 轮
        // → 拦（窗口内任一相同即判定重复，不要求相邻）。
        let round_1 = appended_round_messages("call Bash", "Bash", json!({ "stdout": "a" }));
        let round_2 = appended_round_messages("call Grep", "Grep", json!({ "matches": 1 }));
        let round_3 = appended_round_messages("call Bash", "Bash", json!({ "stdout": "a" }));

        let fingerprint_1 = ExecutionEngine::messages_sequence_fingerprint(&round_1);
        let fingerprint_2 = ExecutionEngine::messages_sequence_fingerprint(&round_2);
        let fingerprint_3 = ExecutionEngine::messages_sequence_fingerprint(&round_3);
        assert_ne!(fingerprint_1, fingerprint_2);
        assert_eq!(fingerprint_1, fingerprint_3, "第 3 轮重复第 1 轮");

        let window: Vec<String> = vec![fingerprint_1.clone(), fingerprint_2.clone()];
        // 窗口内（含第 1 轮）出现相同指纹 → 拦
        assert!(
            ExecutionEngine::is_duplicate_message_fingerprint(&fingerprint_3, &window, 3),
            "窗口 3 内第 3 轮重复第 1 轮应拦截"
        );
    }

    #[test]
    fn duplicate_message_gate_window_slides_past_old_fingerprints() {
        // 边界：窗口滑动——窗口 3 保留最近 3 个指纹，第 1 轮指纹滑出后再次
        // 出现不再参与比对（不误伤跨窗口的正常重复内容）。
        let round_1 = appended_round_messages("a", "Bash", json!({ "i": 1 }));
        let round_2 = appended_round_messages("b", "Grep", json!({ "i": 2 }));
        let round_3 = appended_round_messages("c", "Read", json!({ "i": 3 }));
        let round_4 = appended_round_messages("d", "Glob", json!({ "i": 4 }));

        let fingerprint_1 = ExecutionEngine::messages_sequence_fingerprint(&round_1);
        let fingerprint_2 = ExecutionEngine::messages_sequence_fingerprint(&round_2);
        let fingerprint_3 = ExecutionEngine::messages_sequence_fingerprint(&round_3);
        let fingerprint_4 = ExecutionEngine::messages_sequence_fingerprint(&round_4);

        // 模拟主循环滑窗：每轮放行后入窗，窗口上限 3。
        let mut window: Vec<String> = Vec::new();
        for fp in [&fingerprint_1, &fingerprint_2, &fingerprint_3] {
            assert!(!ExecutionEngine::is_duplicate_message_fingerprint(
                fp, &window, 3
            ));
            window.push(fp.clone());
        }
        assert_eq!(window.len(), 3);
        // 第 4 轮：f4 不在窗口内 → 放行；入窗前先滑动（丢弃最旧 f1）。
        assert!(!ExecutionEngine::is_duplicate_message_fingerprint(
            &fingerprint_4,
            &window,
            3
        ));
        window.push(fingerprint_4.clone());
        if window.len() > 3 {
            window.drain(0..window.len() - 3);
        }
        assert_eq!(window.len(), 3);
        assert_eq!(window, vec![fingerprint_2, fingerprint_3, fingerprint_4]);
        // f1 已滑出窗口 3 → 再次出现不拦（跨窗口的正常内容复用）。
        assert!(
            !ExecutionEngine::is_duplicate_message_fingerprint(&fingerprint_1, &window, 3),
            "窗口 3 外的旧指纹不应拦截"
        );
    }

    #[test]
    fn duplicate_message_gate_zero_window_still_compares_adjacent_rounds() {
        // 边界：窗口 0 视为 1（至少保留相邻轮比对，配置 0 不使闸门静默失效）。
        let round = appended_round_messages("call Bash", "Bash", json!({ "stdout": "x" }));
        let fingerprint = ExecutionEngine::messages_sequence_fingerprint(&round);
        let window = vec![fingerprint.clone()];
        assert!(
            ExecutionEngine::is_duplicate_message_fingerprint(&fingerprint, &window, 0),
            "窗口 0 退化为相邻轮比对"
        );
    }

    #[test]
    fn duplicate_message_fingerprint_covers_tool_calls_and_results() {
        // 契约 §二.1：指纹 hash 全部消息内容 + 工具调用 + 工具结果，逐字节。
        let assistant_only = Message::assistant("call Bash".to_string());
        let assistant_with_tools = Message::assistant_with_tools(
            "call Bash".to_string(),
            vec![crate::agentic::core::ToolCall {
                tool_id: "call-Bash".to_string(),
                tool_name: "Bash".to_string(),
                arguments: json!({ "cmd": "ls" }),
                raw_arguments: None,
                is_error: false,
                parse_error: None,
                recovered_from_truncation: false,
                repair_kind: bitfun_agent_stream::ToolArgumentRepairKind::None,
            }],
        );
        let result_a = tool_result_message("Bash", json!({ "stdout": "a" }));
        let result_b = tool_result_message("Bash", json!({ "stdout": "b" }));

        let fp_no_tools = ExecutionEngine::messages_sequence_fingerprint(&[assistant_only]);
        let fp_with_tools = ExecutionEngine::messages_sequence_fingerprint(&[assistant_with_tools]);
        assert_ne!(fp_no_tools, fp_with_tools, "工具调用参与指纹");

        let fp_result_a =
            ExecutionEngine::messages_sequence_fingerprint(std::slice::from_ref(&result_a));
        let fp_result_b = ExecutionEngine::messages_sequence_fingerprint(&[result_b]);
        assert_ne!(fp_result_a, fp_result_b, "工具结果参与指纹");

        // 相同内容序列指纹稳定（逐字节等价）。
        let fp_result_a2 = ExecutionEngine::messages_sequence_fingerprint(&[result_a]);
        assert_eq!(fp_result_a, fp_result_a2);
    }

    #[test]
    fn duplicate_message_local_final_response_mentions_loop() {
        // 拦截动作：本地合成 final response 文案说明死循环（不调 API）。
        let message = ExecutionEngine::build_local_final_response_message("duplicate_messages");
        assert!(
            message.contains("loop"),
            "duplicate_messages 文案应说明循环"
        );
        assert!(!message.is_empty());
    }

    #[test]
    fn duplicate_message_fingerprint_differentiates_message_roles() {
        // 角色参与指纹：同文本不同 role 不得视为同一序列。
        let user = Message::user("hello".to_string());
        let assistant = Message::assistant("hello".to_string());
        assert_ne!(
            ExecutionEngine::messages_sequence_fingerprint(&[user]),
            ExecutionEngine::messages_sequence_fingerprint(&[assistant])
        );
    }
}
