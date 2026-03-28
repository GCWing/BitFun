//! Execution Engine
//!
//! Executes complete dialog turns, managing loops of multiple model rounds

use super::round_executor::RoundExecutor;
use super::types::{ExecutionContext, ExecutionResult, RoundContext};
use crate::agentic::agents::{get_agent_registry, PromptBuilderContext};
use crate::agentic::core::{Message, MessageContent, MessageHelper, MessageSemanticKind, Session};
use crate::agentic::events::{AgenticEvent, EventPriority, EventQueue};
use crate::agentic::image_analysis::{
    build_multimodal_message_with_images, process_image_contexts_for_provider, ImageContextData,
    ImageLimits,
};
use crate::agentic::session::{CompressionTailPolicy, ContextCompressor, SessionManager};
use crate::agentic::tools::{get_all_registered_tools, SubagentParentInfo};
use crate::agentic::WorkspaceBinding;
use crate::infrastructure::ai::get_global_ai_client_factory;
use crate::service::config::get_global_config_service;
use crate::service::config::types::{ModelCapability, ModelCategory};
use crate::util::errors::{BitFunError, BitFunResult};
use crate::util::token_counter::TokenCounter;
use crate::util::types::Message as AIMessage;
use crate::util::types::ToolDefinition;
use log::{debug, error, info, trace, warn};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

/// Execution engine configuration
#[derive(Debug, Clone)]
pub struct ExecutionEngineConfig {
    pub max_rounds: usize, // Maximum number of rounds to prevent infinite loops
}

