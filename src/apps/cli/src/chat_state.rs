/// Chat state module
///
/// Pure UI rendering state for the chat interface.
/// All session lifecycle and persistence is handled by bitfun-core.
/// This module only maintains transient state needed for TUI rendering.

use std::time::SystemTime;
use std::collections::HashMap;

use bitfun_core::agentic::core::message::{
    Message as CoreMessage, MessageContent, MessageRole as CoreMessageRole,
};
use bitfun_core::agentic::core::strip_prompt_markup;
use bitfun_events::ToolEventData;

use crate::ui::permission::PermissionPrompt;
use crate::ui::question::QuestionPrompt;

// ============ Display Status Types ============

/// Tool display status (for UI rendering)
#[derive(Debug, Clone, PartialEq)]
pub enum ToolDisplayStatus {
    EarlyDetected,
    ParamsPartial,
    Queued,
    Waiting,
    ConfirmationNeeded,
    Confirmed,
    Rejected,
    Pending,
    Running,
    Streaming,
    Success,
    Failed,
    Cancelled,
}

impl ToolDisplayStatus {
    /// Returns true if the tool has entered an active execution phase
    /// (Running, Streaming, or any terminal state). Early pipeline stages
    /// (ParamsPartial, Queued, Waiting) should not overwrite these states,
    /// since priority queue ordering can cause late-arriving low-priority
    /// events to arrive after high-priority state transitions.
    pub fn is_execution_phase(&self) -> bool {
        matches!(
            self,
            ToolDisplayStatus::Running
                | ToolDisplayStatus::Streaming
                | ToolDisplayStatus::Success
                | ToolDisplayStatus::Failed
                | ToolDisplayStatus::Cancelled
                | ToolDisplayStatus::Rejected
        )
    }
}

/// Message role for display
#[derive(Debug, Clone, PartialEq)]
pub enum MessageRole {
    User,
    Assistant,
    System,
    Tool,
}

impl From<&CoreMessageRole> for MessageRole {
    fn from(role: &CoreMessageRole) -> Self {
        match role {
            CoreMessageRole::User => MessageRole::User,
            CoreMessageRole::Assistant => MessageRole::Assistant,
            CoreMessageRole::System => MessageRole::System,
            CoreMessageRole::Tool => MessageRole::Tool,
        }
    }
}

fn display_text_for_role(role: &MessageRole, text: &str) -> String {
    if *role == MessageRole::User {
        strip_prompt_markup(text)
    } else {
        text.to_string()
    }
}

// ============ UI Display Types ============

/// Subagent progress tracking (for Task tool real-time display)
#[derive(Debug, Clone, Default)]
pub struct SubagentProgress {
    /// Total tool calls made by the subagent so far
    pub tool_count: usize,
    /// Name of the currently executing tool in the subagent (if any)
    pub current_tool_name: Option<String>,
    /// Summary/title of the current tool (e.g. file path, command)
    pub current_tool_title: Option<String>,
}

/// Tool call display state (for rendering tool cards)
#[derive(Debug, Clone)]
pub struct ToolDisplayState {
    pub tool_id: String,
    pub tool_name: String,
    pub parameters: serde_json::Value,
    pub status: ToolDisplayStatus,
    pub result: Option<String>,
    pub progress_message: Option<String>,
    pub duration_ms: Option<u64>,
    /// Optional metadata for richer display (e.g. full diff patch, diagnostics)
    pub metadata: Option<serde_json::Value>,
    /// Subagent progress (only for Task tools)
    pub subagent_progress: Option<SubagentProgress>,
}

/// A single content block in a message (text, thinking, or tool call)
#[derive(Debug, Clone)]
pub enum FlowItem {
    /// Text content block
    Text {
        content: String,
        is_streaming: bool,
    },
    /// AI thinking/reasoning block
    Thinking {
        content: String,
    },
    /// Tool call block
    Tool {
        tool_state: ToolDisplayState,
    },
}

