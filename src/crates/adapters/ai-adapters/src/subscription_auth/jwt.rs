//! Minimal JWT claim decoding.
//!
//! We only read the payload (no signature verification): the tokens are issued
//! to this client by the provider and stored locally, so we simply extract
//! metadata such as `exp`, `email`, subject, and the ChatGPT account id.

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

/// Returns the stable `sub` claim if present.
pub(crate) fn subject(token: &str) -> Option<String> {
    decode_claims(token)?
        .get("sub")?
        .as_str()
        .map(str::to_string)
}

/// Checks the unverified JWT `exp` claim for proactive refresh only. Opaque
/// access tokens return false and continue using the provider's stored expiry.
pub(crate) fn expires_within(token: &str, now_ms: i64, skew_ms: i64) -> bool {
    decode_claims(token)
        .and_then(|claims| claims.get("exp").and_then(Value::as_i64))
        .is_some_and(|expires| expires.saturating_mul(1000) <= now_ms.saturating_add(skew_ms))
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

/// Extracts the optional regional inference residency advertised by ChatGPT.
/// `no_constraint` means the credential does not request a residency header.
pub(crate) fn chatgpt_compute_residency(token: &str) -> Option<String> {
    let claims = decode_claims(token)?;
    let residency = claims
        .get("https://api.openai.com/auth")
        .and_then(|auth| auth.get("chatgpt_compute_residency"))
        .and_then(Value::as_str)
        .or_else(|| {
            claims
                .get("chatgpt_compute_residency")
                .and_then(Value::as_str)
        })?;
    if residency.trim().is_empty() || residency == "no_constraint" {
        None
    } else {
        Some(residency.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;

    fn make_token(payload: serde_json::Value) -> String {
        let header = URL_SAFE_NO_PAD.encode(b"{\"alg\":\"none\"}");
        let body = URL_SAFE_NO_PAD.encode(serde_json::to_vec(&payload).unwrap());
        format!("{header}.{body}.sig")
    }

    #[test]
    fn parses_email_account_id_and_compute_residency() {
        let token = make_token(serde_json::json!({
            "exp": 1_800_000_000i64,
            "sub": "user_123",
            "email": "user@example.com",
            "https://api.openai.com/auth": {
                "chatgpt_account_id": "acct_123",
                "chatgpt_compute_residency": "us"
            }
        }));
        assert_eq!(email(&token).as_deref(), Some("user@example.com"));
        assert_eq!(subject(&token).as_deref(), Some("user_123"));
        assert_eq!(chatgpt_account_id(&token).as_deref(), Some("acct_123"));
        assert_eq!(chatgpt_compute_residency(&token).as_deref(), Some("us"));
    }

    #[test]
    fn omits_unconstrained_compute_residency() {
        let token = make_token(serde_json::json!({
            "chatgpt_compute_residency": "no_constraint"
        }));
        assert_eq!(chatgpt_compute_residency(&token), None);
    }

    #[test]
    fn returns_none_for_malformed_token() {
        assert_eq!(decode_claims("not-a-jwt"), None);
    }

    #[test]
    fn checks_jwt_expiry_without_treating_opaque_tokens_as_expired() {
        let token = make_token(serde_json::json!({ "exp": 1_800_000_000i64 }));
        assert!(expires_within(&token, 1_799_999_900_000, 120_000));
        assert!(!expires_within(&token, 1_799_999_000_000, 120_000));
        assert!(!expires_within("opaque-token", 1_799_999_900_000, 120_000));
    }
}
