//! Persistence for subscription-account credentials.
//!
//! OAuth tokens and API keys are stored in the operating-system credential
//! vault (macOS Keychain, Windows Credential Manager, or Linux Secret
//! Service). The JSON file contains only non-secret account metadata and
//! references used to discover the corresponding vault entries.
//!
//! Path: `{dirs::config_dir()}/bitfun/data/subscription_auth.json`.

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock, RwLock};

const STORE_VERSION: u8 = 2;
const KEYRING_SERVICE: &str = "openbitfun.bitfun.subscription-auth.v1";
// Windows Credential Manager limits a generic credential blob to 2560 bytes.
// Leave headroom for platform-store implementations and split every logical
// secret so a long JWT or refresh token remains portable across all hosts.
const SECRET_CHUNK_BYTES: usize = 2_048;

/// A single credential assembled in memory after its secret material has been
/// read from the platform credential vault.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StoredCredential {
    Oauth {
        refresh: String,
        access: String,
        /// Milliseconds since the Unix epoch when `access` expires.
        expires: i64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        account_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        metadata: Option<serde_json::Value>,
    },
    Api {
        key: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        metadata: Option<serde_json::Value>,
    },
}

/// Provider id -> in-memory credential.
pub type Store = HashMap<String, StoredCredential>;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum CredentialMetadata {
    Oauth {
        expires: i64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        account_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        metadata: Option<serde_json::Value>,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        needs_reauthentication: bool,
        /// Unique namespace for this committed set of vault chunks. `None`
        /// denotes the legacy single-password entry keyed by provider.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        secret_set_id: Option<String>,
        #[serde(default, skip_serializing_if = "is_zero")]
        refresh_parts: u32,
        #[serde(default, skip_serializing_if = "is_zero")]
        access_parts: u32,
    },
    Api {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        metadata: Option<serde_json::Value>,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        needs_reauthentication: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        secret_set_id: Option<String>,
        #[serde(default, skip_serializing_if = "is_zero")]
        key_parts: u32,
    },
}

fn is_zero(value: &u32) -> bool {
    *value == 0
}

fn secret_part_count(secret: &str) -> u32 {
    // Even an empty value has one explicit part, distinguishing a committed
    // empty refresh token from a missing vault entry.
    secret.len().max(1).div_ceil(SECRET_CHUNK_BYTES) as u32
}

impl CredentialMetadata {
    fn from_credential(credential: &StoredCredential) -> Self {
        match credential {
            StoredCredential::Oauth {
                refresh,
                access,
                expires,
                account_id,
                metadata,
                ..
            } => Self::Oauth {
                expires: *expires,
                account_id: account_id.clone(),
                metadata: metadata.clone(),
                needs_reauthentication: false,
                secret_set_id: Some(uuid::Uuid::new_v4().simple().to_string()),
                refresh_parts: secret_part_count(refresh),
                access_parts: secret_part_count(access),
            },
            StoredCredential::Api { key, metadata } => Self::Api {
                metadata: metadata.clone(),
                needs_reauthentication: false,
                secret_set_id: Some(uuid::Uuid::new_v4().simple().to_string()),
                key_parts: secret_part_count(key),
            },
        }
    }

    fn requiring_reauthentication(credential: &StoredCredential) -> Self {
        let mut metadata = Self::from_credential(credential);
        match &mut metadata {
            Self::Oauth {
                needs_reauthentication,
                secret_set_id,
                refresh_parts,
                access_parts,
                ..
            } => {
                *needs_reauthentication = true;
                *secret_set_id = None;
                *refresh_parts = 0;
                *access_parts = 0;
            }
            Self::Api {
                needs_reauthentication,
                secret_set_id,
                key_parts,
                ..
            } => {
                *needs_reauthentication = true;
                *secret_set_id = None;
                *key_parts = 0;
            }
        }
        metadata
    }

