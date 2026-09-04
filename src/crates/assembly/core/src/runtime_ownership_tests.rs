use std::sync::{Arc, Barrier};

use openbitfun_services_core::runtime_ownership::{
    RuntimeDeployment, RuntimeOwnershipKey, WorkspaceRuntimeOwnership,
};
use tempfile::tempdir;

use crate::runtime_ownership::CoreRuntimeOwnership;
#[cfg(feature = "dispatch-store")]
use crate::service::dispatch::{DispatchTarget, OutboundDispatchRecord, OutboundDispatchStore};

#[test]
fn embedded_owner_is_idempotent_and_keeps_one_workspace_lease() {
    let ownership_root = tempdir().expect("ownership root");
    let workspace = tempdir().expect("workspace");
    let owner = CoreRuntimeOwnership::embedded_with_facts(
        ownership_root.path().to_path_buf(),
        "openbitfun".to_string(),
        "test",
    );

    owner
        .ensure_local_workspace(workspace.path())
        .expect("first acquisition");
    owner
        .ensure_local_workspace(&workspace.path().join("."))
        .expect("idempotent acquisition");

    let key =
        RuntimeOwnershipKey::for_workspace(workspace.path(), "openbitfun").expect("ownership key");
    assert!(WorkspaceRuntimeOwnership::try_acquire(
        ownership_root.path(),
        &key,
        RuntimeDeployment::Shared,
    )
    .is_err());
}

#[test]
fn embedded_owner_serializes_concurrent_first_acquisition() {
    let ownership_root = tempdir().expect("ownership root");
    let workspace = tempdir().expect("workspace");
    let owner = Arc::new(CoreRuntimeOwnership::embedded_with_facts(
        ownership_root.path().to_path_buf(),
        "openbitfun".to_string(),
        "test",
    ));
    let barrier = Arc::new(Barrier::new(5));
    let mut threads = Vec::new();
    for _ in 0..4 {
        let owner = Arc::clone(&owner);
        let barrier = Arc::clone(&barrier);
        let workspace = workspace.path().to_path_buf();
        threads.push(std::thread::spawn(move || {
            barrier.wait();
            owner.ensure_local_workspace(&workspace)
        }));
    }
    barrier.wait();
    for thread in threads {
        thread
            .join()
            .expect("acquisition thread")
            .expect("concurrent acquisition");
    }

    let key =
        RuntimeOwnershipKey::for_workspace(workspace.path(), "openbitfun").expect("ownership key");
    assert!(WorkspaceRuntimeOwnership::try_acquire(
        ownership_root.path(),
        &key,
        RuntimeDeployment::Shared,
    )
    .is_err());
}

#[test]
fn shared_owner_accepts_only_its_startup_workspace() {
    let ownership_root = tempdir().expect("ownership root");
    let workspace = tempdir().expect("workspace");
    let other_workspace = tempdir().expect("other workspace");
    let owner = CoreRuntimeOwnership::shared_with_facts(
        ownership_root.path().to_path_buf(),
        "openbitfun".to_string(),
        "test",
        workspace.path(),
    )
    .expect("shared owner");

    owner
        .ensure_local_workspace(workspace.path())
        .expect("same workspace");
    let error = owner
        .ensure_local_workspace(other_workspace.path())
        .expect_err("second workspace must fail closed");

    assert_eq!(error.code(), "shared_runtime_workspace_mismatch");
}

#[test]
fn unverified_remote_workspace_cannot_bypass_local_ownership() {
    let ownership_root = tempdir().expect("ownership root");
    let missing_local_path = ownership_root.path().join("remote-path-is-not-local");
    let owner = CoreRuntimeOwnership::embedded_with_facts(
        ownership_root.path().to_path_buf(),
        "openbitfun".to_string(),
        "test",
    );

    let error = owner
        .ensure_workspace_scope(&missing_local_path, Some("connection"), Some("host"))
        .expect_err("raw remote facts are not execution authority");
    assert_eq!(error.code(), "unverified_remote_workspace_scope");
    assert_eq!(
        std::fs::read_dir(ownership_root.path())
            .expect("read ownership root")
            .count(),
        0
    );
}

#[test]
fn verified_remote_workspace_does_not_touch_local_ownership() {
    let ownership_root = tempdir().expect("ownership root");
    let missing_local_path = ownership_root.path().join("remote-path-is-not-local");
    let owner = CoreRuntimeOwnership::embedded_with_facts(
        ownership_root.path().to_path_buf(),
        "openbitfun".to_string(),
        "test",
    );

    owner
        .register_verified_remote_scope(&missing_local_path, "connection", Some("host"))
        .expect("verified remote scope");
    owner
        .ensure_workspace_scope(&missing_local_path, Some("connection"), Some("host"))
        .expect("verified remote scope must skip local ownership");
    assert_eq!(
        std::fs::read_dir(ownership_root.path())
            .expect("read ownership root")
            .count(),
        0
    );
}

