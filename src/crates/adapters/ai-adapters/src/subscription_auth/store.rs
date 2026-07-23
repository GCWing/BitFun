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
use std::collections::{BTreeSet, HashMap, HashSet};
use std::fmt;
use std::fs::{File, OpenOptions};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock, RwLock};
use std::time::Duration;

const STORE_VERSION: u8 = 2;
const CLEANUP_JOURNAL_VERSION: u8 = 1;
const KEYRING_SERVICE: &str = "openbitfun.bitfun.subscription-auth.v1";
const STORE_FILE_LOCK_TIMEOUT: Duration = Duration::from_secs(10);
const STORE_FILE_LOCK_RETRY_INTERVAL: Duration = Duration::from_millis(25);
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
        if self.needs_reauthentication() {
            return Vec::new();
        }
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
            // The old secure-vault representation used one password entry
            // named exactly after the provider.
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

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct SecureStoreFile {
    version: u8,
    #[serde(default)]
    accounts: HashMap<String, CredentialMetadata>,
    /// Monotonic provider epochs, retained even after an account is removed.
    ///
    /// The retained entry is a tombstone: an authorization or token refresh
    /// that began in another BitFun process before logout must not be able to
    /// publish its stale credential after logout has completed.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    provider_revisions: HashMap<String, u64>,
}

impl SecureStoreFile {
    fn active_vault_entries(&self) -> HashSet<String> {
        self.accounts
            .iter()
            .flat_map(|(provider, metadata)| metadata.vault_entries(provider))
            .collect()
    }

    fn provider_revision(&self, provider: &str) -> u64 {
        self.provider_revisions.get(provider).copied().unwrap_or(0)
    }

    fn advance_provider_revision(&mut self, provider: &str) -> Result<u64> {
        let next = self
            .provider_revision(provider)
            .checked_add(1)
            .ok_or_else(|| anyhow!("subscription credential revision overflow for {provider}"))?;
        self.provider_revisions.insert(provider.to_string(), next);
        Ok(next)
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct CleanupJournal {
    version: u8,
    #[serde(default)]
    entries: BTreeSet<String>,
}

#[derive(Debug)]
struct VaultUnavailableError(String);

impl fmt::Display for VaultUnavailableError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for VaultUnavailableError {}

fn vault_unavailable(message: impl Into<String>) -> anyhow::Error {
    VaultUnavailableError(message.into()).into()
}

fn is_vault_unavailable(error: &anyhow::Error) -> bool {
    error
        .chain()
        .any(|cause| cause.downcast_ref::<VaultUnavailableError>().is_some())
}

/// Result used by account discovery so a missing/locked vault entry is visible
/// to the UI instead of silently looking like a never-configured account.
pub(crate) struct LoadState {
    pub credentials: Store,
    pub requires_reauthentication: HashSet<String>,
    /// Metadata and secret entries exist, but the OS vault is currently
    /// locked/unavailable. This is retryable and must not be shown as lost.
    pub vault_unavailable: HashSet<String>,
    pub provider_revisions: HashMap<String, u64>,
}

/// One credential and the durable provider epoch observed in the same store
/// transaction. Callers that perform network refresh work must conditionally
/// commit against this revision instead of blindly resurrecting stale state.
pub(crate) struct VersionedCredential {
    pub credential: Option<StoredCredential>,
    pub revision: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ConditionalCommitOutcome {
    Committed { revision: u64 },
    Conflict { current_revision: u64 },
}

/// Result of removing an account from the committed metadata index. Native
/// vault cleanup can remain pending without making the account look connected.
#[derive(Debug)]
pub(crate) enum RemoveOutcome {
    Removed,
    CleanupPending(String),
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

#[cfg(test)]
fn failing_vault_writes_after() -> &'static Mutex<HashMap<PathBuf, usize>> {
    static WRITES: OnceLock<Mutex<HashMap<PathBuf, usize>>> = OnceLock::new();
    WRITES.get_or_init(|| Mutex::new(HashMap::new()))
}

#[cfg(test)]
fn failing_vault_deletes() -> &'static Mutex<HashSet<PathBuf>> {
    static PATHS: OnceLock<Mutex<HashSet<PathBuf>>> = OnceLock::new();
    PATHS.get_or_init(|| Mutex::new(HashSet::new()))
}

#[cfg(test)]
fn failing_backup_cleanup() -> &'static Mutex<HashSet<PathBuf>> {
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

/// Cross-process owner for the metadata, cleanup journal, and credential-vault
/// transaction. The lock file is stable and is never removed, because replacing
/// it could let two processes lock different inodes for the same store path.
struct StoreFileLock {
    file: File,
    path: PathBuf,
}

impl Drop for StoreFileLock {
    fn drop(&mut self) {
        if let Err(error) = fs2::FileExt::unlock(&self.file) {
            log::warn!(
                "release subscription credential transaction lock {} failed: {error}",
                self.path.display()
            );
        }
    }
}

/// Field order releases the OS lock before the in-process mutex, preserving the
/// global acquisition order for the next local transaction.
struct StoreTransactionGuard {
    _file_lock: StoreFileLock,
    _process_lock: tokio::sync::MutexGuard<'static, ()>,
}

fn store_lock_path(metadata_path: &Path) -> PathBuf {
    metadata_path.with_extension("lock")
}

#[cfg(unix)]
fn configure_store_lock_open(options: &mut OpenOptions) {
    use std::os::unix::fs::OpenOptionsExt;

    options.mode(0o600).custom_flags(libc::O_NOFOLLOW);
}

#[cfg(windows)]
fn configure_store_lock_open(options: &mut OpenOptions) {
    use std::os::windows::fs::OpenOptionsExt;

    // Open the reparse point itself so the regular-file check below rejects it
    // instead of following it to a different transaction-lock file.
    const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
    options.custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
}

#[cfg(not(any(unix, windows)))]
fn configure_store_lock_open(_options: &mut OpenOptions) {}

#[cfg(windows)]
fn store_lock_is_reparse_point(metadata: &std::fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;

    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
    metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(windows))]
fn store_lock_is_reparse_point(_metadata: &std::fs::Metadata) -> bool {
    false
}

fn open_store_lock_file(path: &Path) -> Result<File> {
    match std::fs::symlink_metadata(path) {
        Ok(metadata)
            if !metadata.is_file()
                || metadata.file_type().is_symlink()
                || store_lock_is_reparse_point(&metadata) =>
        {
            return Err(anyhow!(
                "subscription credential transaction lock {} must be a regular file, not a directory, symlink, or reparse point",
                path.display()
            ));
        }
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(error).with_context(|| {
                format!(
                    "inspect subscription credential transaction lock path {}",
                    path.display()
                )
            });
        }
    }

