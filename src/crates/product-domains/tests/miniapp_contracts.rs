#![cfg(feature = "miniapp")]

use bitfun_product_domains::miniapp::bridge_builder::{build_bridge_script, build_csp_content};
use bitfun_product_domains::miniapp::compiler::compile;
use bitfun_product_domains::miniapp::customization::{
    MAX_DECLINED_BUILTIN_UPDATES, MiniAppCustomizationBaseline, MiniAppCustomizationLocalSnapshot,
    MiniAppCustomizationMetadata, MiniAppCustomizationOrigin, MiniAppCustomizationOriginKind,
    apply_draft_customization_metadata, decline_builtin_update_metadata,
    declined_builtin_update_needs_local_snapshot, is_current_declined_builtin_update,
    mark_builtin_update_available_metadata,
};
use bitfun_product_domains::miniapp::draft::{
    MINIAPP_DRAFT_STATUS_APPLIED, MINIAPP_DRAFT_STATUS_DRAFT, build_draft_manifest,
    build_draft_response,
};
use bitfun_product_domains::miniapp::exporter::{ExportCheckResult, ExportTarget};
use bitfun_product_domains::miniapp::host_routing::{
    command_basename_allowed, command_basename_for_allowlist, host_allowed_by_allowlist,
    is_host_primitive,
};
use bitfun_product_domains::miniapp::lifecycle::{
    apply_import_runtime_state, apply_recompile_result, apply_sync_from_fs_result,
    build_deps_revision, build_runtime_state, build_source_revision, build_worker_revision,
    clear_worker_restart_required_state, ensure_runtime_state, mark_deps_installed_state,
    prepare_rollback_app, workspace_dir_string,
};
use bitfun_product_domains::miniapp::permission_policy::resolve_policy;
use bitfun_product_domains::miniapp::ports::{
    MiniAppInstallDepsRequest, MiniAppPortError, MiniAppPortErrorKind, MiniAppPortFuture,
    MiniAppRuntimePort,
};
use bitfun_product_domains::miniapp::runtime::{
    RuntimeKind, candidate_dirs, candidate_executable_path, runtime_lookup_order,
    version_manager_roots, versioned_executable_candidate,
};
use bitfun_product_domains::miniapp::storage::{
    COMPILED_HTML, CUSTOMIZATION_JSON, DRAFT_JSON, DRAFTS_CLEANUP_MARKER, DRAFTS_CLEANUP_PREFIX,
    DRAFTS_DIR, EMPTY_ESM_DEPENDENCIES_JSON, EMPTY_STORAGE_JSON, ESM_DEPS_JSON, INDEX_HTML,
    META_JSON, MiniAppImportLayout, MiniAppStorageLayout, PACKAGE_JSON, PLACEHOLDER_COMPILED_HTML,
    REQUIRED_SOURCE_FILES, SOURCE_DIR, STORAGE_JSON, STYLE_CSS, UI_JS, VERSIONS_DIR, WORKER_JS,
    build_import_fallbacks, build_package_json, parse_npm_dependencies,
};
use bitfun_product_domains::miniapp::types::{
    FsPermissions, MiniApp, MiniAppPermissions, MiniAppRuntimeState, MiniAppSource, NetPermissions,
    NotificationPermissions, NpmDep,
};
use bitfun_product_domains::miniapp::worker::{InstallResult, install_command_for_runtime};
use std::path::{Path, PathBuf};

struct RuntimePortStub;

impl MiniAppRuntimePort for RuntimePortStub {
    fn detect_runtime(
        &self,
    ) -> MiniAppPortFuture<'_, Option<bitfun_product_domains::miniapp::runtime::DetectedRuntime>>
    {
        Box::pin(async { Ok(None) })
    }

    fn install_deps(
        &self,
        _request: MiniAppInstallDepsRequest,
    ) -> MiniAppPortFuture<'_, InstallResult> {
        Box::pin(async {
            Ok(InstallResult {
                success: true,
                stdout: String::new(),
                stderr: String::new(),
            })
        })
    }
}

