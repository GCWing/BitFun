//! PKCE (RFC 7636) helpers shared by the browser OAuth flows.

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use sha2::{Digest, Sha256};
use uuid::Uuid;

/// Unreserved characters allowed in a PKCE `code_verifier`.
const VERIFIER_CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-._~";

/// A PKCE verifier/challenge pair.
#[derive(Debug, Clone)]
pub(crate) struct Pkce {
    pub verifier: String,
    pub challenge: String,
}

impl Pkce {
    /// Generates a 43-character verifier and its S256 challenge.
    pub(crate) fn generate() -> Self {
        let bytes = random_bytes(43);
        let verifier: String = bytes
            .iter()
            .map(|b| VERIFIER_CHARS[(*b as usize) % VERIFIER_CHARS.len()] as char)
            .collect();
        let mut hasher = Sha256::new();
        hasher.update(verifier.as_bytes());
        let challenge = URL_SAFE_NO_PAD.encode(hasher.finalize());
        Self {
            verifier,
            challenge,
        }
    }
}

/// Generates a random URL-safe state string.
pub(crate) fn random_state() -> String {
    URL_SAFE_NO_PAD.encode(random_bytes(32))
}

/// Produces `len` random bytes sourced from v4 UUIDs.
fn random_bytes(len: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(len);
    while out.len() < len {
        out.extend_from_slice(Uuid::new_v4().as_bytes());
    }
    out.truncate(len);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generates_non_empty_verifier_and_challenge() {
        let pkce = Pkce::generate();
        assert_eq!(pkce.verifier.len(), 43);
        assert!(!pkce.challenge.is_empty());
        assert!(pkce.verifier.bytes().all(|b| VERIFIER_CHARS.contains(&b)));
    }

    #[test]
    fn state_is_non_empty_and_random() {
        let a = random_state();
        let b = random_state();
        assert!(!a.is_empty());
        assert_ne!(a, b);
    }
}
