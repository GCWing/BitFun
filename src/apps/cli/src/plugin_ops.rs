//! CLI-local plugin backend operations.
//!
//! Bridges the TUI plugin browser to bitfun-core's managed plugin source and
//! runtime APIs. Lives outside the boundary-scanned `ui/` and `modes/chat/`
//! trees so the TUI backend direct-call ratchet stays clean; the UI layer
//! consumes the plain `PluginItem` / `PluginInstallScope` /
//! `PluginDisplayStatus` types and the operations exposed here, never
//! importing bitfun-core directly.

use std::path::Path;

use bitfun_core::plugin_runtime::{activate_managed_plugin, deactivate_managed_plugin};
use bitfun_core::plugin_source::{
    refresh_managed_plugin_sources, ManagedPluginPackageView, ManagedPluginSourceSnapshot,
    ManagedPluginTrustLevel,
};

/// Three-state display status for a plugin, mirroring the deveco-code
/// plugin manager: `active` (green), `inactive` (red, approved but not
/// running), `disabled` (gray, denied/revoked), `unreviewed` (yellow).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PluginDisplayStatus {
    Active,
    Inactive,
    Disabled,
    Unreviewed,
}

impl PluginDisplayStatus {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Inactive => "inactive",
            Self::Disabled => "disabled",
            Self::Unreviewed => "unreviewed",
        }
    }
}

/// Installation scope for a new plugin, mirroring deveco-code's local/global
/// toggle: `User` installs into the user-level plugins dir, `Project` into the
/// workspace's project-level plugins dir.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PluginInstallScope {
    User,
    Project,
}

impl PluginInstallScope {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Project => "project",
        }
    }

    pub(crate) fn toggle(self) -> Self {
        match self {
            Self::User => Self::Project,
            Self::Project => Self::User,
        }
    }
}

/// A managed plugin package item for display in the browser.
#[derive(Debug, Clone)]
pub(crate) struct PluginItem {
    pub id: String,
    pub version: String,
    pub source_scope: String,
    pub trust_label: String,
    pub activated: bool,
    pub content_hash: String,
    pub status: PluginDisplayStatus,
}

/// Compute the display status from the trust level and activation state.
fn plugin_display_status(trust: ManagedPluginTrustLevel, activated: bool) -> PluginDisplayStatus {
    match trust {
        ManagedPluginTrustLevel::Denied | ManagedPluginTrustLevel::Revoked => {
            PluginDisplayStatus::Disabled
        }
        ManagedPluginTrustLevel::Unknown => PluginDisplayStatus::Unreviewed,
        ManagedPluginTrustLevel::SourceApproved => {
            if activated {
                PluginDisplayStatus::Active
            } else {
                PluginDisplayStatus::Inactive
            }
        }
        _ => PluginDisplayStatus::Unreviewed,
    }
}

/// Render the `ManagedPluginTrustLevel` to a stable short label for display.
pub(crate) fn plugin_trust_label(trust: ManagedPluginTrustLevel) -> &'static str {
    match trust {
        ManagedPluginTrustLevel::Unknown => "unreviewed",
        ManagedPluginTrustLevel::SourceApproved => "source-approved",
        ManagedPluginTrustLevel::Denied => "denied",
        ManagedPluginTrustLevel::Revoked => "revoked",
        _ => "other",
    }
}

/// Map a `ManagedPluginPackageView` to the popup's display item.
pub(crate) fn plugin_item_from_view(view: &ManagedPluginPackageView) -> PluginItem {
    PluginItem {
        id: view.package_id.clone(),
        version: view.version.clone(),
        source_scope: view.source_scope.clone(),
        trust_label: plugin_trust_label(view.trust_level).to_string(),
        activated: view.activated,
        content_hash: view.content_hash.clone(),
        status: plugin_display_status(view.trust_level, view.activated),
    }
}

/// Project a plugin snapshot into display items, preserving the snapshot order.
pub(crate) fn plugin_items_from_snapshot(
    snapshot: &ManagedPluginSourceSnapshot,
) -> Vec<PluginItem> {
    snapshot
        .packages
        .iter()
        .map(plugin_item_from_view)
        .collect()
}

/// Refresh and project the managed plugin snapshot into display items.
pub(crate) fn refresh_plugin_items(
    workspace: &Path,
    rt_handle: &tokio::runtime::Handle,
) -> Vec<PluginItem> {
    tokio::task::block_in_place(|| {
        rt_handle.block_on(async {
            match refresh_managed_plugin_sources(workspace).await {
                Ok(snapshot) => plugin_items_from_snapshot(&snapshot),
                Err(error) => {
                    tracing::error!("Failed to load plugin snapshot: {}", error);
                    Vec::new()
                }
            }
        })
    })
}

/// Toggle a managed plugin's activation. `activate == true` activates;
/// `false` deactivates. Returns a `String` error so the poll loop can render a
/// uniform message regardless of the underlying source error type.
pub(crate) async fn toggle_managed_plugin(
    workspace: &Path,
    plugin_id: &str,
    content_hash: &str,
    activate: bool,
) -> Result<(), String> {
    if activate {
        activate_managed_plugin(workspace, plugin_id, Some(content_hash))
            .await
            .map(|_| ())
            .map_err(|error| error.to_string())
    } else {
        deactivate_managed_plugin(workspace, plugin_id)
            .await
            .map(|_| ())
            .map_err(|error| error.to_string())
    }
}

/// Install a managed plugin from a package specifier.
///
/// TODO: replace with `bitfun_core::plugin_source::install_managed_plugin`
/// once the core install API lands. This skeleton placeholder reports the
/// operation as not yet implemented so the install UI flow can be exercised
/// without crashing the TUI.
pub(crate) async fn install_managed_plugin(
    _workspace: &Path,
    _spec: &str,
    _scope: PluginInstallScope,
) -> Result<(), String> {
    Err("plugin install is not yet implemented (TODO: wire core install API)".to_string())
}
