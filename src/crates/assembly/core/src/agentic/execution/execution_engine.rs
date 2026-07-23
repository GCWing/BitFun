//! Execution Engine
//!
//! Executes complete dialog turns, managing loops of multiple model rounds

use super::model_exchange_trace::{
    prepare_model_exchange_trace_for_workspace, ModelExchangeTraceOperation,
};
use super::round_executor::RoundExecutor;
use super::types::{ExecutionContext, ExecutionResult, RoundContext, RoundResult};
use crate::agentic::agents::{
    build_prompt_context_for_workspace, get_agent_registry, PrependedPromptReminders,
    PromptBuilder, PromptBuilderContext, RuntimeContextNeeds, ToolListingSections,
};
use crate::agentic::context_profile::{ContextProfilePolicy, ModelCapabilityProfile};
use crate::agentic::core::{
    render_system_reminder, InternalReminderKind, Message, MessageContent, MessageHelper,
    MessageRole, MessageSemanticKind, RequestReasoningTokenPolicy, Session,
};
use crate::agentic::events::{AgenticEvent, EventPriority, EventQueue};
use crate::agentic::execution::types::FinishReason;
use crate::agentic::image_analysis::{
    build_multimodal_message_with_images, process_image_contexts_for_provider, ImageContextData,
    ImageLimits,
};
use crate::agentic::round_preempt::RoundInjectionKind;
use crate::agentic::session::{
    CompressionMode, ContextCompressor, SessionManager, TokenAnchor, TokenAnchorInput,
    UserContextCacheIdentity,
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
use crate::service::config::get_global_config_service;
use crate::service::config::types::{
    automatic_max_output_tokens, model_runtime_binding_fingerprint, ModelCapability, ModelCategory,
};
use crate::util::errors::{BitFunError, BitFunResult};
use crate::util::token_counter::TokenCounter;
use crate::util::types::Message as AIMessage;
use crate::util::types::ToolDefinition;
use crate::util::{elapsed_ms_u64, truncate_at_char_boundary};
use bitfun_agent_runtime::output_surface::TOOL_CONTEXT_INLINE_MARKDOWN_IMAGE_DISPLAY_KEY;
use bitfun_agent_runtime::remote_file_delivery::TOOL_CONTEXT_REMOTE_FILE_DELIVERY_KEY;
use bitfun_ai_adapters::ModelExchangeTraceConfig;
use bitfun_core_types::SessionModelBindingPolicy;
use log::{debug, error, info, trace, warn};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;
use tool_runtime::context::PrimaryModelFacts;

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

struct CompressionRuntimeScaffold {
    ai_client: Arc<crate::infrastructure::ai::AIClient>,
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
    context: &'a ExecutionContext,
}

struct FinalizeRoundInput<'a> {
    context_window: usize,
    tool_definitions: Option<Vec<ToolDefinition>>,
    reminder_text: &'a str,
    messages: &'a [Message],
    prepended_reminders: &'a [&'a str],
    primary_model_facts: &'a PrimaryModelFacts,
    execution_context_vars: &'a HashMap<String, String>,
    round_group_id: Option<String>,
    round_number: usize,
    agent_type: String,
    context: &'a ExecutionContext,
    ai_client: Arc<crate::infrastructure::ai::AIClient>,
}

struct CompressionModelSummaryInput<'a> {
    trace_config: Option<ModelExchangeTraceConfig>,
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
}

