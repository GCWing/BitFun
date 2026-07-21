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
pub const MASTER_KEY_LEN: usize = 32;
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
        // KDF parameters come from a remote server. Bound them before Argon2
        // allocates memory so a malicious or corrupted relay cannot force the
        // client into multi-gigabyte allocation or an excessive CPU loop.
        if !(8 * 1024..=256 * 1024).contains(&self.m)
            || !(1..=10).contains(&self.t)
            || !(1..=16).contains(&self.p)
        {
            return Err(anyhow!(
                "argon2 parameters are outside the supported safety range"
            ));
        }
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

/// A delegated token for a paired client (mobile-web / IM bot).
/// The desktop requests this from the relay and transmits it along
/// with the master_key to the paired client via the E2E room channel.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DelegateToken {
    pub token: String,
    pub user_id: String,
}

/// A delegated account identity: token + master_key, for paired clients
/// that don't do Argon2id themselves. The desktop delegates both.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DelegatedIdentity {
    pub token: String,
    pub user_id: String,
    pub master_key: String, // base64-encoded 32-byte master key
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

/// Check whether an account/relay error message indicates an invalid or
/// expired account token (relay auth failure). Shared by Desktop, CLI, and
/// the settings sync engine so they all react to the same relay wording.
pub fn error_indicates_expired_token(message: &str) -> bool {
    let lower = message.to_ascii_lowercase();
    lower.contains("http 401")
        || lower.contains("unauthorized")
        || lower.contains("invalid or expired token")
        || lower.contains("relay auth error")
}

/// Parse a user/config supplied relay base URL at the shared transport
/// boundary. Paths are allowed for reverse-proxy prefixes; credentials,
/// query strings, fragments, and non-HTTP schemes are not.
pub fn validate_relay_base_url(relay_url: &str) -> Result<reqwest::Url> {
    let url = reqwest::Url::parse(relay_url.trim())
        .map_err(|error| anyhow!("invalid relay URL: {error}"))?;
    if !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(anyhow!(
            "relay URL must be an http(s) server address without credentials, query, or fragment"
        ));
    }
    Ok(url)
}

impl Default for AccountClient {
    fn default() -> Self {
        Self::new()
    }
}

