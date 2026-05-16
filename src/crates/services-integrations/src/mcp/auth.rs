//! Remote MCP OAuth runtime support.
//!
//! This module owns the file-backed credential store and OAuth bootstrap logic.
//! `bitfun-core` injects the product data directory and maps errors to its
//! compatibility error type.

use crate::mcp::server::{MCPServerConfig, MCPServerOAuthConfig};
use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use anyhow::{Context, Result};
use async_trait::async_trait;
use base64::{Engine, engine::general_purpose::STANDARD as B64};
use rand::RngCore;
use rmcp::transport::auth::{AuthorizationManager, CredentialStore, OAuthState, StoredCredentials};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use tokio::net::TcpListener;
use tokio::sync::Mutex;

const NONCE_LEN: usize = 12;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum MCPRemoteOAuthStatus {
    AwaitingBrowser,
    AwaitingCallback,
    ExchangingToken,
    Authorized,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MCPRemoteOAuthSessionSnapshot {
    pub server_id: String,
    pub status: MCPRemoteOAuthStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authorization_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub redirect_uri: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

impl MCPRemoteOAuthSessionSnapshot {
    pub fn new(
        server_id: impl Into<String>,
        status: MCPRemoteOAuthStatus,
        authorization_url: Option<String>,
        redirect_uri: Option<String>,
        message: Option<String>,
    ) -> Self {
        Self {
            server_id: server_id.into(),
            status,
            authorization_url,
            redirect_uri,
            message,
        }
    }
}

pub struct PreparedMCPRemoteOAuthAuthorization {
    pub state: OAuthState,
    pub listener: TcpListener,
    pub authorization_url: String,
    pub redirect_uri: String,
}

#[derive(Serialize, Deserialize, Default)]
struct VaultFile {
    entries: HashMap<String, String>,
}

pub struct MCPRemoteOAuthCredentialVault {
    key_path: PathBuf,
    vault_path: PathBuf,
    lock: Mutex<()>,
}

impl MCPRemoteOAuthCredentialVault {
    pub fn new(data_dir: impl Into<PathBuf>) -> Self {
        let data_dir = data_dir.into();
        Self {
            key_path: data_dir.join(".mcp_oauth_vault.key"),
            vault_path: data_dir.join("mcp_oauth_vault.json"),
            lock: Mutex::new(()),
        }
    }

    async fn ensure_key(&self) -> Result<[u8; 32]> {
        if self.key_path.exists() {
            let bytes = tokio::fs::read(&self.key_path)
                .await
                .context("read MCP OAuth vault key")?;
            if bytes.len() != 32 {
                anyhow::bail!("invalid MCP OAuth vault key length");
            }
            let mut key = [0u8; 32];
            key.copy_from_slice(&bytes);
            return Ok(key);
        }

        if let Some(parent) = self.key_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        let mut key = [0u8; 32];
        rand::rngs::OsRng.fill_bytes(&mut key);
        tokio::fs::write(&self.key_path, key.as_slice())
            .await
            .context("write MCP OAuth vault key")?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ =
                std::fs::set_permissions(&self.key_path, std::fs::Permissions::from_mode(0o600));
        }

        Ok(key)
    }

    fn encrypt_value(key: &[u8; 32], plaintext: &str) -> Result<String> {
        let cipher = Aes256Gcm::new_from_slice(key).map_err(|e| anyhow::anyhow!("{}", e))?;
        let mut nonce = [0u8; NONCE_LEN];
        rand::rngs::OsRng.fill_bytes(&mut nonce);
        let ciphertext = cipher
            .encrypt(Nonce::from_slice(&nonce), plaintext.as_bytes())
            .map_err(|e| anyhow::anyhow!("encrypt: {}", e))?;

        let mut blob = Vec::with_capacity(NONCE_LEN + ciphertext.len());
        blob.extend_from_slice(&nonce);
        blob.extend_from_slice(&ciphertext);
        Ok(B64.encode(blob))
    }

    fn decrypt_value(key: &[u8; 32], blob_b64: &str) -> Result<String> {
        let blob = B64
            .decode(blob_b64)
            .context("base64 decode MCP OAuth vault entry")?;
        if blob.len() <= NONCE_LEN {
            anyhow::bail!("MCP OAuth vault entry too short");
        }

        let (nonce, ciphertext) = blob.split_at(NONCE_LEN);
        let cipher = Aes256Gcm::new_from_slice(key).map_err(|e| anyhow::anyhow!("{}", e))?;
        let plaintext = cipher
            .decrypt(Nonce::from_slice(nonce), ciphertext)
            .map_err(|e| anyhow::anyhow!("decrypt: {}", e))?;
        String::from_utf8(plaintext).context("utf8 decode MCP OAuth vault entry")
    }

    pub async fn load(&self, server_id: &str) -> Result<Option<StoredCredentials>> {
        let _guard = self.lock.lock().await;
        if !self.key_path.exists() || !self.vault_path.exists() {
            return Ok(None);
        }

        let bytes = tokio::fs::read(&self.key_path)
            .await
            .context("read MCP OAuth vault key")?;
        if bytes.len() != 32 {
            anyhow::bail!("invalid MCP OAuth vault key length");
        }
        let mut key = [0u8; 32];
        key.copy_from_slice(&bytes);

        let body = tokio::fs::read_to_string(&self.vault_path)
            .await
            .unwrap_or_default();
        let file: VaultFile = serde_json::from_str(&body).unwrap_or_default();
        let Some(entry) = file.entries.get(server_id) else {
            return Ok(None);
        };

        let plaintext = match Self::decrypt_value(&key, entry) {
            Ok(plaintext) => plaintext,
            Err(error) => {
                log::warn!(
                    "Failed to decrypt MCP OAuth credentials for server {}: {}",
                    server_id,
                    error
                );
                return Ok(None);
            }
        };

        Ok(Some(serde_json::from_str(&plaintext)?))
    }

    pub async fn store(&self, server_id: &str, credentials: &StoredCredentials) -> Result<()> {
        let _guard = self.lock.lock().await;
        let key = self.ensure_key().await?;

        let mut file: VaultFile = if self.vault_path.exists() {
            let body = tokio::fs::read_to_string(&self.vault_path)
                .await
                .unwrap_or_default();
            serde_json::from_str(&body).unwrap_or_default()
        } else {
            VaultFile::default()
        };

        let plaintext = serde_json::to_string(credentials)?;
        let encrypted = Self::encrypt_value(&key, &plaintext)?;
        file.entries.insert(server_id.to_string(), encrypted);

        if let Some(parent) = self.vault_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        tokio::fs::write(&self.vault_path, serde_json::to_string_pretty(&file)?)
            .await
            .context("write MCP OAuth vault")?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ =
                std::fs::set_permissions(&self.vault_path, std::fs::Permissions::from_mode(0o600));
        }

        Ok(())
    }

    pub async fn clear(&self, server_id: &str) -> Result<()> {
        let _guard = self.lock.lock().await;
        if !self.vault_path.exists() {
            return Ok(());
        }

        let body = tokio::fs::read_to_string(&self.vault_path)
            .await
            .unwrap_or_default();
        let mut file: VaultFile = serde_json::from_str(&body).unwrap_or_default();
        file.entries.remove(server_id);

        if file.entries.is_empty() {
            let _ = tokio::fs::remove_file(&self.vault_path).await;
        } else {
            tokio::fs::write(&self.vault_path, serde_json::to_string_pretty(&file)?).await?;
        }

        Ok(())
    }
}