    let mut options = OpenOptions::new();
    options.create(true).truncate(false).read(true).write(true);
    configure_store_lock_open(&mut options);
    let file = options.open(path).with_context(|| {
        format!(
            "open subscription credential transaction lock {}",
            path.display()
        )
    })?;
    let metadata = file.metadata().with_context(|| {
        format!(
            "inspect subscription credential transaction lock {}",
            path.display()
        )
    })?;
    if !metadata.is_file() || store_lock_is_reparse_point(&metadata) {
        return Err(anyhow!(
            "subscription credential transaction lock {} must be a regular file, not a directory, symlink, or reparse point",
            path.display()
        ));
    }

    let path_metadata = std::fs::symlink_metadata(path).with_context(|| {
        format!(
            "inspect subscription credential transaction lock path {}",
            path.display()
        )
    })?;
    if !path_metadata.is_file()
        || path_metadata.file_type().is_symlink()
        || store_lock_is_reparse_point(&path_metadata)
    {
        return Err(anyhow!(
            "subscription credential transaction lock {} must be an ordinary file",
            path.display()
        ));
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        file.set_permissions(std::fs::Permissions::from_mode(0o600))
            .with_context(|| {
                format!(
                    "restrict subscription credential transaction lock permissions {}",
                    path.display()
                )
            })?;
    }
    Ok(file)
}

async fn acquire_store_file_lock_with_timeout(
    metadata_path: &Path,
    timeout: Duration,
) -> Result<StoreFileLock> {
    let lock_path = store_lock_path(metadata_path);
    let parent = lock_path.parent().ok_or_else(|| {
        anyhow!(
            "subscription credential transaction lock has no parent directory: {}",
            lock_path.display()
        )
    })?;
    let deadline = tokio::time::Instant::now() + timeout;
    tokio::time::timeout(timeout, tokio::fs::create_dir_all(parent))
        .await
        .map_err(|_| {
            anyhow!(
                "timed out creating subscription credential transaction lock directory {}",
                parent.display()
            )
        })?
        .with_context(|| {
            format!(
                "create subscription credential transaction lock directory {}",
                parent.display()
            )
        })?;

    let open_path = lock_path.clone();
    let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
    if remaining.is_zero() {
        return Err(anyhow!(
            "timed out waiting for subscription credential transaction lock {}",
            lock_path.display()
        ));
    }
    let file = tokio::time::timeout(
        remaining,
        tokio::task::spawn_blocking(move || open_store_lock_file(&open_path)),
    )
    .await
    .map_err(|_| {
        anyhow!(
            "timed out opening subscription credential transaction lock {}",
            lock_path.display()
        )
    })?
    .context("join subscription credential transaction lock open task")??;

    loop {
        match fs2::FileExt::try_lock_exclusive(&file) {
            Ok(()) => {
                if tokio::time::Instant::now() >= deadline {
                    let _ = fs2::FileExt::unlock(&file);
                    return Err(anyhow!(
                        "timed out waiting for subscription credential transaction lock {}; another BitFun process may be updating credentials",
                        lock_path.display()
                    ));
                }
                return Ok(StoreFileLock {
                    file,
                    path: lock_path,
                });
            }
            Err(error) if error.raw_os_error() == fs2::lock_contended_error().raw_os_error() => {
                let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
                if remaining.is_zero() {
                    return Err(anyhow!(
                        "timed out waiting for subscription credential transaction lock {}; another BitFun process may be updating credentials",
                        lock_path.display()
                    ));
                }
                tokio::time::sleep(remaining.min(STORE_FILE_LOCK_RETRY_INTERVAL)).await;
            }
            Err(error) => {
                return Err(error).with_context(|| {
                    format!(
                        "acquire subscription credential transaction lock {}",
                        lock_path.display()
                    )
                });
            }
        }
    }
}

