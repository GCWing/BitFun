//! Shared command router for IM-bot connections (Telegram / Feishu / WeChat).
//!
//! All user-facing menu/command logic lives here.  Platform adapters only
//! handle message I/O and render the platform-agnostic [`HandleResult`] /
//! [`crate::service::remote_connect::bot::menu::MenuView`] returned from
//! [`handle_command`].
//!
//! Public surface kept stable so existing adapters keep compiling:
//!   - Types: `BotChatState`, `BotCommand`, `BotAction`, `BotActionStyle`,
//!     `BotInteractiveRequest`, `BotInteractionHandler`, `BotMessageSender`,
//!     `BotQuestion`, `BotQuestionOption`, `BotDisplayMode`, `BotLanguage`,
//!     `HandleResult`, `ForwardRequest`, `ForwardedTurnResult`, `PendingAction`.
//!   - Functions: `parse_command`, `handle_command`, `welcome_message`,
//!     `complete_im_bot_pairing`, `current_bot_language`,
//!     `execute_forwarded_turn`, `apply_interactive_request`.

use log::{error, info};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

pub use super::locale::{current_bot_language, BotLanguage};
use super::locale::{fmt_count, strings_for, BotStrings};
use super::menu::{MenuItem, MenuItemStyle, MenuView};

// ── Constants ──────────────────────────────────────────────────────

/// How long a pending interactive prompt stays valid before auto-clearing.
const PENDING_TTL_SECS: i64 = 5 * 60;
/// How many invalid replies are tolerated before pending state is auto-cleared.
const PENDING_INVALID_LIMIT: u8 = 3;

// ── Per-chat state ─────────────────────────────────────────────────

/// Display mode for IM bot sessions.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum BotDisplayMode {
    /// Expert mode: can create Code / Cowork sessions on real workspaces.
    #[serde(rename = "pro")]
    Pro,
    /// Default assistant mode: Claw sessions on the assistant workspace.
    #[serde(rename = "assistant")]
    #[default]
    Assistant,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BotChatState {
    pub chat_id: String,
    pub paired: bool,
    pub current_workspace: Option<String>,
    pub current_assistant: Option<String>,
    /// Human-readable name of the active assistant (e.g. "默认助理" / "Bob").
    /// Populated alongside `current_assistant` from `WorkspaceInfo.name` so
    /// the assistant-mode menu body can show a meaningful label instead of
    /// the workspace directory name (which is often a generic
    /// "workspace" / "workspace-<uuid>" folder).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_assistant_name: Option<String>,
    pub current_session_id: Option<String>,
    #[serde(default)]
    pub display_mode: BotDisplayMode,

    /// Active interactive prompt awaiting a user reply.
    /// Not persisted — cleared on bot restart.
    #[serde(skip)]
    pub pending_action: Option<PendingAction>,
    /// Unix timestamp (seconds) when the current `pending_action` becomes
    /// invalid.  Refreshed whenever a new pending action is set.
    #[serde(skip)]
    pub pending_expires_at: i64,
    /// How many invalid replies the user has sent against the current
    /// pending action.  Resets on every successful transition.
    #[serde(skip)]
    pub pending_invalid_count: u8,

    /// Commands corresponding to the items in the most recent menu, used so
    /// numeric replies (`1` ~ `last_menu_commands.len()`) work without
    /// platform-native buttons.  Not persisted.
    #[serde(skip, default)]
    pub last_menu_commands: Vec<String>,
}

impl BotChatState {
    pub fn new(chat_id: String) -> Self {
        Self {
            chat_id,
            paired: false,
            current_workspace: None,
            current_assistant: None,
            current_assistant_name: None,
            current_session_id: None,
            display_mode: BotDisplayMode::Assistant,
            pending_action: None,
            pending_expires_at: 0,
            pending_invalid_count: 0,
            last_menu_commands: Vec::new(),
        }
    }

    /// Returns the workspace root path that should be used to resolve relative
    /// file references emitted by the agent (e.g. markdown links in replies).
    ///
    /// In Pro mode this is the explicitly switched workspace
    /// (`current_workspace`); in Assistant mode the agent runs against the
    /// per-user assistant workspace held in `current_assistant`. IM platform
    /// adapters MUST consult both — looking only at `current_workspace` causes
    /// auto-push to silently drop relative-path attachments produced by
    /// assistant sessions (the most common case for end users).
    pub fn active_workspace_path(&self) -> Option<String> {
        self.current_workspace
            .clone()
            .or_else(|| self.current_assistant.clone())
    }

    fn set_pending(&mut self, action: PendingAction) {
        self.pending_action = Some(action);
        self.pending_expires_at = now_secs() + PENDING_TTL_SECS;
        self.pending_invalid_count = 0;
    }

    fn clear_pending(&mut self) {
        self.pending_action = None;
        self.pending_expires_at = 0;
        self.pending_invalid_count = 0;
    }

    fn pending_expired(&self) -> bool {
        self.pending_action.is_some() && now_secs() > self.pending_expires_at
    }
}

fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

// ── Pending action ─────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum PendingAction {
    SelectWorkspace {
        options: Vec<(String, String)>,
    },
    SelectAssistant {
        options: Vec<(String, String)>,
    },
    SelectSession {
        options: Vec<(String, String)>,
        page: usize,
        has_more: bool,
    },
    AskUserQuestion {
        tool_id: String,
        questions: Vec<BotQuestion>,
        current_index: usize,
        answers: Vec<Value>,
        awaiting_custom_text: bool,
        pending_answer: Option<Value>,
    },
    /// Confirm switching to the other display mode and then run `target_cmd`.
    ConfirmModeSwitch {
        target_mode: BotDisplayMode,
        target_cmd: String,
    },
}

// ── Question DTOs ──────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BotQuestionOption {
    pub label: String,
    #[serde(default)]
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BotQuestion {
    #[serde(default)]
    pub question: String,
    #[serde(default)]
    pub header: String,
    #[serde(default)]
    pub options: Vec<BotQuestionOption>,
    #[serde(rename = "multiSelect", default)]
    pub multi_select: bool,
}

// ── Action / handle result (compat surface) ────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BotActionStyle {
    Primary,
    Default,
}

#[derive(Debug, Clone)]
pub struct BotAction {
    pub label: String,
    pub command: String,
    pub style: BotActionStyle,
}

impl BotAction {
    pub fn primary(label: impl Into<String>, command: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            command: command.into(),
            style: BotActionStyle::Primary,
        }
    }
    pub fn secondary(label: impl Into<String>, command: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            command: command.into(),
            style: BotActionStyle::Default,
        }
    }
}

impl From<MenuItem> for BotAction {
    fn from(item: MenuItem) -> Self {
        let style = match item.style {
            MenuItemStyle::Primary => BotActionStyle::Primary,
            // Danger and Default both map to non-primary on platforms that
            // don't have a native danger style.
            _ => BotActionStyle::Default,
        };
        BotAction {
            label: item.label,
            command: item.command,
            style,
        }
    }
}

pub struct HandleResult {
    pub reply: String,
    pub actions: Vec<BotAction>,
    pub forward_to_session: Option<ForwardRequest>,
    /// Same content as [`MenuView`] — adapters that want to render a richer
    /// view (Telegram inline keyboard, Feishu card, WeChat numbered text)
    /// can read this directly instead of `actions`.
    pub menu: MenuView,
}

#[derive(Debug, Clone)]
pub struct BotInteractiveRequest {
    pub reply: String,
    pub actions: Vec<BotAction>,
    pub menu: MenuView,
    pub pending_action: PendingAction,
}

pub type BotInteractionHandler =
    Arc<dyn Fn(BotInteractiveRequest) -> Pin<Box<dyn Future<Output = ()> + Send>> + Send + Sync>;

pub type BotMessageSender =
    Arc<dyn Fn(String) -> Pin<Box<dyn Future<Output = ()> + Send>> + Send + Sync>;

pub struct ForwardRequest {
    pub session_id: String,
    pub content: String,
    pub agent_type: String,
    pub turn_id: String,
    pub image_contexts: Vec<crate::agentic::image_analysis::ImageContextData>,
}

pub struct ForwardedTurnResult {
    pub display_text: String,
    pub full_text: String,
}

// ── BotCommand ─────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BotCommand {
    /// Show welcome (unpaired) or main menu (paired).  Triggered by
    /// `/start`, `/menu`, `/m`, `菜单`, or `0` at the top level.
    Menu,
    /// Show settings sub-menu.
    Settings,
    /// Show help text.
    Help,
    /// Switch display mode.
    SwitchMode(BotDisplayMode),
    /// Toggle verbose execution-detail mode (persisted globally).
    SetVerbose(bool),
    /// Generic "switch" entry — picks workspace or assistant by mode.
    SwitchContext,
    /// Generic "new session" entry — picks the right session type by mode.
    NewSession,
    /// Specific session creators (kept as hidden aliases).
    NewCodeSession,
    NewCoworkSession,
    NewClawSession,
    /// Resume an existing session (workspace or assistant by mode).
    ResumeSession,
    /// Cancel currently running task.
    CancelTask(Option<String>),
    /// Pairing code submitted before pairing.
    PairingCode(String),
    /// Numeric reply to a menu / pending action.
    NumberSelection(usize),
    /// Free-form chat message forwarded to the AI session.
    ChatMessage(String),
}

// ── Command parsing ────────────────────────────────────────────────

fn normalize_im_command_text(text: &str) -> String {
    text.trim()
        .chars()
        .map(|c| match c {
            '\u{FF10}'..='\u{FF19}' => {
                char::from_u32(c as u32 - 0xFF10 + u32::from(b'0')).unwrap_or(c)
            }
            c => c,
        })
        .collect()
}

fn strip_numeric_reply_suffix(s: &str) -> &str {
    s.trim_end_matches(|c: char| {
        matches!(
            c,
            '.' | '。' | '、' | ',' | '，' | ':' | '：' | ';' | '；' | ')' | '）' | ']' | '】'
        )
    })
    .trim()
}

