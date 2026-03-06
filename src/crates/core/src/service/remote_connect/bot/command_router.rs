//! Shared command router for bot-based connections (Telegram & Feishu).
//!
//! Provides platform-agnostic command parsing, per-chat state management, and
//! dispatch to workspace / session services.  Each platform adapter handles
//! message I/O while this module owns the business logic.

use log::{error, info};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

// ── Per-chat state ──────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BotChatState {
    pub chat_id: String,
    pub paired: bool,
    pub current_workspace: Option<String>,
    pub current_session_id: Option<String>,
    #[serde(skip)]
    pub pending_action: Option<PendingAction>,
}

impl BotChatState {
    pub fn new(chat_id: String) -> Self {
        Self {
            chat_id,
            paired: false,
            current_workspace: None,
            current_session_id: None,
            pending_action: None,
        }
    }
}

#[derive(Debug, Clone)]
pub enum PendingAction {
    SelectWorkspace {
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
}

// ── Parsed command ──────────────────────────────────────────────────

#[derive(Debug)]
pub enum BotCommand {
    Start,
    SwitchWorkspace,
    ResumeSession,
    NewCodeSession,
    NewCoworkSession,
    CancelTask(Option<String>),
    Help,
    PairingCode(String),
    NumberSelection(usize),
    NextPage,
    ChatMessage(String),
}

// ── Handle result ───────────────────────────────────────────────────

pub struct HandleResult {
    pub reply: String,
    pub actions: Vec<BotAction>,
    pub forward_to_session: Option<ForwardRequest>,
}

#[derive(Debug, Clone)]
pub struct BotInteractiveRequest {
    pub reply: String,
    pub actions: Vec<BotAction>,
    pub pending_action: PendingAction,
}

pub type BotInteractionHandler = Arc<
    dyn Fn(BotInteractiveRequest) -> Pin<Box<dyn Future<Output = ()> + Send>> + Send + Sync,
>;

pub type BotMessageSender = Arc<
    dyn Fn(String) -> Pin<Box<dyn Future<Output = ()> + Send>> + Send + Sync,
>;

pub struct ForwardRequest {
    pub session_id: String,
    pub content: String,
    pub agent_type: String,
    pub turn_id: String,
}

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

#[derive(Debug, Clone)]
pub struct BotAction {
    pub label: String,
    pub command: String,
    pub style: BotActionStyle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BotActionStyle {
    Primary,
    Default,
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

// ── Command parsing ─────────────────────────────────────────────────

pub fn parse_command(text: &str) -> BotCommand {
    let trimmed = text.trim();
    if let Some(rest) = trimmed.strip_prefix("/cancel_task") {
        let arg = rest.trim();
        return if arg.is_empty() {
            BotCommand::CancelTask(None)
        } else {
            BotCommand::CancelTask(Some(arg.to_string()))
        };
    }
    match trimmed {
        "/start" => BotCommand::Start,
        "/switch_workspace" => BotCommand::SwitchWorkspace,
        "/resume_session" => BotCommand::ResumeSession,
        "/new_code_session" => BotCommand::NewCodeSession,
        "/new_cowork_session" => BotCommand::NewCoworkSession,
        "/help" => BotCommand::Help,
        "0" => BotCommand::NextPage,
        _ => {
            if trimmed.len() == 6 && trimmed.chars().all(|c| c.is_ascii_digit()) {
                BotCommand::PairingCode(trimmed.to_string())
            } else if let Ok(n) = trimmed.parse::<usize>() {
                if (1..=99).contains(&n) {
                    BotCommand::NumberSelection(n)
                } else {
                    BotCommand::ChatMessage(trimmed.to_string())
                }
            } else {
                BotCommand::ChatMessage(trimmed.to_string())
            }
        }
    }
}

// ── Static messages ─────────────────────────────────────────────────

pub const WELCOME_MESSAGE: &str = "\
Welcome to BitFun!

To connect your BitFun desktop app, please enter the 6-digit pairing code shown in your BitFun Remote Connect panel.

Need a pairing code? Open BitFun Desktop -> Remote Connect -> Telegram/Feishu Bot -> copy the 6-digit code and send it here.";

pub const HELP_MESSAGE: &str = "\
Available commands:
/switch_workspace - List and switch workspaces
/resume_session - Resume an existing session
/new_code_session - Create a new coding session
/new_cowork_session - Create a new cowork session
/cancel_task - Cancel the current task
/help - Show this help message";

pub fn paired_success_message() -> String {
    format!(
        "Pairing successful! BitFun is now connected.\n\n{}",
        HELP_MESSAGE
    )
}

pub fn main_menu_actions() -> Vec<BotAction> {
    vec![
        BotAction::primary("Switch Workspace", "/switch_workspace"),
        BotAction::secondary("Resume Session", "/resume_session"),
        BotAction::secondary("New Code Session", "/new_code_session"),
        BotAction::secondary("New Cowork Session", "/new_cowork_session"),
        BotAction::secondary("Help", "/help"),
    ]
}

fn workspace_required_actions() -> Vec<BotAction> {
    vec![BotAction::primary("Switch Workspace", "/switch_workspace")]
}

fn session_entry_actions() -> Vec<BotAction> {
    vec![
        BotAction::primary("Resume Session", "/resume_session"),
        BotAction::secondary("New Code Session", "/new_code_session"),
        BotAction::secondary("New Cowork Session", "/new_cowork_session"),
    ]
}

fn new_session_actions() -> Vec<BotAction> {
    vec![
        BotAction::primary("New Code Session", "/new_code_session"),
        BotAction::secondary("New Cowork Session", "/new_cowork_session"),
    ]
}

fn cancel_task_actions(command: impl Into<String>) -> Vec<BotAction> {
    vec![BotAction::secondary("Cancel Task", command.into())]
}

// ── Main dispatch ───────────────────────────────────────────────────

pub async fn handle_command(state: &mut BotChatState, cmd: BotCommand) -> HandleResult {
    match cmd {
        BotCommand::Start | BotCommand::Help => {
            if state.paired {
                HandleResult {
                    reply: HELP_MESSAGE.to_string(),
                    actions: main_menu_actions(),
                    forward_to_session: None,
                }
            } else {
                HandleResult {
                    reply: WELCOME_MESSAGE.to_string(),
                    actions: vec![],
                    forward_to_session: None,
                }
            }
        }
        BotCommand::PairingCode(_) => HandleResult {
            reply: "Pairing codes are handled automatically. If you need to re-pair, \
                    please restart the connection from BitFun Desktop."
                .to_string(),
            actions: vec![],
            forward_to_session: None,
        },
        BotCommand::SwitchWorkspace => {
            if !state.paired {
                return not_paired();
            }
            handle_switch_workspace(state).await
        }
        BotCommand::ResumeSession => {
            if !state.paired {
                return not_paired();
            }
            if state.current_workspace.is_none() {
                return need_workspace();
            }
            handle_resume_session(state, 0).await
        }
        BotCommand::NewCodeSession => {
            if !state.paired {
                return not_paired();
            }
            if state.current_workspace.is_none() {
                return need_workspace();
            }
            handle_new_session(state, "agentic").await
        }
        BotCommand::NewCoworkSession => {
            if !state.paired {
                return not_paired();
            }
            if state.current_workspace.is_none() {
                return need_workspace();
            }
            handle_new_session(state, "Cowork").await
        }
        BotCommand::CancelTask(turn_id) => {
            if !state.paired {
                return not_paired();
            }
            handle_cancel_task(state, turn_id.as_deref()).await
        }
        BotCommand::NumberSelection(n) => {
            if !state.paired {
                return not_paired();
            }
            handle_number_selection(state, n).await
        }
        BotCommand::NextPage => {
            if !state.paired {
                return not_paired();
            }
            handle_next_page(state).await
        }
        BotCommand::ChatMessage(msg) => {
            if !state.paired {
                return not_paired();
            }
            handle_chat_message(state, &msg).await
        }
    }
}

// ── Helpers ─────────────────────────────────────────────────────────

fn not_paired() -> HandleResult {
    HandleResult {
        reply: "Not connected to BitFun Desktop. Please enter the 6-digit pairing code first."
            .to_string(),
        actions: vec![],
        forward_to_session: None,
    }
}

fn need_workspace() -> HandleResult {
    HandleResult {
        reply: "No workspace selected. Use /switch_workspace first.".to_string(),
        actions: workspace_required_actions(),
        forward_to_session: None,
    }
}

fn question_option_line(index: usize, option: &BotQuestionOption) -> String {
    if option.description.is_empty() {
        format!("{}. {}", index + 1, option.label)
    } else {
        format!("{}. {} - {}", index + 1, option.label, option.description)
    }
}

fn truncate_action_label(label: &str, max_chars: usize) -> String {
    let trimmed = label.trim();
    if trimmed.chars().count() <= max_chars {
        trimmed.to_string()
    } else {
        let truncated: String = trimmed.chars().take(max_chars.saturating_sub(3)).collect();
        format!("{truncated}...")
    }
}

fn numbered_actions(labels: &[String]) -> Vec<BotAction> {
    labels
        .iter()
        .enumerate()
        .map(|(idx, label)| {
            BotAction::secondary(
                truncate_action_label(label, 28),
                (idx + 1).to_string(),
            )
        })
        .collect()
}

fn build_question_prompt(
    tool_id: String,
    questions: Vec<BotQuestion>,
    current_index: usize,
    answers: Vec<Value>,
    awaiting_custom_text: bool,
    pending_answer: Option<Value>,
) -> BotInteractiveRequest {
    let question = &questions[current_index];
    let mut actions = Vec::new();
    let mut reply = format!(
        "Question {}/{}\n",
        current_index + 1,
        questions.len()
    );
    if !question.header.is_empty() {
        reply.push_str(&format!("{}\n", question.header));
    }
    reply.push_str(&format!("{}\n\n", question.question));
    for (idx, option) in question.options.iter().enumerate() {
        reply.push_str(&format!("{}\n", question_option_line(idx, option)));
    }
    reply.push_str(&format!(
        "{}. Other\n\n",
        question.options.len() + 1
    ));
    if awaiting_custom_text {
        reply.push_str("Please type your custom answer.");
    } else if question.multi_select {
        reply.push_str("Reply with one or more option numbers, separated by commas. Example: 1,3");
    } else {
        reply.push_str("Reply with a single option number.");
        let mut labels: Vec<String> = question
            .options
            .iter()
            .map(|option| option.label.clone())
            .collect();
        labels.push("Other".to_string());
        actions = numbered_actions(&labels);
    }

    BotInteractiveRequest {
        reply,
        actions,
        pending_action: PendingAction::AskUserQuestion {
            tool_id,
            questions,
            current_index,
            answers,
            awaiting_custom_text,
            pending_answer,
        },
    }
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

async fn handle_switch_workspace(state: &mut BotChatState) -> HandleResult {
    use crate::infrastructure::get_workspace_path;
    use crate::service::workspace::get_global_workspace_service;

    let current_ws = get_workspace_path().map(|p| p.to_string_lossy().to_string());

    let ws_service = match get_global_workspace_service() {
        Some(s) => s,
        None => {
            return HandleResult {
                reply: "Workspace service not available.".to_string(),
                actions: vec![],
                forward_to_session: None,
            };
        }
    };

    let workspaces = ws_service.get_recent_workspaces().await;
    if workspaces.is_empty() {
        return HandleResult {
            reply: "No workspaces found. Please open a project in BitFun Desktop first."
                .to_string(),
            actions: vec![],
            forward_to_session: None,
        };
    }

    // Prefer the bot session's own workspace record; fall back to the desktop
    // global path only if the bot has not yet selected one.  Using || across
    // both sources simultaneously can mark two different workspaces as
    // [current] when the desktop and the bot session are on different paths.
    let effective_current: Option<&str> =
        state.current_workspace.as_deref().or(current_ws.as_deref());

    let mut text = String::from("Select a workspace:\n\n");
    let mut options: Vec<(String, String)> = Vec::new();
    for (i, ws) in workspaces.iter().enumerate() {
        let path = ws.root_path.to_string_lossy().to_string();
        let is_current = effective_current == Some(path.as_str());
        let marker = if is_current { " [current]" } else { "" };
        text.push_str(&format!("{}. {}{}\n   {}\n", i + 1, ws.name, marker, path));
        options.push((path, ws.name.clone()));
    }
    text.push_str("\nReply with the workspace number.");

    let action_labels: Vec<String> = options.iter().map(|(_, name)| name.clone()).collect();
    state.pending_action = Some(PendingAction::SelectWorkspace { options });
    HandleResult {
        reply: text,
        actions: numbered_actions(&action_labels),
        forward_to_session: None,
    }
}

async fn handle_resume_session(state: &mut BotChatState, page: usize) -> HandleResult {
    use crate::infrastructure::PathManager;
    use crate::service::conversation::ConversationPersistenceManager;

    let ws_path = match &state.current_workspace {
        Some(p) => std::path::PathBuf::from(p),
        None => return need_workspace(),
    };

    let page_size = 10usize;
    let offset = page * page_size;

    let pm = match PathManager::new() {
        Ok(pm) => std::sync::Arc::new(pm),
        Err(e) => {
            return HandleResult {
                reply: format!("Failed to load sessions: {e}"),
                actions: vec![],
                forward_to_session: None,
            };
        }
    };

    let conv_mgr = match ConversationPersistenceManager::new(pm, ws_path.clone()).await {
        Ok(mgr) => mgr,
        Err(e) => {
            return HandleResult {
                reply: format!("Failed to load sessions: {e}"),
                actions: vec![],
                forward_to_session: None,
            };
        }
    };

    let all_meta = match conv_mgr.get_session_list().await {
        Ok(m) => m,
        Err(e) => {
            return HandleResult {
                reply: format!("Failed to list sessions: {e}"),
                actions: vec![],
                forward_to_session: None,
            };
        }
    };

    if all_meta.is_empty() {
        return HandleResult {
            reply: "No sessions found in this workspace. Use /new_code_session or \
                    /new_cowork_session to create one."
                .to_string(),
            actions: new_session_actions(),
            forward_to_session: None,
        };
    }

    let total = all_meta.len();
    let has_more = offset + page_size < total;
    let sessions: Vec<_> = all_meta.into_iter().skip(offset).take(page_size).collect();

    let ws_name = ws_path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "Unknown".to_string());

    let mut text = format!("Sessions in {} (page {}):\n\n", ws_name, page + 1);
    let mut options: Vec<(String, String)> = Vec::new();
    for (i, s) in sessions.iter().enumerate() {
        let is_current = state.current_session_id.as_deref() == Some(&s.session_id);
        let marker = if is_current { " [current]" } else { "" };
        let ts = chrono::DateTime::from_timestamp(s.last_active_at as i64 / 1000, 0)
            .map(|dt| dt.format("%m-%d %H:%M").to_string())
            .unwrap_or_default();
        let turn_count = s.turn_count;
        let msg_hint = if turn_count == 0 {
            "no messages".to_string()
        } else if turn_count == 1 {
            "1 message".to_string()
        } else {
            format!("{turn_count} messages")
        };
        text.push_str(&format!(
            "{}. [{}] {}{}\n   {} · {}\n",
            i + 1,
            s.agent_type,
            s.session_name,
            marker,
            ts,
            msg_hint,
        ));
        options.push((s.session_id.clone(), s.session_name.clone()));
    }
    if has_more {
        text.push_str("\n0 - Next page\n");
    }
    text.push_str("\nReply with the session number.");

    state.pending_action = Some(PendingAction::SelectSession { options, page, has_more });
    let mut action_labels: Vec<String> = sessions
        .iter()
        .map(|session| format!("[{}] {}", session.agent_type, session.session_name))
        .collect();
    let mut actions = numbered_actions(&action_labels);
    if has_more {
        action_labels.push("Next Page".to_string());
        actions.push(BotAction::secondary("Next Page", "0"));
    }
    HandleResult {
        reply: text,
        actions,
        forward_to_session: None,
    }
}

async fn handle_new_session(state: &mut BotChatState, agent_type: &str) -> HandleResult {
    use crate::agentic::coordination::get_global_coordinator;
    use crate::agentic::core::SessionConfig;

    let coordinator = match get_global_coordinator() {
        Some(c) => c,
        None => {
            return HandleResult {
                reply: "BitFun session system not ready.".to_string(),
                actions: vec![],
                forward_to_session: None,
            };
        }
    };

    let ws_path = state.current_workspace.clone();
    let session_name = match agent_type {
        "Cowork" => "Remote Cowork Session",
        _ => "Remote Code Session",
    };

    match coordinator
        .create_session_with_workspace(
            None,
            session_name.to_string(),
            agent_type.to_string(),
            SessionConfig::default(),
            ws_path.clone(),
        )
        .await
    {
        Ok(session) => {
            let session_id = session.session_id.clone();
            state.current_session_id = Some(session_id.clone());
            let label = if agent_type == "Cowork" {
                "cowork"
            } else {
                "coding"
            };
            let workspace = ws_path.as_deref().unwrap_or("(unknown)");
            HandleResult {
                reply: format!(
                    "Created new {} session: {}\nSession ID: {}\nWorkspace: {}\n\n\
                     You can now send messages to interact with the AI agent.",
                    label, session_name, session_id, workspace
                ),
                actions: vec![],
                forward_to_session: None,
            }
        }
        Err(e) => HandleResult {
            reply: format!("Failed to create session: {e}"),
            actions: vec![],
            forward_to_session: None,
        },
    }
}

async fn handle_number_selection(state: &mut BotChatState, n: usize) -> HandleResult {
    let pending = state.pending_action.take();
    match pending {
        Some(PendingAction::SelectWorkspace { options }) => {
            if n < 1 || n > options.len() {
                state.pending_action = Some(PendingAction::SelectWorkspace { options });
                return HandleResult {
                    reply: format!("Invalid selection. Please enter 1-{}.", state.pending_action.as_ref()
                        .map(|a| match a { PendingAction::SelectWorkspace { options } => options.len(), _ => 0 })
                        .unwrap_or(0)),
                    actions: vec![],
                    forward_to_session: None,
                };
            }
            let (path, name) = options[n - 1].clone();
            select_workspace(state, &path, &name).await
        }
        Some(PendingAction::SelectSession {
            options,
            page,
            has_more,
        }) => {
            if n < 1 || n > options.len() {
                let max = options.len();
                state.pending_action = Some(PendingAction::SelectSession {
                    options,
                    page,
                    has_more,
                });
                return HandleResult {
                    reply: format!("Invalid selection. Please enter 1-{max}."),
                    actions: vec![],
                    forward_to_session: None,
                };
            }
            let (session_id, session_name) = options[n - 1].clone();
            select_session(state, &session_id, &session_name).await
        }
        Some(PendingAction::AskUserQuestion {
            tool_id,
            questions,
            current_index,
            answers,
            awaiting_custom_text,
            pending_answer,
        }) => {
            handle_question_reply(
                state,
                tool_id,
                questions,
                current_index,
                answers,
                awaiting_custom_text,
                pending_answer,
                &n.to_string(),
            )
            .await
        }
        None => handle_chat_message(state, &n.to_string()).await,
    }
}

async fn select_workspace(state: &mut BotChatState, path: &str, name: &str) -> HandleResult {
    use crate::service::workspace::get_global_workspace_service;

    let ws_service = match get_global_workspace_service() {
        Some(s) => s,
        None => {
            return HandleResult {
                reply: "Workspace service not available.".to_string(),
                actions: vec![],
                forward_to_session: None,
            };
        }
    };

    let path_buf = std::path::PathBuf::from(path);
    match ws_service.open_workspace(path_buf).await {
        Ok(info) => {
            if let Err(e) = crate::service::snapshot::initialize_global_snapshot_manager(
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
            let reply = build_workspace_switched_reply(name, session_count);
            let actions = if session_count > 0 {
                session_entry_actions()
            } else {
                new_session_actions()
            };
            HandleResult {
                reply,
                actions,
                forward_to_session: None,
            }
        }
        Err(e) => HandleResult {
            reply: format!("Failed to switch workspace: {e}"),
            actions: vec![],
            forward_to_session: None,
        },
    }
}

async fn count_workspace_sessions(workspace_path: &str) -> usize {
    use crate::infrastructure::PathManager;
    use crate::service::conversation::ConversationPersistenceManager;

    let wp = std::path::PathBuf::from(workspace_path);
    let pm = match PathManager::new() {
        Ok(pm) => std::sync::Arc::new(pm),
        Err(_) => return 0,
    };
    let conv_mgr = match ConversationPersistenceManager::new(pm, wp).await {
        Ok(m) => m,
        Err(_) => return 0,
    };
    conv_mgr
        .get_session_list()
        .await
        .map(|v| v.len())
        .unwrap_or(0)
}

fn build_workspace_switched_reply(name: &str, session_count: usize) -> String {
    let mut reply = format!("Switched to workspace: {name}\n\n");
    if session_count > 0 {
        let s = if session_count == 1 { "" } else { "s" };
        reply.push_str(&format!(
            "This workspace has {session_count} existing session{s}. What would you like to do?\n\n\
             /resume_session - Resume an existing session\n\
             /new_code_session - Start a new coding session\n\
             /new_cowork_session - Start a new cowork session"
        ));
    } else {
        reply.push_str(
            "No sessions found in this workspace. What would you like to do?\n\n\
             /new_code_session - Start a new coding session\n\
             /new_cowork_session - Start a new cowork session",
        );
    }
    reply
}

async fn select_session(
    state: &mut BotChatState,
    session_id: &str,
    session_name: &str,
) -> HandleResult {
    state.current_session_id = Some(session_id.to_string());
    info!("Bot resumed session: {session_id}");

    let last_pair =
        load_last_dialog_pair_from_turns(state.current_workspace.as_deref(), session_id).await;

    let mut reply = format!("Resumed session: {session_name}\n\n");
    if let Some((user_text, assistant_text)) = last_pair {
        reply.push_str("— Last conversation —\n");
        reply.push_str(&format!("You: {user_text}\n\n"));
        reply.push_str(&format!("AI: {assistant_text}\n\n"));
        reply.push_str("You can continue the conversation.");
    } else {
        reply.push_str("You can now send messages to interact with the AI agent.");
    }

    HandleResult {
        reply,
        actions: vec![],
        forward_to_session: None,
    }
}

/// Load the last user/assistant dialog pair from ConversationPersistenceManager,
/// the same data source the desktop frontend uses.
async fn load_last_dialog_pair_from_turns(
    workspace_path: Option<&str>,
    session_id: &str,
) -> Option<(String, String)> {
    use crate::infrastructure::PathManager;
    use crate::service::conversation::ConversationPersistenceManager;

    const MAX_USER_LEN: usize = 200;
    const MAX_AI_LEN: usize = 400;

    let wp = std::path::PathBuf::from(workspace_path?);
    let pm = std::sync::Arc::new(PathManager::new().ok()?);
    let conv_mgr = ConversationPersistenceManager::new(pm, wp).await.ok()?;
    let turns = conv_mgr.load_session_turns(session_id).await.ok()?;
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

/// Strip XML wrapper tags injected by wrap_user_input before storing the message:
///   <user_query>\n{content}\n</user_query>\n<system_reminder>...</system_reminder>
fn strip_user_message_tags(raw: &str) -> String {
    let text = raw.trim();

    // Extract content inside <user_query>...</user_query> if present.
    let inner = if let Some(start) = text.find("<user_query>") {
        let after_open = &text[start + "<user_query>".len()..];
        if let Some(end) = after_open.find("</user_query>") {
            after_open[..end].trim()
        } else {
            // Malformed — use everything after the opening tag.
            after_open.trim()
        }
    } else {
        text
    };

    // Drop any trailing <system_reminder> block.
    let result = if let Some(reminder_pos) = inner.find("<system_reminder>") {
        inner[..reminder_pos].trim()
    } else {
        inner.trim()
    };

    result.to_string()
}

fn truncate_text(text: &str, max_chars: usize) -> String {
    let trimmed = text.trim();
    if trimmed.chars().count() <= max_chars {
        trimmed.to_string()
    } else {
        let truncated: String = trimmed.chars().take(max_chars).collect();
        format!("{truncated}...")
    }
}

async fn handle_cancel_task(
    state: &mut BotChatState,
    requested_turn_id: Option<&str>,
) -> HandleResult {
    use crate::service::remote_connect::remote_server::get_or_init_global_dispatcher;

    let session_id = match state.current_session_id.clone() {
        Some(id) => id,
        None => {
            return HandleResult {
                reply: "No active session to cancel.".to_string(),
                actions: session_entry_actions(),
                forward_to_session: None,
            };
        }
    };

    let dispatcher = get_or_init_global_dispatcher();
    match dispatcher
        .cancel_task(&session_id, requested_turn_id)
        .await
    {
        Ok(_) => {
            state.pending_action = None;
            HandleResult {
                reply: "Cancellation requested for the current task.".to_string(),
                actions: vec![],
                forward_to_session: None,
            }
        }
        Err(e) => HandleResult {
            reply: format!("Failed to cancel task: {e}"),
            actions: vec![],
            forward_to_session: None,
        },
    }
}

fn restore_question_pending_action(
    state: &mut BotChatState,
    tool_id: String,
    questions: Vec<BotQuestion>,
    current_index: usize,
    answers: Vec<Value>,
    awaiting_custom_text: bool,
    pending_answer: Option<Value>,
) {
    state.pending_action = Some(PendingAction::AskUserQuestion {
        tool_id,
        questions,
        current_index,
        answers,
        awaiting_custom_text,
        pending_answer,
    });
}

async fn submit_question_answers(tool_id: &str, answers: &[Value]) -> HandleResult {
    use crate::agentic::tools::user_input_manager::get_user_input_manager;

    let mut payload = serde_json::Map::new();
    for (idx, value) in answers.iter().enumerate() {
        payload.insert(idx.to_string(), value.clone());
    }

    let manager = get_user_input_manager();
    match manager.send_answer(tool_id, Value::Object(payload)) {
        Ok(_) => HandleResult {
            reply: "Answers submitted. Waiting for the assistant to continue...".to_string(),
            actions: vec![],
            forward_to_session: None,
        },
        Err(e) => HandleResult {
            reply: format!("Failed to submit answers: {e}"),
            actions: vec![],
            forward_to_session: None,
        },
    }
}

async fn handle_question_reply(
    state: &mut BotChatState,
    tool_id: String,
    questions: Vec<BotQuestion>,
    current_index: usize,
    mut answers: Vec<Value>,
    awaiting_custom_text: bool,
    pending_answer: Option<Value>,
    message: &str,
) -> HandleResult {
    let Some(question) = questions.get(current_index).cloned() else {
        return HandleResult {
            reply: "Question state is invalid.".to_string(),
            actions: vec![],
            forward_to_session: None,
        };
    };

    if awaiting_custom_text {
        let custom_text = message.trim();
        if custom_text.is_empty() {
            restore_question_pending_action(
                state,
                tool_id,
                questions,
                current_index,
                answers,
                true,
                pending_answer,
            );
            return HandleResult {
                reply: "Custom answer cannot be empty. Please type your custom answer.".to_string(),
                actions: vec![],
                forward_to_session: None,
            };
        }

        let final_value = match pending_answer {
            Some(Value::String(_)) => Value::String(custom_text.to_string()),
            Some(Value::Array(existing)) => {
                let mut values: Vec<Value> = existing
                    .into_iter()
                    .filter(|value| value.as_str() != Some("Other"))
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
                restore_question_pending_action(
                    state,
                    tool_id,
                    questions,
                    current_index,
                    answers,
                    false,
                    None,
                );
                return HandleResult {
                    reply: if question.multi_select {
                        "Invalid input. Reply with option numbers like `1,3`.".to_string()
                    } else {
                        "Invalid input. Reply with a single option number.".to_string()
                    },
                    actions: vec![],
                    forward_to_session: None,
                };
            }
        };

        if !question.multi_select && selections.len() != 1 {
            restore_question_pending_action(
                state,
                tool_id,
                questions,
                current_index,
                answers,
                false,
                None,
            );
            return HandleResult {
                reply: "Please reply with a single option number.".to_string(),
                actions: vec![],
                forward_to_session: None,
            };
        }

        let other_index = question.options.len() + 1;
        let mut labels = Vec::new();
        let mut includes_other = false;
        for selection in selections {
            if selection == other_index {
                includes_other = true;
                labels.push(Value::String("Other".to_string()));
            } else if selection >= 1 && selection <= question.options.len() {
                labels.push(Value::String(
                    question.options[selection - 1].label.clone(),
                ));
            } else {
                restore_question_pending_action(
                    state,
                    tool_id,
                    questions,
                    current_index,
                    answers,
                    false,
                    None,
                );
                return HandleResult {
                    reply: format!(
                        "Invalid selection. Please choose between 1 and {}.",
                        other_index
                    ),
                    actions: vec![],
                    forward_to_session: None,
                };
            }
        }

        let pending_answer = if question.multi_select {
            Some(Value::Array(labels.clone()))
        } else {
            labels.into_iter().next()
        };

        if includes_other {
            restore_question_pending_action(
                state,
                tool_id,
                questions,
                current_index,
                answers,
                true,
                pending_answer,
            );
            return HandleResult {
                reply: "Please type your custom answer for `Other`.".to_string(),
                actions: vec![],
                forward_to_session: None,
            };
        }

        answers.push(if question.multi_select {
            pending_answer.unwrap_or_else(|| Value::Array(Vec::new()))
        } else {
            pending_answer.unwrap_or_else(|| Value::String(String::new()))
        });
    }

    if current_index + 1 < questions.len() {
        let prompt = build_question_prompt(
            tool_id,
            questions,
            current_index + 1,
            answers,
            false,
            None,
        );
        restore_question_pending_action(
            state,
            match &prompt.pending_action {
                PendingAction::AskUserQuestion { tool_id, .. } => tool_id.clone(),
                _ => String::new(),
            },
            match &prompt.pending_action {
                PendingAction::AskUserQuestion { questions, .. } => questions.clone(),
                _ => Vec::new(),
            },
            match &prompt.pending_action {
                PendingAction::AskUserQuestion { current_index, .. } => *current_index,
                _ => 0,
            },
            match &prompt.pending_action {
                PendingAction::AskUserQuestion { answers, .. } => answers.clone(),
                _ => Vec::new(),
            },
            false,
            None,
        );
        return HandleResult {
            reply: prompt.reply,
            actions: prompt.actions,
            forward_to_session: None,
        };
    }

    submit_question_answers(&tool_id, &answers).await
}

async fn handle_next_page(state: &mut BotChatState) -> HandleResult {
    let pending = state.pending_action.take();
    match pending {
        Some(PendingAction::SelectSession { page, has_more, .. }) if has_more => {
            handle_resume_session(state, page + 1).await
        }
        Some(action) => {
            state.pending_action = Some(action);
            HandleResult {
                reply: "No more pages available.".to_string(),
                actions: vec![],
                forward_to_session: None,
            }
        }
        None => handle_chat_message(state, "0").await,
    }
}

async fn handle_chat_message(state: &mut BotChatState, message: &str) -> HandleResult {
    if let Some(PendingAction::AskUserQuestion {
        tool_id,
        questions,
        current_index,
        answers,
        awaiting_custom_text,
        pending_answer,
    }) = state.pending_action.take()
    {
        return handle_question_reply(
            state,
            tool_id,
            questions,
            current_index,
            answers,
            awaiting_custom_text,
            pending_answer,
            message,
        )
        .await;
    }
    if let Some(pending) = state.pending_action.clone() {
        return match pending {
            PendingAction::SelectWorkspace { .. } => HandleResult {
                reply: "Please reply with the workspace number.".to_string(),
                actions: vec![],
                forward_to_session: None,
            },
            PendingAction::SelectSession { has_more, .. } => HandleResult {
                reply: if has_more {
                    "Please reply with the session number, or `0` for the next page.".to_string()
                } else {
                    "Please reply with the session number.".to_string()
                },
                actions: vec![],
                forward_to_session: None,
            },
            PendingAction::AskUserQuestion { .. } => unreachable!(),
        };
    }

    if state.current_workspace.is_none() {
        return HandleResult {
            reply: "No workspace selected. Use /switch_workspace to select one first.".to_string(),
            actions: workspace_required_actions(),
            forward_to_session: None,
        };
    }
    if state.current_session_id.is_none() {
        return HandleResult {
            reply: "No active session. Use /resume_session to resume one or \
                    /new_code_session to create a new one."
                .to_string(),
            actions: session_entry_actions(),
            forward_to_session: None,
        };
    }

    let session_id = state.current_session_id.clone().unwrap();
    let turn_id = format!("turn_{}", uuid::Uuid::new_v4());
    let cancel_command = format!("/cancel_task {}", turn_id);
    HandleResult {
        reply: format!(
            "Processing your message...\n\nIf needed, send `{}` to stop this request.",
            cancel_command
        ),
        actions: cancel_task_actions(cancel_command),
        forward_to_session: Some(ForwardRequest {
            session_id,
            content: message.to_string(),
            agent_type: "agentic".to_string(),
            turn_id,
        }),
    }
}

// ── Forwarded-turn execution ────────────────────────────────────────

/// Execute a forwarded dialog turn and return the AI response text.
///
/// Called from the bot implementations after `handle_command` returns a
/// `ForwardRequest`.  Dispatches the command through
/// `RemoteExecutionDispatcher` (the same path used by mobile), then
/// subscribes to the tracker's broadcast channel for real-time events.
///
/// `message_sender` is called to send intermediate messages (e.g. thinking
/// content) before the final response is returned.
pub async fn execute_forwarded_turn(
    forward: ForwardRequest,
    interaction_handler: Option<BotInteractionHandler>,
    message_sender: Option<BotMessageSender>,
) -> String {
    use crate::agentic::coordination::DialogTriggerSource;
    use crate::service::remote_connect::remote_server::{
        get_or_init_global_dispatcher, TrackerEvent,
    };

    let dispatcher = get_or_init_global_dispatcher();

    let tracker = dispatcher.ensure_tracker(&forward.session_id);
    let mut event_rx = tracker.subscribe();

    if let Err(e) = dispatcher
        .send_message(
            &forward.session_id,
            forward.content,
            Some(&forward.agent_type),
            None,
            DialogTriggerSource::Bot,
            Some(forward.turn_id),
        )
        .await
    {
        return format!("Failed to send message: {e}");
    }

    let result = tokio::time::timeout(std::time::Duration::from_secs(300), async {
        let mut thinking = String::new();
        let mut response = String::new();
        loop {
            match event_rx.recv().await {
                Ok(event) => match event {
                    TrackerEvent::ThinkingChunk(t) => thinking.push_str(&t),
                    TrackerEvent::ThinkingEnd => {
                        if !thinking.is_empty() {
                            if let Some(sender) = message_sender.as_ref() {
                                sender(thinking.clone()).await;
                            }
                            thinking.clear();
                        }
                    }
                    TrackerEvent::TextChunk(t) => response.push_str(&t),
                    TrackerEvent::ToolStarted {
                        tool_id,
                        tool_name,
                        params,
                    } if tool_name == "AskUserQuestion" => {
                        if let Some(questions_value) =
                            params.and_then(|p| p.get("questions").cloned())
                        {
                            if let Ok(questions) =
                                serde_json::from_value::<Vec<BotQuestion>>(questions_value)
                            {
                                let request = build_question_prompt(
                                    tool_id,
                                    questions,
                                    0,
                                    Vec::new(),
                                    false,
                                    None,
                                );
                                if let Some(handler) = interaction_handler.as_ref() {
                                    handler(request).await;
                                }
                            }
                        }
                    }
                    TrackerEvent::TurnCompleted => break,
                    TrackerEvent::TurnFailed(e) => return format!("Error: {e}"),
                    TrackerEvent::TurnCancelled => {
                        return "Task was cancelled.".to_string();
                    }
                    _ => {}
                },
                Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                    log::warn!("Bot event receiver lagged by {n} events");
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                    break;
                }
            }
        }
        response
    })
    .await;

    match result {
        Ok(text) if text.is_empty() => "(No response)".to_string(),
        Ok(mut text) => {
            const MAX_BOT_MSG_LEN: usize = 4000;
            if text.len() > MAX_BOT_MSG_LEN {
                text.truncate(MAX_BOT_MSG_LEN);
                text.push_str("\n\n... (truncated)");
            }
            text
        }
        Err(_) => "Response timed out after 5 minutes.".to_string(),
    }
}
