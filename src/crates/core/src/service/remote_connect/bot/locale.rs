//! Centralised IM-bot strings for the simplified bot UX.
//!
//! All user-facing IM bot strings live here so command routing, menu
//! rendering, and platform adapters can share a single source of truth.
//! New languages add one more `static BotStrings` and one match arm in
//! [`strings_for`].

use serde::{Deserialize, Serialize};

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

/// Centralised string table consumed by command router, menu builder, and
/// platform adapters.  Add new strings here, then translate in both
/// [`STRINGS_ZH`] and [`STRINGS_EN`].
pub struct BotStrings {
    // ── Onboarding ───────────────────────────────────────────────
    pub welcome: &'static str,
    pub paired_success: &'static str,
    pub need_pairing: &'static str,
    pub invalid_pairing_code: &'static str,
    pub bootstrap_workspace_unavailable: &'static str,
    pub bootstrap_session_failed_prefix: &'static str,
    pub bootstrap_ready: &'static str,

    // ── Mode / context labels ────────────────────────────────────
    pub mode_assistant: &'static str,
    pub mode_expert: &'static str,
    pub current_session_label: &'static str,
    pub current_workspace_label: &'static str,
    pub current_assistant_label: &'static str,
    pub no_session: &'static str,
    pub no_workspace: &'static str,
    pub no_assistant: &'static str,

    // ── Main menu (one-line title) ───────────────────────────────
    pub main_title_assistant: &'static str,
    pub main_title_expert: &'static str,
    pub settings_title: &'static str,
    pub welcome_title: &'static str,
    pub need_session_title: &'static str,

    // ── Menu item labels (≤ 14 chars target) ─────────────────────
    pub item_new_session: &'static str,
    pub item_new_code_session: &'static str,
    pub item_new_cowork_session: &'static str,
    pub item_resume_session: &'static str,
    pub item_switch_assistant: &'static str,
    pub item_switch_workspace: &'static str,
    pub item_settings: &'static str,
    pub item_back: &'static str,
    pub item_help: &'static str,
    pub item_switch_to_expert: &'static str,
    pub item_switch_to_assistant: &'static str,
    pub item_verbose_on: &'static str,
    pub item_verbose_off: &'static str,
    pub item_cancel_task: &'static str,
    pub item_confirm_switch: &'static str,
    pub item_next_page: &'static str,
    pub item_other: &'static str,

    // ── Auxiliary labels ─────────────────────────────────────────
    pub question_title: &'static str,
    pub verbose_label: &'static str,
    pub workspace_session_count_fmt: &'static str,

    // ── Footer hints ─────────────────────────────────────────────
    pub footer_reply_or_menu: &'static str,
    pub footer_reply_workspace: &'static str,
    pub footer_reply_assistant: &'static str,
    pub footer_reply_session_or_next: &'static str,
    pub footer_reply_session: &'static str,
    pub footer_question_single: &'static str,
    pub footer_question_multi: &'static str,
    pub footer_question_custom: &'static str,
    pub footer_processing_cancel_hint: &'static str,

    // ── Body / inline texts ──────────────────────────────────────
    pub welcome_body: &'static str,
    pub paired_body_intro: &'static str,
    pub help_body: &'static str,

    pub switch_pick_workspace: &'static str,
    pub switch_pick_assistant: &'static str,
    pub switch_no_workspaces: &'static str,
    pub switch_no_assistants: &'static str,
    pub current_marker: &'static str,

    pub resume_no_sessions: &'static str,
    pub resume_page_label: &'static str,
    pub resume_msg_count_zero: &'static str,
    pub resume_msg_count_one: &'static str,
    pub resume_msg_count_many_fmt: &'static str,
    pub resume_resumed_prefix: &'static str,
    pub resume_last_dialog_header: &'static str,
    pub resume_you_label: &'static str,
    pub resume_continue_hint: &'static str,
    pub resume_first_message_hint: &'static str,