pub fn parse_command(text: &str) -> BotCommand {
    let normalized = normalize_im_command_text(text);
    let trimmed = normalized.trim();
    if let Some(rest) = trimmed.strip_prefix("/cancel_task") {
        let arg = rest.trim();
        return if arg.is_empty() {
            BotCommand::CancelTask(None)
        } else {
            BotCommand::CancelTask(Some(arg.to_string()))
        };
    }
    if let Some(rest) = trimmed.strip_prefix("/cancel") {
        let arg = rest.trim();
        return if arg.is_empty() {
            BotCommand::CancelTask(None)
        } else {
            BotCommand::CancelTask(Some(arg.to_string()))
        };
    }
    let lower = trimmed.to_ascii_lowercase();
    match lower.as_str() {
        // Top-level navigation / settings.
        "/start" | "/menu" | "/m" | "菜单" => return BotCommand::Menu,
        "/settings" | "/s" | "设置" => return BotCommand::Settings,
        "/help" | "/?" | "/h" | "帮助" | "？" => return BotCommand::Help,

        // Mode switches (visible).
        "/expert" | "/pro" | "专业模式" => {
            return BotCommand::SwitchMode(BotDisplayMode::Pro);
        }
        "/assistant" | "助理模式" => {
            return BotCommand::SwitchMode(BotDisplayMode::Assistant);
        }

        // Verbose toggles.
        "/verbose" | "详细" => return BotCommand::SetVerbose(true),
        "/concise" | "简洁" => return BotCommand::SetVerbose(false),

        // Generic switch (picks workspace or assistant by mode).
        "/switch" | "切换" => return BotCommand::SwitchContext,
        // Hidden aliases.
        "/switch_workspace" | "切换工作区" => return BotCommand::SwitchContext,
        "/switch_assistant" | "切换助理" => return BotCommand::SwitchContext,

        // Generic "new" picks the right session type by mode.
        "/new" | "/n" | "新建" | "新建会话" | "新会话" => return BotCommand::NewSession,
        // Hidden aliases / power users.
        "/new_code_session" | "新建编码会话" => return BotCommand::NewCodeSession,
        "/new_cowork_session" | "新建协作会话" => {
            return BotCommand::NewCoworkSession;
        }
        "/new_claw_session" | "新建助理会话" => return BotCommand::NewClawSession,

        // Resume.
        "/resume" | "/r" | "/resume_session" | "恢复" | "恢复会话" => {
            return BotCommand::ResumeSession;
        }
        _ => {}
    }

    if trimmed.len() == 6 && trimmed.chars().all(|c| c.is_ascii_digit()) {
        return BotCommand::PairingCode(trimmed.to_string());
    }

    let num_token = strip_numeric_reply_suffix(trimmed);
    if let Ok(n) = num_token.parse::<usize>() {
        if n <= 99 {
            // `0` is intentionally returned as `NumberSelection(0)` so context
            // such as "next page" inside SelectSession can override the
            // default "0 = back to menu" interpretation.  See `handle_number`.
            return BotCommand::NumberSelection(n);
        }
    }
    BotCommand::ChatMessage(trimmed.to_string())
}

// ── Public welcome / help text (compat) ───────────────────────────

pub fn welcome_message(language: BotLanguage) -> &'static str {
    strings_for(language).welcome
}

// ── MenuView -> HandleResult helpers ───────────────────────────────

fn result_from_menu(state: &mut BotChatState, view: MenuView) -> HandleResult {
    let actions: Vec<BotAction> = view.items.iter().cloned().map(BotAction::from).collect();
    state.last_menu_commands = view.numeric_commands();
    HandleResult {
        reply: view.render_text_block(),
        actions,
        forward_to_session: None,
        menu: view,
    }
}

fn result_from_menu_with_forward(
    state: &mut BotChatState,
    view: MenuView,
    forward: Option<ForwardRequest>,
) -> HandleResult {
    let mut r = result_from_menu(state, view);
    r.forward_to_session = forward;
    r
}

// ── Menu builders ──────────────────────────────────────────────────

fn welcome_view(s: &'static BotStrings) -> MenuView {
    MenuView::plain(s.welcome_title)
        .with_body(s.welcome)
        .with_footer(s.welcome_body)
}

fn ready_to_chat_body(state: &BotChatState, s: &'static BotStrings) -> Option<String> {
    // Always show the workspace / assistant name (a human-meaningful
    // identifier) regardless of whether a session is active. We deliberately
    // do NOT surface `current_session_id` — the random UUID tail (e.g.
    // "5cff6a1") is opaque to the user and adds nothing useful. If the
    // user wants to manage sessions they can use /resume which renders
    // proper session names.
    if state.display_mode == BotDisplayMode::Pro {
        match &state.current_workspace {
            Some(p) => Some(format!(
                "{}: {}",
                s.current_workspace_label,
                short_path_name(p)
            )),
            None => Some(s.no_workspace.to_string()),
        }
    } else {
        // Assistant mode: prefer the cached assistant display name (set by
        // pairing / switch / resume flows from `WorkspaceInfo.name`). The
        // workspace path's directory name is meaningless here — the actual
        // assistant folder is usually `workspace` or `workspace-<uuid>`,
        // both of which look like noise to the user.
        match &state.current_assistant {
            Some(p) => {
                let label = state
                    .current_assistant_name
                    .as_deref()
                    .filter(|n| !n.trim().is_empty())
                    .map(|n| n.to_string())
                    .unwrap_or_else(|| short_path_name(p));
                Some(format!("{}: {}", s.current_assistant_label, label))
            }
            None => Some(s.no_assistant.to_string()),
        }
    }
}

/// One-shot lookup that fills in `current_assistant_name` from the workspace
/// service when the chat state has an `current_assistant` path but no cached
/// display name (e.g. the state was persisted before the field was added).
/// Best-effort: silently no-ops if the workspace service is unavailable or
/// the path is not a known assistant workspace.
async fn refresh_assistant_name_if_missing(state: &mut BotChatState) {
    use crate::service::workspace::get_global_workspace_service;
    if state.current_assistant_name.is_some() {
        return;
    }
    let Some(path) = state.current_assistant.clone() else {
        return;
    };
    let Some(svc) = get_global_workspace_service() else {
        return;
    };
    let workspaces = svc.get_assistant_workspaces().await;
    if let Some(ws) = workspaces
        .into_iter()
        .find(|w| w.root_path.to_string_lossy() == path)
    {
        state.current_assistant_name = Some(ws.name);
    }
}

fn short_path_name(path: &str) -> String {
    std::path::Path::new(path)
        .file_name()
        .and_then(|n| n.to_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| path.to_string())
}

fn main_menu_view(state: &BotChatState, s: &'static BotStrings) -> MenuView {
    let title = if state.display_mode == BotDisplayMode::Pro {
        s.main_title_expert
    } else {
        s.main_title_assistant
    };
    let body = ready_to_chat_body(state, s);
    let mut items: Vec<MenuItem> = Vec::new();
    if state.display_mode == BotDisplayMode::Pro {
        items.push(MenuItem::primary(
            s.item_new_code_session,
            "/new_code_session",
        ));
        items.push(MenuItem::default(
            s.item_new_cowork_session,
            "/new_cowork_session",
        ));
        items.push(MenuItem::default(s.item_resume_session, "/resume"));
        items.push(MenuItem::default(s.item_switch_workspace, "/switch"));
    } else {
        items.push(MenuItem::primary(s.item_new_session, "/new"));
        items.push(MenuItem::default(s.item_resume_session, "/resume"));
        items.push(MenuItem::default(s.item_switch_assistant, "/switch"));
    }
    items.push(MenuItem::default(s.item_settings, "/settings"));
    let mut view = MenuView::plain(title).with_items(items);
    if let Some(b) = body {
        view = view.with_body(b);
    }
    view
}

fn settings_menu_view(verbose: bool, state: &BotChatState, s: &'static BotStrings) -> MenuView {
    let mut items: Vec<MenuItem> = Vec::new();
    if state.display_mode == BotDisplayMode::Pro {
        items.push(MenuItem::default(s.item_switch_to_assistant, "/assistant"));
    } else {
        items.push(MenuItem::default(s.item_switch_to_expert, "/expert"));
    }
    if verbose {
        items.push(MenuItem::default(s.item_verbose_off, "/concise"));
    } else {
        items.push(MenuItem::default(s.item_verbose_on, "/verbose"));
    }
    items.push(MenuItem::default(s.item_help, "/help"));
    items.push(MenuItem::default(s.item_back, "/menu"));
    let body = format!(
        "{} · {}: {}",
        if state.display_mode == BotDisplayMode::Pro {
            s.mode_expert
        } else {
            s.mode_assistant
        },
        s.verbose_label,
        if verbose {
            s.verbose_status_on
        } else {
            s.verbose_status_off
        },
    );
    MenuView::plain(s.settings_title)
        .with_body(body)
        .with_items(items)
}

fn need_session_view(state: &BotChatState, s: &'static BotStrings) -> MenuView {
    let mut items = Vec::new();
    if state.display_mode == BotDisplayMode::Pro {
        items.push(MenuItem::primary(
            s.item_new_code_session,
            "/new_code_session",
        ));
        items.push(MenuItem::default(
            s.item_new_cowork_session,
            "/new_cowork_session",
        ));
    } else {
        items.push(MenuItem::primary(s.item_new_session, "/new"));
    }
    items.push(MenuItem::default(s.item_resume_session, "/resume"));
    items.push(MenuItem::default(s.item_back, "/menu"));
    MenuView::plain(s.need_session_title).with_items(items)
}

fn confirm_mode_switch_view(target_mode: BotDisplayMode, s: &'static BotStrings) -> MenuView {
    let target_label = if target_mode == BotDisplayMode::Pro {
        s.mode_expert
    } else {
        s.mode_assistant
    };
    let body = format!("{} → {}", s.mode_confirm_switch_prefix, target_label);
    MenuView::plain(s.settings_title)
        .with_body(body)
        .with_items(vec![
            MenuItem::primary(s.item_confirm_switch, "1"),
            MenuItem::default(s.item_back, "/menu"),
        ])
}

// ── Public entry points ────────────────────────────────────────────