#[test]
fn miniapp_csp_content_preserves_net_allow_contract() {
    let permissions = MiniAppPermissions {
        net: Some(NetPermissions {
            allow: Some(vec!["api.example.com".to_string()]),
        }),
        ..MiniAppPermissions::default()
    };

    let csp = build_csp_content(&permissions);

    assert_eq!(
        csp,
        "default-src 'none'; script-src 'self' 'unsafe-inline' 'unsafe-eval' https:; style-src 'self' 'unsafe-inline' https:; connect-src 'self' 'self' https://esm.sh api.example.com; img-src 'self' data: https:; font-src 'self' https:; object-src 'none'; base-uri 'self';"
    );
}

#[test]
fn miniapp_permissions_support_host_notifications_without_domain_specific_fields() {
    let permissions: MiniAppPermissions = serde_json::from_value(serde_json::json!({
        "notifications": { "system": true },
        "net": { "allow": ["*"] }
    }))
    .unwrap();

    assert_eq!(
        permissions.notifications,
        Some(NotificationPermissions { system: true })
    );
    assert_eq!(permissions.net.unwrap().allow.unwrap(), vec!["*"]);
}

#[test]
fn miniapp_bridge_exposes_host_notification_namespace() {
    let bridge = build_bridge_script("app-1", "/tmp/app", "/tmp/workspace", "dark", "win32");

    assert!(bridge.contains("notifications:"));
    assert!(bridge.contains("notifications.system"));
    assert!(bridge.contains("system:"));
    assert!(bridge.contains("system.openExternal"));
}

#[test]
fn miniapp_permission_policy_preserves_scope_resolution() {
    let permissions = MiniAppPermissions {
        fs: Some(FsPermissions {
            read: Some(vec!["{appdata}".to_string(), "{workspace}".to_string()]),
            write: Some(vec!["{user-selected}".to_string()]),
        }),
        ..MiniAppPermissions::default()
    };

    let policy = resolve_policy(
        &permissions,
        "app_1",
        Path::new("/tmp/app-data"),
        Some(Path::new("/tmp/workspace")),
        &[PathBuf::from("/tmp/granted")],
    );

    assert_eq!(policy["fs"]["read"][0], "/tmp/app-data");
    assert_eq!(policy["fs"]["read"][1], "/tmp/workspace");
    assert_eq!(policy["fs"]["read"][2], "/tmp/granted");
    assert_eq!(policy["fs"]["write"][0], "/tmp/granted");
}

#[test]
fn miniapp_compiler_preserves_head_injection_contract() {
    let source = MiniAppSource {
        html: r#"<!DOCTYPE html><html><head><meta charset="utf-8"></head><body>x</body></html>"#
            .to_string(),
        ui_js: "console.log('ready')".to_string(),
        ..MiniAppSource::default()
    };

    let out = compile(
        &source,
        &MiniAppPermissions::default(),
        "app-id",
        "/tmp/app",
        "/tmp/workspace",
        "dark",
    )
    .unwrap();

    assert!(out.contains("<meta charset=\"utf-8\">"));
    assert!(out.contains("data-theme-type=\"dark\""));
    assert!(out.contains("<script type=\"module\">"));
    assert!(out.contains("console.log('ready')"));
}

#[test]
fn miniapp_export_and_runtime_dtos_remain_stable() {
    assert_eq!(RuntimeKind::Node, RuntimeKind::Node);

    let target = serde_json::to_string(&ExportTarget::Tauri).unwrap();
    assert_eq!(target, "\"Tauri\"");

    let check = ExportCheckResult {
        ready: false,
        runtime: None,
        missing: vec!["No JS runtime (install Bun or Node.js)".to_string()],
        warnings: Vec::new(),
    };
    let json = serde_json::to_value(&check).unwrap();
    assert_eq!(json["ready"], false);
    assert_eq!(json["missing"][0], "No JS runtime (install Bun or Node.js)");

    let install = InstallResult {
        success: true,
        stdout: "ok".to_string(),
        stderr: String::new(),
    };
    let json = serde_json::to_value(&install).unwrap();
    assert_eq!(json["success"], true);
    assert_eq!(json["stdout"], "ok");
}

