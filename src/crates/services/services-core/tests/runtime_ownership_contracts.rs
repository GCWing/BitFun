use bitfun_services_core::runtime_ownership::{
    RuntimeDeployment, RuntimeOwnershipKey, WorkspaceRuntimeOwnership,
};
use tempfile::tempdir;

#[test]
fn ownership_key_is_stable_and_scoped_by_workspace_and_product() {
    let first_workspace = tempdir().expect("first workspace");
    let second_workspace = tempdir().expect("second workspace");

    let first = RuntimeOwnershipKey::for_workspace(first_workspace.path(), "bitfun")
        .expect("create first ownership key");
    let same = RuntimeOwnershipKey::for_workspace(&first_workspace.path().join("."), "bitfun")
        .expect("normalize same workspace");
    let other_workspace = RuntimeOwnershipKey::for_workspace(second_workspace.path(), "bitfun")
        .expect("create second ownership key");
    let other_product =
        RuntimeOwnershipKey::for_workspace(first_workspace.path(), "bitfun-preview")
            .expect("create product-scoped ownership key");

    assert_eq!(first, same);
    assert_ne!(first, other_workspace);
    assert_ne!(first, other_product);
    assert_eq!(first.as_str().len(), 64);
    assert!(first.as_str().bytes().all(|byte| byte.is_ascii_hexdigit()));
}

#[cfg(unix)]
#[test]
fn ownership_key_preserves_non_utf8_workspace_bytes() {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;

    let root = tempdir().expect("workspace root");
    let first_path = root.path().join(OsString::from_vec(vec![b'w', 0x80]));
    let second_path = root.path().join(OsString::from_vec(vec![b'w', 0x81]));
    std::fs::create_dir(&first_path).expect("first non-UTF-8 workspace");
    std::fs::create_dir(&second_path).expect("second non-UTF-8 workspace");

    let first =
        RuntimeOwnershipKey::for_workspace(&first_path, "bitfun").expect("first ownership key");
    let second =
        RuntimeOwnershipKey::for_workspace(&second_path, "bitfun").expect("second ownership key");

    assert_ne!(first, second);
}

#[test]
fn embedded_owners_can_coexist_but_shared_ownership_is_exclusive() {
    let workspace = tempdir().expect("workspace");
    let ownership_root = tempdir().expect("ownership root");
    let key = RuntimeOwnershipKey::for_workspace(workspace.path(), "bitfun")
        .expect("create ownership key");

    let embedded_one = WorkspaceRuntimeOwnership::try_acquire(
        ownership_root.path(),
        &key,
        RuntimeDeployment::Embedded,
    )
    .expect("first embedded owner");
    let embedded_two = WorkspaceRuntimeOwnership::try_acquire(
        ownership_root.path(),
        &key,
        RuntimeDeployment::Embedded,
    )
    .expect("second embedded owner");

    assert!(WorkspaceRuntimeOwnership::try_acquire(
        ownership_root.path(),
        &key,
        RuntimeDeployment::Shared,
    )
    .is_err());

    drop(embedded_one);
    drop(embedded_two);

    let shared = WorkspaceRuntimeOwnership::try_acquire(
        ownership_root.path(),
        &key,
        RuntimeDeployment::Shared,
    )
    .expect("shared owner after embedded owners release");
    assert!(WorkspaceRuntimeOwnership::try_acquire(
        ownership_root.path(),
        &key,
        RuntimeDeployment::Embedded,
    )
    .is_err());
    drop(shared);

    WorkspaceRuntimeOwnership::try_acquire(
        ownership_root.path(),
        &key,
        RuntimeDeployment::Embedded,
    )
    .expect("ownership released by RAII drop");
}
