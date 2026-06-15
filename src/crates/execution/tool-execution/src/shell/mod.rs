use std::collections::HashMap;

use crate::util::ansi_cleaner::strip_ansi;
use crate::util::string::shell_single_quote;

pub const BASH_RESULT_MAX_OUTPUT_LENGTH: usize = 30_000;
pub const BASH_INTERRUPT_OUTPUT_DRAIN_MS: u64 = 500;

const BANNED_COMMANDS: &[&str] = &[
    "alias",
    // "curl",
    // "curlie",
    // "wget",
    // "axel",
    // "aria2c",
    // "nc",
    // "telnet",
    // "lynx",
    // "w3m",
    // "links",
    // "httpie",
    // "xh",
    // "http-prompt",
    // "chrome",
    // "firefox",
    // "safari",
];

pub fn banned_shell_command(cmd: &str) -> Option<&str> {
    let base_cmd = cmd.split_whitespace().next()?;
    let base_cmd_lc = base_cmd.to_lowercase();
    BANNED_COMMANDS
        .iter()
        .any(|banned| base_cmd_lc == *banned)
        .then_some(base_cmd)
}

pub fn detect_osascript_keystroke_non_ascii(cmd: &str) -> Option<String> {
    if !cmd.contains("osascript") {
        return None;
    }

    let bytes = cmd.as_bytes();
    let needle = b"keystroke";
    let mut i = 0usize;
    while i + needle.len() < bytes.len() {
        if &bytes[i..i + needle.len()] == needle {
            let mut j = i + needle.len();
            while j < bytes.len() && bytes[j] != b'"' {
                j += 1;
            }
            if j >= bytes.len() {
                break;
            }
            let start = j + 1;
            let mut end = start;
            while end < bytes.len() && bytes[end] != b'"' {
                end += 1;
            }
            if end > bytes.len() {
                break;
            }
            let literal = &cmd[start..end.min(cmd.len())];
            if !literal.is_ascii() {
                return Some(literal.to_string());
            }
            i = end + 1;
        } else {
            i += 1;
        }
    }
    None
}

pub fn detect_osascript_im_app(cmd: &str) -> Option<&'static str> {
    if !cmd.contains("osascript") {
        return None;
    }
    const IM_APPS: &[&str] = &[
        "WeChat", "微信", "iMessage", "Messages", "Slack", "Lark", "飞书", "Telegram", "DingTalk",
        "钉钉", "QQ", "Discord", "Teams", "Whatsapp", "WhatsApp",
    ];
    let cmd_lc = cmd.to_lowercase();
    for app in IM_APPS {
        let app_lc = app.to_lowercase();
        if cmd.contains(app) || cmd_lc.contains(&app_lc) {
            return Some(*app);
        }
    }
    None
}

pub fn command_for_working_directory(command: &str, working_directory: Option<&str>) -> String {
    working_directory
        .map(str::trim)
        .filter(|dir| !dir.is_empty())
        .map(|dir| format!("cd {} && {}", shell_single_quote(dir), command))
        .unwrap_or_else(|| command.to_string())
}

pub fn bash_noninteractive_env() -> HashMap<String, String> {
    let mut env = HashMap::new();
    env.insert("BITFUN_NONINTERACTIVE".to_string(), "1".to_string());
    env.insert("GIT_PAGER".to_string(), "cat".to_string());
    env.insert("PAGER".to_string(), "cat".to_string());
    env.insert("GIT_TERMINAL_PROMPT".to_string(), "0".to_string());
    env.insert("GIT_EDITOR".to_string(), "true".to_string());
    env
}

pub fn truncate_output_preserving_tail(s: &str, max_chars: usize) -> String {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= max_chars {
        return s.to_string();
    }

    let tail_bias = max_chars.saturating_mul(4) / 5;
    let separator = "\n... [truncated, middle omitted, tail preserved] ...\n";
    let separator_len = separator.chars().count();

    if separator_len >= max_chars {
        return chars[chars.len() - max_chars..].iter().collect();
    }

    let content_budget = max_chars - separator_len;
    let tail_len = tail_bias.min(content_budget);
    let head_len = content_budget.saturating_sub(tail_len);

    let head: String = chars[..head_len].iter().collect();
    let tail: String = chars[chars.len() - tail_len..].iter().collect();

    format!("{head}{separator}{tail}")
}