#[test]
fn miniapp_storage_layout_preserves_file_shape_contract() {
    let root = PathBuf::from("/bitfun/miniapps");
    let layout = MiniAppStorageLayout::new(&root, "app-1");

    assert_eq!(META_JSON, "meta.json");
    assert_eq!(SOURCE_DIR, "source");
    assert_eq!(INDEX_HTML, "index.html");
    assert_eq!(STYLE_CSS, "style.css");
    assert_eq!(UI_JS, "ui.js");
    assert_eq!(WORKER_JS, "worker.js");
    assert_eq!(PACKAGE_JSON, "package.json");
    assert_eq!(ESM_DEPS_JSON, "esm_dependencies.json");
    assert_eq!(COMPILED_HTML, "compiled.html");
    assert_eq!(STORAGE_JSON, "storage.json");
    assert_eq!(VERSIONS_DIR, "versions");
    assert_eq!(DRAFTS_DIR, ".drafts");
    assert_eq!(DRAFTS_CLEANUP_PREFIX, ".drafts.cleanup-");
    assert_eq!(DRAFTS_CLEANUP_MARKER, ".cleanup-pending");
    assert_eq!(DRAFT_JSON, "draft.json");
    assert_eq!(CUSTOMIZATION_JSON, ".customization.json");

    assert_eq!(layout.app_dir(), root.join("app-1"));
    assert_eq!(layout.meta_path(), root.join("app-1").join(META_JSON));
    assert_eq!(
        layout.source_file_path(INDEX_HTML),
        root.join("app-1").join(SOURCE_DIR).join(INDEX_HTML)
    );
    assert_eq!(
        layout.version_path(3),
        root.join("app-1").join(VERSIONS_DIR).join("v3.json")
    );
    assert_eq!(layout.versions_dir(), root.join("app-1").join(VERSIONS_DIR));
    assert_eq!(
        layout.customization_path(),
        root.join("app-1").join(CUSTOMIZATION_JSON)
    );
    assert_eq!(
        MiniAppStorageLayout::drafts_root(&root),
        root.join(DRAFTS_DIR)
    );
    assert_eq!(
        MiniAppStorageLayout::draft_dir(&root, "app-1", "draft-1"),
        root.join(DRAFTS_DIR).join("app-1").join("draft-1")
    );
    assert_eq!(
        MiniAppStorageLayout::draft_source_dir(&root, "app-1", "draft-1"),
        root.join(DRAFTS_DIR)
            .join("app-1")
            .join("draft-1")
            .join(SOURCE_DIR)
    );
    assert_eq!(
        MiniAppStorageLayout::draft_manifest_path(&root, "app-1", "draft-1"),
        root.join(DRAFTS_DIR)
            .join("app-1")
            .join("draft-1")
            .join(DRAFT_JSON)
    );
    assert_eq!(
        MiniAppStorageLayout::cleanup_drafts_root(&root, "cleanup-id"),
        root.join(".drafts.cleanup-cleanup-id")
    );
}

