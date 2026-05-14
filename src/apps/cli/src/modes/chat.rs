/// Chat mode implementation
///
/// Interactive chat mode with TUI interface.
/// Events are consumed directly from core's EventQueue.
use anyhow::Result;
use arboard::Clipboard;
use crossterm::event::{
    Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEventKind,
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use std::io;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use bitfun_events::AgenticEvent;

use crate::agent::{agentic_system::AgenticSystem, core_adapter::CoreAgentAdapter, Agent};
use crate::chat_state::ChatState;
use crate::config::CliConfig;
use crate::ui::agent_selector::AgentItem;
use crate::ui::chat::{ChatView, MouseGestureOutcome};
use crate::ui::command_palette::PaletteAction;
use crate::ui::mcp_add_dialog::McpAddAction;
use crate::ui::mcp_selector::McpItem;
use crate::ui::model_config_form::{ModelFormAction, ModelFormResult};
use crate::ui::model_selector::ModelItem;
use crate::ui::permission::PermissionAction;
use crate::ui::provider_selector::ProviderSelection;
use crate::ui::question::QuestionAction;
use crate::ui::session_selector::{SessionAction, SessionItem};
use crate::ui::skill_selector::SkillItem;
use crate::ui::subagent_selector::SubagentItem;
use crate::ui::theme::{
    builtin_theme_ids, builtin_theme_json, resolve_appearance, resolve_effective_color_scheme,
    Appearance, EffectiveColorScheme, Theme,
};
use crate::ui::theme_selector::ThemeItem;
use crate::ui::{init_terminal, restore_terminal};
use bitfun_core::agentic::agents::{get_agent_registry, AgentInfo};
use bitfun_core::agentic::tools::implementations::skills::registry::SkillRegistry;
use bitfun_core::service::config::GlobalConfigManager;

fn event_subagent_parent_info(
    event: &bitfun_events::AgenticEvent,
) -> Option<&bitfun_events::SubagentParentInfo> {
    use bitfun_events::AgenticEvent;

    match event {
        AgenticEvent::DialogTurnStarted {
            subagent_parent_info,
            ..
        }
        | AgenticEvent::DialogTurnCompleted {
            subagent_parent_info,
            ..
        }
        | AgenticEvent::DialogTurnCancelled {
            subagent_parent_info,
            ..
        }
        | AgenticEvent::DialogTurnFailed {
            subagent_parent_info,
            ..
        }
        | AgenticEvent::ContextCompressionStarted {
            subagent_parent_info,
            ..
        }
        | AgenticEvent::ContextCompressionCompleted {
            subagent_parent_info,
            ..
        }
        | AgenticEvent::ContextCompressionFailed {
            subagent_parent_info,
            ..
        }
        | AgenticEvent::ModelRoundStarted {
            subagent_parent_info,
            ..
        }
        | AgenticEvent::ModelRoundCompleted {
            subagent_parent_info,
            ..
        }
        | AgenticEvent::TextChunk {
            subagent_parent_info,
            ..
        }
        | AgenticEvent::ThinkingChunk {
            subagent_parent_info,
            ..
        }
        | AgenticEvent::ToolEvent {
            subagent_parent_info,
            ..
        }
        | AgenticEvent::DeepReviewQueueStateChanged {
            subagent_parent_info,
            ..
        }
        | AgenticEvent::UserSteeringInjected {
            subagent_parent_info,
            ..
        } => subagent_parent_info.as_ref(),
        _ => None,
    }
}

/// Keyboard shortcuts help text
const KEYBOARD_SHORTCUTS_HELP: &str = "\
Keyboard Shortcuts\n\
─────────────────────────────────\n\
Tab / Shift+Tab   Switch Agent\n\
Ctrl+P            Command Palette\n\
Ctrl+J / Ctrl+K   Prev / Next Tool\n\
Ctrl+O            Expand / Collapse Tool\n\
Ctrl+E            Toggle Browse Mode\n\
↑ / ↓             Input History\n\
PageUp / PageDown Scroll Messages\n\
Ctrl+Home / End   Jump to Top / Bottom\n\
Ctrl+U            Clear Input\n\
Esc               Back / Interrupt\n\
Ctrl+W            Close All Windows\n\
Ctrl+C            Quit";

/// Spinner/UI redraw interval while a turn is processing.
const SPINNER_REDRAW_INTERVAL_MS: u64 = 100;
/// Coalesce rapid resize bursts to reduce flicker during window drag.
const RESIZE_REDRAW_DEBOUNCE_MS: u64 = 75;

/// Chat mode exit reason
#[derive(Debug, Clone, PartialEq)]
pub enum ChatExitReason {
    /// User exits program
    Quit,
    /// Switch to a different session
    SwitchSession(String),
    /// Create a new session
    NewSession,
}

/// Pending MCP operation (deferred to allow a render frame for loading state)
enum PendingMcpOp {
    Toggle(String),
    Add { name: String, config_json: String },
    Delete(String),
}

enum PendingMcpTask {
    Toggle {
        server_id: String,
        handle: tokio::task::JoinHandle<bitfun_core::util::errors::BitFunResult<()>>,
    },
    Add {
        name: String,
        handle: tokio::task::JoinHandle<bitfun_core::util::errors::BitFunResult<()>>,
    },
    Delete {
        server_id: String,
        handle: tokio::task::JoinHandle<bitfun_core::util::errors::BitFunResult<()>>,
    },
}

#[derive(Default)]
struct NonKeyEventOutcome {
    request_redraw: bool,
    resize_seen: bool,
}

pub struct ChatMode {
    config: CliConfig,
    /// Current agent type (e.g. "agentic", "plan", "debug")
    agent_type: String,
    workspace: Option<String>,
    agent: Arc<CoreAgentAdapter>,
    /// If set, restore this existing session instead of creating a new one
    restore_session_id: Option<String>,
    /// If set, send this prompt automatically when the session starts
    initial_prompt: Option<String>,
    /// Pending MCP operation — set in key handler, executed after one render frame
    pending_mcp_op: Option<PendingMcpOp>,
    /// Running MCP tasks (non-blocking, polled in main loop)
    pending_mcp_tasks: Vec<PendingMcpTask>,
}

/// Map agent_type to a display name for status messages
fn agent_display_name(agent_type: &str) -> &'static str {
    match agent_type {
        "agentic" => "Fang",
        _ => "AI Assistant",
    }
}

impl ChatMode {
    pub fn new(
        config: CliConfig,
        agent_type: String,
        workspace: Option<String>,
        agentic_system: &AgenticSystem,
    ) -> Self {
        let agent = Arc::new(CoreAgentAdapter::new(
            agentic_system.coordinator.clone(),
            agentic_system.event_queue.clone(),
            workspace.clone().map(PathBuf::from),
        ));

        Self {
            config,
            agent_type,
            workspace,
            agent,
            restore_session_id: None,
            initial_prompt: None,
            pending_mcp_op: None,
            pending_mcp_tasks: Vec::new(),
        }
    }

    /// Set a session ID to restore (for "Continue Last Session")
    pub fn with_restore_session(mut self, session_id: String) -> Self {
        self.restore_session_id = Some(session_id);
        self
    }

    /// Set an initial prompt to send automatically when the session starts
    pub fn with_initial_prompt(mut self, prompt: String) -> Self {
        self.initial_prompt = Some(prompt);
        self
    }

    /// Check if any popup is currently visible
    fn any_popup_visible(&self, chat_view: &ChatView) -> bool {
        chat_view.command_palette_visible()
            || chat_view.model_selector_visible()
            || chat_view.agent_selector_visible()
            || chat_view.session_selector_visible()
            || chat_view.skill_selector_visible()
            || chat_view.subagent_selector_visible()
            || chat_view.mcp_selector_visible()
            || chat_view.mcp_add_dialog_visible()
            || chat_view.provider_selector_visible()
            || chat_view.model_config_form_visible()
            || chat_view.theme_selector_visible()
            || chat_view.info_popup_visible()
    }

    /// Close all popups and clear the navigation stack
    fn close_all_popups(&self, chat_view: &mut ChatView) {
        // Cancel theme preview if active
        if chat_view.theme_selector_visible() {
            chat_view.cancel_theme_preview();
        }
        chat_view.hide_command_palette();
        chat_view.hide_model_selector();
        chat_view.hide_agent_selector();
        chat_view.hide_session_selector();
        chat_view.hide_skill_selector();
        chat_view.hide_subagent_selector();
        chat_view.hide_mcp_selector();
        chat_view.hide_mcp_add_dialog();
        chat_view.hide_provider_selector();
        chat_view.hide_model_config_form();
        chat_view.hide_theme_selector();
        chat_view.dismiss_info_popup();
        chat_view.popup_stack.clear();
    }

    /// Navigate back to the previous popup in the stack, or close all if at the root
    fn navigate_back(&self, chat_view: &mut ChatView) {
        // Pop the current popup from the stack and hide it
        if let Some(current) = chat_view.popup_stack.pop() {
            // Hide the current popup
            match current {
                crate::ui::chat::PopupType::CommandPalette => chat_view.hide_command_palette(),
                crate::ui::chat::PopupType::ModelSelector => chat_view.hide_model_selector(),
                crate::ui::chat::PopupType::AgentSelector => chat_view.hide_agent_selector(),
                crate::ui::chat::PopupType::SessionSelector => chat_view.hide_session_selector(),
                crate::ui::chat::PopupType::SkillSelector => chat_view.hide_skill_selector(),
                crate::ui::chat::PopupType::SubagentSelector => chat_view.hide_subagent_selector(),
                crate::ui::chat::PopupType::McpSelector => chat_view.hide_mcp_selector(),
                crate::ui::chat::PopupType::McpAddDialog => chat_view.hide_mcp_add_dialog(),
                crate::ui::chat::PopupType::ProviderSelector => chat_view.hide_provider_selector(),
                crate::ui::chat::PopupType::ModelConfigForm => chat_view.hide_model_config_form(),
                crate::ui::chat::PopupType::ThemeSelector => {
                    chat_view.hide_theme_selector();
                    chat_view.cancel_theme_preview();
                }
                crate::ui::chat::PopupType::InfoPopup => chat_view.dismiss_info_popup(),
            }

            // If there's a previous popup in the stack, re-show it
            if let Some(previous) = chat_view.popup_stack.peek() {
                match previous {
                    crate::ui::chat::PopupType::CommandPalette => {
                        chat_view.reshow_command_palette()
                    }
                    crate::ui::chat::PopupType::ModelSelector => chat_view.reshow_model_selector(),
                    crate::ui::chat::PopupType::AgentSelector => chat_view.reshow_agent_selector(),
                    crate::ui::chat::PopupType::SessionSelector => {
                        chat_view.reshow_session_selector()
                    }
                    crate::ui::chat::PopupType::SkillSelector => chat_view.reshow_skill_selector(),
                    crate::ui::chat::PopupType::SubagentSelector => {
                        chat_view.reshow_subagent_selector()
                    }
                    crate::ui::chat::PopupType::McpSelector => chat_view.reshow_mcp_selector(),
                    crate::ui::chat::PopupType::McpAddDialog => chat_view.reshow_mcp_add_dialog(),
                    crate::ui::chat::PopupType::ProviderSelector => {
                        chat_view.reshow_provider_selector()
                    }
                    crate::ui::chat::PopupType::ModelConfigForm => {
                        chat_view.reshow_model_config_form()
                    }
                    crate::ui::chat::PopupType::ThemeSelector => chat_view.reshow_theme_selector(),
                    crate::ui::chat::PopupType::InfoPopup => {}
                }
            }
        }
    }

