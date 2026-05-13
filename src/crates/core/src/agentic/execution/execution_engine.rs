//! Execution Engine
//!
//! Executes complete dialog turns, managing loops of multiple model rounds

use super::round_executor::RoundExecutor;
use super::types::{ExecutionContext, ExecutionResult, RoundContext, RoundResult};
use crate::agentic::agents::{
    get_agent_registry, PromptBuilder, PromptBuilderContext, RemoteExecutionHints,
};
use crate::agentic::context_profile::{ContextProfilePolicy, ModelCapabilityProfile};
use crate::agentic::core::{
    render_system_reminder, Message, MessageContent, MessageHelper, MessageRole,
    MessageSemanticKind, RequestReasoningTokenPolicy, Session,
};
use crate::agentic::events::{AgenticEvent, EventPriority, EventQueue};
use crate::agentic::execution::types::FinishReason;
use crate::agentic::image_analysis::{
    build_multimodal_message_with_images, process_image_contexts_for_provider, ImageContextData,
    ImageLimits,
};
use crate::agentic::session::{CompressionTailPolicy, ContextCompressor, SessionManager};
use crate::agentic::tools::{resolve_tool_manifest, ResolvedToolManifest, SubagentParentInfo, ToolRuntimeRestrictions};
use crate::agentic::util::build_remote_workspace_layout_preview;
use crate::agentic::{WorkspaceBackend, WorkspaceBinding};
use crate::infrastructure::ai::get_global_ai_client_factory;
use crate::service::config::get_global_config_service;
use crate::service::config::types::{ModelCapability, ModelCategory};
use crate::service::remote_ssh::workspace_state::get_remote_workspace_manager;
use crate::util::errors::{BitFunError, BitFunResult};
use crate::util::token_counter::TokenCounter;
use crate::util::types::Message as AIMessage;
use crate::util::types::ToolDefinition;
use crate::util::{elapsed_ms_u64, truncate_at_char_boundary};
use log::{debug, error, info, trace, warn};
use sha2::{Digest, Sha256};
use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::Path;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

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

/// Execution engine
pub struct ExecutionEngine {
    round_executor: Arc<RoundExecutor>,
    event_queue: Arc<EventQueue>,
    session_manager: Arc<SessionManager>,
    context_compressor: Arc<ContextCompressor>,
    config: ExecutionEngineConfig,
}

impl ExecutionEngine {
    const FINALIZE_AFTER_TOOL_USE_REMINDER: &'static str = "Tool execution for this turn has already completed, but the turn is ending at this round boundary. Do not call any more tools. Provide the final response to the user based on the tool results already available.";
    const FORCE_TEXT_ONLY_REMINDER: &'static str = "STOP. Tool calls are disabled for this final turn. Respond ONLY with a plain-text answer summarizing what you have done and the result for the user. Do not output tool call syntax of any kind.";

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

    fn has_tool_result_after_last_assistant(messages: &[Message]) -> bool {
        let Some(last_assistant_index) = messages
            .iter()
            .rposition(|message| message.role == MessageRole::Assistant)
        else {
            return false;
        };

        messages[last_assistant_index + 1..]
            .iter()
            .any(|message| matches!(message.content, MessageContent::ToolResult { .. }))
    }