/// IM pairing bootstrap: assistant mode + default assistant workspace + new
/// Claw session.  Mutates `state.display_mode/current_assistant/
/// current_session_id` on success.
pub async fn bootstrap_im_chat_after_pairing(state: &mut BotChatState) -> String {
    use crate::service::workspace::get_global_workspace_service;

    state.display_mode = BotDisplayMode::Assistant;
    let language = current_bot_language().await;
    let s = strings_for(language);

    let ws_service = match get_global_workspace_service() {
        Some(s) => s,
        None => return s.bootstrap_workspace_unavailable.to_string(),
    };

    let mut assistants = ws_service.get_assistant_workspaces().await;
    if assistants.is_empty() {
        match ws_service.create_assistant_workspace(None).await {
            Ok(w) => assistants.push(w),
            Err(e) => return format!("{}{e}", s.assistant_create_failed_prefix),
        }
    }

    let picked = assistants
        .iter()
        .find(|w| w.assistant_id.is_none())
        .cloned()
        .or_else(|| assistants.first().cloned());

    let Some(ws_info) = picked else {
        return s.bootstrap_workspace_unavailable.to_string();
    };

    let path_buf = ws_info.root_path.clone();
    if let Err(e) = ws_service.open_workspace(path_buf.clone()).await {
        return format!("{}{e}", s.workspace_open_failed_prefix);
    }
    if let Err(e) =
        crate::service::snapshot::initialize_snapshot_manager_for_workspace(path_buf, None).await
    {
        error!("IM bot bootstrap: snapshot init after pairing: {e}");
    }

    state.current_assistant = Some(ws_info.root_path.to_string_lossy().to_string());
    state.current_assistant_name = Some(ws_info.name.clone());
    state.current_session_id = None;

    let create_res = create_session(state, "Claw").await;
    if state.current_session_id.is_none() {
        let detail = create_res.reply.lines().next().unwrap_or("").to_string();
        return format!("{}{detail}", s.bootstrap_session_failed_prefix);
    }

    s.bootstrap_ready.to_string()
}

/// Mark chat paired, run assistant/session bootstrap, return main menu.
pub async fn complete_im_bot_pairing(state: &mut BotChatState) -> HandleResult {
    state.paired = true;
    let language = current_bot_language().await;
    let s = strings_for(language);
    let note = bootstrap_im_chat_after_pairing(state).await;

    let mut view = main_menu_view(state, s);
    let combined_body = match view.body.take() {
        Some(b) => format!("{}\n\n{}\n\n{}", s.paired_success, note, b),
        None => format!("{}\n\n{}", s.paired_success, note),
    };
    view = view.with_body(combined_body);
    result_from_menu(state, view)
}

/// Public adapter helper: install an interactive request received from the
/// session executor onto the chat state and refresh its TTL.
pub fn apply_interactive_request(state: &mut BotChatState, req: &BotInteractiveRequest) {
    state.set_pending(req.pending_action.clone());
    state.last_menu_commands = req.menu.items.iter().map(|i| i.command.clone()).collect();
}

// ── Dispatch ───────────────────────────────────────────────────────

pub async fn handle_command(
    state: &mut BotChatState,
    cmd: BotCommand,
    images: Vec<super::super::remote_server::ImageAttachment>,
) -> HandleResult {
    let image_contexts: Vec<crate::agentic::image_analysis::ImageContextData> =
        super::super::remote_server::images_to_contexts(if images.is_empty() {
            None
        } else {
            Some(&images)
        });
    dispatch(state, cmd, image_contexts).await
}

async fn dispatch(
    state: &mut BotChatState,
    cmd: BotCommand,
    image_contexts: Vec<crate::agentic::image_analysis::ImageContextData>,
) -> HandleResult {
    let language = current_bot_language().await;
    let s = strings_for(language);

    // Auto-expire pending actions before any branch.
    if state.pending_expired() {
        state.clear_pending();
        let mut view = main_menu_view(state, s);
        view = view.with_body(s.pending_expired);
        return result_from_menu(state, view);
    }

    // Universal escape hatches: /menu and /start always return the main menu
    // and clear any pending action.
    if matches!(cmd, BotCommand::Menu) {
        state.clear_pending();
        return menu_or_welcome(state, s);
    }

    // Pairing-code submitted after pairing already completed → just nudge.
    if let BotCommand::PairingCode(_) = &cmd {
        if state.paired {
            let view = MenuView::plain(s.main_title_assistant)
                .with_body(s.paired_success)
                .with_items(main_menu_view(state, s).items);
            return result_from_menu(state, view);
        }
        // Not paired path is handled by the platform wait_for_pairing loop.
    }

    if !state.paired {
        return result_from_menu(state, welcome_view(s));
    }

    // Lazily resolve `current_assistant_name` for chat states that were
    // persisted before this field existed. Without this, already-paired
    // users would keep seeing the workspace folder name (e.g. "workspace")
    // until they manually re-switch assistants.
    refresh_assistant_name_if_missing(state).await;

    // Handle /cancel as task cancellation when an active session exists.
    if let BotCommand::CancelTask(turn_id) = &cmd {
        return handle_cancel_task(state, turn_id.as_deref(), s).await;
    }

    // Numeric replies: when there is a pending action, route to it.  When
    // there isn't, treat the number as an index into `last_menu_commands`.
    if let BotCommand::NumberSelection(n) = cmd {
        return handle_number(state, n, s).await;
    }

    match cmd {
        BotCommand::Help => result_from_menu(
            state,
            MenuView::plain(s.welcome_title)
                .with_body(s.help_body)
                .with_items(vec![MenuItem::default(s.item_back, "/menu")]),
        ),
        BotCommand::Settings => {
            let verbose = super::load_bot_persistence().verbose_mode;
            result_from_menu(state, settings_menu_view(verbose, state, s))
        }
        BotCommand::SwitchMode(target) => switch_mode(state, target, s).await,
        BotCommand::SetVerbose(on) => set_verbose(state, on, s).await,
        BotCommand::SwitchContext => start_switch(state, s).await,
        BotCommand::NewSession => new_session_for_mode(state, s).await,
        BotCommand::NewCodeSession => guarded_new(state, "agentic", s).await,
        BotCommand::NewCoworkSession => guarded_new(state, "Cowork", s).await,
        BotCommand::NewClawSession => guarded_new(state, "Claw", s).await,
        BotCommand::ResumeSession => start_resume(state, 0, s).await,
        BotCommand::ChatMessage(msg) => handle_chat(state, &msg, image_contexts, s).await,
        BotCommand::Menu
        | BotCommand::CancelTask(_)
        | BotCommand::NumberSelection(_)
        | BotCommand::PairingCode(_) => menu_or_welcome(state, s), // already handled
    }
}

fn menu_or_welcome(state: &mut BotChatState, s: &'static BotStrings) -> HandleResult {
    if state.paired {
        result_from_menu(state, main_menu_view(state, s))
    } else {
        result_from_menu(state, welcome_view(s))
    }
}

// ── Mode switching ─────────────────────────────────────────────────

async fn switch_mode(
    state: &mut BotChatState,
    target: BotDisplayMode,
    s: &'static BotStrings,
) -> HandleResult {
    if state.display_mode == target {
        let body = if target == BotDisplayMode::Pro {
            s.mode_already_expert
        } else {
            s.mode_already_assistant
        };
        let mut view = main_menu_view(state, s);
        view = view.with_body(body);
        return result_from_menu(state, view);
    }
    state.display_mode = target;
    let body = if target == BotDisplayMode::Pro {
        s.mode_switched_to_expert
    } else {
        s.mode_switched_to_assistant
    };
    let mut view = main_menu_view(state, s);
    view = view.with_body(body);
    result_from_menu(state, view)
}

async fn confirm_then_run(
    state: &mut BotChatState,
    target: BotDisplayMode,
    target_cmd: String,
    s: &'static BotStrings,
) -> HandleResult {
    state.set_pending(PendingAction::ConfirmModeSwitch {
        target_mode: target,
        target_cmd,
    });
    result_from_menu(state, confirm_mode_switch_view(target, s))
}

async fn set_verbose(state: &mut BotChatState, on: bool, s: &'static BotStrings) -> HandleResult {
    let mut data = super::load_bot_persistence();
    data.verbose_mode = on;
    super::save_bot_persistence(&data);

    let body = if on {
        s.verbose_enabled
    } else {
        s.verbose_disabled
    };
    let mut view = settings_menu_view(on, state, s);
    view = view.with_body(body);
    result_from_menu(state, view)
}

// ── Switch context (workspace or assistant) ────────────────────────

async fn start_switch(state: &mut BotChatState, s: &'static BotStrings) -> HandleResult {
    use crate::service::workspace::get_global_workspace_service;

    let ws_service = match get_global_workspace_service() {
        Some(s) => s,
        None => {
            return result_from_menu(
                state,
                MenuView::plain(s.workspace_service_unavailable)
                    .with_items(vec![MenuItem::default(s.item_back, "/menu")]),
            );
        }
    };

    if state.display_mode == BotDisplayMode::Pro {
        let workspaces = ws_service.get_recent_workspaces().await;
        if workspaces.is_empty() {
            return result_from_menu(
                state,
                MenuView::plain(s.switch_no_workspaces)
                    .with_items(vec![MenuItem::default(s.item_back, "/menu")]),
            );
        }
        let options: Vec<(String, String)> = workspaces
            .iter()
            .map(|ws| (ws.root_path.to_string_lossy().to_string(), ws.name.clone()))
            .collect();
        let view = workspace_selection_view(state, &options, s);
        state.set_pending(PendingAction::SelectWorkspace { options });
        result_from_menu(state, view)
    } else {
        let assistants = ws_service.get_assistant_workspaces().await;
        if assistants.is_empty() {
            return result_from_menu(
                state,
                MenuView::plain(s.switch_no_assistants)
                    .with_items(vec![MenuItem::default(s.item_back, "/menu")]),
            );
        }
        let options: Vec<(String, String)> = assistants
            .iter()
            .map(|ws| (ws.root_path.to_string_lossy().to_string(), ws.name.clone()))
            .collect();
        let view = assistant_selection_view(state, &options, s);
        state.set_pending(PendingAction::SelectAssistant { options });
        result_from_menu(state, view)
    }
}

fn workspace_selection_view(
    state: &BotChatState,
    options: &[(String, String)],
    s: &'static BotStrings,
) -> MenuView {
    let mut items = Vec::new();
    let mut body = String::new();
    for (i, (path, name)) in options.iter().enumerate() {
        let is_current = state.current_workspace.as_deref() == Some(path.as_str());
        let marker = if is_current { s.current_marker } else { "" };
        body.push_str(&format!("{}. {}{}\n", i + 1, name, marker));
        items.push(MenuItem::default(
            truncate_label(name, 24),
            (i + 1).to_string(),
        ));
    }
    items.push(MenuItem::default(s.item_back, "/menu"));
    MenuView::plain(s.switch_pick_workspace)
        .with_body(body.trim_end().to_string())
        .with_items(items)
        .with_footer(s.footer_reply_workspace)
}

