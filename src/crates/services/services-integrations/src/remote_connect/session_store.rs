//! Machine-bound persistent session store.
//!
//! Saves the full `AccountSession` (token + master_key + user_id) and relay
//! URL to disk, encrypted with a key that combines machine identity and a
//! random per-install secret. Secret files are owner-only on Unix and replaced
//! through a private temporary file. This lets Desktop / CLI restart without
//! requiring a fresh password entry while keeping copied session ciphertext
//! unusable without the separate install key.
//!
//! File location: `~/.bitfun/account_session.enc`
//! Format: base64(nonce || ciphertext) where the plaintext is a JSON
//! payload `{ token, user_id, master_key_b64, relay_url }`.

use std::path::PathBuf;
use std::{fs::OpenOptions, io::Write};

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
    /// Account-bound device id used when the session token was issued.
    /// Optional for backward compatibility with sessions saved before this field.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    device_id: Option<String>,
}

/// Resolve the persistent session file path.
fn session_file_path() -> Result<PathBuf> {
    let home = dirs::home_dir().ok_or_else(|| anyhow!("cannot determine home directory"))?;
    Ok(home.join(".bitfun").join("account_session.enc"))
}

fn session_key_file_path() -> Result<PathBuf> {
    let home = dirs::home_dir().ok_or_else(|| anyhow!("cannot determine home directory"))?;
    Ok(home.join(".bitfun").join("account_session.key"))
}

/// Atomically replace a secret-bearing file and restrict it to the current
/// OS user where Unix permission bits are available. The parent is also made
/// private because older releases may have created `~/.bitfun` under a broad
/// process umask.
fn write_private_file(path: &std::path::Path, contents: &[u8]) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| anyhow!("secret file has no parent directory"))?;
    std::fs::create_dir_all(parent).context("create private data directory")?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o700))
            .context("restrict private data directory permissions")?;
    }

    let mut random = [0u8; 8];
    OsRng.fill_bytes(&mut random);
    let suffix = random
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| anyhow!("secret file has an invalid name"))?;
    let temp_path = parent.join(format!(".{file_name}.{suffix}.tmp"));

    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let write_result = (|| -> Result<()> {
        let mut file = options
            .open(&temp_path)
            .context("create temporary secret file")?;
        file.write_all(contents)
            .context("write temporary secret file")?;
        file.sync_all().context("flush temporary secret file")?;
        drop(file);

        #[cfg(windows)]
        if path.exists() {
            std::fs::remove_file(path).context("replace existing secret file")?;
        }
        std::fs::rename(&temp_path, path).context("install private secret file")?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
                .context("restrict secret file permissions")?;
        }
        Ok(())
    })();
    if write_result.is_err() {
        let _ = std::fs::remove_file(&temp_path);
    }
    write_result
}