    fn needs_reauthentication(&self) -> bool {
        match self {
            Self::Oauth {
                needs_reauthentication,
                ..
            }
            | Self::Api {
                needs_reauthentication,
                ..
            } => *needs_reauthentication,
        }
    }

    fn combine(&self, secret: SecretMaterial) -> Option<StoredCredential> {
        match (self, secret) {
            (
                Self::Oauth {
                    expires,
                    account_id,
                    metadata,
                    ..
                },
                SecretMaterial::Oauth { refresh, access },
            ) => Some(StoredCredential::Oauth {
                refresh,
                access,
                expires: *expires,
                account_id: account_id.clone(),
                metadata: metadata.clone(),
            }),
            (Self::Api { metadata, .. }, SecretMaterial::Api { key }) => {
                Some(StoredCredential::Api {
                    key,
                    metadata: metadata.clone(),
                })
            }
            _ => None,
        }
    }

    fn vault_entries(&self, provider: &str) -> Vec<String> {
        match self {
            Self::Oauth {
                secret_set_id: Some(set_id),
                refresh_parts,
                access_parts,
                ..
            } => secret_entry_names(provider, set_id, "refresh", *refresh_parts)
                .chain(secret_entry_names(
                    provider,
                    set_id,
                    "access",
                    *access_parts,
                ))
                .collect(),
            Self::Api {
                secret_set_id: Some(set_id),
                key_parts,
                ..
            } => secret_entry_names(provider, set_id, "api-key", *key_parts).collect(),
            // The old representation used one password entry named exactly
            // after the provider.
            _ => vec![provider.to_string()],
        }
    }
}

fn secret_entry_names<'a>(
    provider: &'a str,
    set_id: &'a str,
    field: &'a str,
    parts: u32,
) -> impl Iterator<Item = String> + 'a {
    (0..parts).map(move |index| format!("{provider}/v2/{set_id}/{field}/{index}"))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum SecretMaterial {
    Oauth { refresh: String, access: String },
    Api { key: String },
}

impl From<&StoredCredential> for SecretMaterial {
    fn from(value: &StoredCredential) -> Self {
        match value {
            StoredCredential::Oauth {
                refresh, access, ..
            } => Self::Oauth {
                refresh: refresh.clone(),
                access: access.clone(),
            },
            StoredCredential::Api { key, .. } => Self::Api { key: key.clone() },
        }
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct SecureStoreFile {
    version: u8,
    #[serde(default)]
    accounts: HashMap<String, CredentialMetadata>,
}

/// Result used by account discovery so a missing/locked vault entry is visible
/// to the UI instead of silently looking like a never-configured account.
pub(crate) struct LoadState {
    pub credentials: Store,
    pub requires_reauthentication: HashSet<String>,
    /// Metadata and secret entries exist, but the OS vault is currently
    /// locked/unavailable. This is retryable and must not be shown as lost.
    pub vault_unavailable: HashSet<String>,
}

fn store_path_override() -> &'static RwLock<Option<PathBuf>> {
    static OVERRIDE: OnceLock<RwLock<Option<PathBuf>>> = OnceLock::new();
    OVERRIDE.get_or_init(|| RwLock::new(None))
}

/// Test-only secret material, keyed by the overridden metadata path. Tests
/// must never read from or write to a developer's real system credential vault.
fn test_secrets() -> &'static Mutex<HashMap<PathBuf, HashMap<String, Vec<u8>>>> {
    static SECRETS: OnceLock<Mutex<HashMap<PathBuf, HashMap<String, Vec<u8>>>>> = OnceLock::new();
    SECRETS.get_or_init(|| Mutex::new(HashMap::new()))
}

#[cfg(test)]
fn unavailable_test_vaults() -> &'static Mutex<HashSet<PathBuf>> {
    static PATHS: OnceLock<Mutex<HashSet<PathBuf>>> = OnceLock::new();
    PATHS.get_or_init(|| Mutex::new(HashSet::new()))
}

