//! CLI-local plugin backend operations.
//!
//! Bridges the TUI plugin browser to bitfun-core's managed plugin source and
//! runtime APIs. Lives outside the boundary-scanned `ui/` and `modes/chat/`
//! trees so the TUI backend direct-call ratchet stays clean; the UI layer
//! consumes the plain `PluginItem` / `PluginInstallScope` /
//! `PluginDisplayStatus` types and the operations exposed here, never
//! importing bitfun-core directly.

use std::path::{Path, PathBuf};

use bitfun_core::plugin_runtime::{activate_managed_plugin, deactivate_managed_plugin};
use bitfun_core::plugin_source::{
    managed_plugin_install_dirs, refresh_managed_plugin_sources, ManagedPluginPackageView,
    ManagedPluginSourceSnapshot, ManagedPluginTrustLevel,
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
/// `spec` accepts:
/// - a local directory or `.tgz` path (optionally `file://`-prefixed), or
/// - an npm package name (`pkg`, `@scope/pkg`, `pkg@version`).
///
/// The package is fetched/staged in a temp dir, validated for a
/// `bitfun.plugin.json` manifest, then placed at
/// `<plugins_dir>/<package_id>/` in the chosen scope's plugin root. An
/// existing install of the same id is replaced (reinstall). Dir resolution
/// stays in core; the host-specific fetch (npm spawn / tarball extract / copy)
/// lives here in the CLI-local backend.
pub(crate) async fn install_managed_plugin(
    workspace: &Path,
    spec: &str,
    scope: PluginInstallScope,
) -> Result<(), String> {
    let spec = spec.trim();
    if spec.is_empty() {
        return Err("plugin spec is empty".to_string());
    }
    let (user_dir, project_dir) = managed_plugin_install_dirs(workspace)
        .map_err(|error| format!("failed to resolve plugin dirs: {error}"))?;
    let target_root = match scope {
        PluginInstallScope::User => user_dir,
        PluginInstallScope::Project => project_dir,
    };

    let stage = tempfile::tempdir().map_err(|e| format!("failed to create staging dir: {e}"))?;
    let package_dir = fetch_package(spec, stage.path()).await?;
    let manifest_path = package_dir.join("bitfun.plugin.json");
    let package_id = read_manifest_id(&manifest_path)
        .map_err(|e| format!("installed package is not a valid BitFun plugin: {e}"))?;
    validate_package_id(&package_id)?;

    std::fs::create_dir_all(&target_root)
        .map_err(|e| format!("failed to create plugin dir {}: {e}", target_root.display()))?;
    let dest = target_root.join(&package_id);
    if dest.exists() {
        std::fs::remove_dir_all(&dest)
            .map_err(|e| format!("failed to remove existing plugin '{}': {e}", package_id))?;
    }
    let result = rename_or_copy_dir(&package_dir, &dest)
        .map_err(|e| format!("failed to place plugin '{}': {e}", package_id));
    // `stage` drops here and cleans any leftover staging files.
    let _ = stage;
    result?;
    Ok(())
}

/// Fetch a plugin package into `stage`, returning the dir that holds
/// `bitfun.plugin.json`. Auto-detects local path vs npm name.
async fn fetch_package(spec: &str, stage: &Path) -> Result<PathBuf, String> {
    let local = spec.strip_prefix("file://").unwrap_or(spec);
    if std::path::Path::new(local).exists() {
        fetch_local(local, stage).await
    } else {
        fetch_npm(spec, stage).await
    }
}

async fn fetch_local(src: &str, stage: &Path) -> Result<PathBuf, String> {
    let src_path = std::path::Path::new(src);
    if src_path.is_dir() {
        let out = stage.join("package");
        copy_dir_recursive(src_path, &out)?;
        Ok(out)
    } else if src_path.is_file() {
        let extract_dir = stage.join("extract");
        std::fs::create_dir_all(&extract_dir)
            .map_err(|e| format!("failed to create extract dir: {e}"))?;
        extract_tgz_into(src_path, &extract_dir)?;
        locate_package_dir(&extract_dir)
    } else {
        Err(format!(
            "local spec must be a directory or .tgz file: {src}"
        ))
    }
}

async fn fetch_npm(spec: &str, stage: &Path) -> Result<PathBuf, String> {
    // On Windows `npm` ships as `npm.cmd` (a batch wrapper), not `npm.exe`;
    // `Command::new("npm")` would fail with "program not found". Route through
    // `cmd /c npm` so cmd resolves PATHEXT. On Unix npm is a real executable.
    #[cfg(windows)]
    let mut cmd = {
        let mut c = tokio::process::Command::new("cmd");
        c.args(["/c", "npm"]);
        c
    };
    #[cfg(not(windows))]
    let mut cmd = tokio::process::Command::new("npm");

    let output = cmd
        .args(["pack", spec])
        .current_dir(stage)
        .output()
        .await
        .map_err(|e| format!("failed to spawn `npm pack` (is npm on PATH?): {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "`npm pack {spec}` failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    let tgz = find_single_tgz(stage)?;
    let extract_dir = stage.join("extract");
    std::fs::create_dir_all(&extract_dir)
        .map_err(|e| format!("failed to create extract dir: {e}"))?;
    extract_tgz_into(&tgz, &extract_dir)?;
    locate_package_dir(&extract_dir)
}

fn extract_tgz_into(tgz: &Path, out_dir: &Path) -> Result<(), String> {
    let f = std::fs::File::open(tgz)
        .map_err(|e| format!("failed to open tarball {}: {e}", tgz.display()))?;
    let gz = flate2::read::GzDecoder::new(f);
    let mut archive = tar::Archive::new(gz);
    archive
        .unpack(out_dir)
        .map_err(|e| format!("failed to extract tarball: {e}"))?;
    Ok(())
}

fn find_single_tgz(dir: &Path) -> Result<PathBuf, String> {
    let mut found = Vec::new();
    for entry in std::fs::read_dir(dir).map_err(|e| format!("failed to read staging dir: {e}"))? {
        let entry = entry.map_err(|e| format!("failed to read staging entry: {e}"))?;
        let p = entry.path();
        if p.is_file() && p.extension().and_then(|e| e.to_str()) == Some("tgz") {
            found.push(p);
        }
    }
    match found.len() {
        0 => Err("`npm pack` produced no .tgz file".to_string()),
        1 => Ok(found.remove(0)),
        _ => Err(format!(
            "`npm pack` produced multiple .tgz files: {:?}",
            found
        )),
    }
}

/// Locate the extracted dir that actually contains `bitfun.plugin.json`.
/// npm tarballs extract to `<extract_dir>/package/`; BitFun tarballs may
/// extract directly or one level deep.
fn locate_package_dir(extract_dir: &Path) -> Result<PathBuf, String> {
    let inner = extract_dir.join("package");
    if inner.is_dir() && inner.join("bitfun.plugin.json").exists() {
        return Ok(inner);
    }
    if extract_dir.join("bitfun.plugin.json").exists() {
        return Ok(extract_dir.to_path_buf());
    }
    for entry in std::fs::read_dir(extract_dir)
        .map_err(|e| format!("failed to read extracted package: {e}"))?
    {
        let entry = entry.map_err(|e| format!("failed to read extract entry: {e}"))?;
        let p = entry.path();
        if p.is_dir() && p.join("bitfun.plugin.json").exists() {
            return Ok(p);
        }
    }
    Err("installed package is missing bitfun.plugin.json".to_string())
}

fn read_manifest_id(manifest_path: &Path) -> Result<String, String> {
    if !manifest_path.exists() {
        return Err("missing bitfun.plugin.json".to_string());
    }
    let bytes = std::fs::read(manifest_path)
        .map_err(|e| format!("failed to read bitfun.plugin.json: {e}"))?;
    let value: serde_json::Value = serde_json::from_slice(&bytes)
        .map_err(|e| format!("bitfun.plugin.json is not valid JSON: {e}"))?;
    value
        .get("id")
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "bitfun.plugin.json missing non-empty string 'id' field".to_string())
}

/// Reject manifest ids that would escape the plugin root (path traversal).
fn validate_package_id(id: &str) -> Result<(), String> {
    if id.is_empty()
        || id.contains('/')
        || id.contains('\\')
        || id == "."
        || id == ".."
        || id.contains('\0')
    {
        return Err(format!("invalid plugin id from manifest: `{id}`"));
    }
    Ok(())
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dst).map_err(|e| format!("failed to create {}: {e}", dst.display()))?;
    for entry in
        std::fs::read_dir(src).map_err(|e| format!("failed to read {}: {e}", src.display()))?
    {
        let entry = entry.map_err(|e| format!("failed to read entry: {e}"))?;
        let from = entry.path();
        let to = dst.join(entry.file_name());
        if from.is_dir() {
            copy_dir_recursive(&from, &to)?;
        } else {
            std::fs::copy(&from, &to)
                .map_err(|e| format!("failed to copy {}: {e}", from.display()))?;
        }
    }
    Ok(())
}

/// Move `src` to `dst`, falling back to recursive copy+remove across volumes.
fn rename_or_copy_dir(src: &Path, dst: &Path) -> Result<(), String> {
    match std::fs::rename(src, dst) {
        Ok(()) => Ok(()),
        Err(_) => {
            copy_dir_recursive(src, dst)?;
            std::fs::remove_dir_all(src)
                .map_err(|e| format!("failed to clean staging {}: {e}", src.display()))?;
            Ok(())
        }
    }
}
