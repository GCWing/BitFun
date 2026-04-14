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

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum BotLanguage {
    #[serde(rename = "zh-CN")]
    ZhCN,
    #[serde(rename = "en-US")]
    EnUS,
}

impl BotLanguage {
    pub fn is_chinese(self) -> bool {
        matches!(self, Self::ZhCN)
    }
}

/// Display mode for bot sessions - Professional or Assistant
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum BotDisplayMode {
    /// Professional mode: can create Code/Cowork sessions
    #[serde(rename = "pro")]
    Pro,
    /// Assistant mode: can create Claw sessions
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
    pub current_session_id: Option<String>,
    /// Display mode: Professional (Pro) or Assistant
    #[serde(default)]
    pub display_mode: BotDisplayMode,
    #[serde(skip)]
    pub pending_action: Option<PendingAction>,
    /// Pending file downloads awaiting user confirmation.
    /// Key: short token embedded in the download button callback.
    /// Value: absolute file path on the desktop.
    /// Not persisted — cleared on bot restart.
    #[serde(skip)]
    pub pending_files: std::collections::HashMap<String, String>,
    /// Commands for the last bot message that had quick actions (1 → `actions[0].command`).
    /// Not persisted — used so numeric replies work like OpenClaw menu numbers.
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
            current_session_id: None,
            display_mode: BotDisplayMode::Assistant,
            pending_action: None,
            pending_files: std::collections::HashMap::new(),
            last_menu_commands: Vec::new(),
        }
    }
}

pub async fn current_bot_language() -> BotLanguage {
    if let Some(service) = crate::service::get_global_i18n_service().await {
        match service.get_current_locale().await {
            crate::service::LocaleId::ZhCN => BotLanguage::ZhCN,
            crate::service::LocaleId::EnUS => BotLanguage::EnUS,
        }
    } else {
        BotLanguage::ZhCN
    }
}

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
}

// ── Parsed command ──────────────────────────────────────────────────

#[derive(Debug)]
pub enum BotCommand {
    Start,
    SwitchWorkspace,
    SwitchAssistant,
    SwitchMode(BotDisplayMode),
    ResumeSession,
    NewCodeSession,
    NewCoworkSession,
    NewClawSession,
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

/// Result returned by [`execute_forwarded_turn`].
pub struct ForwardedTurnResult {
    /// Truncated text suitable for display in bot messages (≤ 4000 chars).
    pub display_text: String,
    /// Full untruncated response text from the tracker, suitable for
    /// downloadable file link extraction.  Not affected by broadcast lag.
    pub full_text: String,
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

/// Strip trailing list punctuation so "1." / "1、" / "1）" still parse as menu numbers.
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
    match trimmed {
        "/start" => BotCommand::Start,
        "/switch_workspace" => BotCommand::SwitchWorkspace,
        "/switch_assistant" => BotCommand::SwitchAssistant,
        "/pro" => BotCommand::SwitchMode(BotDisplayMode::Pro),
        "/assistant" => BotCommand::SwitchMode(BotDisplayMode::Assistant),
        "/resume_session" => BotCommand::ResumeSession,
        "/new_code_session" => BotCommand::NewCodeSession,
        "/new_cowork_session" => BotCommand::NewCoworkSession,
        "/new_claw_session" => BotCommand::NewClawSession,
        "/help" => BotCommand::Help,
        "0" => BotCommand::NextPage,
        _ => {
            if trimmed.len() == 6 && trimmed.chars().all(|c| c.is_ascii_digit()) {
                BotCommand::PairingCode(trimmed.to_string())
            } else {
                let num_token = strip_numeric_reply_suffix(trimmed);
                if let Ok(n) = num_token.parse::<usize>() {
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
}

// ── Static messages ─────────────────────────────────────────────────

pub fn welcome_message(language: BotLanguage) -> &'static str {
    if language.is_chinese() {
        "\
欢迎使用 BitFun！

要连接你的 BitFun 桌面端，请发送 BitFun Remote Connect 面板里显示的 6 位配对码。

如果你还没有配对码，请打开 BitFun Desktop -> Remote Connect -> Telegram/飞书/微信机器人，复制 6 位配对码并发送到这里。"
    } else {
        "\
Welcome to BitFun!

To connect your BitFun desktop app, please enter the 6-digit pairing code shown in your BitFun Remote Connect panel.

Need a pairing code? Open BitFun Desktop -> Remote Connect -> Telegram/Feishu/WeChat bot -> copy the 6-digit code and send it here."
    }
}

pub fn help_message(language: BotLanguage) -> &'static str {
    if language.is_chinese() {
        "\
可用命令：
/switch_workspace - 列出并切换工作区（专业模式）
/switch_assistant - 列出并切换助理（助理模式）
/pro - 切换到专业模式（可创建 Code/Cowork 会话）
/assistant - 切换到助理模式（可创建助理会话）
/verbose - 开启详细模式（显示任务执行过程）
/concise - 开启简洁模式（仅显示最终结果）
/new_code_session - 创建新的编码会话（专业模式）
/new_cowork_session - 创建新的协作会话（专业模式）
/new_claw_session - 创建新的助理会话（助理模式）
/cancel_task - 取消当前任务
/help - 显示帮助信息"
    } else {
        "\
Available commands:
/switch_workspace - List and switch workspaces (Expert mode)
/switch_assistant - List and switch assistants (Assistant mode)
/pro - Switch to Expert mode (can create Code/Cowork sessions)
/assistant - Switch to Assistant mode (can create Claw sessions)
/verbose - Enable verbose mode (show task execution progress)
/concise - Enable concise mode (only show final results)
/new_code_session - Create a new coding session (Expert mode)
/new_cowork_session - Create a new cowork session (Expert mode)
/new_claw_session - Create a new claw session (Assistant mode)
/cancel_task - Cancel the current task
/help - Show this help message"
    }
}

pub fn paired_success_message(language: BotLanguage) -> String {
    if language.is_chinese() {
        format!("配对成功！BitFun 已连接。\n\n{}", help_message(language))
    } else {
        format!(
            "Pairing successful! BitFun is now connected.\n\n{}",
            help_message(language)
        )
    }
}

/// After IM pairing: assistant mode, default assistant workspace, create a fresh Claw session.
/// Always creates a new session so the user starts with a clean context in the IM channel.
/// Mutates `state` (`display_mode`, `current_assistant`, `current_session_id`). Does not set `paired`.
pub async fn bootstrap_im_chat_after_pairing(state: &mut BotChatState) -> String {
    use crate::service::workspace::get_global_workspace_service;

    state.display_mode = BotDisplayMode::Assistant;
    let language = current_bot_language().await;

    let ws_service = match get_global_workspace_service() {
        Some(s) => s,
        None => {
            return if language.is_chinese() {
                "自动准备未能完成：工作区服务不可用。请稍后在 BitFun 桌面端打开工作区后再试。"
                    .to_string()
            } else {
                "Auto-setup incomplete: workspace service unavailable. Open a workspace in BitFun Desktop and try again."
                    .to_string()
            };
        }
    };

    let mut assistants = ws_service.get_assistant_workspaces().await;
    if assistants.is_empty() {
        match ws_service.create_assistant_workspace(None).await {
            Ok(w) => assistants.push(w),
            Err(e) => {
                return if language.is_chinese() {
                    format!(
                        "自动准备未能完成：无法创建助理工作区（{e}）。请使用 /switch_assistant。"
                    )
                } else {
                    format!(
                        "Auto-setup incomplete: could not create assistant workspace ({e}). Use /switch_assistant."
                    )
                };
            }
        }
    }

    let picked = assistants
        .iter()
        .find(|w| w.assistant_id.is_none())
        .cloned()
        .or_else(|| assistants.first().cloned());

    let Some(ws_info) = picked else {
        return if language.is_chinese() {
            "自动准备未能完成：没有可用助理。请使用 /switch_assistant。".to_string()
        } else {
            "Auto-setup incomplete: no assistant found. Use /switch_assistant.".to_string()
        };
    };

    let path_buf = ws_info.root_path.clone();
    if let Err(e) = ws_service.open_workspace(path_buf.clone()).await {
        return if language.is_chinese() {
            format!("自动准备未能完成：无法打开助理工作区（{e}）。")
        } else {
            format!("Auto-setup incomplete: failed to open assistant workspace ({e}).")
        };
    }
    if let Err(e) =
        crate::service::snapshot::initialize_snapshot_manager_for_workspace(path_buf, None).await
    {
        error!("IM bot bootstrap: snapshot init after pairing: {e}");
    }

    state.current_assistant = Some(ws_info.root_path.to_string_lossy().to_string());
    state.current_session_id = None;

    let create_res = handle_new_session(state, "Claw").await;
    if state.current_session_id.is_none() {
        return if language.is_chinese() {
            format!(
                "已进入助理模式，但未能自动创建会话：{}",
                create_res.reply.lines().next().unwrap_or("未知错误")
            )
        } else {
            format!(
                "Assistant mode is on, but session creation failed: {}",
                create_res.reply.lines().next().unwrap_or("unknown error")
            )
        };
    }

    if language.is_chinese() {
        "已进入助理模式，已为你新建助理会话。直接发送消息即可开始。".to_string()
    } else {
        "Assistant mode is on; a new assistant session was created. Send a message to start."
            .to_string()
    }
}

/// Mark chat paired, run assistant/session bootstrap, return first user-visible message + main menu actions.
pub async fn complete_im_bot_pairing(state: &mut BotChatState) -> HandleResult {
    state.paired = true;
    let language = current_bot_language().await;
    let note = bootstrap_im_chat_after_pairing(state).await;
    let reply = format!("{}\n\n{}", paired_success_message(language), note);
    let actions = main_menu_actions(language, state.display_mode);
    state.last_menu_commands = actions.iter().map(|a| a.command.clone()).collect();
    HandleResult {
        reply,
        actions,
        forward_to_session: None,
    }
}

fn label_switch_workspace(language: BotLanguage) -> &'static str {
    if language.is_chinese() {
        "切换工作区"
    } else {
        "Switch Workspace"
    }
}

fn label_resume_session(language: BotLanguage) -> &'static str {
    if language.is_chinese() {
        "恢复会话"
    } else {
        "Resume Session"
    }
}

fn label_new_code_session(language: BotLanguage) -> &'static str {
    if language.is_chinese() {
        "新建编码会话"
    } else {
        "New Code Session"
    }
}