async fn acquire_store_transaction() -> Result<(PathBuf, StoreTransactionGuard)> {
    // Lock order is process mutex first, OS file lock second everywhere. The
    // unlocked helpers below prevent recursive acquisition during migration.
    let process_lock = store_operation_lock().lock().await;
    let path = store_path()?;
    let file_lock = acquire_store_file_lock_with_timeout(&path, STORE_FILE_LOCK_TIMEOUT).await?;
    Ok((
        path,
        StoreTransactionGuard {
            _file_lock: file_lock,
            _process_lock: process_lock,
        },
    ))
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
pub(crate) fn set_test_vault_write_failure_after(successful_writes: Option<usize>) {
    let path = store_path_for_test_assertion();
    if let Ok(mut failures) = failing_vault_writes_after().lock() {
        match successful_writes {
            Some(count) => {
                failures.insert(path, count);
            }
            None => {
                failures.remove(&path);
            }
        }
    }
}

#[cfg(test)]
pub(crate) fn set_test_vault_delete_failure(fail: bool) {
    let path = store_path_for_test_assertion();
    if let Ok(mut paths) = failing_vault_deletes().lock() {
        if fail {
            paths.insert(path);
        } else {
            paths.remove(&path);
        }
    }
}

#[cfg(test)]
pub(crate) fn set_test_backup_cleanup_failure(path: &Path, fail: bool) {
    if let Ok(mut paths) = failing_backup_cleanup().lock() {
        if fail {
            paths.insert(path.to_path_buf());
        } else {
            paths.remove(path);
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

#[cfg(test)]
fn vault_write_should_fail(path: &Path) -> bool {
    failing_vault_writes_after()
        .lock()
        .map(|mut failures| {
            let Some(remaining) = failures.get_mut(path) else {
                return false;
            };
            if *remaining == 0 {
                true
            } else {
                *remaining -= 1;
                false
            }
        })
        .unwrap_or(true)
}

#[cfg(not(test))]
fn vault_write_should_fail(_path: &Path) -> bool {
    false
}

#[cfg(test)]
fn vault_delete_should_fail(path: &Path) -> bool {
    failing_vault_deletes()
        .lock()
        .map(|paths| paths.contains(path))
        .unwrap_or(true)
}

#[cfg(not(test))]
fn vault_delete_should_fail(_path: &Path) -> bool {
    false
}

#[cfg(test)]
fn backup_cleanup_should_fail(path: &Path) -> bool {
    failing_backup_cleanup()
        .lock()
        .map(|paths| paths.contains(path))
        .unwrap_or(true)
}

#[cfg(all(not(test), windows))]
fn backup_cleanup_should_fail(_path: &Path) -> bool {
    false
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
/// destination but before moving the new temp file into place. If the new file
/// is already present, scrub the stale backup instead; it can contain legacy
/// plaintext credentials from a migration.
#[cfg(windows)]
async fn restore_windows_backup_if_needed(path: &Path) -> Result<()> {
    if path.exists() {
        cleanup_stale_backup(path).await;
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

#[cfg(windows)]
async fn cleanup_stale_backup(path: &Path) {
    let backup = path.with_extension("bak");
    if !backup.exists() {
        return;
    }
    if let Err(error) = scrub_and_remove_file(&backup).await {
        log::warn!(
            "remove stale subscription auth metadata backup failed; cleanup will retry later: {error:#}"
        );
    }
}

#[cfg(any(windows, test))]
async fn scrub_and_remove_file(path: &Path) -> Result<()> {
    if backup_cleanup_should_fail(path) {
        return Err(anyhow!(
            "injected subscription metadata backup cleanup failure"
        ));
    }
    match tokio::fs::OpenOptions::new()
        .write(true)
        .truncate(true)
        .open(path)
        .await
    {
        Ok(file) => file
            .sync_all()
            .await
            .with_context(|| format!("sync scrubbed file {}", path.display()))?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error).with_context(|| format!("scrub file {}", path.display())),
    }
    tokio::fs::remove_file(path)
        .await
        .with_context(|| format!("remove scrubbed file {}", path.display()))
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
            provider_revisions: HashMap::new(),
        });
    };
    parse_secure_file(&bytes, path)
}

fn open_native_keyring_entry(entry_name: &str) -> std::result::Result<keyring_core::Entry, String> {
    if keyring_core::get_default_store().is_none() {
        #[cfg(target_os = "macos")]
        let store = apple_native_keyring_store::keychain::Store::new();
        #[cfg(target_os = "windows")]
        let store = windows_native_keyring_store::Store::new();
        #[cfg(all(
            unix,
            not(any(target_os = "macos", target_os = "ios", target_os = "android"))
        ))]
        let store = zbus_secret_service_keyring_store::Store::new();
        #[cfg(not(any(
            target_os = "macos",
            target_os = "windows",
            all(
                unix,
                not(any(target_os = "macos", target_os = "ios", target_os = "android"))
            )
        )))]
        let store: keyring_core::Result<std::sync::Arc<keyring_core::CredentialStore>> =
            Err(keyring_core::Error::NoDefaultStore);

        // Unlike the keyring v1 facade, failed initialization leaves no sticky
        // once flag. A later UI retry can reconnect to Linux Secret Service.
        let store =
            store.map_err(|error| format!("initialize system credential store: {error}"))?;
        keyring_core::set_default_store(store);
    }
    keyring_core::Entry::new(KEYRING_SERVICE, entry_name)
        .map_err(|error| format!("open system credential entry: {error}"))
}

