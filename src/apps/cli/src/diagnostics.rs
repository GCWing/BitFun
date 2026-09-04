//! Exec-mode exit diagnostics for automated runners.

use std::path::Path;

use openbitfun_agent_runtime::sdk::{PortErrorKind, RuntimeError};
use openbitfun_agent_runtime_ipc::{RuntimeIpcClientError, RuntimeIpcErrorCode};

pub(crate) const EXIT_LINE_PREFIX: &str = "OPENBITFUN_EXIT: ";
pub(crate) const DETAIL_MAX_LEN: usize = 500;
pub(crate) const SESSION_IN_USE_ERROR_CODE: &str = "session_in_use";
pub(crate) const OUTCOME_UNKNOWN_ERROR_CODE: &str = "outcome_unknown";
pub(crate) const SESSION_IN_USE_USER_MESSAGE: &str =
    "This session is open in another OpenBitFun instance. Close it there and retry.";

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

pub(crate) fn cli_error_code(error: &anyhow::Error) -> Option<&'static str> {
    let session_in_use = error.chain().any(|cause| {
        matches!(
            cause.downcast_ref::<RuntimeError>(),
            Some(RuntimeError::Port(port_error))
                if port_error.kind == PortErrorKind::SessionInUse
        ) || matches!(
            cause.downcast_ref::<RuntimeIpcClientError>(),
            Some(RuntimeIpcClientError::Remote(remote))
                if remote.code == RuntimeIpcErrorCode::SessionInUse
        )
    });
    session_in_use.then_some(SESSION_IN_USE_ERROR_CODE)
}

pub(crate) fn user_facing_error_message(error: &anyhow::Error) -> String {
    match cli_error_code(error) {
        Some(SESSION_IN_USE_ERROR_CODE) => SESSION_IN_USE_USER_MESSAGE.to_string(),
        _ => error.to_string(),
    }
}

pub(crate) fn with_session_conflict_help(error: anyhow::Error) -> anyhow::Error {
    if cli_error_code(&error).is_some() {
        error.context(SESSION_IN_USE_USER_MESSAGE)
    } else {
        error
    }
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
    use openbitfun_agent_runtime::sdk::{PortError, PortErrorKind, RuntimeError};
    use openbitfun_agent_runtime_ipc::{
        RuntimeIpcClientError, RuntimeIpcError, RuntimeIpcErrorCode,
    };

    #[test]
    fn format_exit_line_uses_stable_prefix_and_kind() {
        let line = format_exit_line(ExitKind::DialogTurnFailed, "429 Too Many Requests");
        assert_eq!(
            line,
            "OPENBITFUN_EXIT: dialog_turn_failed: 429 Too Many Requests"
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

    #[test]
    fn embedded_session_conflict_keeps_a_stable_code_and_actionable_message() {
        let error = anyhow::Error::new(RuntimeError::Port(PortError::new(
            PortErrorKind::SessionInUse,
            "Session is already open for writing: session-1",
        )));

        assert_eq!(cli_error_code(&error), Some(SESSION_IN_USE_ERROR_CODE));
        assert_eq!(
            user_facing_error_message(&error),
            SESSION_IN_USE_USER_MESSAGE
        );
    }

    #[test]
    fn shared_session_conflict_uses_the_same_cli_projection() {
        let error = anyhow::Error::new(RuntimeIpcClientError::Remote(RuntimeIpcError {
            code: RuntimeIpcErrorCode::SessionInUse,
            message: "Session is already open for writing: session-1".to_string(),
        }));

        assert_eq!(cli_error_code(&error), Some(SESSION_IN_USE_ERROR_CODE));
        assert_eq!(
            user_facing_error_message(&error),
            SESSION_IN_USE_USER_MESSAGE
        );
    }

    #[test]
    fn unrelated_errors_keep_their_original_message() {
        let error = anyhow::anyhow!("provider unavailable");

        assert_eq!(cli_error_code(&error), None);
        assert_eq!(user_facing_error_message(&error), "provider unavailable");
    }
}