fn assistant_selection_view(
    state: &BotChatState,
    options: &[(String, String)],
    s: &'static BotStrings,
) -> MenuView {
    let mut items = Vec::new();
    let mut body = String::new();
    for (i, (path, name)) in options.iter().enumerate() {
        let is_current = state.current_assistant.as_deref() == Some(path.as_str());
        let marker = if is_current { s.current_marker } else { "" };
        body.push_str(&format!("{}. {}{}\n", i + 1, name, marker));
        items.push(MenuItem::default(
            truncate_label(name, 24),
            (i + 1).to_string(),
        ));
    }
    items.push(MenuItem::default(s.item_back, "/menu"));
    MenuView::plain(s.switch_pick_assistant)
        .with_body(body.trim_end().to_string())
        .with_items(items)
        .with_footer(s.footer_reply_assistant)
}

fn session_selection_view(
    state: &BotChatState,
    options: &[(String, String)],
    page: usize,
    has_more: bool,
    s: &'static BotStrings,
) -> MenuView {
    let mut items = Vec::new();
    let mut body = String::new();
    for (i, (id, name)) in options.iter().enumerate() {
        let is_current = state.current_session_id.as_deref() == Some(id.as_str());
        let marker = if is_current { s.current_marker } else { "" };
        body.push_str(&format!("{}. {}{}\n", i + 1, name, marker));
        items.push(MenuItem::default(
            truncate_label(name, 26),
            (i + 1).to_string(),
        ));
    }
    if has_more {
        items.push(MenuItem::default(s.item_next_page, "0"));
    }
    items.push(MenuItem::default(s.item_back, "/menu"));
    let footer = if has_more {
        s.footer_reply_session_or_next
    } else {
        s.footer_reply_session
    };
    MenuView::plain(format!("{} · #{}", s.resume_page_label, page + 1))
        .with_body(body.trim_end().to_string())
        .with_items(items)
        .with_footer(footer)
}

async fn select_workspace(
    state: &mut BotChatState,
    path: &str,
    name: &str,
    s: &'static BotStrings,
) -> HandleResult {
    use crate::service::workspace::get_global_workspace_service;

    let ws_service = match get_global_workspace_service() {
        Some(svc) => svc,
        None => {
            return result_from_menu(state, MenuView::plain(s.workspace_service_unavailable));
        }
    };
    let path_buf = std::path::PathBuf::from(path);
    match ws_service.open_workspace(path_buf).await {
        Ok(info) => {
            if let Err(e) = crate::service::snapshot::initialize_snapshot_manager_for_workspace(
                info.root_path.clone(),
                None,
            )
            .await
            {
                error!("Failed to init snapshot after bot workspace switch: {e}");
            }
            state.current_workspace = Some(path.to_string());
            state.current_session_id = None;
            info!("Bot switched workspace to: {path}");

            let session_count = count_workspace_sessions(path).await;
            let body = format!(
                "{}: {} · {}",
                s.current_workspace_label,
                name,
                fmt_count(s.workspace_session_count_fmt, session_count),
            );
            let mut view = main_menu_view(state, s);
            view = view.with_body(body);
            result_from_menu(state, view)
        }
        Err(e) => result_from_menu(
            state,
            MenuView::plain(format!("{}{e}", s.workspace_open_failed_prefix)),
        ),
    }
}

async fn select_assistant(
    state: &mut BotChatState,
    path: &str,
    name: &str,
    s: &'static BotStrings,
) -> HandleResult {
    use crate::service::workspace::get_global_workspace_service;

    let ws_service = match get_global_workspace_service() {
        Some(svc) => svc,
        None => {
            return result_from_menu(state, MenuView::plain(s.workspace_service_unavailable));
        }
    };
    let path_buf = std::path::PathBuf::from(path);
    match ws_service.open_workspace(path_buf).await {
        Ok(info) => {
            if let Err(e) = crate::service::snapshot::initialize_snapshot_manager_for_workspace(
                info.root_path.clone(),
                None,
            )
            .await
            {
                error!("Failed to init snapshot after bot assistant switch: {e}");
            }
            state.current_assistant = Some(path.to_string());
            state.current_assistant_name = Some(name.to_string());
            state.current_session_id = None;
            info!("Bot switched assistant to: {path}");

            let session_count = count_workspace_sessions(path).await;
            let body = format!(
                "{}: {} · {}",
                s.current_assistant_label,
                name,
                fmt_count(s.workspace_session_count_fmt, session_count),
            );
            let mut view = main_menu_view(state, s);
            view = view.with_body(body);
            result_from_menu(state, view)
        }
        Err(e) => result_from_menu(
            state,
            MenuView::plain(format!("{}{e}", s.workspace_open_failed_prefix)),
        ),
    }
}

async fn count_workspace_sessions(workspace_path: &str) -> usize {
    use crate::agentic::persistence::PersistenceManager;
    use crate::infrastructure::PathManager;

    let wp = std::path::PathBuf::from(workspace_path);
    let pm = match PathManager::new() {
        Ok(pm) => std::sync::Arc::new(pm),
        Err(_) => return 0,
    };
    let store = match PersistenceManager::new(pm) {
        Ok(store) => store,
        Err(_) => return 0,
    };
    store
        .list_session_metadata(&wp)
        .await
        .map(|v| v.len())
        .unwrap_or(0)
}

fn truncate_label(label: &str, max_chars: usize) -> String {
    let trimmed = label.trim();
    if trimmed.chars().count() <= max_chars {
        trimmed.to_string()
    } else {
        let truncated: String = trimmed.chars().take(max_chars.saturating_sub(1)).collect();
        format!("{truncated}…")
    }
}

// ── Resume / new session ──────────────────────────────────────────

async fn start_resume(
    state: &mut BotChatState,
    page: usize,
    s: &'static BotStrings,
) -> HandleResult {
    use crate::agentic::persistence::PersistenceManager;
    use crate::infrastructure::PathManager;

    let ws_path = if state.display_mode == BotDisplayMode::Pro {
        match &state.current_workspace {
            Some(p) => std::path::PathBuf::from(p),
            None => {
                return result_from_menu(
                    state,
                    MenuView::plain(s.no_workspace).with_items(vec![
                        MenuItem::primary(s.item_switch_workspace, "/switch"),
                        MenuItem::default(s.item_back, "/menu"),
                    ]),
                );
            }
        }
    } else {
        match &state.current_assistant {
            Some(p) => std::path::PathBuf::from(p),
            None => {
                return result_from_menu(
                    state,
                    MenuView::plain(s.no_assistant).with_items(vec![
                        MenuItem::primary(s.item_switch_assistant, "/switch"),
                        MenuItem::default(s.item_back, "/menu"),
                    ]),
                );
            }
        }
    };

    let page_size = 10usize;
    let offset = page * page_size;

    let pm = match PathManager::new() {
        Ok(pm) => std::sync::Arc::new(pm),
        Err(e) => {
            return result_from_menu(
                state,
                MenuView::plain(format!("{}{e}", s.session_create_failed_prefix)),
            );
        }
    };
    let store = match PersistenceManager::new(pm) {
        Ok(store) => store,
        Err(e) => {
            return result_from_menu(
                state,
                MenuView::plain(format!("{}{e}", s.session_create_failed_prefix)),
            );
        }
    };
    let all_meta = match store.list_session_metadata(&ws_path).await {
        Ok(m) => m,
        Err(e) => {
            return result_from_menu(
                state,
                MenuView::plain(format!("{}{e}", s.session_create_failed_prefix)),
            );
        }
    };

    if all_meta.is_empty() {
        return result_from_menu(state, need_session_view(state, s));
    }

    let total = all_meta.len();
    let has_more = offset + page_size < total;
    let sessions: Vec<_> = all_meta.into_iter().skip(offset).take(page_size).collect();

    let mut body = String::new();
    let mut items = Vec::new();
    let mut options = Vec::new();
    for (i, sess) in sessions.iter().enumerate() {
        let is_current = state.current_session_id.as_deref() == Some(&sess.session_id);
        let marker = if is_current { s.current_marker } else { "" };
        let ts = chrono::DateTime::from_timestamp(sess.last_active_at as i64 / 1000, 0)
            .map(|dt| dt.format("%m-%d %H:%M").to_string())
            .unwrap_or_default();
        let msg_hint = match sess.turn_count {
            0 => s.resume_msg_count_zero.to_string(),
            1 => s.resume_msg_count_one.to_string(),
            n => fmt_count(s.resume_msg_count_many_fmt, n),
        };
        body.push_str(&format!(
            "{}. [{}] {}{}\n   {} · {}\n",
            i + 1,
            sess.agent_type,
            sess.session_name,
            marker,
            ts,
            msg_hint,
        ));
        items.push(MenuItem::default(
            truncate_label(&format!("[{}] {}", sess.agent_type, sess.session_name), 26),
            (i + 1).to_string(),
        ));
        options.push((sess.session_id.clone(), sess.session_name.clone()));
    }
    if has_more {
        items.push(MenuItem::default(s.item_next_page, "0"));
    }
    items.push(MenuItem::default(s.item_back, "/menu"));

    state.set_pending(PendingAction::SelectSession {
        options,
        page,
        has_more,
    });

    let footer = if has_more {
        s.footer_reply_session_or_next
    } else {
        s.footer_reply_session
    };
    let view = MenuView::plain(format!("{} · #{}", s.resume_page_label, page + 1))
        .with_body(body.trim_end().to_string())
        .with_items(items)
        .with_footer(footer);
    result_from_menu(state, view)
}

async fn select_session(
    state: &mut BotChatState,
    session_id: &str,
    session_name: &str,
    s: &'static BotStrings,
) -> HandleResult {
    state.current_session_id = Some(session_id.to_string());
    info!("Bot resumed session: {session_id}");

    let last_pair =
        load_last_dialog_pair_from_turns(state.current_workspace.as_deref(), session_id).await;
    let mut body = format!("{}{}\n", s.resume_resumed_prefix, session_name);
    if let Some((user_text, ai_text)) = last_pair {
        body.push('\n');
        body.push_str(s.resume_last_dialog_header);
        body.push('\n');
        body.push_str(&format!("{}: {}\n\n", s.resume_you_label, user_text));
        body.push_str(&format!("AI: {}\n\n", ai_text));
        body.push_str(s.resume_continue_hint);
    } else {
        body.push('\n');
        body.push_str(s.resume_first_message_hint);
    }

    // Resumed session leaves the user ready to chat — show no menu so the
    // chat surface stays uncluttered.
    let view = MenuView::plain("").with_body(body);
    result_from_menu(state, view)
}

