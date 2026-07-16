/// Chat mode implementation
///
/// Interactive chat mode with TUI interface.
/// Events are observed through an independent runtime broadcast subscription.
use anyhow::{anyhow, Result};
use arboard::Clipboard;
use crossterm::event::{
    Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEventKind,
};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::broadcast::error::TryRecvError;

use bitfun_events::{AgenticEvent, ToolEventData};

use crate::actions::{
    action_by_id, action_for_alias, ActionContext, ActionHandler, ActionSpec, ActionState,
    ResolvedKeymap,
};
use crate::agent::{core_adapter::CoreAgentAdapter, Agent};
use crate::chat_state::ChatState;
use crate::config::CliConfig;
use crate::runtime::CliRuntimeContext;
use crate::ui::agent_selector::AgentItem;
use crate::ui::chat::{ChatView, MouseGestureOutcome};
use crate::ui::command_palette::PaletteAction;
use crate::ui::login_form::LoginFormAction;
use crate::ui::mcp_add_dialog::McpAddAction;
use crate::ui::mcp_selector::McpItem;
use crate::ui::model_config_form::{ModelFormAction, ModelFormResult};
use crate::ui::model_selector::ModelItem;
use crate::ui::permission::{PermissionAction, ALLOW_ALWAYS_RUNTIME_SCOPE};
use crate::ui::provider_selector::ProviderSelection;
use crate::ui::question::QuestionAction;
use crate::ui::session_selector::{SessionAction, SessionItem};
use crate::ui::skill_selector::{SkillItem, SkillSelectorAction};
use crate::ui::subagent_selector::{SubagentItem, SubagentSelectorAction};
use crate::ui::theme::{
    builtin_theme_ids, builtin_theme_json, resolve_appearance, resolve_effective_color_scheme,
    Appearance, EffectiveColorScheme, Theme,
};
use crate::ui::theme_selector::ThemeItem;
use crate::ui::{init_terminal, restore_terminal, TerminalGuard};
use bitfun_core::agentic::agents::{
    get_agent_registry, AgentInfo, SubAgentSource, SubagentListScope, SubagentQueryContext,
};
use bitfun_core::agentic::tools::implementations::skills::{
    mode_overrides::{
        load_project_mode_skills_document_local, save_project_mode_skills_document_local,
        set_mode_skill_disabled_in_document, set_user_mode_skill_state,
    },
    registry::SkillRegistry,
    ModeSkillInfo, SkillInfo,
};
use bitfun_core::service::config::GlobalConfigManager;
use bitfun_core::service::session_usage::{
    render_usage_report_markdown, SessionUsageReportRequest,
};

/// Spinner/UI redraw interval while a turn is processing.
const SPINNER_REDRAW_INTERVAL_MS: u64 = 100;
/// Coalesce rapid resize bursts to reduce flicker during window drag.
const RESIZE_REDRAW_DEBOUNCE_MS: u64 = 75;

fn agent_event_stream_failure(error: TryRecvError) -> Option<String> {
    match error {
        TryRecvError::Empty => None,
        TryRecvError::Lagged(skipped) => Some(format!(
            "Agent event stream lagged by {skipped} events; chat state can no longer be trusted"
        )),
        TryRecvError::Closed => {
            Some("Agent event stream closed; chat state can no longer be trusted".to_string())
        }
    }
}

fn mark_active_turn_failed(chat_state: &mut ChatState, error: &str) -> bool {
    if chat_state.current_turn_id().is_none() {
        return false;
    }

    chat_state.handle_turn_failed(error);
    true
}

/// Chat mode exit reason
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum ChatExitReason {
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

struct ChatEventContext<'a> {
    this: &'a mut ChatMode,
    chat_view: &'a mut ChatView,
    chat_state: &'a mut ChatState,
    session_id: &'a mut String,
    rt_handle: &'a tokio::runtime::Handle,
    should_quit: &'a mut bool,
    exit_reason: &'a mut ChatExitReason,
}

pub(crate) struct ChatMode {
    config: CliConfig,
    keymap: ResolvedKeymap,
    /// Current agent type (e.g. "agentic", "plan", "debug")
    agent_type: String,
    workspace: Option<String>,
    agent: Arc<CoreAgentAdapter>,
    runtime: Arc<CliRuntimeContext>,
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
    pub(crate) fn new(
        config: CliConfig,
        agent_type: String,
        workspace: Option<String>,
        runtime: Arc<CliRuntimeContext>,
    ) -> Self {
        let agent = Arc::new(CoreAgentAdapter::new(
            runtime.as_ref(),
            workspace.clone().map(PathBuf::from),
        ));

        let keymap = ResolvedKeymap::new(&config.shortcuts);
        Self {
            config,
            keymap,
            agent_type,
            workspace,
            agent,
            runtime,
            restore_session_id: None,
            initial_prompt: None,
            pending_mcp_op: None,
            pending_mcp_tasks: Vec::new(),
        }
    }

    /// Set a session ID to restore (for "Continue Last Session")
    pub(crate) fn with_restore_session(mut self, session_id: String) -> Self {
        self.restore_session_id = Some(session_id);
        self
    }

    /// Set an initial prompt to send automatically when the session starts
    pub(crate) fn with_initial_prompt(mut self, prompt: String) -> Self {
        self.initial_prompt = Some(prompt);
        self
    }

    fn workspace_path_for_sync(&self, chat_state: &ChatState) -> std::path::PathBuf {
        chat_state
            .workspace
            .as_ref()
            .map(std::path::PathBuf::from)
            .or_else(|| self.workspace.clone().map(std::path::PathBuf::from))
            .or_else(|| std::env::current_dir().ok())
            .unwrap_or_else(|| std::path::PathBuf::from("."))
    }

    fn open_login_or_account_panel(
        &self,
        chat_view: &mut ChatView,
        chat_state: &ChatState,
        rt_handle: &tokio::runtime::Handle,
    ) {
        let logged_in =
            tokio::task::block_in_place(|| rt_handle.block_on(crate::account::is_logged_in()));
        if logged_in {
            self.open_account_panel(chat_view, rt_handle);
        } else {
            chat_view.show_login_form();
        }
        let _ = chat_state;
    }

    fn open_account_panel(&self, chat_view: &mut ChatView, rt_handle: &tokio::runtime::Handle) {
        let (info, devices, progress) = tokio::task::block_in_place(|| {
            rt_handle.block_on(async {
                let info = crate::account::account_info().await;
                let devices = crate::account::list_devices().await.unwrap_or_default();
                let progress = crate::account_sync::current_sync_progress().await;
                (info, devices, progress)
            })
        });
        match info {
            Ok(info) => chat_view.show_account_panel(info, devices, progress),
            Err(e) => {
                chat_view.set_status(Some(format!("Failed to load account: {e}")));
                chat_view.show_login_form();
            }
        }
    }