#[test]
fn miniapp_runtime_search_plan_preserves_common_install_locations() {
    let home = PathBuf::from("/home/bitfun");
    let candidates = candidate_dirs(Some(&home));

    assert_eq!(candidates[0], PathBuf::from("/opt/homebrew/bin"));
    assert!(candidates.contains(&home.join(".bun").join("bin")));
    assert!(candidates.contains(&home.join(".asdf").join("shims")));

    let roots = version_manager_roots(Some(&home));
    assert_eq!(roots[0], home.join(".nvm").join("versions").join("node"));
    assert!(roots.contains(&home.join(".fnm").join("node-versions")));

    assert_eq!(runtime_lookup_order(), &["bun", "node"]);
    assert_eq!(
        candidate_executable_path(Path::new("/usr/local/bin"), "node"),
        PathBuf::from("/usr/local/bin").join("node")
    );
    assert_eq!(
        versioned_executable_candidate(Path::new("/home/bitfun/.nvm/versions/node/v20"), "node"),
        PathBuf::from("/home/bitfun/.nvm/versions/node/v20")
            .join("bin")
            .join("node")
    );
}

#[test]
fn miniapp_worker_install_command_preserves_runtime_choice() {
    let bun = install_command_for_runtime(&RuntimeKind::Bun, true);
    assert_eq!(bun.program, "bun");
    assert_eq!(bun.args, &["install", "--production"]);

    let node_with_pnpm = install_command_for_runtime(&RuntimeKind::Node, true);
    assert_eq!(node_with_pnpm.program, "pnpm");
    assert_eq!(node_with_pnpm.args, &["install", "--prod"]);

    let node_without_pnpm = install_command_for_runtime(&RuntimeKind::Node, false);
    assert_eq!(node_without_pnpm.program, "npm");
    assert_eq!(node_without_pnpm.args, &["install", "--production"]);
}

#[test]
fn miniapp_host_routing_preserves_existing_primitive_and_allowlist_contract() {
    assert!(is_host_primitive("fs.readFile"));
    assert!(is_host_primitive("shell.exec"));
    assert!(is_host_primitive("os.info"));
    assert!(is_host_primitive("net.fetch"));
    assert!(!is_host_primitive("storage.get"));
    assert!(!is_host_primitive("custom.method"));
    assert!(!is_host_primitive("shell"));

    assert_eq!(
        command_basename_for_allowlist(r"C:\Program Files\Git\cmd\git.exe"),
        "git"
    );
    assert_eq!(command_basename_for_allowlist("git.exe"), "git");
    assert_eq!(command_basename_for_allowlist("/usr/bin/git"), "git");
    assert_eq!(command_basename_for_allowlist("CARGO"), "cargo");

    assert!(command_basename_allowed(&[], "git"));
    assert!(command_basename_allowed(&["Git".to_string()], "git"));
    assert!(!command_basename_allowed(&["cargo".to_string()], "git"));

    assert!(host_allowed_by_allowlist(&[], "api.example.com"));
    assert!(host_allowed_by_allowlist(
        &["*".to_string()],
        "api.example.com"
    ));
    assert!(host_allowed_by_allowlist(
        &["example.com".to_string()],
        "api.example.com"
    ));
    assert!(host_allowed_by_allowlist(
        &["api.example.com".to_string()],
        "api.example.com"
    ));
    assert!(!host_allowed_by_allowlist(
        &["example.com".to_string()],
        "badexample.com"
    ));
}