async fn load_last_dialog_pair_from_turns(
    workspace_path: Option<&str>,
    session_id: &str,
) -> Option<(String, String)> {
    use crate::agentic::persistence::PersistenceManager;
    use crate::infrastructure::PathManager;

    const MAX_USER_LEN: usize = 200;
    const MAX_AI_LEN: usize = 400;

    let wp = std::path::PathBuf::from(workspace_path?);
    let pm = std::sync::Arc::new(PathManager::new().ok()?);
    let store = PersistenceManager::new(pm).ok()?;
    let turns = store.load_session_turns(&wp, session_id).await.ok()?;
    let turn = turns.last()?;

    let user_text = strip_user_message_tags(&turn.user_message.content);
    if user_text.is_empty() {
        return None;
    }

    let mut ai_text = String::new();
    for round in &turn.model_rounds {
        for t in &round.text_items {
            if t.is_subagent_item.unwrap_or(false) {
                continue;
            }
            if !t.content.is_empty() {
                if !ai_text.is_empty() {
                    ai_text.push('\n');
                }
                ai_text.push_str(&t.content);
            }
        }
    }
    if ai_text.is_empty() {
        return None;
    }
    Some((
        truncate_text(&user_text, MAX_USER_LEN),
        truncate_text(&ai_text, MAX_AI_LEN),
    ))
}

fn strip_user_message_tags(raw: &str) -> String {
    crate::agentic::core::strip_prompt_markup(raw)
}

fn truncate_text(text: &str, max_chars: usize) -> String {
    let trimmed = text.trim();
    if trimmed.chars().count() <= max_chars {
        trimmed.to_string()
    } else {
        let truncated: String = trimmed.chars().take(max_chars).collect();
        format!("{truncated}…")
    }
}

async fn new_session_for_mode(state: &mut BotChatState, s: &'static BotStrings) -> HandleResult {
    let agent_type = if state.display_mode == BotDisplayMode::Pro {
        "agentic"
    } else {
        "Claw"
    };
    guarded_new(state, agent_type, s).await
}

async fn guarded_new(
    state: &mut BotChatState,
    agent_type: &str,
    s: &'static BotStrings,
) -> HandleResult {
    let needs_pro = matches!(agent_type, "agentic" | "Cowork");
    let needs_assistant = matches!(agent_type, "Claw");

    if needs_pro && state.display_mode != BotDisplayMode::Pro {
        let target_cmd = match agent_type {
            "agentic" => "/new_code_session",
            "Cowork" => "/new_cowork_session",
            _ => "/new_code_session",
        };
        return confirm_then_run(state, BotDisplayMode::Pro, target_cmd.to_string(), s).await;
    }
    if needs_assistant && state.display_mode != BotDisplayMode::Assistant {
        return confirm_then_run(
            state,
            BotDisplayMode::Assistant,
            "/new_claw_session".to_string(),
            s,
        )
        .await;
    }
    if needs_pro && state.current_workspace.is_none() {
        return result_from_menu(
            state,
            MenuView::plain(s.no_workspace).with_items(vec![
                MenuItem::primary(s.item_switch_workspace, "/switch"),
                MenuItem::default(s.item_back, "/menu"),
            ]),
        );
    }
    create_session(state, agent_type).await
}

async fn create_session(state: &mut BotChatState, agent_type: &str) -> HandleResult {
    use crate::agentic::coordination::get_global_coordinator;
    use crate::agentic::core::SessionConfig;
    use crate::service::workspace::get_global_workspace_service;

    let language = current_bot_language().await;
    let s = strings_for(language);
    let is_claw = agent_type == "Claw";

    let coordinator = match get_global_coordinator() {
        Some(c) => c,
        None => {
            return result_from_menu(state, MenuView::plain(s.session_system_unavailable));
        }
    };

    let ws_path = if is_claw {
        if let Some(p) = state.current_assistant.clone() {
            Some(p)
        } else {
            let ws_service = match get_global_workspace_service() {
                Some(s) => s,
                None => {
                    return result_from_menu(
                        state,
                        MenuView::plain(s.workspace_service_unavailable),
                    );
                }
            };
            let workspaces = ws_service.get_assistant_workspaces().await;
            let resolved: Option<(String, String)> = if let Some(default_ws) =
                workspaces.into_iter().find(|w| w.assistant_id.is_none())
            {
                Some((
                    default_ws.root_path.to_string_lossy().to_string(),
                    default_ws.name.clone(),
                ))
            } else {
                match ws_service.create_assistant_workspace(None).await {
                    Ok(ws_info) => Some((
                        ws_info.root_path.to_string_lossy().to_string(),
                        ws_info.name.clone(),
                    )),
                    Err(e) => {
                        return result_from_menu(
                            state,
                            MenuView::plain(format!("{}{e}", s.assistant_create_failed_prefix)),
                        );
                    }
                }
            };
            if let Some((ref path, ref name)) = resolved {
                state.current_assistant = Some(path.clone());
                state.current_assistant_name = Some(name.clone());
            }
            resolved.map(|(p, _)| p)
        }
    } else {
        state.current_workspace.clone()
    };

    let session_name = match agent_type {
        "Cowork" => {
            if language.is_chinese() {
                "远程协作会话"
            } else {
                "Remote Cowork Session"
            }
        }
        "Claw" => {
            if language.is_chinese() {
                "远程助理会话"
            } else {
                "Remote Claw Session"
            }
        }
        _ => {
            if language.is_chinese() {
                "远程编码会话"
            } else {
                "Remote Code Session"
            }
        }
    };

    let Some(workspace_path) = ws_path else {
        let view = if is_claw {
            MenuView::plain(s.no_assistant).with_items(vec![
                MenuItem::primary(s.item_switch_assistant, "/switch"),
                MenuItem::default(s.item_back, "/menu"),
            ])
        } else {
            MenuView::plain(s.no_workspace).with_items(vec![
                MenuItem::primary(s.item_switch_workspace, "/switch"),
                MenuItem::default(s.item_back, "/menu"),
            ])
        };
        return result_from_menu(state, view);
    };

    match coordinator
        .create_session_with_workspace(
            None,
            session_name.to_string(),
            agent_type.to_string(),
            SessionConfig {
                workspace_path: Some(workspace_path.clone()),
                ..Default::default()
            },
            workspace_path.clone(),
        )
        .await
    {
        Ok(session) => {
            state.current_session_id = Some(session.session_id.clone());
            let body = format!(
                "{}{}\n{}{}\n\n{}",
                s.session_created_prefix,
                session_name,
                s.session_workspace_label,
                short_path_name(&workspace_path),
                s.session_start_hint,
            );
            let view = MenuView::plain("").with_body(body);
            result_from_menu(state, view)
        }
        Err(e) => result_from_menu(
            state,
            MenuView::plain(format!("{}{e}", s.session_create_failed_prefix)),
        ),
    }
}

// ── Cancel ─────────────────────────────────────────────────────────

async fn handle_cancel_task(
    state: &mut BotChatState,
    requested_turn_id: Option<&str>,
    s: &'static BotStrings,
) -> HandleResult {
    use crate::service::remote_connect::remote_server::get_or_init_global_dispatcher;

    let session_id = match state.current_session_id.clone() {
        Some(id) => id,
        None => {
            return result_from_menu(state, MenuView::plain(s.task_no_active));
        }
    };
    let dispatcher = get_or_init_global_dispatcher();
    match dispatcher.cancel_task(&session_id, requested_turn_id).await {
        Ok(_) => {
            state.clear_pending();
            result_from_menu(state, MenuView::plain(s.task_cancel_requested))
        }
        Err(e) => result_from_menu(
            state,
            MenuView::plain(format!("{}{e}", s.task_cancel_failed_prefix)),
        ),
    }
}

// ── Numeric reply routing ─────────────────────────────────────────

async fn handle_number(state: &mut BotChatState, n: usize, s: &'static BotStrings) -> HandleResult {
    if let Some(pending) = state.pending_action.clone() {
        return route_pending(state, pending, &n.to_string(), s).await;
    }
    // No pending action: 0 always returns to main menu.
    if n == 0 {
        return menu_or_welcome(state, s);
    }
    if n >= 1 && n <= state.last_menu_commands.len() {
        let cmd_str = state.last_menu_commands[n - 1].clone();
        let next_cmd = parse_command(&cmd_str);
        return Box::pin(dispatch(state, next_cmd, vec![])).await;
    }
    handle_chat(state, &n.to_string(), vec![], s).await
}