pub struct LocalShellResultRenderRequest<'a> {
    pub terminal_session_id: &'a str,
    pub working_directory: &'a str,
    pub output_text: &'a str,
    pub interrupted: bool,
    pub timed_out: bool,
    pub exit_code: i32,
    pub shell_state: Option<&'a str>,
}

pub fn render_local_shell_result(request: LocalShellResultRenderRequest<'_>) -> String {
    let mut result_string = String::new();

    result_string.push_str(&format!("<exit_code>{}</exit_code>", request.exit_code));
    if !request.working_directory.is_empty() {
        result_string.push_str(&format!(
            "<working_directory>{}</working_directory>",
            request.working_directory
        ));
    }

    if let Some(output_block) =
        render_output_block_with_limit("output", request.output_text, BASH_RESULT_MAX_OUTPUT_LENGTH)
    {
        result_string.push_str(&output_block);
    }

    if let Some(state) = request.shell_state {
        let cleaned_state = strip_ansi(state);
        result_string.push_str(&format!("<shell_state>{}</shell_state>", cleaned_state));
    }

    if request.timed_out {
        result_string.push_str(
            "<status type=\"timeout\">Command timed out before completion. Partial output, if any, is included above.</status>",
        );
    } else if request.interrupted {
        result_string.push_str(
            "<status type=\"interrupted\">Command was canceled by the user. ASK THE USER what they would like to do next.</status>"
        );
    }

    result_string.push_str(&format!(
        "<terminal_session_id>{}</terminal_session_id>",
        request.terminal_session_id
    ));

    result_string
}

pub fn render_output_block_with_limit(
    tag: &str,
    output_text: &str,
    max_chars: usize,
) -> Option<String> {
    if output_text.is_empty() {
        return None;
    }

    let cleaned_output = strip_ansi(output_text);
    let output_len = cleaned_output.chars().count();
    if max_chars == 0 {
        Some(format!(
            "<{tag} truncated=\"true\">... [truncated, no budget remaining] ...</{tag}>"
        ))
    } else if output_len > max_chars {
        let truncated = truncate_output_preserving_tail(&cleaned_output, max_chars);
        Some(format!("<{tag} truncated=\"true\">{}</{tag}>", truncated))
    } else {
        Some(format!("<{tag}>{}</{tag}>", cleaned_output))
    }
}

pub fn remote_stream_budgets(stdout: &str, stderr: &str) -> (usize, usize) {
    let stdout_len = strip_ansi(stdout).chars().count();
    let stderr_len = strip_ansi(stderr).chars().count();

    if stderr_len >= BASH_RESULT_MAX_OUTPUT_LENGTH {
        return (0, BASH_RESULT_MAX_OUTPUT_LENGTH);
    }

    let stderr_budget = stderr_len;
    let stdout_budget = BASH_RESULT_MAX_OUTPUT_LENGTH.saturating_sub(stderr_budget);
    (stdout_budget.min(stdout_len), stderr_budget)
}

pub struct RemoteShellResultRenderRequest<'a> {
    pub working_directory: &'a str,
    pub stdout: &'a str,
    pub stderr: &'a str,
    pub interrupted: bool,
    pub timed_out: bool,
    pub exit_code: i32,
}

pub fn render_remote_shell_result(request: RemoteShellResultRenderRequest<'_>) -> String {
    let mut result_string = String::new();
    result_string.push_str("<remote_ssh>true</remote_ssh>");
    result_string.push_str(&format!("<exit_code>{}</exit_code>", request.exit_code));
    if !request.working_directory.is_empty() {
        result_string.push_str(&format!(
            "<working_directory>{}</working_directory>",
            request.working_directory
        ));
    }

    let (stdout_budget, stderr_budget) = remote_stream_budgets(request.stdout, request.stderr);

    if let Some(stdout_block) =
        render_output_block_with_limit("stdout", request.stdout, stdout_budget)
    {
        result_string.push_str(&stdout_block);
    }
    if let Some(stderr_block) =
        render_output_block_with_limit("stderr", request.stderr, stderr_budget)
    {
        result_string.push_str(&stderr_block);
    }

    if request.timed_out {
        result_string.push_str(
            "<status type=\"timeout\">Command timed out before completion. Partial stdout/stderr, if any, is included above.</status>",
        );
    } else if request.interrupted {
        result_string.push_str(
            "<status type=\"interrupted\">Command was canceled before completion. ASK THE USER what they would like to do next.</status>",
        );
    }

    result_string
}