    fn refresh_account_panel_live(&self, chat_view: &mut ChatView) {
        if !chat_view.login_form_visible() {
            return;
        }
        let progress = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(crate::account_sync::current_sync_progress())
        });
        let devices = if matches!(
            progress.status,
            crate::account_sync::SyncStatus::Syncing | crate::account_sync::SyncStatus::Done
        ) {
            tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current()
                    .block_on(crate::account::list_devices())
                    .ok()
            })
        } else {
            None
        };
        chat_view.update_account_panel_progress(devices, progress);
    }

    fn start_sync_and_show_account(
        &self,
        is_first_login: bool,
        chat_view: &mut ChatView,
        chat_state: &mut ChatState,
        rt_handle: &tokio::runtime::Handle,
    ) {
        let workspace = self.workspace_path_for_sync(chat_state);
        crate::account_sync::start_auto_sync_background(
            self.runtime.compatibility().clone(),
            is_first_login,
            workspace,
        );
        self.open_account_panel(chat_view, rt_handle);
        chat_state.add_system_message(if is_first_login {
            "Sync started (use local / upload settings).".to_string()
        } else {
            "Sync started (use cloud / download settings).".to_string()
        });
    }

    fn handle_login_form_action(
        &self,
        action: LoginFormAction,
        chat_view: &mut ChatView,
        chat_state: &mut ChatState,
        rt_handle: &tokio::runtime::Handle,
    ) -> Result<Option<ChatExitReason>> {
        match action {
            LoginFormAction::Submit(creds) => {
                let result = tokio::task::block_in_place(|| {
                    rt_handle.block_on(crate::account::login_with_credentials(
                        &creds.relay_url,
                        &creds.username,
                        &creds.password,
                    ))
                });
                match result {
                    Ok(login) => {
                        chat_state.add_system_message(login.status_message.clone());
                        if login.has_cloud_settings {
                            chat_view.show_sync_choice_panel(&login.user_id, &login.relay_url);
                        } else {
                            self.start_sync_and_show_account(
                                true, chat_view, chat_state, rt_handle,
                            );
                        }
                    }
                    Err(e) => {
                        chat_view.login_form_set_error(format!("Login failed: {e}"));
                    }
                }
            }
            LoginFormAction::SyncUseLocal => {
                self.start_sync_and_show_account(true, chat_view, chat_state, rt_handle);
            }
            LoginFormAction::SyncUseCloud => {
                self.start_sync_and_show_account(false, chat_view, chat_state, rt_handle);
            }
            LoginFormAction::SyncCancel => {
                let _ =
                    tokio::task::block_in_place(|| rt_handle.block_on(crate::account::logout()));
                chat_view.show_login_form();
                chat_state.add_system_message("Sync cancelled; logged out.".to_string());
            }
            LoginFormAction::Logout => {
                match tokio::task::block_in_place(|| rt_handle.block_on(crate::account::logout())) {
                    Ok(()) => {
                        chat_view.show_login_form();
                        chat_state.add_system_message("Logged out.".to_string());
                    }
                    Err(e) => {
                        chat_view.login_form_set_error(format!("Logout failed: {e}"));
                    }
                }
            }
            LoginFormAction::Cancel => {
                chat_view.set_status(Some("Account panel closed".to_string()));
            }
            LoginFormAction::None => {}
        }
        Ok(None)
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
            || chat_view.login_form_visible()
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
        chat_view.hide_login_form();
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
                crate::ui::chat::PopupType::LoginForm => chat_view.hide_login_form(),
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
                    crate::ui::chat::PopupType::LoginForm => chat_view.reshow_login_form(),
                    crate::ui::chat::PopupType::ThemeSelector => chat_view.reshow_theme_selector(),
                    crate::ui::chat::PopupType::InfoPopup => {}
                }
            }
        }
    }

    pub(crate) fn run(
        &mut self,
        existing_terminal: Option<TerminalGuard>,
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
        let shortcut_hints = self.keymap.compact_hints(ActionState::chat(false, false));
        let mut chat_view = ChatView::new(theme, shortcut_hints);

        // Create or restore core session
        let rt_handle = tokio::runtime::Handle::current();

        let (mut session_id, mut chat_state) = if let Some(ref restore_id) = self.restore_session_id
        {
            // Restore existing session
            tracing::info!("Restoring session: {}", restore_id);
            let agent = self.agent.clone();
            let rid = restore_id.clone();

            tokio::task::block_in_place(|| {
                rt_handle.block_on(async {
                    // Restore session in core (loads metadata, messages, managers)
                    let (summary, effective_workspace_path) =
                        agent.restore_session_in_current_workspace(&rid).await?;
                    let effective_workspace =
                        Some(effective_workspace_path.to_string_lossy().to_string());

                    // Load historical messages for UI display
                    let messages = agent.get_messages(&rid).await.unwrap_or_default();

                    let state = ChatState::from_core_messages(
                        rid.clone(),
                        summary.session_name,
                        summary.agent_type,
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
                "CLI Session".to_string(),
                self.agent_type.clone(),
                self.workspace.clone(),
            );
            (session_id, state)
        };

        // Keep ChatMode workspace in sync with the session's effective workspace
        self.agent_type = chat_state.agent_type.clone();
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

        let mut event_rx = self.agent.event_source().subscribe();

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

        let mut exit_reason = ChatExitReason::Quit;
        let mut should_quit = false;
        let mut needs_redraw = true;
        let mut subagent_parent_tools: HashMap<String, String> = HashMap::new();
        let mut last_spinner_redraw = Instant::now();
        let mut pending_resize_at: Option<Instant> = None;
        let mut fatal_event_stream_error: Option<String> = None;
        let spinner_redraw_interval = Duration::from_millis(SPINNER_REDRAW_INTERVAL_MS);
        let resize_redraw_debounce = Duration::from_millis(RESIZE_REDRAW_DEBOUNCE_MS);

        while !should_quit {
            chat_view.set_action_state(
                ActionState::chat(chat_state.is_processing, false),
                &self.keymap,
            );

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

            if chat_view.login_form_visible() {
                self.refresh_account_panel_live(&mut chat_view);
                if crate::account_sync::sync_in_flight() {
                    needs_redraw = true;
                }
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
            let mut events = Vec::with_capacity(20);
            for _ in 0..20 {
                match event_rx.try_recv() {
                    Ok(envelope) => events.push(envelope),
                    Err(error) => {
                        let Some(mut failure) = agent_event_stream_failure(error) else {
                            break;
                        };

                        // The adapter records the turn before DialogTurnStarted reaches the UI,
                        // so cancellation must not depend on ChatState having seen that event.
                        let agent = self.agent.clone();
                        if let Err(cancel_error) = tokio::task::block_in_place(|| {
                            rt_handle.block_on(agent.cancel_current_turn())
                        }) {
                            failure = format!(
                                "{failure}; failed to cancel the active turn: {cancel_error}"
                            );
                        }
                        mark_active_turn_failed(&mut chat_state, &failure);
                        chat_view.invalidate_lines_cache();
                        chat_view.set_status(Some(format!("Error: {failure}")));
                        tracing::error!("{failure}");
                        fatal_event_stream_error = Some(failure);
                        break;
                    }
                }
            }
            if fatal_event_stream_error.is_some() {
                break;
            }
            for envelope in events {
                let event = &envelope.event;

                if let AgenticEvent::SubagentSessionLinked {
                    session_id: subagent_session_id,
                    parent_session_id,
                    parent_tool_call_id,
                    ..
                } = event
                {
                    if parent_session_id == &session_id {
                        subagent_parent_tools
                            .insert(subagent_session_id.clone(), parent_tool_call_id.clone());
                    }
                    continue;
                }

                // Check if this is a subagent event that belongs to our session
                if event.session_id() != Some(&session_id) {
                    // Check if this event was emitted by a subagent whose parent is in our session
                    if let Some(parent_tool_call_id) = event
                        .session_id()
                        .and_then(|event_session_id| subagent_parent_tools.get(event_session_id))
                    {
                        // Forward subagent event to the parent Task tool for progress display
                        chat_state.handle_subagent_event(parent_tool_call_id, event);
                        chat_view.invalidate_lines_cache();
                        needs_redraw = true;
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
                        if let ToolEventData::ConfirmationNeeded { identity, .. } = tool_event {
                            if self
                                .runtime
                                .approval_controller()
                                .is_allowed(identity.effective_name())
                            {
                                let agent = self.agent.clone();
                                let tool_id = identity.tool_id.clone();
                                match tokio::task::block_in_place(|| {
                                    rt_handle.block_on(agent.confirm_tool(&tool_id, None))
                                }) {
                                    Ok(()) => continue,
                                    Err(error) => tracing::error!(
                                        "Failed to confirm runtime-approved tool; showing the permission prompt again: {}",
                                        error
                                    ),
                                }
                            }
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
                            if chat_view.login_form_visible() {
                                chat_view.login_form_insert_paste(&normalized);
                            } else {
                                for c in normalized.chars() {
                                    chat_view.handle_char(c);
                                }
                            }
                            needs_redraw = true;
                        }
                        // Process any non-key events that were mixed in
                        for ev in non_key_events {
                            let outcome = Self::handle_non_key_event(
                                ev,
                                ChatEventContext {
                                    this: self,
                                    chat_view: &mut chat_view,
                                    chat_state: &mut chat_state,
                                    session_id: &mut session_id,
                                    rt_handle: &rt_handle,
                                    should_quit: &mut should_quit,
                                    exit_reason: &mut exit_reason,
                                },
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
                                            ChatEventContext {
                                                this: self,
                                                chat_view: &mut chat_view,
                                                chat_state: &mut chat_state,
                                                session_id: &mut session_id,
                                                rt_handle: &rt_handle,
                                                should_quit: &mut should_quit,
                                                exit_reason: &mut exit_reason,
                                            },
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
                                        ChatEventContext {
                                            this: self,
                                            chat_view: &mut chat_view,
                                            chat_state: &mut chat_state,
                                            session_id: &mut session_id,
                                            rt_handle: &rt_handle,
                                            should_quit: &mut should_quit,
                                            exit_reason: &mut exit_reason,
                                        },
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

        let terminal_restore_result = restore_terminal(terminal);
        if let Some(failure) = fatal_event_stream_error {
            if let Err(restore_error) = terminal_restore_result {
                return Err(anyhow!(
                    "{failure}; failed to restore the terminal: {restore_error}"
                ));
            }
            return Err(anyhow!(failure));
        }
        terminal_restore_result?;
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

        let modal_state =
            ActionState::chat(chat_state.is_processing, self.any_popup_visible(chat_view));
        if let Some(action) = self.keymap.resolve_modal_safe(key, modal_state) {
            return self.dispatch_action(action, modal_state, chat_view, chat_state, rt_handle);
        }

        // ── Permission prompt intercepts all keys when active ──
        if let Some(ref mut prompt) = chat_state.permission_prompt {
            let action = prompt.handle_key_event(key);
            match action {
                PermissionAction::AllowOnce => {
                    let tool_id = prompt.tool_id.clone();
                    let agent = self.agent.clone();
                    tracing::info!("User allowed tool once: {}", tool_id);
                    match tokio::task::block_in_place(|| {
                        rt_handle.block_on(agent.confirm_tool(&tool_id, None))
                    }) {
                        Ok(()) => {
                            chat_state.permission_prompt = None;
                            chat_view.set_status(Some("Tool confirmed".to_string()));
                        }
                        Err(error) => {
                            tracing::error!("Failed to confirm tool: {}", error);
                            chat_view.set_status(Some(format!("Error: {error}")));
                        }
                    }
                }
                PermissionAction::AllowAlways => {
                    let tool_id = prompt.tool_id.clone();
                    let tool_name = prompt.tool_name().to_string();
                    let agent = self.agent.clone();
                    tracing::info!(
                        "User allowed tool {}: tool_id={}, tool_name={}",
                        ALLOW_ALWAYS_RUNTIME_SCOPE,
                        tool_id,
                        tool_name
                    );
                    match tokio::task::block_in_place(|| {
                        rt_handle.block_on(agent.confirm_tool(&tool_id, None))
                    }) {
                        Ok(()) => {
                            self.runtime.approval_controller().allow_always(&tool_name);
                            chat_state.permission_prompt = None;
                            chat_view.set_status(Some(format!(
                                "Tool approved {ALLOW_ALWAYS_RUNTIME_SCOPE}"
                            )));
                        }
                        Err(error) => {
                            tracing::error!("Failed to confirm tool: {}", error);
                            chat_view.set_status(Some(format!("Error: {error}")));
                        }
                    }
                }
                PermissionAction::Reject(reason) => {
                    let tool_id = prompt.tool_id.clone();
                    let agent = self.agent.clone();
                    tracing::info!("User rejected tool: {}, reason: {}", tool_id, reason);
                    let reason_clone = reason.clone();
                    match tokio::task::block_in_place(|| {
                        rt_handle.block_on(agent.reject_tool(&tool_id, reason_clone))
                    }) {
                        Ok(()) => {
                            chat_state.permission_prompt = None;
                            chat_view.set_status(Some(format!("Tool rejected: {}", reason)));
                        }
                        Err(error) => {
                            tracing::error!("Failed to reject tool: {}", error);
                            chat_view.set_status(Some(format!("Error: {error}")));
                        }
                    }
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

        // Host recovery keys win over configured actions while a popup is open.
        if self.any_popup_visible(chat_view) {
            let state = ActionState::chat(chat_state.is_processing, true);
            if let Some(action) = self.keymap.resolve_reserved(key, state) {
                return self.dispatch_action(action, state, chat_view, chat_state, rt_handle);
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
                PaletteAction::Dismiss => self.navigate_back(chat_view),
                PaletteAction::None => {}
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
                KeyCode::Enter | KeyCode::Char(' ') => {
                    if let Some(action) = chat_view.skill_selector_confirm() {
                        self.handle_skill_selector_action(action, chat_view, chat_state, rt_handle);
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
                KeyCode::Enter | KeyCode::Char(' ') => {
                    if let Some(action) = chat_view.subagent_selector_confirm() {
                        self.handle_subagent_selector_action(
                            action, chat_view, chat_state, rt_handle,
                        );
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

        if chat_view.login_form_visible() {
            self.refresh_account_panel_live(chat_view);
            let action = chat_view.login_form_handle_key(key);
            return self.handle_login_form_action(action, chat_view, chat_state, rt_handle);
        }

        if let Some(action) = self
            .keymap
            .resolve(key, ActionState::chat(chat_state.is_processing, false))
        {
            return self.dispatch_action(
                action,
                ActionState::chat(chat_state.is_processing, false),
                chat_view,
                chat_state,
                rt_handle,
            );
        }

        match (key.code, key.modifiers) {
            (KeyCode::Backspace, _) => {
                chat_view.handle_backspace();
            }

            (KeyCode::Left, _) => {
                chat_view.move_cursor_left();
            }
            (KeyCode::Right, _) => {
                chat_view.move_cursor_right();
            }

            (KeyCode::Home, _) => {
                chat_view.set_cursor_home();
            }

            (KeyCode::End, _) => {
                chat_view.set_cursor_end();
            }

            (KeyCode::Esc, _) => {
                if chat_view.browse_mode {
                    chat_view.scroll_to_bottom();
                    chat_view.set_status(Some("Exited browse mode".to_string()));
                }
            }

            (KeyCode::Char(c), KeyModifiers::NONE | KeyModifiers::SHIFT)
                if !c.is_control() && c != '\u{0}' =>
            {
                chat_view.handle_char(c);
            }

            _ => {}
        }

        Ok(None)
    }

    /// Apply an exit reason from handle_key_event (shared by normal and batch paths).
    fn apply_exit_reason(reason: ChatExitReason, context: ChatEventContext<'_>) {
        let ChatEventContext {
            this,
            chat_view,
            chat_state,
            session_id,
            rt_handle,
            should_quit,
            exit_reason,
        } = context;
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
        context: ChatEventContext<'_>,
    ) -> Result<NonKeyEventOutcome> {
        let mut outcome = NonKeyEventOutcome::default();
        match event {
            Event::Mouse(mouse) => {
                if context.chat_view.command_palette_captures_mouse(&mouse) {
                    let action = context.chat_view.command_palette_handle_mouse(&mouse);
                    match action {
                        PaletteAction::Execute(id) => {
                            if let Some(reason) = context.this.handle_palette_action(
                                &id,
                                context.chat_view,
                                context.chat_state,
                                context.rt_handle,
                            )? {
                                Self::apply_exit_reason(
                                    reason,
                                    ChatEventContext {
                                        this: &mut *context.this,
                                        chat_view: &mut *context.chat_view,
                                        chat_state: &mut *context.chat_state,
                                        session_id: &mut *context.session_id,
                                        rt_handle: context.rt_handle,
                                        should_quit: &mut *context.should_quit,
                                        exit_reason: &mut *context.exit_reason,
                                    },
                                );
                            }
                        }
                        PaletteAction::Dismiss => context.this.navigate_back(context.chat_view),
                        PaletteAction::None => {}
                    }
                } else if context.chat_view.provider_selector_captures_mouse(&mouse) {
                    if let Some(selection) =
                        context.chat_view.provider_selector_handle_mouse(&mouse)
                    {
                        context
                            .this
                            .handle_provider_selection(selection, context.chat_view);
                    }
                } else if context.chat_view.handle_mouse_event(&mouse) {
                    if let Some(action) = context.chat_view.take_pending_skill_action() {
                        context.this.handle_skill_selector_action(
                            action,
                            context.chat_view,
                            context.chat_state,
                            context.rt_handle,
                        );
                    }
                    if let Some(action) = context.chat_view.take_pending_subagent_action() {
                        context.this.handle_subagent_selector_action(
                            action,
                            context.chat_view,
                            context.chat_state,
                            context.rt_handle,
                        );
                    }
                } else {
                    match mouse.kind {
                        MouseEventKind::ScrollUp => {
                            let total = context.chat_view.count_message_lines(context.chat_state);
                            context.chat_view.scroll_up(3, total);
                        }
                        MouseEventKind::ScrollDown => {
                            context.chat_view.scroll_down(3);
                        }
                        MouseEventKind::Down(MouseButton::Left) => {
                            let _ = context
                                .chat_view
                                .begin_mouse_selection(mouse.column, mouse.row);
                        }
                        MouseEventKind::Drag(MouseButton::Left) => {
                            let _ = context
                                .chat_view
                                .update_mouse_selection(mouse.column, mouse.row);
                        }
                        MouseEventKind::Up(MouseButton::Left) => {
                            match context
                                .chat_view
                                .complete_mouse_selection_or_click(mouse.column, mouse.row)
                            {
                                MouseGestureOutcome::CopyText(text) => {
                                    match Clipboard::new().and_then(|mut cb| cb.set_text(text)) {
                                        Ok(()) => context
                                            .chat_view
                                            .set_status(Some("Copied to clipboard".to_string())),
                                        Err(_) => context.chat_view.set_status(Some(
                                            "Failed to copy selection".to_string(),
                                        )),
                                    }
                                }
                                MouseGestureOutcome::Click(col, row) => {
                                    context.chat_view.handle_mouse_click(col, row);
                                }
                                MouseGestureOutcome::None => {}
                            }
                        }
                        MouseEventKind::Moved
                            if !context
                                .chat_view
                                .update_mouse_selection(mouse.column, mouse.row) =>
                        {
                            context.chat_view.handle_mouse_move(mouse.column, mouse.row);
                        }
                        _ => {}
                    }
                }
                if let Some(action_id) = context.chat_view.take_pending_command() {
                    if let Some(reason) = context.this.handle_action_id(
                        &action_id,
                        context.chat_view,
                        context.chat_state,
                        context.rt_handle,
                    )? {
                        Self::apply_exit_reason(
                            reason,
                            ChatEventContext {
                                this: &mut *context.this,
                                chat_view: &mut *context.chat_view,
                                chat_state: &mut *context.chat_state,
                                session_id: &mut *context.session_id,
                                rt_handle: context.rt_handle,
                                should_quit: &mut *context.should_quit,
                                exit_reason: &mut *context.exit_reason,
                            },
                        );
                    }
                }
                if let Some(theme) = context.chat_view.take_pending_theme_preview() {
                    context
                        .this
                        .preview_theme_selection(&theme, context.chat_view);
                }
                if let Some(server_id) = context.chat_view.take_pending_mcp_toggle() {
                    context
                        .this
                        .toggle_mcp_server(&server_id, context.chat_view);
                }
                outcome.request_redraw = true;
            }
            Event::Paste(text) => {
                if context.chat_view.mcp_add_dialog_visible() {
                    context.chat_view.mcp_add_dialog_handle_paste(&text);
                } else if context.chat_view.login_form_visible() {
                    context.chat_view.login_form_insert_paste(&text);
                } else {
                    let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
                    for c in normalized.chars() {
                        context.chat_view.handle_char(c);
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
        self.handle_action_id(action_id, chat_view, chat_state, rt_handle)
    }

    fn handle_action_id(
        &mut self,
        action_id: &str,
        chat_view: &mut ChatView,
        chat_state: &mut ChatState,
        rt_handle: &tokio::runtime::Handle,
    ) -> Result<Option<ChatExitReason>> {
        let Some(action) = action_by_id(action_id, ActionContext::Chat) else {
            chat_view.set_status(Some(format!("Unknown action: {action_id}")));
            return Ok(None);
        };
        self.dispatch_action(
            action,
            ActionState::chat(chat_state.is_processing, false),
            chat_view,
            chat_state,
            rt_handle,
        )
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

        let Some(action) = action_for_alias(parts[0], ActionContext::Chat) else {
            chat_state.add_system_message(format!(
                "Unknown command: {}\nUse /help to see available commands",
                parts[0]
            ));
            return Ok(None);
        };
        self.dispatch_action(
            action,
            ActionState::chat(chat_state.is_processing, false),
            chat_view,
            chat_state,
            rt_handle,
        )
    }

    fn dispatch_action(
        &mut self,
        action: &'static ActionSpec,
        state: ActionState,
        chat_view: &mut ChatView,
        chat_state: &mut ChatState,
        rt_handle: &tokio::runtime::Handle,
    ) -> Result<Option<ChatExitReason>> {
        if !action.available(state) {
            chat_view.set_status(Some(action.unavailable_message(state)));
            return Ok(None);
        }

        match action.handler {
            ActionHandler::Help => {
                chat_view.show_info_popup(self.keymap.help_text(state));
            }
            ActionHandler::ClearConversation => {
                if chat_state.is_processing {
                    self.cancel_active_turn(chat_view, rt_handle);
                }
                chat_state.clear_messages();
                chat_view.clear_screen();
                chat_view.set_status(Some("Conversation cleared".to_string()));
            }
            ActionHandler::OpenAgentSelector => {
                self.show_agent_selector(chat_view, chat_state, rt_handle);
            }
            ActionHandler::SwitchAgent => {
                self.cycle_agent(chat_view, chat_state, rt_handle);
            }
            ActionHandler::SwitchAgentReverse => {
                self.cycle_agent_reverse(chat_view, chat_state, rt_handle);
            }
            ActionHandler::SelectModel => {
                self.show_model_selector(chat_view, chat_state, rt_handle);
            }
            ActionHandler::SelectTheme => {
                let themes = self.list_available_themes();
                chat_view.begin_theme_preview();
                chat_view.show_theme_selector(themes, Some(self.config.ui.theme_id.clone()));
                chat_view.set_status(Some(
                    "Theme selector: ↑↓ preview, Enter apply, Esc cancel".to_string(),
                ));
            }
            ActionHandler::AddModel => chat_view.show_provider_selector(),
            ActionHandler::NewSession => {
                return Ok(Some(ChatExitReason::NewSession));
            }
            ActionHandler::Sessions => {
                self.show_session_selector(chat_view, chat_state, rt_handle);
            }
            ActionHandler::Skills => {
                self.show_skill_selector(chat_view, chat_state, rt_handle);
            }
            ActionHandler::ReloadSkills => {
                self.reload_skills_from_disk(chat_view, chat_state, rt_handle);
            }
            ActionHandler::Subagents => {
                self.show_subagent_selector(chat_view, chat_state, rt_handle);
            }
            ActionHandler::McpServers => {
                self.show_mcp_selector(chat_view, chat_state, rt_handle);
            }
            ActionHandler::AcpHelp => {
                chat_state.add_system_message(crate::acp_cli::acp_help_text("bitfun-cli"));
                chat_view.set_status(Some(
                    "ACP setup added to the conversation. You can keep typing.".to_string(),
                ));
            }
            ActionHandler::Init => match crate::prompts::get_cli_prompt("init") {
                Some(prompt) => {
                    self.send_message_to_agent(prompt.to_string(), chat_view, chat_state, rt_handle)
                }
                None => chat_state.add_system_message(
                    "Init prompt not found. Please create prompts/init.md in the CLI crate."
                        .to_string(),
                ),
            },
            ActionHandler::History => {
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
            ActionHandler::Usage => self.show_usage_report(chat_view, chat_state, rt_handle),
            ActionHandler::Exit => {
                if chat_state.is_processing {
                    self.cancel_active_turn(chat_view, rt_handle);
                }
                return Ok(Some(ChatExitReason::Quit));
            }
            ActionHandler::Login => {
                self.close_all_popups(chat_view);
                self.open_login_or_account_panel(chat_view, chat_state, rt_handle);
            }
            ActionHandler::Logout => self.logout(chat_state, rt_handle),
            ActionHandler::OpenPalette => chat_view.show_command_palette(state),
            ActionHandler::SubmitInput => {
                return self.submit_input(chat_view, chat_state, rt_handle);
            }
            ActionHandler::Interrupt => self.cancel_active_turn(chat_view, rt_handle),
            ActionHandler::ClosePopups => self.close_all_popups(chat_view),
            ActionHandler::NavigateBack => self.navigate_back(chat_view),
            ActionHandler::InsertNewline => chat_view.handle_newline(),
            ActionHandler::Paste => self.paste_clipboard(chat_view),
            ActionHandler::ToggleFocusedTool => {
                chat_view.toggle_focused_tool_expand(chat_state);
            }
            ActionHandler::PreviousTool => {
                chat_view.cycle_block_tool_focus_prev(chat_state);
            }
            ActionHandler::NextTool => {
                chat_view.cycle_block_tool_focus_next(chat_state);
            }
            ActionHandler::HistoryPrevious => {
                if chat_view.command_menu_visible() {
                    chat_view.command_menu_up();
                } else {
                    chat_view.history_prev();
                }
            }
            ActionHandler::HistoryNext => {
                if chat_view.command_menu_visible() {
                    chat_view.command_menu_down();
                } else {
                    chat_view.history_next();
                }
            }
            ActionHandler::JumpTop => {
                let total = chat_view.count_message_lines(chat_state);
                chat_view.scroll_to_top(total);
                chat_view.set_status(Some("Jumped to conversation top".to_string()));
            }
            ActionHandler::JumpBottom => {
                chat_view.scroll_to_bottom();
                chat_view.set_status(Some("Jumped to conversation bottom".to_string()));
            }
            ActionHandler::ClearInput => chat_view.clear_input(),
            ActionHandler::ToggleBrowse => {
                chat_view.toggle_browse_mode();
                let status = if chat_view.browse_mode {
                    "Entered browse mode, use PageUp/PageDown or mouse wheel to scroll conversation"
                } else {
                    "Exited browse mode"
                };
                chat_view.set_status(Some(status.to_string()));
            }
            ActionHandler::ScrollUp => {
                let total = chat_view.count_message_lines(chat_state);
                chat_view.scroll_up(10, total);
            }
            ActionHandler::ScrollDown => chat_view.scroll_down(10),
        }
        Ok(None)
    }

    fn submit_input(
        &mut self,
        chat_view: &mut ChatView,
        chat_state: &mut ChatState,
        rt_handle: &tokio::runtime::Handle,
    ) -> Result<Option<ChatExitReason>> {
        if let Some(action_id) = chat_view.apply_command_menu_selection() {
            return self.handle_action_id(&action_id, chat_view, chat_state, rt_handle);
        }

        if chat_state.is_processing {
            let trimmed = chat_view.input_text().trim();
            if trimmed.starts_with('/') {
                if let Some(input) = chat_view.send_input() {
                    return self.handle_command(&input, chat_view, chat_state, rt_handle);
                }
            } else if !trimmed.is_empty() {
                chat_view.set_status(Some(
                    "Currently processing. Type a /command, or use the interrupt shortcut."
                        .to_string(),
                ));
            }
            return Ok(None);
        }

        if let Some(input) = chat_view.send_input() {
            tracing::info!("User input: {}", input);
            if input.starts_with('/') {
                return self.handle_command(&input, chat_view, chat_state, rt_handle);
            }
            self.send_message_to_agent(input, chat_view, chat_state, rt_handle);
        }
        Ok(None)
    }

    fn cancel_active_turn(&self, chat_view: &mut ChatView, rt_handle: &tokio::runtime::Handle) {
        tracing::info!("User requested cancellation");
        let agent = self.agent.clone();
        tokio::task::block_in_place(|| {
            rt_handle.block_on(async move {
                if let Err(error) = agent.cancel_current_turn().await {
                    tracing::error!("Failed to cancel turn: {}", error);
                }
            })
        });
        chat_view.set_status(Some("Cancelling...".to_string()));
    }

    fn paste_clipboard(&self, chat_view: &mut ChatView) {
        if let Ok(text) = Clipboard::new().and_then(|mut clipboard| clipboard.get_text()) {
            let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
            for character in normalized.chars() {
                chat_view.handle_char(character);
            }
        }
    }

    fn logout(&self, chat_state: &mut ChatState, rt_handle: &tokio::runtime::Handle) {
        let logged_in =
            tokio::task::block_in_place(|| rt_handle.block_on(crate::account::is_logged_in()));
        if !logged_in {
            chat_state.add_system_message("Not logged in.".to_string());
            return;
        }
        match tokio::task::block_in_place(|| rt_handle.block_on(crate::account::logout())) {
            Ok(()) => chat_state.add_system_message("Logged out.".to_string()),
            Err(error) => chat_state.add_system_message(format!("Logout failed: {error}")),
        }
    }

    fn show_usage_report(
        &self,
        chat_view: &mut ChatView,
        chat_state: &mut ChatState,
        rt_handle: &tokio::runtime::Handle,
    ) {
        if chat_state.is_processing {
            chat_view.set_status(Some(
                "Wait until the session is idle before using /usage.".to_string(),
            ));
            return;
        }

        let session_id = chat_state.core_session_id.clone();
        let workspace_path = chat_state
            .workspace
            .clone()
            .or_else(|| self.workspace.clone())
            .or_else(|| Some(self.agent.workspace_path_string()));
        let agent = self.agent.clone();

        let report_result: Result<bitfun_core::service::session_usage::SessionUsageReport> =
            tokio::task::block_in_place(|| {
                let session_id = session_id.clone();
                let workspace_path = workspace_path.clone();
                let agent = agent.clone();
                rt_handle.block_on(async move {
                    let workspace_path = workspace_path
                        .filter(|path| !path.trim().is_empty())
                        .ok_or_else(|| anyhow!("Workspace path is required for usage reports"))?;

                    let report = agent
                        .generate_session_usage_report(SessionUsageReportRequest {
                            session_id: session_id.clone(),
                            workspace_path: Some(workspace_path),
                            remote_connection_id: None,
                            remote_ssh_host: None,
                            include_hidden_subagents: true,
                        })
                        .await?;

                    let markdown = render_usage_report_markdown(&report);
                    let generated_at = u64::try_from(report.generated_at).unwrap_or_default();
                    let usage_report = serde_json::to_value(&report)
                        .map_err(|error| anyhow!("Failed to serialize usage report: {}", error))?;
                    let metadata = serde_json::json!({
                        "localCommandKind": "usage_report",
                        "reportId": report.report_id.clone(),
                        "schemaVersion": report.schema_version,
                        "generatedAt": report.generated_at,
                        "modelVisible": false,
                        "usageReport": usage_report,
                        "usageReportStatus": "completed",
                    });

                    agent
                        .append_completed_local_command_turn(
                            &session_id,
                            markdown,
                            Some(format!("local-usage-{}", report.report_id)),
                            Some(generated_at),
                            Some(metadata),
                        )
                        .await
                        .map_err(|error| anyhow!(error.to_string()))?;

                    Ok(report)
                })
            });

        match report_result {
            Ok(report) => {
                let markdown = render_usage_report_markdown(&report);
                chat_state.add_assistant_message(markdown);
                chat_view.set_status(Some("Usage report added to conversation".to_string()));
            }
            Err(error) => {
                chat_state
                    .add_system_message(format!("Failed to generate usage report: {}", error));
            }
        }
    }

    fn list_available_themes(&self) -> Vec<ThemeItem> {
        let mut themes = Vec::new();
        for id in builtin_theme_ids() {
            themes.push(ThemeItem { id });
        }

        themes.sort_by_cached_key(|theme| theme.id.to_ascii_lowercase());
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
                    let join_result = tokio::task::block_in_place(|| rt_handle.block_on(handle));

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
                    let join_result = tokio::task::block_in_place(|| rt_handle.block_on(handle));

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
                    let join_result = tokio::task::block_in_place(|| rt_handle.block_on(handle));

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

        let (new_state, restored_agent_type) = tokio::task::block_in_place(|| {
            rt_handle.block_on(async {
                let (session_summary, effective_workspace_path) =
                    agent.restore_session_in_current_workspace(&sid).await?;
                let restored_agent_type = session_summary.agent_type.clone();
                let effective_workspace =
                    Some(effective_workspace_path.to_string_lossy().to_string());

                // Load historical messages from core.
                let messages = agent.get_messages(&sid).await.unwrap_or_default();

                let state = ChatState::from_core_messages(
                    sid.clone(),
                    session_summary.session_name,
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

    /// Show skill list/configuration menu.
    fn show_skill_selector(
        &self,
        chat_view: &mut ChatView,
        _chat_state: &mut ChatState,
        _rt_handle: &tokio::runtime::Handle,
    ) {
        chat_view.show_skill_menu();
    }

    /// Re-scan skill directories from disk and rebuild the registry cache.
    ///
    /// Mirrors Claude Code 2.1.152 `/reload-skills`. Safe to call at any
    /// time — does not require `is_processing` to be false because the
    /// registry swap is atomic and a held `SkillInfo` reference is not
    /// kept across the call.
    fn reload_skills_from_disk(
        &self,
        chat_view: &mut ChatView,
        chat_state: &mut ChatState,
        rt_handle: &tokio::runtime::Handle,
    ) {
        let registry = SkillRegistry::global();
        let workspace = self.agent.workspace_path_buf();
        let outcome = tokio::task::block_in_place(|| {
            // refresh() is the global re-scan entry point; the workspace
            // arg of refresh_for_workspace is currently a no-op upstream,
            // so we call refresh() directly and re-resolve the workspace
            // count afterwards.
            rt_handle.block_on(async {
                registry.refresh().await;
                registry
                    .get_resolved_skills_for_workspace(Some(workspace.as_path()), None)
                    .await
            })
        });

        let count = outcome.len();
        chat_state.add_system_message(format!("Reloaded {} skill(s) from disk.", count));
        chat_view.set_status(Some(format!("Skills reloaded ({} available)", count)));
    }

    fn show_available_skill_list(
        &self,
        chat_view: &mut ChatView,
        chat_state: &mut ChatState,
        rt_handle: &tokio::runtime::Handle,
    ) {
        let skills = tokio::task::block_in_place(|| {
            let workspace = self.agent.workspace_path_buf();
            let agent_type = self.agent_type.clone();
            rt_handle.block_on(async {
                let registry = SkillRegistry::global();
                registry
                    .get_resolved_skills_for_workspace(Some(workspace.as_path()), Some(&agent_type))
                    .await
            })
        });

        if skills.is_empty() {
            chat_state.add_system_message(format!(
                "No enabled skills found for agent mode '{}'. Add skills in .bitfun/skills/, .cursor/skills/, or ~/.cursor/skills/, or enable built-in skills for this mode.",
                self.agent_type
            ));
            return;
        }

        let skill_items: Vec<SkillItem> =
            skills.into_iter().map(Self::skill_item_from_info).collect();

        if skill_items.is_empty() {
            chat_state.add_system_message("No skills found.".to_string());
            return;
        }

        chat_view.show_skill_list(skill_items);
    }

    fn show_skill_config_selector(
        &self,
        chat_view: &mut ChatView,
        chat_state: &mut ChatState,
        rt_handle: &tokio::runtime::Handle,
    ) {
        let skills = tokio::task::block_in_place(|| {
            let workspace = self.agent.workspace_path_buf();
            let agent_type = self.agent_type.clone();
            rt_handle.block_on(async {
                let registry = SkillRegistry::global();
                registry
                    .get_mode_skill_infos_for_workspace(Some(workspace.as_path()), &agent_type)
                    .await
            })
        });

        let skill_items: Vec<SkillItem> = skills
            .into_iter()
            .map(Self::skill_item_from_mode_info)
            .collect();

        if skill_items.is_empty() {
            chat_state.add_system_message("No skills found.".to_string());
            return;
        }

        chat_view.show_skill_config(skill_items);
    }

    fn handle_skill_selector_action(
        &self,
        action: SkillSelectorAction,
        chat_view: &mut ChatView,
        chat_state: &mut ChatState,
        rt_handle: &tokio::runtime::Handle,
    ) {
        match action {
            SkillSelectorAction::ListSkills => {
                self.show_available_skill_list(chat_view, chat_state, rt_handle);
            }
            SkillSelectorAction::ConfigureSkills => {
                self.show_skill_config_selector(chat_view, chat_state, rt_handle);
            }
            SkillSelectorAction::Execute(selected) => {
                chat_view.hide_skill_selector();
                self.apply_skill_selection(&selected, chat_view);
            }
            SkillSelectorAction::Toggle(selected) => {
                self.set_skill_enabled(&selected, !selected.enabled, chat_state, rt_handle);
                self.show_skill_config_selector(chat_view, chat_state, rt_handle);
            }
        }
    }

    /// Apply skill selection: fill input box with execution command
    fn apply_skill_selection(&self, selected: &SkillItem, chat_view: &mut ChatView) {
        chat_view.set_input(&format!("Execute the {} skill.", selected.name));
    }

    fn set_skill_enabled(
        &self,
        selected: &SkillItem,
        enabled: bool,
        chat_state: &mut ChatState,
        rt_handle: &tokio::runtime::Handle,
    ) {
        let workspace = self.agent.workspace_path_buf();
        let mode_id = self.agent_type.clone();
        let skill = selected.clone();

        let result: Result<(), String> = tokio::task::block_in_place(|| {
            rt_handle.block_on(async {
                match skill.level.as_str() {
                    "user" => {
                        set_user_mode_skill_state(
                            &mode_id,
                            &skill.key,
                            enabled,
                            skill.default_enabled,
                        )
                        .await
                        .map_err(|error| error.to_string())?;
                    }
                    "project" => {
                        let mut document = load_project_mode_skills_document_local(&workspace)
                            .await
                            .map_err(|error| error.to_string())?;
                        set_mode_skill_disabled_in_document(
                            &mut document,
                            &mode_id,
                            &skill.key,
                            !enabled,
                        )
                        .map_err(|error| error.to_string())?;
                        save_project_mode_skills_document_local(&workspace, &document)
                            .await
                            .map_err(|error| error.to_string())?;
                    }
                    other => {
                        return Err(format!("Unsupported skill level '{}'", other));
                    }
                }

                Ok(())
            })
        });

        match result {
            Ok(()) => chat_state.add_system_message(format!(
                "Skill '{}' {} for mode '{}'.",
                selected.name,
                if enabled { "enabled" } else { "disabled" },
                self.agent_type
            )),
            Err(error) => chat_state.add_system_message(format!(
                "Failed to update skill '{}': {}",
                selected.name, error
            )),
        }
    }

    fn skill_item_from_info(info: SkillInfo) -> SkillItem {
        SkillItem {
            key: info.key,
            name: info.name,
            description: info.description,
            level: info.level.as_str().to_string(),
            enabled: true,
            selected_for_runtime: true,
            default_enabled: true,
            is_shadowed: info.is_shadowed,
        }
    }

    fn skill_item_from_mode_info(info: ModeSkillInfo) -> SkillItem {
        SkillItem {
            key: info.skill.key,
            name: info.skill.name,
            description: info.skill.description,
            level: info.skill.level.as_str().to_string(),
            enabled: info.effective_enabled,
            selected_for_runtime: info.selected_for_runtime,
            default_enabled: info.default_enabled,
            is_shadowed: info.skill.is_shadowed,
        }
    }

    /// Show subagent list/configuration menu.
    fn show_subagent_selector(
        &self,
        chat_view: &mut ChatView,
        _chat_state: &mut ChatState,
        _rt_handle: &tokio::runtime::Handle,
    ) {
        chat_view.show_subagent_menu();
    }

    fn show_available_subagent_list(
        &self,
        chat_view: &mut ChatView,
        chat_state: &mut ChatState,
        rt_handle: &tokio::runtime::Handle,
    ) {
        let registry = get_agent_registry();
        let subagents = tokio::task::block_in_place(|| {
            let workspace = self.agent.workspace_path_buf();
            let agent_type = self.agent_type.clone();
            rt_handle.block_on(registry.get_subagents_for_query(&SubagentQueryContext {
                parent_agent_type: Some(&agent_type),
                workspace_root: Some(workspace.as_path()),
                list_scope: SubagentListScope::TaskVisible,
                include_disabled: false,
            }))
        });

        if subagents.is_empty() {
            chat_state.add_system_message(format!(
                "No enabled subagents found for agent mode '{}'.",
                self.agent_type
            ));
            return;
        }

        let subagent_items: Vec<SubagentItem> = subagents
            .into_iter()
            .map(Self::subagent_item_from_info)
            .collect();

        if subagent_items.is_empty() {
            chat_state.add_system_message("No subagents found.".to_string());
            return;
        }

        chat_view.show_subagent_list(subagent_items);
    }

    fn show_subagent_config_selector(
        &self,
        chat_view: &mut ChatView,
        chat_state: &mut ChatState,
        rt_handle: &tokio::runtime::Handle,
    ) {
        let registry = get_agent_registry();
        let subagents = tokio::task::block_in_place(|| {
            let workspace = self.agent.workspace_path_buf();
            let agent_type = self.agent_type.clone();
            rt_handle.block_on(registry.get_subagents_for_query(&SubagentQueryContext {
                parent_agent_type: Some(&agent_type),
                workspace_root: Some(workspace.as_path()),
                list_scope: SubagentListScope::RegistryManagement,
                include_disabled: true,
            }))
        });

        let subagent_items: Vec<SubagentItem> = subagents
            .into_iter()
            .map(Self::subagent_item_from_info)
            .collect();

        if subagent_items.is_empty() {
            chat_state.add_system_message("No subagents found.".to_string());
            return;
        }

        chat_view.show_subagent_config(subagent_items);
    }

    fn handle_subagent_selector_action(
        &self,
        action: SubagentSelectorAction,
        chat_view: &mut ChatView,
        chat_state: &mut ChatState,
        rt_handle: &tokio::runtime::Handle,
    ) {
        match action {
            SubagentSelectorAction::ListSubagents => {
                self.show_available_subagent_list(chat_view, chat_state, rt_handle);
            }
            SubagentSelectorAction::ConfigureSubagents => {
                self.show_subagent_config_selector(chat_view, chat_state, rt_handle);
            }
            SubagentSelectorAction::Launch(selected) => {
                chat_view.hide_subagent_selector();
                self.apply_subagent_selection(&selected, chat_view);
            }
            SubagentSelectorAction::Toggle(selected) => {
                self.set_subagent_enabled(&selected, !selected.enabled, chat_state, rt_handle);
                self.show_subagent_config_selector(chat_view, chat_state, rt_handle);
            }
        }
    }

    /// Apply subagent selection: fill input box with launch command
    fn apply_subagent_selection(&self, selected: &SubagentItem, chat_view: &mut ChatView) {
        chat_view.set_input(&format!(
            "Launch subagent {} to finish task: ",
            selected.name
        ));
    }

    fn set_subagent_enabled(
        &self,
        selected: &SubagentItem,
        enabled: bool,
        chat_state: &mut ChatState,
        rt_handle: &tokio::runtime::Handle,
    ) {
        let registry = get_agent_registry();
        let workspace = self.agent.workspace_path_buf();
        let mode_id = self.agent_type.clone();
        let subagent = selected.clone();

        let result: Result<(), String> = tokio::task::block_in_place(|| {
            rt_handle.block_on(async {
                registry
                    .update_subagent_override(
                        &mode_id,
                        &subagent.id,
                        enabled,
                        Some(workspace.as_path()),
                    )
                    .await
                    .map_err(|error| error.to_string())
            })
        });

        match result {
            Ok(()) => chat_state.add_system_message(format!(
                "Subagent '{}' {} for mode '{}'.",
                selected.name,
                if enabled { "enabled" } else { "disabled" },
                self.agent_type
            )),
            Err(error) => chat_state.add_system_message(format!(
                "Failed to update subagent '{}': {}",
                selected.name, error
            )),
        }
    }

    fn subagent_item_from_info(info: AgentInfo) -> SubagentItem {
        let source = match info.subagent_source {
            Some(SubAgentSource::Builtin) => "builtin",
            Some(SubAgentSource::Project) => "project",
            Some(SubAgentSource::User) => "user",
            None => "builtin",
        }
        .to_string();

        SubagentItem {
            key: info.key,
            id: info.id,
            name: info.name,
            description: info.description,
            source,
            enabled: info.effective_enabled,
        }
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
            rt_handle.block_on(async { agent.list_sessions().await.unwrap_or_default() })
        });

        if sessions.is_empty() {
            chat_state.add_system_message("No sessions found.".to_string());
            return;
        }

        let session_items: Vec<SessionItem> = sessions
            .into_iter()
            .map(|s| {
                let last_activity = {
                    let last_activity =
                        std::time::UNIX_EPOCH + Duration::from_millis(s.last_active_at_ms);
                    let elapsed = last_activity.elapsed().unwrap_or_default();
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
            rt_handle.block_on(async { agent.delete_session(&sid).await })
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
            chat_state.current_model_name = format!("{} / {}", result.model_name, result.name);
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
            chat_state.current_model_name = format!("{} / {}", result.model_name, result.name);
            tracing::info!("Updated AI model: {}", model_id);
        } else {
            chat_view.set_status(Some("Failed to update model".to_string()));
        }
    }
}

#[cfg(test)]
mod tests {
    use tokio::sync::broadcast::error::TryRecvError;

    use super::{agent_event_stream_failure, mark_active_turn_failed};
    use crate::actions::{ActionState, ResolvedKeymap};
    use crate::chat_state::ChatState;
    use crate::config::ShortcutsConfig;

    #[test]
    fn agent_event_stream_failure_ignores_empty_queue() {
        assert_eq!(agent_event_stream_failure(TryRecvError::Empty), None);
    }

    #[test]
    fn agent_event_stream_failure_treats_lagged_and_closed_as_fatal() {
        let lagged = agent_event_stream_failure(TryRecvError::Lagged(7))
            .expect("lagged stream must be fatal");
        assert!(lagged.contains("lagged by 7 events"));
        assert!(lagged.contains("can no longer be trusted"));

        let closed =
            agent_event_stream_failure(TryRecvError::Closed).expect("closed stream must be fatal");
        assert!(closed.contains("closed"));
        assert!(closed.contains("can no longer be trusted"));
    }

    #[test]
    fn agent_event_stream_failure_marks_active_turn_failed() {
        let mut state = ChatState::new(
            "session".to_string(),
            "Session".to_string(),
            "agentic".to_string(),
            Some("D:/workspace/current".to_string()),
        );
        state.handle_turn_started("turn", "hello");

        assert!(mark_active_turn_failed(
            &mut state,
            "Agent event stream closed; chat state can no longer be trusted"
        ));
        assert_eq!(state.current_turn_id(), None);
        assert!(!state.is_processing);
    }

    #[test]
    fn shortcut_registry_contract_help_uses_resolved_keymap() {
        let keymap = ResolvedKeymap::new(&ShortcutsConfig::default());

        let help = keymap.help_text(ActionState::chat(false, false));
        assert!(help.contains("Ctrl+P"));
        assert!(help.contains("Command Palette"));
    }
}
