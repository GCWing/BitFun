//! MiniApp lifecycle revision helpers.

use std::path::Path;

use crate::miniapp::types::{
    MiniApp, MiniAppAiContext, MiniAppPermissions, MiniAppRuntimeState, MiniAppSource,
};

#[derive(Debug, Clone)]
pub struct MiniAppCreateInput {
    pub name: String,
    pub description: String,
    pub icon: String,
    pub category: String,
    pub tags: Vec<String>,
    pub source: MiniAppSource,
    pub permissions: MiniAppPermissions,
    pub ai_context: Option<MiniAppAiContext>,
}

#[derive(Debug, Clone, Default)]
pub struct MiniAppUpdatePatch {
    pub name: Option<String>,
    pub description: Option<String>,
    pub icon: Option<String>,
    pub category: Option<String>,
    pub tags: Option<Vec<String>>,
    pub source: Option<MiniAppSource>,
    pub permissions: Option<MiniAppPermissions>,
    pub ai_context: Option<MiniAppAiContext>,
}

impl MiniAppUpdatePatch {
    pub fn source_for_compile<'a>(&'a self, previous: &'a MiniApp) -> &'a MiniAppSource {
        self.source.as_ref().unwrap_or(&previous.source)
    }

    pub fn permissions_for_compile<'a>(&'a self, previous: &'a MiniApp) -> &'a MiniAppPermissions {
        self.permissions.as_ref().unwrap_or(&previous.permissions)
    }
}

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

pub fn build_created_app(
    id: String,
    input: MiniAppCreateInput,
    compiled_html: String,
    now: i64,
) -> MiniApp {
    let version = 1;
    let runtime = build_runtime_state(
        version,
        now,
        &input.source,
        !input.source.npm_dependencies.is_empty(),
        true,
    );

    MiniApp {
        id,
        name: input.name,
        description: input.description,
        icon: input.icon,
        category: input.category,
        tags: input.tags,
        version,
        created_at: now,
        updated_at: now,
        source: input.source,
        compiled_html,
        permissions: input.permissions,
        ai_context: input.ai_context,
        runtime,
        i18n: None,
    }
}

pub fn apply_update_patch(
    previous: &MiniApp,
    patch: MiniAppUpdatePatch,
    compiled_html: String,
    now: i64,
) -> MiniApp {
    let source_changed = patch.source.is_some();
    let permissions_changed = patch.permissions.is_some();
    let mut app = previous.clone();

    if let Some(name) = patch.name {
        app.name = name;
    }
    if let Some(description) = patch.description {
        app.description = description;
    }
    if let Some(icon) = patch.icon {
        app.icon = icon;
    }
    if let Some(category) = patch.category {
        app.category = category;
    }
    if let Some(tags) = patch.tags {
        app.tags = tags;
    }
    if let Some(source) = patch.source {
        app.source = source;
    }
    if let Some(permissions) = patch.permissions {
        app.permissions = permissions;
    }
    if let Some(ai_context) = patch.ai_context {
        app.ai_context = Some(ai_context);
    }

    app.version += 1;
    app.updated_at = now;
    app.compiled_html = compiled_html;

    let deps_changed = previous.source.npm_dependencies != app.source.npm_dependencies;
    if source_changed || permissions_changed {
        app.runtime.source_revision = build_source_revision(app.version, app.updated_at);
        app.runtime.worker_restart_required = true;
    }
    if deps_changed {
        app.runtime.deps_revision = build_deps_revision(&app.source);
        app.runtime.deps_dirty = !app.source.npm_dependencies.is_empty();
        app.runtime.worker_restart_required = true;
    }
    app.runtime.ui_recompile_required = false;
    ensure_runtime_state(&mut app);
    app
}

pub fn prepare_draft_app(mut app: MiniApp, compiled_html: String, now: i64) -> MiniApp {
    app.updated_at = now;
    app.compiled_html = compiled_html;
    ensure_runtime_state(&mut app);
    app
}

pub fn apply_draft_source_sync_result(
    mut app: MiniApp,
    compiled_html: String,
    now: i64,
) -> MiniApp {
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

pub fn apply_draft_permission_update_result(
    mut app: MiniApp,
    permissions: MiniAppPermissions,
    compiled_html: String,
    now: i64,
) -> MiniApp {
    app.permissions = permissions;
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

pub fn apply_draft_to_active(
    current: &MiniApp,
    draft: MiniApp,
    compiled_html: String,
    now: i64,
) -> MiniApp {
    let mut app = current.clone();
    app.name = draft.name;
    app.description = draft.description;
    app.icon = draft.icon;
    app.category = draft.category;
    app.tags = draft.tags;
    app.source = draft.source;
    app.permissions = draft.permissions;
    app.ai_context = draft.ai_context;
    app.i18n = draft.i18n;
    app.version = current.version + 1;
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