impl AccountClient {
    pub fn new() -> Self {
        Self {
            // Sync uploads full encrypted session bundles; 30s is too short for
            // large conversations over slower links.
            http: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(120))
                .build()
                .unwrap_or_else(|_| reqwest::Client::new()),
        }
    }

    fn endpoint(relay_url: &str, path: &str) -> Result<reqwest::Url> {
        let mut url = validate_relay_base_url(relay_url)?;
        let base_path = url.path().trim_end_matches('/').to_string();
        url.set_path(&format!("{base_path}{path}"));
        Ok(url)
    }

    /// Map a non-2xx relay response into a human-readable error.
    async fn into_error(resp: reqwest::Response) -> anyhow::Error {
        let status = resp.status();
        if status == reqwest::StatusCode::PAYLOAD_TOO_LARGE {
            return anyhow!(
                "relay returned HTTP 413 Payload Too Large \
                 (encrypted session/settings blob exceeds relay body limit; \
                  raise Axum DefaultBodyLimit on /api/sync/* and any reverse-proxy \
                  client_max_body_size)"
            );
        }
        if status == reqwest::StatusCode::INSUFFICIENT_STORAGE {
            return anyhow!(
                "relay returned HTTP 507 Insufficient Storage (the configured account or asset quota is full)"
            );
        }
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

    /// Fetch the login challenge and unwrap the master key locally.
    /// Does not call `/api/auth/login` and does not mint a token.
    async fn unwrap_master_key_for_credentials(
        &self,
        relay_url: &str,
        username: &str,
        password: &str,
    ) -> Result<([u8; MASTER_KEY_LEN], ChallengeResponse)> {
        if username.trim().is_empty()
            || username.len() > 128
            || password.is_empty()
            || password.len() > 1024
        {
            return Err(anyhow!("invalid account credential fields"));
        }
        let challenge_req = serde_json::json!({ "username": username });
        let resp = self
            .http
            .post(Self::endpoint(relay_url, "/api/auth/login/challenge")?)
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
        if salt.len() != SALT_LEN {
            return Err(anyhow!("invalid account salt length"));
        }
        let params: KdfParams = serde_json::from_str(&challenge.argon2_params)
            .map_err(|e| anyhow!("parse argon2_params: {e}"))?;

        // GCM tag failure means the password is wrong.
        let kek = derive_kek(password, &salt, &params)?;
        let master_key = unwrap_master_key(&kek, &challenge.wrapped_master_key)
            .map_err(|_| anyhow!("invalid username or password"))?;
        Ok((master_key, challenge))
    }

    /// Verify username/password against an existing session without minting a
    /// new relay token. Uses the same challenge + KEK unwrap path as `login`.
    /// Succeeds only when the unwrapped master key matches `expected_master_key`.
    pub async fn verify_password_for_master_key(
        &self,
        relay_url: &str,
        username: &str,
        password: &str,
        expected_master_key: &[u8; MASTER_KEY_LEN],
    ) -> Result<()> {
        let (master_key, _) = self
            .unwrap_master_key_for_credentials(relay_url, username, password)
            .await?;
        if master_key != *expected_master_key {
            return Err(anyhow!("invalid username or password"));
        }
        Ok(())
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
        let (master_key, challenge) = self
            .unwrap_master_key_for_credentials(relay_url, username, password)
            .await?;

        let kdf_salt = BASE64
            .decode(&challenge.kdf_salt)
            .map_err(|e| anyhow!("b64 decode kdf_salt: {e}"))?;
        if kdf_salt.len() != SALT_LEN {
            return Err(anyhow!("invalid account KDF salt length"));
        }
        let params: KdfParams = serde_json::from_str(&challenge.argon2_params)
            .map_err(|e| anyhow!("parse argon2_params: {e}"))?;

        // Derive the server-verifiable hash and submit it.
        let password_hash = derive_password_hash(password, &kdf_salt, &params)?;
        let login_req = serde_json::json!({
            "username": username,
            "password_hash": password_hash,
            "device_id": device.device_id,
            "device_name": device.device_name,
        });
        let resp = self
            .http
            .post(Self::endpoint(relay_url, "/api/auth/login")?)
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
    /// Returns the `version` written into the upsert body (client-generated LWW clock).
    pub async fn upload_session(
        &self,
        relay_url: &str,
        session: &AccountSession,
        session_id: &str,
        plaintext: &str,
    ) -> Result<i64> {
        let (data, nonce) = Self::seal(session, plaintext)?;
        let version = chrono::Utc::now().timestamp_millis();
        let body = serde_json::json!({
            "session_id": session_id,
            "encrypted_data": data,
            "nonce": nonce,
            "version": version,
        });
        let resp = self
            .http
            .post(Self::endpoint(relay_url, "/api/sync/sessions")?)
            .header("Authorization", Self::auth_header(session))
            .json(&body)
            .send()
            .await?;
        if !resp.status().is_success() {
            return Err(Self::into_error(resp).await);
        }
        Ok(version)
    }

    /// List encrypted session blobs updated after `since` without decrypting.
    /// `session_id` / `version` are plaintext metadata; payload stays sealed.
    pub async fn list_session_entries(
        &self,
        relay_url: &str,
        session: &AccountSession,
        since: i64,
    ) -> Result<Vec<ListedSessionEntry>> {
        let mut url = Self::endpoint(relay_url, "/api/sync/sessions")?;
        url.query_pairs_mut()
            .append_pair("since", &since.max(0).to_string());
        let resp = self
            .http
            .get(url)
            .header("Authorization", Self::auth_header(session))
            .send()
            .await?;
        if !resp.status().is_success() {
            return Err(Self::into_error(resp).await);
        }
        let bytes = resp.bytes().await?;
        let payload: SessionsListResponse = serde_json::from_slice(&bytes)
            .map_err(|e| anyhow!("decode sessions list failed ({} bytes): {e}", bytes.len()))?;
        Ok(payload
            .sessions
            .into_iter()
            .map(|entry| ListedSessionEntry {
                session_id: entry.session_id,
                encrypted_data: entry.encrypted_data,
                nonce: entry.nonce,
                version: entry.version,
            })
            .collect())
    }

    /// Decrypt one listed session entry locally.
    pub fn decrypt_session_entry(
        session: &AccountSession,
        entry: &ListedSessionEntry,
    ) -> Result<FetchedSession> {
        let plaintext = Self::open(session, &entry.encrypted_data, &entry.nonce)?;
        Ok(FetchedSession {
            session_id: entry.session_id.clone(),
            plaintext,
            version: entry.version,
        })
    }

    /// Fetch encrypted session blobs updated after `since` (relay `version`).
    /// Pass `since = 0` for a full list. Each entry is decrypted locally.
    pub async fn fetch_sessions(
        &self,
        relay_url: &str,
        session: &AccountSession,
        since: i64,
    ) -> Result<Vec<FetchedSession>> {
        let entries = self.list_session_entries(relay_url, session, since).await?;
        entries
            .into_iter()
            .map(|entry| Self::decrypt_session_entry(session, &entry))
            .collect()
    }

    /// Fetch and decrypt a single session blob by id.
    pub async fn fetch_session(
        &self,
        relay_url: &str,
        session: &AccountSession,
        session_id: &str,
    ) -> Result<Option<FetchedSession>> {
        let resp = self
            .http
            .get(Self::endpoint(
                relay_url,
                &format!("/api/sync/sessions/{}", urlencoding::encode(session_id)),
            )?)
            .header("Authorization", Self::auth_header(session))
            .send()
            .await?;
        if resp.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }
        if !resp.status().is_success() {
            return Err(Self::into_error(resp).await);
        }
        let entry: SessionEntry = resp.json().await?;
        let plaintext = Self::open(session, &entry.encrypted_data, &entry.nonce)?;
        Ok(Some(FetchedSession {
            session_id: entry.session_id,
            plaintext,
            version: entry.version,
        }))
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
                &format!("/api/sync/sessions/{}", urlencoding::encode(session_id)),
            )?)
            .header("Authorization", Self::auth_header(session))
            .send()
            .await?;
        if !resp.status().is_success() {
            return Err(Self::into_error(resp).await);
        }
        Ok(())
    }

    /// Upload an encrypted settings blob (keyed by the user, not per-device).
    /// Returns the version sent with the upload so callers can record a sync
    /// cursor that matches what the relay actually stored.
    pub async fn upload_settings(
        &self,
        relay_url: &str,
        session: &AccountSession,
        plaintext: &str,
    ) -> Result<i64> {
        let (data, nonce) = Self::seal(session, plaintext)?;
        let version = chrono::Utc::now().timestamp_millis();
        let body = serde_json::json!({
            "encrypted_data": data,
            "nonce": nonce,
            "version": version,
        });
        let resp = self
            .http
            .post(Self::endpoint(relay_url, "/api/sync/settings")?)
            .header("Authorization", Self::auth_header(session))
            .json(&body)
            .send()
            .await?;
        if !resp.status().is_success() {
            return Err(Self::into_error(resp).await);
        }
        Ok(version)
    }

    /// Fetch and decrypt the settings blob. Returns `None` if no settings exist.
    pub async fn fetch_settings(
        &self,
        relay_url: &str,
        session: &AccountSession,
    ) -> Result<Option<String>> {
        Ok(self
            .fetch_settings_with_version(relay_url, session)
            .await?
            .map(|b| b.plaintext))
    }

    /// Fetch and decrypt the settings blob, including the relay version.
    /// The version lets the caller skip applying if the cloud hasn't changed
    /// since the last pull.
    pub async fn fetch_settings_with_version(
        &self,
        relay_url: &str,
        session: &AccountSession,
    ) -> Result<Option<SettingsBlob>> {
        let resp = self
            .http
            .get(Self::endpoint(relay_url, "/api/sync/settings")?)
            .header("Authorization", Self::auth_header(session))
            .send()
            .await?;
        if resp.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }
        if !resp.status().is_success() {
            return Err(Self::into_error(resp).await);
        }
        let opt: Option<SettingsEntry> = resp.json().await?;
        match opt {
            None => Ok(None),
            Some(entry) => {
                let pt = Self::open(session, &entry.encrypted_data, &entry.nonce)?;
                Ok(Some(SettingsBlob {
                    plaintext: pt,
                    version: entry.version,
                }))
            }
        }
    }

    // ── Device RPC (browse/control other same-account devices) ────────────

    /// Delegate a new token for the same account (for paired mobile/IM clients).
    /// Returns the new token + user_id. The caller is responsible for securely
    /// transmitting the token + master_key to the paired client.
    pub async fn delegate_token(
        &self,
        relay_url: &str,
        session: &AccountSession,
    ) -> Result<DelegateToken> {
        let resp = self
            .http
            .post(Self::endpoint(relay_url, "/api/auth/delegate")?)
            .header("Authorization", Self::auth_header(session))
            .send()
            .await?;
        if !resp.status().is_success() {
            return Err(Self::into_error(resp).await);
        }
        let auth: AuthResponse = resp.json().await?;
        Ok(DelegateToken {
            token: auth.token,
            user_id: auth.user_id,
        })
    }

    /// Revoke the account token on the relay (server-side logout).
    pub async fn revoke_token(&self, relay_url: &str, session: &AccountSession) -> Result<()> {
        let resp = self
            .http
            .post(Self::endpoint(relay_url, "/api/auth/logout")?)
            .header("Authorization", Self::auth_header(session))
            .send()
            .await?;
        if !resp.status().is_success() {
            // Non-fatal — best-effort revocation
            log::warn!("revoke_token: relay returned {}", resp.status());
        }
        Ok(())
    }

    /// List all devices in the account (online + offline). Returns
    /// `(device_id, device_name, online, last_seen_at)`.
    pub async fn list_devices(
        &self,
        relay_url: &str,
        session: &AccountSession,
    ) -> Result<Vec<DeviceInfo>> {
        let resp = self
            .http
            .get(Self::endpoint(relay_url, "/api/devices")?)
            .header("Authorization", Self::auth_header(session))
            .send()
            .await?;
        if !resp.status().is_success() {
            return Err(Self::into_error(resp).await);
        }
        let entries: Vec<DeviceListEntry> = resp.json().await?;
        Ok(entries
            .into_iter()
            .map(|e| DeviceInfo {
                device_id: e.device_id,
                device_name: e.device_name,
                // Legacy relays omit `online` and only return currently-online
                // devices; treat a missing field as online.
                online: e.online.unwrap_or(true),
                last_seen_at: e.last_seen_at,
            })
            .collect())
    }

    /// Remove a device from the account (DELETE /api/devices/:id).
    pub async fn delete_device(
        &self,
        relay_url: &str,
        session: &AccountSession,
        target_device_id: &str,
    ) -> Result<()> {
        let resp = self
            .http
            .delete(Self::endpoint(
                relay_url,
                &format!("/api/devices/{}", urlencoding::encode(target_device_id)),
            )?)
            .header("Authorization", Self::auth_header(session))
            .send()
            .await?;
        if !resp.status().is_success() {
            return Err(Self::into_error(resp).await);
        }
        Ok(())
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
                &format!("/api/devices/{}/rpc", urlencoding::encode(target_device_id)),
            )?)
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