#[cfg(test)]
fn failing_metadata_writes() -> &'static Mutex<HashSet<PathBuf>> {
    static PATHS: OnceLock<Mutex<HashSet<PathBuf>>> = OnceLock::new();
    PATHS.get_or_init(|| Mutex::new(HashSet::new()))
}

fn native_keyring_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

fn store_operation_lock() -> &'static tokio::sync::Mutex<()> {
    static LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
    &LOCK
}

/// Overrides the metadata path for tests. The override also switches secret
/// persistence to the process-local test vault above.
pub fn set_store_path_for_test(path: PathBuf) {
    if let Ok(mut guard) = store_path_override().write() {
        *guard = Some(path);
    }
}

#[cfg(test)]
pub(crate) fn store_path_for_test_assertion() -> PathBuf {
    overridden_store_path().expect("subscription test store path is configured")
}

#[cfg(test)]
pub(crate) fn test_vault_entries_for_assertion() -> HashMap<String, Vec<u8>> {
    let path = store_path_for_test_assertion();
    test_secrets()
        .lock()
        .ok()
        .and_then(|vault| vault.get(&path).cloned())
        .unwrap_or_default()
}

#[cfg(test)]
pub(crate) fn set_test_vault_unavailable(unavailable: bool) {
    let path = store_path_for_test_assertion();
    if let Ok(mut paths) = unavailable_test_vaults().lock() {
        if unavailable {
            paths.insert(path);
        } else {
            paths.remove(&path);
        }
    }
}

#[cfg(test)]
pub(crate) fn set_test_metadata_write_failure(fail: bool) {
    let path = store_path_for_test_assertion();
    if let Ok(mut paths) = failing_metadata_writes().lock() {
        if fail {
            paths.insert(path);
        } else {
            paths.remove(&path);
        }
    }
}

#[cfg(test)]
fn test_vault_is_unavailable(path: &Path) -> bool {
    unavailable_test_vaults()
        .lock()
        .map(|paths| paths.contains(path))
        .unwrap_or(true)
}

#[cfg(not(test))]
fn test_vault_is_unavailable(_path: &Path) -> bool {
    false
}

#[cfg(test)]
fn metadata_write_should_fail(path: &Path) -> bool {
    failing_metadata_writes()
        .lock()
        .map(|paths| paths.contains(path))
        .unwrap_or(true)
}

#[cfg(not(test))]
fn metadata_write_should_fail(_path: &Path) -> bool {
    false
}

fn overridden_store_path() -> Option<PathBuf> {
    store_path_override()
        .read()
        .ok()
        .and_then(|guard| guard.clone())
}

fn store_path() -> Result<PathBuf> {
    if let Some(path) = overridden_store_path() {
        return Ok(path);
    }
    let base = dirs::config_dir().ok_or_else(|| anyhow!("system config directory unavailable"))?;
    Ok(base
        .join("bitfun")
        .join("data")
        .join("subscription_auth.json"))
}

async fn read_bytes(path: &Path) -> Result<Option<Vec<u8>>> {
    #[cfg(windows)]
    restore_windows_backup_if_needed(path).await?;
    if !path.exists() {
        return Ok(None);
    }
    let bytes = tokio::fs::read(path)
        .await
        .with_context(|| format!("read subscription auth metadata at {}", path.display()))?;
    Ok((!bytes.is_empty()).then_some(bytes))
}

/// Recover the old metadata index if the process stopped after rotating the
/// destination but before moving the new temp file into place.
#[cfg(windows)]
async fn restore_windows_backup_if_needed(path: &Path) -> Result<()> {
    if path.exists() {
        return Ok(());
    }
    let backup = path.with_extension("bak");
    if !backup.exists() {
        return Ok(());
    }
    tokio::fs::rename(&backup, path).await.with_context(|| {
        format!(
            "restore interrupted subscription auth metadata {} -> {}",
            backup.display(),
            path.display()
        )
    })
}

