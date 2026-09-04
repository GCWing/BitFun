//! Process-level ownership primitives for local Agent Runtime deployments.
//!
//! This module does not select a workspace or start an Agent Runtime. Product
//! assembly may use the shared/exclusive lock to prevent an embedded runtime
//! and a future shared runtime from owning the same workspace simultaneously.

use crate::file_lock::{FileLock, FileLockError, FileLockMode};
use sha2::{Digest, Sha256};
use std::fmt::{self, Write as _};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeDeployment {
    Embedded,
    Shared,
}

impl RuntimeDeployment {
    fn as_str(self) -> &'static str {
        match self {
            Self::Embedded => "embedded",
            Self::Shared => "shared",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RuntimeOwnershipKey(String);

impl RuntimeOwnershipKey {
    pub fn for_workspace(
        workspace_root: &Path,
        product_identity: &str,
    ) -> Result<Self, RuntimeOwnershipError> {
        validate_product_identity(product_identity)?;
        let canonical_workspace = dunce::canonicalize(workspace_root).map_err(|source| {
            RuntimeOwnershipError::CanonicalizeWorkspace {
                path: workspace_root.to_path_buf(),
                source,
            }
        })?;

        let mut hasher = Sha256::new();
        hasher.update(b"openbitfun-runtime-ownership-v2\0");
        hasher.update(product_identity.as_bytes());
        hasher.update(b"\0");
        hash_canonical_path(&mut hasher, &canonical_workspace);
        let digest = hasher.finalize();
        let mut encoded = String::with_capacity(digest.len() * 2);
        for byte in digest {
            write!(&mut encoded, "{byte:02x}").expect("writing to String cannot fail");
        }
        Ok(Self(encoded))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    fn lock_path(&self, ownership_root: &Path) -> PathBuf {
        ownership_root.join(format!("{}.lock", self.0))
    }
}

pub struct WorkspaceRuntimeOwnership {
    deployment: RuntimeDeployment,
    _lock: FileLock,
}

impl WorkspaceRuntimeOwnership {
    pub fn try_acquire(
        ownership_root: &Path,
        key: &RuntimeOwnershipKey,
        deployment: RuntimeDeployment,
    ) -> Result<Self, RuntimeOwnershipError> {
        std::fs::create_dir_all(ownership_root).map_err(|source| {
            RuntimeOwnershipError::CreateOwnershipDirectory {
                path: ownership_root.to_path_buf(),
                source,
            }
        })?;
        let path = key.lock_path(ownership_root);
        let mode = match deployment {
            RuntimeDeployment::Embedded => FileLockMode::Shared,
            RuntimeDeployment::Shared => FileLockMode::Exclusive,
        };
        let lock = FileLock::try_acquire(&path, mode).map_err(|error| match error {
            FileLockError::Open(source) => RuntimeOwnershipError::OpenLockFile {
                path: path.clone(),
                source,
            },
            FileLockError::Unavailable(source) => {
                RuntimeOwnershipError::OwnershipUnavailable { deployment, source }
            }
        })?;

        Ok(Self {
            deployment,
            _lock: lock,
        })
    }

    pub fn deployment(&self) -> RuntimeDeployment {
        self.deployment
    }
}

impl fmt::Debug for WorkspaceRuntimeOwnership {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("WorkspaceRuntimeOwnership")
            .field("deployment", &self.deployment)
            .finish_non_exhaustive()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum RuntimeOwnershipError {
    #[error("product identity must be non-empty, bounded, and contain no control characters")]
    InvalidProductIdentity,
    #[error("failed to canonicalize runtime workspace {path}")]
    CanonicalizeWorkspace {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to create runtime ownership directory {path}")]
    CreateOwnershipDirectory {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to open runtime ownership lock {path}")]
    OpenLockFile {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("{deployment} runtime ownership is unavailable")]
    OwnershipUnavailable {
        deployment: RuntimeDeployment,
        #[source]
        source: std::io::Error,
    },
}

impl RuntimeOwnershipError {
    /// Stable low-cardinality classification for product diagnostics and logs.
    pub fn code(&self) -> &'static str {
        match self {
            Self::InvalidProductIdentity => "invalid_product_identity",
            Self::CanonicalizeWorkspace { .. } => "canonicalize_workspace_failed",
            Self::CreateOwnershipDirectory { .. } => "ownership_root_create_failed",
            Self::OpenLockFile { .. } => "ownership_lock_open_failed",
            Self::OwnershipUnavailable { .. } => "runtime_ownership_unavailable",
        }
    }
}

impl fmt::Display for RuntimeDeployment {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

fn validate_product_identity(value: &str) -> Result<(), RuntimeOwnershipError> {
    if value.is_empty() || value.len() > 128 || value.chars().any(char::is_control) {
        return Err(RuntimeOwnershipError::InvalidProductIdentity);
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