/// A device in the account (online or offline).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceInfo {
    pub device_id: String,
    pub device_name: String,
    pub online: bool,
    pub last_seen_at: Option<i64>,
}

#[derive(Deserialize)]
struct DeviceListEntry {
    device_id: String,
    device_name: String,
    /// Absent on legacy relays that only listed in-memory online devices.
    /// `None` means "legacy online list" → treat as online.
    #[serde(default)]
    online: Option<bool>,
    #[serde(default)]
    last_seen_at: Option<i64>,
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
    #[serde(default)]
    version: i64,
}

/// Decrypted session sync blob with relay version metadata.
#[derive(Debug, Clone)]
pub struct FetchedSession {
    pub session_id: String,
    pub plaintext: String,
    pub version: i64,
}

/// Encrypted session list entry (`session_id`/`version` are not secret).
#[derive(Debug, Clone)]
pub struct ListedSessionEntry {
    pub session_id: String,
    pub encrypted_data: String,
    pub nonce: String,
    pub version: i64,
}

/// A settings blob with version metadata, returned by `fetch_settings_with_version`.
#[derive(Debug, Clone)]
pub struct SettingsBlob {
    pub plaintext: String,
    pub version: i64,
}

#[derive(Deserialize)]
struct SettingsEntry {
    encrypted_data: String,
    nonce: String,
    #[serde(default)]
    version: i64,
    #[serde(default)]
    #[allow(dead_code)]
    updated_at: i64,
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
        assert!(params.build().is_ok());
        assert!(KdfParams {
            m: u32::MAX,
            t: 3,
            p: 4,
        }
        .build()
        .is_err());
    }

    #[test]
    fn relay_endpoint_accepts_http_servers_and_rejects_ambiguous_urls() {
        let endpoint =
            AccountClient::endpoint("https://relay.example.com/prefix/", "/api/devices").unwrap();
        assert_eq!(
            endpoint.as_str(),
            "https://relay.example.com/prefix/api/devices"
        );

        for invalid in [
            "file:///tmp/relay",
            "https://user:pass@relay.example.com",
            "https://relay.example.com?target=other",
            "https://relay.example.com/#fragment",
        ] {
            assert!(AccountClient::endpoint(invalid, "/api/devices").is_err());
        }
    }
}