    pub processing: &'static str,
    pub queued: &'static str,
    pub no_response: &'static str,
    pub task_cancelled: &'static str,
    pub task_cancel_requested: &'static str,
    pub task_cancel_failed_prefix: &'static str,
    pub task_no_active: &'static str,
    pub timeout_one_hour: &'static str,
    pub error_prefix: &'static str,
    pub send_failed_prefix: &'static str,

    pub mode_switched_to_expert: &'static str,
    pub mode_switched_to_assistant: &'static str,
    pub mode_already_expert: &'static str,
    pub mode_already_assistant: &'static str,
    pub mode_confirm_switch_prefix: &'static str,

    pub verbose_enabled: &'static str,
    pub verbose_disabled: &'static str,
    pub verbose_status_on: &'static str,
    pub verbose_status_off: &'static str,

    pub session_created_prefix: &'static str,
    pub session_workspace_label: &'static str,
    pub session_start_hint: &'static str,
    pub session_create_failed_prefix: &'static str,
    pub session_system_unavailable: &'static str,
    pub workspace_service_unavailable: &'static str,
    pub workspace_open_failed_prefix: &'static str,
    pub assistant_create_failed_prefix: &'static str,

    pub pending_expired: &'static str,
    pub pending_invalid_input: &'static str,
    pub pending_invalid_after_retries: &'static str,
    pub pending_back_hint: &'static str,

    pub answers_submitted: &'static str,
    pub answers_submit_failed_prefix: &'static str,
    pub question_invalid_state: &'static str,
    pub question_custom_required: &'static str,
    pub question_custom_for_other_prefix: &'static str,

    pub thinking_label: &'static str,

    pub auto_push_intro_one: &'static str,
    pub auto_push_intro_many_fmt: &'static str,
    pub auto_push_skip_too_large_fmt: &'static str,
    pub auto_push_failed_fmt: &'static str,
}