async fn route_pending(
    state: &mut BotChatState,
    pending: PendingAction,
    raw_input: &str,
    s: &'static BotStrings,
) -> HandleResult {
    match pending {
        PendingAction::SelectWorkspace { options } => {
            let parsed: Option<usize> = raw_input.parse().ok();
            match parsed {
                Some(0) => {
                    state.clear_pending();
                    menu_or_welcome(state, s)
                }
                Some(n) if n >= 1 && n <= options.len() => {
                    state.clear_pending();
                    let (path, name) = options[n - 1].clone();
                    select_workspace(state, &path, &name, s).await
                }
                _ => {
                    state.set_pending(PendingAction::SelectWorkspace { options });
                    Box::pin(pending_invalid(state, s)).await
                }
            }
        }
        PendingAction::SelectAssistant { options } => {
            let parsed: Option<usize> = raw_input.parse().ok();
            match parsed {
                Some(0) => {
                    state.clear_pending();
                    menu_or_welcome(state, s)
                }
                Some(n) if n >= 1 && n <= options.len() => {
                    state.clear_pending();
                    let (path, name) = options[n - 1].clone();
                    select_assistant(state, &path, &name, s).await
                }
                _ => {
                    state.set_pending(PendingAction::SelectAssistant { options });
                    Box::pin(pending_invalid(state, s)).await
                }
            }
        }
        PendingAction::SelectSession {
            options,
            page,
            has_more,
        } => {
            let parsed: Option<usize> = raw_input.parse().ok();
            match parsed {
                Some(0) if has_more => {
                    state.clear_pending();
                    start_resume(state, page + 1, s).await
                }
                Some(0) => {
                    state.clear_pending();
                    menu_or_welcome(state, s)
                }
                Some(n) if n >= 1 && n <= options.len() => {
                    state.clear_pending();
                    let (id, name) = options[n - 1].clone();
                    select_session(state, &id, &name, s).await
                }
                _ => {
                    state.set_pending(PendingAction::SelectSession {
                        options,
                        page,
                        has_more,
                    });
                    Box::pin(pending_invalid(state, s)).await
                }
            }
        }
        PendingAction::AskUserQuestion {
            tool_id,
            questions,
            current_index,
            answers,
            awaiting_custom_text,
            pending_answer,
        } => {
            handle_question_reply(
                state,
                tool_id,
                questions,
                current_index,
                answers,
                awaiting_custom_text,
                pending_answer,
                raw_input,
                s,
            )
            .await
        }
        PendingAction::ConfirmModeSwitch {
            target_mode,
            target_cmd,
        } => {
            let parsed: Option<usize> = raw_input.parse().ok();
            match parsed {
                Some(1) => {
                    state.clear_pending();
                    state.display_mode = target_mode;
                    let next_cmd = parse_command(&target_cmd);
                    Box::pin(dispatch(state, next_cmd, vec![])).await
                }
                Some(0) => {
                    state.clear_pending();
                    menu_or_welcome(state, s)
                }
                _ => {
                    state.set_pending(PendingAction::ConfirmModeSwitch {
                        target_mode,
                        target_cmd,
                    });
                    Box::pin(pending_invalid(state, s)).await
                }
            }
        }
    }
}

/// Re-show the current pending view with an "invalid input" prefix so the
/// user retains context.  After [`PENDING_INVALID_LIMIT`] consecutive invalid
/// replies the pending state is cleared and the user is returned to the main
/// menu.
async fn pending_invalid(state: &mut BotChatState, s: &'static BotStrings) -> HandleResult {
    state.pending_invalid_count = state.pending_invalid_count.saturating_add(1);
    if state.pending_invalid_count >= PENDING_INVALID_LIMIT {
        state.clear_pending();
        let mut view = main_menu_view(state, s);
        view = view.with_body(s.pending_invalid_after_retries);
        return result_from_menu(state, view);
    }
    // Re-render the pending prompt with an invalid-input notice so the user
    // sees the option list again instead of just an opaque error.
    let pending = match state.pending_action.clone() {
        Some(p) => p,
        None => {
            return result_from_menu(state, main_menu_view(state, s));
        }
    };
    let mut view = match &pending {
        PendingAction::SelectWorkspace { options } => workspace_selection_view(state, options, s),
        PendingAction::SelectAssistant { options } => assistant_selection_view(state, options, s),
        PendingAction::SelectSession {
            options,
            page,
            has_more,
        } => session_selection_view(state, options, *page, *has_more, s),
        PendingAction::AskUserQuestion {
            questions,
            current_index,
            awaiting_custom_text,
            ..
        } => build_question_view(s, questions, *current_index, *awaiting_custom_text),
        PendingAction::ConfirmModeSwitch { target_mode, .. } => {
            confirm_mode_switch_view(*target_mode, s)
        }
    };
    let original_body = view.body.take().unwrap_or_default();
    let new_body = if original_body.is_empty() {
        s.pending_invalid_input.to_string()
    } else {
        format!("{}\n\n{}", s.pending_invalid_input, original_body)
    };
    view = view.with_body(new_body);
    result_from_menu(state, view)
}

// ── Question handling ─────────────────────────────────────────────

fn question_option_line(index: usize, option: &BotQuestionOption) -> String {
    if option.description.is_empty() {
        format!("{}. {}", index + 1, option.label)
    } else {
        format!("{}. {} - {}", index + 1, option.label, option.description)
    }
}

fn build_question_view(
    s: &'static BotStrings,
    questions: &[BotQuestion],
    current_index: usize,
    awaiting_custom_text: bool,
) -> MenuView {
    let question = &questions[current_index];
    let title = format!(
        "{} {}/{}",
        s.question_title,
        current_index + 1,
        questions.len()
    );

    let mut body = String::new();
    if !question.header.is_empty() {
        body.push_str(&question.header);
        body.push('\n');
    }
    body.push_str(&question.question);
    body.push_str("\n\n");
    for (idx, option) in question.options.iter().enumerate() {
        body.push_str(&question_option_line(idx, option));
        body.push('\n');
    }
    body.push_str(&format!(
        "{}. {}\n",
        question.options.len() + 1,
        s.item_other,
    ));

    let footer = if awaiting_custom_text {
        s.footer_question_custom
    } else if question.multi_select {
        s.footer_question_multi
    } else {
        s.footer_question_single
    };

    let mut items: Vec<MenuItem> = Vec::new();
    if !awaiting_custom_text && !question.multi_select {
        for (idx, option) in question.options.iter().enumerate() {
            items.push(MenuItem::default(
                truncate_label(&option.label, 24),
                (idx + 1).to_string(),
            ));
        }
        items.push(MenuItem::default(
            s.item_other,
            (question.options.len() + 1).to_string(),
        ));
    }
    items.push(MenuItem::default(s.item_back, "/menu"));

    MenuView::plain(title)
        .with_body(body.trim_end().to_string())
        .with_items(items)
        .with_footer(footer)
}

fn parse_question_numbers(input: &str) -> Option<Vec<usize>> {
    let mut result = Vec::new();
    for part in input.split(',') {
        let trimmed = part.trim();
        if trimmed.is_empty() {
            continue;
        }
        let value = trimmed.parse::<usize>().ok()?;
        result.push(value);
    }
    if result.is_empty() {
        None
    } else {
        Some(result)
    }
}

#[allow(clippy::too_many_arguments)]
async fn handle_question_reply(
    state: &mut BotChatState,
    tool_id: String,
    questions: Vec<BotQuestion>,
    current_index: usize,
    mut answers: Vec<Value>,
    awaiting_custom_text: bool,
    pending_answer: Option<Value>,
    message: &str,
    s: &'static BotStrings,
) -> HandleResult {
    let Some(question) = questions.get(current_index).cloned() else {
        return result_from_menu(state, MenuView::plain(s.question_invalid_state));
    };

    if awaiting_custom_text {
        let custom_text = message.trim();
        if custom_text.is_empty() {
            state.set_pending(PendingAction::AskUserQuestion {
                tool_id,
                questions,
                current_index,
                answers,
                awaiting_custom_text: true,
                pending_answer,
            });
            return result_from_menu(state, MenuView::plain(s.question_custom_required));
        }
        let final_value = match pending_answer {
            Some(Value::Array(existing)) => {
                let mut values: Vec<Value> = existing
                    .into_iter()
                    .filter(|v| v.as_str() != Some("Other"))
                    .collect();
                values.push(Value::String(custom_text.to_string()));
                Value::Array(values)
            }
            _ => Value::String(custom_text.to_string()),
        };
        answers.push(final_value);
    } else {
        let selections = match parse_question_numbers(message) {
            Some(values) => values,
            None => {
                state.set_pending(PendingAction::AskUserQuestion {
                    tool_id,
                    questions,
                    current_index,
                    answers,
                    awaiting_custom_text: false,
                    pending_answer: None,
                });
                return Box::pin(pending_invalid(state, s)).await;
            }
        };
        if !question.multi_select && selections.len() != 1 {
            state.set_pending(PendingAction::AskUserQuestion {
                tool_id,
                questions,
                current_index,
                answers,
                awaiting_custom_text: false,
                pending_answer: None,
            });
            return Box::pin(pending_invalid(state, s)).await;
        }
        let other_index = question.options.len() + 1;
        let mut labels = Vec::new();
        let mut includes_other = false;
        for selection in selections {
            if selection == other_index {
                includes_other = true;
                labels.push(Value::String(s.item_other.to_string()));
            } else if selection >= 1 && selection <= question.options.len() {
                labels.push(Value::String(question.options[selection - 1].label.clone()));
            } else {
                state.set_pending(PendingAction::AskUserQuestion {
                    tool_id,
                    questions,
                    current_index,
                    answers,
                    awaiting_custom_text: false,
                    pending_answer: None,
                });
                let _ = other_index;
                return Box::pin(pending_invalid(state, s)).await;
            }
        }
        let pending_answer_next = if question.multi_select {
            Some(Value::Array(labels.clone()))
        } else {
            labels.into_iter().next()
        };
        if includes_other {
            state.set_pending(PendingAction::AskUserQuestion {
                tool_id,
                questions,
                current_index,
                answers,
                awaiting_custom_text: true,
                pending_answer: pending_answer_next,
            });
            return result_from_menu(state, MenuView::plain(s.question_custom_for_other_prefix));
        }
        answers.push(if question.multi_select {
            pending_answer_next.unwrap_or_else(|| Value::Array(Vec::new()))
        } else {
            pending_answer_next.unwrap_or_else(|| Value::String(String::new()))
        });
    }

    if current_index + 1 < questions.len() {
        let view = build_question_view(s, &questions, current_index + 1, false);
        state.set_pending(PendingAction::AskUserQuestion {
            tool_id,
            questions,
            current_index: current_index + 1,
            answers,
            awaiting_custom_text: false,
            pending_answer: None,
        });
        return result_from_menu(state, view);
    }

    state.clear_pending();
    submit_question_answers(&tool_id, &answers, s).await
}

async fn submit_question_answers(
    tool_id: &str,
    answers: &[Value],
    s: &'static BotStrings,
) -> HandleResult {
    use crate::agentic::tools::user_input_manager::get_user_input_manager;

    let mut payload = serde_json::Map::new();
    for (idx, value) in answers.iter().enumerate() {
        payload.insert(idx.to_string(), value.clone());
    }
    let manager = get_user_input_manager();
    match manager.send_answer(tool_id, Value::Object(payload)) {
        Ok(_) => HandleResult {
            reply: s.answers_submitted.to_string(),
            actions: vec![],
            forward_to_session: None,
            menu: MenuView::plain(s.answers_submitted),
        },
        Err(e) => HandleResult {
            reply: format!("{}{e}", s.answers_submit_failed_prefix),
            actions: vec![],
            forward_to_session: None,
            menu: MenuView::plain(format!("{}{e}", s.answers_submit_failed_prefix)),
        },
    }
}