/// A chat message for UI rendering (converted from core Message + streaming state)
#[derive(Debug, Clone)]
pub struct ChatMessage {
    pub id: String,
    pub role: MessageRole,
    pub timestamp: SystemTime,
    pub flow_items: Vec<FlowItem>,
    pub is_streaming: bool,
    /// Monotonically increasing version number; incremented on every content change.
    /// Used by render cache to detect stale entries without deep comparison.
    pub version: u64,
}

impl ChatMessage {
    /// Convert a core Message to a UI ChatMessage
    pub fn from_core_message(msg: &CoreMessage) -> Self {
        let role = MessageRole::from(&msg.role);
        let mut flow_items = Vec::new();

        match &msg.content {
            MessageContent::Text(text) => {
                if !text.is_empty() {
                    flow_items.push(FlowItem::Text {
                        content: display_text_for_role(&role, text),
                        is_streaming: false,
                    });
                }
            }
            MessageContent::Mixed {
                reasoning_content,
                text,
                tool_calls,
            } => {
                // Add reasoning/thinking block if present
                if let Some(reasoning) = reasoning_content {
                    if !reasoning.is_empty() {
                        flow_items.push(FlowItem::Thinking {
                            content: reasoning.clone(),
                        });
                    }
                }

                // Add text block if present
                if !text.is_empty() {
                    flow_items.push(FlowItem::Text {
                        content: display_text_for_role(&role, text),
                        is_streaming: false,
                    });
                }

                // Add tool call blocks
                for tc in tool_calls {
                    flow_items.push(FlowItem::Tool {
                        tool_state: ToolDisplayState {
                            tool_id: tc.tool_id.clone(),
                            tool_name: tc.tool_name.clone(),
                            parameters: tc.arguments.clone(),
                            status: ToolDisplayStatus::Success, // Historical messages are completed
                            result: None,
                            progress_message: None,
                            duration_ms: None,
                            metadata: None,
                            subagent_progress: None,
                        },
                    });
                }
            }
            MessageContent::Multimodal { text, .. } => {
                if !text.is_empty() {
                    flow_items.push(FlowItem::Text {
                        content: display_text_for_role(&role, text),
                        is_streaming: false,
                    });
                }
            }
            MessageContent::ToolResult {
                tool_id,
                tool_name,
                result,
                is_error,
                ..
            } => {
                let result_str = extract_fallback_summary(result);
                flow_items.push(FlowItem::Tool {
                    tool_state: ToolDisplayState {
                        tool_id: tool_id.clone(),
                        tool_name: tool_name.clone(),
                        parameters: serde_json::Value::Null,
                        status: if *is_error {
                            ToolDisplayStatus::Failed
                        } else {
                            ToolDisplayStatus::Success
                        },
                        result: Some(result_str),
                        progress_message: None,
                        subagent_progress: None,
                        duration_ms: None,
                        metadata: Some(result.clone()),
                    },
                });
            }
        }

        Self {
            id: msg.id.clone(),
            role,
            timestamp: msg.timestamp,
            flow_items,
            is_streaming: false,
            version: 0,
        }
    }
}

// ============ Chat Metadata ============

/// Statistics for the current chat session
#[derive(Debug, Clone, Default)]
pub struct ChatMetadata {
    pub message_count: usize,
    pub tool_calls: usize,
    pub total_rounds: usize,
    pub total_tokens: usize,
}

// ============ ChatState ============

/// Complete UI state for the chat interface.
/// This is the single source of truth for rendering — but NOT for persistence.
/// All persistence is handled by bitfun-core's SessionManager.
pub struct ChatState {
    /// Core session ID (the real session managed by core)
    pub core_session_id: String,
    /// Session display name
    pub session_name: String,
    /// Agent type
    pub agent_type: String,
    /// Workspace path
    pub workspace: Option<String>,
    /// Current model display name (shown in shortcuts bar)
    pub current_model_name: String,
    /// Messages for UI rendering
    pub messages: Vec<ChatMessage>,
    /// Session statistics
    pub metadata: ChatMetadata,

    // -- Streaming state (transient, not persisted) --

