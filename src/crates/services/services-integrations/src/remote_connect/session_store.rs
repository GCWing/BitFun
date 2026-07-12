//! Machine-bound persistent session store.
//!
//! Saves the full `AccountSession` (token + master_key + user_id) and
//! relay URL to disk, encrypted with a machine-derived key so the file
//! is useless if copied to another machine.  This lets Desktop / CLI
//! restart without requiring a fresh password entry.
//!
//! File location: `~/.bitfun/account_session.enc`
//! Format: base64(nonce || ciphertext) where the plaintext is a JSON
//! payload `{ token, user_id, master_key_b64, relay_url }`.

use std::path::PathBuf;

use aes_gcm::aead::{Aead, KeyInit, OsRng};
use aes_gcm::{Aes256Gcm, Nonce};
use anyhow::{anyhow, Context, Result};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const NONCE_SIZE: usize = 12;

/// The on-disk JSON payload (plaintext before encryption).
#[derive(Serialize, Deserialize)]
struct SessionPayload {
    token: String,
    user_id: String,
    /// Base64-encoded 32-byte master key.
    master_key_b64: String,
    relay_url: String,
}

/// Resolve the persistent session file path.
fn session_file_path() -> Result<PathBuf> {
    let home = dirs::home_dir().ok_or_else(|| anyhow!("cannot determine home directory"))?;
    Ok(home.join(".bitfun").join("account_session.enc"))
}

/// Derive a machine-bound AES-256 key from stable machine identifiers.
///
/// Combines hostname, OS username, OS type, and platform constant into
/// SHA-256 to produce a 32-byte key.  The file is useless on a different
/// machine (different hostname / username combination).
fn derive_machine_key() -> [u8; 32] {
    let mut hasher = Sha256::new();

    // Hostname / machine name
    if let Some(hostname) = hostname_string() {
        hasher.update(hostname.as_bytes());
    }
    hasher.update(b"|");

    // OS username
    if let Some(username) = username_string() {
        hasher.update(username.as_bytes());
    }
    hasher.update(b"|");

    // OS type (linux / macos / windows)
    hasher.update(std::env::consts::OS.as_bytes());
    hasher.update(b"|");

    // Application-specific constant to domain-separate the key
    hasher.update(b"BitFun::session_store::v1");

    let result = hasher.finalize();
    let mut key = [0u8; 32];
    key.copy_from_slice(&result);
    key
}

/// Best-effort hostname retrieval (cross-platform).
fn hostname_string() -> Option<String> {
    // `hostname::get()` from the `hostname` crate would be cleaner, but
    // to avoid adding a new dependency we use environment / std methods.
    //
    // On Unix: read /etc/hostname or call `gethostname` via libc.
    // On Windows: `%COMPUTERNAME%`.
    if cfg!(target_os = "windows") {
        std::env::var("COMPUTERNAME").ok()
    } else {
        // Try `hostname` command as a portable fallback.
        std::process::Command::new("hostname")
            .output()
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .map(|s| s.trim().to_string())
            .or_else(|| {
                std::fs::read_to_string("/etc/hostname")
                    .ok()
                    .map(|s| s.trim().to_string())
            })
    }
}

/// Best-effort current username retrieval (cross-platform).
fn username_string() -> Option<String> {
    if cfg!(target_os = "windows") {
        std::env::var("USERNAME").or_else(|_| std::env::var("USER")).ok()
    } else {
        std::env::var("USER")
            .ok()
            .or_else(|| {
                std::process::Command::new("whoami")
                    .output()
                    .ok()
                    .and_then(|o| String::from_utf8(o.stdout).ok())
                    .map(|s| s.trim().to_string())
            })
    }
}

// ── Public API ──────────────────────────────────────────────────────────

/// Persist the session (token, master_key, user_id, relay_url) to disk,
/// encrypted with the machine-bound key.
pub fn save_session(
    token: &str,
    user_id: &str,
    master_key: &[u8; 32],
    relay_url: &str,
) -> Result<()> {
    let payload = SessionPayload {
        token: token.to_string(),
        user_id: user_id.to_string(),
        master_key_b64: BASE64.encode(master_key),
        relay_url: relay_url.to_string(),
    };
    let json = serde_json::to_string(&payload).context("serialize session payload")?;

    let key = derive_machine_key();
    let cipher =
        Aes256Gcm::new_from_slice(&key).map_err(|e| anyhow!("cipher init: {e}"))?;

    let mut nonce_bytes = [0u8; NONCE_SIZE];
    OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ciphertext = cipher
        .encrypt(nonce, json.as_bytes())
        .map_err(|e| anyhow!("encrypt session: {e}"))?;

    // Pack: nonce (12 bytes) || ciphertext, then base64-encode the whole thing.
    let mut packed = Vec::with_capacity(NONCE_SIZE + ciphertext.len());
    packed.extend_from_slice(&nonce_bytes);
    packed.extend_from_slice(&ciphertext);
    let encoded = BASE64.encode(&packed);

    let path = session_file_path()?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).context("create session dir")?;
    }
    std::fs::write(&path, encoded).context("write session file")?;

    log::debug!("Session persisted to {:?}", path);
    Ok(())
}

/// Load and decrypt the session from disk.
/// Returns `Ok(None)` if the file doesn't exist (not an error).
pub fn load_session() -> Result<Option<(String, String, [u8; 32], String)>> {
    let path = session_file_path()?;
    let encoded = match std::fs::read_to_string(&path) {
        Ok(data) => data,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(anyhow!("read session file: {e}").context(path.to_string_lossy().to_string())),
    };

    let packed = BASE64
        .decode(encoded.trim())
        .map_err(|e| anyhow!("base64 decode session: {e}"))?;
    if packed.len() < NONCE_SIZE {
        return Err(anyhow!("session file too short"));
    }

    let (nonce_bytes, ciphertext) = packed.split_at(NONCE_SIZE);
    let nonce_arr: [u8; NONCE_SIZE] = nonce_bytes
        .try_into()
        .map_err(|_| anyhow!("nonce conversion"))?;

    let key = derive_machine_key();
    let cipher =
        Aes256Gcm::new_from_slice(&key).map_err(|e| anyhow!("cipher init: {e}"))?;
    let plaintext = cipher
        .decrypt(Nonce::from_slice(&nonce_arr), ciphertext)
        .map_err(|e| anyhow!("decrypt session: {e}"))?;

    let payload: SessionPayload =
        serde_json::from_slice(&plaintext).context("deserialize session payload")?;

    let master_key_bytes = BASE64
        .decode(&payload.master_key_b64)
        .map_err(|e| anyhow!("decode master key: {e}"))?;
    let mut master_key = [0u8; 32];
    if master_key_bytes.len() != 32 {
        return Err(anyhow!("invalid master key length"));
    }
    master_key.copy_from_slice(&master_key_bytes);

    Ok(Some((
        payload.token,
        payload.user_id,
        master_key,
        payload.relay_url,
    )))
}

/// Remove the persisted session file (called on logout).
pub fn clear_session() {
    if let Ok(path) = session_file_path() {
        let _ = std::fs::remove_file(&path);
    }
}
