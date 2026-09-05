//! First-party product assembly for local Agent Runtime ownership.
//!
//! The reusable lock primitive lives in `openbitfun-services-core`. This owner
//! selects one deployment for the process, retains acquired workspace leases,
//! and keeps that deployment fact out of Agent Runtime SDK and wire contracts.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use log::{info, warn};
use openbitfun_services_core::product_identity::product_id;
pub use openbitfun_services_core::runtime_ownership::RuntimeDeployment;
use openbitfun_services_core::runtime_ownership::{
    RuntimeOwnershipError, RuntimeOwnershipKey, WorkspaceRuntimeOwnership,
};

use crate::infrastructure::PathManager;

enum CoreRuntimeOwnershipDeployment {
    Embedded {
        leases: Mutex<HashMap<RuntimeOwnershipKey, WorkspaceRuntimeOwnership>>,
    },
    Shared {
        key: RuntimeOwnershipKey,
        _lease: WorkspaceRuntimeOwnership,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct VerifiedRemoteRuntimeScope {
    workspace_path: String,
    connection_id: String,
    ssh_host: Option<String>,
}

/// Process-lifetime owner for first-party local Agent Runtime workspaces.
pub struct CoreRuntimeOwnership {
    ownership_root: PathBuf,
    product_identity: String,
    entrypoint: &'static str,
    deployment: CoreRuntimeOwnershipDeployment,
    verified_remote_scopes: Mutex<HashSet<VerifiedRemoteRuntimeScope>>,
}

impl CoreRuntimeOwnership {
    /// Builds and acquires the process owner for a fixed local workspace.
    pub fn fixed_workspace(
        path_manager: &PathManager,
        entrypoint: &'static str,
        workspace: &Path,
        deployment: RuntimeDeployment,
    ) -> Result<Self, CoreRuntimeOwnershipError> {
        match deployment {
            RuntimeDeployment::Embedded => {
                let owner = Self::embedded(path_manager, entrypoint);
                owner.ensure_local_workspace(workspace)?;
                Ok(owner)
            }
            RuntimeDeployment::Shared => Self::shared(path_manager, entrypoint, workspace),
        }
    }

    /// Builds the normal first-party Embedded deployment.
    pub fn embedded(path_manager: &PathManager, entrypoint: &'static str) -> Self {
        Self::embedded_with_facts(
            path_manager.agent_runtime_ownership_dir(),
            product_id().to_string(),
            entrypoint,
        )
    }

    /// Builds the opt-in single-workspace Shared deployment and acquires its
    /// exclusive lease before any Agent Runtime is initialized.
    pub fn shared(
        path_manager: &PathManager,
        entrypoint: &'static str,
        workspace: &Path,
    ) -> Result<Self, CoreRuntimeOwnershipError> {
        Self::shared_with_facts(
            path_manager.agent_runtime_ownership_dir(),
            product_id().to_string(),
            entrypoint,
            workspace,
        )
    }

    pub(crate) fn embedded_with_facts(
        ownership_root: PathBuf,
        product_identity: String,
        entrypoint: &'static str,
    ) -> Self {
        Self {
            ownership_root,
            product_identity,
            entrypoint,
            deployment: CoreRuntimeOwnershipDeployment::Embedded {
                leases: Mutex::new(HashMap::new()),
            },
            verified_remote_scopes: Mutex::new(HashSet::new()),
        }
    }

    pub(crate) fn shared_with_facts(
        ownership_root: PathBuf,
        product_identity: String,
        entrypoint: &'static str,
        workspace: &Path,
    ) -> Result<Self, CoreRuntimeOwnershipError> {
        let key = RuntimeOwnershipKey::for_workspace(workspace, &product_identity)?;
        let lease = WorkspaceRuntimeOwnership::try_acquire(
            &ownership_root,
            &key,
            RuntimeDeployment::Shared,
        )
        .inspect_err(|error| {
            log_acquisition_failure(entrypoint, RuntimeDeployment::Shared, &key, error);
        })?;
        log_acquired(entrypoint, RuntimeDeployment::Shared, &key);
        Ok(Self {
            ownership_root,
            product_identity,
            entrypoint,
            deployment: CoreRuntimeOwnershipDeployment::Shared { key, _lease: lease },
            verified_remote_scopes: Mutex::new(HashSet::new()),
        })
    }

    /// Records a Remote workspace binding resolved by the Workspace owner.
    /// Raw transport strings are never sufficient to bypass local ownership.
    pub(crate) fn register_verified_remote_scope(
        &self,
        workspace: &Path,
        connection_id: &str,
        ssh_host: Option<&str>,
    ) -> Result<(), CoreRuntimeOwnershipError> {
        let scope = verified_remote_scope(workspace, connection_id, ssh_host)?;
        self.verified_remote_scopes
            .lock()
            .map_err(|_| CoreRuntimeOwnershipError::OwnershipStateUnavailable)?
            .insert(scope);
        Ok(())
    }

    /// Acquires the local workspace unless structured remote facts assign
    /// execution ownership to another host.
    pub fn ensure_workspace_scope(
        &self,
        workspace: &Path,
        remote_connection_id: Option<&str>,
        remote_ssh_host: Option<&str>,
    ) -> Result<(), CoreRuntimeOwnershipError> {
        if let Some(connection_id) = remote_connection_id
            .map(str::trim)
            .filter(|connection_id| !connection_id.is_empty())
        {
            let requested = verified_remote_scope(workspace, connection_id, remote_ssh_host)?;
            let verified = self
                .verified_remote_scopes
                .lock()
                .map_err(|_| CoreRuntimeOwnershipError::OwnershipStateUnavailable)?
                .iter()
                .any(|known| remote_scope_matches(known, &requested));
            if verified {
                return Ok(());
            }
            return Err(CoreRuntimeOwnershipError::UnverifiedRemoteWorkspaceScope);
        }
        self.ensure_local_workspace(workspace)
    }

    /// Idempotently retains ownership of one local workspace for this process.
    pub fn ensure_local_workspace(
        &self,
        workspace: &Path,
    ) -> Result<(), CoreRuntimeOwnershipError> {
        let key = RuntimeOwnershipKey::for_workspace(workspace, &self.product_identity)?;
        match &self.deployment {
            CoreRuntimeOwnershipDeployment::Embedded { leases } => {
                let mut leases = leases
                    .lock()
                    .map_err(|_| CoreRuntimeOwnershipError::OwnershipStateUnavailable)?;
                if leases.contains_key(&key) {
                    return Ok(());
                }
                let lease = WorkspaceRuntimeOwnership::try_acquire(
                    &self.ownership_root,
                    &key,
                    RuntimeDeployment::Embedded,
                )
                .inspect_err(|error| {
                    log_acquisition_failure(
                        self.entrypoint,
                        RuntimeDeployment::Embedded,
                        &key,
                        error,
                    );
                })?;
                log_acquired(self.entrypoint, RuntimeDeployment::Embedded, &key);
                leases.insert(key, lease);
                Ok(())
            }
            CoreRuntimeOwnershipDeployment::Shared {
                key: shared_key, ..
            } if shared_key == &key => Ok(()),
            CoreRuntimeOwnershipDeployment::Shared { .. } => {
                warn!(
                    "Shared Agent Runtime rejected a second local workspace: entrypoint={}, error_code=shared_runtime_workspace_mismatch",
                    self.entrypoint
                );
                Err(CoreRuntimeOwnershipError::SharedRuntimeWorkspaceMismatch)
            }
        }
    }

    /// Tests whether another local Runtime currently owns this workspace.
    pub fn runtime_owner_present(
        path_manager: &PathManager,
        workspace: &Path,
    ) -> Result<bool, CoreRuntimeOwnershipError> {
        let key = RuntimeOwnershipKey::for_workspace(workspace, product_id())?;
        match WorkspaceRuntimeOwnership::try_acquire(
            &path_manager.agent_runtime_ownership_dir(),
            &key,
            RuntimeDeployment::Shared,
        ) {
            Ok(_) => Ok(false),
            Err(RuntimeOwnershipError::OwnershipUnavailable { .. }) => Ok(true),
            Err(error) => Err(error.into()),
        }
    }

    /// Distinguishes compatible Embedded shared locks from a Shared Runtime's
    /// exclusive lock without publishing another deployment protocol.
    pub fn embedded_runtime_owner_present(
        path_manager: &PathManager,
        workspace: &Path,
    ) -> Result<bool, CoreRuntimeOwnershipError> {
        let key = RuntimeOwnershipKey::for_workspace(workspace, product_id())?;
        let ownership_root = path_manager.agent_runtime_ownership_dir();
        match WorkspaceRuntimeOwnership::try_acquire(
            &ownership_root,
            &key,
            RuntimeDeployment::Shared,
        ) {
            Ok(_) => Ok(false),
            Err(RuntimeOwnershipError::OwnershipUnavailable { .. }) => {
                match WorkspaceRuntimeOwnership::try_acquire(
                    &ownership_root,
                    &key,
                    RuntimeDeployment::Embedded,
                ) {
                    Ok(_) => Ok(true),
                    Err(RuntimeOwnershipError::OwnershipUnavailable { .. }) => Ok(false),
                    Err(error) => Err(error.into()),
                }
            }
            Err(error) => Err(error.into()),
        }
    }

    /// Product-wide identity used by ownership and private first-party IPC.
    pub fn distribution_identity() -> &'static str {
        product_id()
    }

    pub fn error_message(&self, error: &CoreRuntimeOwnershipError) -> String {
        let deployment = match &self.deployment {
            CoreRuntimeOwnershipDeployment::Embedded { .. } => RuntimeDeployment::Embedded,
            CoreRuntimeOwnershipDeployment::Shared { .. } => RuntimeDeployment::Shared,
        };
        error.startup_message(deployment, self.entrypoint)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum CoreRuntimeOwnershipError {
    #[error(transparent)]
    Primitive(#[from] RuntimeOwnershipError),
    #[error("runtime ownership state is unavailable")]
    OwnershipStateUnavailable,
    #[error("Shared Agent Runtime is limited to its startup workspace")]
    SharedRuntimeWorkspaceMismatch,
    #[error("remote workspace binding was not verified by the Workspace owner")]
    UnverifiedRemoteWorkspaceScope,
}

impl CoreRuntimeOwnershipError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Primitive(error) => error.code(),
            Self::OwnershipStateUnavailable => "ownership_state_unavailable",
            Self::SharedRuntimeWorkspaceMismatch => "shared_runtime_workspace_mismatch",
            Self::UnverifiedRemoteWorkspaceScope => "unverified_remote_workspace_scope",
        }
    }

    pub fn startup_message(&self, deployment: RuntimeDeployment, entrypoint: &str) -> String {
        let prefix = format!("Agent Runtime ownership failed ({}): {self}", self.code());
        if !matches!(
            self,
            Self::Primitive(RuntimeOwnershipError::OwnershipUnavailable { .. })
        ) {
            return prefix;
        }
        let guidance = match deployment {
            RuntimeDeployment::Embedded if entrypoint == "cli-interactive" => "A Shared TUI Runtime owns this workspace; use `openbitfun chat --shared`, or close its clients and wait up to 30 seconds",
            RuntimeDeployment::Embedded => "A Shared TUI Runtime owns this workspace; close its clients and wait up to 30 seconds before retrying this application",
            RuntimeDeployment::Shared => "An Embedded OpenBitFun process owns this workspace; close it before using `--shared`",
        };
        format!("{prefix}. {guidance}")
    }
}

fn verified_remote_scope(
    workspace: &Path,
    connection_id: &str,
    ssh_host: Option<&str>,
) -> Result<VerifiedRemoteRuntimeScope, CoreRuntimeOwnershipError> {
    let connection_id = connection_id.trim();
    if connection_id.is_empty() {
        return Err(CoreRuntimeOwnershipError::UnverifiedRemoteWorkspaceScope);
    }
    let mut workspace_path = workspace.to_string_lossy().replace('\\', "/");
    while workspace_path.len() > 1 && workspace_path.ends_with('/') {
        workspace_path.pop();
    }
    if workspace_path.is_empty() {
        return Err(CoreRuntimeOwnershipError::UnverifiedRemoteWorkspaceScope);
    }
    Ok(VerifiedRemoteRuntimeScope {
        workspace_path,
        connection_id: connection_id.to_string(),
        ssh_host: ssh_host
            .map(str::trim)
            .filter(|host| !host.is_empty())
            .map(str::to_ascii_lowercase),
    })
}

fn remote_scope_matches(
    known: &VerifiedRemoteRuntimeScope,
    requested: &VerifiedRemoteRuntimeScope,
) -> bool {
    known.workspace_path == requested.workspace_path
        && known.connection_id == requested.connection_id
        && requested
            .ssh_host
            .as_ref()
            .is_none_or(|host| known.ssh_host.as_ref() == Some(host))
}

fn log_acquired(entrypoint: &str, deployment: RuntimeDeployment, key: &RuntimeOwnershipKey) {
    info!(
        "Agent Runtime ownership acquired: deployment={}, entrypoint={}, ownership_key_prefix={}",
        deployment,
        entrypoint,
        key_prefix(key)
    );
}

fn log_acquisition_failure(
    entrypoint: &str,
    deployment: RuntimeDeployment,
    key: &RuntimeOwnershipKey,
    error: &RuntimeOwnershipError,
) {
    warn!(
        "Agent Runtime ownership unavailable: deployment={}, entrypoint={}, error_code={}, ownership_key_prefix={}",
        deployment,
        entrypoint,
        error.code(),
        key_prefix(key)
    );
}

fn key_prefix(key: &RuntimeOwnershipKey) -> &str {
    key.as_str().get(..12).unwrap_or(key.as_str())
}