fn label_new_cowork_session(language: BotLanguage) -> &'static str {
    if language.is_chinese() {
        "新建协作会话"
    } else {
        "New Cowork Session"
    }
}

fn label_new_claw_session(language: BotLanguage) -> &'static str {
    if language.is_chinese() {
        "新建助理会话"
    } else {
        "New Claw Session"
    }
}

fn label_switch_assistant(language: BotLanguage) -> &'static str {
    if language.is_chinese() {
        "切换助理"
    } else {
        "Switch Assistant"
    }
}

fn label_help(language: BotLanguage) -> &'static str {
    if language.is_chinese() {
        "帮助"
    } else {
        "Help"
    }
}

fn label_cancel_task(language: BotLanguage) -> &'static str {
    if language.is_chinese() {
        "取消任务"
    } else {
        "Cancel Task"
    }
}

fn label_next_page(language: BotLanguage) -> &'static str {
    if language.is_chinese() {
        "下一页"
    } else {
        "Next Page"
    }
}

fn label_switch_pro_mode(language: BotLanguage) -> &'static str {
    if language.is_chinese() {
        "专业模式"
    } else {
        "Expert Mode"
    }
}

fn label_switch_assistant_mode(language: BotLanguage) -> &'static str {
    if language.is_chinese() {
        "助理模式"
    } else {
        "Assistant Mode"
    }
}

fn other_label(language: BotLanguage) -> &'static str {
    if language.is_chinese() {
        "其他"
    } else {
        "Other"
    }
}

pub fn main_menu_actions(language: BotLanguage, display_mode: BotDisplayMode) -> Vec<BotAction> {
    let is_pro = display_mode == BotDisplayMode::Pro;

    if is_pro {
        // Pro mode: show workspace switch
        vec![
            BotAction::primary(label_switch_workspace(language), "/switch_workspace"),
            BotAction::secondary(label_resume_session(language), "/resume_session"),
            BotAction::secondary(label_switch_assistant_mode(language), "/assistant"),
            BotAction::secondary(label_new_code_session(language), "/new_code_session"),
            BotAction::secondary(label_new_cowork_session(language), "/new_cowork_session"),
            BotAction::secondary(label_help(language), "/help"),
        ]
    } else {
        // Assistant mode: show assistant switch (not workspace)
        vec![
            BotAction::primary(label_switch_assistant(language), "/switch_assistant"),
            BotAction::secondary(label_resume_session(language), "/resume_session"),
            BotAction::secondary(label_switch_pro_mode(language), "/pro"),
            BotAction::secondary(label_new_claw_session(language), "/new_claw_session"),
            BotAction::secondary(label_help(language), "/help"),
        ]
    }
}

fn pro_mode_actions(language: BotLanguage) -> Vec<BotAction> {
    vec![
        BotAction::primary(label_new_code_session(language), "/new_code_session"),
        BotAction::secondary(label_new_cowork_session(language), "/new_cowork_session"),
        BotAction::secondary(label_switch_workspace(language), "/switch_workspace"),
        BotAction::secondary(label_switch_assistant_mode(language), "/assistant"),
        BotAction::secondary(label_help(language), "/help"),
    ]
}

fn assistant_mode_actions(language: BotLanguage) -> Vec<BotAction> {
    vec![
        BotAction::primary(label_new_claw_session(language), "/new_claw_session"),
        BotAction::secondary(label_switch_assistant(language), "/switch_assistant"),
        BotAction::secondary(label_switch_pro_mode(language), "/pro"),
        BotAction::secondary(label_help(language), "/help"),
    ]
}

fn workspace_required_actions(language: BotLanguage) -> Vec<BotAction> {
    vec![BotAction::primary(
        label_switch_workspace(language),
        "/switch_workspace",
    )]
}

fn assistant_required_actions(language: BotLanguage) -> Vec<BotAction> {
    vec![BotAction::primary(
        label_switch_assistant(language),
        "/switch_assistant",
    )]
}

fn session_entry_actions(language: BotLanguage, display_mode: BotDisplayMode) -> Vec<BotAction> {
    let is_pro = display_mode == BotDisplayMode::Pro;
    if is_pro {
        vec![
            BotAction::primary(label_resume_session(language), "/resume_session"),
            BotAction::secondary(label_new_code_session(language), "/new_code_session"),
            BotAction::secondary(label_new_cowork_session(language), "/new_cowork_session"),
            BotAction::secondary(label_switch_workspace(language), "/switch_workspace"),
            BotAction::secondary(label_switch_assistant_mode(language), "/assistant"),
            BotAction::secondary(label_help(language), "/help"),
        ]
    } else {
        vec![
            BotAction::primary(label_resume_session(language), "/resume_session"),
            BotAction::secondary(label_new_claw_session(language), "/new_claw_session"),
            BotAction::secondary(label_switch_assistant(language), "/switch_assistant"),
            BotAction::secondary(label_switch_pro_mode(language), "/pro"),
            BotAction::secondary(label_help(language), "/help"),
        ]
    }
}