async fn get_secret_bytes(entry_name: &str) -> Result<Option<Vec<u8>>> {
    if let Some(path) = overridden_store_path() {
        if test_vault_is_unavailable(&path) {
            return Err(vault_unavailable("subscription test vault unavailable"));
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
        let entry = open_native_keyring_entry(&entry_name)?;
        match entry.get_secret() {
            Ok(secret) => Ok(Some(secret)),
            Err(keyring_core::Error::NoEntry) => Ok(None),
            Err(err) => Err(format!("read system credential entry: {err}")),
        }
    })
    .await
    .context("join system credential read task")?
    .map_err(vault_unavailable)
}

/// Reads the v1 combined JSON entry. It was written through the password API,
/// which uses a platform-specific text encoding on Windows, so it cannot be
/// safely read through `get_secret` there.
async fn get_legacy_password(provider: &str) -> Result<Option<String>> {
    if let Some(path) = overridden_store_path() {
        if test_vault_is_unavailable(&path) {
            return Err(vault_unavailable("subscription test vault unavailable"));
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
        let entry = open_native_keyring_entry(&provider)?;
        match entry.get_password() {
            Ok(secret) => Ok(Some(secret)),
            Err(keyring_core::Error::NoEntry) => Ok(None),
            Err(err) => Err(format!("read legacy system credential entry: {err}")),
        }
    })
    .await
    .context("join legacy system credential read task")?
    .map_err(vault_unavailable)
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
            return Err(vault_unavailable("subscription test vault unavailable"));
        }
        if vault_write_should_fail(&path) {
            return Err(vault_unavailable(
                "injected subscription test vault write failure",
            ));
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
        let entry = open_native_keyring_entry(&entry_name)?;
        entry
            .set_secret(&secret)
            .map_err(|err| format!("write system credential entry: {err}"))
    })
    .await
    .context("join system credential write task")?
    .map_err(vault_unavailable)
}

async fn delete_secret_entry(entry_name: &str) -> Result<()> {
    if let Some(path) = overridden_store_path() {
        if test_vault_is_unavailable(&path) {
            return Err(vault_unavailable("subscription test vault unavailable"));
        }
        if vault_delete_should_fail(&path) {
            return Err(vault_unavailable(
                "injected subscription test vault delete failure",
            ));
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
        let entry = open_native_keyring_entry(&entry_name)?;
        match entry.delete_credential() {
            Ok(()) | Err(keyring_core::Error::NoEntry) => Ok(()),
            Err(err) => Err(format!("delete system credential entry: {err}")),
        }
    })
    .await
    .context("join system credential delete task")?
    .map_err(vault_unavailable)
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

    for (field, value) in fields {
        for (index, chunk) in secret_chunks(value).into_iter().enumerate() {
            let entry_name = format!("{provider}/v2/{set_id}/{field}/{index}");
            set_secret_bytes(&entry_name, chunk).await?;
        }
    }
    Ok(())
}

fn cleanup_journal_path(metadata_path: &Path) -> PathBuf {
    metadata_path.with_extension("cleanup.json")
}

async fn read_cleanup_journal(metadata_path: &Path) -> Result<CleanupJournal> {
    let path = cleanup_journal_path(metadata_path);
    let Some(bytes) = read_bytes(&path).await? else {
        return Ok(CleanupJournal {
            version: CLEANUP_JOURNAL_VERSION,
            entries: BTreeSet::new(),
        });
    };
    let journal: CleanupJournal = serde_json::from_slice(&bytes).with_context(|| {
        format!(
            "parse subscription credential cleanup journal {}",
            path.display()
        )
    })?;
    if journal.version != CLEANUP_JOURNAL_VERSION {
        return Err(anyhow!(
            "unsupported subscription credential cleanup journal version {} at {}",
            journal.version,
            path.display()
        ));
    }
    Ok(journal)
}

async fn write_cleanup_journal(metadata_path: &Path, journal: &CleanupJournal) -> Result<()> {
    let path = cleanup_journal_path(metadata_path);
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await.with_context(|| {
            format!(
                "create subscription credential cleanup journal directory {}",
                parent.display()
            )
        })?;
    }
    let bytes = serde_json::to_vec_pretty(journal)?;
    write_atomic(&path, &bytes).await
}