#[test]
fn ssh_host_without_connection_id_cannot_bypass_local_ownership() {
    let ownership_root = tempdir().expect("ownership root");
    let workspace = tempdir().expect("workspace");
    let shared = CoreRuntimeOwnership::shared_with_facts(
        ownership_root.path().to_path_buf(),
        "openbitfun".to_string(),
        "shared-test",
        workspace.path(),
    )
    .expect("shared owner");
    let embedded = CoreRuntimeOwnership::embedded_with_facts(
        ownership_root.path().to_path_buf(),
        "openbitfun".to_string(),
        "embedded-test",
    );

    let error = embedded
        .ensure_workspace_scope(workspace.path(), None, Some("host-only"))
        .expect_err("host-only facts must still protect local storage");

    assert_eq!(error.code(), "runtime_ownership_unavailable");
    drop(shared);
}

#[cfg(feature = "dispatch-store")]
#[tokio::test]
async fn dispatch_observer_record_never_acquires_local_workspace_ownership() {
    let ownership_root = tempdir().expect("ownership root");
    let workspace = tempdir().expect("workspace");
    let outbound_root = tempdir().expect("outbound root");
    let shared = CoreRuntimeOwnership::shared_with_facts(
        ownership_root.path().to_path_buf(),
        "openbitfun".to_string(),
        "shared-test",
        workspace.path(),
    )
    .expect("shared owner");
    let ownership_entries_before = std::fs::read_dir(ownership_root.path())
        .expect("read ownership root")
        .count();

    let store = OutboundDispatchStore::new_in_root_for_tests(outbound_root.path().join("outbound"));
    let workspace_path = workspace.path().to_string_lossy().into_owned();
    let record = OutboundDispatchRecord::new(
        "job-remote".to_string(),
        DispatchTarget::Ssh {
            connection_id: "server-a".to_string(),
            workspace_path: workspace_path.clone(),
            display_name: "Server A".to_string(),
        },
        "session-remote".to_string(),
        workspace_path,
        "Run only on the target",
        "queued",
    )
    .expect("record");
    store
        .bind_if_absent(&record)
        .await
        .expect("observer index must not contend for runtime ownership");

    assert_eq!(
        std::fs::read_dir(ownership_root.path())
            .expect("read ownership root")
            .count(),
        ownership_entries_before,
        "observer persistence must not create a local workspace lease"
    );
    drop(shared);
}

#[test]
fn startup_errors_expose_codes_without_mislabeling_path_failures_as_conflicts() {
    let ownership_root = tempdir().expect("ownership root");
    let owner = CoreRuntimeOwnership::embedded_with_facts(
        ownership_root.path().to_path_buf(),
        "openbitfun".to_string(),
        "test",
    );
    let missing = ownership_root.path().join("missing-workspace");

    let error = owner
        .ensure_local_workspace(&missing)
        .expect_err("missing workspace must fail");
    let message = error.startup_message(RuntimeDeployment::Embedded, "sdk-host");

    assert!(message.contains("canonicalize_workspace_failed"));
    assert!(!message.contains("Shared TUI Runtime owns"));
}

#[test]
fn ownership_conflict_guidance_matches_the_calling_product_surface() {
    let ownership_root = tempdir().expect("ownership root");
    let workspace = tempdir().expect("workspace");
    let shared = CoreRuntimeOwnership::shared_with_facts(
        ownership_root.path().to_path_buf(),
        "openbitfun".to_string(),
        "shared-tui-runtime",
        workspace.path(),
    )
    .expect("shared owner");
    let error = CoreRuntimeOwnership::embedded_with_facts(
        ownership_root.path().to_path_buf(),
        "openbitfun".to_string(),
        "sdk-host",
    )
    .ensure_local_workspace(workspace.path())
    .expect_err("Shared owner must block an Embedded SDK Host");

    let tui_message = error.startup_message(RuntimeDeployment::Embedded, "cli-interactive");
    assert!(tui_message.contains("openbitfun chat --shared"));
    for entrypoint in ["cli-headless", "acp", "sdk-host", "desktop"] {
        let message = error.startup_message(RuntimeDeployment::Embedded, entrypoint);
        assert!(
            !message.contains("openbitfun chat --shared"),
            "{entrypoint}"
        );
        assert!(message.contains("close its clients"), "{entrypoint}");
    }
    drop(shared);
}
