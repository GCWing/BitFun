#![cfg(feature = "miniapp")]

use bitfun_product_domains::miniapp::bridge_builder::build_csp_content;
use bitfun_product_domains::miniapp::compiler::compile;
use bitfun_product_domains::miniapp::exporter::{ExportCheckResult, ExportTarget};
use bitfun_product_domains::miniapp::host_routing::{
    command_basename_for_allowlist, is_host_primitive,
};
use bitfun_product_domains::miniapp::lifecycle::{
    build_deps_revision, build_runtime_state, build_source_revision, build_worker_revision,
    ensure_runtime_state, workspace_dir_string,
};
use bitfun_product_domains::miniapp::permission_policy::resolve_policy;
use bitfun_product_domains::miniapp::ports::{
    MiniAppInstallDepsRequest, MiniAppPortError, MiniAppPortErrorKind, MiniAppPortFuture,
    MiniAppRuntimePort,
};
use bitfun_product_domains::miniapp::runtime::{
    candidate_dirs, version_manager_roots, RuntimeKind,
};
use bitfun_product_domains::miniapp::storage::{
    build_package_json, parse_npm_dependencies, MiniAppStorageLayout, COMPILED_HTML, ESM_DEPS_JSON,
    INDEX_HTML, META_JSON, PACKAGE_JSON, SOURCE_DIR, STORAGE_JSON, STYLE_CSS, UI_JS, VERSIONS_DIR,
    WORKER_JS,
};
use bitfun_product_domains::miniapp::types::{
    FsPermissions, MiniApp, MiniAppPermissions, MiniAppRuntimeState, MiniAppSource, NetPermissions,
    NpmDep,
};
use bitfun_product_domains::miniapp::worker::{install_command_for_runtime, InstallResult};
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