fn new_session_actions(language: BotLanguage, display_mode: BotDisplayMode) -> Vec<BotAction> {
    let is_pro = display_mode == BotDisplayMode::Pro;
    if is_pro {
        vec![
            BotAction::primary(label_new_code_session(language), "/new_code_session"),
            BotAction::secondary(label_new_cowork_session(language), "/new_cowork_session"),
            BotAction::secondary(label_switch_workspace(language), "/switch_workspace"),
            BotAction::secondary(label_switch_assistant_mode(language), "/assistant"),
            BotAction::secondary(label_help(language), "/help"),
        ]
    } else {
        vec![
            BotAction::primary(label_new_claw_session(language), "/new_claw_session"),
            BotAction::secondary(label_switch_assistant(language), "/switch_assistant"),
            BotAction::secondary(label_switch_pro_mode(language), "/pro"),
            BotAction::secondary(label_help(language), "/help"),
        ]
    }
}

fn cancel_task_actions(language: BotLanguage, command: impl Into<String>) -> Vec<BotAction> {
    vec![BotAction::secondary(
        label_cancel_task(language),
        command.into(),
    )]
}

// ── Main dispatch ───────────────────────────────────────────────────

async fn dispatch_im_bot_command(
    state: &mut BotChatState,
    cmd: BotCommand,
    image_contexts: Vec<crate::agentic::image_analysis::ImageContextData>,
) -> HandleResult {
    let r = dispatch_im_bot_command_inner(state, cmd, image_contexts).await;
    if !r.actions.is_empty() {
        state.last_menu_commands = r.actions.iter().map(|a| a.command.clone()).collect();
    }
    r
}

async fn dispatch_im_bot_command_inner(
    state: &mut BotChatState,
    cmd: BotCommand,
    image_contexts: Vec<crate::agentic::image_analysis::ImageContextData>,
) -> HandleResult {
    let language = current_bot_language().await;
    match cmd {
        BotCommand::Start | BotCommand::Help => {
            if state.paired {
                HandleResult {
                    reply: help_message(language).to_string(),
                    actions: main_menu_actions(language, state.display_mode),
                    forward_to_session: None,
                }
            } else {
                HandleResult {
                    reply: welcome_message(language).to_string(),
                    actions: vec![],
                    forward_to_session: None,
                }
            }
        }
        BotCommand::SwitchMode(new_mode) => {
            if !state.paired {
                not_paired(language)
            } else {
                state.display_mode = new_mode;
                let mode_name = if new_mode == BotDisplayMode::Pro {
                    if language.is_chinese() {
                        "专业模式"
                    } else {
                        "Expert Mode"
                    }
                } else {
                    if language.is_chinese() {
                        "助理模式"
                    } else {
                        "Assistant Mode"
                    }
                };
                let desc = if new_mode == BotDisplayMode::Pro {
                    if language.is_chinese() {
                        "适合目标明确、一次完成的即时任务。"
                    } else {
                        "Best for focused, one-shot tasks with a clear goal."
                    }
                } else {
                    if language.is_chinese() {
                        "适合持续推进、需要延续上下文和个人偏好的任务。"
                    } else {
                        "Best for ongoing work with context and personal preferences."
                    }
                };
                HandleResult {
                    reply: if language.is_chinese() {
                        format!("已切换到 {}\n\n{}\n\n你现在可以：", mode_name, desc)
                    } else {
                        format!("Switched to {}\n\n{}\n\nYou can now:", mode_name, desc)
                    },
                    actions: if new_mode == BotDisplayMode::Pro {
                        pro_mode_actions(language)
                    } else {
                        assistant_mode_actions(language)
                    },
                    forward_to_session: None,
                }
            }
        }
        BotCommand::PairingCode(_) => HandleResult {
            reply: if language.is_chinese() {
                "配对码会自动处理。如果你需要重新配对，请在 BitFun Desktop 中重新启动连接。"
                    .to_string()
            } else {
                "Pairing codes are handled automatically. If you need to re-pair, please restart the connection from BitFun Desktop."
                    .to_string()
            },
            actions: vec![],
            forward_to_session: None,
        },
        BotCommand::SwitchWorkspace => {
            if !state.paired {
                return not_paired(language);
            }
            handle_switch_workspace(state).await
        }
        BotCommand::SwitchAssistant => {
            if !state.paired {
                return not_paired(language);
            }
            handle_switch_assistant(state).await
        }
        BotCommand::ResumeSession => {
            if !state.paired {
                return not_paired(language);
            }
            if state.display_mode == BotDisplayMode::Pro {
                if state.current_workspace.is_none() {
                    return need_workspace(language);
                }
            } else {
                if state.current_assistant.is_none() {
                    return need_assistant(language);
                }
            }
            handle_resume_session(state, 0).await
        }
        BotCommand::NewCodeSession => {
            if !state.paired {
                return not_paired(language);
            }
            // Code session only available in Pro mode
            if state.display_mode != BotDisplayMode::Pro {
                return wrong_mode_for_pro(language);
            }
            if state.current_workspace.is_none() {
                return need_workspace(language);
            }
            handle_new_session(state, "agentic").await
        }
        BotCommand::NewCoworkSession => {
            if !state.paired {
                return not_paired(language);
            }
            // Cowork session only available in Pro mode
            if state.display_mode != BotDisplayMode::Pro {
                return wrong_mode_for_pro(language);
            }
            if state.current_workspace.is_none() {
                return need_workspace(language);
            }
            handle_new_session(state, "Cowork").await
        }
        BotCommand::NewClawSession => {
            if !state.paired {
                return not_paired(language);
            }
            // Claw session only available in Assistant mode
            if state.display_mode != BotDisplayMode::Assistant {
                return wrong_mode_for_assistant(language);
            }
            // Claw sessions don't need workspace
            handle_new_session(state, "Claw").await
        }
        BotCommand::CancelTask(turn_id) => {
            if !state.paired {
                return not_paired(language);
            }
            handle_cancel_task(state, turn_id.as_deref()).await
        }
        BotCommand::NumberSelection(n) => {
            if !state.paired {
                return not_paired(language);
            }
            handle_number_selection(state, n).await
        }
        BotCommand::NextPage => {
            if !state.paired {
                return not_paired(language);
            }
            handle_next_page(state).await
        }
        BotCommand::ChatMessage(msg) => {
            if !state.paired {
                return not_paired(language);
            }
            handle_chat_message(state, &msg, image_contexts).await
        }
    }
}

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
    dispatch_im_bot_command(state, cmd, image_contexts).await
}

// ── Helpers ─────────────────────────────────────────────────────────

fn not_paired(language: BotLanguage) -> HandleResult {
    HandleResult {
        reply: if language.is_chinese() {
            "尚未连接到 BitFun Desktop。请先发送 6 位配对码。".to_string()
        } else {
            "Not connected to BitFun Desktop. Please enter the 6-digit pairing code first."
                .to_string()
        },
        actions: vec![],
        forward_to_session: None,
    }
}

fn need_workspace(language: BotLanguage) -> HandleResult {
    HandleResult {
        reply: if language.is_chinese() {
            "尚未选择工作区。请先使用 /switch_workspace。".to_string()
        } else {
            "No workspace selected. Use /switch_workspace first.".to_string()
        },
        actions: workspace_required_actions(language),
        forward_to_session: None,
    }
}