impl Default for ExecutionEngineConfig {
    fn default() -> Self {
        Self { max_rounds: 200 }
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

/// Execution engine
pub struct ExecutionEngine {
    round_executor: Arc<RoundExecutor>,
    event_queue: Arc<EventQueue>,
    session_manager: Arc<SessionManager>,
    context_compressor: Arc<ContextCompressor>,
    config: ExecutionEngineConfig,
}

impl ExecutionEngine {
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
        messages: &mut [Message],
        tools: Option<&[ToolDefinition]>,
    ) -> usize {
        let mut total: usize = messages.iter_mut().map(|m| m.get_tokens()).sum();
        total += 3;

        if let Some(tool_defs) = tools {
            total += TokenCounter::estimate_tool_definitions_tokens(tool_defs);
        }

        total
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

    /// Indices of the last `max_rounds` messages that bear images (`max_rounds` = 2 → keep images only there).
    fn image_bearing_indices_to_keep(messages: &[Message], max_rounds: usize) -> HashSet<usize> {
        let with_images: Vec<usize> = messages
            .iter()
            .enumerate()
            .filter(|(_, m)| Self::message_bears_images(m))
            .map(|(i, _)| i)
            .collect();
        let n = with_images.len();
        if n <= max_rounds {
            return with_images.into_iter().collect();
        }
        with_images[n - max_rounds..].iter().copied().collect()
    }

    async fn build_ai_messages_for_send(
        messages: &[Message],
        provider: &str,
        workspace_path: Option<&Path>,
        current_turn_id: &str,
        attach_images: bool,
    ) -> BitFunResult<Vec<AIMessage>> {
        /// Only the last this many **messages** that contain images keep their images for the API.
        const MAX_IMAGE_BEARING_MESSAGE_ROUNDS: usize = 2;

        let limits = ImageLimits::for_provider(provider);

        let mut result = Vec::with_capacity(messages.len());
        let mut attached_image_count = 0usize;

        let keep_image_messages = if attach_images {
            Self::image_bearing_indices_to_keep(messages, MAX_IMAGE_BEARING_MESSAGE_ROUNDS)
        } else {
            HashSet::new()
        };

        for (msg_idx, msg) in messages.iter().enumerate() {
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
        match self
            .context_compressor
            .compress_turns(
                session_id,
                context_window,
                turn_index_to_keep,
                turns,
                tail_policy,
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

                info!(
                    "Compression completed: messages {} -> {}, compression_count={}",
                    old_messages_len,
                    new_messages.len(),
                    session.compression_state.compression_count
                );

                // Update session state
                let _ = self
                    .session_manager
                    .update_compression_state(session_id, session.compression_state.clone())
                    .await;

                // Calculate duration
                let duration_ms = start_time.elapsed().as_millis() as u64;

                // Recalculate tokens after compression
                let compressed_tokens = Self::estimate_request_tokens_internal(
                    &mut new_messages,
                    tool_definitions.as_deref(),
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
                        summary_source: if compression_result.has_model_summary {
                            "model".to_string()
                        } else {
                            "local_fallback".to_string()
                        },
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
            let duration_ms = start_time.elapsed().as_millis() as u64;
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

        match self
            .context_compressor
            .compress_turns(
                session_id,
                context_window,
                turns.len(),
                turns,
                tail_policy,
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

                let duration_ms = start_time.elapsed().as_millis() as u64;
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
        // Get configuration for whether to support preserving historical thinking content
        let enable_thinking = ai_client.config.enable_thinking_process;
        let support_preserved_thinking = ai_client.config.support_preserved_thinking;
        let context_window = ai_client.config.context_window as usize;

        // 3. Get System Prompt from current Agent
        debug!(
            "Building system prompt from agent: {}, model={}",
            current_agent.name(),
            ai_client.config.model
        );
        let system_prompt = {
            let workspace_str = context
                .workspace
                .as_ref()
                .map(|workspace| workspace.root_path_string());
            let prompt_context = workspace_str.map(|workspace_path| {
                PromptBuilderContext::new(
                    workspace_path,
                    Some(context.session_id.clone()),
                    Some(ai_client.config.model.clone()),
                )
            });
            current_agent
                .get_system_prompt(prompt_context.as_ref())
                .await?
        };
        debug!("System prompt built, length: {} bytes", system_prompt.len());
        let system_prompt_message = Message::system(system_prompt.clone());

        // Add System Prompt to the beginning of message list (only for this execution, not persisted)
        let mut messages = vec![system_prompt_message.clone()];
        messages.extend(initial_messages);

        let mut round_index = 0;
        let mut total_tools = 0;
        let mut last_assistant_message = Message::assistant("".to_string());

        // Save the last token usage statistics
        let mut last_usage: Option<crate::util::types::ai::GeminiUsage> = None;

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

        // 4. Get available tools list (read tool configuration for current mode from global config)
        let allowed_tools = agent_registry
            .get_agent_tools(
                &agent_type,
                context
                    .workspace
                    .as_ref()
                    .map(|workspace| workspace.root_path()),
            )
            .await;
        let enable_tools = context
            .context
            .get("enable_tools")
            .and_then(|v| v.parse::<bool>().ok())
            .unwrap_or(true);
        let (available_tools, tool_definitions) = if enable_tools {
            debug!(
                "Agent tools: agent={}, tool_count={}",
                agent_type,
                allowed_tools.len()
            );
            self.get_available_tools_and_definitions(
                &allowed_tools,
                context.workspace.as_ref(),
                &agent_type,
            )
            .await
        } else {
            (vec![], None)
        };

        let enable_context_compression = session.config.enable_context_compression;
        let compression_threshold = session.config.compression_threshold;
        // Detect whether the primary model supports multimodal image inputs.
        // When false, multimodal user messages are converted to text placeholders before the provider call.
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
            // Check round limit
            if round_index >= self.config.max_rounds {
                warn!(
                    "Reached max rounds limit: {}, stopping execution",
                    self.config.max_rounds
                );
                break;
            }

            MessageHelper::compute_keep_thinking_flags(
                &mut messages,
                enable_thinking,
                support_preserved_thinking,
            );

            // Check and compress before sending AI request
            let current_tokens =
                Self::estimate_request_tokens_internal(&mut messages, tool_definitions.as_deref());
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

            if !should_compress {
                debug!(
                    "No compression needed: session={}, token_usage={:.1}%, threshold={:.1}%",
                    context.session_id,
                    token_usage_ratio * 100.0,
                    compression_threshold * 100.0
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
                    }
                    Ok(None) => {
                        debug!("All turns need to be kept, no compression performed");
                    }
                    Err(e) => {
                        error!(
                            "Round {} compression failed: {}, continuing with uncompressed context",
                            round_index, e
                        );
                    }
                }
            }

            // Create round context
            let mut round_context_vars = execution_context_vars.clone();
            if context.skip_tool_confirmation {
                round_context_vars.insert("skip_tool_confirmation".to_string(), "true".to_string());
            }
            let round_context = RoundContext {
                session_id: context.session_id.clone(),
                subagent_parent_info: context.subagent_parent_info.clone(),
                dialog_turn_id: context.dialog_turn_id.clone(),
                turn_index: context.turn_index,
                round_number: round_index,
                workspace: context.workspace.clone(),
                messages: messages.clone(),
                available_tools: available_tools.clone(),
                model_name: ai_client.config.model.clone(),
                agent_type: agent_type.clone(),
                context_vars: round_context_vars,
                cancellation_token: CancellationToken::new(),
                workspace_services: context.workspace_services.clone(),
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
            last_assistant_message = round_result.assistant_message.clone();

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

            // If no more rounds, dialog turn ends
            if !round_result.has_more_rounds {
                debug!(
                    "Model round {} ended, reason: {:?}",
                    round_index, round_result.finish_reason
                );
                break;
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
                    break;
                }
            }

            // Check if cancelled after each round
            let dialog_turn_cancelled =
                !self.round_executor.has_active_dialog_turn(&dialog_turn_id);
            if dialog_turn_cancelled {
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

        let duration_ms = start_time.elapsed().as_millis() as u64;

        info!(
            "Dialog turn loop completed: turn={}, rounds={}, total_tools={}",
            context.dialog_turn_id,
            round_index + 1,
            total_tools
        );

        // Emit dialog turn completed event
        debug!("Preparing to send DialogTurnCompleted event");

        let _ = self
            .event_queue
            .enqueue(
                AgenticEvent::DialogTurnCompleted {
                    session_id: context.session_id.clone(),
                    turn_id: context.dialog_turn_id.clone(),
                    total_rounds: round_index + 1,
                    total_tools,
                    duration_ms,
                    subagent_parent_info: event_subagent_parent_info,
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
                round_index + 1,
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
            final_message: last_assistant_message,
            total_rounds: round_index + 1,
            success: true,
            new_messages,
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

    /// Get available tool names and definitions: 1. Tool itself is enabled 2. Allowed in mode or is MCP tool
    async fn get_available_tools_and_definitions(
        &self,
        mode_allowed_tools: &[String],
        workspace: Option<&crate::agentic::WorkspaceBinding>,
        agent_type: &str,
    ) -> (Vec<String>, Option<Vec<ToolDefinition>>) {
        // Use get_all_registered_tools to get all tools including MCP tools
        let all_tools = get_all_registered_tools().await;

        // Filter tools: 1) Check if enabled 2) Check if mode allows
        let mut tool_definitions = Vec::new();
        let description_context = crate::agentic::tools::framework::ToolUseContext {
            tool_call_id: None,
            message_id: None,
            agent_type: Some(agent_type.to_string()),
            session_id: None,
            dialog_turn_id: None,
            workspace: workspace.cloned(),
            safe_mode: None,
            abort_controller: None,
            read_file_timestamps: Default::default(),
            options: None,
            response_state: None,
            image_context_provider: None,
            computer_use_host: None,
            subagent_parent_info: None,
            cancellation_token: None,
            workspace_services: None,
        };
        for tool in &all_tools {
            if !tool.is_enabled().await {
                continue;
            }

            let tool_name = tool.name().to_string();
            // MCP tools are automatically allowed (all tools starting with mcp_)
            if mode_allowed_tools.contains(&tool_name) || tool_name.starts_with("mcp_") {
                let description = tool
                    .description_with_context(Some(&description_context))
                    .await
                    .unwrap_or_else(|_| format!("Tool: {}", tool.name()));

                tool_definitions.push(ToolDefinition {
                    name: tool.name().to_string(),
                    description,
                    parameters: tool.input_schema(),
                });
            }
        }

        // Order tools for the model API: terminal → file-ish tools → **`ComputerUse`** (locate /
        // screenshot / keys) **before** split mouse tools so the list matches “sense then act”.
        let tool_ordering: HashMap<String, usize> = [
            ("Task", 1),
            ("Bash", 2),
            ("TerminalControl", 3),
            ("Glob", 4),
            ("Grep", 5),
            ("Read", 6),
            ("Edit", 7),
            ("Write", 8),
            ("Delete", 9),
            ("WebFetch", 10),
            ("WebSearch", 11),
            ("TodoWrite", 12),
            ("Skill", 13),
            ("Log", 14),
            ("MermaidInteractive", 15),
            ("ComputerUse", 16),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_string(), v))
        .collect();
        tool_definitions.sort_by_key(|tool| tool_ordering.get(&tool.name).unwrap_or(&100));

        let enabled_tool_names: Vec<String> =
            tool_definitions.iter().map(|d| d.name.clone()).collect();

        (enabled_tool_names, Some(tool_definitions))
    }

    /// Emit event
    async fn emit_event(&self, event: AgenticEvent, priority: EventPriority) {
        let _ = self.event_queue.enqueue(event, Some(priority)).await;
    }
}

#[cfg(test)]
mod tests {
    use super::ExecutionEngine;
    use crate::service::config::types::AIConfig;
    use crate::service::config::types::AIModelConfig;

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
}
