//! Stable App Server error semantics.

use serde::{Deserialize, Serialize};

pub const UNSUPPORTED_CODE: i64 = -32001;
pub const SESSION_IN_USE_CODE: i64 = -32002;
pub const STALE_REVISION_CODE: i64 = -32003;
pub const OUTCOME_UNKNOWN_CODE: i64 = -32004;
pub const STREAM_INVALIDATED_CODE: i64 = -32005;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AppServerErrorKind {
    Unsupported,
    SessionInUse,
    StaleRevision,
    OutcomeUnknown,
    StreamInvalidated,
    InvalidRequest,
    Internal,
}

impl AppServerErrorKind {
    pub const fn json_rpc_code(self) -> i64 {
        match self {
            Self::Unsupported => UNSUPPORTED_CODE,
            Self::SessionInUse => SESSION_IN_USE_CODE,
            Self::StaleRevision => STALE_REVISION_CODE,
            Self::OutcomeUnknown => OUTCOME_UNKNOWN_CODE,
            Self::StreamInvalidated => STREAM_INVALIDATED_CODE,
            Self::InvalidRequest => -32602,
            Self::Internal => -32603,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppServerErrorData {
    pub kind: AppServerErrorKind,
    #[serde(default)]
    pub retryable: bool,
    #[serde(default)]
    pub outcome_unknown: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capability: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::{AppServerErrorData, AppServerErrorKind, OUTCOME_UNKNOWN_CODE};

    #[test]
    fn outcome_unknown_has_a_stable_code_and_explicit_retry_semantics() {
        assert_eq!(
            AppServerErrorKind::OutcomeUnknown.json_rpc_code(),
            OUTCOME_UNKNOWN_CODE
        );
        let data = AppServerErrorData {
            kind: AppServerErrorKind::OutcomeUnknown,
            retryable: false,
            outcome_unknown: true,
            capability: None,
            request_id: Some("request-1".to_string()),
        };
        let value = serde_json::to_value(data).expect("serialize error data");
        assert_eq!(value["outcomeUnknown"], true);
        assert_eq!(value["retryable"], false);
    }
}
