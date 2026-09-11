//! Device-pair message keys for GitHub-authenticated OpenBitFun Relay sessions.
//! Public keys come from the relay's same-account directory. Private keys are
//! generated and retained by each device. The relay wire envelope is unchanged.

use anyhow::{anyhow, Result};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use hkdf::Hkdf;
use sha2::Sha256;
use x25519_dalek::{PublicKey, StaticSecret};

pub const KDF_SALT: &[u8] = b"OpenBitFun Relay v1.0.0 device key";

pub fn generate_secret() -> [u8; 32] {
    StaticSecret::random_from_rng(rand::rngs::OsRng).to_bytes()
}

/// Derive a distinct bootstrap key that is stable for an idempotent request.
/// Knowledge of the target key does not reveal the provisioning device's key.
pub fn provisioning_secret(
    parent_secret: &[u8; 32],
    device_id: &str,
    request_id: &str,
) -> [u8; 32] {
    let mut secret = [0; 32];
    let info = format!("{device_id}:{request_id}");
    Hkdf::<Sha256>::new(
        Some(b"OpenBitFun device provisioning v1.0.0"),
        parent_secret,
    )
    .expand(info.as_bytes(), &mut secret)
    .expect("32-byte HKDF output is valid");
    secret
}

pub fn public_key(secret: &[u8; 32]) -> [u8; 32] {
    PublicKey::from(&StaticSecret::from(*secret)).to_bytes()
}

pub fn public_key_base64(secret: &[u8; 32]) -> String {
    BASE64.encode(public_key(secret))
}

/// X25519 + HKDF-SHA256. Ordering public keys makes the derivation symmetric,
/// while binding both identities and the protocol domain to the final AES key.
pub fn derive_message_key(secret: &[u8; 32], peer_public: &[u8; 32]) -> Result<[u8; 32]> {
    let local_public = public_key(secret);
    let shared = StaticSecret::from(*secret).diffie_hellman(&PublicKey::from(*peer_public));
    if !shared.was_contributory() {
        return Err(anyhow!("invalid peer public key"));
    }
    let (first, second) = if local_public < *peer_public {
        (&local_public, peer_public)
    } else {
        (peer_public, &local_public)
    };
    let mut info = [0u8; 64];
    info[..32].copy_from_slice(first);
    info[32..].copy_from_slice(second);
    let mut key = [0u8; 32];
    Hkdf::<Sha256>::new(Some(KDF_SALT), shared.as_bytes())
        .expand(&info, &mut key)
        .map_err(|_| anyhow!("device message key derivation failed"))?;
    Ok(key)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::remote_connect::encryption::{decrypt_from_base64, encrypt_to_base64};

    #[test]
    fn provisioning_keys_are_isolated_and_replayable() {
        let parent = [7; 32];
        let key = provisioning_secret(&parent, "target", "request");
        assert_ne!(key, parent);
        assert_eq!(key, provisioning_secret(&parent, "target", "request"));
        assert_ne!(key, provisioning_secret(&parent, "other", "request"));
        assert_ne!(key, provisioning_secret(&parent, "target", "other"));
    }

    #[test]
    fn messages_round_trip_between_independent_devices_only() {
        let alice = [7; 32];
        let bob = [11; 32];
        let charlie = [17; 32];
        let sending = derive_message_key(&alice, &public_key(&bob)).unwrap();
        let receiving = derive_message_key(&bob, &public_key(&alice)).unwrap();
        assert_eq!(sending, receiving);
        assert_eq!(
            sending
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>(),
            "6e8f5da837e91e9ddb09c5aa7dee229e731fc94499d29d10dcf5f1437193f56c"
        );
        let (data, nonce) = encrypt_to_base64(&sending, "remote approval").unwrap();
        assert_eq!(
            decrypt_from_base64(&receiving, &data, &nonce).unwrap(),
            "remote approval"
        );
        let unrelated = derive_message_key(&charlie, &public_key(&alice)).unwrap();
        assert!(decrypt_from_base64(&unrelated, &data, &nonce).is_err());
        assert!(derive_message_key(&alice, &[0; 32]).is_err());
        let mut low_order = [0; 32];
        low_order[0] = 1;
        assert!(derive_message_key(&alice, &low_order).is_err());
    }
}