fn parse_secure_file(bytes: &[u8], path: &Path) -> Result<SecureStoreFile> {
    let file: SecureStoreFile = serde_json::from_slice(bytes)
        .with_context(|| format!("parse subscription auth metadata at {}", path.display()))?;
    if file.version != STORE_VERSION {
        return Err(anyhow!(
            "unsupported subscription auth metadata version {} at {}",
            file.version,
            path.display()
        ));
    }
    Ok(file)
}

async fn read_secure_file(path: &Path) -> Result<SecureStoreFile> {
    let Some(bytes) = read_bytes(path).await? else {
        return Ok(SecureStoreFile {
            version: STORE_VERSION,
            accounts: HashMap::new(),
        });
    };
    parse_secure_file(&bytes, path)
}

async fn get_secret_bytes(entry_name: &str) -> Result<Option<Vec<u8>>> {
    if let Some(path) = overridden_store_path() {
        if test_vault_is_unavailable(&path) {
            return Err(anyhow!("subscription test vault unavailable"));
        }
        return test_secrets()
            .lock()
            .map_err(|_| anyhow!("subscription test vault lock poisoned"))
            .map(|vault| {
                vault
                    .get(&path)
                    .and_then(|items| items.get(entry_name))
                    .cloned()
            });
    }

    let entry_name = entry_name.to_string();
    tokio::task::spawn_blocking(move || {
        let _guard = native_keyring_lock()
            .lock()
            .map_err(|_| "subscription keyring lock poisoned".to_string())?;
        let entry = keyring::Entry::new(KEYRING_SERVICE, &entry_name)
            .map_err(|err| format!("open system credential entry: {err}"))?;
        match entry.get_secret() {
            Ok(secret) => Ok(Some(secret)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(err) => Err(format!("read system credential entry: {err}")),
        }
    })
    .await
    .context("join system credential read task")?
    .map_err(anyhow::Error::msg)
}

/// Reads the v1 combined JSON entry. It was written through the password API,
/// which uses a platform-specific text encoding on Windows, so it cannot be
/// safely read through `get_secret` there.
async fn get_legacy_password(provider: &str) -> Result<Option<String>> {
    if let Some(path) = overridden_store_path() {
        if test_vault_is_unavailable(&path) {
            return Err(anyhow!("subscription test vault unavailable"));
        }
        return test_secrets()
            .lock()
            .map_err(|_| anyhow!("subscription test vault lock poisoned"))
            .map(|vault| {
                vault
                    .get(&path)
                    .and_then(|items| items.get(provider))
                    .and_then(|bytes| String::from_utf8(bytes.clone()).ok())
            });
    }

    let provider = provider.to_string();
    tokio::task::spawn_blocking(move || {
        let _guard = native_keyring_lock()
            .lock()
            .map_err(|_| "subscription keyring lock poisoned".to_string())?;
        let entry = keyring::Entry::new(KEYRING_SERVICE, &provider)
            .map_err(|err| format!("open system credential entry: {err}"))?;
        match entry.get_password() {
            Ok(secret) => Ok(Some(secret)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(err) => Err(format!("read legacy system credential entry: {err}")),
        }
    })
    .await
    .context("join legacy system credential read task")?
    .map_err(anyhow::Error::msg)
}

async fn set_secret_bytes(entry_name: &str, secret: Vec<u8>) -> Result<()> {
    if secret.len() > SECRET_CHUNK_BYTES {
        return Err(anyhow!(
            "subscription credential chunk exceeds portable size limit: {} bytes",
            secret.len()
        ));
    }
    if let Some(path) = overridden_store_path() {
        if test_vault_is_unavailable(&path) {
            return Err(anyhow!("subscription test vault unavailable"));
        }
        let mut vault = test_secrets()
            .lock()
            .map_err(|_| anyhow!("subscription test vault lock poisoned"))?;
        vault
            .entry(path)
            .or_default()
            .insert(entry_name.to_string(), secret);
        return Ok(());
    }

    let entry_name = entry_name.to_string();
    tokio::task::spawn_blocking(move || {
        let _guard = native_keyring_lock()
            .lock()
            .map_err(|_| "subscription keyring lock poisoned".to_string())?;
        let entry = keyring::Entry::new(KEYRING_SERVICE, &entry_name)
            .map_err(|err| format!("open system credential entry: {err}"))?;
        entry
            .set_secret(&secret)
            .map_err(|err| format!("write system credential entry: {err}"))
    })
    .await
    .context("join system credential write task")?
    .map_err(anyhow::Error::msg)
}

async fn delete_secret_entry(entry_name: &str) -> Result<()> {
    if let Some(path) = overridden_store_path() {
        if test_vault_is_unavailable(&path) {
            return Err(anyhow!("subscription test vault unavailable"));
        }
        if let Ok(mut vault) = test_secrets().lock() {
            if let Some(items) = vault.get_mut(&path) {
                items.remove(entry_name);
            }
        }
        return Ok(());
    }

    let entry_name = entry_name.to_string();
    tokio::task::spawn_blocking(move || {
        let _guard = native_keyring_lock()
            .lock()
            .map_err(|_| "subscription keyring lock poisoned".to_string())?;
        let entry = keyring::Entry::new(KEYRING_SERVICE, &entry_name)
            .map_err(|err| format!("open system credential entry: {err}"))?;
        match entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(err) => Err(format!("delete system credential entry: {err}")),
        }
    })
    .await
    .context("join system credential delete task")?
    .map_err(anyhow::Error::msg)
}

fn secret_chunks(secret: &str) -> Vec<Vec<u8>> {
    if secret.is_empty() {
        return vec![Vec::new()];
    }
    secret
        .as_bytes()
        .chunks(SECRET_CHUNK_BYTES)
        .map(<[u8]>::to_vec)
        .collect()
}

async fn read_chunked_field(
    provider: &str,
    set_id: &str,
    field: &str,
    parts: u32,
) -> Result<Option<String>> {
    if parts == 0 {
        return Ok(None);
    }
    let mut bytes = Vec::new();
    for entry_name in secret_entry_names(provider, set_id, field, parts) {
        let Some(mut part) = get_secret_bytes(&entry_name).await? else {
            return Ok(None);
        };
        bytes.append(&mut part);
    }
    match String::from_utf8(bytes) {
        Ok(secret) => Ok(Some(secret)),
        Err(error) => {
            log::warn!(
                "subscription credential vault chunks are invalid for provider {provider} field {field}: {error}"
            );
            Ok(None)
        }
    }
}

async fn read_secret_material(
    provider: &str,
    metadata: &CredentialMetadata,
) -> Result<Option<SecretMaterial>> {
    match metadata {
        CredentialMetadata::Oauth {
            secret_set_id: Some(set_id),
            refresh_parts,
            access_parts,
            ..
        } => {
            let Some(refresh) =
                read_chunked_field(provider, set_id, "refresh", *refresh_parts).await?
            else {
                return Ok(None);
            };
            let Some(access) =
                read_chunked_field(provider, set_id, "access", *access_parts).await?
            else {
                return Ok(None);
            };
            Ok(Some(SecretMaterial::Oauth { refresh, access }))
        }
        CredentialMetadata::Api {
            secret_set_id: Some(set_id),
            key_parts,
            ..
        } => Ok(read_chunked_field(provider, set_id, "api-key", *key_parts)
            .await?
            .map(|key| SecretMaterial::Api { key })),
        // Backward-compatible read of the original combined JSON password.
        _ => {
            let Some(secret) = get_legacy_password(provider).await? else {
                return Ok(None);
            };
            match serde_json::from_str(&secret) {
                Ok(material) => Ok(Some(material)),
                Err(error) => {
                    log::warn!(
                        "legacy subscription credential vault entry is invalid for provider {provider}: {error}"
                    );
                    Ok(None)
                }
            }
        }
    }
}

async fn write_secret_material(
    provider: &str,
    metadata: &CredentialMetadata,
    credential: &StoredCredential,
) -> Result<()> {
    let (set_id, fields): (&str, Vec<(&str, &str)>) = match (metadata, credential) {
        (
            CredentialMetadata::Oauth {
                secret_set_id: Some(set_id),
                ..
            },
            StoredCredential::Oauth {
                refresh, access, ..
            },
        ) => (set_id, vec![("refresh", refresh), ("access", access)]),
        (
            CredentialMetadata::Api {
                secret_set_id: Some(set_id),
                ..
            },
            StoredCredential::Api { key, .. },
        ) => (set_id, vec![("api-key", key)]),
        _ => return Err(anyhow!("subscription credential metadata type mismatch")),
    };

    let mut written: Vec<String> = Vec::new();
    for (field, value) in fields {
        for (index, chunk) in secret_chunks(value).into_iter().enumerate() {
            let entry_name = format!("{provider}/v2/{set_id}/{field}/{index}");
            if let Err(error) = set_secret_bytes(&entry_name, chunk).await {
                for previous in written {
                    let _ = delete_secret_entry(&previous).await;
                }
                return Err(error);
            }
            written.push(entry_name);
        }
    }
    Ok(())
}

async fn delete_secret_material(provider: &str, metadata: &CredentialMetadata) -> Result<()> {
    let mut first_error = None;
    for entry_name in metadata.vault_entries(provider) {
        if let Err(error) = delete_secret_entry(&entry_name).await {
            if first_error.is_none() {
                first_error = Some(error);
            }
        }
    }
    if let Some(error) = first_error {
        Err(error)
    } else {
        Ok(())
    }
}

async fn write_secure_file(path: &Path, file: &SecureStoreFile) -> Result<()> {
    if metadata_write_should_fail(path) {
        return Err(anyhow!("injected subscription metadata write failure"));
    }
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .with_context(|| format!("create subscription auth directory {}", parent.display()))?;
    }
    let bytes = serde_json::to_vec_pretty(file)?;
    write_atomic(path, &bytes).await
}

