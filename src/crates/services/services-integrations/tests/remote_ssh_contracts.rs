#![cfg(feature = "remote-ssh")]

use bitfun_services_integrations::remote_ssh::{
    canonicalize_local_workspace_root, local_workspace_roots_equal,
    local_workspace_stable_storage_id, normalize_local_workspace_root_for_stable_id,
    normalize_remote_workspace_path, remote_root_to_mirror_subpath, remote_workspace_runtime_root,
    remote_workspace_session_mirror_dir, remote_workspace_stable_id,
    sanitize_remote_mirror_path_component, sanitize_ssh_connection_id_for_local_dir,
    sanitize_ssh_hostname_for_mirror, unresolved_remote_session_storage_dir,
    unresolved_remote_session_storage_key, workspace_logical_key, workspace_session_identity,
    ContainerAccess, ContainerWorkspaceConfig, RemoteWorkspace, RemoteWorkspaceRegistry,
    SSHAuthMethod, SSHConnectionConfig, SavedAuthType, SavedConnection, LOCAL_WORKSPACE_SSH_HOST,
};

#[test]
fn remote_ssh_legacy_agent_auth_keeps_default_private_key_fallback() {
    let config: SSHConnectionConfig = serde_json::from_value(serde_json::json!({
        "id": "conn-1",
        "name": "dev",
        "host": "example.com",
        "port": 22,
        "username": "alice",
        "auth": { "type": "Agent" },
        "defaultWorkspace": "/repo"
    }))
    .unwrap();

    match config.auth {
        SSHAuthMethod::Agent {
            key_fingerprint,
            fallback_key_path,
        } => {
            assert_eq!(key_fingerprint, None);
            assert_eq!(fallback_key_path.as_deref(), Some("~/.ssh/id_rsa"));
        }
        _ => panic!("legacy agent auth must remain agent-compatible"),
    }
    assert_eq!(config.proxy_jump, None);
    assert_eq!(config.container, None);
    assert_eq!(config.options.connect_timeout_secs, 30);
    assert_eq!(config.options.auth_timeout_secs, 60);
    assert_eq!(config.options.auth_attempts, 3);
    assert_eq!(config.options.connect_attempts, 1);

    let saved: SavedConnection = serde_json::from_value(serde_json::json!({
        "id": "conn-1",
        "name": "dev",
        "host": "example.com",
        "port": 22,
        "username": "alice",
        "authType": { "type": "Agent" },
        "defaultWorkspace": "/repo",
        "lastConnected": 1
    }))
    .unwrap();

    assert!(matches!(
        saved.auth_type,
        SavedAuthType::Agent {
            key_fingerprint: None,
            ref fallback_key_path,
        } if fallback_key_path.as_deref() == Some("~/.ssh/id_rsa")
    ));
    assert_eq!(saved.proxy_jump, None);
    assert_eq!(saved.container, None);
    assert_eq!(saved.options.connect_timeout_secs, 30);
    assert_eq!(saved.options.auth_timeout_secs, 60);
    assert_eq!(saved.options.auth_attempts, 3);
    assert_eq!(saved.options.connect_attempts, 1);
}

#[test]
fn remote_target_contract_uses_proxy_jump_and_kebab_case_container_access() {
    let config = SSHConnectionConfig {
        id: "conn-1".to_string(),
        name: "train".to_string(),
        host: "train.internal".to_string(),
        port: 22,
        username: "trainer".to_string(),
        auth: SSHAuthMethod::PrivateKey {
            key_path: "~/.ssh/train".to_string(),
            passphrase: None,
            certificate_path: None,
        },
        default_workspace: Some("/workspace".to_string()),
        proxy_jump: Some("jump1,jump2".to_string()),
        container: Some(ContainerWorkspaceConfig {
            name: "trainer-dev".to_string(),
            access: ContainerAccess::DockerExec,
            local: false,
            docker_path: "docker".to_string(),
            shell: "/bin/bash".to_string(),
            user: Some("trainer".to_string()),
            interactive: true,
        }),
        options: Default::default(),
    };

    let json = serde_json::to_value(&config).unwrap();
    assert_eq!(json["proxyJump"], "jump1,jump2");
    assert_eq!(json["container"]["access"], "docker-exec");
    assert_eq!(json["container"]["dockerPath"], "docker");
    let round_trip: SSHConnectionConfig = serde_json::from_value(json).unwrap();
    assert_eq!(
        round_trip.container.unwrap().access,
        ContainerAccess::DockerExec
    );
}