pub struct BackgroundCommandStatusFacts {
    pub exit_code: Option<i32>,
    pub timed_out: bool,
    pub interrupted: bool,
}

pub struct BackgroundCommandDeliveryTextRequest<'a> {
    pub command: &'a str,
    pub terminal_session_id: &'a str,
    pub working_directory: &'a str,
    pub status: BackgroundCommandStatusFacts,
    pub output_file_reference: &'a str,
    pub output_persist_error: Option<&'a str>,
}

pub fn format_background_command_delivery_text(
    request: BackgroundCommandDeliveryTextRequest<'_>,
) -> String {
    let (status, summary) = if request.status.timed_out {
        ("timeout", "Background Bash command timed out.")
    } else if request.status.interrupted {
        ("interrupted", "Background Bash command was interrupted.")
    } else if request.status.exit_code == Some(0) {
        (
            "completed",
            "Background Bash command completed successfully.",
        )
    } else {
        (
            "failed",
            "Background Bash command completed with a non-zero exit code.",
        )
    };
    let exit_code_attr = request
        .status
        .exit_code
        .map(|code| format!(" exit_code=\"{}\"", code))
        .unwrap_or_default();
    let persistence_line = request.output_persist_error.map_or_else(
        || {
            format!(
                "Full output was saved to: {}",
                request.output_file_reference
            )
        },
        |error| {
            format!(
                "Output persistence encountered an error while writing {}: {}",
                request.output_file_reference, error
            )
        },
    );

    format!(
        "{summary}\n<background_command status=\"{status}\" terminal_session_id=\"{terminal_session_id}\"{exit_code_attr}>\nCommand: {command}\nWorking directory: {working_directory}\n{persistence_line}\n</background_command>",
        terminal_session_id = request.terminal_session_id,
        command = request.command,
        working_directory = request.working_directory,
    )
}

pub fn format_background_command_display_text(status: BackgroundCommandStatusFacts) -> String {
    if status.timed_out {
        "Background Bash command timed out.".to_string()
    } else if status.interrupted {
        "Background Bash command was interrupted.".to_string()
    } else if status.exit_code == Some(0) {
        "Background Bash command completed successfully.".to_string()
    } else {
        "Background Bash command completed with a non-zero exit code.".to_string()
    }
}

pub struct BackgroundCommandErrorTextRequest<'a> {
    pub command: &'a str,
    pub terminal_session_id: &'a str,
    pub working_directory: &'a str,
    pub output_file_reference: &'a str,
    pub error: &'a str,
    pub output_persist_error: Option<&'a str>,
}

pub fn format_background_command_error_text(
    request: BackgroundCommandErrorTextRequest<'_>,
) -> String {
    let persistence_line = request.output_persist_error.map_or_else(
        || {
            format!(
                "Any captured output was saved to: {}",
                request.output_file_reference
            )
        },
        |persist_error| {
            format!(
                "Output persistence encountered an error while writing {}: {}",
                request.output_file_reference, persist_error
            )
        },
    );

    format!(
        "Background Bash command failed before producing a final completion result.\n<background_command status=\"error\" terminal_session_id=\"{terminal_session_id}\">\nCommand: {command}\nWorking directory: {working_directory}\n{persistence_line}\nError: {error}\n</background_command>",
        terminal_session_id = request.terminal_session_id,
        command = request.command,
        working_directory = request.working_directory,
        error = request.error,
    )
}

pub fn format_background_command_error_display_text() -> String {
    "Background Bash command failed before producing a final completion result.".to_string()
}
