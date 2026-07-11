//! Account login client: Argon2id key derivation + register/login flows.
//!
//! The relay never sees the plaintext password or the master key. This module
//! derives the KEK and password hash locally, wraps the random master key with
//! the KEK, and sends only non-secret artifacts (salts, hashes, wrapped key)
//! to the relay. The master key lives in memory only after login.

use anyhow::{anyhow, Result};
use argon2::{Algorithm, Argon2, Params, Version};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
#[cfg(test)]
use rand::RngCore;
use serde::{Deserialize, Serialize};

use crate::remote_connect::device::DeviceIdentity;
use crate::remote_connect::encryption::{decrypt, encrypt};

/// Salt length for provisioning (client-side account-blob export). Only used
/// by tests until that tooling lands.
#[allow(dead_code)]
const SALT_LEN: usize = 16;
const MASTER_KEY_LEN: usize = 32;
const NONCE_LEN: usize = 12;

/// Argon2id parameters used for key derivation. Stored on the relay (non-secret)
/// so the client can rebuild the identical KDF on login.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KdfParams {
    /// Memory cost in KiB (e.g. 65536 = 64 MB).
    pub m: u32,
    /// Time cost (iterations).
    pub t: u32,
    /// Parallelism (lanes).
    pub p: u32,
}

impl Default for KdfParams {
    fn default() -> Self {
        // OWASP-recommended Argon2id baseline: 64 MB, 3 iterations, 4 lanes.
        Self {
            m: 65536,
            t: 3,
            p: 4,
        }
    }
}

impl KdfParams {
    fn build(&self) -> Result<Argon2<'static>> {
        let params = Params::new(self.m, self.t, self.p, Some(MASTER_KEY_LEN))
            .map_err(|e| anyhow!("invalid argon2 params: {e}"))?;
        Ok(Argon2::new(Algorithm::Argon2id, Version::V0x13, params))
    }
}

/// A successful account session: the relay token + the decrypted master key.
/// The master key lives in memory only; it is never persisted to disk.
#[derive(Debug, Clone)]
pub struct AccountSession {
    pub token: String,
    pub user_id: String,
    pub master_key: [u8; MASTER_KEY_LEN],
}

// ── Key derivation & wrapping ───────────────────────────────────────────

/// Derive the KEK (key-encryption key) from the password. The KEK never leaves
/// the client; it is used only to wrap/unwrap the master key.
fn derive_kek(password: &str, salt: &[u8], params: &KdfParams) -> Result<[u8; MASTER_KEY_LEN]> {
    let argon2 = params.build()?;
    let mut out = [0u8; MASTER_KEY_LEN];
    argon2
        .hash_password_into(password.as_bytes(), salt, &mut out)
        .map_err(|e| anyhow!("argon2 kek: {e}"))?;
    Ok(out)
}

/// Derive the password hash for server-side verification (uses a separate salt
/// so the KEK and the server-verifiable hash cannot be correlated).
fn derive_password_hash(password: &str, kdf_salt: &[u8], params: &KdfParams) -> Result<String> {
    let argon2 = params.build()?;
    let mut out = [0u8; MASTER_KEY_LEN];
    argon2
        .hash_password_into(password.as_bytes(), kdf_salt, &mut out)
        .map_err(|e| anyhow!("argon2 pwd hash: {e}"))?;
    Ok(BASE64.encode(out))
}

/// Pack wrapped ciphertext + nonce into a single storable string: `"ct.nonce"`.
/// Used by the provisioning path (client-side account-blob export for admin
/// import); not needed for login, hence `dead_code` until that tooling lands.
#[allow(dead_code)]
fn pack_wrapped(ct_b64: &str, nonce_b64: &str) -> String {
    format!("{ct_b64}.{nonce_b64}")
}

/// Split a packed `"ct.nonce"` string back into its parts.
fn unpack_wrapped(packed: &str) -> Result<(String, String)> {
    let (ct, nonce) = packed
        .split_once('.')
        .ok_or_else(|| anyhow!("invalid wrapped master key format"))?;
    Ok((ct.to_string(), nonce.to_string()))
}

/// Wrap (encrypt) the master key with the KEK → `"ct.nonce"`.
/// Provisioning helper (see `pack_wrapped`); unused by login.
#[allow(dead_code)]
fn wrap_master_key(
    kek: &[u8; MASTER_KEY_LEN],
    master_key: &[u8; MASTER_KEY_LEN],
) -> Result<String> {
    let (ct, nonce) = encrypt(kek, master_key.as_slice())?;
    Ok(pack_wrapped(&BASE64.encode(ct), &BASE64.encode(&nonce[..])))
}