    pub fn run(
        &mut self,
        existing_terminal: Option<Terminal<CrosstermBackend<io::Stdout>>>,
    ) -> Result<ChatExitReason> {
        tracing::info!("Starting Chat mode, Agent: {}", self.agent_type);
        if let Some(ws) = &self.workspace {
            tracing::info!("Workspace: {}", ws);
        }

        let mut terminal = match existing_terminal {
            Some(t) => t,
            None => init_terminal()?,
        };

        let appearance = resolve_appearance(&self.config.ui.theme);
        let scheme = resolve_effective_color_scheme(&self.config.ui.color_scheme);
        let base_is_light = appearance.is_light();
        let base = match (base_is_light, scheme) {
            (_, EffectiveColorScheme::Monochrome) => Theme::monochrome(),
            (true, EffectiveColorScheme::Ansi16) => Theme::light_ansi16(),
            (true, EffectiveColorScheme::Truecolor) => Theme::light(),
            (false, EffectiveColorScheme::Ansi16) => Theme::dark_ansi16(),
            (false, EffectiveColorScheme::Truecolor) => Theme::dark(),
        };
        let theme = self.resolve_configured_theme(base, appearance, scheme);
        let mut chat_view = ChatView::new(theme);

        // Create or restore core session
        let rt_handle = tokio::runtime::Handle::current();

        let (mut session_id, mut chat_state) = if let Some(ref restore_id) = self.restore_session_id
        {
            // Restore existing session
            tracing::info!("Restoring session: {}", restore_id);
            let agent = self.agent.clone();
            let rid = restore_id.clone();
            let agent_type = self.agent_type.clone();
            let workspace = self.workspace.clone();

            tokio::task::block_in_place(|| {
                rt_handle.block_on(async {
                    // Restore session in core (loads metadata, messages, managers)
                    agent.restore_session(&rid).await?;

                    // Prefer session's stored workspace_path over startup workspace
                    let effective_workspace = agent
                        .coordinator()
                        .get_session_manager()
                        .get_session(&rid)
                        .and_then(|s| s.config.workspace_path.clone())
                        .or(workspace);

                    // Load historical messages for UI display
                    let messages = agent
                        .coordinator()
                        .get_messages(&rid)
                        .await
                        .unwrap_or_default();

                    let state = ChatState::from_core_messages(
                        rid.clone(),
                        format!("Restored Session"),
                        agent_type,
                        effective_workspace,
                        &messages,
                    );

                    tracing::info!(
                        "Session restored: {}, {} messages loaded",
                        rid,
                        messages.len()
                    );

                    Ok::<_, anyhow::Error>((rid, state))
                })
            })?
        } else {
            // Create new session
            let session_id = tokio::task::block_in_place(|| {
                rt_handle.block_on(self.agent.ensure_session(&self.agent_type))
            })?;
            tracing::info!("Core session ready: {}", session_id);

            let state = ChatState::new(
                session_id.clone(),
                format!("CLI Session"),
                self.agent_type.clone(),
                self.workspace.clone(),
            );
            (session_id, state)
        };

        // Keep ChatMode workspace in sync with the session's effective workspace
        self.workspace = chat_state.workspace.clone();

        // Load current model name for display
        self.load_current_model_name(&mut chat_state, &rt_handle);

        if self.agent_type == "HarmonyOSDev" {
            let deveco_home = std::env::var("DEVECO_HOME").ok();
            let missing = deveco_home
                .as_deref()
                .map(|s| s.trim().is_empty())
                .unwrap_or(true);
            if missing {
                chat_state.add_system_message(
                    "HarmonyOSDev tip: HmosCompilation requires DEVECO_HOME (DevEco Studio install path). If compilation fails, set DEVECO_HOME and restart the terminal."
                        .to_string(),
                );
            }
        }

        // Send initial prompt if provided (from startup page input)
        if let Some(prompt) = self.initial_prompt.take() {
            tracing::info!("Sending initial prompt: {}", prompt);
            if prompt.starts_with('/') {
                // Slash commands will be handled in the main loop
                chat_view.text_input.set_text(&prompt);
            } else {
                let display_name = agent_display_name(&self.agent_type);
                chat_view.set_status(Some(format!("{} is thinking...", display_name)));

                let agent = self.agent.clone();
                let agent_type = self.agent_type.clone();
                match tokio::task::block_in_place(|| {
                    rt_handle.block_on(agent.send_message(prompt, &agent_type))
                }) {
                    Ok(turn_id) => {
                        tracing::info!("Started initial turn: {}", turn_id);
                    }
                    Err(e) => {
                        tracing::error!("Failed to send initial prompt: {}", e);
                        chat_view.set_status(Some(format!("Error: {}", e)));
                    }
                }
            }
        }

        let event_queue = self.agent.event_queue().clone();

        let mut exit_reason = ChatExitReason::Quit;
        let mut should_quit = false;
        let mut needs_redraw = true;
        let mut last_spinner_redraw = Instant::now();
        let mut pending_resize_at: Option<Instant> = None;
        let spinner_redraw_interval = Duration::from_millis(SPINNER_REDRAW_INTERVAL_MS);
        let resize_redraw_debounce = Duration::from_millis(RESIZE_REDRAW_DEBOUNCE_MS);

        while !should_quit {
            // Coalesce rapid resize bursts before invalidating caches and redrawing.
            if let Some(last_resize_at) = pending_resize_at {
                if last_resize_at.elapsed() >= resize_redraw_debounce {
                    chat_view.invalidate_lines_cache();
                    needs_redraw = true;
                    pending_resize_at = None;
                }
            }

            // Keep spinner animation smooth without forcing full redraw every loop.
            // Pause spinner updates while resize is still being debounced.
            if pending_resize_at.is_some() {
                last_spinner_redraw = Instant::now();
            } else if chat_state.is_processing {
                if last_spinner_redraw.elapsed() >= spinner_redraw_interval {
                    needs_redraw = true;
                    last_spinner_redraw = Instant::now();
                }
            } else {
                last_spinner_redraw = Instant::now();
            }

            // Poll completion of non-blocking MCP operations before rendering.
            if self.poll_mcp_task_completion(&mut chat_view, &mut chat_state, &rt_handle) {
                needs_redraw = true;
            }

            let mut did_render_this_loop = false;
            if needs_redraw {
                terminal.draw(|frame| {
                    chat_view.render(frame, &chat_state);
                })?;
                needs_redraw = false;
                did_render_this_loop = true;
            }

            // 1.5. Execute pending MCP operations (after render so loading state is visible)
            if let Some(op) = self.pending_mcp_op.take() {
                if !did_render_this_loop {
                    terminal.draw(|frame| {
                        chat_view.render(frame, &chat_state);
                    })?;
                }
                match op {
                    PendingMcpOp::Toggle(server_id) => {
                        self.execute_mcp_toggle(
                            &server_id,
                            &mut chat_view,
                            &mut chat_state,
                            &rt_handle,
                        );
                    }
                    PendingMcpOp::Add { name, config_json } => {
                        self.execute_mcp_add(
                            &name,
                            &config_json,
                            &mut chat_view,
                            &mut chat_state,
                            &rt_handle,
                        );
                    }
                    PendingMcpOp::Delete(server_id) => {
                        self.execute_mcp_delete(
                            &server_id,
                            &mut chat_view,
                            &mut chat_state,
                            &rt_handle,
                        );
                    }
                }
                needs_redraw = true;
            }

            // 2. Process core events (non-blocking)
            let events =
                tokio::task::block_in_place(|| rt_handle.block_on(event_queue.dequeue_batch(20)));
            for envelope in events {
                let event = &envelope.event;

                // Check if this is a subagent event that belongs to our session
                if event.session_id() != Some(&session_id) {
                    // Check if this event was emitted by a subagent whose parent is in our session
                    if let Some(parent_info) = event_subagent_parent_info(event) {
                        if parent_info.session_id == session_id {
                            // Forward subagent event to the parent Task tool for progress display
                            chat_state.handle_subagent_event(&parent_info.tool_call_id, event);
                            chat_view.invalidate_lines_cache();
                            needs_redraw = true;
                        }
                    }
                    continue;
                }

                tracing::debug!("Processing core event: {:?}", event);

                match event {
                    AgenticEvent::DialogTurnStarted {
                        turn_id,
                        user_input,
                        ..
                    } => {
                        chat_state.handle_turn_started(turn_id, user_input);
                        chat_view.invalidate_lines_cache();
                        needs_redraw = true;
                    }

                    AgenticEvent::TextChunk { turn_id, text, .. } => {
                        if chat_state.current_turn_id() == Some(turn_id.as_str()) {
                            chat_state.handle_text_chunk(text);
                            chat_view.invalidate_lines_cache();
                            needs_redraw = true;
                        } else {
                            tracing::debug!(
                                "Ignoring TextChunk for non-active turn: active={:?}, event={}",
                                chat_state.current_turn_id(),
                                turn_id
                            );
                        }
                    }

                    AgenticEvent::ThinkingChunk {
                        turn_id, content, ..
                    } => {
                        if chat_state.current_turn_id() == Some(turn_id.as_str()) {
                            chat_state.handle_thinking_chunk(content);
                            chat_view.invalidate_lines_cache();
                            needs_redraw = true;
                        } else {
                            tracing::debug!(
                                "Ignoring ThinkingChunk for non-active turn: active={:?}, event={}",
                                chat_state.current_turn_id(),
                                turn_id
                            );
                        }
                    }

                    AgenticEvent::ToolEvent {
                        turn_id,
                        tool_event,
                        ..
                    } => {
                        if chat_state.current_turn_id() != Some(turn_id.as_str()) {
                            tracing::debug!(
                                "Ignoring ToolEvent for non-active turn: active={:?}, event={}",
                                chat_state.current_turn_id(),
                                turn_id
                            );
                            continue;
                        }
                        chat_state.handle_tool_event(tool_event);
                        chat_view.invalidate_lines_cache();
                        needs_redraw = true;
                    }

                    AgenticEvent::DialogTurnCompleted {
                        turn_id,
                        total_rounds,
                        total_tools,
                        ..
                    } => {
                        if chat_state.current_turn_id() == Some(turn_id.as_str()) {
                            chat_state.handle_turn_completed(*total_rounds, *total_tools);
                            chat_view.invalidate_lines_cache();
                            chat_view.set_status(None);
                            needs_redraw = true;
                            tracing::info!("Dialog turn completed");
                        } else {
                            tracing::debug!(
                                "Ignoring DialogTurnCompleted for non-active turn: active={:?}, event={}",
                                chat_state.current_turn_id(),
                                turn_id
                            );
                        }
                    }

                    AgenticEvent::DialogTurnFailed { turn_id, error, .. } => {
                        if chat_state.current_turn_id() == Some(turn_id.as_str()) {
                            chat_state.handle_turn_failed(error);
                            chat_view.invalidate_lines_cache();
                            chat_view.set_status(Some(format!("Error: {}", error)));
                            needs_redraw = true;
                            tracing::error!("Dialog turn failed: {}", error);
                        } else {
                            tracing::debug!(
                                "Ignoring DialogTurnFailed for non-active turn: active={:?}, event={}",
                                chat_state.current_turn_id(),
                                turn_id
                            );
                        }
                    }

                    AgenticEvent::DialogTurnCancelled { turn_id, .. } => {
                        let active_turn_id = chat_state.current_turn_id();
                        if active_turn_id.is_none() || active_turn_id == Some(turn_id.as_str()) {
                            chat_state.handle_turn_cancelled();
                            chat_view.invalidate_lines_cache();
                            chat_view.set_status(Some("Cancelled".to_string()));
                            needs_redraw = true;
                            tracing::info!("Dialog turn cancelled");
                        } else {
                            tracing::debug!(
                                "Ignoring DialogTurnCancelled for non-active turn: active={:?}, event={}",
                                chat_state.current_turn_id(),
                                turn_id
                            );
                        }
                    }

                    AgenticEvent::TokenUsageUpdated {
                        turn_id,
                        total_tokens,
                        ..
                    } => {
                        if chat_state.current_turn_id() == Some(turn_id.as_str()) {
                            chat_state.handle_token_usage(*total_tokens);
                            needs_redraw = true;
                        }
                    }

                    AgenticEvent::SystemError { error, .. } => {
                        chat_state.add_system_message(format!("[System error: {}]", error));
                        chat_view.invalidate_lines_cache();
                        chat_view.set_status(Some(format!("System error: {}", error)));
                        needs_redraw = true;
                        tracing::error!("System error: {}", error);
                    }

                    // Other events we don't need to handle in the UI
                    _ => {}
                }
            }

            // 3. Process terminal input
            if crossterm::event::poll(Duration::from_millis(16))? {
                if let Ok(first_event) = crossterm::event::read() {
                    // Batch-collect all immediately available events (paste detection).
                    // On Windows, bracketed paste is broken (crossterm #962) and
                    // pasted text arrives as rapid Key events with Enter mixed in.
                    let mut events = vec![first_event];
                    // Short wait to let rapid paste events arrive in the same batch.
                    // Duration::ZERO would split pastes across loop iterations.
                    while crossterm::event::poll(Duration::from_millis(5))? {
                        if let Ok(ev) = crossterm::event::read() {
                            events.push(ev);
                        } else {
                            break;
                        }
                    }

                    // Detect if this batch looks like a paste: multiple Key events
                    // that include at least one Enter and at least one printable char.
                    let is_paste_batch = if events.len() > 2 {
                        let mut has_enter = false;
                        let mut has_char = false;
                        for ev in &events {
                            if let Event::Key(k) = ev {
                                if k.kind == KeyEventKind::Press || k.kind == KeyEventKind::Repeat {
                                    match k.code {
                                        KeyCode::Enter => has_enter = true,
                                        KeyCode::Char(c) if !c.is_control() => has_char = true,
                                        _ => {}
                                    }
                                }
                            }
                        }
                        has_enter && has_char
                    } else {
                        false
                    };

                    if is_paste_batch {
                        // Treat entire batch as pasted text
                        let mut paste_buf = String::new();
                        let mut non_key_events = Vec::new();
                        for ev in events {
                            match ev {
                                Event::Key(k)
                                    if k.kind == KeyEventKind::Press
                                        || k.kind == KeyEventKind::Repeat =>
                                {
                                    match k.code {
                                        KeyCode::Char(c) => paste_buf.push(c),
                                        KeyCode::Enter => paste_buf.push('\n'),
                                        _ => {}
                                    }
                                }
                                other => non_key_events.push(other),
                            }
                        }
                        if !paste_buf.is_empty() {
                            let normalized = paste_buf.replace("\r\n", "\n").replace('\r', "\n");
                            for c in normalized.chars() {
                                chat_view.handle_char(c);
                            }
                            needs_redraw = true;
                        }
                        // Process any non-key events that were mixed in
                        for ev in non_key_events {
                            let outcome = Self::handle_non_key_event(
                                ev,
                                self,
                                &mut chat_view,
                                &mut chat_state,
                                &mut session_id,
                                &rt_handle,
                                &mut should_quit,
                                &mut exit_reason,
                            )?;
                            if outcome.request_redraw {
                                needs_redraw = true;
                            }
                            if outcome.resize_seen {
                                pending_resize_at = Some(Instant::now());
                            }
                        }
                    } else {
                        // Normal single/few events — process each individually
                        for ev in events {
                            match ev {
                                Event::Key(key) => {
                                    if let Some(reason) = self.handle_key_event(
                                        key,
                                        &mut chat_view,
                                        &mut chat_state,
                                        &rt_handle,
                                    )? {
                                        Self::apply_exit_reason(
                                            reason,
                                            self,
                                            &mut chat_view,
                                            &mut chat_state,
                                            &mut session_id,
                                            &rt_handle,
                                            &mut should_quit,
                                            &mut exit_reason,
                                        );
                                    }
                                    if key.kind == KeyEventKind::Press
                                        || key.kind == KeyEventKind::Repeat
                                    {
                                        needs_redraw = true;
                                    }
                                }
                                other => {
                                    let outcome = Self::handle_non_key_event(
                                        other,
                                        self,
                                        &mut chat_view,
                                        &mut chat_state,
                                        &mut session_id,
                                        &rt_handle,
                                        &mut should_quit,
                                        &mut exit_reason,
                                    )?;
                                    if outcome.request_redraw {
                                        needs_redraw = true;
                                    }
                                    if outcome.resize_seen {
                                        pending_resize_at = Some(Instant::now());
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        restore_terminal(terminal)?;
        tracing::info!("Chat mode exited");

        Ok(exit_reason)
    }

    fn handle_key_event(
        &mut self,
        key: KeyEvent,
        chat_view: &mut ChatView,
        chat_state: &mut ChatState,
        rt_handle: &tokio::runtime::Handle,
    ) -> Result<Option<ChatExitReason>> {
        if key.kind != KeyEventKind::Press && key.kind != KeyEventKind::Repeat {
            return Ok(None);
        }

        // ── Permission prompt intercepts all keys when active ──
        if let Some(ref mut prompt) = chat_state.permission_prompt {
            let action = prompt.handle_key_event(key);
            match action {
                PermissionAction::AllowOnce => {
                    let tool_id = prompt.tool_id.clone();
                    let agent = self.agent.clone();
                    chat_state.permission_prompt = None;
                    tracing::info!("User allowed tool once: {}", tool_id);
                    tokio::task::block_in_place(|| {
                        rt_handle.block_on(async move {
                            if let Err(e) = agent.confirm_tool(&tool_id, None).await {
                                tracing::error!("Failed to confirm tool: {}", e);
                            }
                        })
                    });
                    chat_view.set_status(Some("Tool confirmed".to_string()));
                }
                PermissionAction::AllowAlways => {
                    let tool_id = prompt.tool_id.clone();
                    let agent = self.agent.clone();
                    chat_state.permission_prompt = None;
                    tracing::info!("User allowed tool always: {}", tool_id);
                    tokio::task::block_in_place(|| {
                        rt_handle.block_on(async move {
                            if let Err(e) = agent.confirm_tool(&tool_id, None).await {
                                tracing::error!("Failed to confirm tool: {}", e);
                            }
                            // Skip all future tool confirmations
                            if let Ok(svc) =
                                bitfun_core::service::config::get_global_config_service().await
                            {
                                if let Err(e) =
                                    svc.set_config("ai.skip_tool_confirmation", true).await
                                {
                                    tracing::warn!("Failed to set skip_tool_confirmation: {}", e);
                                }
                            }
                        })
                    });
                    chat_view.set_status(Some("Tool confirmed (always)".to_string()));
                }
                PermissionAction::Reject(reason) => {
                    let tool_id = prompt.tool_id.clone();
                    let agent = self.agent.clone();
                    chat_state.permission_prompt = None;
                    tracing::info!("User rejected tool: {}, reason: {}", tool_id, reason);
                    let reason_clone = reason.clone();
                    tokio::task::block_in_place(|| {
                        rt_handle.block_on(async move {
                            if let Err(e) = agent.reject_tool(&tool_id, reason_clone).await {
                                tracing::error!("Failed to reject tool: {}", e);
                            }
                        })
                    });
                    chat_view.set_status(Some(format!("Tool rejected: {}", reason)));
                }
                PermissionAction::None => {
                    // Permission prompt consumed the key, no further action
                }
            }
            return Ok(None);
        }

        // ── Question prompt intercepts all keys when active ──
        if let Some(ref mut prompt) = chat_state.question_prompt {
            let action = prompt.handle_key_event(key);
            match action {
                QuestionAction::Submit(answers) => {
                    let tool_id = prompt.tool_id.clone();
                    let agent = self.agent.clone();
                    chat_state.question_prompt = None;
                    tracing::info!("User submitted answers for tool: {}", tool_id);
                    tokio::task::block_in_place(|| {
                        rt_handle.block_on(async move {
                            if let Err(e) = agent.submit_user_answers(&tool_id, answers).await {
                                tracing::error!("Failed to submit answers: {}", e);
                            }
                        })
                    });
                    chat_view.set_status(Some("Answers submitted".to_string()));
                }
                QuestionAction::Reject => {
                    let tool_id = prompt.tool_id.clone();
                    chat_state.question_prompt = None;
                    tracing::info!("User dismissed question prompt: {}", tool_id);
                    chat_view.set_status(Some("Question dismissed".to_string()));
                }
                QuestionAction::None => {
                    // Question prompt consumed the key, no further action
                }
            }
            return Ok(None);
        }

        // ── Normal key handling ──

        // Global popup navigation: Ctrl+W closes all popups, Esc navigates back
        if self.any_popup_visible(chat_view) {
            match (key.code, key.modifiers) {
                (KeyCode::Char('w'), KeyModifiers::CONTROL) => {
                    self.close_all_popups(chat_view);
                    return Ok(None);
                }
                (KeyCode::Esc, _) => {
                    self.navigate_back(chat_view);
                    return Ok(None);
                }
                _ => {}
            }
        }

        // Info popup intercepts all keys when visible
        if chat_view.info_popup_visible() {
            chat_view.dismiss_info_popup();
            return Ok(None);
        }

        // Command palette intercepts all keys when visible
        if chat_view.command_palette_visible() {
            let action = chat_view.command_palette_handle_key(key);
            match action {
                PaletteAction::Execute(id) => {
                    return self.handle_palette_action(&id, chat_view, chat_state, rt_handle);
                }
                PaletteAction::Dismiss | PaletteAction::None => {}
            }
            return Ok(None);
        }

        // Handle popup events first (when visible)
        if chat_view.model_selector_visible() {
            match key.code {
                KeyCode::Up => chat_view.model_selector_up(),
                KeyCode::Down => chat_view.model_selector_down(),
                KeyCode::Enter => {
                    if let Some(selected) = chat_view.model_selector_confirm() {
                        chat_view.hide_model_selector();
                        self.apply_model_selection(&selected, chat_view, chat_state, rt_handle);
                    }
                }
                KeyCode::Char('e') => {
                    if let Some(selected) = chat_view.model_selector_confirm() {
                        chat_view.hide_model_selector();
                        self.edit_model(&selected, chat_view, rt_handle);
                    }
                }
                // Note: Esc is handled globally for navigation back
                _ => {}
            }
            return Ok(None);
        }

        if chat_view.theme_selector_visible() {
            match key.code {
                KeyCode::Up => {
                    chat_view.theme_selector_up();
                    if let Some(selected) = chat_view.theme_selector_selected() {
                        self.preview_theme_selection(&selected, chat_view);
                    }
                }
                KeyCode::Down => {
                    chat_view.theme_selector_down();
                    if let Some(selected) = chat_view.theme_selector_selected() {
                        self.preview_theme_selection(&selected, chat_view);
                    }
                }
                KeyCode::Enter => {
                    if let Some(selected) = chat_view.theme_selector_confirm() {
                        chat_view.hide_theme_selector();
                        self.apply_theme_selection(&selected, chat_view);
                        chat_view.commit_theme_preview();
                    }
                }
                // Note: Esc is handled globally for navigation back
                _ => {}
            }
            return Ok(None);
        }

        if chat_view.agent_selector_visible() {
            match key.code {
                KeyCode::Up => chat_view.agent_selector_up(),
                KeyCode::Down => chat_view.agent_selector_down(),
                KeyCode::Enter => {
                    if let Some(selected) = chat_view.agent_selector_confirm() {
                        chat_view.hide_agent_selector();
                        self.apply_agent_selection(&selected, chat_state);
                    }
                }
                // Note: Esc is handled globally for navigation back
                _ => {}
            }
            return Ok(None);
        }

        if chat_view.session_selector_visible() {
            let action = chat_view.session_selector_handle_key(key);
            match action {
                SessionAction::Switch(item) => {
                    return Ok(Some(ChatExitReason::SwitchSession(item.session_id)));
                }
                SessionAction::Delete(item) => {
                    self.handle_session_delete(&item, chat_view, chat_state, rt_handle);
                }
                SessionAction::Close | SessionAction::None => {}
            }
            return Ok(None);
        }

        if chat_view.skill_selector_visible() {
            match key.code {
                KeyCode::Up => chat_view.skill_selector_up(),
                KeyCode::Down => chat_view.skill_selector_down(),
                KeyCode::Enter => {
                    if let Some(selected) = chat_view.skill_selector_confirm() {
                        chat_view.hide_skill_selector();
                        self.apply_skill_selection(&selected, chat_view);
                    }
                }
                // Note: Esc is handled globally for navigation back
                _ => {}
            }
            return Ok(None);
        }

        if chat_view.subagent_selector_visible() {
            match key.code {
                KeyCode::Up => chat_view.subagent_selector_up(),
                KeyCode::Down => chat_view.subagent_selector_down(),
                KeyCode::Enter => {
                    if let Some(selected) = chat_view.subagent_selector_confirm() {
                        chat_view.hide_subagent_selector();
                        self.apply_subagent_selection(&selected, chat_view);
                    }
                }
                // Note: Esc is handled globally for navigation back
                _ => {}
            }
            return Ok(None);
        }

        if chat_view.mcp_selector_visible() {
            match key.code {
                KeyCode::Up => chat_view.mcp_selector_up(),
                KeyCode::Down => chat_view.mcp_selector_down(),
                KeyCode::Enter | KeyCode::Char(' ') => {
                    if let Some(selected) = chat_view.mcp_selector_confirm() {
                        self.toggle_mcp_server(&selected.id, chat_view);
                    }
                }
                KeyCode::Char('a') => {
                    // Open add dialog (hide selector first)
                    chat_view.hide_mcp_selector();
                    chat_view.show_mcp_add_dialog();
                }
                KeyCode::Char('d') => {
                    if let Some(selected) = chat_view.mcp_selector_confirm() {
                        // First press: enter confirm-delete mode
                        // Second press: actually delete (handled by confirm_delete state)
                        if chat_view.mcp_selector_is_confirm_delete(&selected.id) {
                            self.delete_mcp_server(&selected.id, chat_view);
                        } else {
                            chat_view.mcp_selector_start_confirm_delete(selected.id.clone());
                        }
                    }
                }
                KeyCode::Char('e') => {
                    chat_view.hide_mcp_selector();
                    self.open_mcp_config(chat_state);
                }
                // Note: Esc is handled globally for navigation back
                _ => {
                    // Any other key cancels the confirm-delete state
                    chat_view.mcp_selector_cancel_confirm_delete();
                }
            }
            return Ok(None);
        }

        if chat_view.mcp_add_dialog_visible() {
            let action = chat_view.mcp_add_dialog_handle_key(key);
            match action {
                McpAddAction::Confirm { name, config_json } => {
                    self.add_mcp_server(&name, &config_json, chat_view);
                }
                McpAddAction::Cancel => {
                    // Re-open the MCP selector
                    self.show_mcp_selector(chat_view, chat_state, rt_handle);
                }
                McpAddAction::None => {}
            }
            return Ok(None);
        }

        if chat_view.provider_selector_visible() {
            if let Some(selection) = chat_view.provider_selector_handle_key(key) {
                self.handle_provider_selection(selection, chat_view);
            }
            return Ok(None);
        }

        if chat_view.model_config_form_visible() {
            let action = chat_view.model_config_form_handle_key(key);
            match action {
                ModelFormAction::Save(result) => {
                    if result.editing_model_id.is_some() {
                        self.update_existing_model(result, chat_view, chat_state, rt_handle);
                    } else {
                        self.save_new_model(result, chat_view, chat_state, rt_handle);
                    }
                }
                ModelFormAction::Cancel => {
                    chat_view.set_status(Some("Model form cancelled".to_string()));
                }
                ModelFormAction::None => {}
            }
            return Ok(None);
        }

        match (key.code, key.modifiers) {
            // Ctrl+V: read clipboard directly (reliable paste on Windows where
            // bracketed paste is broken — crossterm issue #962)
            (KeyCode::Char('v'), KeyModifiers::CONTROL) => {
                match Clipboard::new().and_then(|mut cb| cb.get_text()) {
                    Ok(text) if !text.is_empty() => {
                        let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
                        for c in normalized.chars() {
                            chat_view.handle_char(c);
                        }
                    }
                    _ => {}
                }
            }

            (KeyCode::Char('c'), KeyModifiers::CONTROL) => {
                // If processing, cancel the current turn instead of quitting
                if chat_state.is_processing {
                    tracing::info!("User requested cancellation");
                    let agent = self.agent.clone();
                    tokio::task::block_in_place(|| {
                        rt_handle.block_on(async move {
                            if let Err(e) = agent.cancel_current_turn().await {
                                tracing::error!("Failed to cancel turn: {}", e);
                            }
                        })
                    });
                    chat_view.set_status(Some("Cancelling...".to_string()));
                    return Ok(None);
                }
                tracing::info!("User requested quit");
                return Ok(Some(ChatExitReason::Quit));
            }

            (KeyCode::Char('p'), KeyModifiers::CONTROL) => {
                chat_view.show_command_palette();
                return Ok(None);
            }

            // Alt+Enter: insert newline in input
            (KeyCode::Enter, m) if m.contains(KeyModifiers::ALT) => {
                chat_view.handle_newline();
            }

            (KeyCode::Enter, _) => {
                if let Some(cmd) = chat_view.apply_command_menu_selection() {
                    let cmd_result = self.handle_command(&cmd, chat_view, chat_state, rt_handle)?;
                    return Ok(cmd_result);
                }

                if chat_state.is_processing {
                    let trimmed = chat_view.input_text().trim();
                    if trimmed.starts_with('/') {
                        if let Some(input) = chat_view.send_input() {
                            let cmd_result =
                                self.handle_command(&input, chat_view, chat_state, rt_handle)?;
                            return Ok(cmd_result);
                        }
                    } else if !trimmed.is_empty() {
                        chat_view.set_status(Some(
                            "Currently processing. Type a /command, or press Ctrl+C to cancel."
                                .to_string(),
                        ));
                    }
                    return Ok(None);
                }

                if let Some(input) = chat_view.send_input() {
                    tracing::info!("User input: {}", input);

                    if input.starts_with('/') {
                        let cmd_result =
                            self.handle_command(&input, chat_view, chat_state, rt_handle)?;
                        return Ok(cmd_result);
                    }

                    // Send message to agent
                    let display_name = agent_display_name(&self.agent_type);
                    chat_view.set_status(Some(format!("{} is thinking...", display_name)));

                    let agent = self.agent.clone();
                    let input_clone = input.clone();
                    let agent_type = self.agent_type.clone();
                    match tokio::task::block_in_place(|| {
                        rt_handle.block_on(agent.send_message(input_clone, &agent_type))
                    }) {
                        Ok(turn_id) => {
                            tracing::info!("Started turn: {}", turn_id);
                        }
                        Err(e) => {
                            tracing::error!("Failed to send message: {}", e);
                            chat_view.set_status(Some(format!("Error: {}", e)));
                        }
                    }
                }
            }

            (KeyCode::Backspace, _) => {
                chat_view.handle_backspace();
            }

            (KeyCode::Left, _) => {
                chat_view.move_cursor_left();
            }
            (KeyCode::Right, _) => {
                chat_view.move_cursor_right();
            }

            // Ctrl+O: toggle expand/collapse on focused block tool
            (KeyCode::Char('o'), KeyModifiers::CONTROL) => {
                chat_view.toggle_focused_tool_expand(chat_state);
            }

            // Ctrl+J: focus previous block tool (up)
            (KeyCode::Char('j'), KeyModifiers::CONTROL) => {
                chat_view.cycle_block_tool_focus_prev(chat_state);
            }

            // Ctrl+K: focus next block tool (down)
            (KeyCode::Char('k'), KeyModifiers::CONTROL) => {
                chat_view.cycle_block_tool_focus_next(chat_state);
            }

            // ↑↓: input history only. Conversation scrolling stays on PageUp/PageDown or mouse.
            (KeyCode::Up, KeyModifiers::NONE) => {
                if chat_view.command_menu_visible() {
                    chat_view.command_menu_up();
                } else {
                    chat_view.history_prev();
                }
            }
            (KeyCode::Down, KeyModifiers::NONE) => {
                if chat_view.command_menu_visible() {
                    chat_view.command_menu_down();
                } else {
                    chat_view.history_next();
                }
            }

            (KeyCode::Home, KeyModifiers::CONTROL) => {
                let total = chat_view.count_message_lines(chat_state);
                chat_view.scroll_to_top(total);
                chat_view.set_status(Some("Jumped to conversation top".to_string()));
            }

            (KeyCode::End, KeyModifiers::CONTROL) => {
                chat_view.scroll_to_bottom();
                chat_view.set_status(Some("Jumped to conversation bottom".to_string()));
            }

            (KeyCode::Home, _) => {
                chat_view.set_cursor_home();
            }

            (KeyCode::End, _) => {
                chat_view.set_cursor_end();
            }

            (KeyCode::Char('u'), KeyModifiers::CONTROL) => {
                chat_view.clear_input();
            }

            (KeyCode::Char('e'), KeyModifiers::CONTROL) => {
                chat_view.toggle_browse_mode();
                let status_msg = if chat_view.browse_mode {
                    "Entered browse mode, use PageUp/PageDown or mouse wheel to scroll conversation"
                } else {
                    "Exited browse mode"
                };
                chat_view.set_status(Some(status_msg.to_string()));
            }

            (KeyCode::PageUp, _) => {
                let total = chat_view.count_message_lines(chat_state);
                chat_view.scroll_up(10, total);
            }

            (KeyCode::PageDown, _) => {
                chat_view.scroll_down(10);
            }

            (KeyCode::Esc, _) => {
                if chat_state.is_processing {
                    tracing::info!("User requested cancellation (Esc)");
                    let agent = self.agent.clone();
                    tokio::task::block_in_place(|| {
                        rt_handle.block_on(async move {
                            if let Err(e) = agent.cancel_current_turn().await {
                                tracing::error!("Failed to cancel turn: {}", e);
                            }
                        })
                    });
                    chat_view.set_status(Some("Cancelling...".to_string()));
                    return Ok(None);
                }
                if chat_view.browse_mode {
                    chat_view.scroll_to_bottom();
                    chat_view.set_status(Some("Exited browse mode".to_string()));
                }
            }

            (KeyCode::Tab, _) => {
                if !chat_state.is_processing {
                    self.cycle_agent(chat_view, chat_state, rt_handle);
                }
            }

            (KeyCode::BackTab, _) => {
                if !chat_state.is_processing {
                    self.cycle_agent_reverse(chat_view, chat_state, rt_handle);
                }
            }

            (KeyCode::Char(c), KeyModifiers::NONE | KeyModifiers::SHIFT) => {
                if !c.is_control() && c != '\u{0}' {
                    chat_view.handle_char(c);
                }
            }

            _ => {}
        }

        Ok(None)
    }

    /// Apply an exit reason from handle_key_event (shared by normal and batch paths).
    fn apply_exit_reason(
        reason: ChatExitReason,
        this: &mut Self,
        chat_view: &mut ChatView,
        chat_state: &mut ChatState,
        session_id: &mut String,
        rt_handle: &tokio::runtime::Handle,
        should_quit: &mut bool,
        exit_reason: &mut ChatExitReason,
    ) {
        match reason {
            ChatExitReason::SwitchSession(new_session_id) => {
                match this.switch_to_session(
                    &new_session_id,
                    session_id,
                    chat_state,
                    chat_view,
                    rt_handle,
                ) {
                    Ok(()) => tracing::info!("Switched to session: {}", new_session_id),
                    Err(e) => {
                        chat_state.add_system_message(format!("Failed to switch session: {}", e));
                        tracing::error!("Failed to switch session: {}", e);
                    }
                }
            }
            ChatExitReason::NewSession => {
                match this.create_new_session(session_id, chat_state, chat_view, rt_handle) {
                    Ok(()) => tracing::info!("Created new session: {}", session_id),
                    Err(e) => {
                        chat_state
                            .add_system_message(format!("Failed to create new session: {}", e));
                        tracing::error!("Failed to create new session: {}", e);
                    }
                }
            }
            other => {
                *should_quit = true;
                *exit_reason = other;
            }
        }
    }

    /// Handle non-key events (Mouse, Paste, Resize, etc.).
    fn handle_non_key_event(
        event: Event,
        this: &mut Self,
        chat_view: &mut ChatView,
        chat_state: &mut ChatState,
        session_id: &mut String,
        rt_handle: &tokio::runtime::Handle,
        should_quit: &mut bool,
        exit_reason: &mut ChatExitReason,
    ) -> Result<NonKeyEventOutcome> {
        let mut outcome = NonKeyEventOutcome::default();
        match event {
            Event::Mouse(mouse) => {
                if chat_view.command_palette_captures_mouse(&mouse) {
                    let action = chat_view.command_palette_handle_mouse(&mouse);
                    match action {
                        PaletteAction::Execute(id) => {
                            if let Some(reason) =
                                this.handle_palette_action(&id, chat_view, chat_state, rt_handle)?
                            {
                                Self::apply_exit_reason(
                                    reason,
                                    this,
                                    chat_view,
                                    chat_state,
                                    session_id,
                                    rt_handle,
                                    should_quit,
                                    exit_reason,
                                );
                            }
                        }
                        PaletteAction::Dismiss | PaletteAction::None => {}
                    }
                } else if chat_view.provider_selector_captures_mouse(&mouse) {
                    if let Some(selection) = chat_view.provider_selector_handle_mouse(&mouse) {
                        this.handle_provider_selection(selection, chat_view);
                    }
                } else if !chat_view.handle_mouse_event(&mouse) {
                    match mouse.kind {
                        MouseEventKind::ScrollUp => {
                            let total = chat_view.count_message_lines(chat_state);
                            chat_view.scroll_up(3, total);
                        }
                        MouseEventKind::ScrollDown => {
                            chat_view.scroll_down(3);
                        }
                        MouseEventKind::Down(MouseButton::Left) => {
                            let _ = chat_view.begin_mouse_selection(mouse.column, mouse.row);
                        }
                        MouseEventKind::Drag(MouseButton::Left) => {
                            let _ = chat_view.update_mouse_selection(mouse.column, mouse.row);
                        }
                        MouseEventKind::Up(MouseButton::Left) => {
                            match chat_view
                                .complete_mouse_selection_or_click(mouse.column, mouse.row)
                            {
                                MouseGestureOutcome::CopyText(text) => {
                                    match Clipboard::new().and_then(|mut cb| cb.set_text(text)) {
                                        Ok(()) => chat_view
                                            .set_status(Some("Copied to clipboard".to_string())),
                                        Err(_) => chat_view.set_status(Some(
                                            "Failed to copy selection".to_string(),
                                        )),
                                    }
                                }
                                MouseGestureOutcome::Click(col, row) => {
                                    chat_view.handle_mouse_click(col, row);
                                }
                                MouseGestureOutcome::None => {}
                            }
                        }
                        MouseEventKind::Moved => {
                            if !chat_view.update_mouse_selection(mouse.column, mouse.row) {
                                chat_view.handle_mouse_move(mouse.column, mouse.row);
                            }
                        }
                        _ => {}
                    }
                }
                if let Some(cmd) = chat_view.take_pending_command() {
                    if let Some(reason) =
                        this.handle_command(&cmd, chat_view, chat_state, rt_handle)?
                    {
                        Self::apply_exit_reason(
                            reason,
                            this,
                            chat_view,
                            chat_state,
                            session_id,
                            rt_handle,
                            should_quit,
                            exit_reason,
                        );
                    }
                }
                if let Some(theme) = chat_view.take_pending_theme_preview() {
                    this.preview_theme_selection(&theme, chat_view);
                }
                if let Some(server_id) = chat_view.take_pending_mcp_toggle() {
                    this.toggle_mcp_server(&server_id, chat_view);
                }
                outcome.request_redraw = true;
            }
            Event::Paste(text) => {
                if chat_view.mcp_add_dialog_visible() {
                    chat_view.mcp_add_dialog_handle_paste(&text);
                } else {
                    let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
                    for c in normalized.chars() {
                        chat_view.handle_char(c);
                    }
                }
                outcome.request_redraw = true;
            }
            Event::Resize(_, _) => {
                outcome.resize_seen = true;
            }
            _ => {}
        }
        Ok(outcome)
    }

    /// Handle command palette action
    fn handle_palette_action(
        &mut self,
        action_id: &str,
        chat_view: &mut ChatView,
        chat_state: &mut ChatState,
        rt_handle: &tokio::runtime::Handle,
    ) -> Result<Option<ChatExitReason>> {
        // Hide command palette but keep it in stack for back navigation
        // (unless the action switches away or exits)
        let keep_in_stack = matches!(action_id, "new_session" | "exit");
        if !keep_in_stack {
            chat_view.hide_command_palette();
        }

        match action_id {
            // Session group
            "new_session" => {
                if chat_state.is_processing {
                    chat_view.set_status(Some(
                        "Cannot start a new session while processing. Press Ctrl+C to cancel first."
                            .to_string(),
                    ));
                    return Ok(None);
                }
                return Ok(Some(ChatExitReason::NewSession));
            }
            "sessions" => {
                if chat_state.is_processing {
                    chat_view.set_status(Some(
                        "Cannot switch sessions while processing. Press Ctrl+C to cancel first."
                            .to_string(),
                    ));
                    return Ok(None);
                }
                self.show_session_selector(chat_view, chat_state, rt_handle);
            }
            // Prompt group
            "skills" => {
                self.show_skill_selector(chat_view, chat_state, rt_handle);
            }
            "subagents" => {
                self.show_subagent_selector(chat_view, chat_state, rt_handle);
            }
            // Models group
            "select_model" => {
                self.show_model_selector(chat_view, chat_state, rt_handle);
            }
            "add_model" => {
                chat_view.show_provider_selector();
            }
            // Agent group
            "switch_agent" => {
                self.show_agent_selector(chat_view, chat_state, rt_handle);
            }
            // MCP group
            "mcp_servers" => {
                self.show_mcp_selector(chat_view, chat_state, rt_handle);
            }
            // System group
            "help" => {
                chat_view.show_info_popup(KEYBOARD_SHORTCUTS_HELP.to_string());
            }
            "exit" => {
                if chat_state.is_processing {
                    tracing::info!("User requested cancellation via palette exit");
                    let agent = self.agent.clone();
                    tokio::task::block_in_place(|| {
                        rt_handle.block_on(async move {
                            if let Err(e) = agent.cancel_current_turn().await {
                                tracing::error!("Failed to cancel turn: {}", e);
                            }
                        })
                    });
                }
                return Ok(Some(ChatExitReason::Quit));
            }
            _ => {
                chat_view.set_status(Some(format!("Unknown palette action: {}", action_id)));
            }
        }
        Ok(None)
    }

    /// Handle shortcut commands
    fn handle_command(
        &mut self,
        command: &str,
        chat_view: &mut ChatView,
        chat_state: &mut ChatState,
        rt_handle: &tokio::runtime::Handle,
    ) -> Result<Option<ChatExitReason>> {
        let parts: Vec<&str> = command.split_whitespace().collect();
        if parts.is_empty() {
            return Ok(None);
        }

        match parts[0] {
            "/help" => {
                chat_view.show_info_popup(KEYBOARD_SHORTCUTS_HELP.to_string());
            }
            "/clear" => {
                if chat_state.is_processing {
                    tracing::info!("User requested cancellation via /clear");
                    let agent = self.agent.clone();
                    tokio::task::block_in_place(|| {
                        rt_handle.block_on(async move {
                            if let Err(e) = agent.cancel_current_turn().await {
                                tracing::error!("Failed to cancel turn: {}", e);
                            }
                        })
                    });
                }
                chat_state.clear_messages();
                chat_view.clear_screen();
                chat_view.set_status(Some("Conversation cleared".to_string()));
            }
            "/agents" => {
                self.show_agent_selector(chat_view, chat_state, rt_handle);
            }
            "/models" => {
                self.show_model_selector(chat_view, chat_state, rt_handle);
            }
            "/theme" => {
                let themes = self.list_available_themes();
                chat_view.begin_theme_preview();
                chat_view.show_theme_selector(themes, Some(self.config.ui.theme_id.clone()));
                chat_view.set_status(Some(
                    "Theme selector: ↑↓ preview, Enter apply, Esc cancel".to_string(),
                ));
            }
            "/connect" => {
                chat_view.show_provider_selector();
            }
            "/new" => {
                if chat_state.is_processing {
                    chat_view.set_status(Some(
                        "Cannot start a new session while processing. Press Ctrl+C to cancel first."
                            .to_string(),
                    ));
                    return Ok(None);
                }
                return Ok(Some(ChatExitReason::NewSession));
            }
            "/sessions" => {
                if chat_state.is_processing {
                    chat_view.set_status(Some(
                        "Cannot switch sessions while processing. Press Ctrl+C to cancel first."
                            .to_string(),
                    ));
                    return Ok(None);
                }
                self.show_session_selector(chat_view, chat_state, rt_handle);
            }
            "/mcps" => {
                self.show_mcp_selector(chat_view, chat_state, rt_handle);
            }
            "/init" => match crate::prompts::get_cli_prompt("init") {
                Some(prompt) => {
                    self.send_message_to_agent(
                        prompt.to_string(),
                        chat_view,
                        chat_state,
                        rt_handle,
                    );
                }
                None => {
                    chat_state.add_system_message(
                        "Init prompt not found. Please create prompts/init.md in the CLI crate."
                            .to_string(),
                    );
                }
            },
            "/skills" => {
                self.show_skill_selector(chat_view, chat_state, rt_handle);
            }
            "/subagents" => {
                self.show_subagent_selector(chat_view, chat_state, rt_handle);
            }
            "/history" => {
                chat_state.add_system_message(format!(
                    "Current session statistics:\n\
                             • Messages: {}\n\
                             • Tool calls: {}\n\
                             • Tokens: {}",
                    chat_state.metadata.message_count,
                    chat_state.metadata.tool_calls,
                    chat_state.metadata.total_tokens
                ));
            }
            "/exit" => {
                if chat_state.is_processing {
                    tracing::info!("User requested cancellation via /exit");
                    let agent = self.agent.clone();
                    tokio::task::block_in_place(|| {
                        rt_handle.block_on(async move {
                            if let Err(e) = agent.cancel_current_turn().await {
                                tracing::error!("Failed to cancel turn: {}", e);
                            }
                        })
                    });
                }
                return Ok(Some(ChatExitReason::Quit));
            }
            _ => {
                chat_state.add_system_message(format!(
                    "Unknown command: {}\nUse /help to see available commands",
                    parts[0]
                ));
            }
        }

        Ok(None)
    }

    fn list_available_themes(&self) -> Vec<ThemeItem> {
        let mut themes = Vec::new();
        for id in builtin_theme_ids() {
            themes.push(ThemeItem { id });
        }

        themes.sort_by(|a, b| a.id.to_ascii_lowercase().cmp(&b.id.to_ascii_lowercase()));
        themes.dedup_by(|a, b| a.id == b.id);
        themes
    }

    fn resolve_configured_theme(
        &self,
        base: Theme,
        appearance: Appearance,
        scheme: EffectiveColorScheme,
    ) -> Theme {
        self.resolve_theme_by_id(base, appearance, scheme, self.config.ui.theme_id.trim())
    }

    fn resolve_theme_by_id(
        &self,
        base: Theme,
        appearance: Appearance,
        scheme: EffectiveColorScheme,
        id: &str,
    ) -> Theme {
        if scheme == EffectiveColorScheme::Monochrome {
            return Theme::monochrome();
        }

        if id.is_empty() {
            return base;
        }

        if let Some(json) = builtin_theme_json(id) {
            return base
                .apply_opencode_theme_json(json, appearance)
                .unwrap_or(base)
                .with_effective_scheme(scheme);
        }

        base
    }

    fn preview_theme_selection(&mut self, theme: &ThemeItem, chat_view: &mut ChatView) {
        let appearance = resolve_appearance(&self.config.ui.theme);
        let scheme = resolve_effective_color_scheme(&self.config.ui.color_scheme);
        let base_is_light = appearance.is_light();
        let base = match (base_is_light, scheme) {
            (_, EffectiveColorScheme::Monochrome) => Theme::monochrome(),
            (true, EffectiveColorScheme::Ansi16) => Theme::light_ansi16(),
            (true, EffectiveColorScheme::Truecolor) => Theme::light(),
            (false, EffectiveColorScheme::Ansi16) => Theme::dark_ansi16(),
            (false, EffectiveColorScheme::Truecolor) => Theme::dark(),
        };

        let resolved = self.resolve_theme_by_id(base, appearance, scheme, theme.id.trim());
        chat_view.set_theme(resolved);
        chat_view.set_status(Some(format!(
            "Preview theme: {} (Enter apply, Esc cancel)",
            theme.id
        )));
    }

    fn apply_theme_selection(&mut self, theme: &ThemeItem, chat_view: &mut ChatView) {
        let appearance = resolve_appearance(&self.config.ui.theme);
        let scheme = resolve_effective_color_scheme(&self.config.ui.color_scheme);
        let base_is_light = appearance.is_light();
        let base = match (base_is_light, scheme) {
            (_, EffectiveColorScheme::Monochrome) => Theme::monochrome(),
            (true, EffectiveColorScheme::Ansi16) => Theme::light_ansi16(),
            (true, EffectiveColorScheme::Truecolor) => Theme::light(),
            (false, EffectiveColorScheme::Ansi16) => Theme::dark_ansi16(),
            (false, EffectiveColorScheme::Truecolor) => Theme::dark(),
        };

        self.config.ui.theme_id = theme.id.clone();
        if let Err(e) = self.config.save() {
            chat_view.set_status(Some(format!("Failed to save config: {}", e)));
        }

        let resolved = self.resolve_theme_by_id(base, appearance, scheme, theme.id.trim());
        chat_view.set_theme(resolved);
        chat_view.set_status(Some(format!("Theme set to: {}", theme.id)));
    }

    fn get_mode_agents(&self, rt_handle: &tokio::runtime::Handle) -> Vec<AgentInfo> {
        let registry = get_agent_registry();
        let modes = tokio::task::block_in_place(|| rt_handle.block_on(registry.get_modes_info()));
        modes
    }

    fn cycle_agent(
        &mut self,
        chat_view: &mut ChatView,
        chat_state: &mut ChatState,
        rt_handle: &tokio::runtime::Handle,
    ) {
        self.switch_agent_by_offset(1, chat_view, chat_state, rt_handle);
    }

    fn cycle_agent_reverse(
        &mut self,
        chat_view: &mut ChatView,
        chat_state: &mut ChatState,
        rt_handle: &tokio::runtime::Handle,
    ) {
        self.switch_agent_by_offset(-1, chat_view, chat_state, rt_handle);
    }

    fn switch_agent_by_offset(
        &mut self,
        offset: isize,
        _chat_view: &mut ChatView,
        chat_state: &mut ChatState,
        rt_handle: &tokio::runtime::Handle,
    ) {
        let modes = self.get_mode_agents(rt_handle);
        if modes.len() <= 1 {
            return;
        }

        let current_idx = modes
            .iter()
            .position(|m| m.id == self.agent_type)
            .unwrap_or(0);

        let len = modes.len() as isize;
        let next_idx = ((current_idx as isize + offset) % len + len) % len;
        let next = &modes[next_idx as usize];

        self.agent_type = next.id.clone();
        chat_state.agent_type = next.id.clone();
    }

    /// Load current model name from global config for display
    fn load_current_model_name(
        &self,
        chat_state: &mut ChatState,
        rt_handle: &tokio::runtime::Handle,
    ) {
        let agent_type = self.agent_type.clone();
        let result: Option<String> = tokio::task::block_in_place(|| {
            rt_handle.block_on(async {
                let config_service = GlobalConfigManager::get_service().await.ok()?;
                let models: Vec<bitfun_core::service::config::AIModelConfig> =
                    config_service.get_ai_models().await.ok()?;
                let global_config: bitfun_core::service::config::GlobalConfig =
                    config_service.get_config(None).await.ok()?;

                // Resolve model ID for the current agent
                let model_id = global_config
                    .ai
                    .agent_models
                    .get(&agent_type)
                    .cloned()
                    .or_else(|| global_config.ai.default_models.primary.clone())
                    .unwrap_or_else(|| "primary".to_string());

                fn provider_display_name(
                    model: &bitfun_core::service::config::AIModelConfig,
                ) -> String {
                    let raw_name = model.name.trim();
                    let model_name = model.model_name.trim();
                    if !raw_name.is_empty() && !model_name.is_empty() {
                        let dashed_suffix = format!(" - {}", model_name);
                        let slash_suffix = format!("/{}", model_name);
                        if let Some(provider) = raw_name.strip_suffix(&dashed_suffix) {
                            return provider.trim().to_string();
                        }
                        if let Some(provider) = raw_name.strip_suffix(&slash_suffix) {
                            return provider.trim().to_string();
                        }
                    }
                    if raw_name.is_empty() {
                        model.provider.clone()
                    } else {
                        raw_name.to_string()
                    }
                }

                fn model_display_name(
                    model: &bitfun_core::service::config::AIModelConfig,
                ) -> String {
                    format!("{} / {}", model.model_name, provider_display_name(model))
                }

                // Find model name
                let model_name = if model_id == "primary" {
                    // Resolve primary model
                    let primary_id = global_config.ai.default_models.primary.as_deref()?;
                    models
                        .iter()
                        .find(|m| m.id == primary_id)
                        .map(model_display_name)
                } else {
                    models
                        .iter()
                        .find(|m| m.id == model_id)
                        .map(model_display_name)
                };

                model_name
            })
        });

        if let Some(name) = result {
            chat_state.current_model_name = name;
        }
    }

    /// Show model selector popup with all available models
    fn show_model_selector(
        &self,
        chat_view: &mut ChatView,
        chat_state: &mut ChatState,
        rt_handle: &tokio::runtime::Handle,
    ) {
        let agent_type = self.agent_type.clone();
        let result = tokio::task::block_in_place(|| {
            rt_handle.block_on(async {
                let config_service = match GlobalConfigManager::get_service().await {
                    Ok(s) => s,
                    Err(e) => {
                        tracing::error!("Failed to get config service: {}", e);
                        return None;
                    }
                };

                let models: Vec<bitfun_core::service::config::AIModelConfig> =
                    config_service.get_ai_models().await.ok()?;
                let global_config: bitfun_core::service::config::GlobalConfig =
                    config_service.get_config(None).await.ok()?;

                // Get current model ID
                let current_model_id = global_config
                    .ai
                    .agent_models
                    .get(&agent_type)
                    .cloned()
                    .or_else(|| global_config.ai.default_models.primary.clone());

                // Convert to ModelItem list (only enabled models)
                let model_items: Vec<ModelItem> = models
                    .into_iter()
                    .filter(|m| m.enabled)
                    .map(|m| ModelItem {
                        id: m.id,
                        name: m.name,
                        provider: m.provider,
                        model_name: m.model_name,
                    })
                    .collect();

                Some((model_items, current_model_id))
            })
        });

        match result {
            Some((models, current_id)) if !models.is_empty() => {
                chat_view.show_model_selector(models, current_id);
            }
            _ => {
                chat_state.add_system_message(
                    "No available models found. Please configure models first.".to_string(),
                );
            }
        }
    }

    /// Apply model selection: update global config and chat state
    fn apply_model_selection(
        &self,
        selected: &ModelItem,
        _chat_view: &mut ChatView,
        chat_state: &mut ChatState,
        rt_handle: &tokio::runtime::Handle,
    ) {
        let selected_id = selected.id.clone();
        let selected_display_name = format!("{} / {}", selected.model_name, selected.name);
        let modes = self.get_mode_agents(rt_handle);

        let success = tokio::task::block_in_place(|| {
            rt_handle.block_on(async {
                let config_service = match GlobalConfigManager::get_service().await {
                    Ok(s) => s,
                    Err(e) => {
                        tracing::error!("Failed to get config service: {}", e);
                        return false;
                    }
                };

                // Update default primary model
                if let Err(e) = config_service
                    .set_config("ai.default_models.primary", &selected_id)
                    .await
                {
                    tracing::error!("Failed to set default primary model: {}", e);
                    return false;
                }

                // Update agent_models for all modes
                for mode in &modes {
                    let path = format!("ai.agent_models.{}", mode.id);
                    if let Err(e) = config_service.set_config(&path, &selected_id).await {
                        tracing::error!("Failed to set model for mode '{}': {}", mode.id, e);
                    }
                }

                true
            })
        });

        if success {
            chat_state.current_model_name = selected_display_name.clone();
            tracing::info!(
                "Model switched to: {} ({})",
                selected_display_name,
                selected_id
            );
        } else {
            tracing::error!(
                "Failed to switch model: {} ({})",
                selected_display_name,
                selected_id
            );
        }
    }

    /// Show agent selector popup with all available agent modes
    fn show_agent_selector(
        &self,
        chat_view: &mut ChatView,
        chat_state: &mut ChatState,
        rt_handle: &tokio::runtime::Handle,
    ) {
        let modes = self.get_mode_agents(rt_handle);
        if modes.is_empty() {
            chat_state.add_system_message("No mode agents available".to_string());
            return;
        }

        let agent_items: Vec<AgentItem> = modes
            .into_iter()
            .map(|m| AgentItem {
                id: m.id,
                description: m.description,
            })
            .collect();

        chat_view.show_agent_selector(agent_items, Some(self.agent_type.clone()));
    }

    /// Apply agent selection: switch agent type
    fn apply_agent_selection(&mut self, selected: &AgentItem, chat_state: &mut ChatState) {
        if selected.id == self.agent_type {
            return;
        }
        self.agent_type = selected.id.clone();
        chat_state.agent_type = selected.id.clone();
        tracing::info!("Switched to agent: {}", selected.id);

        if selected.id == "HarmonyOSDev" {
            let deveco_home = std::env::var("DEVECO_HOME").ok();
            let missing = deveco_home
                .as_deref()
                .map(|s| s.trim().is_empty())
                .unwrap_or(true);
            if missing {
                chat_state.add_system_message(
                    "HarmonyOSDev tip: HmosCompilation requires DEVECO_HOME (DevEco Studio install path). If compilation fails, set DEVECO_HOME and restart the terminal."
                        .to_string(),
                );
            }
        }
    }

    // ============ MCP management ============

    /// Show MCP server selector popup
    fn show_mcp_selector(
        &self,
        chat_view: &mut ChatView,
        _chat_state: &mut ChatState,
        rt_handle: &tokio::runtime::Handle,
    ) {
        let items = self.get_mcp_items(rt_handle);
        // Show even if empty — user can press 'a' to add
        chat_view.show_mcp_selector(items);
    }

    /// Get MCP server items for display
    fn get_mcp_items(&self, rt_handle: &tokio::runtime::Handle) -> Vec<McpItem> {
        let mcp_service = match crate::get_mcp_service() {
            Some(svc) => svc,
            None => return Vec::new(),
        };

        let server_manager = mcp_service.server_manager();
        let config_service = mcp_service.config_service();

        tokio::task::block_in_place(|| {
            rt_handle.block_on(async {
                let configs = match config_service.load_all_configs().await {
                    Ok(c) => c,
                    Err(e) => {
                        tracing::error!("Failed to load MCP configs: {}", e);
                        return Vec::new();
                    }
                };

                let tool_registry =
                    bitfun_core::agentic::tools::registry::get_global_tool_registry();
                let registry_lock = tool_registry.read().await;
                let all_tools = registry_lock.get_all_tools();

                let mut items = Vec::new();
                for config in configs {
                    let status = if !config.enabled {
                        "Stopped".to_string()
                    } else {
                        // Avoid blocking UI while a slow auto-start server holds internal write lock.
                        match tokio::time::timeout(
                            Duration::from_millis(30),
                            server_manager.get_server_status(&config.id),
                        )
                        .await
                        {
                            Ok(Ok(s)) => format!("{:?}", s),
                            Ok(Err(_)) => "Unknown".to_string(),
                            Err(_) => "Starting".to_string(),
                        }
                    };

                    // Count tools from this server
                    let prefix = format!("mcp_{}_", config.id);
                    let tool_count = all_tools
                        .iter()
                        .filter(|t| t.name().starts_with(&prefix))
                        .count();

                    let server_type = format!("{:?}", config.server_type).to_lowercase();

                    items.push(McpItem {
                        id: config.id.clone(),
                        name: config.name.clone(),
                        server_type,
                        status,
                        enabled: config.enabled,
                        tool_count,
                    });
                }
                items
            })
        })
    }

    /// Schedule an MCP server toggle (deferred to allow loading state to render)
    fn toggle_mcp_server(&mut self, server_id: &str, chat_view: &mut ChatView) {
        if self.pending_mcp_op.is_some() || self.is_mcp_server_task_running(server_id) {
            return;
        }

        // Set loading indicator immediately — will be rendered before execution
        chat_view.mcp_selector_set_loading(Some(server_id.to_string()));
        self.pending_mcp_op = Some(PendingMcpOp::Toggle(server_id.to_string()));
    }

    /// Execute MCP server toggle (called from main loop after render)
    fn execute_mcp_toggle(
        &mut self,
        server_id: &str,
        chat_view: &mut ChatView,
        chat_state: &mut ChatState,
        rt_handle: &tokio::runtime::Handle,
    ) {
        let mcp_service = match crate::get_mcp_service() {
            Some(svc) => svc.clone(),
            None => {
                chat_state.add_system_message("MCP service not initialized".to_string());
                chat_view.mcp_selector_set_loading(None);
                return;
            }
        };

        let server_manager = mcp_service.server_manager();
        let task_server_id = server_id.to_string();
        let tracked_server_id = task_server_id.clone();

        let handle = rt_handle.spawn(async move {
            let status = server_manager.get_server_status(&task_server_id).await;
            match status {
                Ok(bitfun_core::service::mcp::MCPServerStatus::Connected)
                | Ok(bitfun_core::service::mcp::MCPServerStatus::Healthy) => {
                    server_manager.stop_server(&task_server_id).await
                }
                _ => server_manager.start_server(&task_server_id).await,
            }
        });

        self.pending_mcp_tasks.push(PendingMcpTask::Toggle {
            server_id: tracked_server_id,
            handle,
        });
    }

    fn is_mcp_server_task_running(&self, server_id: &str) -> bool {
        self.pending_mcp_tasks.iter().any(|task| match task {
            PendingMcpTask::Toggle { server_id: id, .. }
            | PendingMcpTask::Delete { server_id: id, .. } => id == server_id,
            PendingMcpTask::Add { .. } => false,
        })
    }

    fn has_pending_mcp_add_task(&self) -> bool {
        self.pending_mcp_tasks
            .iter()
            .any(|task| matches!(task, PendingMcpTask::Add { .. }))
    }

    fn poll_mcp_task_completion(
        &mut self,
        chat_view: &mut ChatView,
        chat_state: &mut ChatState,
        rt_handle: &tokio::runtime::Handle,
    ) -> bool {
        let mut changed = false;
        let mut i = 0;
        while i < self.pending_mcp_tasks.len() {
            let finished = match &self.pending_mcp_tasks[i] {
                PendingMcpTask::Toggle { handle, .. }
                | PendingMcpTask::Add { handle, .. }
                | PendingMcpTask::Delete { handle, .. } => handle.is_finished(),
            };
            if !finished {
                i += 1;
                continue;
            }

            let task = self.pending_mcp_tasks.swap_remove(i);
            changed = true;
            match task {
                PendingMcpTask::Toggle { server_id, handle } => {
                    let join_result = tokio::task::block_in_place(|| {
                        rt_handle.block_on(async move { handle.await })
                    });

                    match join_result {
                        Ok(Ok(())) => {}
                        Ok(Err(e)) => {
                            tracing::error!("Failed to toggle MCP server {}: {}", server_id, e);
                            chat_state.add_system_message(format!(
                                "Failed to toggle MCP server '{}': {}",
                                server_id, e
                            ));
                        }
                        Err(e) => {
                            tracing::error!("MCP toggle task join error for {}: {}", server_id, e);
                            chat_state.add_system_message(format!(
                                "MCP server '{}' task failed: {}",
                                server_id, e
                            ));
                        }
                    }

                    chat_view.mcp_selector_set_loading(None);
                    let updated_items = self.get_mcp_items(rt_handle);
                    chat_view.mcp_selector_update_items(updated_items);
                }
                PendingMcpTask::Add { name, handle } => {
                    let join_result = tokio::task::block_in_place(|| {
                        rt_handle.block_on(async move { handle.await })
                    });

                    match join_result {
                        Ok(Ok(())) => {
                            chat_state.add_system_message(format!(
                                "MCP server '{}' added and started",
                                name
                            ));
                            self.show_mcp_selector(chat_view, chat_state, rt_handle);
                        }
                        Ok(Err(e)) => {
                            chat_state
                                .add_system_message(format!("Failed to add MCP server: {}", e));
                        }
                        Err(e) => {
                            chat_state.add_system_message(format!(
                                "MCP add task failed for '{}': {}",
                                name, e
                            ));
                        }
                    }
                    chat_view.set_status(None);
                }
                PendingMcpTask::Delete { server_id, handle } => {
                    let join_result = tokio::task::block_in_place(|| {
                        rt_handle.block_on(async move { handle.await })
                    });

                    match join_result {
                        Ok(Ok(())) => {
                            chat_state
                                .add_system_message(format!("MCP server '{}' deleted", server_id));
                        }
                        Ok(Err(e)) => {
                            chat_state
                                .add_system_message(format!("Failed to delete MCP server: {}", e));
                        }
                        Err(e) => {
                            chat_state.add_system_message(format!(
                                "MCP delete task failed for '{}': {}",
                                server_id, e
                            ));
                        }
                    }

                    chat_view.mcp_selector_set_loading(None);
                    let updated_items = self.get_mcp_items(rt_handle);
                    if updated_items.is_empty() {
                        chat_view.hide_mcp_selector();
                    } else {
                        chat_view.mcp_selector_update_items(updated_items);
                    }
                }
            }
        }
        changed
    }

    /// Schedule adding a new MCP server (deferred to allow loading state to render)
    fn add_mcp_server(&mut self, name: &str, config_json_str: &str, chat_view: &mut ChatView) {
        if self.pending_mcp_op.is_some() || self.has_pending_mcp_add_task() {
            return;
        }

        chat_view.set_status(Some(format!("Adding MCP server '{}'...", name)));
        self.pending_mcp_op = Some(PendingMcpOp::Add {
            name: name.to_string(),
            config_json: config_json_str.to_string(),
        });
    }

    /// Execute MCP server add (called from main loop after render)
    fn execute_mcp_add(
        &mut self,
        name: &str,
        config_json_str: &str,
        _chat_view: &mut ChatView,
        chat_state: &mut ChatState,
        rt_handle: &tokio::runtime::Handle,
    ) {
        let mcp_service = match crate::get_mcp_service() {
            Some(svc) => svc.clone(),
            None => {
                chat_state.add_system_message("MCP service not initialized".to_string());
                return;
            }
        };

        let config_value: serde_json::Value = match serde_json::from_str(config_json_str) {
            Ok(v) => v,
            Err(e) => {
                chat_state.add_system_message(format!("Invalid JSON: {}", e));
                _chat_view.set_status(None);
                return;
            }
        };

        let name_owned = name.to_string();
        let task_name = name_owned.clone();
        let handle = rt_handle.spawn(async move {
            let config_obj = config_value.as_object().ok_or_else(|| {
                bitfun_core::util::errors::BitFunError::Validation(
                    "MCP server config must be a JSON object".to_string(),
                )
            })?;

            let server_type = match config_obj.get("type").and_then(|v| v.as_str()) {
                Some("sse") => bitfun_core::service::mcp::MCPServerType::Remote,
                Some("streamable-http") | Some("streamable_http") | Some("http") => {
                    bitfun_core::service::mcp::MCPServerType::Remote
                }
                _ => bitfun_core::service::mcp::MCPServerType::Local,
            };

            let transport = match config_obj.get("type").and_then(|v| v.as_str()) {
                Some("sse") => bitfun_core::service::mcp::MCPServerTransport::Sse,
                Some("streamable-http") | Some("streamable_http") | Some("http") => {
                    bitfun_core::service::mcp::MCPServerTransport::StreamableHttp
                }
                _ => bitfun_core::service::mcp::MCPServerTransport::Stdio,
            };

            let command = config_obj
                .get("command")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let args = config_obj
                .get("args")
                .and_then(|v| v.as_array())
                .map(|values| {
                    values
                        .iter()
                        .filter_map(|v| v.as_str().map(|s| s.to_string()))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            let env = config_obj
                .get("env")
                .and_then(|v| v.as_object())
                .map(|map| {
                    map.iter()
                        .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                        .collect::<std::collections::HashMap<_, _>>()
                })
                .unwrap_or_default();
            let headers = config_obj
                .get("headers")
                .and_then(|v| v.as_object())
                .map(|map| {
                    map.iter()
                        .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                        .collect::<std::collections::HashMap<_, _>>()
                })
                .unwrap_or_default();
            let url = config_obj
                .get("url")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let auto_start = config_obj
                .get("autoStart")
                .or_else(|| config_obj.get("auto_start"))
                .and_then(|v| v.as_bool())
                .unwrap_or(true);
            let enabled = config_obj
                .get("enabled")
                .and_then(|v| v.as_bool())
                .unwrap_or(true);

            let config = bitfun_core::service::mcp::MCPServerConfig {
                id: name_owned.clone(),
                name: name_owned.clone(),
                server_type,
                transport: Some(transport),
                command,
                args,
                env,
                headers,
                url,
                auto_start,
                enabled,
                location: bitfun_core::service::mcp::ConfigLocation::User,
                capabilities: Vec::new(),
                settings: Default::default(),
                oauth: config_obj
                    .get("oauth")
                    .cloned()
                    .and_then(|value| serde_json::from_value(value).ok()),
                xaa: config_obj
                    .get("xaa")
                    .cloned()
                    .and_then(|value| serde_json::from_value(value).ok()),
            };

            mcp_service.server_manager().add_server(config).await?;

            Ok::<(), bitfun_core::util::errors::BitFunError>(())
        });
        self.pending_mcp_tasks.push(PendingMcpTask::Add {
            name: task_name,
            handle,
        });
    }

    /// Schedule deleting an MCP server (deferred to allow loading state to render)
    fn delete_mcp_server(&mut self, server_id: &str, chat_view: &mut ChatView) {
        if self.pending_mcp_op.is_some() || self.is_mcp_server_task_running(server_id) {
            return;
        }

        chat_view.mcp_selector_set_loading(Some(server_id.to_string()));
        chat_view.mcp_selector_cancel_confirm_delete();
        self.pending_mcp_op = Some(PendingMcpOp::Delete(server_id.to_string()));
    }

    /// Execute MCP server delete (called from main loop after render)
    fn execute_mcp_delete(
        &mut self,
        server_id: &str,
        chat_view: &mut ChatView,
        chat_state: &mut ChatState,
        rt_handle: &tokio::runtime::Handle,
    ) {
        let mcp_service = match crate::get_mcp_service() {
            Some(svc) => svc.clone(),
            None => {
                chat_state.add_system_message("MCP service not initialized".to_string());
                chat_view.mcp_selector_set_loading(None);
                return;
            }
        };

        let server_id_owned = server_id.to_string();
        let task_server_id = server_id_owned.clone();
        let handle = rt_handle.spawn(async move {
            // Delete config first so UI can reflect removal immediately even if stop is blocked.
            mcp_service
                .config_service()
                .delete_server_config(&server_id_owned)
                .await?;

            // Best-effort async cleanup: slow startups may hold process write lock for a long time.
            // Retry stop with short timeout, without blocking the delete operation completion.
            let cleanup_service = mcp_service.clone();
            let cleanup_server_id = server_id_owned.clone();
            tokio::spawn(async move {
                for attempt in 1..=20 {
                    let stop_result = tokio::time::timeout(
                        Duration::from_millis(250),
                        cleanup_service
                            .server_manager()
                            .stop_server(&cleanup_server_id),
                    )
                    .await;

                    match stop_result {
                        Ok(Ok(())) => return,
                        Ok(Err(bitfun_core::util::errors::BitFunError::NotFound(_))) => return,
                        Ok(Err(e)) => {
                            tracing::debug!(
                                "Best-effort MCP stop failed: id={} attempt={} error={}",
                                cleanup_server_id,
                                attempt,
                                e
                            );
                        }
                        Err(_) => {
                            tracing::debug!(
                                "Best-effort MCP stop timed out: id={} attempt={}",
                                cleanup_server_id,
                                attempt
                            );
                        }
                    }

                    tokio::time::sleep(Duration::from_millis(250)).await;
                }

                tracing::warn!(
                    "Best-effort MCP stop exhausted retries: id={}",
                    cleanup_server_id
                );
            });

            Ok::<(), bitfun_core::util::errors::BitFunError>(())
        });

        self.pending_mcp_tasks.push(PendingMcpTask::Delete {
            server_id: task_server_id,
            handle,
        });
    }

    /// Open MCP config file in system editor or show its path
    fn open_mcp_config(&self, chat_state: &mut ChatState) {
        match bitfun_core::infrastructure::try_get_path_manager_arc() {
            Ok(path_manager) => {
                let config_file = path_manager.app_config_file();
                chat_state.add_system_message(format!(
                    "MCP servers are configured in:\n  {}\n\n\
                     Edit the \"mcp_servers\" section. Example (Cursor format):\n\
                     {{\n  \"mcp_servers\": {{\n    \"mcpServers\": {{\n      \
                     \"my-server\": {{\n        \"type\": \"stdio\",\n        \
                     \"command\": \"npx\",\n        \"args\": [\"-y\", \"@modelcontextprotocol/server-xxx\"]\n      \
                     }}\n    }}\n  }}\n}}",
                    config_file.display()
                ));
            }
            Err(_) => {
                chat_state.add_system_message(
                    "Could not determine config file path. Check ~/.config/bitfun/config/app.json"
                        .to_string(),
                );
            }
        }
    }

    /// Switch to a different session: restore it from core, reload messages, update state
    fn switch_to_session(
        &mut self,
        new_session_id: &str,
        session_id: &mut String,
        chat_state: &mut ChatState,
        chat_view: &mut ChatView,
        rt_handle: &tokio::runtime::Handle,
    ) -> Result<()> {
        let agent = self.agent.clone();
        let sid = new_session_id.to_string();
        let agent_type = self.agent_type.clone();
        let workspace = self.workspace.clone();

        let (new_state, restored_agent_type) = tokio::task::block_in_place(|| {
            rt_handle.block_on(async {
                // Restore session in core
                agent.restore_session(&sid).await?;

                // Get session info for agent_type and workspace
                let workspace_path = agent.workspace_path_buf();
                let sessions = agent
                    .coordinator()
                    .list_sessions(&workspace_path)
                    .await
                    .unwrap_or_default();
                let session_summary = sessions.iter().find(|s| s.session_id == sid);
                let restored_agent_type = session_summary
                    .map(|s| s.agent_type.clone())
                    .unwrap_or_else(|| agent_type.clone());
                let session_name = session_summary
                    .map(|s| s.session_name.clone())
                    .unwrap_or_else(|| "Restored Session".to_string());

                // Use the current workspace filtered by the session list; fall back to the
                // workspace supplied when this chat view was created.
                let effective_workspace = workspace
                    .clone()
                    .or_else(|| Some(workspace_path.to_string_lossy().to_string()));

                // Sync global workspace path from restored session
                if let Some(ref ws) = effective_workspace {
                    agent
                        .set_workspace_path(Some(std::path::PathBuf::from(ws)))
                        .await;
                }

                // Load historical messages from core.
                let messages = agent.coordinator().get_messages(&sid).await.unwrap_or_default();

                let state = ChatState::from_core_messages(
                    sid.clone(),
                    session_name,
                    restored_agent_type.clone(),
                    effective_workspace,
                    &messages,
                );

                Ok::<_, anyhow::Error>((state, restored_agent_type))
            })
        })?;

        // Update session state
        *session_id = new_session_id.to_string();
        *chat_state = new_state;
        self.agent_type = restored_agent_type;
        self.workspace = chat_state.workspace.clone();

        // Reload model name
        self.load_current_model_name(chat_state, rt_handle);

        // Reset view state
        chat_view.scroll_to_bottom();
        chat_view.set_status(Some(format!("Switched to session: {}", new_session_id)));

        Ok(())
    }

    /// Create a new session: reset state and start fresh
    fn create_new_session(
        &mut self,
        session_id: &mut String,
        chat_state: &mut ChatState,
        chat_view: &mut ChatView,
        rt_handle: &tokio::runtime::Handle,
    ) -> Result<()> {
        let agent = self.agent.clone();
        let agent_type = self.agent_type.clone();
        let workspace = self.workspace.clone();

        let new_session_id = tokio::task::block_in_place(|| {
            rt_handle.block_on(agent.create_new_session(&agent_type))
        })?;

        let new_state = ChatState::new(
            new_session_id.clone(),
            "CLI Session".to_string(),
            agent_type,
            workspace,
        );

        *session_id = new_session_id;
        *chat_state = new_state;
        self.workspace = chat_state.workspace.clone();

        // Reload model name
        self.load_current_model_name(chat_state, rt_handle);

        // Reset view state
        chat_view.clear_screen();
        chat_view.scroll_to_bottom();
        chat_view.set_status(Some("New session created".to_string()));

        Ok(())
    }

    /// Show skill selector popup with all available skills
    fn show_skill_selector(
        &self,
        chat_view: &mut ChatView,
        chat_state: &mut ChatState,
        rt_handle: &tokio::runtime::Handle,
    ) {
        let skills = tokio::task::block_in_place(|| {
            rt_handle.block_on(async {
                let registry = SkillRegistry::global();
                registry.refresh().await;
                registry.get_all_skills().await
            })
        });

        if skills.is_empty() {
            chat_state.add_system_message("No skills found. Add skills in .bitfun/skills/, .cursor/skills/, or ~/.cursor/skills/".to_string());
            return;
        }

        let skill_items: Vec<SkillItem> = skills
            .into_iter()
            .map(|s| SkillItem {
                name: s.name,
                description: s.description,
                level: s.level.as_str().to_string(),
            })
            .collect();

        if skill_items.is_empty() {
            chat_state.add_system_message("No skills found.".to_string());
            return;
        }

        chat_view.show_skill_selector(skill_items);
    }

    /// Apply skill selection: fill input box with execution command
    fn apply_skill_selection(&self, selected: &SkillItem, chat_view: &mut ChatView) {
        chat_view.set_input(&format!("Execute the {} skill.", selected.name));
    }

    /// Show subagent selector popup with all available subagents
    fn show_subagent_selector(
        &self,
        chat_view: &mut ChatView,
        chat_state: &mut ChatState,
        rt_handle: &tokio::runtime::Handle,
    ) {
        let registry = get_agent_registry();
        let subagents = tokio::task::block_in_place(|| {
            let workspace = self.workspace.clone().map(PathBuf::from);
            rt_handle.block_on(registry.get_subagents_info(workspace.as_deref()))
        });

        if subagents.is_empty() {
            chat_state.add_system_message("No subagents found.".to_string());
            return;
        }

        let subagent_items: Vec<SubagentItem> = subagents
            .into_iter()
            .map(|s| {
                let source = match s.subagent_source {
                    Some(bitfun_core::agentic::agents::SubAgentSource::Builtin) => {
                        "builtin".to_string()
                    }
                    Some(bitfun_core::agentic::agents::SubAgentSource::Project) => {
                        "project".to_string()
                    }
                    Some(bitfun_core::agentic::agents::SubAgentSource::User) => "user".to_string(),
                    None => "builtin".to_string(),
                };
                SubagentItem {
                    id: s.id,
                    name: s.name,
                    description: s.description,
                    source,
                }
            })
            .collect();

        if subagent_items.is_empty() {
            chat_state.add_system_message("No enabled subagents found.".to_string());
            return;
        }

        chat_view.show_subagent_selector(subagent_items);
    }

    /// Apply subagent selection: fill input box with launch command
    fn apply_subagent_selection(&self, selected: &SubagentItem, chat_view: &mut ChatView) {
        chat_view.set_input(&format!(
            "Launch subagent {} to finish task: ",
            selected.name
        ));
    }

    /// Send a message to the agent programmatically (used by slash commands like /init)
    fn send_message_to_agent(
        &self,
        message: String,
        chat_view: &mut ChatView,
        chat_state: &mut ChatState,
        rt_handle: &tokio::runtime::Handle,
    ) {
        if chat_state.is_processing {
            chat_state.add_system_message("Already processing, please wait.".to_string());
            return;
        }

        let display_name = agent_display_name(&self.agent_type);
        chat_view.set_status(Some(format!("{} is thinking...", display_name)));

        let agent = self.agent.clone();
        let agent_type = self.agent_type.clone();
        match tokio::task::block_in_place(|| {
            rt_handle.block_on(agent.send_message(message, &agent_type))
        }) {
            Ok(turn_id) => {
                tracing::info!("Started turn: {}", turn_id);
            }
            Err(e) => {
                tracing::error!("Failed to send message: {}", e);
                chat_view.set_status(Some(format!("Error: {}", e)));
            }
        }
    }

    /// Show session selector popup with all available sessions
    fn show_session_selector(
        &self,
        chat_view: &mut ChatView,
        chat_state: &mut ChatState,
        rt_handle: &tokio::runtime::Handle,
    ) {
        let agent = self.agent.clone();
        let current_session_id = chat_state.core_session_id.clone();

        let sessions = tokio::task::block_in_place(|| {
            rt_handle.block_on(async {
                agent
                    .coordinator()
                    .list_sessions(&agent.workspace_path_buf())
                    .await
                    .unwrap_or_default()
            })
        });

        if sessions.is_empty() {
            chat_state.add_system_message("No sessions found.".to_string());
            return;
        }

        let session_items: Vec<SessionItem> = sessions
            .into_iter()
            .map(|s| {
                let last_activity = {
                    let elapsed = s.last_activity_at.elapsed().unwrap_or_default();
                    if elapsed.as_secs() < 60 {
                        "just now".to_string()
                    } else if elapsed.as_secs() < 3600 {
                        format!("{}m ago", elapsed.as_secs() / 60)
                    } else if elapsed.as_secs() < 86400 {
                        format!("{}h ago", elapsed.as_secs() / 3600)
                    } else {
                        format!("{}d ago", elapsed.as_secs() / 86400)
                    }
                };
                SessionItem {
                    session_id: s.session_id,
                    session_name: s.session_name,
                    last_activity,
                    workspace: self.workspace.clone(),
                }
            })
            .collect();

        chat_view.show_session_selector(session_items, Some(current_session_id));
    }

    /// Handle session deletion from the session selector
    fn handle_session_delete(
        &self,
        item: &SessionItem,
        chat_view: &mut ChatView,
        chat_state: &mut ChatState,
        rt_handle: &tokio::runtime::Handle,
    ) {
        // Prevent deleting the currently active session
        if item.session_id == chat_state.core_session_id {
            chat_view.set_status(Some("Cannot delete the active session".to_string()));
            return;
        }

        let agent = self.agent.clone();
        let sid = item.session_id.clone();

        let result = tokio::task::block_in_place(|| {
            rt_handle.block_on(async {
                let workspace_path = agent.workspace_path_buf();
                agent.coordinator().delete_session(&workspace_path, &sid).await
            })
        });

        match result {
            Ok(()) => {
                chat_view.session_selector_remove_item(&item.session_id);
                chat_view.set_status(Some(format!("Session deleted: {}", item.session_name)));
                tracing::info!("Deleted session: {}", item.session_id);
            }
            Err(e) => {
                chat_view.set_status(Some(format!("Failed to delete session: {}", e)));
                tracing::error!("Failed to delete session: {}", e);
            }
        }
    }

    /// Handle provider selection result (step 1 → step 2)
    fn handle_provider_selection(&self, selection: ProviderSelection, chat_view: &mut ChatView) {
        match selection {
            ProviderSelection::Provider(template) => {
                let default_model = template.models.first().cloned().unwrap_or_default();
                chat_view.show_model_config_form_from_provider(
                    &template.name,
                    &template.base_url,
                    &template.format,
                    &default_model,
                );
            }
            ProviderSelection::Custom => {
                chat_view.show_model_config_form_custom();
            }
        }
    }

    /// Save new model to global config
    fn save_new_model(
        &self,
        result: ModelFormResult,
        chat_view: &mut ChatView,
        chat_state: &mut ChatState,
        rt_handle: &tokio::runtime::Handle,
    ) {
        let model_id = format!(
            "model_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis()
        );

        // Parse custom headers JSON if provided
        let custom_headers: Option<std::collections::HashMap<String, String>> =
            if result.custom_headers.is_empty() {
                None
            } else {
                serde_json::from_str(&result.custom_headers).ok()
            };

        let custom_request_body: Option<String> = if result.custom_request_body.is_empty() {
            None
        } else {
            Some(result.custom_request_body.clone())
        };

        let model_config = bitfun_core::service::config::AIModelConfig {
            id: model_id.clone(),
            name: result.name.clone(),
            provider: result.provider_format.clone(),
            model_name: result.model_name.clone(),
            base_url: result.base_url.clone(),
            api_key: result.api_key.clone(),
            context_window: Some(result.context_window),
            max_tokens: Some(result.max_tokens),
            enabled: true,
            enable_thinking_process: result.enable_thinking || result.support_preserved_thinking,
            skip_ssl_verify: result.skip_ssl_verify,
            custom_headers,
            custom_headers_mode: if result.custom_headers_mode.is_empty()
                || result.custom_headers_mode == "merge"
            {
                None
            } else {
                Some(result.custom_headers_mode.clone())
            },
            custom_request_body,
            ..Default::default()
        };

        let success = tokio::task::block_in_place(|| {
            rt_handle.block_on(async {
                let config_service = match GlobalConfigManager::get_service().await {
                    Ok(s) => s,
                    Err(e) => {
                        tracing::error!("Failed to get config service: {}", e);
                        return false;
                    }
                };

                if let Err(e) = config_service.add_ai_model(model_config).await {
                    tracing::error!("Failed to add AI model: {}", e);
                    return false;
                }

                // Auto-set as primary model if no primary model exists
                match config_service
                    .get_config::<bitfun_core::service::config::GlobalConfig>(None)
                    .await
                {
                    Ok(global_config) => {
                        let has_primary = global_config
                            .ai
                            .default_models
                            .primary
                            .as_ref()
                            .map(|p| !p.is_empty())
                            .unwrap_or(false);
                        if !has_primary {
                            if let Err(e) = config_service
                                .set_config("ai.default_models.primary", &model_id)
                                .await
                            {
                                tracing::warn!("Failed to auto-set primary model: {}", e);
                            } else {
                                tracing::info!("Auto-set primary model: {}", model_id);
                            }
                        }
                    }
                    Err(e) => {
                        tracing::warn!("Failed to read config for auto-primary: {}", e);
                    }
                }

                true
            })
        });

        if success {
            chat_view.set_status(Some(format!("Model added: {}", result.name)));
            chat_state.current_model_name =
                format!("{} / {}", result.model_name, result.name);
            tracing::info!("Added new AI model: {} ({})", model_id, result.model_name);
        } else {
            chat_view.set_status(Some("Failed to add model".to_string()));
        }
    }

    /// Fetch full model config and open the edit form
    fn edit_model(
        &self,
        selected: &ModelItem,
        chat_view: &mut ChatView,
        rt_handle: &tokio::runtime::Handle,
    ) {
        let model_id = selected.id.clone();
        let result = tokio::task::block_in_place(|| {
            rt_handle.block_on(async {
                let config_service = GlobalConfigManager::get_service().await.ok()?;
                let models: Vec<bitfun_core::service::config::AIModelConfig> =
                    config_service.get_ai_models().await.ok()?;
                models.into_iter().find(|m| m.id == model_id)
            })
        });

        match result {
            Some(model) => {
                let form_data = ModelFormResult {
                    editing_model_id: Some(model.id.clone()),
                    name: model.name,
                    model_name: model.model_name,
                    base_url: model.base_url,
                    api_key: model.api_key,
                    provider_format: model.provider.clone(),
                    context_window: model.context_window.unwrap_or(128000),
                    max_tokens: model.max_tokens.unwrap_or(8192),
                    enable_thinking: model.enable_thinking_process,
                    support_preserved_thinking: model.inline_think_in_text,
                    skip_ssl_verify: model.skip_ssl_verify,
                    custom_headers: model
                        .custom_headers
                        .map(|h| serde_json::to_string(&h).unwrap_or_default())
                        .unwrap_or_default(),
                    custom_headers_mode: model
                        .custom_headers_mode
                        .unwrap_or_else(|| "merge".to_string()),
                    custom_request_body: model.custom_request_body.unwrap_or_default(),
                };
                chat_view.show_model_config_form_for_edit(&model.id, &form_data);
            }
            None => {
                chat_view.set_status(Some("Failed to load model configuration".to_string()));
            }
        }
    }

    /// Update an existing model in global config
    fn update_existing_model(
        &self,
        result: ModelFormResult,
        chat_view: &mut ChatView,
        chat_state: &mut ChatState,
        rt_handle: &tokio::runtime::Handle,
    ) {
        let model_id = match &result.editing_model_id {
            Some(id) => id.clone(),
            None => return,
        };

        let custom_headers: Option<std::collections::HashMap<String, String>> =
            if result.custom_headers.is_empty() {
                None
            } else {
                serde_json::from_str(&result.custom_headers).ok()
            };

        let custom_request_body: Option<String> = if result.custom_request_body.is_empty() {
            None
        } else {
            Some(result.custom_request_body.clone())
        };

        let model_config = bitfun_core::service::config::AIModelConfig {
            id: model_id.clone(),
            name: result.name.clone(),
            provider: result.provider_format.clone(),
            model_name: result.model_name.clone(),
            base_url: result.base_url.clone(),
            api_key: result.api_key.clone(),
            context_window: Some(result.context_window),
            max_tokens: Some(result.max_tokens),
            enabled: true,
            enable_thinking_process: result.enable_thinking || result.support_preserved_thinking,
            skip_ssl_verify: result.skip_ssl_verify,
            custom_headers,
            custom_headers_mode: if result.custom_headers_mode.is_empty()
                || result.custom_headers_mode == "merge"
            {
                None
            } else {
                Some(result.custom_headers_mode.clone())
            },
            custom_request_body,
            ..Default::default()
        };

        let success = tokio::task::block_in_place(|| {
            rt_handle.block_on(async {
                let config_service = match GlobalConfigManager::get_service().await {
                    Ok(s) => s,
                    Err(e) => {
                        tracing::error!("Failed to get config service: {}", e);
                        return false;
                    }
                };

                if let Err(e) = config_service
                    .update_ai_model(&model_id, model_config)
                    .await
                {
                    tracing::error!("Failed to update AI model: {}", e);
                    return false;
                }

                true
            })
        });

        if success {
            chat_view.set_status(Some(format!("Model updated: {}", result.name)));
            chat_state.current_model_name =
                format!("{} / {}", result.model_name, result.name);
            tracing::info!("Updated AI model: {}", model_id);
        } else {
            chat_view.set_status(Some("Failed to update model".to_string()));
        }
    }
}