/// Derive a machine-bound AES-256 key from stable machine identifiers.
///
/// Combines hostname, OS username, OS type, and platform constant into
/// SHA-256 to produce a 32-byte key.  The file is useless on a different
/// machine (different hostname / username combination).
fn derive_legacy_machine_key() -> [u8; 32] {
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

fn derive_machine_key(local_secret: &[u8; 32]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(derive_legacy_machine_key());
    hasher.update(b"|BitFun::session_store::v2|");
    hasher.update(local_secret);
    let result = hasher.finalize();
    let mut key = [0u8; 32];
    key.copy_from_slice(&result);
    key
}

fn load_or_create_local_session_secret() -> Result<[u8; 32]> {
    let path = session_key_file_path()?;
    match std::fs::read(&path) {
        Ok(bytes) => {
            return bytes
                .try_into()
                .map_err(|_| anyhow!("local account session key has an invalid length"));
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(anyhow!("read local account session key: {error}")),
    }

    let mut secret = [0u8; 32];
    OsRng.fill_bytes(&mut secret);
    let parent = path
        .parent()
        .ok_or_else(|| anyhow!("session key has no parent directory"))?;
    std::fs::create_dir_all(parent).context("create session key directory")?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o700))
            .context("restrict session key directory permissions")?;
    }

    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    match options.open(&path) {
        Ok(mut file) => {
            file.write_all(&secret)
                .context("write local account session key")?;
            file.sync_all().context("flush local account session key")?;
            Ok(secret)
        }
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            let bytes = std::fs::read(&path).context("read concurrently created session key")?;
            bytes
                .try_into()
                .map_err(|_| anyhow!("local account session key has an invalid length"))
        }
        Err(error) => Err(anyhow!("create local account session key: {error}")),
    }
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
        std::env::var("USERNAME")
            .or_else(|_| std::env::var("USER"))
            .ok()
    } else {
        std::env::var("USER").ok().or_else(|| {
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
    save_session_with_device(token, user_id, master_key, relay_url, None)
}

/// Persist the session including the account-bound `device_id`.
pub fn save_session_with_device(
    token: &str,
    user_id: &str,
    master_key: &[u8; 32],
    relay_url: &str,
    device_id: Option<&str>,
) -> Result<()> {
    let payload = SessionPayload {
        token: token.to_string(),
        user_id: user_id.to_string(),
        master_key_b64: BASE64.encode(master_key),
        relay_url: relay_url.to_string(),
        device_id: device_id
            .map(str::trim)
            .filter(|id| !id.is_empty())
            .map(str::to_string),
    };
    let json = serde_json::to_string(&payload).context("serialize session payload")?;

    let local_secret = load_or_create_local_session_secret()?;
    let key = derive_machine_key(&local_secret);
    let cipher = Aes256Gcm::new_from_slice(&key).map_err(|e| anyhow!("cipher init: {e}"))?;

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
    write_private_file(&path, encoded.as_bytes()).context("write session file")?;

    log::debug!("Session persisted to {:?}", path);
    Ok(())
}

/// Loaded account session fields from disk.
#[derive(Debug, Clone)]
pub struct LoadedSession {
    pub token: String,
    pub user_id: String,
    pub master_key: [u8; 32],
    pub relay_url: String,
    pub device_id: Option<String>,
}

/// Load and decrypt the session from disk.
/// Returns `Ok(None)` if the file doesn't exist (not an error).
pub fn load_session() -> Result<Option<(String, String, [u8; 32], String)>> {
    Ok(load_session_detailed()?.map(|s| (s.token, s.user_id, s.master_key, s.relay_url)))
}

/// Load and decrypt the session, including optional account-bound `device_id`.
pub fn load_session_detailed() -> Result<Option<LoadedSession>> {
    let path = session_file_path()?;
    let encoded = match std::fs::read_to_string(&path) {
        Ok(data) => data,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => {
            return Err(
                anyhow!("read session file: {e}").context(path.to_string_lossy().to_string())
            )
        }
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

    let local_secret = load_or_create_local_session_secret()?;
    let key = derive_machine_key(&local_secret);
    let cipher = Aes256Gcm::new_from_slice(&key).map_err(|e| anyhow!("cipher init: {e}"))?;
    let (plaintext, used_legacy_key) =
        match cipher.decrypt(Nonce::from_slice(&nonce_arr), ciphertext) {
            Ok(plaintext) => (plaintext, false),
            Err(_) => {
                // Compatibility with v1 sessions. Successful legacy loads are
                // immediately re-encrypted with the per-install random secret.
                let legacy_key = derive_legacy_machine_key();
                let legacy_cipher = Aes256Gcm::new_from_slice(&legacy_key)
                    .map_err(|e| anyhow!("legacy cipher init: {e}"))?;
                let plaintext = legacy_cipher
                    .decrypt(Nonce::from_slice(&nonce_arr), ciphertext)
                    .map_err(|e| anyhow!("decrypt session: {e}"))?;
                (plaintext, true)
            }
        };

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

    let loaded = LoadedSession {
        token: payload.token,
        user_id: payload.user_id,
        master_key,
        relay_url: payload.relay_url,
        device_id: payload.device_id,
    };
    if used_legacy_key {
        if let Err(error) = save_session_with_device(
            &loaded.token,
            &loaded.user_id,
            &loaded.master_key,
            &loaded.relay_url,
            loaded.device_id.as_deref(),
        ) {
            log::warn!("Failed to migrate legacy account session encryption: {error}");
        }
    }
    Ok(Some(loaded))
}

/// Remove the persisted session file (called on logout).
pub fn clear_session() {
    if let Ok(path) = session_file_path() {
        let _ = std::fs::remove_file(&path);
    }
}

// ── Credential hint (non-secret: username + relay_url) ─────────────────
// Shared by Desktop and CLI so login forms pre-fill the same values.

fn credential_hint_path() -> Result<PathBuf> {
    let home = dirs::home_dir().ok_or_else(|| anyhow!("cannot determine home directory"))?;
    Ok(home.join(".bitfun").join("account_hint.json"))
}

/// Non-secret login pre-fill (never stores password or master key).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountHint {
    pub username: String,
    pub relay_url: String,
}

/// Persist username + relay URL for the next login form.
pub fn save_credential_hint(username: &str, relay_url: &str) {
    let hint = AccountHint {
        username: username.to_string(),
        relay_url: relay_url.to_string(),
    };
    let Ok(path) = credential_hint_path() else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(json) = serde_json::to_string(&hint) {
        let _ = write_private_file(&path, json.as_bytes());
    }
}

/// Load the persisted credential hint, if any.
pub fn load_credential_hint() -> Option<AccountHint> {
    let path = credential_hint_path().ok()?;
    let json = std::fs::read_to_string(&path).ok()?;
    serde_json::from_str(&json).ok()
}

/// Clear the credential hint (called on logout).
pub fn clear_credential_hint() {
    if let Ok(path) = credential_hint_path() {
        let _ = std::fs::remove_file(&path);
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn private_file_writer_uses_owner_only_permissions_and_replaces_atomically() {
        let root = tempfile::tempdir().unwrap();
        let private_dir = root.path().join("private");
        let path = private_dir.join("session.enc");

        write_private_file(&path, b"first").unwrap();
        write_private_file(&path, b"second").unwrap();

        assert_eq!(std::fs::read(&path).unwrap(), b"second");
        assert_eq!(
            std::fs::metadata(&private_dir)
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o700
        );
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert!(std::fs::read_dir(&private_dir)
            .unwrap()
            .all(|entry| entry.unwrap().path() == path));
    }
}
