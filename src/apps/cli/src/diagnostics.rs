//! Exec-mode exit diagnostics for automated runners.

use std::path::Path;

pub(crate) const EXIT_LINE_PREFIX: &str = "BITFUN_EXIT: ";
pub(crate) const DETAIL_MAX_LEN: usize = 500;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ExitKind {
    SessionCreateFailed,
    SendMessageFailed,
    DialogTurnFailed,
    PermissionRejected,
    Cancelled,
    EventStreamFailed,
    SettlementTimedOut,
    SystemError,
    ExecError,
    PatchUnavailable,
    PatchWriteFailed,
}

impl ExitKind {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::SessionCreateFailed => "session_create_failed",
            Self::SendMessageFailed => "send_message_failed",
            Self::DialogTurnFailed => "dialog_turn_failed",
            Self::PermissionRejected => "permission_rejected",
            Self::Cancelled => "cancelled",
            Self::EventStreamFailed => "event_stream_failed",
            Self::SettlementTimedOut => "settlement_timed_out",
            Self::SystemError => "system_error",
            Self::ExecError => "exec_error",
            Self::PatchUnavailable => "patch_unavailable",
            Self::PatchWriteFailed => "patch_write_failed",
        }
    }
}

#[derive(Debug, Clone, Default)]
pub(crate) struct ExitContext<'a> {
    pub session_id: Option<&'a str>,
    pub turn_id: Option<&'a str>,
    pub agent_type: Option<&'a str>,
    pub workspace: Option<&'a Path>,
}

pub(crate) fn sanitize_exit_detail(detail: &str) -> String {
    let collapsed = detail.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.chars().count() <= DETAIL_MAX_LEN {
        return collapsed;
    }
    let truncated: String = collapsed.chars().take(DETAIL_MAX_LEN).collect();
    format!("{truncated}...")
}

pub(crate) fn format_exit_line(kind: ExitKind, detail: &str) -> String {
    format!(
        "{}{}: {}",
        EXIT_LINE_PREFIX,
        kind.as_str(),
        sanitize_exit_detail(detail)
    )
}

pub(crate) fn emit_exit_diagnostic(kind: ExitKind, detail: &str, ctx: &ExitContext<'_>) {
    eprintln!("{}", format_exit_line(kind, detail));
    tracing::error!(
        kind = kind.as_str(),
        session_id = ctx.session_id.unwrap_or("-"),
        turn_id = ctx.turn_id.unwrap_or("-"),
        agent_type = ctx.agent_type.unwrap_or("-"),
        workspace = ?ctx.workspace,
        detail = %detail,
        "exec exit diagnostic"
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_exit_line_uses_stable_prefix_and_kind() {
        let line = format_exit_line(ExitKind::DialogTurnFailed, "429 Too Many Requests");
        assert_eq!(
            line,
            "BITFUN_EXIT: dialog_turn_failed: 429 Too Many Requests"
        );
    }

    #[test]
    fn sanitize_exit_detail_collapses_whitespace_and_newlines() {
        let detail = "line one\nline two\t\tline three";
        assert_eq!(sanitize_exit_detail(detail), "line one line two line three");
    }

    #[test]
    fn sanitize_exit_detail_truncates_long_messages() {
        let detail = "x".repeat(DETAIL_MAX_LEN + 10);
        let sanitized = sanitize_exit_detail(&detail);
        assert!(sanitized.ends_with("..."));
        assert!(sanitized.chars().count() <= DETAIL_MAX_LEN + 3);
    }
}