    /// Current turn ID being processed
    current_turn_id: Option<String>,
    /// Ordered flow items for the current streaming message.
    /// Text, thinking, and tool blocks are interleaved in chronological order,
    /// matching the actual conversation flow (inspired by opencode's Part model).
    current_flow_items: Vec<FlowItem>,
    /// Index from tool_id to position in current_flow_items (for fast in-place updates)
    tool_index: HashMap<String, usize>,
    /// Whether the assistant is currently processing
    pub is_processing: bool,

    // -- Permission state --

    /// Current pending permission prompt (if a tool needs user confirmation)
    pub permission_prompt: Option<PermissionPrompt>,

    // -- Question state --

    /// Current pending question prompt (if AskUserQuestion tool is waiting for answers)
    pub question_prompt: Option<QuestionPrompt>,
}

impl ChatState {
    /// Create a new ChatState for a fresh session
    pub fn new(
        core_session_id: String,
        session_name: String,
        agent_type: String,
        workspace: Option<String>,
    ) -> Self {
        Self {
            core_session_id,
            session_name,
            agent_type,
            workspace,
            current_model_name: String::new(),
            messages: Vec::new(),
            metadata: ChatMetadata::default(),
            current_turn_id: None,
            current_flow_items: Vec::new(),
            tool_index: HashMap::new(),
            is_processing: false,
            permission_prompt: None,
            question_prompt: None,
        }
    }

    /// Load historical messages from core and create ChatState.
    ///
    /// Tool results (ToolResult messages) are merged back into the corresponding
    /// tool calls (in Mixed messages) so that tool cards render with full result data.
    pub fn from_core_messages(
        core_session_id: String,
        session_name: String,
        agent_type: String,
        workspace: Option<String>,
        core_messages: &[CoreMessage],
    ) -> Self {
        // Step 1: Build tool_id -> (result_summary, metadata, is_error) lookup from ToolResult messages
        let mut tool_results: HashMap<String, (String, Option<serde_json::Value>, bool)> =
            HashMap::new();
        for msg in core_messages {
            if let MessageContent::ToolResult {
                tool_id,
                result,
                is_error,
                ..
            } = &msg.content
            {
                let result_str = extract_fallback_summary(result);
                tool_results.insert(
                    tool_id.clone(),
                    (result_str, Some(result.clone()), *is_error),
                );
            }
        }

        // Step 2: Convert messages, merging tool results into tool call display states
        let messages: Vec<ChatMessage> = core_messages
            .iter()
            .filter(|msg| {
                // Skip tool result messages (merged into tool cards above)
                !matches!(msg.role, CoreMessageRole::Tool)
                // Skip system messages (internal)
                && !matches!(msg.role, CoreMessageRole::System)
            })
            .map(|msg| {
                let mut chat_msg = ChatMessage::from_core_message(msg);
                // Merge tool results into corresponding tool display states
                for item in &mut chat_msg.flow_items {
                    if let FlowItem::Tool { tool_state } = item {
                        if let Some((result_str, metadata, is_error)) =
                            tool_results.get(&tool_state.tool_id)
                        {
                            tool_state.result = Some(result_str.clone());
                            tool_state.metadata = metadata.clone();
                            if *is_error {
                                tool_state.status = ToolDisplayStatus::Failed;
                            }
                        }
                    }
                }
                chat_msg
            })
            .collect();

        let tool_count = tool_results.len();

        let mut state = Self::new(core_session_id, session_name, agent_type, workspace);
        state.metadata.message_count = messages.len();
        state.metadata.tool_calls = tool_count;
        state.messages = messages;
        state
    }

    // ============ Event Handlers ============

    /// Handle the start of a new dialog turn
    pub fn handle_turn_started(&mut self, turn_id: &str, user_input: &str) {
        self.current_turn_id = Some(turn_id.to_string());
        self.current_flow_items.clear();
        self.tool_index.clear();
        self.is_processing = true;
        let user_display_input = strip_prompt_markup(user_input);

        // Add user message
        self.messages.push(ChatMessage {
            id: uuid::Uuid::new_v4().to_string(),
            role: MessageRole::User,
            timestamp: SystemTime::now(),
            flow_items: vec![FlowItem::Text {
                content: user_display_input,
                is_streaming: false,
            }],
            is_streaming: false,
            version: 0,
        });
        self.metadata.message_count += 1;

        // Add empty assistant message (will be filled by streaming)
        self.messages.push(ChatMessage {
            id: uuid::Uuid::new_v4().to_string(),
            role: MessageRole::Assistant,
            timestamp: SystemTime::now(),
            flow_items: Vec::new(),
            is_streaming: true,
            version: 0,
        });
    }

