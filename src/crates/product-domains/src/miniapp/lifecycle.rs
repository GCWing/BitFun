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

pub fn mark_deps_installed_state(app: &mut MiniApp) {
    ensure_runtime_state(app);
    app.runtime.deps_dirty = false;
    app.runtime.worker_restart_required = true;
}

pub fn clear_worker_restart_required_state(app: &mut MiniApp) -> bool {
    ensure_runtime_state(app);
    if app.runtime.worker_restart_required {
        app.runtime.worker_restart_required = false;
        return true;
    }
    false
}

pub fn prepare_rollback_app(current: &MiniApp, mut target: MiniApp, now: i64) -> MiniApp {
    target.version = current.version + 1;
    target.updated_at = now;
    target.runtime = build_runtime_state(
        target.version,
        target.updated_at,
        &target.source,
        !target.source.npm_dependencies.is_empty(),
        true,
    );
    target
}

pub fn apply_recompile_result(app: &mut MiniApp, compiled_html: String, now: i64) {
    app.compiled_html = compiled_html;
    app.updated_at = now;
    ensure_runtime_state(app);
    app.runtime.ui_recompile_required = false;
}

pub fn apply_sync_from_fs_result(
    previous: &MiniApp,
    source: MiniAppSource,
    compiled_html: String,
    now: i64,
) -> MiniApp {
    let mut app = previous.clone();
    app.source = source;
    app.version += 1;
    app.updated_at = now;
    app.compiled_html = compiled_html;
    app.runtime = build_runtime_state(
        app.version,
        app.updated_at,
        &app.source,
        !app.source.npm_dependencies.is_empty(),
        true,
    );
    app
}

pub fn apply_import_runtime_state(app: &mut MiniApp) {
    app.runtime = build_runtime_state(
        app.version,
        app.updated_at,
        &app.source,
        !app.source.npm_dependencies.is_empty(),
        true,
    );
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