#[test]
fn miniapp_lifecycle_helpers_preserve_runtime_revision_contract() {
    let source = MiniAppSource {
        npm_dependencies: vec![
            NpmDep {
                name: "zeta".to_string(),
                version: "2.0.0".to_string(),
            },
            NpmDep {
                name: "alpha".to_string(),
                version: "^1.0.0".to_string(),
            },
        ],
        ..MiniAppSource::default()
    };

    assert_eq!(build_source_revision(3, 1234), "src:3:1234");
    assert_eq!(build_deps_revision(&source), "alpha@^1.0.0|zeta@2.0.0");

    let runtime = build_runtime_state(3, 1234, &source, true, true);
    assert_eq!(runtime.source_revision, "src:3:1234");
    assert_eq!(runtime.deps_revision, "alpha@^1.0.0|zeta@2.0.0");
    assert!(runtime.deps_dirty);
    assert!(runtime.worker_restart_required);
    assert!(!runtime.ui_recompile_required);

    let mut app = sample_miniapp_for_lifecycle(source);
    assert!(ensure_runtime_state(&mut app));
    assert_eq!(app.runtime.source_revision, "src:3:1234");
    assert_eq!(app.runtime.deps_revision, "alpha@^1.0.0|zeta@2.0.0");
    assert!(!ensure_runtime_state(&mut app));

    assert_eq!(
        build_worker_revision(&app, r#"{"fs":{}}"#),
        r#"src:3:1234::alpha@^1.0.0|zeta@2.0.0::{"fs":{}}"#
    );
    assert_eq!(
        workspace_dir_string(Some(Path::new("/tmp/workspace"))),
        "/tmp/workspace"
    );
    assert_eq!(workspace_dir_string(None), "");
}

#[test]
fn miniapp_lifecycle_manager_state_helpers_preserve_core_transitions() {
    let source = MiniAppSource {
        npm_dependencies: vec![NpmDep {
            name: "lodash".to_string(),
            version: "^4.17.21".to_string(),
        }],
        ..MiniAppSource::default()
    };
    let mut app = sample_miniapp_for_lifecycle(source.clone());

    mark_deps_installed_state(&mut app);
    assert_eq!(app.runtime.source_revision, "src:3:1234");
    assert_eq!(app.runtime.deps_revision, "lodash@^4.17.21");
    assert!(!app.runtime.deps_dirty);
    assert!(app.runtime.worker_restart_required);

    assert!(clear_worker_restart_required_state(&mut app));
    assert!(!app.runtime.worker_restart_required);
    assert!(!clear_worker_restart_required_state(&mut app));

    apply_recompile_result(&mut app, "<html>fresh</html>".to_string(), 2000);
    assert_eq!(app.compiled_html, "<html>fresh</html>");
    assert_eq!(app.updated_at, 2000);
    assert!(!app.runtime.ui_recompile_required);
    assert_eq!(app.runtime.source_revision, "src:3:1234");

    let current = sample_miniapp_for_lifecycle(MiniAppSource::default());
    let rollback_target = sample_miniapp_for_lifecycle(source.clone());
    let rolled_back = prepare_rollback_app(&current, rollback_target, 3000);
    assert_eq!(rolled_back.version, current.version + 1);
    assert_eq!(rolled_back.updated_at, 3000);
    assert!(rolled_back.runtime.deps_dirty);
    assert!(rolled_back.runtime.worker_restart_required);
    assert_eq!(rolled_back.runtime.deps_revision, "lodash@^4.17.21");

    let synced =
        apply_sync_from_fs_result(&current, source, "<html>synced</html>".to_string(), 4000);
    assert_eq!(synced.version, current.version + 1);
    assert_eq!(synced.updated_at, 4000);
    assert_eq!(synced.compiled_html, "<html>synced</html>");
    assert!(synced.runtime.deps_dirty);
    assert!(synced.runtime.worker_restart_required);

    let mut imported = synced.clone();
    imported.runtime.worker_restart_required = false;
    imported.runtime.deps_dirty = false;
    apply_import_runtime_state(&mut imported);
    assert!(imported.runtime.deps_dirty);
    assert!(imported.runtime.worker_restart_required);
    assert_eq!(imported.runtime.source_revision, "src:4:4000");
    assert_eq!(imported.runtime.deps_revision, "lodash@^4.17.21");
}

#[test]
fn miniapp_storage_package_json_contract_remains_stable() {
    let deps = parse_npm_dependencies(
        r#"{
            "name": "miniapp-demo",
            "dependencies": {
                "left-pad": "^1.3.0",
                "local-only": { "workspace": true }
            }
        }"#,
    )
    .unwrap();

    assert!(deps.contains(&NpmDep {
        name: "left-pad".to_string(),
        version: "^1.3.0".to_string(),
    }));
    assert!(deps.contains(&NpmDep {
        name: "local-only".to_string(),
        version: "*".to_string(),
    }));

    let package = build_package_json(
        "demo",
        &[NpmDep {
            name: "lodash".to_string(),
            version: "^4.17.21".to_string(),
        }],
    );

    assert_eq!(package["name"], "miniapp-demo");
    assert_eq!(package["private"], true);
    assert_eq!(package["dependencies"]["lodash"], "^4.17.21");
}