fn need_assistant(language: BotLanguage) -> HandleResult {
    HandleResult {
        reply: if language.is_chinese() {
            "尚未选择助理。请先使用 /switch_assistant。".to_string()
        } else {
            "No assistant selected. Use /switch_assistant first.".to_string()
        },
        actions: assistant_required_actions(language),
        forward_to_session: None,
    }
}

fn wrong_mode_for_pro(language: BotLanguage) -> HandleResult {
    HandleResult {
        reply: if language.is_chinese() {
            "该会话只能在专业模式下创建。请先发送 /pro 切换到专业模式。".to_string()
        } else {
            "This session can only be created in Expert mode. Please send /pro to switch to Expert mode.".to_string()
        },
        actions: pro_mode_actions(language),
        forward_to_session: None,
    }
}

fn wrong_mode_for_assistant(language: BotLanguage) -> HandleResult {
    HandleResult {
        reply: if language.is_chinese() {
            "该会话只能在助理模式下创建。请先发送 /assistant 切换到助理模式。".to_string()
        } else {
            "This session can only be created in Assistant mode. Please send /assistant to switch to Assistant mode.".to_string()
        },
        actions: assistant_mode_actions(language),
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
            BotAction::secondary(truncate_action_label(label, 28), (idx + 1).to_string())
        })
        .collect()
}

fn build_question_prompt(
    language: BotLanguage,
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
        "{} {}/{}\n",
        if language.is_chinese() {
            "问题"
        } else {
            "Question"
        },
        current_index + 1,
        questions.len(),
    );
    if !question.header.is_empty() {
        reply.push_str(&format!("{}\n", question.header));
    }
    reply.push_str(&format!("{}\n\n", question.question));
    for (idx, option) in question.options.iter().enumerate() {
        reply.push_str(&format!("{}\n", question_option_line(idx, option)));
    }
    reply.push_str(&format!(
        "{}. {}\n\n",
        question.options.len() + 1,
        other_label(language),
    ));
    if awaiting_custom_text {
        reply.push_str(if language.is_chinese() {
            "请输入你的自定义答案。"
        } else {
            "Please type your custom answer."
        });
    } else if question.multi_select {
        reply.push_str(if language.is_chinese() {
            "请回复一个或多个选项编号，用逗号分隔，例如：1,3"
        } else {
            "Reply with one or more option numbers, separated by commas. Example: 1,3"
        });
    } else {
        reply.push_str(if language.is_chinese() {
            "请回复单个选项编号。"
        } else {
            "Reply with a single option number."
        });
        let mut labels: Vec<String> = question
            .options
            .iter()
            .map(|option| option.label.clone())
            .collect();
        labels.push(other_label(language).to_string());
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
    use crate::service::workspace::get_global_workspace_service;
    let language = current_bot_language().await;

    let ws_service = match get_global_workspace_service() {
        Some(s) => s,
        None => {
            return HandleResult {
                reply: if language.is_chinese() {
                    "工作区服务不可用。".to_string()
                } else {
                    "Workspace service not available.".to_string()
                },
                actions: vec![],
                forward_to_session: None,
            };
        }
    };

    let workspaces = ws_service.get_recent_workspaces().await;
    if workspaces.is_empty() {
        return HandleResult {
            reply: if language.is_chinese() {
                "未找到工作区。请先在 BitFun Desktop 中打开一个项目。".to_string()
            } else {
                "No workspaces found. Please open a project in BitFun Desktop first.".to_string()
            },
            actions: vec![],
            forward_to_session: None,
        };
    }

    let effective_current: Option<&str> = state.current_workspace.as_deref();

    let mut text = if language.is_chinese() {
        String::from("请选择工作区：\n\n")
    } else {
        String::from("Select a workspace:\n\n")
    };
    let mut options: Vec<(String, String)> = Vec::new();
    for (i, ws) in workspaces.iter().enumerate() {
        let path = ws.root_path.to_string_lossy().to_string();
        let is_current = effective_current == Some(path.as_str());
        let marker = if is_current {
            if language.is_chinese() {
                " [当前]"
            } else {
                " [current]"
            }
        } else {
            ""
        };
        text.push_str(&format!("{}. {}{}\n   {}\n", i + 1, ws.name, marker, path));
        options.push((path, ws.name.clone()));
    }
    text.push_str(if language.is_chinese() {
        "\n请回复工作区编号。"
    } else {
        "\nReply with the workspace number."
    });

    let action_labels: Vec<String> = options.iter().map(|(_, name)| name.clone()).collect();
    state.pending_action = Some(PendingAction::SelectWorkspace { options });
    HandleResult {
        reply: text,
        actions: numbered_actions(&action_labels),
        forward_to_session: None,
    }
}

async fn handle_switch_assistant(state: &mut BotChatState) -> HandleResult {
    use crate::service::workspace::get_global_workspace_service;
    let language = current_bot_language().await;

    let ws_service = match get_global_workspace_service() {
        Some(s) => s,
        None => {
            return HandleResult {
                reply: if language.is_chinese() {
                    "工作区服务不可用。".to_string()
                } else {
                    "Workspace service not available.".to_string()
                },
                actions: vec![],
                forward_to_session: None,
            };
        }
    };

    let assistants = ws_service.get_assistant_workspaces().await;
    if assistants.is_empty() {
        return HandleResult {
            reply: if language.is_chinese() {
                "未找到助理。请先在 BitFun Desktop 中创建助理。".to_string()
            } else {
                "No assistants found. Please create an assistant in BitFun Desktop first."
                    .to_string()
            },
            actions: assistant_mode_actions(language),
            forward_to_session: None,
        };
    }

    let effective_current: Option<&str> = state.current_assistant.as_deref();

    let mut text = if language.is_chinese() {
        String::from("请选择助理：\n\n")
    } else {
        String::from("Select an assistant:\n\n")
    };
    let mut options: Vec<(String, String)> = Vec::new();
    for (i, ws) in assistants.iter().enumerate() {
        let path = ws.root_path.to_string_lossy().to_string();
        let is_current = effective_current == Some(path.as_str());
        let marker = if is_current {
            if language.is_chinese() {
                " [当前]"
            } else {
                " [current]"
            }
        } else {
            ""
        };
        text.push_str(&format!("{}. {}{}\n", i + 1, ws.name, marker));
        options.push((path, ws.name.clone()));
    }
    text.push_str(if language.is_chinese() {
        "\n请回复助理编号。"
    } else {
        "\nReply with the assistant number."
    });

    let action_labels: Vec<String> = options.iter().map(|(_, name)| name.clone()).collect();
    state.pending_action = Some(PendingAction::SelectAssistant { options });
    HandleResult {
        reply: text,
        actions: numbered_actions(&action_labels),
        forward_to_session: None,
    }
}