    /// Handle a text chunk from the AI.
    /// Appends to the last Text flow item if it exists, otherwise creates a new one.
    /// This ensures text and tool blocks remain interleaved in chronological order.
    pub fn handle_text_chunk(&mut self, text: &str) {
        // Try to append to the last flow item if it's a Text block
        if let Some(FlowItem::Text { content, .. }) = self.current_flow_items.last_mut() {
            content.push_str(text);
        } else {
            // Last item is not Text (it's a Tool, Thinking, or empty) — create a new Text block
            self.current_flow_items.push(FlowItem::Text {
                content: text.to_string(),
                is_streaming: true,
            });
        }
        self.rebuild_streaming_message();
    }

    /// Handle a thinking/reasoning chunk from the AI.
    /// Thinking blocks typically appear at the start, before text/tool content.
    /// Appends to the last Thinking flow item if it exists, otherwise creates a new one.
    pub fn handle_thinking_chunk(&mut self, content: &str) {
        // Try to append to the last Thinking block
        // (Thinking usually comes before text, so check the last item)
        let appended = if let Some(FlowItem::Thinking { content: existing }) = self.current_flow_items.last_mut() {
            existing.push_str(content);
            true
        } else {
            false
        };

        if !appended {
            // Also check if there's a Thinking block earlier that we should append to
            // (e.g., if a Text block was inserted after Thinking but more thinking arrives)
            // For simplicity, just create a new Thinking block — this is rare in practice
            self.current_flow_items.push(FlowItem::Thinking {
                content: content.to_string(),
            });
        }
        self.rebuild_streaming_message();
    }