#[test]
fn miniapp_storage_import_fallback_contract_remains_stable() {
    let root = PathBuf::from("/miniapps/incoming");
    let layout = MiniAppImportLayout::new(&root);

    assert_eq!(layout.meta_path(), root.join(META_JSON));
    assert_eq!(layout.source_dir(), root.join(SOURCE_DIR));
    assert_eq!(
        layout.source_file_path(INDEX_HTML),
        root.join(SOURCE_DIR).join(INDEX_HTML)
    );
    assert_eq!(
        layout.required_source_file_paths(),
        vec![
            (INDEX_HTML, root.join(SOURCE_DIR).join(INDEX_HTML)),
            (STYLE_CSS, root.join(SOURCE_DIR).join(STYLE_CSS)),
            (UI_JS, root.join(SOURCE_DIR).join(UI_JS)),
            (WORKER_JS, root.join(SOURCE_DIR).join(WORKER_JS)),
        ]
    );
    assert_eq!(
        layout.esm_dependencies_path(),
        root.join(SOURCE_DIR).join(ESM_DEPS_JSON)
    );
    assert_eq!(layout.package_json_path(), root.join(PACKAGE_JSON));
    assert_eq!(layout.storage_json_path(), root.join(STORAGE_JSON));

    assert_eq!(
        REQUIRED_SOURCE_FILES,
        &[INDEX_HTML, STYLE_CSS, UI_JS, WORKER_JS]
    );
    assert_eq!(EMPTY_ESM_DEPENDENCIES_JSON, "[]");
    assert_eq!(EMPTY_STORAGE_JSON, "{}");
    assert_eq!(
        PLACEHOLDER_COMPILED_HTML,
        "<!DOCTYPE html><html><head><meta charset=\"utf-8\"></head><body>Loading...</body></html>"
    );

    let package = build_package_json("imported-app", &[]);
    assert_eq!(package["name"], "miniapp-imported-app");
    assert_eq!(package["private"], true);
    assert_eq!(package["dependencies"], serde_json::json!({}));

    let fallbacks = build_import_fallbacks("imported-app");
    assert_eq!(fallbacks.esm_dependencies_json, "[]");
    assert_eq!(fallbacks.storage_json, "{}");
    assert_eq!(fallbacks.compiled_html, PLACEHOLDER_COMPILED_HTML);
    assert_eq!(fallbacks.package_json, package);
}

#[test]
fn miniapp_ports_keep_runtime_boundary_lightweight() {
    let decoded: MiniAppInstallDepsRequest = serde_json::from_value(serde_json::json!({
        "appId": "demo",
        "dependencies": [{"name": "lodash", "version": "^4.17.21"}]
    }))
    .unwrap();
    assert_eq!(decoded.app_id, "demo");
    assert_eq!(decoded.dependencies[0].name, "lodash");

    let request = MiniAppInstallDepsRequest {
        app_id: "demo".to_string(),
        dependencies: vec![NpmDep {
            name: "lodash".to_string(),
            version: "^4.17.21".to_string(),
        }],
    };

    let json = serde_json::to_value(&request).unwrap();
    assert_eq!(json["appId"], "demo");
    assert!(json.get("appDir").is_none());
    assert_eq!(json["dependencies"][0]["name"], "lodash");

    let error = MiniAppPortError::new(MiniAppPortErrorKind::RuntimeUnavailable, "missing node");
    let json = serde_json::to_value(error).unwrap();
    assert_eq!(json["kind"], "runtime_unavailable");
    assert_eq!(json["message"], "missing node");

    let port: &dyn MiniAppRuntimePort = &RuntimePortStub;
    let _future = port.detect_runtime();
}