async fn migrate_legacy_store(path: &Path, legacy: Store) -> Result<LoadState> {
    let mut secure = SecureStoreFile {
        version: STORE_VERSION,
        accounts: HashMap::new(),
    };
    let mut written: Vec<(String, CredentialMetadata)> = Vec::new();
    for (provider, credential) in &legacy {
        let metadata = CredentialMetadata::from_credential(credential);
        if let Err(error) = write_secret_material(provider, &metadata, credential).await {
            for (previous_provider, previous_metadata) in written {
                let _ = delete_secret_material(&previous_provider, &previous_metadata).await;
            }
            // Never keep plaintext tokens after the security migration has
            // been attempted. Preserve account labels/expiry only and make the
            // required one-time sign-in explicit to the UI.
            secure.accounts = legacy
                .iter()
                .map(|(key, value)| {
                    (
                        key.clone(),
                        CredentialMetadata::requiring_reauthentication(value),
                    )
                })
                .collect();
            write_secure_file(path, &secure).await?;
            log::warn!(
                "subscription credential vault migration failed; plaintext credentials were removed and reauthentication is required: {error:#}"
            );
            return Ok(LoadState {
                credentials: Store::new(),
                requires_reauthentication: secure.accounts.keys().cloned().collect(),
                vault_unavailable: HashSet::new(),
            });
        }
        secure.accounts.insert(provider.clone(), metadata.clone());
        written.push((provider.clone(), metadata));
    }
    if let Err(error) = write_secure_file(path, &secure).await {
        for (provider, metadata) in written {
            let _ = delete_secret_material(&provider, &metadata).await;
        }
        return Err(error);
    }
    log::info!("subscription credentials migrated to the system credential vault");
    Ok(LoadState {
        credentials: legacy,
        requires_reauthentication: HashSet::new(),
        vault_unavailable: HashSet::new(),
    })
}