    /// Handle a tool event.
    /// New tools are appended to current_flow_items in chronological order.
    /// Existing tools are updated in-place via tool_index for O(1) lookup.
    pub fn handle_tool_event(&mut self, tool_event: &ToolEventData) {
        match tool_event {
            ToolEventData::EarlyDetected { tool_id, tool_name } => {
                self.insert_or_update_tool(tool_id, |_existing| {
                    // Should not exist yet, but handle gracefully
                }, || ToolDisplayState {
                    tool_id: tool_id.clone(),
                    tool_name: tool_name.clone(),
                    parameters: serde_json::Value::Null,
                    status: ToolDisplayStatus::EarlyDetected,
                    result: None,
                    progress_message: None,
                    duration_ms: None,
                    metadata: None,
                    subagent_progress: None,
                });
                self.rebuild_streaming_message();
            }

            ToolEventData::ParamsPartial {
                tool_id, params, ..
            } => {
                self.update_tool(tool_id, |tool| {
                    // Only update status if not yet in an advanced execution state.
                    // Due to priority queue ordering, ParamsPartial (Normal priority) may
                    // arrive after Started (High priority), which would incorrectly
                    // revert the status from Running back to ParamsPartial.
                    if !tool.status.is_execution_phase() {
                        tool.status = ToolDisplayStatus::ParamsPartial;
                    }
                    tool.progress_message = Some(params.clone());
                });
                self.rebuild_streaming_message();
            }

            ToolEventData::Queued {
                tool_id, position, ..
            } => {
                self.update_tool(tool_id, |tool| {
                    if !tool.status.is_execution_phase() {
                        tool.status = ToolDisplayStatus::Queued;
                    }
                    tool.progress_message = Some(format!("Queue position: {}", position));
                });
                self.rebuild_streaming_message();
            }

            ToolEventData::Waiting {
                tool_id,
                dependencies,
                ..
            } => {
                self.update_tool(tool_id, |tool| {
                    if !tool.status.is_execution_phase() {
                        tool.status = ToolDisplayStatus::Waiting;
                    }
                    tool.progress_message =
                        Some(format!("Waiting for: {:?}", dependencies));
                });
                self.rebuild_streaming_message();
            }

            ToolEventData::Started {
                tool_id,
                tool_name,
                params,
                timeout_seconds: _,
            } => {
                let params_for_update = params.clone();
                let params_for_create = params.clone();
                let tool_name_clone = tool_name.clone();
                self.insert_or_update_tool(tool_id, |tool| {
                    tool.status = ToolDisplayStatus::Running;
                    tool.parameters = params_for_update;
                }, || ToolDisplayState {
                    tool_id: tool_id.clone(),
                    tool_name: tool_name_clone,
                    parameters: params_for_create,
                    status: ToolDisplayStatus::Running,
                    result: None,
                    progress_message: None,
                    duration_ms: None,
                    metadata: None,
                    subagent_progress: None,
                });
                self.metadata.tool_calls += 1;

                // Auto-create question prompt for AskUserQuestion tool
                if tool_name == "AskUserQuestion" {
                    if let Some(prompt) = QuestionPrompt::from_params(
                        tool_id.clone(),
                        params,
                    ) {
                        self.question_prompt = Some(prompt);
                    }
                }

                self.rebuild_streaming_message();
            }

            ToolEventData::Progress {
                tool_id, message, ..
            } => {
                self.update_tool(tool_id, |tool| {
                    tool.progress_message = Some(message.clone());
                });
                self.rebuild_streaming_message();
            }

            ToolEventData::Streaming {
                tool_id,
                chunks_received,
                ..
            } => {
                self.update_tool(tool_id, |tool| {
                    tool.status = ToolDisplayStatus::Streaming;
                    tool.progress_message =
                        Some(format!("Received {} chunks", chunks_received));
                });
                self.rebuild_streaming_message();
            }

            ToolEventData::ConfirmationNeeded { tool_id, tool_name, params } => {
                self.update_tool(tool_id, |tool| {
                    tool.status = ToolDisplayStatus::ConfirmationNeeded;
                    tool.progress_message =
                        Some("Waiting for user confirmation".to_string());
                });
                // Auto-create permission prompt for user interaction
                self.permission_prompt = Some(PermissionPrompt::new(
                    tool_id.clone(),
                    tool_name.clone(),
                    params.clone(),
                ));
                self.rebuild_streaming_message();
            }

            ToolEventData::Confirmed { tool_id, .. } => {
                self.update_tool(tool_id, |tool| {
                    tool.status = ToolDisplayStatus::Confirmed;
                });
                // Clear permission prompt if it matches this tool
                if self.permission_prompt.as_ref().map(|p| &p.tool_id) == Some(tool_id) {
                    self.permission_prompt = None;
                }
                self.rebuild_streaming_message();
            }

            ToolEventData::Rejected { tool_id, .. } => {
                self.update_tool(tool_id, |tool| {
                    tool.status = ToolDisplayStatus::Rejected;
                    tool.result = Some("User rejected execution".to_string());
                });
                // Clear permission prompt if it matches this tool
                if self.permission_prompt.as_ref().map(|p| &p.tool_id) == Some(tool_id) {
                    self.permission_prompt = None;
                }
                self.rebuild_streaming_message();
            }

            ToolEventData::Completed {
                tool_id,
                tool_name,
                result,
                result_for_assistant,
                duration_ms,
                ..
            } => {
                // Prefer result_for_assistant from tool, fallback to extracting from JSON
                let result_str = result_for_assistant.clone()
                    .unwrap_or_else(|| extract_fallback_summary(result));
                let metadata = result.clone();
                let dur = *duration_ms;
                self.update_tool(tool_id, |tool| {
                    let is_hmos_failed = tool_name == "HmosCompilation"
                        && result
                            .get("success")
                            .and_then(|v| v.as_bool())
                            == Some(false);
                    tool.status = if is_hmos_failed {
                        ToolDisplayStatus::Failed
                    } else {
                        ToolDisplayStatus::Success
                    };
                    tool.result = Some(result_str);
                    tool.metadata = Some(metadata);
                    tool.duration_ms = Some(dur);
                });
                // Clear question prompt if this tool completed
                if self.question_prompt.as_ref().map(|p| &p.tool_id) == Some(tool_id) {
                    self.question_prompt = None;
                }
                self.rebuild_streaming_message();
            }

            ToolEventData::Failed {
                tool_id, error, ..
            } => {
                let err = error.clone();
                self.update_tool(tool_id, |tool| {
                    tool.status = ToolDisplayStatus::Failed;
                    tool.result = Some(err);
                });
                // Clear question prompt if this tool failed
                if self.question_prompt.as_ref().map(|p| &p.tool_id) == Some(tool_id) {
                    self.question_prompt = None;
                }
                self.rebuild_streaming_message();
            }

            ToolEventData::Cancelled {
                tool_id, reason, ..
            } => {
                let rsn = reason.clone();
                self.update_tool(tool_id, |tool| {
                    tool.status = ToolDisplayStatus::Cancelled;
                    tool.result = Some(rsn);
                });
                // Clear question prompt if this tool was cancelled
                if self.question_prompt.as_ref().map(|p| &p.tool_id) == Some(tool_id) {
                    self.question_prompt = None;
                }
                self.rebuild_streaming_message();
            }

            // StreamChunk and other variants we don't need to display
            _ => {}
        }
    }