/// Unwrap (decrypt) the master key with the KEK. A GCM tag failure means the
/// password is wrong.
fn unwrap_master_key(kek: &[u8; MASTER_KEY_LEN], packed: &str) -> Result<[u8; MASTER_KEY_LEN]> {
    let (ct_b64, nonce_b64) = unpack_wrapped(packed)?;
    let ct = BASE64
        .decode(&ct_b64)
        .map_err(|e| anyhow!("b64 decode wrapped ct: {e}"))?;
    let nonce_vec = BASE64
        .decode(&nonce_b64)
        .map_err(|e| anyhow!("b64 decode wrapped nonce: {e}"))?;
    if nonce_vec.len() != NONCE_LEN {
        return Err(anyhow!("invalid wrapped nonce length"));
    }
    let mut nonce = [0u8; NONCE_LEN];
    nonce.copy_from_slice(&nonce_vec);
    let pt = decrypt(kek, &ct, &nonce)?;
    if pt.len() != MASTER_KEY_LEN {
        return Err(anyhow!("decrypted master key has wrong length"));
    }
    let mut mk = [0u8; MASTER_KEY_LEN];
    mk.copy_from_slice(&pt);
    Ok(mk)
}

// ── Relay HTTP client ───────────────────────────────────────────────────

#[derive(Deserialize)]
struct AuthResponse {
    token: String,
    user_id: String,
}

#[derive(Deserialize)]
struct ChallengeResponse {
    salt: String,
    kdf_salt: String,
    argon2_params: String,
    wrapped_master_key: String,
}

#[derive(Deserialize)]
struct ErrorBody {
    error: String,
    #[serde(default)]
    retry_after_secs: Option<i64>,
}

/// HTTP client for the relay's account endpoints.
pub struct AccountClient {
    http: reqwest::Client,
}

impl Default for AccountClient {
    fn default() -> Self {
        Self::new()
    }
}

impl AccountClient {
    pub fn new() -> Self {
        Self {
            http: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .unwrap_or_else(|_| reqwest::Client::new()),
        }
    }

    fn endpoint(relay_url: &str, path: &str) -> String {
        let base = relay_url.trim_end_matches('/');
        format!("{base}{path}")
    }

    /// Map a non-2xx relay response into a human-readable error.
    async fn into_error(resp: reqwest::Response) -> anyhow::Error {
        let status = resp.status();
        match resp.json::<ErrorBody>().await {
            Ok(body) => {
                let msg = body.error;
                if let Some(retry) = body.retry_after_secs {
                    anyhow!("{msg} (HTTP {status}, retry in {retry}s)")
                } else {
                    anyhow!("{msg} (HTTP {status})")
                }
            }
            Err(_) => anyhow!("relay returned HTTP {status}"),
        }
    }

    /// Log in to an existing account. Fetches the KDF challenge, derives the KEK
    /// locally, unwraps the master key (GCM failure ⇒ wrong password), then
    /// verifies the password hash with the relay to obtain a token.
    pub async fn login(
        &self,
        relay_url: &str,
        username: &str,
        password: &str,
        device: &DeviceIdentity,
    ) -> Result<AccountSession> {
        // 1. Challenge: fetch salts + wrapped master key.
        let challenge_req = serde_json::json!({ "username": username });
        let resp = self
            .http
            .post(Self::endpoint(relay_url, "/api/auth/login/challenge"))
            .json(&challenge_req)
            .send()
            .await?;
        if !resp.status().is_success() {
            return Err(Self::into_error(resp).await);
        }
        let challenge: ChallengeResponse = resp.json().await?;

        let salt = BASE64
            .decode(&challenge.salt)
            .map_err(|e| anyhow!("b64 decode salt: {e}"))?;
        let kdf_salt = BASE64
            .decode(&challenge.kdf_salt)
            .map_err(|e| anyhow!("b64 decode kdf_salt: {e}"))?;
        let params: KdfParams = serde_json::from_str(&challenge.argon2_params)
            .map_err(|e| anyhow!("parse argon2_params: {e}"))?;

        // 2. Derive KEK and unwrap the master key. A failure here means the
        //    password is wrong (the GCM tag won't verify).
        let kek = derive_kek(password, &salt, &params)?;
        let master_key = unwrap_master_key(&kek, &challenge.wrapped_master_key)
            .map_err(|_| anyhow!("invalid username or password"))?;

        // 3. Derive the server-verifiable hash and submit it.
        let password_hash = derive_password_hash(password, &kdf_salt, &params)?;
        let login_req = serde_json::json!({
            "username": username,
            "password_hash": password_hash,
            "device_id": device.device_id,
            "device_name": device.device_name,
        });
        let resp = self
            .http
            .post(Self::endpoint(relay_url, "/api/auth/login"))
            .json(&login_req)
            .send()
            .await?;
        if !resp.status().is_success() {
            return Err(Self::into_error(resp).await);
        }
        let auth: AuthResponse = resp.json().await?;
        Ok(AccountSession {
            token: auth.token,
            user_id: auth.user_id,
            master_key,
        })
    }

