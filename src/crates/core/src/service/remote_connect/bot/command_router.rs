//! Shared command router for bot-based connections (Telegram & Feishu).
//!
//! Provides platform-agnostic command parsing, per-chat state management, and
//! dispatch to workspace / session services.  Each platform adapter handles
//! message I/O while this module owns the business logic.

use log::{error, info};
use serde::{Deserialize, Serialize};

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
}

// ── Parsed command ──────────────────────────────────────────────────

#[derive(Debug)]
pub enum BotCommand {
    Start,
    SwitchWorkspace,
    ResumeSession,
    NewCodeSession,
    NewCoworkSession,
    Help,
    PairingCode(String),
    NumberSelection(usize),
    NextPage,
    ChatMessage(String),
}

// ── Handle result ───────────────────────────────────────────────────

pub struct HandleResult {
    pub reply: String,
    pub forward_to_session: Option<ForwardRequest>,
}

pub struct ForwardRequest {
    pub session_id: String,
    pub content: String,
    pub agent_type: String,
    pub workspace_path: Option<String>,
}

// ── Command parsing ─────────────────────────────────────────────────

pub fn parse_command(text: &str) -> BotCommand {
    let trimmed = text.trim();
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
/help - Show this help message";

pub fn paired_success_message() -> String {
    format!("Pairing successful! BitFun is now connected.\n\n{}", HELP_MESSAGE)
}

// ── Main dispatch ───────────────────────────────────────────────────

pub async fn handle_command(state: &mut BotChatState, cmd: BotCommand) -> HandleResult {
    match cmd {
        BotCommand::Start | BotCommand::Help => {
            let reply = if state.paired {
                HELP_MESSAGE.to_string()
            } else {
                WELCOME_MESSAGE.to_string()
            };
            HandleResult { reply, forward_to_session: None }
        }
        BotCommand::PairingCode(_) => HandleResult {
            reply: "Pairing codes are handled automatically. If you need to re-pair, \
                    please restart the connection from BitFun Desktop."
                .to_string(),
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
        forward_to_session: None,
    }
}

fn need_workspace() -> HandleResult {
    HandleResult {
        reply: "No workspace selected. Use /switch_workspace first.".to_string(),
        forward_to_session: None,
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
                forward_to_session: None,
            };
        }
    };

    let workspaces = ws_service.get_recent_workspaces().await;
    if workspaces.is_empty() {
        return HandleResult {
            reply: "No workspaces found. Please open a project in BitFun Desktop first."
                .to_string(),
            forward_to_session: None,
        };
    }

    // Prefer the bot session's own workspace record; fall back to the desktop
    // global path only if the bot has not yet selected one.  Using || across
    // both sources simultaneously can mark two different workspaces as
    // [current] when the desktop and the bot session are on different paths.
    let effective_current: Option<&str> = state
        .current_workspace
        .as_deref()
        .or(current_ws.as_deref());

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

    state.pending_action = Some(PendingAction::SelectWorkspace { options });
    HandleResult { reply: text, forward_to_session: None }
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
                forward_to_session: None,
            };
        }
    };

    let conv_mgr = match ConversationPersistenceManager::new(pm, ws_path.clone()).await {
        Ok(mgr) => mgr,
        Err(e) => {
            return HandleResult {
                reply: format!("Failed to load sessions: {e}"),
                forward_to_session: None,
            };
        }
    };

    let all_meta = match conv_mgr.get_session_list().await {
        Ok(m) => m,
        Err(e) => {
            return HandleResult {
                reply: format!("Failed to list sessions: {e}"),
                forward_to_session: None,
            };
        }
    };

    if all_meta.is_empty() {
        return HandleResult {
            reply: "No sessions found in this workspace. Use /new_code_session or \
                    /new_cowork_session to create one."
                .to_string(),
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
    HandleResult { reply: text, forward_to_session: None }
}

async fn handle_new_session(state: &mut BotChatState, agent_type: &str) -> HandleResult {
    use crate::agentic::coordination::get_global_coordinator;
    use crate::agentic::core::SessionConfig;

    let coordinator = match get_global_coordinator() {
        Some(c) => c,
        None => {
            return HandleResult {
                reply: "BitFun session system not ready.".to_string(),
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
            persist_new_session(&session_id, session_name, agent_type, ws_path.as_deref()).await;
            state.current_session_id = Some(session_id.clone());
            let label = if agent_type == "Cowork" { "cowork" } else { "coding" };
            let workspace = ws_path.as_deref().unwrap_or("(unknown)");
            HandleResult {
                reply: format!(
                    "Created new {} session: {}\nSession ID: {}\nWorkspace: {}\n\n\
                     You can now send messages to interact with the AI agent.",
                    label, session_name, session_id, workspace
                ),
                forward_to_session: None,
            }
        }
        Err(e) => HandleResult {
            reply: format!("Failed to create session: {e}"),
            forward_to_session: None,
        },
    }
}

async fn persist_new_session(
    session_id: &str,
    session_name: &str,
    agent_type: &str,
    workspace_path: Option<&str>,
) {
    use crate::infrastructure::PathManager;
    use crate::service::conversation::{
        ConversationPersistenceManager, SessionMetadata, SessionStatus,
    };

    let Some(wp_str) = workspace_path else { return };
    let wp = std::path::PathBuf::from(wp_str);

    let pm = match PathManager::new() {
        Ok(pm) => std::sync::Arc::new(pm),
        Err(_) => return,
    };
    let conv_mgr = match ConversationPersistenceManager::new(pm, wp).await {
        Ok(m) => m,
        Err(_) => return,
    };

    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    let meta = SessionMetadata {
        session_id: session_id.to_string(),
        session_name: session_name.to_string(),
        agent_type: agent_type.to_string(),
        model_name: "default".to_string(),
        created_at: now_ms,
        last_active_at: now_ms,
        turn_count: 0,
        message_count: 0,
        tool_call_count: 0,
        status: SessionStatus::Active,
        terminal_session_id: None,
        snapshot_session_id: None,
        tags: vec![],
        custom_metadata: None,
        todos: None,
        workspace_path: workspace_path.map(String::from),
    };
    if let Err(e) = conv_mgr.save_session_metadata(&meta).await {
        error!("Failed to persist bot session metadata: {e}");
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
                    forward_to_session: None,
                };
            }
            let (path, name) = options[n - 1].clone();
            select_workspace(state, &path, &name).await
        }
        Some(PendingAction::SelectSession { options, page, has_more }) => {
            if n < 1 || n > options.len() {
                let max = options.len();
                state.pending_action = Some(PendingAction::SelectSession { options, page, has_more });
                return HandleResult {
                    reply: format!("Invalid selection. Please enter 1-{max}."),
                    forward_to_session: None,
                };
            }
            let (session_id, session_name) = options[n - 1].clone();
            select_session(state, &session_id, &session_name).await
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
            HandleResult { reply, forward_to_session: None }
        }
        Err(e) => HandleResult {
            reply: format!("Failed to switch workspace: {e}"),
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
    conv_mgr.get_session_list().await.map(|v| v.len()).unwrap_or(0)
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
    use crate::agentic::coordination::get_global_coordinator;

    if let Some(coordinator) = get_global_coordinator() {
        let _ = coordinator.restore_session(session_id).await;
    }

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

    HandleResult { reply, forward_to_session: None }
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
                forward_to_session: None,
            }
        }
        None => handle_chat_message(state, "0").await,
    }
}

