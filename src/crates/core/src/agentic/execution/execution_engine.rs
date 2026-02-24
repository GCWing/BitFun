//! Execution Engine
//!
//! Executes complete dialog turns, managing loops of multiple model rounds

use super::round_executor::RoundExecutor;
use super::types::{ExecutionContext, ExecutionResult, RoundContext};
use crate::agentic::agents::get_agent_registry;
use crate::agentic::core::{Message, MessageHelper};
use crate::agentic::events::{AgenticEvent, EventPriority, EventQueue};
use crate::agentic::session::SessionManager;
use crate::agentic::tools::{get_all_registered_tools, SubagentParentInfo};
use crate::infrastructure::ai::get_global_ai_client_factory;
use crate::infrastructure::get_workspace_path;
use crate::util::errors::{BitFunError, BitFunResult};
use crate::util::token_counter::TokenCounter;
use crate::util::types::Message as AIMessage;
use crate::util::types::ToolDefinition;
use log::{debug, error, info, trace, warn};
use std::collections::HashMap;
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

/// Execution engine
pub struct ExecutionEngine {
    round_executor: Arc<RoundExecutor>,
    event_queue: Arc<EventQueue>,
    session_manager: Arc<SessionManager>,
    config: ExecutionEngineConfig,
}

impl ExecutionEngine {
    pub fn new(
        round_executor: Arc<RoundExecutor>,
        event_queue: Arc<EventQueue>,
        session_manager: Arc<SessionManager>,
        config: ExecutionEngineConfig,
    ) -> Self {
        Self {
            round_executor,
            event_queue,
            session_manager,
            config,
        }
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
    ) -> BitFunResult<Option<(usize, Vec<Message>, Vec<AIMessage>)>> {
        let event_subagent_parent_info = subagent_parent_info.map(|info| info.clone().into());
        let mut session = self
            .session_manager
            .get_session(session_id)
            .ok_or_else(|| BitFunError::NotFound(format!("Session not found: {}", session_id)))?;

        let compression_manager = self.session_manager.get_compression_manager();

        // Record start time
        let start_time = std::time::Instant::now();

        let old_messages_len = messages.len();
        // Preprocess turns
        let (turn_index_to_keep, turns) = compression_manager
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
        match compression_manager
            .compress_turns(session_id, context_window, turn_index_to_keep, turns)
            .await
        {
            Ok(compressed_messages) => {
                let mut new_messages = vec![system_prompt_message];
                new_messages.extend(compressed_messages);
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
                let new_ai_messages: Vec<AIMessage> =
                    MessageHelper::convert_messages(&new_messages);
                let compressed_tokens = TokenCounter::estimate_request_tokens(
                    &new_ai_messages,
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
                        has_summary: true,
                        subagent_parent_info: event_subagent_parent_info.clone(),
                    },
                    EventPriority::Normal,
                )
                .await;

                Ok(Some((compressed_tokens, new_messages, new_ai_messages)))
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
        let current_agent = agent_registry
            .get_agent(&agent_type)
            .ok_or_else(|| BitFunError::NotFound(format!("Agent not found: {}", agent_type)))?;
        info!(
            "Current Agent: {} ({})",
            current_agent.name(),
            current_agent.id()
        );

        // 2. Get System Prompt from current Agent
        debug!(
            "Building system prompt from agent: {}",
            current_agent.name()
        );
        let system_prompt = {
            let workspace_path = get_workspace_path();

            let workspace_str = workspace_path.as_ref().map(|p| p.display().to_string());
            let system_prompt = current_agent
                .get_system_prompt(workspace_str.as_deref())
                .await?;

            system_prompt
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

        // Add detailed logging showing received message history
        debug!(
            "Executing dialog turn: dialog_turn_id={}, mode={}, agent={}, initial_messages={}, messages_len={}",
            dialog_turn_id,
            current_agent.name(),
            context.agent_type,
            initial_count,
            messages.len()
        );
        trace!(
            "Message history details: dialog_turn_id={}, session_id={}, roles={:?}",
            dialog_turn_id,
            context.session_id,
            messages
                .iter()
                .map(|m| format!("{:?}", m.role))
                .collect::<Vec<_>>()
        );

        // 3. Get available tools list (read tool configuration for current mode from global config)
        let allowed_tools = agent_registry.get_agent_tools(&agent_type).await;
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
            self.get_available_tools_and_definitions(&allowed_tools)
                .await
        } else {
            (vec![], None)
        };

        // Get session configuration
        let session = self
            .session_manager
            .get_session(&context.session_id)
            .ok_or_else(|| {
                BitFunError::Session(format!("Session not found: {}", context.session_id))
            })?;
        let enable_context_compression = session.config.enable_context_compression;
        let compression_threshold = session.config.compression_threshold;

        // 4. Get AI client
        // Get model ID from AgentRegistry
        let model_id = agent_registry
            .get_model_id_for_agent(&agent_type)
            .await
            .map_err(|e| BitFunError::AIClient(format!("Failed to get model ID: {}", e)))?;
        info!(
            "Agent using model: agent={}, model_id={}",
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
            let mut ai_messages = MessageHelper::convert_messages(&messages);

            // Check and compress before sending AI request
            let current_tokens =
                TokenCounter::estimate_request_tokens(&ai_messages, tool_definitions.as_deref());
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
                    )
                    .await
                {
                    Ok(Some((compressed_tokens, compressed_messages, compressed_ai_messages))) => {
                        info!(
                            "Round {} compression completed: messages {} -> {}, tokens {} -> {}",
                            round_index,
                            messages.len(),
                            compressed_messages.len(),
                            current_tokens,
                            compressed_tokens,
                        );

                        messages = compressed_messages;
                        ai_messages = compressed_ai_messages;
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
            let round_context = RoundContext {
                session_id: context.session_id.clone(),
                subagent_parent_info: context.subagent_parent_info.clone(),
                dialog_turn_id: context.dialog_turn_id.clone(),
                turn_index: context.turn_index,
                round_number: round_index,
                messages: messages.clone(),
                available_tools: available_tools.clone(),
                model_name: context
                    .context
                    .get("model_name")
                    .cloned()
                    .unwrap_or_else(|| "default".to_string()),
                agent_type: agent_type.clone(),
                context_vars: context.context.clone(),
                cancellation_token: CancellationToken::new(),
            };

            // Execute single model round
            debug!(
                "Starting model round: round_index={}, messages={}",
                round_index,
                messages.len()
            );

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

            // Immediately save assistant message (prevent loss on cancellation)
            if let Err(e) = self
                .session_manager
                .add_message(&context.session_id, round_result.assistant_message.clone())
                .await
            {
                warn!("Failed to save assistant message in real-time: {}", e);
            }

            // Add tool result messages to history
            for tool_result_msg in round_result.tool_result_messages.iter() {
                messages.push(tool_result_msg.clone());

                // Immediately save tool result message
                if let Err(e) = self
                    .session_manager
                    .add_message(&context.session_id, tool_result_msg.clone())
                    .await
                {
                    warn!("Failed to save tool result message in real-time: {}", e);
                }
            }

            debug!(
                "Saved round messages in real-time: round_index={}, assistant + {} tool results",
                round_index,
                round_result.tool_result_messages.len()
            );

            // If no more rounds, dialog turn ends
            if !round_result.has_more_rounds {
                debug!(
                    "Model round {} ended, reason: {:?}",
                    round_index, round_result.finish_reason
                );
                break;
            }

            // Count tools
            total_tools += round_result.tool_calls.len();

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

        self.emit_event(
            AgenticEvent::DialogTurnCompleted {
                session_id: context.session_id.clone(),
                turn_id: context.dialog_turn_id.clone(),
                total_rounds: round_index + 1,
                total_tools,
                duration_ms,
                subagent_parent_info: event_subagent_parent_info,
            },
            EventPriority::High,
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
    ) -> (Vec<String>, Option<Vec<ToolDefinition>>) {
        // Use get_all_registered_tools to get all tools including MCP tools
        let all_tools = get_all_registered_tools().await;

        // Filter tools: 1) Check if enabled 2) Check if mode allows
        let mut enabled_tool_names = Vec::new();
        let mut tool_definitions = Vec::new();
        for tool in &all_tools {
            if !tool.is_enabled().await {
                continue;
            }

            let tool_name = tool.name().to_string();
            // MCP tools are automatically allowed (all tools starting with mcp_)
            if mode_allowed_tools.contains(&tool_name) || tool_name.starts_with("mcp_") {
                enabled_tool_names.push(tool_name);

                let description = tool
                    .description()
                    .await
                    .unwrap_or_else(|_| format!("Tool: {}", tool.name()));

                tool_definitions.push(ToolDefinition {
                    name: tool.name().to_string(),
                    description,
                    parameters: tool.input_schema(),
                });
            }
        }

        let tool_ordering = {
            let ordering = vec![
                "Task",
                "Bash",
                "Glob",
                "Grep",
                "Read",
                "Edit",
                "Write",
                "Delete",
                "WebFetch",
                "WebSearch",
                "TodoWrite",
                "Skill",
                "Log",
                "MermaidInteractive",
                "IdeControl",
            ];
            let num_tools = ordering.len();
            ordering
                .into_iter()
                .map(|s| s.to_string())
                .zip(1..=num_tools)
                .collect::<HashMap<String, usize>>()
        };
        tool_definitions.sort_by_key(|tool| tool_ordering.get(&tool.name).unwrap_or(&100));

        (enabled_tool_names, Some(tool_definitions))
    }

    /// Emit event
    async fn emit_event(&self, event: AgenticEvent, priority: EventPriority) {
        let _ = self.event_queue.enqueue(event, Some(priority)).await;
    }
}
