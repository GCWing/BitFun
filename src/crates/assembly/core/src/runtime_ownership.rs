//! First-party product assembly for local Agent Runtime ownership.
//!
//! The reusable lock primitive lives in `bitfun-services-core`. This owner
//! selects one deployment for the process, retains acquired workspace leases,
//! and keeps that deployment fact out of Agent Runtime SDK and wire contracts.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

pub use bitfun_services_core::runtime_ownership::RuntimeDeployment;
use bitfun_services_core::runtime_ownership::{
    RuntimeOwnershipError, RuntimeOwnershipKey, WorkspaceRuntimeOwnership,
};
use log::{info, warn};

use crate::infrastructure::PathManager;

const DEFAULT_PRODUCT_IDENTITY: &str = "bitfun";

enum CoreRuntimeOwnershipDeployment {
    Embedded {
        leases: Mutex<HashMap<RuntimeOwnershipKey, EmbeddedWorkspaceLease>>,
    },
    Shared {
        key: RuntimeOwnershipKey,
        _lease: WorkspaceRuntimeOwnership,
    },
}

struct EmbeddedWorkspaceLease {
    _lease: WorkspaceRuntimeOwnership,
    committed: bool,
    provisional_claims: usize,
}

/// A local workspace lease held while the workspace owner performs its open.
/// Dropping an uncommitted claim rolls back only a lease acquired for failed
/// in-flight opens; an already committed process lease remains process-bound.
pub(crate) struct ProvisionalLocalWorkspaceOwnership<'a> {
    owner: &'a CoreRuntimeOwnership,
    key: RuntimeOwnershipKey,
    provisional: bool,
}

impl ProvisionalLocalWorkspaceOwnership<'_> {
    pub(crate) fn commit(mut self) -> Result<(), CoreRuntimeOwnershipError> {
        if !self.provisional {
            return Ok(());
        }
        let CoreRuntimeOwnershipDeployment::Embedded { leases } = &self.owner.deployment else {
            return Ok(());
        };
        let mut leases = leases
            .lock()
            .map_err(|_| CoreRuntimeOwnershipError::OwnershipStateUnavailable)?;
        let lease = leases
            .get_mut(&self.key)
            .ok_or(CoreRuntimeOwnershipError::OwnershipStateUnavailable)?;
        lease.provisional_claims = lease.provisional_claims.saturating_sub(1);
        lease.committed = true;
        self.provisional = false;
        Ok(())
    }

    pub(crate) fn validate_workspace_path(
        &self,
        workspace: &Path,
    ) -> Result<(), CoreRuntimeOwnershipError> {
        let actual = RuntimeOwnershipKey::for_workspace(workspace, &self.owner.product_identity)?;
        if actual != self.key {
            return Err(CoreRuntimeOwnershipError::WorkspaceIdentityChangedDuringOpen);
        }
        Ok(())
    }
}