/// Loads credentials plus vault availability state. Legacy plaintext files are
/// migrated in place and immediately rewritten without secret fields.
async fn load_with_state_unlocked() -> Result<LoadState> {
    let path = store_path()?;
    let Some(bytes) = read_bytes(&path).await? else {
        return Ok(LoadState {
            credentials: Store::new(),
            requires_reauthentication: HashSet::new(),
            vault_unavailable: HashSet::new(),
        });
    };

    let secure = match parse_secure_file(&bytes, &path) {
        Ok(file) => file,
        Err(secure_error) => match serde_json::from_slice::<Store>(&bytes) {
            Ok(legacy) => return migrate_legacy_store(&path, legacy).await,
            Err(_) => return Err(secure_error),
        },
    };

    let mut credentials = Store::new();
    let mut requires_reauthentication = HashSet::new();
    let mut vault_unavailable = HashSet::new();
    for (provider, metadata) in secure.accounts {
        if metadata.needs_reauthentication() {
            requires_reauthentication.insert(provider);
            continue;
        }
        let material = match read_secret_material(&provider, &metadata).await {
            Ok(Some(material)) => material,
            Ok(None) => {
                requires_reauthentication.insert(provider);
                continue;
            }
            Err(error) => {
                log::warn!(
                    "subscription credential vault is unavailable for provider {provider}: {error:#}"
                );
                vault_unavailable.insert(provider);
                continue;
            }
        };
        if let Some(credential) = metadata.combine(material) {
            credentials.insert(provider, credential);
        } else {
            requires_reauthentication.insert(provider);
        }
    }
    Ok(LoadState {
        credentials,
        requires_reauthentication,
        vault_unavailable,
    })
}