    /// Emergency truncation: drop oldest API rounds (assistant+tool pairs)
    /// from the front of the message list until estimated tokens fit within
    /// `context_window`.  System messages and the first user message are
    /// always preserved.
    fn emergency_truncate_messages(
        messages: Vec<Message>,
        context_window: usize,
        tools: Option<&[ToolDefinition]>,
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

    fn should_use_fast_auto_model(turn_index: usize, original_user_input: &str) -> bool {
        turn_index == 0 && original_user_input.chars().count() <= 10
    }

    fn collect_unlocked_collapsed_tools(
        messages: &[Message],
        collapsed_tools: &[String],
    ) -> Vec<String> {
        let collapsed_set: HashSet<&str> = collapsed_tools.iter().map(String::as_str).collect();
        let mut unlocked = BTreeSet::new();

        for message in messages {
            let MessageContent::ToolResult {
                tool_name,
                result,
                is_error,
                ..
            } = &message.content
            else {
                continue;
            };

            if *is_error || tool_name != "GetToolSpec" {
                continue;
            }

            let Some(tool_name) = result.get("tool_name").and_then(|v| v.as_str())
            else {
                continue;
            };

            if collapsed_set.contains(tool_name) {
                unlocked.insert(tool_name.to_string());
            }
        }

        unlocked.into_iter().collect()
    }

    async fn build_prompt_context(
        context: &ExecutionContext,
        model_name: &str,
        supports_image_understanding: bool,
        has_additional_tools: bool,
    ) -> Option<PromptBuilderContext> {
        let workspace_path = context
            .workspace
            .as_ref()
            .map(|workspace| workspace.root_path_string())?;

        let base = PromptBuilderContext::new(
            workspace_path.clone(),
            Some(context.session_id.clone()),
            Some(model_name.to_string()),
        )
        .with_supports_image_understanding(supports_image_understanding)
        .with_additional_tools_hint(has_additional_tools);

        let Some(workspace) = context.workspace.as_ref() else {
            return Some(base);
        };
        if !workspace.is_remote() {
            return Some(base);
        }

        let Some(connection_id) = workspace.connection_id() else {
            return Some(base);
        };
        let Some(manager) = get_remote_workspace_manager() else {
            warn!(
                "Remote workspace active but RemoteWorkspaceStateManager is missing; using client OS hints only"
            );
            return Some(base);
        };

        let ssh_manager = manager.get_ssh_manager().await;
        let file_service = manager.get_file_service().await;
        let (kernel_name, hostname) = if let Some(ref ssh) = ssh_manager {
            if let Some(info) = ssh.get_server_info(connection_id).await {
                (info.os_type, info.hostname)
            } else {
                ("Linux".to_string(), "remote".to_string())
            }
        } else {
            ("Linux".to_string(), "remote".to_string())
        };
        let connection_display_name = match &workspace.backend {
            WorkspaceBackend::Remote {
                connection_name, ..
            } => connection_name.clone(),
            _ => connection_id.to_string(),
        };
        let remote_layout = if let Some(ref fs) = file_service {
            match build_remote_workspace_layout_preview(fs, connection_id, &workspace_path, 200)
                .await
            {
                Ok((_, preview)) => Some(preview),
                Err(e) => {
                    warn!("Remote workspace layout for prompt failed: {}", e);
                    None
                }
            }
        } else {
            None
        };

        Some(base.with_remote_prompt_overlay(
            RemoteExecutionHints {
                connection_display_name,
                kernel_name,
                hostname,
            },
            remote_layout,
        ))
    }

    pub(crate) async fn resolve_model_id_for_turn(
        &self,
        session: &Session,
        agent_type: &str,
        workspace: Option<&WorkspaceBinding>,
        original_user_input: &str,
        turn_index: usize,
    ) -> BitFunResult<String> {
        let agent_registry = get_agent_registry();
        let fallback_model_id = agent_registry
            .get_model_id_for_agent(agent_type, workspace.map(|binding| binding.root_path()))
            .await
            .map_err(|e| BitFunError::AIClient(format!("Failed to get model ID: {}", e)))?;
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

        let model_id = if configured_model_id == "auto" || resolved_configured_model_id == "auto" {
            let use_fast_model = Self::should_use_fast_auto_model(turn_index, original_user_input);
            let fallback_model = if use_fast_model { "fast" } else { "primary" };
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

    /// Synthesize one extra finalize round with tools disabled, returning the
    /// resulting assistant message + metadata. Used by both the
    /// "summarize tool results" and "force text after thinking-only" paths.
    #[allow(clippy::too_many_arguments)]
    async fn run_finalize_round(
        &self,
        ai_client: Arc<crate::infrastructure::ai::AIClient>,
        context: &ExecutionContext,
        agent_type: String,
        round_number: usize,
        execution_context_vars: &HashMap<String, String>,
        primary_supports_image_understanding: bool,
        request_context_reminder: Option<&str>,
        messages: &[Message],
        reminder_text: &str,
        context_window: usize,
    ) -> BitFunResult<RoundResult> {
        let mut final_ai_messages = Self::build_ai_messages_for_send(
            messages,
            &ai_client.config.format,
            context
                .workspace
                .as_ref()
                .map(|workspace| workspace.root_path()),
            &context.dialog_turn_id,
            primary_supports_image_understanding,
            request_context_reminder,
        )
        .await?;
        final_ai_messages.push(AIMessage::user(reminder_text.to_string()));

        let round_context = RoundContext {
            session_id: context.session_id.clone(),
            subagent_parent_info: context.subagent_parent_info.clone(),
            dialog_turn_id: context.dialog_turn_id.clone(),
            turn_index: context.turn_index,
            round_number,
            workspace: context.workspace.clone(),
            messages: messages.to_vec(),
            available_tools: Vec::new(),
            collapsed_tools: Vec::new(),
            unlocked_collapsed_tools: Vec::new(),
            model_name: ai_client.config.model.clone(),
            agent_type,
            context_vars: execution_context_vars.clone(),
            runtime_tool_restrictions: context.runtime_tool_restrictions.clone(),
            steering_interrupt: None,
            cancellation_token: CancellationToken::new(),
            workspace_services: context.workspace_services.clone(),
            recover_partial_on_cancel: context.recover_partial_on_cancel,
        };

        // Tools are disabled here (None) — model must respond in plain text.
        self.round_executor
            .execute_round(
                ai_client,
                round_context,
                final_ai_messages,
                None,
                Some(context_window),
            )
            .await
    }

    async fn build_ai_messages_for_send(
        messages: &[Message],
        provider: &str,
        workspace_path: Option<&Path>,
        current_turn_id: &str,
        attach_images: bool,
        prepended_user_context: Option<&str>,
    ) -> BitFunResult<Vec<AIMessage>> {
        /// Only the last this many **messages** that contain images keep their images for the API.
        const MAX_IMAGE_BEARING_MESSAGE_ROUNDS: usize = 2;

        let limits = ImageLimits::for_provider(provider);

        let trimmed_user_context = prepended_user_context.and_then(|text| {
            let trimmed = text.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed)
            }
        });
        let mut result =
            Vec::with_capacity(messages.len() + usize::from(trimmed_user_context.is_some()));
        let mut attached_image_count = 0usize;
        let first_non_system_index = messages
            .iter()
            .position(|msg| msg.role != crate::agentic::core::MessageRole::System)
            .unwrap_or(messages.len());
        let mut user_context_injected = false;

        let keep_image_messages = if attach_images {
            Self::image_bearing_indices_to_keep(messages, MAX_IMAGE_BEARING_MESSAGE_ROUNDS)
        } else {
            HashSet::new()
        };

        for (msg_idx, msg) in messages.iter().enumerate() {
            if !user_context_injected && msg_idx == first_non_system_index {
                if let Some(user_context) = trimmed_user_context {
                    result.push(AIMessage::user(render_system_reminder(user_context)));
                }
                user_context_injected = true;
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

        if !user_context_injected {
            if let Some(user_context) = trimmed_user_context {
                result.push(AIMessage::user(render_system_reminder(user_context)));
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
                .or_else(|| image.image_path.as_ref().filter(|s| !s.is_empty()).cloned())
                .unwrap_or_else(|| image.id.clone());

            content.push_str(&format!(
                "- {} ({}, image_id={})\n",
                name, image.mime_type, image.id
            ));
        }
        content.push_str("]\n");

        content.push_str("Note: image inspection is not available for this session.\n");

        content
    }

    /// Compress context, will emit compression events (Started, Completed, and Failed)
    #[allow(clippy::too_many_arguments)]
    pub async fn compress_messages(
        &self,
        session_id: &str,
        dialog_turn_id: &str,
        subagent_parent_info: Option<SubagentParentInfo>,
        messages: Vec<Message>,
        current_tokens: usize,
        context_window: usize,
        tool_definitions: &Option<Vec<ToolDefinition>>,
        system_prompt_message: Message,
        compression_contract_limit: usize,
        tail_policy: CompressionTailPolicy,
    ) -> BitFunResult<Option<(usize, Vec<Message>)>> {
        let event_subagent_parent_info = subagent_parent_info.map(|info| info.clone().into());
        let mut session = self
            .session_manager
            .get_session(session_id)
            .ok_or_else(|| BitFunError::NotFound(format!("Session not found: {}", session_id)))?;

        // Record start time
        let start_time = std::time::Instant::now();

        let old_messages_len = messages.len();
        // Preprocess turns
        let (turn_index_to_keep, turns) = self
            .context_compressor
            .preprocess_turns(session_id, context_window, messages)
            .await?;
        if turn_index_to_keep == 0 {
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
                tokens_before: current_tokens,
                context_window,
                threshold: session.config.compression_threshold,
                subagent_parent_info: event_subagent_parent_info.clone(),
            },
            EventPriority::Normal,
        )
        .await;

        // Execute compression
        let compression_contract = self
            .session_manager
            .compression_contract_for_session(session_id, compression_contract_limit);
        match self
            .context_compressor
            .compress_turns_with_contract(
                session_id,
                context_window,
                turn_index_to_keep,
                turns,
                tail_policy,
                compression_contract,
            )
            .await
        {
            Ok(compression_result) => {
                self.session_manager
                    .replace_context_messages(session_id, compression_result.messages.clone())
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
                let compressed_tokens = Self::estimate_request_tokens_internal(
                    &mut new_messages,
                    tool_definitions.as_deref(),
                );
                let summary_source = if compression_result.has_model_summary {
                    "model"
                } else {
                    "local_fallback"
                };

                info!(
                    "Compression completed: session_id={}, turn_id={}, messages {} -> {}, tokens {} -> {}, compression_count={}, duration_ms={}, summary_source={}",
                    session_id,
                    dialog_turn_id,
                    old_messages_len,
                    new_messages.len(),
                    current_tokens,
                    compressed_tokens,
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
                        tokens_before: current_tokens,
                        tokens_after: compressed_tokens,
                        compression_ratio: (compressed_tokens as f64) / (current_tokens as f64),
                        duration_ms,
                        has_summary: compression_result.has_model_summary,
                        summary_source: summary_source.to_string(),
                        subagent_parent_info: event_subagent_parent_info.clone(),
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
                        subagent_parent_info: event_subagent_parent_info.clone(),
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
        session_id: &str,
        dialog_turn_id: &str,
        messages: Vec<Message>,
        current_tokens: usize,
        context_window: usize,
        trigger: &str,
        tail_policy: CompressionTailPolicy,
    ) -> BitFunResult<ContextCompactionOutcome> {
        let mut session = self
            .session_manager
            .get_session(session_id)
            .ok_or_else(|| BitFunError::NotFound(format!("Session not found: {}", session_id)))?;
        let start_time = std::time::Instant::now();
        let compression_id = format!("compression_{}", uuid::Uuid::new_v4());

        self.emit_event(
            AgenticEvent::ContextCompressionStarted {
                session_id: session_id.to_string(),
                turn_id: dialog_turn_id.to_string(),
                compression_id: compression_id.clone(),
                trigger: trigger.to_string(),
                tokens_before: current_tokens,
                context_window,
                threshold: session.config.compression_threshold,
                subagent_parent_info: None,
            },
            EventPriority::Normal,
        )
        .await;

        let turns = self
            .context_compressor
            .collect_all_turns_for_manual_compaction(session_id, messages)?;

        if turns.is_empty() {
            let duration_ms = elapsed_ms_u64(start_time);
            let tokens_after = current_tokens;
            let compression_ratio = if current_tokens == 0 {
                1.0
            } else {
                (tokens_after as f64) / (current_tokens as f64)
            };

            self.emit_event(
                AgenticEvent::ContextCompressionCompleted {
                    session_id: session_id.to_string(),
                    turn_id: dialog_turn_id.to_string(),
                    compression_id: compression_id.clone(),
                    compression_count: session.compression_state.compression_count,
                    tokens_before: current_tokens,
                    tokens_after,
                    compression_ratio,
                    duration_ms,
                    has_summary: false,
                    summary_source: "none".to_string(),
                    subagent_parent_info: None,
                },
                EventPriority::Normal,
            )
            .await;

            return Ok(ContextCompactionOutcome {
                compression_id,
                compression_count: session.compression_state.compression_count,
                tokens_before: current_tokens,
                tokens_after,
                compression_ratio,
                duration_ms,
                has_summary: false,
                summary_source: "none".to_string(),
                applied: false,
            });
        }

        let is_review_subagent = get_agent_registry()
            .get_subagent_is_review(&session.agent_type)
            .unwrap_or(false);
        let model_id = session.config.model_id.as_deref().unwrap_or_default();
        let context_profile_policy = ContextProfilePolicy::for_agent_context_and_model(
            &session.agent_type,
            is_review_subagent,
            model_id,
            model_id,
        );
        let compression_contract = self.session_manager.compression_contract_for_session(
            session_id,
            context_profile_policy.compression_contract_limit,
        );
        match self
            .context_compressor
            .compress_turns_with_contract(
                session_id,
                context_window,
                turns.len(),
                turns,
                tail_policy,
                compression_contract,
            )
            .await
        {
            Ok(compression_result) => {
                let mut compressed_messages = compression_result.messages;
                self.session_manager
                    .replace_context_messages(session_id, compressed_messages.clone())
                    .await;

                session.compression_state.increment_compression_count();
                let compression_count = session.compression_state.compression_count;
                let _ = self
                    .session_manager
                    .update_compression_state(session_id, session.compression_state.clone())
                    .await;

                let duration_ms = elapsed_ms_u64(start_time);
                let tokens_after = compressed_messages
                    .iter_mut()
                    .map(|message| message.get_tokens())
                    .sum::<usize>();
                let compression_ratio = if current_tokens == 0 {
                    1.0
                } else {
                    (tokens_after as f64) / (current_tokens as f64)
                };

                self.emit_event(
                    AgenticEvent::ContextCompressionCompleted {
                        session_id: session_id.to_string(),
                        turn_id: dialog_turn_id.to_string(),
                        compression_id: compression_id.clone(),
                        compression_count,
                        tokens_before: current_tokens,
                        tokens_after,
                        compression_ratio,
                        duration_ms,
                        has_summary: compression_result.has_model_summary,
                        summary_source: if compression_result.has_model_summary {
                            "model".to_string()
                        } else {
                            "local_fallback".to_string()
                        },
                        subagent_parent_info: None,
                    },
                    EventPriority::Normal,
                )
                .await;

                Ok(ContextCompactionOutcome {
                    compression_id,
                    compression_count,
                    tokens_before: current_tokens,
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
                        subagent_parent_info: None,
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
        let event_subagent_parent_info =
            context.subagent_parent_info.clone().map(|info| info.into());
        let dialog_turn_id = context.dialog_turn_id.clone();

        debug!(
            "Executing dialog turn implementation: dialog_turn_id={}",
            dialog_turn_id
        );

        // Things that remain constant in a dialog turn: 1.agent, 2.system prompt, 3.tools, 4.ai client
        // 1. Get current agent
        let agent_registry = get_agent_registry();
        if let Some(workspace) = context.workspace.as_ref() {
            agent_registry
                .load_custom_subagents(workspace.root_path())
                .await;
        }
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
        let ai_client = ai_client_factory
            .get_client_resolved(&model_id)
            .await
            .map_err(|e| {
                BitFunError::AIClient(format!(
                    "Failed to get AI client (model_id={}): {}",
                    model_id, e
                ))
            })?;

        // Primary model vision capability (tools + system prompt appendix; also used below for API message stripping).
        let (resolved_primary_model_id, primary_supports_image_understanding) = {
            let config_service = get_global_config_service().await.ok();
            if let Some(service) = config_service {
                let ai_config: crate::service::config::types::AIConfig =
                    service.get_config(Some("ai")).await.unwrap_or_default();

                let resolved_id = Self::resolve_configured_model_id(&ai_config, &model_id);

                let model_cfg = ai_config
                    .models
                    .iter()
                    .find(|m| m.id == resolved_id)
                    .or_else(|| ai_config.models.iter().find(|m| m.name == resolved_id))
                    .or_else(|| {
                        ai_config
                            .models
                            .iter()
                            .find(|m| m.model_name == resolved_id)
                    })
                    .or_else(|| {
                        ai_config.models.iter().find(|m| {
                            m.model_name == ai_client.config.model
                                && m.provider == ai_client.config.format
                        })
                    });

                let supports = model_cfg.is_some_and(|m| {
                    m.capabilities
                        .iter()
                        .any(|cap| matches!(cap, ModelCapability::ImageUnderstanding))
                        || matches!(m.category, ModelCategory::Multimodal)
                });

                (resolved_id, supports)
            } else {
                warn!(
                    "Config service unavailable, assuming primary model is text-only for image input gating"
                );
                (model_id.clone(), false)
            }
        };

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
        let tool_manifest = if enable_tools {
            debug!(
                "Agent tools: agent={}, tool_count={}",
                agent_type,
                allowed_tools.len()
            );
            Some(
                self.get_available_tools_and_definitions(
                    &allowed_tools,
                    &tool_policy.exposure_overrides,
                    context.workspace.as_ref(),
                    context.workspace_services.as_ref(),
                    &agent_type,
                    primary_supports_image_understanding,
                )
                .await,
            )
        } else {
            None
        };
        let collapsed_tools = tool_manifest
            .as_ref()
            .map(|manifest| manifest.collapsed_tool_names.clone())
            .unwrap_or_default();
        let has_additional_tools = !collapsed_tools.is_empty();
        let (available_tools, tool_definitions) = if let Some(manifest) = tool_manifest {
            (manifest.allowed_tool_names, Some(manifest.tool_definitions))
        } else {
            (vec![], None)
        };

        // 4. Get System Prompt from current Agent
        debug!(
            "Building system prompt from agent: {}, model={}",
            current_agent.name(),
            ai_client.config.model
        );
        let prompt_context = Self::build_prompt_context(
            &context,
            &ai_client.config.model,
            primary_supports_image_understanding,
            has_additional_tools,
        )
        .await;
        let request_context_reminder = if let Some(prompt_context) = prompt_context.as_ref() {
            PromptBuilder::new(prompt_context.clone())
                .build_request_context_reminder(&current_agent.request_context_policy())
                .await
        } else {
            None
        };
        let system_prompt = current_agent
            .get_system_prompt(prompt_context.as_ref())
            .await?;
        debug!("System prompt built, length: {} bytes", system_prompt.len());
        debug!(
            "Request context reminder built, length: {} bytes",
            request_context_reminder
                .as_ref()
                .map(|text| text.len())
                .unwrap_or(0)
        );
        let system_prompt_message = Message::system(system_prompt.clone());

        // Add System Prompt to the beginning of message list (only for this execution, not persisted)
        let mut messages = vec![system_prompt_message.clone()];
        messages.extend(initial_messages);

        let mut round_index = 0;
        let mut completed_rounds = 0usize;
        let mut total_tools = 0;
        let mut last_partial_recovery_reason: Option<String> = None;
        let mut finalization_reason: Option<&'static str> = None;
        let mut consecutive_compression_failures: u32 = 0;
        const MAX_CONSECUTIVE_COMPRESSION_FAILURES: u32 = 3;

        // P0: Loop detection: track recent tool call signatures
        let mut recent_tool_signatures: Vec<String> = Vec::new();
        let mut loop_detected = false;
        let mut loop_recovery_attempts: usize = 0;
        const MAX_LOOP_RECOVERY_ATTEMPTS: usize = 3;
        let mut full_compression_count = 0usize;
        let mut compression_failure_count = 0u32;

        // Save the last token usage statistics
        let mut last_usage: Option<crate::util::types::ai::GeminiUsage> = None;

        // Track thinking-only rescue reminders for observability. This counter
        // is not a stop condition.
        let mut thinking_only_rescue_attempts: usize = 0;

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
        let compression_threshold = session.config.compression_threshold;

        let mut execution_context_vars = context.context.clone();
        execution_context_vars.insert(
            "primary_model_id".to_string(),
            resolved_primary_model_id.clone(),
        );
        execution_context_vars.insert(
            "primary_model_name".to_string(),
            ai_client.config.model.clone(),
        );
        execution_context_vars.insert(
            "primary_model_provider".to_string(),
            ai_client.config.format.clone(),
        );
        execution_context_vars.insert(
            "primary_model_supports_image_understanding".to_string(),
            primary_supports_image_understanding.to_string(),
        );
        execution_context_vars.insert("turn_index".to_string(), context.turn_index.to_string());

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
            let current_tokens =
                Self::estimate_request_tokens_internal(&messages, tool_definitions.as_deref());
            debug!(
                "Round {} token usage before send: {} / {} tokens ({:.1}%)",
                round_index,
                current_tokens,
                context_window,
                (current_tokens as f32 / context_window as f32) * 100.0
            );

            let token_usage_ratio = current_tokens as f32 / context_window as f32;
            let should_compress =
                enable_context_compression && token_usage_ratio >= compression_threshold;

            // Circuit breaker: skip full compression if it has failed too many
            // consecutive times.  Microcompact and emergency truncation still run.
            let circuit_breaker_open =
                consecutive_compression_failures >= MAX_CONSECUTIVE_COMPRESSION_FAILURES;

            if !should_compress {
                debug!(
                    "No compression needed: session={}, token_usage={:.1}%, threshold={:.1}%",
                    context.session_id,
                    token_usage_ratio * 100.0,
                    compression_threshold * 100.0
                );
            } else if circuit_breaker_open {
                warn!(
                    "Compression circuit breaker open ({} consecutive failures), skipping full compression for round {}",
                    consecutive_compression_failures, round_index
                );
            } else {
                info!(
                    "Triggering context compression: session={}, token_usage={:.1}%, threshold={:.1}%",
                    context.session_id,
                    token_usage_ratio * 100.0,
                    compression_threshold * 100.0
                );

                match self
                    .compress_messages(
                        &context.session_id,
                        &context.dialog_turn_id,
                        context.subagent_parent_info.clone(),
                        messages.clone(),
                        current_tokens,
                        context_window,
                        &tool_definitions,
                        system_prompt_message.clone(),
                        context_profile_policy.compression_contract_limit,
                        CompressionTailPolicy::PreserveLiveFrontier,
                    )
                    .await
                {
                    Ok(Some((compressed_tokens, compressed_messages))) => {
                        info!(
                            "Round {} compression completed: messages {} -> {}, tokens {} -> {}",
                            round_index,
                            messages.len(),
                            compressed_messages.len(),
                            current_tokens,
                            compressed_tokens,
                        );

                        messages = compressed_messages;
                        full_compression_count += 1;
                        consecutive_compression_failures = 0;
                    }
                    Ok(None) => {
                        debug!("All turns need to be kept, no compression performed");
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
            let post_compress_tokens =
                Self::estimate_request_tokens_internal(&messages, tool_definitions.as_deref());
            if post_compress_tokens > context_window {
                warn!(
                    "Round {} tokens ({}) still exceed context_window ({}) after compression, performing emergency truncation",
                    round_index, post_compress_tokens, context_window
                );
                messages = Self::emergency_truncate_messages(
                    messages,
                    context_window,
                    tool_definitions.as_deref(),
                );
                let after_truncate =
                    Self::estimate_request_tokens_internal(&messages, tool_definitions.as_deref());
                info!(
                    "Emergency truncation complete: tokens {} -> {}",
                    post_compress_tokens, after_truncate
                );
            }

            let before_send_tokens =
                Self::estimate_request_tokens_internal(&messages, tool_definitions.as_deref());
            ContextHealthSnapshot::from_runtime_observations(
                ContextHealthSnapshot::token_usage_ratio(before_send_tokens, context_window),
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
            let mut round_context_vars = execution_context_vars.clone();
            if context.skip_tool_confirmation {
                round_context_vars.insert("skip_tool_confirmation".to_string(), "true".to_string());
            }
            let unlocked_collapsed_tools =
                Self::collect_unlocked_collapsed_tools(&messages, &collapsed_tools);
            let round_context = RoundContext {
                session_id: context.session_id.clone(),
                subagent_parent_info: context.subagent_parent_info.clone(),
                dialog_turn_id: context.dialog_turn_id.clone(),
                turn_index: context.turn_index,
                round_number: round_index,
                workspace: context.workspace.clone(),
                messages: messages.clone(),
                available_tools: available_tools.clone(),
                collapsed_tools: collapsed_tools.clone(),
                unlocked_collapsed_tools,
                model_name: ai_client.config.model.clone(),
                agent_type: agent_type.clone(),
                context_vars: round_context_vars,
                runtime_tool_restrictions: context.runtime_tool_restrictions.clone(),
                steering_interrupt: context.round_steering.as_ref().map(|source| {
                    crate::agentic::round_preempt::DialogRoundSteeringInterrupt::new(
                        context.session_id.clone(),
                        context.dialog_turn_id.clone(),
                        Arc::clone(source),
                    )
                }),
                cancellation_token: CancellationToken::new(),
                workspace_services: context.workspace_services.clone(),
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
                request_context_reminder.as_deref(),
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

            // P0: Consecutive same-tool-call loop detection
            if !round_result.tool_calls.is_empty() {
                let mut sigs: Vec<String> = round_result
                    .tool_calls
                    .iter()
                    .map(|tc| {
                        let args_str = tc.arguments.to_string();
                        let args_summary = Self::tool_signature_args_summary(&args_str);
                        format!("{}:{}", tc.tool_name, args_summary)
                    })
                    .collect();
                sigs.sort();
                let round_sig = sigs.join("|");
                recent_tool_signatures.push(round_sig);
            } else {
                recent_tool_signatures.clear();
            }

            let after_round_tokens =
                Self::estimate_request_tokens_internal(&messages, tool_definitions.as_deref());
            let after_round_health = ContextHealthSnapshot::from_runtime_observations(
                ContextHealthSnapshot::token_usage_ratio(after_round_tokens, context_window),
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
            if recent_tool_signatures.len() >= max_consec {
                let tail = &recent_tool_signatures[recent_tool_signatures.len() - max_consec..];
                if tail.windows(2).all(|w| w[0] == w[1]) {
                    if loop_recovery_attempts < MAX_LOOP_RECOVERY_ATTEMPTS {
                        loop_recovery_attempts += 1;
                        warn!(
                            "Loop detected: {} consecutive rounds with identical tool signatures, injecting recovery prompt #{}",
                            max_consec, loop_recovery_attempts
                        );
                        let reminder = format!(
                            "<system_reminder>Loop detected: you have repeated the same tool call with identical arguments {} times in a row. \
                            This means the approach is not making progress. You MUST now change your strategy: \
                            (1) if the tool keeps failing, try a completely different approach or tool; \
                            (2) if you are stuck, step back and reason about the root cause before acting; \
                            (3) if the task is genuinely impossible with the available tools, provide a clear explanation to the user. \
                            Do NOT repeat the same tool call again.</system_reminder>",
                            max_consec
                        );
                        let user_msg = Message::user(reminder);
                        messages.push(user_msg.clone());
                        if let Err(e) = self
                            .session_manager
                            .add_message(&context.session_id, user_msg)
                            .await
                        {
                            warn!("Failed to persist loop recovery reminder: {}", e);
                        }
                        // Clear the recent signatures so the detector resets after recovery.
                        recent_tool_signatures.clear();
                        // Do NOT break — continue the loop so the model gets a chance to recover.
                    } else {
                        warn!(
                            "Loop detected: {} consecutive rounds with identical tool signatures, max recovery attempts ({}) exhausted, stopping",
                            max_consec, MAX_LOOP_RECOVERY_ATTEMPTS
                        );
                        loop_detected = true;
                        finalization_reason = Some("loop_detected");
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
            if Self::is_periodic_tool_signature_loop(&recent_tool_signatures, max_consec) {
                let window_size = max_consec.max(1).saturating_mul(2);
                if loop_recovery_attempts < MAX_LOOP_RECOVERY_ATTEMPTS {
                    loop_recovery_attempts += 1;
                    warn!(
                        "Loop detected: last {} rounds form a periodic tool-call pattern (<= {} distinct signatures, each repeated), injecting recovery prompt #{}",
                        window_size, max_consec, loop_recovery_attempts
                    );
                    let reminder = format!(
                        "<system_reminder>Loop detected: your last {} tool calls form a repeating pattern with no new progress. \
                        You are cycling between the same actions without advancing the task. You MUST now change your strategy: \
                        (1) try a completely different approach or tool; \
                        (2) step back and reason about the root cause before acting; \
                        (3) if the task is genuinely impossible with the available tools, provide a clear explanation to the user. \
                        Do NOT repeat the same pattern of tool calls.</system_reminder>",
                        window_size
                    );
                    let user_msg = Message::user(reminder);
                    messages.push(user_msg.clone());
                    if let Err(e) = self
                        .session_manager
                        .add_message(&context.session_id, user_msg)
                        .await
                    {
                        warn!("Failed to persist periodic loop recovery reminder: {}", e);
                    }
                    // Clear the recent signatures so the detector resets after recovery.
                    recent_tool_signatures.clear();
                    // Do NOT break — continue the loop so the model gets a chance to recover.
                } else {
                    warn!(
                        "Loop detected: last {} rounds form a periodic tool-call pattern, max recovery attempts ({}) exhausted, stopping",
                        window_size, MAX_LOOP_RECOVERY_ATTEMPTS
                    );
                    loop_detected = true;
                    finalization_reason = Some("loop_detected");
                    break;
                }
            }

            // User-steering messages submitted while this turn is running: drain and inject
            // them as user messages into the working history before starting the next round
            // (Codex-style mid-turn injection). This does NOT end the current turn, in
            // contrast with the `round_preempt` path below which finalizes the turn so a
            // queued *new turn* can take over. If the model wanted to finish but the user
            // steered, we keep the turn running so the steering message gets a response.
            let mut steering_injected = false;
            if let Some(steer) = context.round_steering.as_ref() {
                let pending = steer.take_pending(&context.session_id, &context.dialog_turn_id);
                if !pending.is_empty() {
                    info!(
                        "Injecting {} user steering message(s) at round boundary: session_id={}, dialog_turn_id={}, round_index={}",
                        pending.len(),
                        context.session_id,
                        context.dialog_turn_id,
                        round_index
                    );
                    for steering_msg in pending {
                        // Wrap the steering content in a system_reminder envelope so the
                        // model treats it as an out-of-band course correction layered on
                        // top of the running task, not as a brand-new top-level instruction
                        // that supersedes everything before it. Matches Codex CLI semantics.
                        let wrapped = format!(
                            "<system_reminder>\nThe user sent a new message while this turn was running. You have just finished the previous atomic action; handle this new user message now as the current direction, while preserving the existing conversation and task context. Do not ignore it or wait for a separate future turn.\n\nNew user message:\n{}\n</system_reminder>",
                            steering_msg.content
                        );
                        let user_msg = Message::user(wrapped);
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
                                steering_id: steering_msg.id,
                                content: steering_msg.content,
                                display_content: steering_msg.display_content,
                                subagent_parent_info: event_subagent_parent_info.clone(),
                            },
                            EventPriority::Normal,
                        )
                        .await;
                        steering_injected = true;
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
            // - Model emitted user-visible text  -> final answer, end the turn.
            // - Model emitted thinking only      -> stalled mid-reasoning. Inject a
            //   system_reminder asking it to either act (call a tool) or finish
            //   (write the answer), and continue.
            // - Model emitted nothing at all     -> partial recovery / truncation.
            //   Retrying without new context will not help, so end the turn.
            if steering_injected {
                // fall through to next round so the model can respond to the steering
            } else if !round_result.has_more_rounds {
                if round_result.had_assistant_text {
                    debug!(
                        "Model round {} ended with final answer, reason: {:?}",
                        round_index, round_result.finish_reason
                    );
                    break;
                } else if round_result.had_thinking_content {
                    thinking_only_rescue_attempts += 1;
                    let reminder = "<system_reminder>The previous round produced internal reasoning only — no tool call and no user-visible response. You MUST now either: (1) call the single tool that best advances the user's task, or (2) write your final answer to the user. Do not produce another round of reasoning without taking action.</system_reminder>".to_string();
                    let user_msg = Message::user(reminder.clone());
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

            // Queued user message while this turn was running: stop after a full model round.
            // The round output has already been reflected in the in-memory message caches.
            // No special deferral for tool-confirmation phases: we do not require the user to
            // finish confirming before this boundary check runs; the check applies as soon as
            // this `execute_round` completes (same as any other round).
            if let Some(preempt) = context.round_preempt.as_ref() {
                if preempt.should_yield_after_round(&context.session_id) {
                    preempt.clear_yield_after_round(&context.session_id);
                    info!(
                        "Yielding dialog turn after model round (queued user message): session_id={}, dialog_turn_id={}, round_index={}",
                        context.session_id, context.dialog_turn_id, round_index
                    );
                    finalization_reason = Some("queued_user_message");
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

                // Emit cancellation event
                self.emit_event(
                    AgenticEvent::DialogTurnCancelled {
                        session_id: context.session_id.clone(),
                        turn_id: context.dialog_turn_id.clone(),
                        subagent_parent_info: event_subagent_parent_info.clone(),
                    },
                    EventPriority::High,
                )
                .await;

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
        // Defaults to "complete" (model produced a final answer naturally) and
        // is overridden by finalize / fallback paths below.
        let mut effective_finish_reason: &'static str = match finalization_reason {
            Some(r) => r,
            None => "complete",
        };
        let mut finalize_fallback_text_used = false;

        if let Some(reason) = finalization_reason {
            // If the turn yielded after tool use, ask the model to summarize
            // tool results before the next queued user message takes over.
            let needs_finalize_after_tool_use = reason == "queued_user_message"
                && messages
                    .iter()
                    .rev()
                    .find(|message| message.role == MessageRole::Assistant)
                    .is_some_and(Self::assistant_has_tool_calls)
                && Self::has_tool_result_after_last_assistant(&messages);

            if needs_finalize_after_tool_use {
                info!(
                    "Finalizing dialog turn: session_id={}, turn_id={}, reason={}",
                    context.session_id, context.dialog_turn_id, reason
                );

                let final_round_result = self
                    .run_finalize_round(
                        ai_client.clone(),
                        &context,
                        agent_type.clone(),
                        completed_rounds,
                        &execution_context_vars,
                        primary_supports_image_understanding,
                        request_context_reminder.as_deref(),
                        &messages,
                        Self::FINALIZE_AFTER_TOOL_USE_REMINDER,
                        context_window,
                    )
                    .await?;

                let mut accepted =
                    !Self::assistant_has_tool_calls(&final_round_result.assistant_message);
                let mut chosen_assistant_message: Option<Message> = None;
                let mut chosen_usage: Option<crate::util::types::ai::GeminiUsage> =
                    final_round_result.usage.clone();

                if accepted {
                    chosen_assistant_message = Some(final_round_result.assistant_message.clone());
                } else {
                    // P1-10: First finalize round still returned tool calls
                    // (rare; tools were not provided, but model hallucinated).
                    // One last attempt with a stricter text-only reminder.
                    warn!(
                        "Finalize round still returned tool calls; retrying with text-only reminder: session_id={}, turn_id={}",
                        context.session_id, context.dialog_turn_id
                    );
                    let retry_result = self
                        .run_finalize_round(
                            ai_client.clone(),
                            &context,
                            agent_type.clone(),
                            completed_rounds,
                            &execution_context_vars,
                            primary_supports_image_understanding,
                            request_context_reminder.as_deref(),
                            &messages,
                            Self::FORCE_TEXT_ONLY_REMINDER,
                            context_window,
                        )
                        .await?;
                    finalize_fallback_text_used = true;
                    if Self::assistant_has_tool_calls(&retry_result.assistant_message) {
                        warn!(
                            "Text-only retry also returned tool calls; keeping prior messages: session_id={}, turn_id={}",
                            context.session_id, context.dialog_turn_id
                        );
                    } else {
                        accepted = true;
                        chosen_usage = retry_result.usage.clone();
                        chosen_assistant_message = Some(retry_result.assistant_message);
                    }
                }

                if let Some(msg) = chosen_assistant_message {
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

                if !accepted {
                    effective_finish_reason = "finalize_failed";
                } else if finalize_fallback_text_used {
                    effective_finish_reason = "finalize_text_only_forced";
                }
            }
        }

        let duration_ms = elapsed_ms_u64(start_time);

        info!(
            "Dialog turn loop completed: turn={}, rounds={}, total_tools={}, reason={}",
            context.dialog_turn_id, completed_rounds, total_tools, effective_finish_reason
        );

        let finish_reason = FinishReason::Complete;
        // success reflects whether we ended with a usable final answer.
        let success = !loop_detected
            && !matches!(
                effective_finish_reason,
                "finalize_failed" | "empty_round" | "max_rounds"
            );

        // Emit dialog turn completed event
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
                    subagent_parent_info: event_subagent_parent_info,
                    partial_recovery_reason: last_partial_recovery_reason,
                    success: Some(success),
                    finish_reason: Some(effective_finish_reason.to_string()),
                },
                None,
            )
            .await;

        debug!("DialogTurnCompleted event sent");

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

    /// Cleanup cancellation token (for external calls)
    pub async fn cleanup_cancel_token(&self, dialog_turn_id: &str) {
        self.round_executor
            .cleanup_dialog_turn(dialog_turn_id)
            .await
    }

    /// Get available tool names and definitions: 1. Tool itself is enabled 2. Explicitly allowed in mode config
    async fn get_available_tools_and_definitions(
        &self,
        allowed_tools: &[String],
        exposure_overrides: &crate::agentic::agents::AgentToolPolicyOverrides,
        workspace: Option<&crate::agentic::WorkspaceBinding>,
        workspace_services: Option<&crate::agentic::workspace::WorkspaceServices>,
        agent_type: &str,
        primary_supports_image_understanding: bool,
    ) -> ResolvedToolManifest {
        let mut tool_opts_custom = HashMap::new();
        tool_opts_custom.insert(
            "primary_model_supports_image_understanding".to_string(),
            serde_json::Value::Bool(primary_supports_image_understanding),
        );
        let description_context = crate::agentic::tools::framework::ToolUseContext {
            tool_call_id: None,
            agent_type: Some(agent_type.to_string()),
            session_id: None,
            dialog_turn_id: None,
            workspace: workspace.cloned(),
            unlocked_collapsed_tools: Vec::new(),
            custom_data: tool_opts_custom,
            computer_use_host: None,
            cancellation_token: None,
            runtime_tool_restrictions: ToolRuntimeRestrictions::default(),
            workspace_services: workspace_services.cloned(),
        };
        resolve_tool_manifest(allowed_tools, exposure_overrides, &description_context).await
    }

    /// Emit event
    async fn emit_event(&self, event: AgenticEvent, priority: EventPriority) {
        let _ = self.event_queue.enqueue(event, Some(priority)).await;
    }
}

#[cfg(test)]
mod tests {
    use super::{ContextHealthSnapshot, ExecutionEngine};
    use crate::agentic::core::{Message, ToolCall, ToolResult};
    use crate::service::config::types::AIConfig;
    use crate::service::config::types::AIModelConfig;
    use serde_json::json;
    use sha2::{Digest, Sha256};

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

    #[test]
    fn auto_model_uses_fast_for_short_first_message() {
        assert!(ExecutionEngine::should_use_fast_auto_model(0, "你好"));
        assert!(ExecutionEngine::should_use_fast_auto_model(0, "1234567890"));
    }

    #[test]
    fn auto_model_uses_primary_for_long_first_message() {
        assert!(!ExecutionEngine::should_use_fast_auto_model(
            0,
            "12345678901"
        ));
    }

    #[test]
    fn auto_model_uses_primary_after_first_turn() {
        assert!(!ExecutionEngine::should_use_fast_auto_model(1, "短消息"));
    }

    #[test]
    fn resolve_configured_fast_model_falls_back_to_primary_when_fast_is_stale() {
        let mut ai_config = AIConfig::default();
        ai_config.models = vec![build_model("model-primary", "Primary", "claude-sonnet-4.5")];
        ai_config.default_models.primary = Some("model-primary".to_string());
        ai_config.default_models.fast = Some("deleted-fast-model".to_string());

        assert_eq!(
            ExecutionEngine::resolve_configured_model_id(&ai_config, "fast"),
            "model-primary"
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
    fn assistant_has_tool_calls_detects_mixed_tool_message() {
        let message = Message::assistant_with_tools(
            String::new(),
            vec![ToolCall {
                tool_id: "tool-1".to_string(),
                tool_name: "Read".to_string(),
                arguments: json!({ "path": "README.md" }),
                raw_arguments: None,
                is_error: false,
                recovered_from_truncation: false,
            }],
        );

        assert!(ExecutionEngine::assistant_has_tool_calls(&message));
        assert!(!ExecutionEngine::assistant_has_tool_calls(
            &Message::assistant("done".to_string())
        ));
    }

    #[test]
    fn collects_unlocked_collapsed_tools_from_visible_get_tool_spec_results() {
        let visible_get_tool_spec_result = Message::tool_result(ToolResult {
            tool_id: "tool-1".to_string(),
            tool_name: "GetToolSpec".to_string(),
            result: json!({
                "tool_name": "WebFetch",
            }),
            result_for_assistant: None,
            is_error: false,
            duration_ms: Some(1),
            image_attachments: None,
        });
        let hidden_get_tool_spec_result = Message::tool_result(ToolResult {
            tool_id: "tool-2".to_string(),
            tool_name: "GetToolSpec".to_string(),
            result: json!({
                "tool_name": "Read",
            }),
            result_for_assistant: None,
            is_error: false,
            duration_ms: Some(1),
            image_attachments: None,
        });
        let failed_get_tool_spec_result = Message::tool_result(ToolResult {
            tool_id: "tool-3".to_string(),
            tool_name: "GetToolSpec".to_string(),
            result: json!({
                "tool_name": "GetFileDiff",
            }),
            result_for_assistant: None,
            is_error: true,
            duration_ms: Some(1),
            image_attachments: None,
        });

        let unlocked = ExecutionEngine::collect_unlocked_collapsed_tools(
            &[
                visible_get_tool_spec_result,
                hidden_get_tool_spec_result,
                failed_get_tool_spec_result,
            ],
            &["WebFetch".to_string(), "GetFileDiff".to_string()],
        );

        assert_eq!(unlocked, vec!["WebFetch".to_string()]);
    }

    #[test]
    fn detects_tool_result_after_last_assistant() {
        let assistant = Message::assistant_with_tools(
            String::new(),
            vec![ToolCall {
                tool_id: "tool-1".to_string(),
                tool_name: "Read".to_string(),
                arguments: json!({ "path": "README.md" }),
                raw_arguments: None,
                is_error: false,
                recovered_from_truncation: false,
            }],
        );
        let tool_result = Message::tool_result(ToolResult {
            tool_id: "tool-1".to_string(),
            tool_name: "Read".to_string(),
            result: json!({ "content": "hello" }),
            result_for_assistant: Some("hello".to_string()),
            is_error: false,
            duration_ms: Some(1),
            image_attachments: None,
        });

        assert!(ExecutionEngine::has_tool_result_after_last_assistant(&[
            Message::user("read it".to_string()),
            assistant.clone(),
            tool_result,
        ]));
        assert!(!ExecutionEngine::has_tool_result_after_last_assistant(&[
            Message::user("read it".to_string()),
            assistant,
        ]));
    }

    fn command_result(tool_name: &str, success: bool, exit_code: Option<i32>) -> Message {
        Message::tool_result(ToolResult {
            tool_id: format!("{}-tool", tool_name),
            tool_name: tool_name.to_string(),
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