#[derive(Clone)]
pub struct MCPRemoteOAuthCredentialStore {
    data_dir: PathBuf,
    server_id: String,
}

impl MCPRemoteOAuthCredentialStore {
    pub fn new(data_dir: impl Into<PathBuf>, server_id: impl Into<String>) -> Self {
        Self {
            data_dir: data_dir.into(),
            server_id: server_id.into(),
        }
    }
}

#[async_trait]
impl CredentialStore for MCPRemoteOAuthCredentialStore {
    async fn load(&self) -> Result<Option<StoredCredentials>, rmcp::transport::auth::AuthError> {
        MCPRemoteOAuthCredentialVault::new(self.data_dir.clone())
            .load(&self.server_id)
            .await
            .map_err(|error| rmcp::transport::auth::AuthError::InternalError(error.to_string()))
    }

    async fn save(
        &self,
        credentials: StoredCredentials,
    ) -> Result<(), rmcp::transport::auth::AuthError> {
        MCPRemoteOAuthCredentialVault::new(self.data_dir.clone())
            .store(&self.server_id, &credentials)
            .await
            .map_err(|error| rmcp::transport::auth::AuthError::InternalError(error.to_string()))
    }

    async fn clear(&self) -> Result<(), rmcp::transport::auth::AuthError> {
        MCPRemoteOAuthCredentialVault::new(self.data_dir.clone())
            .clear(&self.server_id)
            .await
            .map_err(|error| rmcp::transport::auth::AuthError::InternalError(error.to_string()))
    }
}