async fn handle_chat_message(state: &mut BotChatState, message: &str) -> HandleResult {
    if state.current_workspace.is_none() {
        return HandleResult {
            reply: "No workspace selected. Use /switch_workspace to select one first.".to_string(),
            forward_to_session: None,
        };
    }
    if state.current_session_id.is_none() {
        return HandleResult {
            reply: "No active session. Use /resume_session to resume one or \
                    /new_code_session to create a new one."
                .to_string(),
            forward_to_session: None,
        };
    }

    let session_id = state.current_session_id.clone().unwrap();
    let workspace_path = state.current_workspace.clone();

    let agent_type = {
        use crate::agentic::coordination::get_global_coordinator;
        get_global_coordinator()
            .and_then(|c| {
                c.get_session_manager()
                    .get_session(&session_id)
                    .map(|s| s.agent_type.clone())
            })
            .unwrap_or_else(|| "agentic".to_string())
    };

    HandleResult {
        reply: "Processing your message...".to_string(),
        forward_to_session: Some(ForwardRequest {
            session_id,
            content: message.to_string(),
            agent_type,
            workspace_path,
        }),
    }
}

// ── Forwarded-turn execution ────────────────────────────────────────

enum StreamChunk {
    Text(String),
    Done,
    Error(String),
}

struct BotResponseCollector {
    session_id: String,
    chunk_tx: tokio::sync::mpsc::UnboundedSender<StreamChunk>,
}

