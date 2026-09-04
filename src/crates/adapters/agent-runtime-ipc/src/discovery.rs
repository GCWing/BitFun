use fs2::FileExt;
use serde::{de::Error as _, Deserialize, Deserializer, Serialize, Serializer};
use sha2::{Digest, Sha256};
use std::fmt::{self, Write as _};
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::PROTOCOL_VERSION;

const MAX_DISCOVERY_BYTES: u64 = 16 * 1024;
const DISCOVERY_REPLACE_ATTEMPTS: usize = 6;
const DISCOVERY_READ_ATTEMPTS: usize = 4;

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct RuntimeInstanceIdentity(String);

impl RuntimeInstanceIdentity {
    pub fn for_workspace(
        workspace_root: &Path,
        product_identity: &str,
        release_channel: &str,
        user_identity: &str,
        protocol_version: u32,
    ) -> Result<Self, RuntimeIpcDiscoveryError> {
        validate_identity_part(product_identity)?;
        validate_identity_part(release_channel)?;
        validate_identity_part(user_identity)?;
        let canonical_workspace = dunce::canonicalize(workspace_root).map_err(|source| {
            RuntimeIpcDiscoveryError::CanonicalizeWorkspace {
                path: workspace_root.to_path_buf(),
                source,
            }
        })?;

        let mut hasher = Sha256::new();
        hasher.update(b"openbitfun-agent-runtime-instance-v2\0");
        for part in [product_identity, release_channel, user_identity] {
            hasher.update(part.as_bytes());
            hasher.update(b"\0");
        }
        hasher.update(protocol_version.to_le_bytes());
        hasher.update(b"\0");
        hash_canonical_path(&mut hasher, &canonical_workspace);
        let digest = hasher.finalize();
        let mut encoded = String::with_capacity(digest.len() * 2);
        for byte in digest {
            write!(&mut encoded, "{byte:02x}").expect("writing to String cannot fail");
        }
        Ok(Self(encoded))
    }

    pub fn parse(value: impl Into<String>) -> Result<Self, RuntimeIpcDiscoveryError> {
        let value = value.into();
        if value.len() != 64
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(RuntimeIpcDiscoveryError::InvalidInstanceIdentity);
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for RuntimeInstanceIdentity {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("RuntimeInstanceIdentity")
            .field(&self.0)
            .finish()
    }
}

impl Serialize for RuntimeInstanceIdentity {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for RuntimeInstanceIdentity {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::parse(value).map_err(D::Error::custom)
    }
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DiscoveryRecord {
    pub protocol_version: u32,
    pub instance_identity: RuntimeInstanceIdentity,
    pub endpoint: String,
    pub process_id: u32,
    pub token: String,
    pub owner_id: String,
}

impl DiscoveryRecord {
    pub fn new(
        instance_identity: RuntimeInstanceIdentity,
        endpoint: String,
        process_id: u32,
        token: String,
        owner_id: String,
    ) -> Self {
        Self {
            protocol_version: PROTOCOL_VERSION,
            instance_identity,
            endpoint,
            process_id,
            token,
            owner_id,
        }
    }
}

impl fmt::Debug for DiscoveryRecord {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DiscoveryRecord")
            .field("protocol_version", &self.protocol_version)
            .field("instance_identity", &self.instance_identity)
            .field("endpoint", &self.endpoint)
            .field("process_id", &self.process_id)
            .field("token", &"[REDACTED]")
            .field("owner_id", &self.owner_id)
            .finish()
    }
}

pub struct DiscoveryStore {
    root: PathBuf,
    identity: RuntimeInstanceIdentity,
}

impl DiscoveryStore {
    pub fn new(root: &Path, identity: RuntimeInstanceIdentity) -> Self {
        Self {
            root: root.to_path_buf(),
            identity,
        }
    }

    pub fn identity(&self) -> &RuntimeInstanceIdentity {
        &self.identity
    }

    pub fn read(&self) -> Result<Option<DiscoveryRecord>, RuntimeIpcDiscoveryError> {
        let path = self.record_path();
        let Some(mut file) = open_discovery_with_retry(&path)? else {
            return Ok(None);
        };
        let size = file
            .metadata()
            .map_err(|source| RuntimeIpcDiscoveryError::ReadDiscovery {
                path: path.clone(),
                source,
            })?
            .len();
        if size > MAX_DISCOVERY_BYTES {
            return Err(RuntimeIpcDiscoveryError::DiscoveryTooLarge { size });
        }
        let mut bytes = Vec::with_capacity(size as usize);
        file.read_to_end(&mut bytes)
            .map_err(|source| RuntimeIpcDiscoveryError::ReadDiscovery {
                path: path.clone(),
                source,
            })?;
        let record = serde_json::from_slice::<DiscoveryRecord>(&bytes)
            .map_err(RuntimeIpcDiscoveryError::InvalidDiscovery)?;
        if record.instance_identity != self.identity {
            return Err(RuntimeIpcDiscoveryError::WrongInstanceIdentity);
        }
        Ok(Some(record))
    }