// ── Free-form chat handling ───────────────────────────────────────

/// Look up the agent type a session was created with (e.g. "Claw", "Cowork",
/// "agentic").  Returns `None` if the coordinator is unavailable or the
/// session is not currently hot in memory; in that case `send_message` will
/// lazily restore the session from disk and `resolve_agent_type` falls back
/// to the safe default ("agentic"), so chat keeps working.
async fn resolve_session_agent_type(session_id: &str) -> Option<String> {
    use crate::agentic::coordination::get_global_coordinator;

    let coordinator = get_global_coordinator()?;
    coordinator
        .get_session_manager()
        .get_session(session_id)
        .map(|s| s.agent_type.clone())
}

async fn handle_chat(
    state: &mut BotChatState,
    message: &str,
    image_contexts: Vec<crate::agentic::image_analysis::ImageContextData>,
    s: &'static BotStrings,
) -> HandleResult {
    // If there is a pending action, route the message to it (text answer for
    // questions, "ignore" for menu-style pendings).
    if let Some(pending) = state.pending_action.clone() {
        return route_pending(state, pending, message, s).await;
    }

    if state.display_mode == BotDisplayMode::Pro && state.current_workspace.is_none() {
        return result_from_menu(
            state,
            MenuView::plain(s.no_workspace).with_items(vec![
                MenuItem::primary(s.item_switch_workspace, "/switch"),
                MenuItem::default(s.item_back, "/menu"),
            ]),
        );
    }
    if state.current_session_id.is_none() {
        return result_from_menu(state, need_session_view(state, s));
    }

    let session_id = state.current_session_id.clone().unwrap();
    let turn_id = format!("turn_{}", uuid::Uuid::new_v4());

    // Pick the agent type from the actual session — NOT a hardcoded
    // "agentic" — otherwise every chat message goes through the Code
    // (`agentic`) agent regardless of what kind of session was created.
    // Concretely: the IM pairing bootstrap creates a `Claw` session for
    // assistant mode, but the old hardcoded value caused all subsequent
    // messages to be re-routed to the Code agent and the assistant flow
    // was effectively bypassed.  We mirror the agent type the session was
    // actually created with, falling back to "agentic" only if the session
    // is missing in memory (e.g. needs lazy restore — `send_message` will
    // also normalize via `resolve_agent_type`).
    let agent_type = resolve_session_agent_type(&session_id)
        .await
        .unwrap_or_else(|| "agentic".to_string());

    // Intentionally do NOT send a "Processing..." / "Queued" interstitial
    // message with a Cancel-task menu. The session manager queues new user
    // messages automatically: the user can simply send another message and
    // it will be processed once the current atomic step finishes. Showing
    // a cancel button adds noise (especially on WeChat where every reply
    // costs a context_token slot) without giving the user anything they
    // actually need. The empty `MenuView::default()` here is silently
    // dropped by every adapter's `send_handle_result` (see the
    // empty-text guards in weixin.rs / feishu.rs / telegram.rs).
    let view = MenuView::default();

    let forward = ForwardRequest {
        session_id,
        content: message.to_string(),
        agent_type,
        turn_id,
        image_contexts,
    };

    result_from_menu_with_forward(state, view, Some(forward))
}

// ── Forwarded turn execution (largely unchanged) ──────────────────

pub async fn execute_forwarded_turn(
    forward: ForwardRequest,
    interaction_handler: Option<BotInteractionHandler>,
    message_sender: Option<BotMessageSender>,
    verbose_mode: bool,
) -> ForwardedTurnResult {
    use crate::agentic::coordination::{DialogSubmissionPolicy, DialogTriggerSource};
    use crate::service::remote_connect::remote_server::{
        get_or_init_global_dispatcher, TrackerEvent,
    };

    let language = current_bot_language().await;
    let s = strings_for(language);

    let dispatcher = get_or_init_global_dispatcher();
    let tracker = dispatcher.ensure_tracker(&forward.session_id);
    let mut event_rx = tracker.subscribe();

    let target_turn_id = forward.turn_id.clone();

    if let Err(e) = dispatcher
        .send_message(
            &forward.session_id,
            forward.content,
            Some(&forward.agent_type),
            forward.image_contexts,
            DialogSubmissionPolicy::for_source(DialogTriggerSource::Bot),
            Some(forward.turn_id.clone()),
        )
        .await
    {
        let msg = format!("{}{e}", s.send_failed_prefix);
        return ForwardedTurnResult {
            display_text: msg.clone(),
            full_text: msg,
        };
    }

    let result = tokio::time::timeout(std::time::Duration::from_secs(3600), async {
        let mut response = String::new();
        let mut thinking_buf = String::new();

        let streams_our_turn = || {
            tracker
                .snapshot_active_turn()
                .map(|st| st.turn_id == target_turn_id)
                .unwrap_or(false)
        };

        loop {
            match event_rx.recv().await {
                Ok(event) => match event {
                    TrackerEvent::ThinkingChunk(chunk) => {
                        if !streams_our_turn() {
                            continue;
                        }
                        thinking_buf.push_str(&chunk);
                    }
                    TrackerEvent::ThinkingEnd => {
                        if !streams_our_turn() {
                            continue;
                        }
                        if verbose_mode && !thinking_buf.trim().is_empty() {
                            if let Some(sender) = message_sender.as_ref() {
                                let content = truncate_at_char_boundary(&thinking_buf, 500);
                                let msg = format!("[{}] {}", s.thinking_label, content);
                                sender(msg).await;
                            }
                        }
                        thinking_buf.clear();
                    }
                    TrackerEvent::TextChunk(t) => {
                        if !streams_our_turn() {
                            continue;
                        }
                        response.push_str(&t);
                    }
                    TrackerEvent::ToolStarted {
                        tool_id,
                        tool_name,
                        params,
                    } => {
                        if !streams_our_turn() {
                            continue;
                        }
                        // Only AskUserQuestion needs an IM-side prompt; every
                        // other tool call is internal and not surfaced to the
                        // user (verbose mode keeps thinking summaries only —
                        // see ToolCompleted handler below).
                        if tool_name == "AskUserQuestion" {
                            if let Some(questions_value) =
                                params.and_then(|p| p.get("questions").cloned())
                            {
                                if let Ok(questions) =
                                    serde_json::from_value::<Vec<BotQuestion>>(questions_value)
                                {
                                    let view = build_question_view(s, &questions, 0, false);
                                    let actions: Vec<BotAction> =
                                        view.items.iter().cloned().map(BotAction::from).collect();
                                    let request = BotInteractiveRequest {
                                        reply: view.render_text_block(),
                                        actions,
                                        menu: view,
                                        pending_action: PendingAction::AskUserQuestion {
                                            tool_id,
                                            questions,
                                            current_index: 0,
                                            answers: Vec::new(),
                                            awaiting_custom_text: false,
                                            pending_answer: None,
                                        },
                                    };
                                    if let Some(handler) = interaction_handler.as_ref() {
                                        handler(request).await;
                                    }
                                }
                            }
                        }
                    }
                    TrackerEvent::ToolCompleted { .. } => {
                        // Verbose mode used to push a `[ToolName] params => OK 627ms`
                        // line for every tool call. That is noisy on IM channels
                        // (especially WeChat where each line costs a context_token
                        // slot) and provides little value to the end user — they
                        // only care about the thinking summary and the final
                        // answer. Drop the tool-call notifications entirely while
                        // keeping `ThinkingEnd` summaries for verbose mode.
                    }
                    TrackerEvent::TurnCompleted { turn_id } => {
                        if turn_id == target_turn_id {
                            break;
                        }
                    }
                    TrackerEvent::TurnFailed { turn_id, error } => {
                        if turn_id == target_turn_id {
                            let msg = format!("{}{}", s.error_prefix, error);
                            return ForwardedTurnResult {
                                display_text: msg.clone(),
                                full_text: msg,
                            };
                        }
                    }
                    TrackerEvent::TurnCancelled { turn_id } => {
                        if turn_id == target_turn_id {
                            return ForwardedTurnResult {
                                display_text: s.task_cancelled.to_string(),
                                full_text: s.task_cancelled.to_string(),
                            };
                        }
                    }
                },
                Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                    log::warn!("Bot event receiver lagged by {n} events");
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                    break;
                }
            }
        }

        let full_text = tracker.accumulated_text();
        let full_text = if full_text.is_empty() {
            response
        } else {
            full_text
        };

        // Do NOT truncate here. Each IM adapter knows its own per-message
        // size limit and chunks accordingly (e.g. WeChat splits via
        // `chunk_text_for_weixin`, Telegram chunks at 4096 chars). A global
        // 4000-char hard cut here would silently drop the tail of long
        // replies (e.g. PPT outlines, code reviews) and confuse users with
        // a "(truncated)" suffix they cannot recover from.
        let display_text = full_text.clone();

        ForwardedTurnResult {
            display_text: if display_text.is_empty() {
                s.no_response.to_string()
            } else {
                display_text
            },
            full_text,
        }
    })
    .await;

    result.unwrap_or_else(|_| ForwardedTurnResult {
        display_text: s.timeout_one_hour.to_string(),
        full_text: String::new(),
    })
}

fn truncate_at_char_boundary(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        return s.to_string();
    }
    let mut end = max_len;
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}...", &s[..end])
}

// ── Tests ─────────────────────────────────────────────────────────

#[cfg(test)]
mod parse_command_tests {
    use super::*;

    #[test]
    fn numeric_menu_with_trailing_dot() {
        assert!(matches!(
            parse_command("1."),
            BotCommand::NumberSelection(1)
        ));
        assert!(matches!(
            parse_command("2。"),
            BotCommand::NumberSelection(2)
        ));
    }

    #[test]
    fn fullwidth_digit_one() {
        assert!(matches!(
            parse_command("１"),
            BotCommand::NumberSelection(1)
        ));
    }

    #[test]
    fn zero_parsed_as_number_selection() {
        // `0` stays as a numeric selection so it can mean "next page" or
        // "back" depending on which pending action is active.  The
        // top-level "no pending" → main-menu fallback is implemented in
        // `handle_number`.
        assert!(matches!(parse_command("0"), BotCommand::NumberSelection(0)));
    }