const STRINGS_ZH: BotStrings = BotStrings {
    welcome: "\
欢迎使用 BitFun。

请在 BitFun 桌面端打开 Remote Connect 面板，复制 6 位配对码并发送到这里完成连接。",
    paired_success: "配对成功，BitFun 已连接。",
    need_pairing: "尚未连接 BitFun 桌面端。请先发送 6 位配对码。",
    invalid_pairing_code: "配对码无效或已过期，请到桌面端重新生成后再发送。",
    bootstrap_workspace_unavailable: "工作区服务暂时不可用，请稍后再试。",
    bootstrap_session_failed_prefix: "已进入助理模式，但创建会话失败：",
    bootstrap_ready: "已为你新建助理会话，直接发送消息即可开始。",

    mode_assistant: "助理模式",
    mode_expert: "专业模式",
    current_session_label: "当前会话",
    current_workspace_label: "当前工作区",
    current_assistant_label: "当前助理",
    no_session: "尚未选择会话",
    no_workspace: "尚未选择工作区",
    no_assistant: "尚未选择助理",

    main_title_assistant: "BitFun · 助理模式",
    main_title_expert: "BitFun · 专业模式",
    settings_title: "设置",
    welcome_title: "BitFun",
    need_session_title: "请先选择或新建会话",

    item_new_session: "新建会话",
    item_new_code_session: "新建编码会话",
    item_new_cowork_session: "新建协作会话",
    item_resume_session: "恢复会话",
    item_switch_assistant: "切换助理",
    item_switch_workspace: "切换工作区",
    item_settings: "设置",
    item_back: "返回",
    item_help: "帮助",
    item_switch_to_expert: "切换到专业模式",
    item_switch_to_assistant: "切换到助理模式",
    item_verbose_on: "开启执行细节",
    item_verbose_off: "关闭执行细节",
    item_cancel_task: "取消任务",
    item_confirm_switch: "切换并继续",
    item_next_page: "下一页",
    item_other: "其他",

    question_title: "问题",
    verbose_label: "执行细节",
    workspace_session_count_fmt: "{n} 个会话",

    footer_reply_or_menu: "回复编号，或发送 /menu 返回主菜单",
    footer_reply_workspace: "回复工作区编号，或发送 0 返回",
    footer_reply_assistant: "回复助理编号，或发送 0 返回",
    footer_reply_session_or_next: "回复会话编号；发送 0 查看下一页或返回",
    footer_reply_session: "回复会话编号，或发送 0 返回",
    footer_question_single: "回复单个选项编号；发送 /menu 退出",
    footer_question_multi: "回复一个或多个选项编号（如 1,3）；发送 /menu 退出",
    footer_question_custom: "请输入你的自定义答案；发送 /menu 退出",
    footer_processing_cancel_hint: "如需中止，回复 /cancel 或点击「取消任务」",

    welcome_body: "当前未配对。",
    paired_body_intro: "可以直接发送消息开始对话。",
    help_body: "\
常用命令：
/menu  返回主菜单
/new   新建会话
/resume  恢复历史会话
/switch  切换助理或工作区
/cancel  取消当前任务
/expert  /assistant  切换模式
/verbose /concise  开关执行细节
/help  显示本帮助",

    switch_pick_workspace: "请选择要切换的工作区：",
    switch_pick_assistant: "请选择要切换的助理：",
    switch_no_workspaces: "尚未发现工作区，请先在 BitFun 桌面端打开一个项目。",
    switch_no_assistants: "尚未发现助理，请先在 BitFun 桌面端创建一个助理。",
    current_marker: " · 当前",

    resume_no_sessions: "当前还没有会话，可以发送 /new 直接新建。",
    resume_page_label: "会话历史",
    resume_msg_count_zero: "无消息",
    resume_msg_count_one: "1 条消息",
    resume_msg_count_many_fmt: "{n} 条消息",
    resume_resumed_prefix: "已恢复会话：",
    resume_last_dialog_header: "— 最近一次对话 —",
    resume_you_label: "你",
    resume_continue_hint: "可以继续对话。",
    resume_first_message_hint: "发送一条消息即可开始。",

    processing: "正在处理你的消息……",
    queued: "消息已加入队列，等当前步骤结束会自动接续。",
    no_response: "（无回复）",
    task_cancelled: "任务已取消。",
    task_cancel_requested: "已请求取消当前任务。",
    task_cancel_failed_prefix: "取消任务失败：",
    task_no_active: "当前没有正在运行的任务。",
    timeout_one_hour: "等待响应超时（1 小时）。",
    error_prefix: "错误：",
    send_failed_prefix: "发送失败：",

    mode_switched_to_expert: "已切换到专业模式，可创建编码 / 协作会话。",
    mode_switched_to_assistant: "已切换到助理模式，适合日常持续对话。",
    mode_already_expert: "当前已在专业模式。",
    mode_already_assistant: "当前已在助理模式。",
    mode_confirm_switch_prefix: "该操作需要切换到另一种模式，确认继续吗？",

    verbose_enabled: "已开启「执行细节」，下一次任务会显示思考与工具过程。",
    verbose_disabled: "已关闭「执行细节」，仅显示最终结果。",
    verbose_status_on: "开",
    verbose_status_off: "关",

    session_created_prefix: "已创建新会话：",
    session_workspace_label: "工作区：",
    session_start_hint: "可以发送消息开始对话。",
    session_create_failed_prefix: "创建会话失败：",
    session_system_unavailable: "BitFun 会话系统尚未就绪，请稍后再试。",
    workspace_service_unavailable: "工作区服务暂时不可用。",
    workspace_open_failed_prefix: "打开工作区失败：",
    assistant_create_failed_prefix: "创建助理工作区失败：",

    pending_expired: "上一步已超时，已为你返回主菜单。",
    pending_invalid_input: "输入无效，请按提示回复或发送 /menu 返回主菜单。",
    pending_invalid_after_retries: "多次输入无效，已为你返回主菜单。",
    pending_back_hint: "发送 0 或 /menu 返回主菜单。",

    answers_submitted: "答案已提交，等待助手继续……",
    answers_submit_failed_prefix: "提交答案失败：",
    question_invalid_state: "问题状态无效，请重新发起对话。",
    question_custom_required: "自定义答案不能为空，请重新输入。",
    question_custom_for_other_prefix: "请为「其他」输入你的自定义答案：",

    thinking_label: "思考中",

    auto_push_intro_one: "正在为你发送 1 个文件……",
    auto_push_intro_many_fmt: "正在为你发送 {n} 个文件……",
    auto_push_skip_too_large_fmt: "已跳过「{name}」：{size} 超过 {limit} 上限，请改用桌面端获取。",
    auto_push_failed_fmt: "发送「{name}」失败：{err}",
};

