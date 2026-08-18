//! Compatibility facade for managed plugin package review.
//!
//! Product assembly supplies BitFun path ownership. Concrete discovery,
//! integrity checks, locking, and trust persistence are service concerns.

use bitfun_services_integrations::plugin_source::ManagedPluginSourceService;
use std::path::Path;

pub type ManagedPluginTrustLevel =
    bitfun_services_integrations::plugin_source::ManagedPluginTrustLevel;
pub type ManagedPluginTrustDecision =
    bitfun_services_integrations::plugin_source::ManagedPluginTrustDecision;
pub type ManagedPluginPackageView =
    bitfun_services_integrations::plugin_source::ManagedPluginPackageView;
pub type ManagedPluginSourceIssue =
    bitfun_services_integrations::plugin_source::ManagedPluginSourceIssue;
pub type ManagedPluginSourceSnapshot =
    bitfun_services_integrations::plugin_source::ManagedPluginSourceSnapshot;
pub type ManagedPluginSourceError =
    bitfun_services_integrations::plugin_source::ManagedPluginSourceError;

/// Refresh BitFun-managed user and workspace package roots.
pub async fn refresh_managed_plugin_sources(
    workspace: &Path,
) -> Result<ManagedPluginSourceSnapshot, ManagedPluginSourceError> {
    Ok(managed_plugin_source_service(workspace)?
        .refresh(workspace)
        .await)
}

/// Apply a workspace-scoped trust decision without enabling package execution.
pub async fn set_managed_plugin_trust(
    workspace: &Path,
    package_id: &str,
    decision: ManagedPluginTrustDecision,
) -> Result<ManagedPluginSourceSnapshot, ManagedPluginSourceError> {
    managed_plugin_source_service(workspace)?
        .set_trust(workspace, package_id, decision)
        .await
}

/// Resolve the `(user, project)` managed plugin install target directories.
///
/// Used by CLI-local installers to place a fetched package into the right
/// plugin root. Dir resolution stays in core (it owns path management); the
/// host-specific fetch/extract work lives in the app layer.
pub fn managed_plugin_install_dirs(
    workspace: &Path,
) -> Result<(std::path::PathBuf, std::path::PathBuf), ManagedPluginSourceError> {
    let path_manager = crate::infrastructure::try_get_path_manager_arc().map_err(|error| {
        ManagedPluginSourceError::TrustStore(format!(
            "managed plugin product paths are unavailable: {error}"
        ))
    })?;
    Ok((
        path_manager.user_plugins_dir(),
        path_manager.project_plugins_dir(workspace),
    ))
}

pub(crate) fn managed_plugin_source_service(
    workspace: &Path,
) -> Result<ManagedPluginSourceService, ManagedPluginSourceError> {
    let path_manager = crate::infrastructure::try_get_path_manager_arc().map_err(|error| {
        ManagedPluginSourceError::TrustStore(format!(
            "managed plugin product paths are unavailable: {error}"
        ))
    })?;
    Ok(ManagedPluginSourceService::new(
        path_manager.user_plugins_dir(),
        path_manager.user_data_dir(),
        path_manager.project_plugins_dir(workspace),
        workspace.to_path_buf(),
        path_manager.project_plugin_trust_file(workspace),
    ))
}
