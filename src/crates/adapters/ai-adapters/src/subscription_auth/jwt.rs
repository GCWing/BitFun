//! Minimal JWT claim decoding.
//!
//! We only read the payload (no signature verification): the tokens are issued
//! to this client by the provider and stored locally, so we simply extract
//! metadata such as `exp`, `email`, and the ChatGPT account id.

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use serde_json::Value;

/// Decodes the JWT payload segment into a JSON value.
pub(crate) fn decode_claims(token: &str) -> Option<Value> {
    let mut parts = token.splitn(3, '.');
    let _header = parts.next()?;
    let payload = parts.next()?;
    let bytes = URL_SAFE_NO_PAD.decode(payload).ok()?;
    serde_json::from_slice::<Value>(&bytes).ok()
}

/// Returns the `email` claim if present.
pub(crate) fn email(token: &str) -> Option<String> {
    decode_claims(token)?
        .get("email")?
        .as_str()
        .map(str::to_string)
}

/// Extracts the ChatGPT account id from a Codex id/access token, mirroring
/// OpenCode's `extractAccountIdFromClaims`.
pub(crate) fn chatgpt_account_id(token: &str) -> Option<String> {
    let claims = decode_claims(token)?;
    if let Some(id) = claims.get("chatgpt_account_id").and_then(Value::as_str) {
        return Some(id.to_string());
    }
    if let Some(id) = claims
        .get("https://api.openai.com/auth")
        .and_then(|auth| auth.get("chatgpt_account_id"))
        .and_then(Value::as_str)
    {
        return Some(id.to_string());
    }
    claims
        .get("organizations")
        .and_then(Value::as_array)
        .and_then(|orgs| orgs.first())
        .and_then(|org| org.get("id"))
        .and_then(Value::as_str)
        .map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};

    fn make_token(payload: serde_json::Value) -> String {
        let header = URL_SAFE_NO_PAD.encode(b"{\"alg\":\"none\"}");
        let body = URL_SAFE_NO_PAD.encode(serde_json::to_vec(&payload).unwrap());
        format!("{header}.{body}.sig")
    }

    #[test]
    fn parses_email_and_account_id() {
        let token = make_token(serde_json::json!({
            "exp": 1_800_000_000i64,
            "email": "user@example.com",
            "https://api.openai.com/auth": { "chatgpt_account_id": "acct_123" }
        }));
        assert_eq!(email(&token).as_deref(), Some("user@example.com"));
        assert_eq!(chatgpt_account_id(&token).as_deref(), Some("acct_123"));
    }

    #[test]
    fn returns_none_for_malformed_token() {
        assert_eq!(decode_claims("not-a-jwt"), None);
    }
}
