use crate::config::NotificationMethod;
use std::io::{self, Write};

const MAX_NOTIFICATION_CHARS: usize = 120;

pub(crate) fn notify(
    writer: &mut impl Write,
    method: NotificationMethod,
    message: &str,
) -> io::Result<()> {
    let sequence = notification_sequence(method, message, supports_osc9(), in_tmux());
    writer.write_all(&sequence)?;
    writer.flush()
}

fn supports_osc9() -> bool {
    let term_program = std::env::var("TERM_PROGRAM")
        .unwrap_or_default()
        .to_ascii_lowercase();
    let term = std::env::var("TERM")
        .unwrap_or_default()
        .to_ascii_lowercase();
    ["ghostty", "iterm", "kitty", "warp", "wezterm"]
        .iter()
        .any(|candidate| term_program.contains(candidate) || term.contains(candidate))
}

fn in_tmux() -> bool {
    std::env::var("TMUX")
        .ok()
        .is_some_and(|value| !value.trim().is_empty())
}

fn notification_sequence(
    method: NotificationMethod,
    message: &str,
    osc9_supported: bool,
    tmux: bool,
) -> Vec<u8> {
    let method = match method {
        NotificationMethod::Auto if osc9_supported => NotificationMethod::Osc9,
        NotificationMethod::Auto => NotificationMethod::Bel,
        explicit => explicit,
    };
    if method == NotificationMethod::Bel {
        return vec![0x07];
    }

    let message = sanitize_message(message);
    if tmux {
        format!("\x1bPtmux;\x1b\x1b]9;{message}\x07\x1b\\").into_bytes()
    } else {
        format!("\x1b]9;{message}\x07").into_bytes()
    }
}

fn sanitize_message(message: &str) -> String {
    let normalized = message
        .chars()
        .take(MAX_NOTIFICATION_CHARS)
        .map(|character| {
            if character.is_control() {
                ' '
            } else {
                character
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    if normalized.is_empty() {
        "OpenBitFun needs attention".to_string()
    } else {
        normalized
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::NotificationMethod;

    #[test]
    fn auto_uses_osc9_only_when_the_terminal_is_known_to_support_it() {
        assert_eq!(
            notification_sequence(NotificationMethod::Auto, "Done", true, false),
            b"\x1b]9;Done\x07"
        );
        assert_eq!(
            notification_sequence(NotificationMethod::Auto, "Done", false, false),
            b"\x07"
        );
    }

    #[test]
    fn osc9_sanitizes_control_sequences_and_wraps_tmux_passthrough() {
        assert_eq!(
            notification_sequence(
                NotificationMethod::Osc9,
                "Permission\x1b[31m\nrequired",
                false,
                true,
            ),
            b"\x1bPtmux;\x1b\x1b]9;Permission [31m required\x07\x1b\\"
        );
    }

    #[test]
    fn empty_notification_messages_keep_a_bounded_safe_fallback() {
        let sequence = notification_sequence(NotificationMethod::Osc9, "\n\t", true, false);
        assert_eq!(sequence, b"\x1b]9;OpenBitFun needs attention\x07");
        assert!(
            notification_sequence(NotificationMethod::Osc9, &"x".repeat(1_000), true, false,).len()
                < 160
        );
    }
}