async fn handle_resume_session(state: &mut BotChatState, page: usize) -> HandleResult {
    use crate::agentic::persistence::PersistenceManager;
    use crate::infrastructure::PathManager;
    let language = current_bot_language().await;

    let ws_path = if state.display_mode == BotDisplayMode::Pro {
        match &state.current_workspace {
            Some(p) => std::path::PathBuf::from(p),
            None => return need_workspace(language),
        }
    } else {
        match &state.current_assistant {
            Some(p) => std::path::PathBuf::from(p),
            None => return need_assistant(language),
        }
    };

    let page_size = 10usize;
    let offset = page * page_size;

    let pm = match PathManager::new() {
        Ok(pm) => std::sync::Arc::new(pm),
        Err(e) => {
            return HandleResult {
                reply: if language.is_chinese() {
                    format!("加载会话失败：{e}")
                } else {
                    format!("Failed to load sessions: {e}")
                },
                actions: vec![],
                forward_to_session: None,
            };
        }
    };

    let store = match PersistenceManager::new(pm) {
        Ok(store) => store,
        Err(e) => {
            return HandleResult {
                reply: if language.is_chinese() {
                    format!("加载会话失败：{e}")
                } else {
                    format!("Failed to load sessions: {e}")
                },
                actions: vec![],
                forward_to_session: None,
            };
        }
    };

    let all_meta = match store.list_session_metadata(&ws_path).await {
        Ok(m) => m,
        Err(e) => {
            return HandleResult {
                reply: if language.is_chinese() {
                    format!("列出会话失败：{e}")
                } else {
                    format!("Failed to list sessions: {e}")
                },
                actions: vec![],
                forward_to_session: None,
            };
        }
    };

    if all_meta.is_empty() {
        let reply = if language.is_chinese() {
            if state.display_mode == BotDisplayMode::Pro {
                "当前工作区没有会话。请使用 /new_code_session 或 /new_cowork_session 创建一个。"
                    .to_string()
            } else {
                "当前工作区没有会话。请使用 /new_claw_session 创建一个。".to_string()
            }
        } else {
            if state.display_mode == BotDisplayMode::Pro {
                "No sessions found in this workspace. Use /new_code_session or /new_cowork_session to create one.".to_string()
            } else {
                "No sessions found in this workspace. Use /new_claw_session to create one."
                    .to_string()
            }
        };
        return HandleResult {
            reply,
            actions: new_session_actions(language, state.display_mode),
            forward_to_session: None,
        };
    }

    let total = all_meta.len();
    let has_more = offset + page_size < total;
    let sessions: Vec<_> = all_meta.into_iter().skip(offset).take(page_size).collect();

    let ws_name = ws_path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| {
            if language.is_chinese() {
                "未知".to_string()
            } else {
                "Unknown".to_string()
            }
        });

    let mut text = if language.is_chinese() {
        format!("{} 中的会话（第 {} 页）：\n\n", ws_name, page + 1)
    } else {
        format!("Sessions in {} (page {}):\n\n", ws_name, page + 1)
    };
    let mut options: Vec<(String, String)> = Vec::new();
    for (i, s) in sessions.iter().enumerate() {
        let is_current = state.current_session_id.as_deref() == Some(&s.session_id);
        let marker = if is_current {
            if language.is_chinese() {
                " [当前]"
            } else {
                " [current]"
            }
        } else {
            ""
        };
        let ts = chrono::DateTime::from_timestamp(s.last_active_at as i64 / 1000, 0)
            .map(|dt| dt.format("%m-%d %H:%M").to_string())
            .unwrap_or_default();
        let turn_count = s.turn_count;
        let msg_hint = if turn_count == 0 {
            if language.is_chinese() {
                "无消息".to_string()
            } else {
                "no messages".to_string()
            }
        } else if turn_count == 1 {
            if language.is_chinese() {
                "1 条消息".to_string()
            } else {
                "1 message".to_string()
            }
        } else {
            if language.is_chinese() {
                format!("{turn_count} 条消息")
            } else {
                format!("{turn_count} messages")
            }
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
        text.push_str(if language.is_chinese() {
            "\n0 - 下一页\n"
        } else {
            "\n0 - Next page\n"
        });
    }
    text.push_str(if language.is_chinese() {
        "\n请回复会话编号。"
    } else {
        "\nReply with the session number."
    });

    state.pending_action = Some(PendingAction::SelectSession {
        options,
        page,
        has_more,
    });
    let mut action_labels: Vec<String> = sessions
        .iter()
        .map(|session| format!("[{}] {}", session.agent_type, session.session_name))
        .collect();
    let mut actions = numbered_actions(&action_labels);
    if has_more {
        action_labels.push(label_next_page(language).to_string());
        actions.push(BotAction::secondary(label_next_page(language), "0"));
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
    use crate::service::workspace::get_global_workspace_service;

    let language = current_bot_language().await;
    let is_claw = agent_type == "Claw";

    let coordinator = match get_global_coordinator() {
        Some(c) => c,
        None => {
            return HandleResult {
                reply: if language.is_chinese() {
                    "BitFun 会话系统尚未就绪。".to_string()
                } else {
                    "BitFun session system not ready.".to_string()
                },
                actions: vec![],
                forward_to_session: None,
            };
        }
    };

    let ws_path = if is_claw {
        // For Claw sessions, prefer current_assistant, or get/create default
        if let Some(ref assistant_path) = state.current_assistant {
            Some(assistant_path.clone())
        } else {
            let ws_service = match get_global_workspace_service() {
                Some(s) => s,
                None => {
                    return HandleResult {
                        reply: if language.is_chinese() {
                            "工作区服务不可用。".to_string()
                        } else {
                            "Workspace service not available.".to_string()
                        },
                        actions: vec![],
                        forward_to_session: None,
                    };
                }
            };

            // Get or create default assistant workspace
            let workspaces = ws_service.get_assistant_workspaces().await;
            let resolved = if let Some(default_ws) =
                workspaces.into_iter().find(|w| w.assistant_id.is_none())
            {
                Some(default_ws.root_path.to_string_lossy().to_string())
            } else {
                match ws_service.create_assistant_workspace(None).await {
                    Ok(ws_info) => Some(ws_info.root_path.to_string_lossy().to_string()),
                    Err(e) => {
                        return HandleResult {
                            reply: if language.is_chinese() {
                                format!("创建助理工作区失败：{}", e)
                            } else {
                                format!("Failed to create assistant workspace: {}", e)
                            },
                            actions: vec![],
                            forward_to_session: None,
                        };
                    }
                }
            };
            if let Some(ref path) = resolved {
                state.current_assistant = Some(path.clone());
            }
            resolved
        }
    } else {
        // For Code/Cowork sessions, use current workspace
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
        return if is_claw {
            need_assistant(language)
        } else {
            need_workspace(language)
        };
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
            let session_id = session.session_id.clone();
            state.current_session_id = Some(session_id.clone());
            let label = match agent_type {
                "Cowork" => {
                    if language.is_chinese() {
                        "协作"
                    } else {
                        "cowork"
                    }
                }
                "Claw" => {
                    if language.is_chinese() {
                        "助理"
                    } else {
                        "claw"
                    }
                }
                _ => {
                    if language.is_chinese() {
                        "编码"
                    } else {
                        "coding"
                    }
                }
            };
            let workspace_display = workspace_path.clone();
            HandleResult {
                reply: if language.is_chinese() {
                    format!(
                        "已创建新的{}会话：{}\n工作区：{}\n\n你现在可以发送消息与 AI 助手交互。",
                        label, session_name, workspace_display
                    )
                } else {
                    format!(
                        "Created new {} session: {}\nWorkspace: {}\n\nYou can now send messages to interact with the AI agent.",
                        label, session_name, workspace_display
                    )
                },
                actions: vec![],
                forward_to_session: None,
            }
        }
        Err(e) => HandleResult {
            reply: if language.is_chinese() {
                format!("创建会话失败：{e}")
            } else {
                format!("Failed to create session: {e}")
            },
            actions: vec![],
            forward_to_session: None,
        },
    }
}