/// Serializes discovery with migrations and metadata mutations so callers
/// never observe or race a partially rewritten credential index.
pub(crate) async fn load_with_state() -> Result<LoadState> {
    let _guard = store_operation_lock().lock().await;
    load_with_state_unlocked().await
}

/// Loads all credentials that are currently available from the system vault.
pub async fn load() -> Result<Store> {
    Ok(load_with_state().await?.credentials)
}

/// Loads one provider credential without exposing its secret in the metadata
/// file. `None` means the provider needs a new sign-in.
pub async fn load_entry(provider: &str) -> Result<Option<StoredCredential>> {
    let mut state = load_with_state().await?;
    if state.vault_unavailable.contains(provider) {
        return Err(anyhow!(
            "system credential vault is locked or unavailable; unlock it and retry"
        ));
    }
    Ok(state.credentials.remove(provider))
}

/// Inserts or replaces a provider credential. Secret material is committed to
/// the platform vault before the non-secret metadata advertises the entry.
pub async fn upsert(provider: &str, credential: StoredCredential) -> Result<()> {
    let _guard = store_operation_lock().lock().await;
    let path = store_path()?;
    // Trigger one-time migration before modifying an older file.
    let _ = load_with_state_unlocked().await?;
    let mut file = read_secure_file(&path).await?;
    let previous = file.accounts.get(provider).cloned();
    let metadata = CredentialMetadata::from_credential(&credential);
    write_secret_material(provider, &metadata, &credential).await?;
    file.accounts.insert(provider.to_string(), metadata.clone());
    if let Err(error) = write_secure_file(&path, &file).await {
        let _ = delete_secret_material(provider, &metadata).await;
        return Err(error);
    }
    if let Some(previous) = previous {
        if let Err(error) = delete_secret_material(provider, &previous).await {
            log::warn!(
                "remove superseded subscription credential chunks failed for provider {provider}: {error:#}"
            );
        }
    }
    Ok(())
}