/// Durably records vault entries before an operation can make them unreachable.
/// Reconciliation always compares the journal with the committed metadata, so
/// a crash before metadata commit cannot delete the still-active credential.
async fn schedule_cleanup<I>(metadata_path: &Path, entries: I) -> Result<()>
where
    I: IntoIterator<Item = String>,
{
    let mut journal = read_cleanup_journal(metadata_path).await?;
    let before = journal.entries.len();
    journal.entries.extend(entries);
    if journal.entries.len() != before {
        write_cleanup_journal(metadata_path, &journal).await?;
    }
    Ok(())
}

/// Deletes only entries that are not referenced by the committed metadata.
/// Failed deletions remain durable in the journal for a later startup/retry.
async fn reconcile_cleanup_journal(
    metadata_path: &Path,
    active_entries: &HashSet<String>,
) -> Result<()> {
    let mut journal = read_cleanup_journal(metadata_path).await?;
    if journal.entries.is_empty() {
        return Ok(());
    }

    let mut first_error = None;
    let pending: Vec<String> = journal.entries.iter().cloned().collect();
    for entry_name in pending {
        if active_entries.contains(&entry_name) {
            journal.entries.remove(&entry_name);
            continue;
        }
        match delete_secret_entry(&entry_name).await {
            Ok(()) => {
                journal.entries.remove(&entry_name);
            }
            Err(error) => {
                if first_error.is_none() {
                    first_error = Some(error);
                }
            }
        }
    }
    write_cleanup_journal(metadata_path, &journal).await?;
    if let Some(error) = first_error {
        Err(error.context("subscription credential cleanup remains pending"))
    } else {
        Ok(())
    }
}

#[cfg(test)]
pub(crate) async fn cleanup_journal_entries_for_assertion() -> BTreeSet<String> {
    let path = store_path_for_test_assertion();
    read_cleanup_journal(&path)
        .await
        .map(|journal| journal.entries)
        .unwrap_or_default()
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
        provider_revisions: HashMap::new(),
    };

    for (provider, credential) in &legacy {
        secure.accounts.insert(
            provider.clone(),
            CredentialMetadata::from_credential(credential),
        );
    }
    let new_entries = secure.active_vault_entries();
    // Commit cleanup intent before the first vault write so a process stop can
    // never strand partially-written migration chunks.
    schedule_cleanup(path, new_entries.iter().cloned()).await?;

    for (provider, credential) in &legacy {
        let metadata = secure
            .accounts
            .get(provider)
            .expect("legacy migration metadata exists");
        if let Err(error) = write_secret_material(provider, metadata, credential).await {
            if let Err(cleanup_error) = reconcile_cleanup_journal(path, &HashSet::new()).await {
                log::warn!(
                    "cleanup after interrupted subscription credential migration remains pending: {cleanup_error:#}"
                );
            }

            // A locked or temporarily unavailable native vault must not turn
            // a retryable migration into permanent credential loss. Leave the
            // legacy file untouched and retry after the vault is available.
            if is_vault_unavailable(&error) {
                log::warn!(
                    "subscription credential vault migration deferred because the vault is unavailable: {error:#}"
                );
                return Ok(LoadState {
                    credentials: Store::new(),
                    requires_reauthentication: HashSet::new(),
                    vault_unavailable: legacy.keys().cloned().collect(),
                    provider_revisions: HashMap::new(),
                });
            }
            return Err(error.context("migrate subscription credential to the system vault"));
        }
    }

    if let Err(error) = write_secure_file(path, &secure).await {
        if let Err(cleanup_error) = reconcile_cleanup_journal(path, &HashSet::new()).await {
            log::warn!(
                "cleanup after failed subscription credential metadata migration remains pending: {cleanup_error:#}"
            );
        }
        return Err(error);
    }
    if let Err(error) = reconcile_cleanup_journal(path, &new_entries).await {
        // The entries are active after this commit. A later load can safely
        // remove their journal markers without deleting the credential.
        log::warn!(
            "finalize subscription credential migration cleanup journal failed; retrying later: {error:#}"
        );
    }
    log::info!("subscription credentials migrated to the system credential vault");
    Ok(LoadState {
        credentials: legacy,
        requires_reauthentication: HashSet::new(),
        vault_unavailable: HashSet::new(),
        provider_revisions: HashMap::new(),
    })
}

/// Loads credentials plus vault availability state. Legacy plaintext files are
/// migrated in place and immediately rewritten without secret fields.
async fn load_with_state_unlocked(path: &Path) -> Result<LoadState> {
    let Some(bytes) = read_bytes(path).await? else {
        if let Err(error) = reconcile_cleanup_journal(path, &HashSet::new()).await {
            log::warn!(
                "subscription credential cleanup remains pending while no accounts are configured: {error:#}"
            );
        }
        return Ok(LoadState {
            credentials: Store::new(),
            requires_reauthentication: HashSet::new(),
            vault_unavailable: HashSet::new(),
            provider_revisions: HashMap::new(),
        });
    };

    let secure = match parse_secure_file(&bytes, path) {
        Ok(file) => file,
        Err(secure_error) => match serde_json::from_slice::<Store>(&bytes) {
            Ok(legacy) => return migrate_legacy_store(path, legacy).await,
            Err(_) => return Err(secure_error),
        },
    };

    let active_entries = secure.active_vault_entries();
    let provider_revisions = secure.provider_revisions.clone();
    if let Err(error) = reconcile_cleanup_journal(path, &active_entries).await {
        log::warn!("subscription credential cleanup remains pending: {error:#}");
    }

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
        provider_revisions,
    })
}