const STRINGS_EN: BotStrings = BotStrings {
    welcome: "\
Welcome to BitFun.

Open Remote Connect in BitFun Desktop and send the 6-digit pairing code here to connect.",
    paired_success: "Pairing successful. BitFun is now connected.",
    need_pairing: "Not connected yet. Please send the 6-digit pairing code first.",
    invalid_pairing_code: "Invalid or expired pairing code. Generate a new one in BitFun Desktop and try again.",
    bootstrap_workspace_unavailable: "Workspace service is unavailable. Please try again shortly.",
    bootstrap_session_failed_prefix: "Assistant mode is on but session creation failed: ",
    bootstrap_ready: "A new assistant session is ready. Send a message to start.",

    mode_assistant: "Assistant Mode",
    mode_expert: "Expert Mode",
    current_session_label: "Current session",
    current_workspace_label: "Current workspace",
    current_assistant_label: "Current assistant",
    no_session: "No session selected",
    no_workspace: "No workspace selected",
    no_assistant: "No assistant selected",

    main_title_assistant: "BitFun · Assistant",
    main_title_expert: "BitFun · Expert",
    settings_title: "Settings",
    welcome_title: "BitFun",
    need_session_title: "Pick or create a session first",

    item_new_session: "New Session",
    item_new_code_session: "New Code Session",
    item_new_cowork_session: "New Cowork Session",
    item_resume_session: "Resume Session",
    item_switch_assistant: "Switch Assistant",
    item_switch_workspace: "Switch Workspace",
    item_settings: "Settings",
    item_back: "Back",
    item_help: "Help",
    item_switch_to_expert: "Switch to Expert Mode",
    item_switch_to_assistant: "Switch to Assistant Mode",
    item_verbose_on: "Show Execution Details",
    item_verbose_off: "Hide Execution Details",
    item_cancel_task: "Cancel Task",
    item_confirm_switch: "Switch & Continue",
    item_next_page: "Next Page",
    item_other: "Other",

    question_title: "Question",
    verbose_label: "Execution details",
    workspace_session_count_fmt: "{n} sessions",

    footer_reply_or_menu: "Reply with a number, or send /menu to return.",
    footer_reply_workspace: "Reply with a workspace number, or 0 to go back.",
    footer_reply_assistant: "Reply with an assistant number, or 0 to go back.",
    footer_reply_session_or_next: "Reply with a session number; send 0 for next page or to go back.",
    footer_reply_session: "Reply with a session number, or 0 to go back.",
    footer_question_single: "Reply with one option number; send /menu to exit.",
    footer_question_multi: "Reply with one or more option numbers (e.g. 1,3); send /menu to exit.",
    footer_question_custom: "Type your custom answer; send /menu to exit.",
    footer_processing_cancel_hint: "To stop, reply /cancel or tap Cancel Task.",

    welcome_body: "Not paired yet.",
    paired_body_intro: "Send a message to start the conversation.",
    help_body: "\
Common commands:
/menu  Return to the main menu
/new   Create a new session
/resume  Resume an existing session
/switch  Switch assistant or workspace
/cancel  Cancel the current task
/expert  /assistant  Switch modes
/verbose /concise  Toggle execution details
/help  Show this help",

    switch_pick_workspace: "Pick a workspace to switch to:",
    switch_pick_assistant: "Pick an assistant to switch to:",
    switch_no_workspaces: "No workspaces found. Open a project in BitFun Desktop first.",
    switch_no_assistants: "No assistants found. Create one in BitFun Desktop first.",
    current_marker: " · current",

    resume_no_sessions: "No sessions yet. Send /new to create one.",
    resume_page_label: "Sessions",
    resume_msg_count_zero: "no messages",
    resume_msg_count_one: "1 message",
    resume_msg_count_many_fmt: "{n} messages",
    resume_resumed_prefix: "Resumed session: ",
    resume_last_dialog_header: "— Last conversation —",
    resume_you_label: "You",
    resume_continue_hint: "You can continue the conversation.",
    resume_first_message_hint: "Send a message to start.",

    processing: "Processing your message…",
    queued: "Message queued. It will run when the current step finishes.",
    no_response: "(no response)",
    task_cancelled: "Task cancelled.",
    task_cancel_requested: "Cancellation requested.",
    task_cancel_failed_prefix: "Failed to cancel: ",
    task_no_active: "No active task to cancel.",
    timeout_one_hour: "Response timed out after 1 hour.",
    error_prefix: "Error: ",
    send_failed_prefix: "Send failed: ",

    mode_switched_to_expert: "Switched to Expert mode. You can create code or cowork sessions.",
    mode_switched_to_assistant: "Switched to Assistant mode. Best for ongoing conversations.",
    mode_already_expert: "Already in Expert mode.",
    mode_already_assistant: "Already in Assistant mode.",
    mode_confirm_switch_prefix: "This action needs the other mode. Switch and continue?",

    verbose_enabled: "Execution details enabled. The next task will show thinking and tool steps.",
    verbose_disabled: "Execution details disabled. Only final results will be shown.",
    verbose_status_on: "on",
    verbose_status_off: "off",

    session_created_prefix: "New session created: ",
    session_workspace_label: "Workspace: ",
    session_start_hint: "Send a message to start the conversation.",
    session_create_failed_prefix: "Failed to create session: ",
    session_system_unavailable: "BitFun session system is not ready yet.",
    workspace_service_unavailable: "Workspace service unavailable.",
    workspace_open_failed_prefix: "Failed to open workspace: ",
    assistant_create_failed_prefix: "Failed to create assistant workspace: ",

    pending_expired: "Previous step expired. Returned to the main menu.",
    pending_invalid_input: "Invalid input. Follow the prompt above, or send /menu to return.",
    pending_invalid_after_retries: "Too many invalid replies. Returned to the main menu.",
    pending_back_hint: "Send 0 or /menu to return to the main menu.",

    answers_submitted: "Answers submitted. Waiting for the assistant to continue…",
    answers_submit_failed_prefix: "Failed to submit answers: ",
    question_invalid_state: "Question state is invalid; please restart the conversation.",
    question_custom_required: "Custom answer cannot be empty. Please type it again.",
    question_custom_for_other_prefix: "Type your custom answer for `Other`: ",

    thinking_label: "Thinking",

    auto_push_intro_one: "Sending 1 file for you…",
    auto_push_intro_many_fmt: "Sending {n} files for you…",
    auto_push_skip_too_large_fmt: "Skipping \"{name}\": {size} exceeds the {limit} limit. Please grab it from BitFun Desktop instead.",
    auto_push_failed_fmt: "Failed to send \"{name}\": {err}",
};

pub fn strings_for(language: BotLanguage) -> &'static BotStrings {
    match language {
        BotLanguage::ZhCN => &STRINGS_ZH,
        BotLanguage::EnUS => &STRINGS_EN,
    }
}

/// Substitute `{n}` placeholder in formatted strings.
pub fn fmt_count(template: &str, n: usize) -> String {
    template.replace("{n}", &n.to_string())
}