#[test]
fn miniapp_draft_contract_preserves_manifest_and_response_shape() {
    let app = sample_miniapp_for_lifecycle(MiniAppSource::default());
    let manifest = build_draft_manifest("app-1", "draft-1", 7, 1234);

    assert_eq!(manifest.app_id, "app-1");
    assert_eq!(manifest.draft_id, "draft-1");
    assert_eq!(manifest.source_version, 7);
    assert_eq!(manifest.status, MINIAPP_DRAFT_STATUS_DRAFT);
    assert_eq!(manifest.created_at, 1234);
    assert_eq!(manifest.updated_at, 1234);

    let json = serde_json::to_value(&manifest).unwrap();
    assert_eq!(json["appId"], "app-1");
    assert_eq!(json["draftId"], "draft-1");
    assert_eq!(json["sourceVersion"], 7);

    let response = build_draft_response("/tmp/draft", app, manifest.clone());
    assert_eq!(response.app_id, "app-1");
    assert_eq!(response.draft_root, "/tmp/draft");
    assert_eq!(response.app.id, "demo");

    let mut applied = manifest;
    applied.mark_applied(2345);
    assert_eq!(applied.status, MINIAPP_DRAFT_STATUS_APPLIED);
    assert_eq!(applied.updated_at, 2345);
}

#[test]
fn miniapp_customization_apply_helper_preserves_builtin_override_policy() {
    let metadata = apply_draft_customization_metadata(
        None,
        MiniAppCustomizationBaseline::Builtin {
            builtin_id: "builtin-pr-review".to_string(),
            builtin_version: 4,
        },
        "draft-1",
        1234,
    );

    assert_eq!(
        metadata.origin.kind,
        MiniAppCustomizationOriginKind::Builtin
    );
    assert_eq!(
        metadata.origin.builtin_id.as_deref(),
        Some("builtin-pr-review")
    );
    assert_eq!(metadata.origin.builtin_version, Some(4));
    assert!(metadata.local_override);
    assert_eq!(metadata.last_applied_draft_id.as_deref(), Some("draft-1"));
    assert!(metadata.available_builtin_update.is_none());
    assert_eq!(metadata.updated_at, 1234);

    let updated = apply_draft_customization_metadata(
        Some(metadata),
        MiniAppCustomizationBaseline::Builtin {
            builtin_id: "builtin-pr-review".to_string(),
            builtin_version: 5,
        },
        "draft-2",
        2345,
    );

    assert_eq!(updated.origin.builtin_version, Some(5));
    assert!(updated.local_override);
    assert_eq!(updated.last_applied_draft_id.as_deref(), Some("draft-2"));
    assert!(updated.available_builtin_update.is_none());

    let user_created = MiniAppCustomizationMetadata {
        origin: MiniAppCustomizationOrigin {
            kind: MiniAppCustomizationOriginKind::UserCreated,
            builtin_id: None,
            builtin_version: None,
        },
        local_override: false,
        last_applied_draft_id: None,
        available_builtin_update: None,
        declined_builtin_updates: Vec::new(),
        updated_at: 10,
    };
    let user_created_update = apply_draft_customization_metadata(
        Some(user_created),
        MiniAppCustomizationBaseline::Builtin {
            builtin_id: "builtin-pr-review".to_string(),
            builtin_version: 6,
        },
        "draft-3",
        3456,
    );

    assert_eq!(
        user_created_update.origin.kind,
        MiniAppCustomizationOriginKind::UserCreated
    );
    assert!(!user_created_update.local_override);
    assert_eq!(
        user_created_update.last_applied_draft_id.as_deref(),
        Some("draft-3")
    );
    assert_eq!(user_created_update.updated_at, 3456);
}

