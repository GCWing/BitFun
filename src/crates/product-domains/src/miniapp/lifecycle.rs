//! MiniApp lifecycle revision helpers.

use std::path::Path;

use crate::miniapp::types::{MiniApp, MiniAppRuntimeState, MiniAppSource};

pub fn build_source_revision(version: u32, updated_at: i64) -> String {
    format!("src:{version}:{updated_at}")
}

pub fn build_deps_revision(source: &MiniAppSource) -> String {
    let mut deps: Vec<String> = source
        .npm_dependencies
        .iter()
        .map(|dep| format!("{}@{}", dep.name, dep.version))
        .collect();
    deps.sort();
    deps.join("|")
}

pub fn build_runtime_state(
    version: u32,
    updated_at: i64,
    source: &MiniAppSource,
    deps_dirty: bool,
    worker_restart_required: bool,
) -> MiniAppRuntimeState {
    MiniAppRuntimeState {
        source_revision: build_source_revision(version, updated_at),
        deps_revision: build_deps_revision(source),
        deps_dirty,
        worker_restart_required,
        ui_recompile_required: false,
    }
}

pub fn ensure_runtime_state(app: &mut MiniApp) -> bool {
    let mut changed = false;
    if app.runtime.source_revision.is_empty() {
        app.runtime.source_revision = build_source_revision(app.version, app.updated_at);
        changed = true;
    }
    let deps_revision = build_deps_revision(&app.source);
    if app.runtime.deps_revision != deps_revision {
        app.runtime.deps_revision = deps_revision;
        changed = true;
    }
    changed
}

pub fn build_worker_revision(app: &MiniApp, policy_json: &str) -> String {
    format!(
        "{}::{}::{}",
        app.runtime.source_revision, app.runtime.deps_revision, policy_json
    )
}

pub fn workspace_dir_string(workspace_root: Option<&Path>) -> String {
    workspace_root
        .map(|path| path.to_string_lossy().to_string())
        .unwrap_or_default()
}