#[test]
fn remote_workspace_defaults_keep_older_files_loadable() {
    let workspace: RemoteWorkspace = serde_json::from_value(serde_json::json!({
        "connectionId": "conn-1"
    }))
    .unwrap();

    assert_eq!(workspace.connection_id, "conn-1");
    assert_eq!(workspace.remote_path, "");
    assert_eq!(workspace.connection_name, "");
    assert_eq!(workspace.ssh_host, "");
}

#[test]
fn remote_workspace_path_helpers_preserve_current_identity_contract() {
    assert_eq!(
        normalize_remote_workspace_path(r"\\home\\user\\repo//src"),
        "/home/user/repo/src"
    );
    assert_eq!(normalize_remote_workspace_path("///"), "/");
    assert_eq!(
        normalize_remote_workspace_path("/home/user/repo/"),
        "/home/user/repo"
    );

    #[cfg(windows)]
    assert_eq!(
        sanitize_ssh_connection_id_for_local_dir("ssh-root@1.95.50.146:22"),
        "ssh-root@1.95.50.146-22"
    );
    #[cfg(not(windows))]
    assert_eq!(
        sanitize_ssh_connection_id_for_local_dir("ssh-root@1.95.50.146:22"),
        "ssh-root@1.95.50.146:22"
    );
    assert_eq!(
        sanitize_ssh_connection_id_for_local_dir("../unsafe/id"),
        "..-unsafe-id"
    );
    assert_eq!(sanitize_ssh_connection_id_for_local_dir(".."), "_dotdot_");

    assert_eq!(sanitize_remote_mirror_path_component(""), "_");
    assert_eq!(sanitize_remote_mirror_path_component("."), "_dot_");
    assert_eq!(sanitize_remote_mirror_path_component(".."), "_dotdot_");
    assert!(remote_root_to_mirror_subpath("/../../escape")
        .components()
        .all(|component| !matches!(component, std::path::Component::ParentDir)));
    assert_eq!(
        remote_root_to_mirror_subpath("/home/user/../project"),
        std::path::PathBuf::from("home").join("project"),
        "safe legacy dot segments must keep their previous effective mirror path"
    );
    #[cfg(windows)]
    {
        assert_eq!(sanitize_remote_mirror_path_component("CON"), "_CON");
        assert_eq!(sanitize_remote_mirror_path_component("report. "), "report");
    }
    assert_eq!(
        sanitize_ssh_hostname_for_mirror(" Example.COM "),
        "example.com"
    );
    assert_eq!(
        remote_root_to_mirror_subpath("/home/user/repo"),
        std::path::PathBuf::from("home").join("user").join("repo")
    );
    assert_eq!(
        remote_root_to_mirror_subpath("/"),
        std::path::PathBuf::from("_root")
    );

    assert_eq!(
        workspace_logical_key(LOCAL_WORKSPACE_SSH_HOST, "/Users/p/w"),
        "localhost:/Users/p/w"
    );

    let local_id = local_workspace_stable_storage_id("/Users/foo/BitFun");
    assert_eq!(local_id, "local_1d9bbee7a88cb84fc9500423130a3e99");

    let remote_id = remote_workspace_stable_id("myhost", "/root/proj");
    assert_eq!(remote_id, "remote_0b6e9c54b3e51fd56bf721ed35c1ce88");

    let unresolved_key = unresolved_remote_session_storage_key(" conn-1 ", "/home/u/p");
    assert_eq!(unresolved_key, "d1c72f60fc1b7cb99599cf21");
}

#[test]
fn remote_workspace_session_paths_use_supplied_mirror_root() {
    let mirror_root = std::path::PathBuf::from("/bitfun/remote_ssh");

    assert_eq!(
        remote_workspace_runtime_root(&mirror_root, " Example.COM ", "/home/user/repo"),
        mirror_root
            .join("example.com")
            .join("home")
            .join("user")
            .join("repo")
    );
    assert_eq!(
        remote_workspace_session_mirror_dir(&mirror_root, " Example.COM ", "/"),
        mirror_root
            .join("example.com")
            .join("_root")
            .join("sessions")
    );
    assert_eq!(
        unresolved_remote_session_storage_dir(&mirror_root, " conn-1 ", "/home/u/p"),
        mirror_root
            .join("_unresolved")
            .join("d1c72f60fc1b7cb99599cf21")
            .join("sessions")
    );
}