    pub fn write(&self, record: &DiscoveryRecord) -> Result<(), RuntimeIpcDiscoveryError> {
        if record.instance_identity != self.identity {
            return Err(RuntimeIpcDiscoveryError::WrongInstanceIdentity);
        }
        ensure_private_directory(&self.root)?;
        let bytes =
            serde_json::to_vec(record).map_err(RuntimeIpcDiscoveryError::SerializeDiscovery)?;
        if bytes.len() as u64 > MAX_DISCOVERY_BYTES {
            return Err(RuntimeIpcDiscoveryError::DiscoveryTooLarge {
                size: bytes.len() as u64,
            });
        }
        let path = self.record_path();
        let mut temporary = tempfile::NamedTempFile::new_in(&self.root).map_err(|source| {
            RuntimeIpcDiscoveryError::WriteDiscovery {
                path: path.clone(),
                source,
            }
        })?;
        temporary
            .write_all(&bytes)
            .and_then(|_| temporary.as_file().sync_all())
            .map_err(|source| RuntimeIpcDiscoveryError::WriteDiscovery {
                path: path.clone(),
                source,
            })?;
        let temporary_path = temporary.into_temp_path();
        replace_discovery_atomically(temporary_path.as_ref(), &path)
            .map_err(|source| RuntimeIpcDiscoveryError::WriteDiscovery { path, source })
    }

    pub fn remove_if_owned(
        &self,
        expected: &DiscoveryRecord,
    ) -> Result<bool, RuntimeIpcDiscoveryError> {
        if self.read()?.as_ref() != Some(expected) {
            return Ok(false);
        }
        let path = self.record_path();
        match std::fs::remove_file(&path) {
            Ok(()) => Ok(true),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(source) => Err(RuntimeIpcDiscoveryError::RemoveDiscovery { path, source }),
        }
    }

    fn record_path(&self) -> PathBuf {
        self.root.join(format!("{}.json", self.identity.as_str()))
    }
}

pub struct RuntimeInstanceLock {
    file: File,
}

impl RuntimeInstanceLock {
    pub fn try_acquire(
        root: &Path,
        identity: &RuntimeInstanceIdentity,
    ) -> Result<Self, RuntimeIpcDiscoveryError> {
        ensure_private_directory(root)?;
        let path = root.join(format!("{}.instance.lock", identity.as_str()));
        let mut options = OpenOptions::new();
        options.create(true).truncate(false).read(true).write(true);
        configure_private_file(&mut options);
        let file =
            options
                .open(&path)
                .map_err(|source| RuntimeIpcDiscoveryError::OpenInstanceLock {
                    path: path.clone(),
                    source,
                })?;
        FileExt::try_lock_exclusive(&file)
            .map_err(|source| RuntimeIpcDiscoveryError::InstanceAlreadyOwned { source })?;
        Ok(Self { file })
    }
}

impl Drop for RuntimeInstanceLock {
    fn drop(&mut self) {
        let _ = FileExt::unlock(&self.file);
    }
}

#[derive(Debug, thiserror::Error)]
pub enum RuntimeIpcDiscoveryError {
    #[error(
        "runtime identity parts must be non-empty, bounded, and contain no control characters"
    )]
    InvalidIdentityPart,
    #[error("runtime instance identity must be a lowercase SHA-256 digest")]
    InvalidInstanceIdentity,
    #[error("failed to canonicalize runtime workspace {path}")]
    CanonicalizeWorkspace {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to create private runtime directory {path}")]
    CreateRuntimeDirectory {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to read runtime discovery record {path}")]
    ReadDiscovery {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to write runtime discovery record {path}")]
    WriteDiscovery {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to remove runtime discovery record {path}")]
    RemoveDiscovery {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("runtime discovery record exceeds {MAX_DISCOVERY_BYTES} bytes: {size}")]
    DiscoveryTooLarge { size: u64 },
    #[error("runtime discovery record is invalid")]
    InvalidDiscovery(#[source] serde_json::Error),
    #[error("failed to serialize runtime discovery record")]
    SerializeDiscovery(#[source] serde_json::Error),
    #[error("runtime discovery record belongs to another instance")]
    WrongInstanceIdentity,
    #[error("failed to open runtime instance lock {path}")]
    OpenInstanceLock {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("runtime instance is already owned")]
    InstanceAlreadyOwned {
        #[source]
        source: std::io::Error,
    },
}

fn validate_identity_part(value: &str) -> Result<(), RuntimeIpcDiscoveryError> {
    if value.is_empty() || value.len() > 128 || value.chars().any(char::is_control) {
        return Err(RuntimeIpcDiscoveryError::InvalidIdentityPart);
    }
    Ok(())
}

fn hash_canonical_path(hasher: &mut Sha256, path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        hasher.update(path.as_os_str().as_bytes());
    }
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        for unit in path.as_os_str().encode_wide() {
            hasher.update(unit.to_le_bytes());
        }
    }
}

fn ensure_private_directory(path: &Path) -> Result<(), RuntimeIpcDiscoveryError> {
    std::fs::create_dir_all(path).map_err(|source| {
        RuntimeIpcDiscoveryError::CreateRuntimeDirectory {
            path: path.to_path_buf(),
            source,
        }
    })?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700)).map_err(
            |source| RuntimeIpcDiscoveryError::CreateRuntimeDirectory {
                path: path.to_path_buf(),
                source,
            },
        )?;
    }
    Ok(())
}