/// Serializes discovery with migrations and metadata mutations so callers
/// never observe or race a partially rewritten credential index.
pub(crate) async fn load_with_state() -> Result<LoadState> {
    let (path, _transaction) = acquire_store_transaction().await?;
    load_with_state_unlocked(&path).await
}

/// Loads all credentials that are currently available from the system vault.
pub async fn load() -> Result<Store> {
    Ok(load_with_state().await?.credentials)
}

/// Loads one provider credential without exposing its secret in the metadata
/// file. `None` means the provider needs a new sign-in.
pub async fn load_entry(provider: &str) -> Result<Option<StoredCredential>> {
    Ok(load_entry_with_revision(provider).await?.credential)
}

/// Loads a credential and its provider tombstone revision from one locked
/// metadata/vault snapshot.
pub(crate) async fn load_entry_with_revision(provider: &str) -> Result<VersionedCredential> {
    let mut state = load_with_state().await?;
    if state.vault_unavailable.contains(provider) {
        return Err(anyhow!(
            "system credential vault is locked or unavailable; unlock it and retry"
        ));
    }
    Ok(VersionedCredential {
        credential: state.credentials.remove(provider),
        revision: state.provider_revisions.get(provider).copied().unwrap_or(0),
    })
}

/// Captures the provider epoch before a long-running authorization begins.
/// Logout retains and advances this value even when no credential is present.
pub(crate) async fn credential_revision(provider: &str) -> Result<u64> {
    let state = load_with_state().await?;
    Ok(state.provider_revisions.get(provider).copied().unwrap_or(0))
}

/// Inserts or replaces a provider credential. Secret material is committed to
/// the platform vault before the non-secret metadata advertises the entry.
pub async fn upsert(provider: &str, credential: StoredCredential) -> Result<()> {
    match upsert_internal(provider, credential, None).await? {
        ConditionalCommitOutcome::Committed { .. } => Ok(()),
        ConditionalCommitOutcome::Conflict { .. } => {
            unreachable!("unconditional subscription credential commit cannot conflict")
        }
    }
}

/// Commits only if no other process changed or logged out this provider since
/// `expected_revision` was captured. The comparison, vault write, metadata
/// revision advance, and metadata commit all run under the store OS lock.
pub(crate) async fn upsert_if_revision(
    provider: &str,
    expected_revision: u64,
    credential: StoredCredential,
) -> Result<ConditionalCommitOutcome> {
    upsert_internal(provider, credential, Some(expected_revision)).await
}

async fn upsert_internal(
    provider: &str,
    credential: StoredCredential,
    expected_revision: Option<u64>,
) -> Result<ConditionalCommitOutcome> {
    let (path, _transaction) = acquire_store_transaction().await?;
    // Trigger one-time migration before modifying an older file.
    let _ = load_with_state_unlocked(&path).await?;
    let mut file = read_secure_file(&path).await?;
    let current_revision = file.provider_revision(provider);
    if expected_revision.is_some_and(|expected| expected != current_revision) {
        return Ok(ConditionalCommitOutcome::Conflict { current_revision });
    }
    let next_revision = file.advance_provider_revision(provider)?;
    let active_before = file.active_vault_entries();
    let previous = file.accounts.get(provider).cloned();
    let metadata = CredentialMetadata::from_credential(&credential);
    let mut cleanup_entries = metadata.vault_entries(provider);
    let mut previous_entries = previous
        .as_ref()
        .map(|value| value.vault_entries(provider))
        .unwrap_or_default();
    if previous_entries.is_empty() {
        previous_entries.push(provider.to_string());
    }
    cleanup_entries.extend(previous_entries);
    schedule_cleanup(&path, cleanup_entries).await?;

    if let Err(error) = write_secret_material(provider, &metadata, &credential).await {
        if let Err(cleanup_error) = reconcile_cleanup_journal(&path, &active_before).await {
            log::warn!(
                "cleanup after failed subscription credential write remains pending for provider {provider}: {cleanup_error:#}"
            );
        }
        return Err(error);
    }
    file.accounts.insert(provider.to_string(), metadata.clone());
    if let Err(error) = write_secure_file(&path, &file).await {
        if let Err(cleanup_error) = reconcile_cleanup_journal(&path, &active_before).await {
            log::warn!(
                "cleanup after failed subscription metadata commit remains pending for provider {provider}: {cleanup_error:#}"
            );
        }
        return Err(error);
    }
    if let Err(error) = reconcile_cleanup_journal(&path, &file.active_vault_entries()).await {
        log::warn!(
            "remove superseded subscription credential chunks remains pending for provider {provider}: {error:#}"
        );
    }
    Ok(ConditionalCommitOutcome::Committed {
        revision: next_revision,
    })
}