    // ── Encrypted sync (sessions + settings) ────────────────────────────────
    //
    // All sync payloads are encrypted with the in-memory master_key via
    // AES-256-GCM before leaving this device. The relay stores opaque
    // ciphertext only.

    fn auth_header(session: &AccountSession) -> String {
        format!("Bearer {}", session.token)
    }

    /// Encrypt `plaintext` with the master key, returning base64 `(data, nonce)`.
    fn seal(session: &AccountSession, plaintext: &str) -> Result<(String, String)> {
        encrypt(&session.master_key, plaintext.as_bytes())
            .map(|(ct, nonce)| (BASE64.encode(ct), BASE64.encode(&nonce[..])))
            .map_err(|e| anyhow!("encrypt sync blob: {e}"))
    }

    /// Decrypt base64 `(data, nonce)` with the master key.
    fn open(session: &AccountSession, data_b64: &str, nonce_b64: &str) -> Result<String> {
        let ct = BASE64
            .decode(data_b64)
            .map_err(|e| anyhow!("b64 decode sync ct: {e}"))?;
        let nonce_vec = BASE64
            .decode(nonce_b64)
            .map_err(|e| anyhow!("b64 decode sync nonce: {e}"))?;
        if nonce_vec.len() != NONCE_LEN {
            return Err(anyhow!("invalid sync nonce length"));
        }
        let mut nonce = [0u8; NONCE_LEN];
        nonce.copy_from_slice(&nonce_vec);
        let pt = decrypt(&session.master_key, &ct, &nonce)
            .map_err(|e| anyhow!("decrypt sync blob: {e}"))?;
        String::from_utf8(pt).map_err(|e| anyhow!("sync blob utf8: {e}"))
    }

    /// Upload (or replace) a single encrypted session blob for this device.
    pub async fn upload_session(
        &self,
        relay_url: &str,
        session: &AccountSession,
        session_id: &str,
        plaintext: &str,
    ) -> Result<()> {
        let (data, nonce) = Self::seal(session, plaintext)?;
        let body = serde_json::json!({
            "session_id": session_id,
            "encrypted_data": data,
            "nonce": nonce,
            "version": chrono::Utc::now().timestamp_millis(),
        });
        let resp = self
            .http
            .post(Self::endpoint(relay_url, "/api/sync/sessions"))
            .header("Authorization", Self::auth_header(session))
            .json(&body)
            .send()
            .await?;
        if !resp.status().is_success() {
            return Err(Self::into_error(resp).await);
        }
        Ok(())
    }

    /// Fetch all encrypted session blobs for the account, returning decrypted
    /// `(session_id, plaintext)` pairs.
    pub async fn fetch_sessions(
        &self,
        relay_url: &str,
        session: &AccountSession,
    ) -> Result<Vec<(String, String)>> {
        let resp = self
            .http
            .get(Self::endpoint(relay_url, "/api/sync/sessions"))
            .header("Authorization", Self::auth_header(session))
            .send()
            .await?;
        if !resp.status().is_success() {
            return Err(Self::into_error(resp).await);
        }
        let payload: SessionsListResponse = resp.json().await?;
        payload
            .sessions
            .into_iter()
            .map(|entry| {
                let pt = Self::open(session, &entry.encrypted_data, &entry.nonce)?;
                Ok((entry.session_id, pt))
            })
            .collect()
    }

    /// Delete a session blob (tombstone) — used when a session is removed.
    pub async fn delete_session(
        &self,
        relay_url: &str,
        session: &AccountSession,
        session_id: &str,
    ) -> Result<()> {
        let resp = self
            .http
            .delete(Self::endpoint(
                relay_url,
                &format!("/api/sync/sessions/{session_id}"),
            ))
            .header("Authorization", Self::auth_header(session))
            .send()
            .await?;
        if !resp.status().is_success() {
            return Err(Self::into_error(resp).await);
        }
        Ok(())
    }

    /// Upload an encrypted settings blob (keyed by the user, not per-device).
    pub async fn upload_settings(
        &self,
        relay_url: &str,
        session: &AccountSession,
        plaintext: &str,
    ) -> Result<()> {
        let (data, nonce) = Self::seal(session, plaintext)?;
        let body = serde_json::json!({
            "encrypted_data": data,
            "nonce": nonce,
            "version": chrono::Utc::now().timestamp_millis(),
        });
        let resp = self
            .http
            .post(Self::endpoint(relay_url, "/api/sync/settings"))
            .header("Authorization", Self::auth_header(session))
            .json(&body)
            .send()
            .await?;
        if !resp.status().is_success() {
            return Err(Self::into_error(resp).await);
        }
        Ok(())
    }