    /// Handle a subagent event by updating the parent Task tool's progress.
    ///
    /// When a subagent emits events (tool started, completed, etc.), we forward
    /// key information to the parent Task tool so the UI can show real-time progress.
    pub fn handle_subagent_event(&mut self, parent_tool_id: &str, event: &bitfun_events::AgenticEvent) {
        use bitfun_events::AgenticEvent;

        match event {
            AgenticEvent::ToolEvent { tool_event, .. } => {
                match tool_event {
                    ToolEventData::Started {
                        tool_name,
                        params,
                        ..
                    } => {
                        let title = extract_tool_title(tool_name, params);
                        self.update_tool(parent_tool_id, |tool| {
                            let progress = tool.subagent_progress.get_or_insert_with(SubagentProgress::default);
                            progress.tool_count += 1;
                            progress.current_tool_name = Some(tool_name.clone());
                            progress.current_tool_title = title;
                        });
                        self.rebuild_streaming_message();
                    }
                    ToolEventData::Completed {
                        tool_name,
                        result_for_assistant,
                        result: _,
                        ..
                    } => {
                        let summary = result_for_assistant.clone()
                            .unwrap_or_else(|| tool_name.clone());
                        self.update_tool(parent_tool_id, |tool| {
                            let progress = tool.subagent_progress.get_or_insert_with(SubagentProgress::default);
                            progress.current_tool_name = Some(tool_name.clone());
                            progress.current_tool_title = Some(summary);
                        });
                        self.rebuild_streaming_message();
                    }
                    ToolEventData::Failed { tool_name, error, .. } => {
                        self.update_tool(parent_tool_id, |tool| {
                            let progress = tool.subagent_progress.get_or_insert_with(SubagentProgress::default);
                            progress.current_tool_name = Some(tool_name.clone());
                            progress.current_tool_title = Some(format!("Error: {}", truncate_string(error, 60)));
                        });
                        self.rebuild_streaming_message();
                    }
                    _ => {}
                }
            }
            AgenticEvent::ModelRoundStarted { round_index, .. } => {
                if *round_index > 0 {
                    self.update_tool(parent_tool_id, |tool| {
                        let progress = tool.subagent_progress.get_or_insert_with(SubagentProgress::default);
                        progress.current_tool_name = None;
                        progress.current_tool_title = Some(format!("Round {}", round_index + 1));
                    });
                    self.rebuild_streaming_message();
                }
            }
            _ => {}
        }
    }