/// Removes one provider from both the native vault and metadata index.
pub async fn remove(provider: &str) -> Result<()> {
    let _guard = store_operation_lock().lock().await;
    let path = store_path()?;
    let _ = load_with_state_unlocked().await?;
    let mut file = read_secure_file(&path).await?;
    let previous = file.accounts.remove(provider);
    // Commit the discovery-index removal first. If this write fails, the old
    // metadata and vault chunks remain a usable pair. A crash after this point
    // can leave only unreachable vault chunks, never a visible broken account.
    write_secure_file(&path, &file).await?;
    let cleanup = match previous.as_ref() {
        Some(metadata) => delete_secret_material(provider, metadata).await,
        None => delete_secret_entry(provider).await,
    };
    if let Err(error) = cleanup {
        log::warn!(
            "remove unreachable subscription credential chunks failed for provider {provider}: {error:#}"
        );
    }
    Ok(())
}

/// Persists all supplied credentials. Kept for compatibility with focused
/// tests; production refresh/login paths should call [`upsert`] for one
/// provider so concurrent providers cannot overwrite each other's tokens.
pub async fn save(store: &Store) -> Result<()> {
    for (provider, credential) in store {
        upsert(provider, credential.clone()).await?;
    }
    Ok(())
}

/// Writes `bytes` atomically (temp file + rename). Although the v2 file is
/// non-secret, restrictive Unix permissions protect account metadata too.
async fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    use tokio::io::AsyncWriteExt;

    let tmp = path.with_extension("tmp");
    #[cfg(unix)]
    let open_options = {
        let mut options = tokio::fs::OpenOptions::new();
        options.write(true).create(true).truncate(true).mode(0o600);
        options
    };
    #[cfg(not(unix))]
    let open_options = {
        let mut options = tokio::fs::OpenOptions::new();
        options.write(true).create(true).truncate(true);
        options
    };

    {
        let mut file = open_options
            .open(&tmp)
            .await
            .with_context(|| format!("create subscription auth temp file {}", tmp.display()))?;
        file.write_all(bytes)
            .await
            .with_context(|| format!("write subscription auth temp file {}", tmp.display()))?;
        file.sync_all()
            .await
            .with_context(|| format!("sync subscription auth temp file {}", tmp.display()))?;
    }
    replace_metadata_file(&tmp, path).await
}

#[cfg(not(windows))]
async fn replace_metadata_file(tmp: &Path, path: &Path) -> Result<()> {
    tokio::fs::rename(tmp, path).await.with_context(|| {
        format!(
            "rename subscription auth metadata {} -> {}",
            tmp.display(),
            path.display()
        )
    })
}

/// Windows does not reliably replace an existing destination with `rename`.
/// Rotate the prior metadata file to a backup first and restore it if moving
/// the newly-synced temp file into place fails.
#[cfg(windows)]
async fn replace_metadata_file(tmp: &Path, path: &Path) -> Result<()> {
    let backup = path.with_extension("bak");
    match tokio::fs::remove_file(&backup).await {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(error).with_context(|| {
                format!(
                    "remove stale subscription auth metadata backup {}",
                    backup.display()
                )
            });
        }
    }

    let had_existing = match tokio::fs::rename(path, &backup).await {
        Ok(()) => true,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
        Err(error) => {
            return Err(error).with_context(|| {
                format!(
                    "rotate subscription auth metadata {} -> {}",
                    path.display(),
                    backup.display()
                )
            });
        }
    };

    if let Err(error) = tokio::fs::rename(tmp, path).await {
        if had_existing {
            if let Err(restore_error) = tokio::fs::rename(&backup, path).await {
                return Err(anyhow!(
                    "replace subscription auth metadata failed: {error}; restoring {} also failed: {restore_error}",
                    backup.display()
                ));
            }
        }
        return Err(error).with_context(|| {
            format!(
                "rename subscription auth metadata {} -> {}",
                tmp.display(),
                path.display()
            )
        });
    }

    if had_existing {
        tokio::fs::remove_file(&backup).await.with_context(|| {
            format!(
                "remove subscription auth metadata backup {}",
                backup.display()
            )
        })?;
    }
    Ok(())
}
