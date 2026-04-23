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
    /// AX-first desktop: the targeted application could not be resolved by
    /// the supplied selector (name / bundle_id / pid). Distinct from
    /// `NOT_FOUND` (which means a sub-element inside an app is missing).
    AppNotFound,
    /// AX-first desktop: a node `idx` provided by the caller is no longer
    /// valid because the host has re-dumped the tree since the snapshot
    /// the caller saw. Re-acquire via `desktop.get_app_state` and retry.
    AxNodeStale,
    /// AX-first desktop: this host cannot inject input events into the
    /// target app without stealing user focus (e.g. macOS without
    /// Accessibility permission, or non-macOS where the PID-event path is
    /// not yet wired). Callers can fall back to the foreground
    /// `desktop.click` path or escalate permissions.
    BackgroundInputUnavailable,
    /// AX-first desktop: the `node_idx` supplied to `click_element` /
    /// `locate_element` is no longer present in the cached snapshot
    /// (re-dump happened or window/state churned). Distinct from
    /// `AX_NODE_STALE` which is for `app_*` actions; same recovery —
    /// re-call `desktop.get_app_state` and reuse the new idx.
    AxIdxStale,
    /// AX-first desktop: this platform host does not support resolving
    /// elements by `node_idx` (currently linux/windows). Caller should
    /// fall back to `text_contains` / `title_contains` + `role_substring`.
    AxIdxNotSupported,
    /// `mouse_move(use_screen_coordinates=true)` got an `(x,y)` that
    /// does not lie on any visible display. Almost always means the model
    /// confused image-pixel coords with global screen coords.
    DesktopCoordOutOfDisplay,
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
            ErrorCode::AppNotFound => "APP_NOT_FOUND",
            ErrorCode::AxNodeStale => "AX_NODE_STALE",
            ErrorCode::BackgroundInputUnavailable => "BACKGROUND_INPUT_UNAVAILABLE",
            ErrorCode::AxIdxStale => "AX_IDX_STALE",
            ErrorCode::AxIdxNotSupported => "AX_IDX_NOT_SUPPORTED",
            ErrorCode::DesktopCoordOutOfDisplay => "DESKTOP_COORD_OUT_OF_DISPLAY",
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
            "APP_NOT_FOUND" => Self::AppNotFound,
            "AX_NODE_STALE" => Self::AxNodeStale,
            "BACKGROUND_INPUT_UNAVAILABLE" => Self::BackgroundInputUnavailable,
            "AX_IDX_STALE" => Self::AxIdxStale,
            "AX_IDX_NOT_SUPPORTED" => Self::AxIdxNotSupported,
            "DESKTOP_COORD_OUT_OF_DISPLAY" => Self::DesktopCoordOutOfDisplay,
            _ => return None,
        })
    }
}