    /// Handle dialog turn completion
    pub fn handle_turn_completed(&mut self, total_rounds: usize, _total_tools: usize) {
        // Finalize the streaming message
        if let Some(last_msg) = self.messages.last_mut() {
            if last_msg.role == MessageRole::Assistant {
                last_msg.is_streaming = false;
                // Mark all text flow items as not streaming
                for item in &mut last_msg.flow_items {
                    if let FlowItem::Text { is_streaming, .. } = item {
                        *is_streaming = false;
                    }
                }
                last_msg.version += 1;
            }
        }

        self.metadata.total_rounds += total_rounds;
        self.current_turn_id = None;
        self.current_flow_items.clear();
        self.tool_index.clear();
        self.is_processing = false;
        self.permission_prompt = None;
        self.question_prompt = None;
    }

    /// Handle dialog turn failure
    pub fn handle_turn_failed(&mut self, error: &str) {
        // Add error to the last assistant message
        if let Some(last_msg) = self.messages.last_mut() {
            if last_msg.role == MessageRole::Assistant {
                last_msg.is_streaming = false;
                last_msg.flow_items.push(FlowItem::Text {
                    content: format!("[Error: {}]", error),
                    is_streaming: false,
                });
                last_msg.version += 1;
            }
        }

        self.current_turn_id = None;
        self.current_flow_items.clear();
        self.tool_index.clear();
        self.is_processing = false;
        self.permission_prompt = None;
        self.question_prompt = None;
    }

    /// Handle dialog turn cancellation
    pub fn handle_turn_cancelled(&mut self) {
        if let Some(last_msg) = self.messages.last_mut() {
            if last_msg.role == MessageRole::Assistant {
                last_msg.is_streaming = false;
                last_msg.flow_items.push(FlowItem::Text {
                    content: "[Cancelled]".to_string(),
                    is_streaming: false,
                });
                last_msg.version += 1;
            }
        }

        self.current_turn_id = None;
        self.current_flow_items.clear();
        self.tool_index.clear();
        self.is_processing = false;
        self.permission_prompt = None;
        self.question_prompt = None;
    }

    /// Handle token usage update
    pub fn handle_token_usage(&mut self, total_tokens: usize) {
        self.metadata.total_tokens = total_tokens;
    }

    /// Add a system message (for commands like /help, /clear, etc.)
    pub fn add_system_message(&mut self, content: String) {
        self.messages.push(ChatMessage {
            id: uuid::Uuid::new_v4().to_string(),
            role: MessageRole::System,
            timestamp: SystemTime::now(),
            flow_items: vec![FlowItem::Text {
                content,
                is_streaming: false,
            }],
            is_streaming: false,
            version: 0,
        });
    }

    /// Clear all messages (for /clear command)
    pub fn clear_messages(&mut self) {
        self.messages.clear();
    }

    /// Get the current turn ID (if processing)
    pub fn current_turn_id(&self) -> Option<&str> {
        self.current_turn_id.as_deref()
    }

    // ============ Internal ============

    /// Rebuild the last assistant message from current streaming state.
    /// Simply clones the chronologically-ordered current_flow_items into the message.
    /// Text, thinking, and tool blocks are already interleaved in the correct order.
    fn rebuild_streaming_message(&mut self) {
        let last_msg = match self.messages.last_mut() {
            Some(msg) if msg.role == MessageRole::Assistant && msg.is_streaming => msg,
            _ => return,
        };

        last_msg.flow_items = self.current_flow_items.clone();
        last_msg.version += 1;
    }

    /// Insert a new tool into current_flow_items (appended at end, preserving chronological order),
    /// or update an existing tool in-place if it already exists.
    fn insert_or_update_tool(
        &mut self,
        tool_id: &str,
        update_fn: impl FnOnce(&mut ToolDisplayState),
        create_fn: impl FnOnce() -> ToolDisplayState,
    ) {
        if let Some(&idx) = self.tool_index.get(tool_id) {
            // Tool already exists — update in-place
            if let Some(FlowItem::Tool { tool_state }) = self.current_flow_items.get_mut(idx) {
                update_fn(tool_state);
            }
        } else {
            // New tool — append to flow items in chronological order
            let new_state = create_fn();
            let idx = self.current_flow_items.len();
            self.current_flow_items.push(FlowItem::Tool {
                tool_state: new_state,
            });
            self.tool_index.insert(tool_id.to_string(), idx);
        }
    }