    #[test]
    fn menu_aliases() {
        assert!(matches!(parse_command("/menu"), BotCommand::Menu));
        assert!(matches!(parse_command("/m"), BotCommand::Menu));
        assert!(matches!(parse_command("菜单"), BotCommand::Menu));
        assert!(matches!(parse_command("/start"), BotCommand::Menu));
    }

    #[test]
    fn settings_aliases() {
        assert!(matches!(parse_command("/settings"), BotCommand::Settings));
        assert!(matches!(parse_command("设置"), BotCommand::Settings));
    }

    #[test]
    fn verbose_concise_real_commands() {
        assert!(matches!(
            parse_command("/verbose"),
            BotCommand::SetVerbose(true)
        ));
        assert!(matches!(
            parse_command("/concise"),
            BotCommand::SetVerbose(false)
        ));
    }

    #[test]
    fn switch_aliases() {
        assert!(matches!(
            parse_command("/switch"),
            BotCommand::SwitchContext
        ));
        assert!(matches!(
            parse_command("/switch_workspace"),
            BotCommand::SwitchContext
        ));
        assert!(matches!(
            parse_command("/switch_assistant"),
            BotCommand::SwitchContext
        ));
        assert!(matches!(parse_command("切换"), BotCommand::SwitchContext));
    }

    #[test]
    fn new_session_aliases() {
        assert!(matches!(parse_command("/new"), BotCommand::NewSession));
        assert!(matches!(
            parse_command("/new_code_session"),
            BotCommand::NewCodeSession
        ));
        assert!(matches!(
            parse_command("/new_cowork_session"),
            BotCommand::NewCoworkSession
        ));
        assert!(matches!(
            parse_command("/new_claw_session"),
            BotCommand::NewClawSession
        ));
    }

    #[test]
    fn resume_aliases() {
        assert!(matches!(
            parse_command("/resume"),
            BotCommand::ResumeSession
        ));
        assert!(matches!(parse_command("/r"), BotCommand::ResumeSession));
        assert!(matches!(
            parse_command("/resume_session"),
            BotCommand::ResumeSession
        ));
    }

    #[test]
    fn cancel_aliases() {
        assert!(matches!(
            parse_command("/cancel"),
            BotCommand::CancelTask(None)
        ));
        match parse_command("/cancel_task turn_abc") {
            BotCommand::CancelTask(Some(id)) => assert_eq!(id, "turn_abc"),
            _ => panic!("expected cancel task with id"),
        }
    }

    #[test]
    fn pairing_code_detected() {
        match parse_command("123456") {
            BotCommand::PairingCode(c) => assert_eq!(c, "123456"),
            _ => panic!("expected pairing code"),
        }
    }

    #[test]
    fn chat_message_fallback() {
        assert!(matches!(
            parse_command("hello world"),
            BotCommand::ChatMessage(_)
        ));
    }
}

#[cfg(test)]
mod state_tests {
    use super::*;

    #[test]
    fn pending_expires_after_ttl() {
        let mut state = BotChatState::new("c".into());
        state.set_pending(PendingAction::SelectWorkspace { options: vec![] });
        assert!(state.pending_action.is_some());
        assert!(!state.pending_expired());
        state.pending_expires_at = now_secs() - 1;
        assert!(state.pending_expired());
    }

    #[test]
    fn active_workspace_path_prefers_pro_workspace_then_assistant() {
        let mut state = BotChatState::new("c".into());
        assert_eq!(state.active_workspace_path(), None);

        state.current_assistant = Some("/tmp/assistant-ws".to_string());
        assert_eq!(
            state.active_workspace_path().as_deref(),
            Some("/tmp/assistant-ws"),
            "assistant path is the fallback when no Pro workspace is set"
        );

        state.current_workspace = Some("/tmp/pro-ws".to_string());
        assert_eq!(
            state.active_workspace_path().as_deref(),
            Some("/tmp/pro-ws"),
            "Pro workspace wins over the assistant path when both are set"
        );
    }

    #[test]
    fn clear_pending_resets_counters() {
        let mut state = BotChatState::new("c".into());
        state.set_pending(PendingAction::SelectWorkspace { options: vec![] });
        state.pending_invalid_count = 2;
        state.clear_pending();
        assert!(state.pending_action.is_none());
        assert_eq!(state.pending_invalid_count, 0);
        assert_eq!(state.pending_expires_at, 0);
    }
}

#[cfg(test)]
mod menu_tests {
    use super::*;

    #[test]
    fn main_menu_assistant_has_four_items() {
        let state = BotChatState::new("c".into());
        let view = main_menu_view(&state, strings_for(BotLanguage::ZhCN));
        assert_eq!(view.items.len(), 4);
        assert!(view.items.iter().any(|i| i.command == "/new"));
        assert!(view.items.iter().any(|i| i.command == "/resume"));
        assert!(view.items.iter().any(|i| i.command == "/switch"));
        assert!(view.items.iter().any(|i| i.command == "/settings"));
    }

    #[test]
    fn main_menu_expert_has_five_items() {
        let mut state = BotChatState::new("c".into());
        state.display_mode = BotDisplayMode::Pro;
        let view = main_menu_view(&state, strings_for(BotLanguage::ZhCN));
        assert_eq!(view.items.len(), 5);
        assert!(view.items.iter().any(|i| i.command == "/new_code_session"));
    }

    /// Main menu must NOT surface the random session UUID tail. The user
    /// only cares about the workspace / assistant name; the session ID is
    /// noise (see /resume for proper session management).
    #[test]
    fn main_menu_body_omits_session_id() {
        let mut state = BotChatState::new("c".into());
        state.current_assistant = Some("/tmp/my-assistant".to_string());
        state.current_assistant_name = Some("我的助理".to_string());
        state.current_session_id = Some("abcdef12-3456-7890-abcd-ef1234567890".to_string());
        let s = strings_for(BotLanguage::ZhCN);
        let view = main_menu_view(&state, s);
        let body = view.body.as_deref().unwrap_or("");
        assert!(
            !body.contains("567890") && !body.contains("ef1234567890"),
            "session UUID tail leaked into body: {body}"
        );
        assert!(body.contains("我的助理"), "assistant name missing: {body}");
    }

    /// Assistant mode must show the assistant's display name rather than
    /// the workspace directory's `file_name`. The directory is usually a
    /// generic "workspace" / "workspace-<uuid>" folder which is meaningless
    /// to the user.
    #[test]
    fn assistant_mode_body_uses_display_name_not_dir_name() {
        let mut state = BotChatState::new("c".into());
        state.current_assistant = Some("/tmp/bitfun_assistants/workspace-abc123".to_string());
        state.current_assistant_name = Some("默认助理".to_string());
        let s = strings_for(BotLanguage::ZhCN);
        let view = main_menu_view(&state, s);
        let body = view.body.as_deref().unwrap_or("");
        assert!(
            body.contains("默认助理"),
            "expected assistant display name in body, got: {body}"
        );
        assert!(
            !body.contains("workspace-abc123"),
            "workspace directory name leaked into body: {body}"
        );
    }

    /// Expert mode keeps showing the workspace directory name (it IS the
    /// project name, which is what the user expects to see).
    #[test]
    fn expert_mode_body_still_uses_workspace_dir_name() {
        let mut state = BotChatState::new("c".into());
        state.display_mode = BotDisplayMode::Pro;
        state.current_workspace = Some("/tmp/projects/MyApp".to_string());
        // `current_assistant_name` should not affect Pro mode at all.
        state.current_assistant_name = Some("ignored".to_string());
        let s = strings_for(BotLanguage::ZhCN);
        let view = main_menu_view(&state, s);
        let body = view.body.as_deref().unwrap_or("");
        assert!(body.contains("MyApp"), "workspace name missing: {body}");
        assert!(
            !body.contains("ignored"),
            "assistant name leaked into Pro mode: {body}"
        );
    }

    /// When the cached assistant display name is missing (e.g. legacy
    /// persisted state), fall back to the path's last segment instead of
    /// rendering an empty label or panicking.
    #[test]
    fn assistant_mode_body_falls_back_to_path_when_name_missing() {
        let mut state = BotChatState::new("c".into());
        state.current_assistant = Some("/tmp/my-assistant-folder".to_string());
        state.current_assistant_name = None;
        let s = strings_for(BotLanguage::ZhCN);
        let view = main_menu_view(&state, s);
        let body = view.body.as_deref().unwrap_or("");
        assert!(
            body.contains("my-assistant-folder"),
            "expected fallback to path tail, got: {body}"
        );
    }

    #[test]
    fn main_menu_body_omits_session_label_text() {
        let mut state = BotChatState::new("c".into());
        state.current_assistant = Some("/tmp/my-assistant".to_string());
        state.current_session_id = Some("session-xyz".to_string());
        let s = strings_for(BotLanguage::ZhCN);
        let view = main_menu_view(&state, s);
        let body = view.body.as_deref().unwrap_or("");
        assert!(
            !body.contains(s.current_session_label),
            "current_session_label leaked into body: {body}"
        );
    }
}

#[cfg(test)]
mod handle_chat_tests {
    use super::*;

    /// `handle_chat` must NOT push a "Processing… [Cancel Task]" interstitial
    /// to the user. The session manager queues new messages automatically;
    /// showing a cancel button just adds noise (and on WeChat costs a
    /// context_token slot per send).
    #[tokio::test]
    async fn chat_message_forwards_silently_without_processing_menu() {
        let mut state = BotChatState::new("peer".into());
        state.paired = true;
        state.current_assistant = Some("/tmp/a".into());
        state.current_session_id = Some("s1".into());
        let s = strings_for(BotLanguage::ZhCN);
        let result = handle_chat(&mut state, "hello bitfun", vec![], s).await;

        assert!(
            result.forward_to_session.is_some(),
            "chat message must still be forwarded to the session"
        );
        assert!(
            result.menu.title.is_empty()
                && result.menu.items.is_empty()
                && result.menu.body.is_none()
                && result.menu.footer_hint.is_none(),
            "handle_chat must return an empty MenuView so adapters skip the send: {:?}",
            result.menu
        );
        assert!(
            !result.reply.contains(s.processing) && !result.reply.contains(s.queued),
            "processing/queued text must not be sent: {}",
            result.reply
        );
        assert!(
            !result.reply.contains(s.item_cancel_task),
            "cancel-task button must not be sent: {}",
            result.reply
        );
    }
}