impl Drop for ProvisionalLocalWorkspaceOwnership<'_> {
    fn drop(&mut self) {
        if !self.provisional {
            return;
        }
        let CoreRuntimeOwnershipDeployment::Embedded { leases } = &self.owner.deployment else {
            return;
        };
        let Ok(mut leases) = leases.lock() else {
            return;
        };
        let should_remove = leases.get_mut(&self.key).is_some_and(|lease| {
            lease.provisional_claims = lease.provisional_claims.saturating_sub(1);
            !lease.committed && lease.provisional_claims == 0
        });
        if should_remove {
            leases.remove(&self.key);
        }
    }
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
            product_identity().to_string(),
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
            product_identity().to_string(),
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
                if let Some(lease) = leases.get_mut(&key) {
                    lease.committed = true;
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
                leases.insert(
                    key,
                    EmbeddedWorkspaceLease {
                        _lease: lease,
                        committed: true,
                        provisional_claims: 0,
                    },
                );
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

    /// Acquires a reversible local ownership claim for an in-flight workspace
    /// open. The caller commits only after the Workspace owner accepts the
    /// path; otherwise `Drop` releases a newly acquired process lease.
    pub(crate) fn begin_local_workspace_open(
        &self,
        workspace: &Path,
    ) -> Result<ProvisionalLocalWorkspaceOwnership<'_>, CoreRuntimeOwnershipError> {
        let key = RuntimeOwnershipKey::for_workspace(workspace, &self.product_identity)?;
        self.begin_local_workspace_with_key(key)
    }

    /// Acquires the same reversible claim for a product-managed workspace that
    /// has not been created yet. This prevents directory allocation from
    /// preceding Runtime ownership.
    pub(crate) fn begin_managed_local_workspace_creation(
        &self,
        workspace: &Path,
    ) -> Result<ProvisionalLocalWorkspaceOwnership<'_>, CoreRuntimeOwnershipError> {
        let key = RuntimeOwnershipKey::for_workspace_candidate(workspace, &self.product_identity)?;
        self.begin_local_workspace_with_key(key)
    }

    fn begin_local_workspace_with_key(
        &self,
        key: RuntimeOwnershipKey,
    ) -> Result<ProvisionalLocalWorkspaceOwnership<'_>, CoreRuntimeOwnershipError> {
        match &self.deployment {
            CoreRuntimeOwnershipDeployment::Embedded { leases } => {
                let mut leases = leases
                    .lock()
                    .map_err(|_| CoreRuntimeOwnershipError::OwnershipStateUnavailable)?;
                if let Some(lease) = leases.get_mut(&key) {
                    if lease.committed {
                        return Ok(ProvisionalLocalWorkspaceOwnership {
                            owner: self,
                            key,
                            provisional: false,
                        });
                    }
                    lease.provisional_claims += 1;
                    return Ok(ProvisionalLocalWorkspaceOwnership {
                        owner: self,
                        key,
                        provisional: true,
                    });
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
                leases.insert(
                    key.clone(),
                    EmbeddedWorkspaceLease {
                        _lease: lease,
                        committed: false,
                        provisional_claims: 1,
                    },
                );
                Ok(ProvisionalLocalWorkspaceOwnership {
                    owner: self,
                    key,
                    provisional: true,
                })
            }
            CoreRuntimeOwnershipDeployment::Shared {
                key: shared_key, ..
            } if shared_key == &key => Ok(ProvisionalLocalWorkspaceOwnership {
                owner: self,
                key,
                provisional: false,
            }),
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
        let key = RuntimeOwnershipKey::for_workspace(workspace, product_identity())?;
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
        let key = RuntimeOwnershipKey::for_workspace(workspace, product_identity())?;
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
        product_identity()
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
    #[error("workspace identity changed while Runtime ownership was being acquired")]
    WorkspaceIdentityChangedDuringOpen,
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
            Self::WorkspaceIdentityChangedDuringOpen => "workspace_identity_changed_during_open",
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
            RuntimeDeployment::Embedded if entrypoint == "cli-interactive" => "A Shared TUI Runtime owns this workspace; use `bitfun chat --shared`, or close its clients and wait up to 30 seconds",
            RuntimeDeployment::Embedded => "A Shared TUI Runtime owns this workspace; close its clients and wait up to 30 seconds before retrying this application",
            RuntimeDeployment::Shared => "An Embedded BitFun process owns this workspace; close it before using `--shared`",
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

fn product_identity() -> &'static str {
    option_env!("BITFUN_PRODUCT_BINARY_NAME").unwrap_or(DEFAULT_PRODUCT_IDENTITY)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uncommitted_embedded_workspace_claim_is_released() {
        let ownership_root = tempfile::tempdir().expect("ownership root");
        let workspace = tempfile::tempdir().expect("workspace");
        let owner = CoreRuntimeOwnership::embedded_with_facts(
            ownership_root.path().to_path_buf(),
            "bitfun".to_string(),
            "test",
        );

        let claim = owner
            .begin_local_workspace_open(workspace.path())
            .expect("provisional owner");
        drop(claim);

        let key =
            RuntimeOwnershipKey::for_workspace(workspace.path(), "bitfun").expect("ownership key");
        WorkspaceRuntimeOwnership::try_acquire(
            ownership_root.path(),
            &key,
            RuntimeDeployment::Shared,
        )
        .expect("a failed open must not retain its provisional lease");
    }

    #[test]
    fn one_successful_concurrent_claim_commits_the_process_lease() {
        let ownership_root = tempfile::tempdir().expect("ownership root");
        let workspace = tempfile::tempdir().expect("workspace");
        let owner = CoreRuntimeOwnership::embedded_with_facts(
            ownership_root.path().to_path_buf(),
            "bitfun".to_string(),
            "test",
        );

        let failed_open = owner
            .begin_local_workspace_open(workspace.path())
            .expect("first provisional owner");
        let successful_open = owner
            .begin_local_workspace_open(workspace.path())
            .expect("second provisional owner");
        successful_open.commit().expect("commit successful open");
        drop(failed_open);

        let key =
            RuntimeOwnershipKey::for_workspace(workspace.path(), "bitfun").expect("ownership key");
        assert!(matches!(
            WorkspaceRuntimeOwnership::try_acquire(
                ownership_root.path(),
                &key,
                RuntimeDeployment::Shared,
            ),
            Err(RuntimeOwnershipError::OwnershipUnavailable { .. })
        ));
    }

    #[test]
    fn managed_workspace_creation_is_claimed_before_the_directory_exists() {
        let ownership_root = tempfile::tempdir().expect("ownership root");
        let managed_root = tempfile::tempdir().expect("managed root");
        let workspace = managed_root
            .path()
            .join("personal_assistant")
            .join("workspace");
        let owner = CoreRuntimeOwnership::embedded_with_facts(
            ownership_root.path().to_path_buf(),
            "bitfun".to_string(),
            "test",
        );

        let claim = owner
            .begin_managed_local_workspace_creation(&workspace)
            .expect("claim managed workspace before creation");
        std::fs::create_dir_all(&workspace).expect("create managed workspace");
        claim
            .validate_workspace_path(&workspace)
            .expect("candidate key must match the created workspace");

        let key = RuntimeOwnershipKey::for_workspace(&workspace, "bitfun").expect("ownership key");
        assert!(matches!(
            WorkspaceRuntimeOwnership::try_acquire(
                ownership_root.path(),
                &key,
                RuntimeDeployment::Shared,
            ),
            Err(RuntimeOwnershipError::OwnershipUnavailable { .. })
        ));
    }
}