async fn handle_number_selection(state: &mut BotChatState, n: usize) -> HandleResult {
    let language = current_bot_language().await;
    let pending = state.pending_action.take();
    match pending {
        Some(PendingAction::SelectWorkspace { options }) => {
            if n < 1 || n > options.len() {
                state.pending_action = Some(PendingAction::SelectWorkspace { options });
                return HandleResult {
                    reply: if language.is_chinese() {
                        format!(
                            "无效选择。请输入 1-{}。",
                            state
                                .pending_action
                                .as_ref()
                                .map(|a| match a {
                                    PendingAction::SelectWorkspace { options } => options.len(),
                                    _ => 0,
                                })
                                .unwrap_or(0)
                        )
                    } else {
                        format!(
                            "Invalid selection. Please enter 1-{}.",
                            state
                                .pending_action
                                .as_ref()
                                .map(|a| match a {
                                    PendingAction::SelectWorkspace { options } => options.len(),
                                    _ => 0,
                                })
                                .unwrap_or(0)
                        )
                    },
                    actions: vec![],
                    forward_to_session: None,
                };
            }
            let (path, name) = options[n - 1].clone();
            select_workspace(state, &path, &name).await
        }
        Some(PendingAction::SelectAssistant { options }) => {
            if n < 1 || n > options.len() {
                state.pending_action = Some(PendingAction::SelectAssistant { options });
                return HandleResult {
                    reply: if language.is_chinese() {
                        format!(
                            "无效选择。请输入 1-{}。",
                            state
                                .pending_action
                                .as_ref()
                                .map(|a| match a {
                                    PendingAction::SelectAssistant { options } => options.len(),
                                    _ => 0,
                                })
                                .unwrap_or(0)
                        )
                    } else {
                        format!(
                            "Invalid selection. Please enter 1-{}.",
                            state
                                .pending_action
                                .as_ref()
                                .map(|a| match a {
                                    PendingAction::SelectAssistant { options } => options.len(),
                                    _ => 0,
                                })
                                .unwrap_or(0)
                        )
                    },
                    actions: vec![],
                    forward_to_session: None,
                };
            }
            let (path, name) = options[n - 1].clone();
            select_assistant(state, &path, &name).await
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
                    reply: if language.is_chinese() {
                        format!("无效选择。请输入 1-{max}。")
                    } else {
                        format!("Invalid selection. Please enter 1-{max}.")
                    },
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
        None => {
            if n >= 1 && n <= state.last_menu_commands.len() {
                let cmd_str = state.last_menu_commands[n - 1].clone();
                let next_cmd = parse_command(&cmd_str);
                Box::pin(dispatch_im_bot_command(state, next_cmd, vec![])).await
            } else {
                handle_chat_message(state, &n.to_string(), vec![]).await
            }
        }
    }
}

async fn select_workspace(state: &mut BotChatState, path: &str, name: &str) -> HandleResult {
    use crate::service::workspace::get_global_workspace_service;
    let language = current_bot_language().await;

    let ws_service = match get_global_workspace_service() {
        Some(s) => s,
        None => {
            return HandleResult {
                reply: if language.is_chinese() {
                    "工作区服务不可用。".to_string()
                } else {
                    "Workspace service not available.".to_string()
                },
                actions: vec![],
                forward_to_session: None,
            };
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
            let reply =
                build_workspace_switched_reply(language, name, session_count, state.display_mode);
            let actions = if session_count > 0 {
                session_entry_actions(language, state.display_mode)
            } else {
                new_session_actions(language, state.display_mode)
            };
            HandleResult {
                reply,
                actions,
                forward_to_session: None,
            }
        }
        Err(e) => HandleResult {
            reply: if language.is_chinese() {
                format!("切换工作区失败：{e}")
            } else {
                format!("Failed to switch workspace: {e}")
            },
            actions: vec![],
            forward_to_session: None,
        },
    }
}

async fn select_assistant(state: &mut BotChatState, path: &str, name: &str) -> HandleResult {
    use crate::service::workspace::get_global_workspace_service;
    let language = current_bot_language().await;

    let ws_service = match get_global_workspace_service() {
        Some(s) => s,
        None => {
            return HandleResult {
                reply: if language.is_chinese() {
                    "工作区服务不可用。".to_string()
                } else {
                    "Workspace service not available.".to_string()
                },
                actions: vec![],
                forward_to_session: None,
            };
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
            state.current_session_id = None;
            info!("Bot switched assistant to: {path}");

            let session_count = count_workspace_sessions(path).await;
            let reply = if language.is_chinese() {
                format!("已切换到助理：{}\n\n会话数：{}", name, session_count)
            } else {
                format!(
                    "Switched to assistant: {}\n\nSessions: {}",
                    name, session_count
                )
            };
            let actions = if session_count > 0 {
                session_entry_actions(language, state.display_mode)
            } else {
                new_session_actions(language, state.display_mode)
            };
            HandleResult {
                reply,
                actions,
                forward_to_session: None,
            }
        }
        Err(e) => HandleResult {
            reply: if language.is_chinese() {
                format!("切换助理失败：{e}")
            } else {
                format!("Failed to switch assistant: {e}")
            },
            actions: vec![],
            forward_to_session: None,
        },
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

fn build_workspace_switched_reply(
    language: BotLanguage,
    name: &str,
    session_count: usize,
    display_mode: BotDisplayMode,
) -> String {
    let is_pro = display_mode == BotDisplayMode::Pro;
    let mode_label = if is_pro {
        if language.is_chinese() {
            "专业模式"
        } else {
            "Expert Mode"
        }
    } else {
        if language.is_chinese() {
            "助理模式"
        } else {
            "Assistant Mode"
        }
    };

    let mut reply = if language.is_chinese() {
        format!("已切换到工作区：{}\n当前模式：{}\n\n", name, mode_label)
    } else {
        format!(
            "Switched to workspace: {}\nCurrent mode: {}\n\n",
            name, mode_label
        )
    };

    if session_count > 0 {
        if language.is_chinese() {
            reply.push_str(&format!(
                "这个工作区已有 {session_count} 个会话。你想做什么？\n\n"
            ));
        } else {
            let s = if session_count == 1 { "" } else { "s" };
            reply.push_str(&format!(
                "This workspace has {session_count} existing session{s}. What would you like to do?\n\n"
            ));
        }
    } else {
        if language.is_chinese() {
            reply.push_str("这个工作区还没有会话。你想做什么？\n\n");
        } else {
            reply.push_str("No sessions found in this workspace. What would you like to do?\n\n");
        }
    }

    if is_pro {
        if language.is_chinese() {
            reply.push_str(
                "/resume_session - 恢复已有会话\n\
                 /new_code_session - 开始新的编码会话\n\
                 /new_cowork_session - 开始新的协作会话\n\
                 /assistant - 切换到助理模式",
            );
        } else {
            reply.push_str(
                "/resume_session - Resume an existing session\n\
                 /new_code_session - Start a new coding session\n\
                 /new_cowork_session - Start a new cowork session\n\
                 /assistant - Switch to Assistant mode",
            );
        }
    } else {
        if language.is_chinese() {
            reply.push_str(
                "/resume_session - 恢复已有会话\n\
                 /new_claw_session - 开始新的助理会话\n\
                 /pro - 切换到专业模式",
            );
        } else {
            reply.push_str(
                "/resume_session - Resume an existing session\n\
                 /new_claw_session - Start a new claw session\n\
                 /pro - Switch to Expert mode",
            );
        }
    }
    reply
}

async fn select_session(
    state: &mut BotChatState,
    session_id: &str,
    session_name: &str,
) -> HandleResult {
    let language = current_bot_language().await;
    state.current_session_id = Some(session_id.to_string());
    info!("Bot resumed session: {session_id}");

    let last_pair =
        load_last_dialog_pair_from_turns(state.current_workspace.as_deref(), session_id).await;

    let mut reply = if language.is_chinese() {
        format!("已恢复会话：{session_name}\n\n")
    } else {
        format!("Resumed session: {session_name}\n\n")
    };
    if let Some((user_text, assistant_text)) = last_pair {
        reply.push_str(if language.is_chinese() {
            "— 最近一次对话 —\n"
        } else {
            "— Last conversation —\n"
        });
        reply.push_str(&format!(
            "{}: {user_text}\n\n",
            if language.is_chinese() { "你" } else { "You" }
        ));
        reply.push_str(&format!("{}: {assistant_text}\n\n", "AI"));
        reply.push_str(if language.is_chinese() {
            "你可以继续对话。"
        } else {
            "You can continue the conversation."
        });
    } else {
        reply.push_str(if language.is_chinese() {
            "你现在可以发送消息与 AI 助手交互。"
        } else {
            "You can now send messages to interact with the AI agent."
        });
    }

    HandleResult {
        reply,
        actions: vec![],
        forward_to_session: None,
    }
}

/// Load the last user/assistant dialog pair from the unified project session store,
/// the same data source the desktop frontend uses.
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

/// Strip prompt markup injected before storing the message.
fn strip_user_message_tags(raw: &str) -> String {
    crate::agentic::core::strip_prompt_markup(raw)
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
    let language = current_bot_language().await;

    let session_id = match state.current_session_id.clone() {
        Some(id) => id,
        None => {
            return HandleResult {
                reply: if language.is_chinese() {
                    "当前没有可取消的活动会话。".to_string()
                } else {
                    "No active session to cancel.".to_string()
                },
                actions: session_entry_actions(language, state.display_mode),
                forward_to_session: None,
            };
        }
    };

    let dispatcher = get_or_init_global_dispatcher();
    match dispatcher.cancel_task(&session_id, requested_turn_id).await {
        Ok(_) => {
            state.pending_action = None;
            HandleResult {
                reply: if language.is_chinese() {
                    "已请求取消当前任务。".to_string()
                } else {
                    "Cancellation requested for the current task.".to_string()
                },
                actions: vec![],
                forward_to_session: None,
            }
        }
        Err(e) => HandleResult {
            reply: if language.is_chinese() {
                format!("取消任务失败：{e}")
            } else {
                format!("Failed to cancel task: {e}")
            },
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
) -> HandleResult {
    let language = current_bot_language().await;
    let Some(question) = questions.get(current_index).cloned() else {
        return HandleResult {
            reply: if language.is_chinese() {
                "问题状态无效。".to_string()
            } else {
                "Question state is invalid.".to_string()
            },
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
                reply: if language.is_chinese() {
                    "自定义答案不能为空。请输入你的自定义答案。".to_string()
                } else {
                    "Custom answer cannot be empty. Please type your custom answer.".to_string()
                },
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
                        if language.is_chinese() {
                            "输入无效。请回复选项编号，例如 `1,3`。".to_string()
                        } else {
                            "Invalid input. Reply with option numbers like `1,3`.".to_string()
                        }
                    } else {
                        if language.is_chinese() {
                            "输入无效。请回复单个选项编号。".to_string()
                        } else {
                            "Invalid input. Reply with a single option number.".to_string()
                        }
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
                reply: if language.is_chinese() {
                    "请回复单个选项编号。".to_string()
                } else {
                    "Please reply with a single option number.".to_string()
                },
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
                labels.push(Value::String(other_label(language).to_string()));
            } else if selection >= 1 && selection <= question.options.len() {
                labels.push(Value::String(question.options[selection - 1].label.clone()));
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
                        "{} 1 {} {}。",
                        if language.is_chinese() {
                            "无效选择。请选择"
                        } else {
                            "Invalid selection. Please choose between"
                        },
                        if language.is_chinese() { "到" } else { "and" },
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
                reply: if language.is_chinese() {
                    format!("请为“{}”输入你的自定义答案。", other_label(language))
                } else {
                    "Please type your custom answer for `Other`.".to_string()
                },
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
            language,
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

    let mut result = submit_question_answers(&tool_id, &answers).await;
    if language.is_chinese()
        && result.reply == "Answers submitted. Waiting for the assistant to continue..."
    {
        result.reply = "答案已提交，等待助手继续...".to_string();
    }
    result
}

async fn handle_next_page(state: &mut BotChatState) -> HandleResult {
    let language = current_bot_language().await;
    let pending = state.pending_action.take();
    match pending {
        Some(PendingAction::SelectSession { page, has_more, .. }) if has_more => {
            handle_resume_session(state, page + 1).await
        }
        Some(action) => {
            state.pending_action = Some(action);
            HandleResult {
                reply: if language.is_chinese() {
                    "没有更多页面了。".to_string()
                } else {
                    "No more pages available.".to_string()
                },
                actions: vec![],
                forward_to_session: None,
            }
        }
        None => handle_chat_message(state, "0", vec![]).await,
    }
}

async fn handle_chat_message(
    state: &mut BotChatState,
    message: &str,
    image_contexts: Vec<crate::agentic::image_analysis::ImageContextData>,
) -> HandleResult {
    let language = current_bot_language().await;
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
                reply: if language.is_chinese() {
                    "请回复工作区编号。".to_string()
                } else {
                    "Please reply with the workspace number.".to_string()
                },
                actions: vec![],
                forward_to_session: None,
            },
            PendingAction::SelectAssistant { .. } => HandleResult {
                reply: if language.is_chinese() {
                    "请回复助理编号。".to_string()
                } else {
                    "Please reply with the assistant number.".to_string()
                },
                actions: vec![],
                forward_to_session: None,
            },
            PendingAction::SelectSession { has_more, .. } => HandleResult {
                reply: if has_more {
                    if language.is_chinese() {
                        "请回复会话编号，或回复 `0` 查看下一页。".to_string()
                    } else {
                        "Please reply with the session number, or `0` for the next page."
                            .to_string()
                    }
                } else {
                    if language.is_chinese() {
                        "请回复会话编号。".to_string()
                    } else {
                        "Please reply with the session number.".to_string()
                    }
                },
                actions: vec![],
                forward_to_session: None,
            },
            PendingAction::AskUserQuestion { .. } => unreachable!(),
        };
    }

    if state.display_mode == BotDisplayMode::Pro && state.current_workspace.is_none() {
        return HandleResult {
            reply: if language.is_chinese() {
                "尚未选择工作区。请先使用 /switch_workspace 选择工作区。".to_string()
            } else {
                "No workspace selected. Use /switch_workspace to select one first.".to_string()
            },
            actions: workspace_required_actions(language),
            forward_to_session: None,
        };
    }
    if state.current_session_id.is_none() {
        let reply = if language.is_chinese() {
            if state.display_mode == BotDisplayMode::Pro {
                "当前没有活动会话。请使用 /resume_session 恢复已有会话，或使用 /new_code_session、/new_cowork_session 创建新会话。"
                    .to_string()
            } else {
                "当前没有活动会话。请使用 /resume_session 恢复已有会话，或使用 /new_claw_session 创建新会话。"
                    .to_string()
            }
        } else {
            if state.display_mode == BotDisplayMode::Pro {
                "No active session. Use /resume_session to resume one or /new_code_session, /new_cowork_session to create a new one."
                    .to_string()
            } else {
                "No active session. Use /resume_session to resume one or /new_claw_session to create a new one."
                    .to_string()
            }
        };
        return HandleResult {
            reply,
            actions: session_entry_actions(language, state.display_mode),
            forward_to_session: None,
        };
    }

    let session_id = state.current_session_id.clone().unwrap();
    let turn_id = format!("turn_{}", uuid::Uuid::new_v4());

    let session_busy = {
        use crate::agentic::coordination::get_global_coordinator;
        use crate::agentic::core::SessionState;
        get_global_coordinator()
            .and_then(|c| c.get_session_manager().get_session(&session_id))
            .is_some_and(|s| matches!(s.state, SessionState::Processing { .. }))
    };

    if session_busy {
        return HandleResult {
            reply: if language.is_chinese() {
                "消息已加入队列，将在当前助手步骤结束后自动处理。".to_string()
            } else {
                "Your message was queued and will run after the current assistant step finishes."
                    .to_string()
            },
            actions: vec![],
            forward_to_session: Some(ForwardRequest {
                session_id,
                content: message.to_string(),
                agent_type: "agentic".to_string(),
                turn_id,
                image_contexts,
            }),
        };
    }

    let cancel_command = format!("/cancel_task {}", turn_id);
    let verbose_mode = super::load_bot_persistence().verbose_mode;
    let processing_line = if language.is_chinese() {
        "正在处理你的消息..."
    } else {
        "Processing your message..."
    };
    let reply = if verbose_mode {
        format!(
            "{}\n\n{}",
            processing_line,
            if language.is_chinese() {
                format!("如需停止本次请求，请发送 `{}`。", cancel_command)
            } else {
                format!("If needed, send `{}` to stop this request.", cancel_command)
            },
        )
    } else {
        processing_line.to_string()
    };
    HandleResult {
        reply,
        actions: cancel_task_actions(language, cancel_command),
        forward_to_session: Some(ForwardRequest {
            session_id,
            content: message.to_string(),
            agent_type: "agentic".to_string(),
            turn_id,
            image_contexts,
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
        let msg = if language.is_chinese() {
            format!("发送消息失败：{e}")
        } else {
            format!("Failed to send message: {e}")
        };
        return ForwardedTurnResult {
            display_text: msg.clone(),
            full_text: msg,
        };
    }

    let result = tokio::time::timeout(std::time::Duration::from_secs(3600), async {
        let mut response = String::new();
        let mut thinking_buf = String::new();
        // Cache tool params from ToolStarted so we can display them on ToolCompleted.
        let mut tool_params_cache: std::collections::HashMap<String, Option<serde_json::Value>> =
            std::collections::HashMap::new();

        let streams_our_turn = || {
            tracker
                .snapshot_active_turn()
                .map(|s| s.turn_id == target_turn_id)
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
                                let msg = if language.is_chinese() {
                                    format!("[思考过程]\n{content}")
                                } else {
                                    format!("[Thinking]\n{content}")
                                };
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
                        if tool_name == "AskUserQuestion" {
                            if let Some(questions_value) =
                                params.and_then(|p| p.get("questions").cloned())
                            {
                                if let Ok(questions) =
                                    serde_json::from_value::<Vec<BotQuestion>>(questions_value)
                                {
                                    let request = build_question_prompt(
                                        language,
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
                        } else {
                            tool_params_cache.insert(tool_id, params);
                        }
                    }
                    TrackerEvent::ToolCompleted {
                        tool_id,
                        tool_name,
                        duration_ms,
                        success,
                    } => {
                        if !streams_our_turn() {
                            continue;
                        }
                        if verbose_mode {
                            if let Some(sender) = message_sender.as_ref() {
                                let params_str = tool_params_cache
                                    .remove(&tool_id)
                                    .flatten()
                                    .and_then(|p| format_tool_params_slim(&p))
                                    .unwrap_or_default();
                                let duration_str = duration_ms
                                    .map(|ms| {
                                        if ms >= 1000 {
                                            format!("{:.1}s", ms as f64 / 1000.0)
                                        } else {
                                            format!("{}ms", ms)
                                        }
                                    })
                                    .unwrap_or_default();
                                let status = if success { "OK" } else { "FAILED" };
                                let msg = if params_str.is_empty() {
                                    format!("[{tool_name}] {status} {duration_str}")
                                } else {
                                    format!(
                                        "[{tool_name}] {params_str}\n=> {status} {duration_str}"
                                    )
                                };
                                sender(msg).await;
                            }
                        }
                    }
                    TrackerEvent::TurnCompleted { turn_id } => {
                        if turn_id == target_turn_id {
                            break;
                        }
                    }
                    TrackerEvent::TurnFailed { turn_id, error } => {
                        if turn_id == target_turn_id {
                            let msg = if language.is_chinese() {
                                format!("错误: {error}")
                            } else {
                                format!("Error: {error}")
                            };
                            return ForwardedTurnResult {
                                display_text: msg.clone(),
                                full_text: msg,
                            };
                        }
                    }
                    TrackerEvent::TurnCancelled { turn_id } => {
                        if turn_id == target_turn_id {
                            let msg = if language.is_chinese() {
                                "任务已取消。".to_string()
                            } else {
                                "Task was cancelled.".to_string()
                            };
                            return ForwardedTurnResult {
                                display_text: msg.clone(),
                                full_text: msg,
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

        let mut display_text = full_text.clone();
        const MAX_BOT_MSG_LEN: usize = 4000;
        if display_text.len() > MAX_BOT_MSG_LEN {
            let mut end = MAX_BOT_MSG_LEN;
            while !display_text.is_char_boundary(end) {
                end -= 1;
            }
            display_text.truncate(end);
            display_text.push_str("\n\n... (truncated)");
        }

        ForwardedTurnResult {
            display_text: if display_text.is_empty() {
                if language.is_chinese() {
                    "（无回复）".to_string()
                } else {
                    "(No response)".to_string()
                }
            } else {
                display_text
            },
            full_text,
        }
    })
    .await;

    result.unwrap_or_else(|_| ForwardedTurnResult {
        display_text: if language.is_chinese() {
            "等待 1 小时后响应超时。".to_string()
        } else {
            "Response timed out after 1 hour.".to_string()
        },
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

/// Format tool params into a compact display string for bot messages.
/// Filters out large string values and truncates remaining ones.
fn format_tool_params_slim(params: &serde_json::Value) -> Option<String> {
    const MAX_VAL_LEN: usize = 120;
    match params {
        serde_json::Value::Object(obj) => {
            let parts: Vec<String> = obj
                .iter()
                .filter_map(|(k, v)| {
                    let val_str = match v {
                        serde_json::Value::String(s) => {
                            if s.len() > MAX_VAL_LEN {
                                return None;
                            }
                            s.clone()
                        }
                        serde_json::Value::Bool(b) => b.to_string(),
                        serde_json::Value::Number(n) => n.to_string(),
                        serde_json::Value::Null => "null".to_string(),
                        _ => {
                            let json = serde_json::to_string(v).unwrap_or_default();
                            if json.len() > MAX_VAL_LEN {
                                return None;
                            }
                            json
                        }
                    };
                    Some(format!("{k}: {val_str}"))
                })
                .collect();
            if parts.is_empty() {
                None
            } else {
                Some(parts.join(", "))
            }
        }
        serde_json::Value::String(s) => Some(truncate_at_char_boundary(s, MAX_VAL_LEN)),
        _ => None,
    }
}

#[cfg(test)]
mod parse_command_tests {
    use super::{parse_command, BotCommand};

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
}