    /// Fetch and decrypt the settings blob. Returns `None` if no settings exist.
    pub async fn fetch_settings(
        &self,
        relay_url: &str,
        session: &AccountSession,
    ) -> Result<Option<String>> {
        let resp = self
            .http
            .get(Self::endpoint(relay_url, "/api/sync/settings"))
            .header("Authorization", Self::auth_header(session))
            .send()
            .await?;
        if resp.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }
        if !resp.status().is_success() {
            return Err(Self::into_error(resp).await);
        }
        // The relay returns `Json<Option<SettingsBlob>>`, so we parse as optional.
        let opt: Option<SettingsEntry> = resp.json().await?;
        match opt {
            None => Ok(None),
            Some(entry) => {
                let pt = Self::open(session, &entry.encrypted_data, &entry.nonce)?;
                Ok(Some(pt))
            }
        }
    }

    // ── Device RPC (browse/control other same-account devices) ────────────

    /// List all online devices in the account. Returns `(device_id, device_name)`.
    pub async fn list_devices(
        &self,
        relay_url: &str,
        session: &AccountSession,
    ) -> Result<Vec<(String, String)>> {
        let resp = self
            .http
            .get(Self::endpoint(relay_url, "/api/devices"))
            .header("Authorization", Self::auth_header(session))
            .send()
            .await?;
        if !resp.status().is_success() {
            return Err(Self::into_error(resp).await);
        }
        let entries: Vec<DeviceListEntry> = resp.json().await?;
        Ok(entries
            .into_iter()
            .map(|e| (e.device_id, e.device_name))
            .collect())
    }

    /// Send an encrypted RemoteCommand to a target device via HTTP RPC.
    /// The relay routes the opaque ciphertext to the target device's WS,
    /// waits for the response, and returns the encrypted response.
    /// The caller is responsible for encrypting the command and decrypting
    /// the response with the account master_key.
    pub async fn device_rpc(
        &self,
        relay_url: &str,
        session: &AccountSession,
        target_device_id: &str,
        plaintext_command: &str,
    ) -> Result<String> {
        // Encrypt the command with the master key
        let (data, nonce) = Self::seal(session, plaintext_command)?;
        let body = serde_json::json!({
            "encrypted_data": data,
            "nonce": nonce,
        });
        let resp = self
            .http
            .post(Self::endpoint(
                relay_url,
                &format!("/api/devices/{target_device_id}/rpc"),
            ))
            .header("Authorization", Self::auth_header(session))
            .json(&body)
            .send()
            .await?;
        if !resp.status().is_success() {
            return Err(Self::into_error(resp).await);
        }
        let entry: RpcResponseEntry = resp.json().await?;
        // Decrypt the response with the master key
        Self::open(session, &entry.encrypted_data, &entry.nonce)
    }
}

#[derive(Deserialize)]
struct DeviceListEntry {
    device_id: String,
    device_name: String,
}

#[derive(Deserialize)]
struct RpcResponseEntry {
    encrypted_data: String,
    nonce: String,
}

#[derive(Deserialize)]
struct SessionsListResponse {
    sessions: Vec<SessionEntry>,
}

#[derive(Deserialize)]
struct SessionEntry {
    session_id: String,
    encrypted_data: String,
    nonce: String,
}

#[derive(Deserialize)]
struct SettingsEntry {
    encrypted_data: String,
    nonce: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrap_unwrap_round_trip() {
        let params = KdfParams::default();
        let mut salt = [0u8; SALT_LEN];
        let mut master_key = [0u8; MASTER_KEY_LEN];
        rand::thread_rng().fill_bytes(&mut salt);
        rand::thread_rng().fill_bytes(&mut master_key);

        let kek = derive_kek("correct-horse-battery", &salt, &params).unwrap();
        let wrapped = wrap_master_key(&kek, &master_key).unwrap();
        let recovered = unwrap_master_key(&kek, &wrapped).unwrap();
        assert_eq!(recovered, master_key);
    }

    #[test]
    fn wrong_password_fails_to_unwrap() {
        let params = KdfParams::default();
        let mut salt = [0u8; SALT_LEN];
        let mut master_key = [0u8; MASTER_KEY_LEN];
        rand::thread_rng().fill_bytes(&mut salt);
        rand::thread_rng().fill_bytes(&mut master_key);

        let kek = derive_kek("correct-password", &salt, &params).unwrap();
        let wrapped = wrap_master_key(&kek, &master_key).unwrap();

        let wrong_kek = derive_kek("wrong-password", &salt, &params).unwrap();
        assert!(unwrap_master_key(&wrong_kek, &wrapped).is_err());
    }

    #[test]
    fn kdf_params_round_trip() {
        let params = KdfParams::default();
        let json = serde_json::to_string(&params).unwrap();
        let parsed: KdfParams = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.m, params.m);
        assert_eq!(parsed.t, params.t);
        assert_eq!(parsed.p, params.p);
    }
}
