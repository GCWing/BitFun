use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum SessionKind {
    #[default]
    Standard,
    Subagent,
    EphemeralChild,
}

/// Whether a persisted subagent session may accept another delegated turn.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum SessionContinuationPolicy {
    #[default]
    Reusable,
    FreshOnly,
}

/// Whether model reconciliation may replace the session's selected model.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum SessionModelBindingPolicy {
    #[default]
    Mutable,
    ApprovedImmutable,
}

pub fn validate_session_id(session_id: &str) -> Result<(), String> {
    if session_id.is_empty() {
        return Err("session_id cannot be empty".to_string());
    }
    if session_id == "." || session_id == ".." {
        return Err("session_id cannot be '.' or '..'".to_string());
    }
    if session_id.contains('/') || session_id.contains('\\') {
        return Err("session_id cannot contain path separators".to_string());
    }
    if session_id.chars().any(char::is_control) {
        return Err("session_id cannot contain control characters".to_string());
    }
    let bytes = session_id.as_bytes();
    if bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' {
        return Err("session_id cannot use a drive-relative path prefix".to_string());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::validate_session_id;

    #[test]
    fn session_ids_are_single_safe_path_components() {
        for valid in [
            "session-1",
            "session_2",
            "A9",
            "id with space",
            "会话-1",
            "miniapp-customize:builtin-gomoku:1",
        ] {
            validate_session_id(valid).expect("valid session id");
        }
        for invalid in [
            "",
            ".",
            "..",
            "../outside",
            "a/b",
            "a\\b",
            "C:\\outside",
            "C:outside",
            "miniapp-customize:../outside:1",
            "control\0character",
        ] {
            assert!(
                validate_session_id(invalid).is_err(),
                "unsafe session id must fail: {invalid}"
            );
        }
    }
}
