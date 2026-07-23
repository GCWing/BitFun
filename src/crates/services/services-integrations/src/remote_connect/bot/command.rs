use super::state::BotDisplayMode;
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
    /// Switch the model used by the current session.
    SwitchModel,
    /// Cancel currently running task.
    CancelTask(Option<String>),
    /// Pairing code submitted before pairing.
    PairingCode(String),
    /// Numeric reply to a menu / pending action.
    NumberSelection(usize),
    /// Free-form chat message forwarded to the AI session.
    ChatMessage(String),
    /// List all online same-account devices (multi-device control).
    ListDevices,
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

        // Switch model for the current session.
        "/model" | "/switch_model" | "切换模型" => return BotCommand::SwitchModel,
        _ => {}
    }

    // Multi-device commands (require account login on paired desktop).
    if lower == "/devices" || lower == "/list_devices" || lower == "设备列表" {
        return BotCommand::ListDevices;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_core_navigation_aliases() {
        assert!(matches!(parse_command("/menu"), BotCommand::Menu));
        assert!(matches!(parse_command("/settings"), BotCommand::Settings));
        assert!(matches!(parse_command("/help"), BotCommand::Help));
        assert!(matches!(
            parse_command("/resume"),
            BotCommand::ResumeSession
        ));
    }

    #[test]
    fn parses_mode_verbose_and_cancel_commands() {
        assert!(matches!(
            parse_command("/pro"),
            BotCommand::SwitchMode(BotDisplayMode::Pro)
        ));
        assert!(matches!(
            parse_command("/assistant"),
            BotCommand::SwitchMode(BotDisplayMode::Assistant)
        ));
        assert!(matches!(
            parse_command("/verbose"),
            BotCommand::SetVerbose(true)
        ));
        assert!(matches!(
            parse_command("/concise"),
            BotCommand::SetVerbose(false)
        ));
        assert!(matches!(
            parse_command("/cancel"),
            BotCommand::CancelTask(None)
        ));
        assert!(matches!(
            parse_command("/cancel_task turn-1"),
            BotCommand::CancelTask(Some(id)) if id == "turn-1"
        ));
    }

    #[test]
    fn parses_pairing_numeric_and_chat_fallbacks() {
        assert!(matches!(
            parse_command("123456"),
            BotCommand::PairingCode(code) if code == "123456"
        ));
        assert!(matches!(
            parse_command("１"),
            BotCommand::NumberSelection(1)
        ));
        assert!(matches!(parse_command("0"), BotCommand::NumberSelection(0)));
        assert!(matches!(
            parse_command("hello"),
            BotCommand::ChatMessage(text) if text == "hello"
        ));
    }
}

// ── Public welcome / help text (compat) ───────────────────────────