pub async fn has_stored_oauth_credentials(
    data_dir: impl Into<PathBuf>,
    server_id: &str,
) -> Result<bool> {
    let store = MCPRemoteOAuthCredentialStore::new(data_dir, server_id.to_string());
    let credentials = store.load().await?;
    Ok(credentials.and_then(|entry| entry.token_response).is_some())
}

pub async fn clear_stored_oauth_credentials(
    data_dir: impl Into<PathBuf>,
    server_id: &str,
) -> Result<()> {
    MCPRemoteOAuthCredentialStore::new(data_dir, server_id.to_string())
        .clear()
        .await?;
    Ok(())
}

pub async fn build_authorization_manager(
    data_dir: impl Into<PathBuf>,
    server_id: &str,
    server_url: &str,
) -> Result<(AuthorizationManager, bool)> {
    let mut manager = AuthorizationManager::new(server_url).await?;
    manager.set_credential_store(MCPRemoteOAuthCredentialStore::new(
        data_dir,
        server_id.to_string(),
    ));
    let initialized = manager.initialize_from_store().await?;
    Ok((manager, initialized))
}

fn normalize_callback_host(config: &MCPServerOAuthConfig) -> String {
    config
        .callback_host
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("127.0.0.1")
        .to_string()
}

fn normalize_callback_path(config: &MCPServerOAuthConfig) -> String {
    let path = config
        .callback_path
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("/oauth/callback");

    if path.starts_with('/') {
        path.to_string()
    } else {
        format!("/{}", path)
    }
}

fn effective_oauth_config(config: &MCPServerConfig) -> MCPServerOAuthConfig {
    let mut oauth = config.oauth.clone().unwrap_or_default();
    if oauth.client_name.is_none() {
        oauth.client_name = Some(format!("BitFun MCP Client ({})", config.name));
    }
    oauth
}

pub async fn prepare_remote_oauth_authorization(
    data_dir: impl Into<PathBuf>,
    config: &MCPServerConfig,
) -> Result<PreparedMCPRemoteOAuthAuthorization> {
    let data_dir = data_dir.into();
    let oauth = effective_oauth_config(config);
    let server_url = config.url.as_deref().ok_or_else(|| {
        anyhow::anyhow!(
            "Remote MCP server '{}' must have a URL for OAuth",
            config.id
        )
    })?;

    let host = normalize_callback_host(&oauth);
    let listener = TcpListener::bind((host.as_str(), oauth.callback_port.unwrap_or(0)))
        .await
        .with_context(|| {
            format!(
                "Failed to bind OAuth callback listener for server '{}'",
                config.id
            )
        })?;
    let port = listener
        .local_addr()
        .with_context(|| {
            format!(
                "Failed to resolve OAuth callback listener for server '{}'",
                config.id
            )
        })?
        .port();
    let redirect_uri = format!(
        "http://{}:{}{}",
        host,
        port,
        normalize_callback_path(&oauth)
    );

    let scopes = oauth.scopes.iter().map(String::as_str).collect::<Vec<_>>();
    let mut state = OAuthState::new(server_url, None).await?;
    if let OAuthState::Unauthorized(manager) = &mut state {
        manager.set_credential_store(MCPRemoteOAuthCredentialStore::new(
            data_dir.clone(),
            config.id.clone(),
        ));
    }

    match oauth.client_metadata_url.as_deref() {
        Some(client_metadata_url) => {
            state
                .start_authorization_with_metadata_url(
                    &scopes,
                    &redirect_uri,
                    oauth.client_name.as_deref(),
                    Some(client_metadata_url),
                )
                .await?;
        }
        None => {
            state
                .start_authorization(&scopes, &redirect_uri, oauth.client_name.as_deref())
                .await?;
        }
    }

    let authorization_url = state.get_authorization_url().await?;

    Ok(PreparedMCPRemoteOAuthAuthorization {
        state,
        listener,
        authorization_url,
        redirect_uri,
    })
}
