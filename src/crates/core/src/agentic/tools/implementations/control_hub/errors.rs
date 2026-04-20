//! Stable, machine-readable error codes returned inside the ControlHub
//! `error.code` field. Models can branch on these codes deterministically
//! instead of scraping free-form English error text.
//!
//! New codes MUST be additive — never repurpose an existing code.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ErrorCode {
    /// `domain` / `action` pair is not implemented or unknown.
    UnknownDomain,
    UnknownAction,
    /// Required parameter missing or wrong type.
    InvalidParams,
    /// Capability not available in this build / OS / runtime (e.g. desktop
    /// host absent on the server runtime, browser CDP not installed).
    NotAvailable,
    /// OS-level permission is required (e.g. macOS Accessibility).
    PermissionDenied,
    /// Operation timed out.
    Timeout,
    /// A target (DOM node, AX element, OCR text, app, page, file…) was not found.
    NotFound,
    /// Multiple candidates matched but the caller did not disambiguate.
    Ambiguous,
    /// A cached element / tab / screenshot / @ref reference is no longer valid;
    /// the model must re-acquire it (re-snapshot, re-screenshot, re-list).
    StaleRef,
    /// A safety / readiness guard refused the action (e.g. Computer Use's
    /// "fresh screenshot required before click" guard).
    GuardRejected,
    /// The targeted display / monitor was wrong or could not be resolved.
    WrongDisplay,
    /// A targeted browser tab / page could not be resolved or addressed.
    WrongTab,
    /// Backend reported an internal error not classified above.
    Internal,
    /// Frontend (SelfControl / app domain) reported an error during execution.
    FrontendError,
    /// The action requires a session / handle (e.g. `terminal_session_id`,
    /// `tab_handle`) that the caller did not provide.
    MissingSession,
}

impl ErrorCode {
    pub fn as_str(self) -> &'static str {
        match self {
            ErrorCode::UnknownDomain => "UNKNOWN_DOMAIN",
            ErrorCode::UnknownAction => "UNKNOWN_ACTION",
            ErrorCode::InvalidParams => "INVALID_PARAMS",
            ErrorCode::NotAvailable => "NOT_AVAILABLE",
            ErrorCode::PermissionDenied => "PERMISSION_DENIED",
            ErrorCode::Timeout => "TIMEOUT",
            ErrorCode::NotFound => "NOT_FOUND",
            ErrorCode::Ambiguous => "AMBIGUOUS",
            ErrorCode::StaleRef => "STALE_REF",
            ErrorCode::GuardRejected => "GUARD_REJECTED",
            ErrorCode::WrongDisplay => "WRONG_DISPLAY",
            ErrorCode::WrongTab => "WRONG_TAB",
            ErrorCode::Internal => "INTERNAL",
            ErrorCode::FrontendError => "FRONTEND_ERROR",
            ErrorCode::MissingSession => "MISSING_SESSION",
        }
    }

    /// Parse a wire-format error code (e.g. `"NOT_FOUND"`) back into the
    /// enum. Used by `ControlHub` to recover the structured code from
    /// frontend (SelfControl) errors that arrive as `[CODE] message` strings.
    /// Case-insensitive; unknown codes return `None`.
    #[allow(clippy::should_implement_trait)] // we want an Option, not a Result
    pub fn from_str(s: &str) -> Option<Self> {
        let s = s.trim().to_ascii_uppercase();
        Some(match s.as_str() {
            "UNKNOWN_DOMAIN" => Self::UnknownDomain,
            "UNKNOWN_ACTION" => Self::UnknownAction,
            "INVALID_PARAMS" => Self::InvalidParams,
            "NOT_AVAILABLE" => Self::NotAvailable,
            "PERMISSION_DENIED" => Self::PermissionDenied,
            "TIMEOUT" => Self::Timeout,
            "NOT_FOUND" => Self::NotFound,
            "AMBIGUOUS" => Self::Ambiguous,
            "STALE_REF" => Self::StaleRef,
            "GUARD_REJECTED" => Self::GuardRejected,
            "WRONG_DISPLAY" => Self::WrongDisplay,
            "WRONG_TAB" => Self::WrongTab,
            "INTERNAL" => Self::Internal,
            "FRONTEND_ERROR" => Self::FrontendError,
            "MISSING_SESSION" => Self::MissingSession,
            _ => return None,
        })
    }
}