#[test]
fn local_workspace_identity_helpers_preserve_canonical_root_contract() {
    let workspace_root = std::env::temp_dir().join(format!(
        "bitfun-services-remote-ssh-contract-{}",
        std::process::id()
    ));
    let nested = workspace_root.join("nested");
    std::fs::create_dir_all(&nested).expect("workspace root should exist");

    let (canonical_path, stable_root) =
        canonicalize_local_workspace_root(&workspace_root).expect("canonical local root");
    assert_eq!(
        stable_root,
        normalize_local_workspace_root_for_stable_id(&workspace_root)
            .expect("normalized local root")
    );
    assert_eq!(
        stable_root,
        canonical_path.to_string_lossy().replace('\\', "/")
    );
    assert!(local_workspace_roots_equal(
        &workspace_root,
        &workspace_root
    ));
    assert!(!local_workspace_roots_equal(&workspace_root, &nested));

    let _ = std::fs::remove_dir_all(workspace_root);
}

#[test]
fn workspace_session_identity_preserves_local_and_remote_contracts() {
    let workspace_root = std::env::temp_dir().join(format!(
        "bitfun-services-workspace-identity-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&workspace_root).expect("workspace root should exist");

    let local =
        workspace_session_identity(&workspace_root.to_string_lossy(), None, None).expect("local");
    assert_eq!(local.hostname, LOCAL_WORKSPACE_SSH_HOST);
    assert!(!local.is_remote());
    assert_eq!(local.remote_connection_id, None);

    let remote = workspace_session_identity(
        r"\\home\\wsp\\project//",
        Some(" conn-1 "),
        Some(" ssh.dev "),
    )
    .expect("remote");
    assert_eq!(remote.hostname, "ssh.dev");
    assert_eq!(remote.logical_workspace_path(), "/home/wsp/project");
    assert_eq!(remote.remote_connection_id.as_deref(), Some("conn-1"));
    assert!(remote.is_remote());

    assert!(
        workspace_session_identity("/home/wsp/project", Some("conn-1"), None).is_none(),
        "remote identity requires a resolvable SSH host"
    );

    let _ = std::fs::remove_dir_all(workspace_root);
}

#[tokio::test]
async fn remote_workspace_registry_preserves_ambiguous_root_resolution_contract() {
    let registry = RemoteWorkspaceRegistry::new();
    registry
        .register_remote_workspace(
            "/".to_string(),
            "conn-a".to_string(),
            "Server A".to_string(),
            "host-a".to_string(),
        )
        .await;
    registry
        .register_remote_workspace(
            "/".to_string(),
            "conn-b".to_string(),
            "Server B".to_string(),
            "host-b".to_string(),
        )
        .await;

    assert!(registry.lookup_connection("/tmp", None).await.is_none());

    registry
        .set_active_connection_hint(Some("conn-a".to_string()))
        .await;
    let hinted = registry.lookup_connection("/tmp", None).await.unwrap();
    assert_eq!(hinted.connection_id, "conn-a");
    assert_eq!(hinted.ssh_host, "host-a");

    let preferred = registry
        .lookup_connection("/tmp", Some("conn-b"))
        .await
        .unwrap();
    assert_eq!(preferred.connection_id, "conn-b");
    assert_eq!(preferred.ssh_host, "host-b");
}

#[tokio::test]
async fn remote_workspace_registry_preserves_legacy_state_and_clear_contract() {
    let registry = RemoteWorkspaceRegistry::new();
    assert!(!registry.has_any().await);
    assert!(!registry.get_state().await.is_active);

    registry
        .register_remote_workspace(
            "/repo".to_string(),
            "conn-1".to_string(),
            "Dev Server".to_string(),
            "dev.example.com".to_string(),
        )
        .await;

    let state = registry.get_state().await;
    assert!(state.is_active);
    assert_eq!(state.connection_id.as_deref(), Some("conn-1"));
    assert_eq!(state.remote_path.as_deref(), Some("/repo"));
    assert_eq!(state.connection_name.as_deref(), Some("Dev Server"));

    registry
        .unregister_remote_workspace("conn-1", "/repo")
        .await;
    assert!(!registry.has_any().await);
    assert!(!registry.get_state().await.is_active);
}