impl ExecutionEngine {
    const AUTO_COMPRESSION_SAFETY_RESERVE_TOKENS: usize = 10_000;
    const FINALIZE_AFTER_REPEATED_TOOL_FAILURES_REMINDER: &'static str = "This turn must end now because repeated tool failures have prevented further progress. Ignore any unfinished work. Your task now is to give the user a final answer. Do not call any more tools; any tool call will fail. Respond in plain text only. Summarize what was completed, what failed, the evidence available from the tool results, and the single best next step for the user.";
    const FINALIZE_AFTER_MAX_ROUNDS_REMINDER: &'static str = "This turn must end now because it has reached the round limit. Ignore any unfinished work. Your task now is to give the user a final answer. Do not call any more tools; any tool call will fail. Respond in plain text only. Summarize the most useful completed work and evidence collected so far, and clearly distinguish resolved items from anything still unresolved.";
    const FINALIZE_TOOL_DENIED_MESSAGE: &'static str =
        "Tool use is disabled for finalize. Respond with plain text only.";
    const FINALIZE_USER_FOLLOWUP: &'static str =
        "Provide a final answer. You MUST not call any tools.";

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
        }
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

    fn compression_trigger_budget(
        context_window: usize,
        configured_max_tokens: Option<u32>,
    ) -> CompressionTriggerBudget {
        let output_reserve_tokens = configured_max_tokens
            .map(|value| value as usize)
            .unwrap_or_else(|| automatic_max_output_tokens(context_window as u32) as usize);
        let safety_reserve_tokens = Self::AUTO_COMPRESSION_SAFETY_RESERVE_TOKENS;
        let input_limit =
            context_window.saturating_sub(output_reserve_tokens + safety_reserve_tokens);

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

    fn build_local_final_response_message(reason: &str) -> String {
        match reason {
            "repeated_tool_failures" => {
                "I'm stopping here because repeated tool failures prevented further progress in this turn.".to_string()
            }
            "max_rounds" => {
                "I'm stopping here because this turn reached its round limit before I could complete a final response.".to_string()
            }
            _ => "I'm stopping here because this turn could not be completed successfully.".to_string(),
        }
    }

    fn should_mark_has_final_response(
        has_assistant_message: bool,
        used_local_final_response_synthesis: bool,
    ) -> bool {
        has_assistant_message && !used_local_final_response_synthesis
    }

    fn build_finalize_cache_anchor_messages(turn_id: &str, reminder_text: &str) -> Vec<Message> {
        vec![
            Message::internal_reminder(
                InternalReminderKind::FinalizeCacheAnchor,
                reminder_text.to_string(),
            )
            .with_turn_id(turn_id.to_string()),
            Message::user(Self::FINALIZE_USER_FOLLOWUP.to_string())
                .with_semantic_kind(MessageSemanticKind::InternalReminder)
                .with_internal_reminder_kind(InternalReminderKind::FinalizeCacheAnchor)
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

    async fn build_cached_prepended_prompt_reminders(
        &self,
        session_id: &str,
        current_agent: &dyn crate::agentic::agents::Agent,
        prompt_context: Option<&PromptBuilderContext>,
        _context_vars: &HashMap<String, String>,
    ) -> PrependedPromptReminders {
        let Some(prompt_context) = prompt_context.cloned() else {
            return PrependedPromptReminders::default();
        };

        // Extract remote execution info before prompt_context is moved into PromptBuilder.
        let remote_connection_for_cache = prompt_context
            .remote_execution
            .as_ref()
            .map(|remote| remote.connection_display_name.replace('|', "/"));

        let prompt_builder = PromptBuilder::new(prompt_context);
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
        let user_context_identity = {
            let base_identity = current_agent.user_context_cache_identity();
            // Append the remote connection to the cache scope so a failed overlay
            // (cached without remote hints) does not persist across reconnects.
            if let Some(connection) = &remote_connection_for_cache {
                UserContextCacheIdentity::new(format!(
                    "{}|remote:{}",
                    base_identity.scope_key, connection
                ))
            } else {
                base_identity
            }
        };
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
            let built_user_context = prompt_builder
                .build_user_context_reminder(&current_agent.user_context_policy())
                .await;
            if let Some(ref user_context) = built_user_context {
                self.session_manager
                    .remember_user_context(
                        session_id,
                        user_context_identity.clone(),
                        user_context.clone(),
                    )
                    .await;
            }
            built_user_context
        };
        let runtime_context = prompt_builder.build_runtime_context_reminder().await;

        PrependedPromptReminders {
            deferred_tool_listing: prompt_builder.build_deferred_tool_listing_reminder(),
            skill_listing: baseline_tool_sections
                .as_ref()
                .and_then(|sections| sections.render_skill_listing_reminder()),
            agent_listing: baseline_tool_sections
                .as_ref()
                .and_then(|sections| sections.render_agent_listing_reminder()),
            runtime_context,
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
                &input.context.session_id,
                input.current_agent,
                prompt_context.as_ref(),
                &input.context.context,
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
            "Turn prompt scaffold resolved: session_id={}, turn_id={}, stage={}, system_prompt_len={} bytes, skill_listing_len={}, agent_listing_len={}, deferred_tool_listing_len={}, user_context_len={}, runtime_context_len={}",
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

    pub(crate) async fn resolve_model_id_for_turn(
        &self,
        session: &Session,
        agent_type: &str,
        workspace: Option<&WorkspaceBinding>,
        original_user_input: &str,
        turn_index: usize,
    ) -> BitFunResult<String> {
        let config_service = get_global_config_service().await.map_err(|e| {
            BitFunError::AIClient(format!(
                "Failed to get config service for model resolution: {}",
                e
            ))
        })?;
        let ai_config: crate::service::config::types::AIConfig = config_service
            .get_config(Some("ai"))
            .await
            .unwrap_or_default();
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
            return Ok(model_id.to_string());
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
        let resolved_configured_model_id =
            Self::resolve_configured_model_id(&ai_config, &configured_model_id);

        let model_id = if configured_model_id == "auto"
            || configured_model_id == "default"
            || resolved_configured_model_id == "auto"
        {
            let fallback_model = "primary";
            let resolved_model_id = ai_config.resolve_model_selection(fallback_model);

            if let Some(resolved_model_id) = resolved_model_id {
                info!(
                    "Auto model resolved without locking session: session_id={}, turn_index={}, user_input_chars={}, strategy={}, resolved_model_id={}",
                    session.session_id,
                    turn_index,
                    original_user_input.chars().count(),
                    fallback_model,
                    resolved_model_id
                );

                resolved_model_id
            } else {
                warn!(
                    "Auto model strategy unresolved, keeping symbolic selector: session_id={}, strategy={}",
                    session.session_id, fallback_model
                );
                fallback_model.to_string()
            }
        } else {
            resolved_configured_model_id
        };

        Ok(model_id)
    }

    /// Omit from model request: UI-only verification frames and legacy auto desktop snapshots.
    fn skip_message_for_model_send(msg: &Message) -> bool {
        matches!(
            msg.metadata.semantic_kind.as_ref(),
            Some(MessageSemanticKind::ComputerUseVerificationScreenshot)
                | Some(MessageSemanticKind::ComputerUsePostActionSnapshot)
        )
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
            input.prepended_reminders,
        )
        .await?;
        final_ai_messages.push(AIMessage::user(render_system_reminder(input.reminder_text)));
        final_ai_messages.push(AIMessage::user(Self::FINALIZE_USER_FOLLOWUP.to_string()));

        let model_exchange_trace_dir = self
            .session_manager
            .persistent_model_exchange_trace_dir(&input.context.session_id)
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
            deferred_tools: Vec::new(),
            loaded_deferred_tool_specs: Vec::new(),
            model_config_id: input.primary_model_facts.model_id.clone(),
            effective_model_name: input.ai_client.config.model.clone(),
            primary_model_facts: input.primary_model_facts.clone(),
            agent_type: input.agent_type,
            context_vars: input.execution_context_vars.clone(),
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

    async fn build_ai_messages_for_send(
        messages: &[Message],
        provider: &str,
        workspace_path: Option<&Path>,
        current_turn_id: &str,
        attach_images: bool,
        prepended_reminders: &[&str],
    ) -> BitFunResult<Vec<AIMessage>> {
        /// Only the last this many **messages** that contain images keep their images for the API.
        const MAX_IMAGE_BEARING_MESSAGE_ROUNDS: usize = 2;

        let limits = ImageLimits::for_provider(provider);

        let trimmed_reminders = prepended_reminders
            .iter()
            .map(|text| text.trim())
            .filter(|text| !text.is_empty())
            .collect::<Vec<_>>();
        let mut result = Vec::with_capacity(messages.len() + trimmed_reminders.len());
        let mut attached_image_count = 0usize;
        let first_non_system_index = messages
            .iter()
            .position(|msg| msg.role != crate::agentic::core::MessageRole::System)
            .unwrap_or(messages.len());
        let mut prepended_reminders_injected = false;

        let keep_image_messages = if attach_images {
            Self::image_bearing_indices_to_keep(messages, MAX_IMAGE_BEARING_MESSAGE_ROUNDS)
        } else {
            HashSet::new()
        };

        for (msg_idx, msg) in messages.iter().enumerate() {
            if !prepended_reminders_injected && msg_idx == first_non_system_index {
                for reminder in &trimmed_reminders {
                    result.push(AIMessage::user(render_system_reminder(reminder)));
                }
                prepended_reminders_injected = true;
            }

            if Self::skip_message_for_model_send(msg) {
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
                            MAX_IMAGE_BEARING_MESSAGE_ROUNDS
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
                                    MAX_IMAGE_BEARING_MESSAGE_ROUNDS
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
            for reminder in trimmed_reminders {
                result.push(AIMessage::user(render_system_reminder(reminder)));
            }
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
        let prepended_reminders = prepended_prompt_reminders.ordered_reminders();
        let mut compression_messages = Self::build_ai_messages_for_send(
            runtime_messages,
            provider,
            workspace.map(|workspace| workspace.root_path()),
            dialog_turn_id,
            attach_images,
            &prepended_reminders,
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
        trace_config: Option<ModelExchangeTraceConfig>,
        max_tries: usize,
    ) -> BitFunResult<String> {
        let mut last_error = None;
        let base_wait_time_ms = 500;

        for attempt in 0..max_tries {
            let result = ai_client
                .send_message_with_trace(
                    request_messages.clone(),
                    tool_definitions.clone(),
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
                    warn!(
                        "Compression summary generation failed (attempt {}/{}): {}",
                        attempt + 1,
                        max_tries,
                        err
                    );
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
                input.trace_config,
                2,
            )
            .await?;
        let summary =
            ContextCompressor::normalize_model_summary_output(&raw_summary).ok_or_else(|| {
                BitFunError::AIClient(
                    "Model-based compression returned <analysis> without a usable <summary>"
                        .to_string(),
                )
            })?;
        Ok(Some(summary))
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
        let model_id = self
            .resolve_model_id_for_turn(
                session,
                &context.agent_type,
                context.workspace.as_ref(),
                &original_user_input,
                context.turn_index,
            )
            .await?;

        let ai_client_factory = get_global_ai_client_factory().await.map_err(|e| {
            BitFunError::AIClient(format!("Failed to get AI client factory: {}", e))
        })?;
        let ai_client_result = if matches!(
            session.config.model_binding_policy,
            SessionModelBindingPolicy::ApprovedImmutable
        ) {
            ai_client_factory
                .get_client_by_approved_binding(
                    &model_id,
                    session
                        .config
                        .model_binding_fingerprint
                        .as_deref()
                        .unwrap_or_default(),
                )
                .await
        } else {
            ai_client_factory.get_client_resolved(&model_id).await
        };
        let ai_client = ai_client_result.map_err(|e| {
            BitFunError::AIClient(format!(
                "Failed to get AI client (model_id={}): {}",
                model_id, e
            ))
        })?;

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
        let allowed_tools = tool_policy.allowed_tools.clone();
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
                stage: "compression_scaffold",
            })
            .await?;

        Ok(CompressionRuntimeScaffold {
            ai_client,
            tool_definitions,
            system_prompt_message: turn_prompt_scaffold.system_prompt_message,
            prepended_prompt_reminders: turn_prompt_scaffold.prepended_prompt_reminders,
            primary_supports_image_understanding,
            compression_contract_limit: context_profile_policy.compression_contract_limit,
        })
    }

    /// Compress context, will emit compression events (Started, Completed, and Failed)
    #[allow(clippy::too_many_arguments)]
    async fn compress_messages(
        &self,
        session_id: &str,
        dialog_turn_id: &str,
        runtime_messages: Vec<Message>,
        before_pressure: TokenPressureSnapshot,
        context_window: usize,
        ai_client: Arc<crate::infrastructure::ai::AIClient>,
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
        let turns = self
            .context_compressor
            .collect_turns_for_auto_compression(session_id, runtime_messages.clone())?;
        if turns.is_empty() {
            return Ok(None);
        }

        // Generate compression ID
        let compression_id = format!("compression_{}", uuid::Uuid::new_v4());

        // Emit compression started event
        self.emit_event(
            AgenticEvent::ContextCompressionStarted {
                session_id: session_id.to_string(),
                turn_id: dialog_turn_id.to_string(),
                compression_id: compression_id.clone(),
                trigger: "auto".to_string(),
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
                trigger: Some("auto"),
            },
            ai_client.as_ref(),
        )
        .await;
        let model_summary = match self
            .generate_compression_model_summary(CompressionModelSummaryInput {
                ai_client,
                runtime_messages: &runtime_messages,
                dialog_turn_id,
                workspace,
                tool_definitions,
                prepended_prompt_reminders,
                primary_supports_image_understanding,
                trace_config,
            })
            .await
        {
            Ok(summary) => summary,
            Err(err) => {
                warn!(
                    "Model-based compression failed, falling back to structured local compression: {}",
                    err
                );
                None
            }
        };
        match self.context_compressor.compress_turns_with_contract(
            session_id,
            context_window,
            turns,
            CompressionMode::Auto,
            compression_contract,
            model_summary,
        ) {
            Ok(mut compression_result) => {
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
                        "auto",
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
                    },
                    EventPriority::Normal,
                )
                .await;

                Ok(Some((compressed_tokens, new_messages)))
            }
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
    pub async fn compact_session_context(
        &self,
        session_id: String,
        dialog_turn_id: String,
        context: ExecutionContext,
        messages: Vec<Message>,
        trigger: &str,
    ) -> BitFunResult<ContextCompactionOutcome> {
        let mut session = self
            .session_manager
            .get_session(&session_id)
            .ok_or_else(|| BitFunError::NotFound(format!("Session not found: {}", session_id)))?;
        let start_time = std::time::Instant::now();
        let compression_id = format!("compression_{}", uuid::Uuid::new_v4());
        let scaffold = self
            .resolve_compression_runtime_scaffold(&session, &context)
            .await?;
        let context_window = (scaffold.ai_client.config.context_window as usize)
            .min(session.config.max_context_tokens);
        let prepended_reminders = scaffold.prepended_prompt_reminders.ordered_reminders();
        let prepended_reminder_tokens =
            Self::prepended_reminder_tokens_for_pressure(&prepended_reminders);
        let compression_trigger_budget =
            Self::compression_trigger_budget(context_window, scaffold.ai_client.config.max_tokens);
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

        let turns = self
            .context_compressor
            .collect_all_turns_for_manual_compaction(&session_id, messages.clone())?;

        if turns.is_empty() {
            let duration_ms = elapsed_ms_u64(start_time);
            let tokens_after = before_pressure.total_tokens;
            let compression_ratio = if before_pressure.total_tokens == 0 {
                1.0
            } else {
                (tokens_after as f64) / (before_pressure.total_tokens as f64)
            };
            info!(
                "Manual compression skipped: session_id={}, turn_id={}, reason=no_eligible_turns, total_tokens={}, system_tokens={}, tool_tokens={}, prepended_reminder_tokens={}, conversation_tokens={}, context_window={}, input_limit={}, output_reserve={}, safety_reserve={}, usage={:.3}, duration_ms={}",
                session_id,
                dialog_turn_id,
                before_pressure.total_tokens,
                before_pressure.system_tokens,
                before_pressure.tool_tokens,
                before_pressure.prepended_reminder_tokens,
                before_pressure.conversation_tokens,
                before_pressure.context_window,
                before_pressure.input_limit,
                before_pressure.output_reserve_tokens,
                before_pressure.safety_reserve_tokens,
                before_pressure.usage_ratio,
                duration_ms
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
                },
                EventPriority::Normal,
            )
            .await;

            return Ok(ContextCompactionOutcome {
                compression_id,
                compression_count: session.compression_state.compression_count,
                tokens_before: before_pressure.total_tokens,
                tokens_after,
                compression_ratio,
                duration_ms,
                has_summary: false,
                summary_source: "none".to_string(),
                applied: false,
            });
        }

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
        let model_summary = match self
            .generate_compression_model_summary(CompressionModelSummaryInput {
                ai_client: scaffold.ai_client.clone(),
                runtime_messages: &runtime_messages,
                dialog_turn_id: &dialog_turn_id,
                workspace: context.workspace.as_ref(),
                tool_definitions: &scaffold.tool_definitions,
                prepended_prompt_reminders: &scaffold.prepended_prompt_reminders,
                primary_supports_image_understanding: scaffold.primary_supports_image_understanding,
                trace_config,
            })
            .await
        {
            Ok(summary) => summary,
            Err(err) => {
                warn!(
                    "Model-based manual compaction failed, falling back to structured local compression: {}",
                    err
                );
                None
            }
        };
        match self.context_compressor.compress_turns_with_contract(
            &session_id,
            context_window,
            turns,
            CompressionMode::Manual,
            compression_contract,
            model_summary,
        ) {
            Ok(mut compression_result) => {
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
                    },
                    EventPriority::Normal,
                )
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

                Err(BitFunError::Session(err.to_string()))
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
        let initial_count = initial_messages.len();

        let dialog_turn_id = context.dialog_turn_id.clone();

        info!("Starting dialog turn: dialog_turn_id={}", dialog_turn_id);

        // Execute actual logic
        let result = self
            .execute_dialog_turn_impl(
                agent_type,
                initial_messages,
                context,
                start_time,
                initial_count,
            )
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
        initial_count: usize,
    ) -> BitFunResult<ExecutionResult> {
        let dialog_turn_id = context.dialog_turn_id.clone();

        debug!(
            "Executing dialog turn implementation: dialog_turn_id={}",
            dialog_turn_id
        );

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

        let model_id = self
            .resolve_model_id_for_turn(
                &session,
                &agent_type,
                context.workspace.as_ref(),
                &original_user_input,
                context.turn_index,
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
        let ai_client_result = if matches!(
            session.config.model_binding_policy,
            SessionModelBindingPolicy::ApprovedImmutable
        ) {
            ai_client_factory
                .get_client_by_approved_binding(
                    &model_id,
                    session
                        .config
                        .model_binding_fingerprint
                        .as_deref()
                        .unwrap_or_default(),
                )
                .await
        } else {
            ai_client_factory.get_client_resolved(&model_id).await
        };
        let ai_client = ai_client_result.map_err(|e| {
            BitFunError::AIClient(format!(
                "Failed to get AI client (model_id={}): {}",
                model_id, e
            ))
        })?;

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
        let allowed_tools = tool_policy.allowed_tools.clone();
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
        let mut turn_prompt_scaffold = self
            .resolve_turn_prompt_scaffold(TurnPromptScaffoldInput {
                context: &context,
                current_agent: current_agent.as_ref(),
                model_name: &ai_client.config.model,
                supports_image_understanding: primary_supports_image_understanding,
                tool_listing_sections: tool_listing_sections.clone(),
                runtime_context_needs,
                stage: "turn_start",
            })
            .await?;

        // Add System Prompt to the beginning of message list (only for this execution, not persisted)
        let mut messages = vec![turn_prompt_scaffold.system_prompt_message.clone()];
        messages.extend(initial_messages);

        let mut round_index = 0;
        let mut completed_rounds = 0usize;
        let mut total_tools = 0;
        let mut last_partial_recovery_reason: Option<String> = None;
        let mut finalization_reason: Option<&'static str> = None;
        let mut consecutive_compression_failures: u32 = 0;
        const MAX_CONSECUTIVE_COMPRESSION_FAILURES: u32 = 3;

        // Track tool-call patterns for context health, but only use rounds with
        // actual failed tool results for no-progress recovery decisions.
        let mut recent_tool_signatures: Vec<String> = Vec::new();
        let mut recent_failed_tool_signatures: Vec<String> = Vec::new();
        let mut failed_tool_recovery_attempts: usize = 0;
        const MAX_FAILED_TOOL_RECOVERY_ATTEMPTS: usize = 3;
        const MAX_PARTIAL_CONTINUATION_ATTEMPTS: usize = 3;
        let mut full_compression_count = 0usize;
        let mut compression_failure_count = 0u32;

        // Save the last token usage statistics
        let mut last_usage: Option<crate::util::types::ai::GeminiUsage> = None;

        // Track thinking-only rescue reminders for observability. This counter
        // is not a stop condition.
        let mut thinking_only_rescue_attempts: usize = 0;
        let mut partial_continuation_attempts: usize = 0;

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
        let compression_trigger_budget =
            Self::compression_trigger_budget(context_window, ai_client.config.max_tokens);

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
            let (token_pressure, anchor_details) =
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

            let should_compress = enable_context_compression
                && token_pressure.total_tokens >= token_pressure.input_limit;
            let mut send_pressure_reusable = true;

            // Circuit breaker: skip full compression if it has failed too many
            // consecutive times.  Microcompact and emergency truncation still run.
            let circuit_breaker_open =
                consecutive_compression_failures >= MAX_CONSECUTIVE_COMPRESSION_FAILURES;

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

                match self
                    .compress_messages(
                        &context.session_id,
                        &context.dialog_turn_id,
                        messages.clone(),
                        token_pressure,
                        context_window,
                        ai_client.clone(),
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
                            "Round {} compression completed: messages {} -> {}, tokens {} -> {}",
                            round_index,
                            messages.len(),
                            compressed_messages.len(),
                            token_pressure.total_tokens,
                            compressed_tokens,
                        );

                        messages = compressed_messages;
                        turn_prompt_scaffold = self
                            .resolve_turn_prompt_scaffold(TurnPromptScaffoldInput {
                                context: &context,
                                current_agent: current_agent.as_ref(),
                                model_name: &ai_client.config.model,
                                supports_image_understanding: primary_supports_image_understanding,
                                tool_listing_sections: tool_listing_sections.clone(),
                                runtime_context_needs,
                                stage: "after_context_compression",
                            })
                            .await?;
                        Self::apply_turn_prompt_scaffold_to_messages(
                            &mut messages,
                            &turn_prompt_scaffold,
                        );
                        full_compression_count += 1;
                        consecutive_compression_failures = 0;
                        send_pressure_reusable = false;
                    }
                    Ok(None) => {
                        debug!("No eligible multi-turn context available for compression");
                        consecutive_compression_failures = 0;
                    }
                    Err(e) => {
                        consecutive_compression_failures += 1;
                        compression_failure_count += 1;
                        error!(
                            "Round {} compression failed ({}/{}): {}, continuing with uncompressed context",
                            round_index,
                            consecutive_compression_failures,
                            MAX_CONSECUTIVE_COMPRESSION_FAILURES,
                            e
                        );
                    }
                }
            }

            // L2: Emergency truncation — if tokens still exceed context_window
            // after all compression layers, drop oldest API rounds until we fit.
            let send_prepended_reminders = turn_prompt_scaffold
                .prepended_prompt_reminders
                .ordered_reminders();
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
            let round_context_vars = execution_context_vars.clone();
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
                deferred_tools: deferred_tools.clone(),
                loaded_deferred_tool_specs,
                model_config_id: model_id.clone(),
                effective_model_name: ai_client.config.model.clone(),
                primary_model_facts: primary_model_facts.clone(),
                agent_type: agent_type.clone(),
                context_vars: round_context_vars,
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

            let ai_messages = Self::build_ai_messages_for_send(
                &messages,
                &ai_client.config.format,
                context
                    .workspace
                    .as_ref()
                    .map(|workspace| workspace.root_path()),
                &context.dialog_turn_id,
                primary_supports_image_understanding,
                &send_prepended_reminders,
            )
            .await?;

            let round_result = self
                .round_executor
                .execute_round(
                    ai_client.clone(),
                    round_context,
                    ai_messages,
                    tool_definitions.clone(),
                    Some(context_window),
                )
                .await?;

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

                // Update the in-memory message caches immediately so subsequent rounds see it.
                if let Err(e) = self
                    .session_manager
                    .add_message(&context.session_id, tool_result_msg.clone())
                    .await
                {
                    warn!("Failed to update tool result message in memory: {}", e);
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
                    if failed_tool_recovery_attempts < MAX_FAILED_TOOL_RECOVERY_ATTEMPTS {
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
                            max_consec, MAX_FAILED_TOOL_RECOVERY_ATTEMPTS
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
                if failed_tool_recovery_attempts < MAX_FAILED_TOOL_RECOVERY_ATTEMPTS {
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
                            window_size, MAX_FAILED_TOOL_RECOVERY_ATTEMPTS
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
                            RoundInjectionKind::UserSteering => format!(
                                "<system_reminder>\nThe user sent a new message while this turn was running. You have just finished the previous atomic action; handle this new user message now as the current direction, while preserving the existing conversation and task context. Do not ignore it or wait for a separate future turn.\n\nNew user message:\n{}\n</system_reminder>",
                                injection.content
                            ),
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
                        let user_msg = Message::internal_reminder(reminder_kind, wrapped)
                            .with_turn_id(context.dialog_turn_id.clone());
                        messages.push(user_msg.clone());
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
                            if partial_continuation_attempts <= MAX_PARTIAL_CONTINUATION_ATTEMPTS {
                                let reminder = format!(
                                    "<system_reminder>Your previous assistant response was interrupted mid-stream ({reason}). Continue writing from exactly where you stopped. Do not repeat content that was already delivered; pick up seamlessly and complete the answer.</system_reminder>"
                                );
                                let user_msg = Message::internal_reminder(
                                    InternalReminderKind::InterruptedContinue,
                                    reminder.clone(),
                                )
                                .with_turn_id(context.dialog_turn_id.clone());
                                messages.push(user_msg.clone());
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
                                    MAX_PARTIAL_CONTINUATION_ATTEMPTS,
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
                        break;
                    }
                } else if round_result.had_thinking_content {
                    thinking_only_rescue_attempts += 1;
                    let reminder = "<system_reminder>The previous round produced internal reasoning only — no tool call and no user-visible response. You MUST now either: (1) call the single tool that best advances the user's task, or (2) write your final answer to the user. Do not produce another round of reasoning without taking action.</system_reminder>".to_string();
                    let user_msg = Message::internal_reminder(
                        InternalReminderKind::ThinkingOnlyRescue,
                        reminder.clone(),
                    )
                    .with_turn_id(context.dialog_turn_id.clone());
                    messages.push(user_msg.clone());
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

                if context.emit_lifecycle_events {
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
                let finalize_round_group_id = Some(format!(
                    "{}:finalize:{}",
                    context.dialog_turn_id, completed_rounds
                ));
                info!(
                    "Finalizing dialog turn: session_id={}, turn_id={}, reason={}",
                    context.session_id, context.dialog_turn_id, reason
                );

                let finalize_prepended_reminders = turn_prompt_scaffold
                    .prepended_prompt_reminders
                    .ordered_reminders();
                let final_round_result = self
                    .run_finalize_round(FinalizeRoundInput {
                        ai_client: ai_client.clone(),
                        context: &context,
                        agent_type: agent_type.clone(),
                        round_number: completed_rounds,
                        round_group_id: finalize_round_group_id.clone(),
                        execution_context_vars: &execution_context_vars,
                        primary_model_facts: &primary_model_facts,
                        prepended_reminders: &finalize_prepended_reminders,
                        messages: &messages,
                        reminder_text: finalize_reminder,
                        tool_definitions: tool_definitions.clone(),
                        context_window,
                    })
                    .await?;

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
                    let retry_result = self
                        .run_finalize_round(FinalizeRoundInput {
                            ai_client: ai_client.clone(),
                            context: &context,
                            agent_type: agent_type.clone(),
                            round_number: completed_rounds,
                            round_group_id: finalize_round_group_id.clone(),
                            execution_context_vars: &execution_context_vars,
                            primary_model_facts: &primary_model_facts,
                            prepended_reminders: &finalize_prepended_reminders,
                            messages: &messages,
                            reminder_text: finalize_reminder,
                            tool_definitions: tool_definitions.clone(),
                            context_window,
                        })
                        .await?;
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
                    if let Err(e) = self
                        .session_manager
                        .add_message(&context.session_id, msg)
                        .await
                    {
                        warn!("Failed to update final assistant message in memory: {}", e);
                    }
                }
            } else if reason == "partial_truncated" {
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
                "max_rounds" | "repeated_tool_failures"
            );

        // Post-processing hook: when a DeepResearch dialog turn finishes
        // successfully, renumber `cit_XXX` references in the final report
        // into consecutive `[N]` display IDs. Two gates apply (agent type +
        // dialog success) so other agents and failed turns are unaffected.
        #[cfg(feature = "product-full")]
        {
            if bitfun_agent_runtime::deep_research::should_post_process_research_report(
                &agent_type,
                success,
            ) {
                if let Some(workspace) = context.workspace.as_ref() {
                    bitfun_services_integrations::deep_research::run_for_session_workspace(
                        workspace.root_path(),
                        &context.session_id,
                    )
                    .await;
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
                        partial_recovery_reason: last_partial_recovery_reason,
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
        if let Some(usage) = last_usage {
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

        // Calculate newly generated messages
        let safe_initial_count = initial_count.min(messages.len()); // Ensure no out-of-bounds
        let new_messages = messages[safe_initial_count..].to_vec();

        if safe_initial_count != initial_count {
            warn!(
                "initial_count ({}) exceeds messages length ({}), adjusted to {}",
                initial_count,
                messages.len(),
                safe_initial_count
            );
        }

        Ok(ExecutionResult {
            final_message: messages
                .iter()
                .rev()
                .find(|message| message.role == MessageRole::Assistant)
                .cloned()
                .unwrap_or_else(|| Message::assistant(String::new())),
            total_rounds: completed_rounds,
            success,
            new_messages,
            finish_reason,
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
    use super::{ContextHealthSnapshot, ExecutionEngine, TurnPromptScaffold};
    use crate::agentic::agents::PrependedPromptReminders;
    use crate::agentic::core::{InternalReminderKind, Message, MessageRole, ToolCall, ToolResult};
    use crate::agentic::session::{TokenAnchor, TokenAnchorInput};
    use crate::agentic::tools::ToolRuntimeRestrictions;
    use crate::service::config::types::AIConfig;
    use crate::service::config::types::AIModelConfig;
    use crate::util::types::ToolDefinition;
    use serde_json::json;
    use sha2::{Digest, Sha256};
    use std::collections::HashMap;

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
    fn compression_trigger_budget_uses_the_automatic_output_tier_when_max_tokens_is_unset() {
        let budget = ExecutionEngine::compression_trigger_budget(128_000, None);

        assert_eq!(budget.output_reserve_tokens, 32_000);
        assert_eq!(budget.safety_reserve_tokens, 10_000);
        assert_eq!(budget.input_limit, 86_000);
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
}
