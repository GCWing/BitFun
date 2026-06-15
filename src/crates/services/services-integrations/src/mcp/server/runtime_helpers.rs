//! MCP server runtime helper contracts.

use std::collections::HashMap;

const AUTHORIZATION_KEYS: [&str; 3] = ["Authorization", "authorization", "AUTHORIZATION"];

pub fn is_mcp_auth_error_message(message: &str) -> bool {
    let msg = message.to_ascii_lowercase();
    let patterns = [
        "unauthorized",
        "forbidden",
        "auth required",
        "authorization required",
        "authentication required",
        "authentication failed",
        "oauth authorization required",
        "oauth token refresh failed",
        "token refresh failed",
        "www-authenticate",
        "invalid token",
        "token expired",
        "access token expired",
        "refresh token",
        "session expired",
        "status code: 401",
        "status code: 403",
        " 401 ",
        " 403 ",
    ];
    patterns.iter().any(|p| msg.contains(p))
}

pub fn merge_mcp_remote_headers(
    headers: &HashMap<String, String>,
    env: &HashMap<String, String>,
) -> HashMap<String, String> {
    let mut merged_headers = headers.clone();
    if AUTHORIZATION_KEYS
        .iter()
        .all(|key| !merged_headers.contains_key(*key))
    {
        // Backward compatibility: older BitFun configs store `Authorization` under `env`.
        if let Some(value) = AUTHORIZATION_KEYS.iter().find_map(|key| env.get(*key)) {
            merged_headers.insert("Authorization".to_string(), value.clone());
        }
    }

    merged_headers
}