#[test]
fn miniapp_customization_builtin_update_policy_preserves_decline_contract() {
    let mut metadata = apply_draft_customization_metadata(
        None,
        MiniAppCustomizationBaseline::Builtin {
            builtin_id: "builtin-pr-review".to_string(),
            builtin_version: 4,
        },
        "draft-1",
        1234,
    );

    let available = mark_builtin_update_available_metadata(metadata, 5, "hash-v5", 2000, false);
    assert!(available.should_surface_update);
    assert!(available.metadata_changed);
    metadata = available.metadata;
    assert_eq!(
        metadata
            .available_builtin_update
            .as_ref()
            .unwrap()
            .source_hash,
        "hash-v5"
    );

    metadata = decline_builtin_update_metadata(
        metadata,
        5,
        "hash-v5",
        2100,
        Some(MiniAppCustomizationLocalSnapshot {
            version: 7,
            updated_at: 2200,
        }),
    );

    assert!(metadata.available_builtin_update.is_none());
    assert_eq!(metadata.updated_at, 2100);
    assert_eq!(metadata.declined_builtin_updates.len(), 1);
    assert_eq!(
        metadata.declined_builtin_updates[0]
            .last_applied_draft_id
            .as_deref(),
        Some("draft-1")
    );
    assert!(declined_builtin_update_needs_local_snapshot(
        &metadata, "hash-v5"
    ));
    assert!(is_current_declined_builtin_update(
        &metadata,
        "hash-v5",
        Some(MiniAppCustomizationLocalSnapshot {
            version: 7,
            updated_at: 2200,
        }),
    ));
    assert!(!is_current_declined_builtin_update(
        &metadata,
        "hash-v5",
        Some(MiniAppCustomizationLocalSnapshot {
            version: 8,
            updated_at: 2200,
        }),
    ));

    let suppressed =
        mark_builtin_update_available_metadata(metadata.clone(), 5, "hash-v5", 2300, true);
    assert!(!suppressed.should_surface_update);
    assert!(!suppressed.metadata_changed);
    assert!(suppressed.metadata.available_builtin_update.is_none());

    let fallback = is_current_declined_builtin_update(&metadata, "hash-v5", None);
    assert!(fallback);
}

#[test]
fn miniapp_customization_decline_policy_updates_existing_and_trims_old_records() {
    let mut metadata = apply_draft_customization_metadata(
        None,
        MiniAppCustomizationBaseline::Builtin {
            builtin_id: "builtin-pr-review".to_string(),
            builtin_version: 4,
        },
        "draft-1",
        1000,
    );

    metadata = decline_builtin_update_metadata(metadata, 5, "hash-v5", 2000, None);
    metadata = decline_builtin_update_metadata(metadata, 5, "hash-v5", 2500, None);
    assert_eq!(metadata.declined_builtin_updates.len(), 1);
    assert_eq!(metadata.declined_builtin_updates[0].declined_at, 2500);

    for idx in 0..=MAX_DECLINED_BUILTIN_UPDATES {
        metadata = decline_builtin_update_metadata(
            metadata,
            6 + idx as u32,
            &format!("hash-{}", idx),
            3000 + idx as i64,
            None,
        );
    }

    assert_eq!(
        metadata.declined_builtin_updates.len(),
        MAX_DECLINED_BUILTIN_UPDATES
    );
    assert!(
        !metadata
            .declined_builtin_updates
            .iter()
            .any(|record| record.source_hash == "hash-v5")
    );
}

fn sample_miniapp_for_lifecycle(source: MiniAppSource) -> MiniApp {
    MiniApp {
        id: "demo".to_string(),
        name: "Demo".to_string(),
        description: "Demo app".to_string(),
        icon: "sparkles".to_string(),
        category: "tools".to_string(),
        tags: Vec::new(),
        version: 3,
        created_at: 1,
        updated_at: 1234,
        source,
        compiled_html: "<html></html>".to_string(),
        permissions: MiniAppPermissions::default(),
        ai_context: None,
        runtime: MiniAppRuntimeState::default(),
        i18n: None,
    }
}