/// Removes one provider from both the native vault and metadata index.
pub(crate) async fn remove(provider: &str) -> Result<RemoveOutcome> {
    let (path, _transaction) = acquire_store_transaction().await?;
    let _ = load_with_state_unlocked(&path).await?;
    let mut file = read_secure_file(&path).await?;
    // Advance even when the account is already absent. The persisted entry is
    // the logout tombstone that rejects authorizations captured beforehand in
    // another process.
    file.advance_provider_revision(provider)?;
    let active_before = file.active_vault_entries();
    let previous = file.accounts.get(provider).cloned();
    let mut cleanup_entries = previous
        .as_ref()
        .map(|metadata| metadata.vault_entries(provider))
        .unwrap_or_default();
    if cleanup_entries.is_empty() {
        cleanup_entries.push(provider.to_string());
    }
    // Persist cleanup intent before metadata can stop referencing the vault
    // entries. Reconciliation protects them if the metadata commit fails.
    schedule_cleanup(&path, cleanup_entries).await?;
    file.accounts.remove(provider);
    if let Err(error) = write_secure_file(&path, &file).await {
        if let Err(cleanup_error) = reconcile_cleanup_journal(&path, &active_before).await {
            log::warn!(
                "cleanup journal reconciliation after failed sign-out remains pending for provider {provider}: {cleanup_error:#}"
            );
        }
        return Err(error);
    }
    if let Err(error) = reconcile_cleanup_journal(&path, &file.active_vault_entries()).await {
        return Ok(RemoveOutcome::CleanupPending(format!(
            "native credential cleanup is pending; unlock the credential vault and retry: {error:#}"
        )));
    }
    Ok(RemoveOutcome::Removed)
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
    replace_metadata_file_windows(tmp, path).await
}

#[cfg(any(windows, test))]
pub(crate) async fn replace_metadata_file_windows(tmp: &Path, path: &Path) -> Result<()> {
    let backup = path.with_extension("bak");
    if backup.exists() {
        // A stale backup can be a pre-v2 plaintext credential file. Scrub it
        // before reusing the deterministic recovery path.
        scrub_and_remove_file(&backup).await.with_context(|| {
            format!(
                "remove stale subscription auth metadata backup {}",
                backup.display()
            )
        })?;
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
        // The new metadata is already committed. Backup cleanup must not be
        // reported as a failed commit: callers would otherwise delete the new
        // vault chunks. Keep it for a scrub-and-delete retry on the next read.
        if let Err(error) = scrub_and_remove_file(&backup).await {
            log::warn!(
                "remove committed subscription auth metadata backup failed; cleanup will retry later: {error:#}"
            );
        }
    }
    Ok(())
}

#[cfg(test)]
mod file_lock_tests {
    use super::*;

    const LOCK_CHILD_METADATA_ENV: &str = "BITFUN_SUBAUTH_LOCK_CHILD_METADATA";
    const LOCK_CHILD_STARTED_ENV: &str = "BITFUN_SUBAUTH_LOCK_CHILD_STARTED";
    const LOCK_CHILD_OBSERVED_ENV: &str = "BITFUN_SUBAUTH_LOCK_CHILD_OBSERVED";

    fn temporary_metadata_path(label: &str) -> PathBuf {
        std::env::temp_dir()
            .join(format!(
                "bitfun-subauth-lock-{label}-{}",
                uuid::Uuid::new_v4()
            ))
            .join("subscription_auth.json")
    }

    #[tokio::test]
    async fn independent_file_descriptors_contend_for_the_store_lock() {
        let metadata_path = temporary_metadata_path("descriptor-contention");
        let first = acquire_store_file_lock_with_timeout(&metadata_path, Duration::from_secs(1))
            .await
            .expect("first descriptor should acquire the transaction lock");

        let error =
            match acquire_store_file_lock_with_timeout(&metadata_path, Duration::from_millis(100))
                .await
            {
                Ok(_) => panic!("an independent descriptor must not bypass the held OS lock"),
                Err(error) => error,
            };
        assert!(error.to_string().contains("timed out waiting"));

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            let mode = std::fs::metadata(store_lock_path(&metadata_path))
                .expect("lock metadata")
                .permissions()
                .mode()
                & 0o777;
            assert_eq!(mode, 0o600);
        }