fn configure_private_file(options: &mut OpenOptions) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    #[cfg(not(unix))]
    let _ = options;
}

fn open_discovery_for_read(path: &Path) -> std::io::Result<File> {
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        use windows::Win32::Storage::FileSystem::{
            FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE,
        };
        options.share_mode((FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE).0);
    }
    options.open(path)
}

fn open_discovery_with_retry(path: &Path) -> Result<Option<File>, RuntimeIpcDiscoveryError> {
    let mut last_transient_error = None;
    for attempt in 0..DISCOVERY_READ_ATTEMPTS {
        match open_discovery_for_read(path) {
            Ok(file) => return Ok(Some(file)),
            Err(error) if is_transient_discovery_read_error(&error) => {
                last_transient_error = Some(error);
                if attempt + 1 < DISCOVERY_READ_ATTEMPTS {
                    std::thread::sleep(Duration::from_millis(1));
                }
            }
            Err(source) => {
                return Err(RuntimeIpcDiscoveryError::ReadDiscovery {
                    path: path.to_path_buf(),
                    source,
                });
            }
        }
    }
    match last_transient_error {
        Some(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Some(source) => Err(RuntimeIpcDiscoveryError::ReadDiscovery {
            path: path.to_path_buf(),
            source,
        }),
        None => unreachable!("a non-empty read retry loop records a terminal result"),
    }
}

fn is_transient_discovery_read_error(error: &std::io::Error) -> bool {
    error.kind() == std::io::ErrorKind::NotFound
        || cfg!(windows) && matches!(error.raw_os_error(), Some(32 | 33))
}

fn replace_discovery_atomically(temporary: &Path, target: &Path) -> std::io::Result<()> {
    let mut last_error = None;
    for attempt in 0..DISCOVERY_REPLACE_ATTEMPTS {
        match replace_discovery_once(temporary, target) {
            Ok(()) => {
                sync_discovery_parent(target)?;
                return Ok(());
            }
            Err(error) if is_retryable_replace_error(&error) => {
                last_error = Some(error);
                if attempt + 1 < DISCOVERY_REPLACE_ATTEMPTS {
                    std::thread::sleep(Duration::from_millis(2 * (attempt as u64 + 1)));
                }
            }
            Err(error) => return Err(error),
        }
    }
    Err(last_error.expect("a bounded replace loop always records its final error"))
}

#[cfg(windows)]
fn replace_discovery_once(temporary: &Path, target: &Path) -> std::io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows::core::PCWSTR;
    use windows::Win32::Storage::FileSystem::{
        MoveFileExW, ReplaceFileW, MOVEFILE_WRITE_THROUGH, REPLACEFILE_WRITE_THROUGH,
    };

    let target_exists = target.exists();
    let temporary = temporary
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    let target = target
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    // SAFETY: both buffers are owned, NUL-terminated UTF-16 paths that remain
    // alive for the call. The temporary file is created in the target's
    // directory, so replacement cannot cross volumes.
    unsafe {
        if target_exists {
            ReplaceFileW(
                PCWSTR(target.as_ptr()),
                PCWSTR(temporary.as_ptr()),
                PCWSTR::null(),
                REPLACEFILE_WRITE_THROUGH,
                None,
                None,
            )
        } else {
            MoveFileExW(
                PCWSTR(temporary.as_ptr()),
                PCWSTR(target.as_ptr()),
                MOVEFILE_WRITE_THROUGH,
            )
        }
    }
    .map_err(|error| std::io::Error::other(error.to_string()))
}

#[cfg(not(windows))]
fn replace_discovery_once(temporary: &Path, target: &Path) -> std::io::Result<()> {
    std::fs::rename(temporary, target)
}

#[cfg(unix)]
fn sync_discovery_parent(target: &Path) -> std::io::Result<()> {
    File::open(
        target
            .parent()
            .ok_or_else(|| std::io::Error::other("discovery path has no parent"))?,
    )?
    .sync_all()
}

#[cfg(not(unix))]
fn sync_discovery_parent(_target: &Path) -> std::io::Result<()> {
    Ok(())
}

fn is_retryable_replace_error(error: &std::io::Error) -> bool {
    matches!(
        error.kind(),
        std::io::ErrorKind::PermissionDenied
            | std::io::ErrorKind::WouldBlock
            | std::io::ErrorKind::Interrupted
            | std::io::ErrorKind::TimedOut
            | std::io::ErrorKind::AlreadyExists
            | std::io::ErrorKind::Other
    )
}