    /// Update an existing tool in current_flow_items via tool_index.
    /// No-op if the tool_id is not found (defensive).
    fn update_tool(
        &mut self,
        tool_id: &str,
        update_fn: impl FnOnce(&mut ToolDisplayState),
    ) {
        if let Some(&idx) = self.tool_index.get(tool_id) {
            if let Some(FlowItem::Tool { tool_state }) = self.current_flow_items.get_mut(idx) {
                update_fn(tool_state);
            }
        }
    }
}

/// Extract a human-readable summary from a tool result JSON Value.
/// Used as fallback when `display_summary` is not provided (e.g. MCP tools, old data).
fn extract_fallback_summary(result: &serde_json::Value) -> String {
    if let Some(obj) = result.as_object() {
        // Try common text fields first
        for key in &[
            "display_summary",
            "result_for_assistant",
            "output",
            "result",
            "content",
            "message",
        ] {
            if let Some(text) = obj.get(*key).and_then(|v| v.as_str()) {
                if !text.is_empty() && text.len() < 200 {
                    return text.to_string();
                } else if !text.is_empty() {
                    let truncated: String = text.chars().take(200).collect();
                    return format!("{}...", truncated);
                }
            }
        }

        // Try success field
        if let Some(true) = obj.get("success").and_then(|v| v.as_bool()) {
            return "Done".to_string();
        }

        // Try extracting key parameter values
        let priority_keys = ["path", "file_path", "query", "pattern", "command", "url"];
        for key in &priority_keys {
            if let Some(s) = obj.get(*key).and_then(|v| v.as_str()) {
                if !s.is_empty() && s.len() < 100 {
                    return s.to_string();
                }
            }
        }
    }

    // If it's a plain string
    if let Some(text) = result.as_str() {
        if text.len() < 200 {
            return text.to_string();
        }
        let truncated: String = text.chars().take(200).collect();
        return format!("{}...", truncated);
    }

    "Done".to_string()
}

/// Extract a short title from tool parameters for subagent progress display.
/// Returns a concise description like the file path, command, or query.
fn extract_tool_title(tool_name: &str, params: &serde_json::Value) -> Option<String> {
    let obj = params.as_object()?;

    // Tool-specific extraction for common tools
    match tool_name {
        "Read" | "Write" | "Edit" | "Delete" | "GetFileDiff" => {
            obj.get("path").and_then(|v| v.as_str()).map(|s| truncate_string(s, 50))
        }
        "Bash" => {
            obj.get("command").and_then(|v| v.as_str()).map(|s| truncate_string(s, 50))
        }
        "Grep" => {
            obj.get("pattern").and_then(|v| v.as_str()).map(|s| truncate_string(s, 40))
        }
        "Glob" | "LS" => {
            obj.get("glob_pattern")
                .or_else(|| obj.get("target_directory"))
                .and_then(|v| v.as_str())
                .map(|s| truncate_string(s, 50))
        }
        "WebSearch" => {
            obj.get("search_term").and_then(|v| v.as_str()).map(|s| truncate_string(s, 40))
        }
        "WebFetch" => {
            obj.get("url").and_then(|v| v.as_str()).map(|s| truncate_string(s, 50))
        }
        _ => {
            // Generic: try common parameter names
            for key in &["path", "file_path", "command", "query", "pattern", "url", "description"] {
                if let Some(s) = obj.get(*key).and_then(|v| v.as_str()) {
                    if !s.is_empty() {
                        return Some(truncate_string(s, 50));
                    }
                }
            }
            None
        }
    }
}

/// Truncate a string to a maximum number of characters, adding "..." if truncated.
fn truncate_string(s: &str, max_len: usize) -> String {
    if s.chars().count() <= max_len {
        s.to_string()
    } else {
        let truncated: String = s.chars().take(max_len).collect();
        format!("{}...", truncated)
    }
}