        drop(first);
        let _second = acquire_store_file_lock_with_timeout(&metadata_path, Duration::from_secs(1))
            .await
            .expect("lock should be released when its guard drops");
    }

    #[tokio::test]
    async fn second_file_transaction_cannot_reconcile_before_metadata_commit() {
        let metadata_path = temporary_metadata_path("commit-order");
        let first = acquire_store_file_lock_with_timeout(&metadata_path, Duration::from_secs(1))
            .await
            .expect("first transaction lock");
        let (attempting_tx, attempting_rx) = tokio::sync::oneshot::channel();
        let second_path = metadata_path.clone();
        let second = tokio::spawn(async move {
            let _ = attempting_tx.send(());
            let _lock =
                acquire_store_file_lock_with_timeout(&second_path, Duration::from_secs(5)).await?;
            reconcile_cleanup_journal(&second_path, &HashSet::new()).await?;
            let committed = read_secure_file(&second_path).await?;
            Ok::<u8, anyhow::Error>(committed.version)
        });

        attempting_rx.await.expect("second transaction started");
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert!(
            !second.is_finished(),
            "reconciliation must remain behind the first transaction lock"
        );
        assert!(!metadata_path.exists());

        write_secure_file(
            &metadata_path,
            &SecureStoreFile {
                version: STORE_VERSION,
                accounts: HashMap::new(),
                provider_revisions: HashMap::new(),
            },
        )
        .await
        .expect("commit metadata while the first transaction owns the lock");
        drop(first);

        let observed_version = tokio::time::timeout(Duration::from_secs(2), second)
            .await
            .expect("second transaction should proceed after commit")
            .expect("second transaction task")
            .expect("second transaction result");
        assert_eq!(observed_version, STORE_VERSION);
    }

    #[tokio::test]
    async fn cross_process_lock_child() {
        let Some(metadata_path) = std::env::var_os(LOCK_CHILD_METADATA_ENV).map(PathBuf::from)
        else {
            return;
        };
        let started_path = PathBuf::from(
            std::env::var_os(LOCK_CHILD_STARTED_ENV).expect("child started marker path"),
        );
        let observed_path = PathBuf::from(
            std::env::var_os(LOCK_CHILD_OBSERVED_ENV).expect("child observed marker path"),
        );
        std::fs::write(&started_path, b"started").expect("write child started marker");

        let _lock = acquire_store_file_lock_with_timeout(&metadata_path, Duration::from_secs(5))
            .await
            .expect("child transaction lock");
        // Keep a lock-regression failure inside the process-local test vault;
        // it must never touch the developer's native credential store.
        set_store_path_for_test(metadata_path.clone());
        let committed = read_secure_file(&metadata_path)
            .await
            .expect("child reads committed metadata");
        reconcile_cleanup_journal(&metadata_path, &committed.active_vault_entries())
            .await
            .expect("child reconciliation");
        std::fs::write(observed_path, committed.accounts.len().to_string())
            .expect("write child observation");
    }

    #[tokio::test]
    async fn separate_process_waits_for_metadata_commit_before_reconciliation() {
        let metadata_path = temporary_metadata_path("cross-process-commit-order");
        let first = acquire_store_file_lock_with_timeout(&metadata_path, Duration::from_secs(1))
            .await
            .expect("parent transaction lock");
        let parent = metadata_path.parent().expect("metadata parent");
        let started_path = parent.join("child-started");
        let observed_path = parent.join("child-observed");
        let committed_metadata = CredentialMetadata::Api {
            metadata: None,
            needs_reauthentication: false,
            secret_set_id: Some("committed-set".to_string()),
            key_parts: 1,
        };
        write_cleanup_journal(
            &metadata_path,
            &CleanupJournal {
                version: CLEANUP_JOURNAL_VERSION,
                entries: committed_metadata
                    .vault_entries("codex")
                    .into_iter()
                    .collect(),
            },
        )
        .await
        .expect("stage cleanup intent before vault write and metadata commit");
        let mut child = std::process::Command::new(std::env::current_exe().expect("test binary"))
            .arg("--exact")
            .arg("subscription_auth::store::file_lock_tests::cross_process_lock_child")
            .arg("--nocapture")
            .env(LOCK_CHILD_METADATA_ENV, &metadata_path)
            .env(LOCK_CHILD_STARTED_ENV, &started_path)
            .env(LOCK_CHILD_OBSERVED_ENV, &observed_path)
            .spawn()
            .expect("spawn lock-contending child process");

        tokio::time::timeout(Duration::from_secs(2), async {
            while !started_path.exists() {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("child should begin its transaction attempt");
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert!(
            !observed_path.exists(),
            "child process must not reconcile while the parent transaction is uncommitted"
        );

        write_secure_file(
            &metadata_path,
            &SecureStoreFile {
                version: STORE_VERSION,
                accounts: HashMap::from([("codex".to_string(), committed_metadata)]),
                provider_revisions: HashMap::new(),
            },
        )
        .await
        .expect("parent metadata commit");
        drop(first);

        let status = tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                if let Some(status) = child.try_wait().expect("poll child process") {
                    break status;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("child should finish after the parent releases the lock");
        assert!(status.success(), "child transaction failed: {status}");
        assert_eq!(
            std::fs::read_to_string(observed_path).expect("child observation"),
            "1"
        );
        assert!(read_cleanup_journal(&metadata_path)
            .await
            .expect("read reconciled journal")
            .entries
            .is_empty());
    }

    #[tokio::test]
    async fn transaction_lock_rejects_non_regular_paths() {
        let metadata_path = temporary_metadata_path("regular-file-check");
        let lock_path = store_lock_path(&metadata_path);
        std::fs::create_dir_all(&lock_path).expect("create directory at lock path");

        let error = match acquire_store_file_lock_with_timeout(
            &metadata_path,
            Duration::from_secs(1),
        )
        .await
        {
            Ok(_) => panic!("a directory must not be accepted as a lock file"),
            Err(error) => error,
        };
        let message = format!("{error:#}");
        assert!(message.contains("transaction lock"));
        assert!(message.contains("directory") || message.contains("regular file"));
    }
}