#[async_trait::async_trait]
impl crate::agentic::events::EventSubscriber for BotResponseCollector {
    async fn on_event(
        &self,
        event: &crate::agentic::events::AgenticEvent,
    ) -> crate::util::errors::BitFunResult<()> {
        use bitfun_events::AgenticEvent as AE;
        match event {
            AE::TextChunk { text, session_id, .. } if session_id == &self.session_id => {
                let _ = self.chunk_tx.send(StreamChunk::Text(text.clone()));
            }
            AE::DialogTurnCompleted { session_id, .. } if session_id == &self.session_id => {
                let _ = self.chunk_tx.send(StreamChunk::Done);
            }
            AE::DialogTurnFailed { session_id, error, .. } if session_id == &self.session_id => {
                let _ = self.chunk_tx.send(StreamChunk::Error(error.clone()));
            }
            _ => {}
        }
        Ok(())
    }
}

/// Execute a forwarded dialog turn and return the AI response text.
///
/// Called from the bot implementations after `handle_command` returns a
/// `ForwardRequest`.  Subscribes to session events, starts the turn, and
/// collects text chunks until completion or timeout.
pub async fn execute_forwarded_turn(forward: ForwardRequest) -> String {
    use crate::agentic::coordination::get_global_coordinator;

    let coordinator = match get_global_coordinator() {
        Some(c) => c,
        None => return "Session system not ready.".to_string(),
    };

    if let Some(wp) = &forward.workspace_path {
        use crate::infrastructure::{get_workspace_path, set_workspace_path};
        let current = get_workspace_path().map(|p| p.to_string_lossy().to_string());
        if current.as_deref() != Some(wp.as_str()) {
            set_workspace_path(Some(std::path::PathBuf::from(wp)));
        }
    }

    let (chunk_tx, mut chunk_rx) = tokio::sync::mpsc::unbounded_channel::<StreamChunk>();
    let subscriber_id = format!("bot_forward_{}", uuid::Uuid::new_v4());
    let collector = BotResponseCollector {
        session_id: forward.session_id.clone(),
        chunk_tx,
    };
    coordinator.subscribe_internal(subscriber_id.clone(), collector);

    let turn_id = format!("turn_{}", chrono::Utc::now().timestamp_millis());
    if let Err(e) = coordinator
        .start_dialog_turn(
            forward.session_id.clone(),
            forward.content,
            Some(turn_id),
            forward.agent_type,
            true,
        )
        .await
    {
        coordinator.unsubscribe_internal(&subscriber_id);
        return format!("Failed to send message: {e}");
    }

    let sub_id = subscriber_id.clone();
    let result = tokio::time::timeout(std::time::Duration::from_secs(300), async {
        let mut response = String::new();
        while let Some(chunk) = chunk_rx.recv().await {
            match chunk {
                StreamChunk::Text(t) => response.push_str(&t),
                StreamChunk::Done => break,
                StreamChunk::Error(e) => return format!("Error: {e}"),
            }
        }
        response
    })
    .await;

    if let Some(coord) = get_global_coordinator() {
        coord.unsubscribe_internal(&sub_id);
    }

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
