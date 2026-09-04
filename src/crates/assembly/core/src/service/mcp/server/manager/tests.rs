use std::sync::Arc;
use std::time::Duration;

use openbitfun_services_integrations::mcp::auth::rmcp_compat::StoredCredentials;
use openbitfun_services_integrations::mcp::auth::MCPRemoteOAuthCredentialVault;

async fn persisted_test_manager() -> (
    tempfile::TempDir,
    super::MCPServerManager,
    Arc<crate::service::mcp::config::MCPConfigService>,
) {
    let root = tempfile::tempdir().expect("tempdir");
    let path_manager = Arc::new(
        crate::infrastructure::PathManager::with_user_root_for_tests(root.path().join("config")),
    );
    let config_service = Arc::new(
        crate::service::config::ConfigService::with_settings(
            crate::service::config::ConfigManagerSettings {
                path_manager: Some(path_manager),
                auto_save: false,
                backup_count: 0,
            },
        )
        .await
        .expect("config service"),
    );
    let mcp_config_service = Arc::new(
        crate::service::mcp::config::MCPConfigService::new(config_service)
            .expect("MCP config service"),
    );
    let manager = super::MCPServerManager::assemble(Arc::clone(&mcp_config_service), None);
    (root, manager, mcp_config_service)
}

fn persisted_local_config(id: &str) -> crate::service::mcp::MCPServerConfig {
    use crate::service::mcp::{ConfigLocation, MCPServerTimeouts, MCPServerType};
    use std::collections::HashMap;

    crate::service::mcp::MCPServerConfig {
        id: id.to_string(),
        name: id.to_string(),
        server_type: MCPServerType::Local,
        transport: None,
        command: Some("unused-test-command".to_string()),
        args: Vec::new(),
        env: HashMap::new(),
        working_directory: None,
        inherit_parent_environment: None,
        headers: HashMap::new(),
        url: None,
        auto_start: false,
        enabled: true,
        location: ConfigLocation::User,
        capabilities: Vec::new(),
        settings: HashMap::new(),
        oauth: None,
        oauth_enabled: None,
        xaa: None,
        timeouts: MCPServerTimeouts::default(),
    }
}

#[test]
fn ephemeral_retirement_waits_for_in_flight_connection_users_but_is_bounded() {
    let grace = Duration::from_secs(30);
    assert!(super::should_finish_ephemeral_retirement(
        2,
        Duration::ZERO,
        grace
    ));
    assert!(!super::should_finish_ephemeral_retirement(
        3,
        Duration::from_secs(10),
        grace
    ));
    assert!(super::should_finish_ephemeral_retirement(3, grace, grace));
}

#[test]
fn retired_external_start_cannot_publish_after_handshake() {
    assert!(super::external_start_publication_allowed(false, true));
    assert!(super::external_start_publication_allowed(true, false));
    assert!(!super::external_start_publication_allowed(true, true));
}

#[test]
fn superseded_external_start_token_cannot_clean_up_current_instance() {
    let first = std::sync::Arc::new(());
    let current = std::sync::Arc::new(());

    assert!(super::external_start_token_is_current(Some(&first), &first));
    assert!(!super::external_start_token_is_current(
        Some(&current),
        &first
    ));
    assert!(!super::external_start_token_is_current(None, &first));
}

#[tokio::test]
async fn duplicate_persisted_lifecycle_operation_is_rejected_without_queueing() {
    let (_root, manager, _config_service) = persisted_test_manager().await;
    let first = manager
        .try_begin_persisted_server_operation("notion")
        .expect("first lifecycle operation should start");

    let duplicate = manager.try_begin_persisted_server_operation("notion");
    assert!(duplicate.is_err(), "duplicate operation must not be queued");
    assert!(duplicate
        .err()
        .expect("duplicate error")
        .to_string()
        .contains("already in progress"));

    assert!(
        manager
            .try_begin_persisted_server_operation("playwright")
            .is_ok(),
        "an unrelated server operation must remain independent"
    );

    drop(first);
    assert!(
        manager
            .try_begin_persisted_server_operation("notion")
            .is_ok(),
        "a later operation should start after the active operation finishes"
    );
}

#[tokio::test]
async fn repeated_initialization_preserves_manual_runtime_state() {
    use crate::service::mcp::MCPServerStatus;

    let (_root, manager, config_service) = persisted_test_manager().await;
    let config = persisted_local_config("existing");
    config_service
        .save_server_config(&config)
        .await
        .expect("save config");

    manager.initialize_all().await.expect("initialize runtime");
    manager.stop_server("existing").await.expect("stop runtime");
    assert_eq!(
        manager.get_server_status("existing").await.unwrap(),
        MCPServerStatus::Stopped
    );

    manager
        .initialize_all()
        .await
        .expect("repeat initialization");
    assert_eq!(
        manager.get_server_status("existing").await.unwrap(),
        MCPServerStatus::Stopped
    );
}

#[tokio::test]
async fn reconciling_an_addition_preserves_unchanged_runtime_state() {
    use crate::service::mcp::MCPServerStatus;

    let (_root, manager, config_service) = persisted_test_manager().await;
    let existing = persisted_local_config("existing");
    config_service
        .save_server_config(&existing)
        .await
        .expect("save existing config");
    manager.initialize_all().await.expect("initialize runtime");
    manager.stop_server("existing").await.expect("stop runtime");

    let previous = vec![existing.clone()];
    let added = persisted_local_config("added");
    let current = vec![existing.clone(), added];
    manager
        .reconcile_persisted_configs(previous, current)
        .await
        .expect("reconcile configs");

    assert_eq!(
        manager.get_server_status("existing").await.unwrap(),
        MCPServerStatus::Stopped
    );
    assert_eq!(
        manager.get_server_status("added").await.unwrap(),
        MCPServerStatus::Uninitialized
    );
}

#[tokio::test]
async fn failed_external_start_publishes_a_reason_before_runtime_cleanup() {
    use crate::service::mcp::{ConfigLocation, MCPServerConfig, MCPServerTimeouts, MCPServerType};
    use std::collections::HashMap;

    let root = tempfile::tempdir().expect("tempdir");
    let path_manager = Arc::new(
        crate::infrastructure::PathManager::with_user_root_for_tests(root.path().join("config")),
    );
    let config_service = Arc::new(
        crate::service::config::ConfigService::with_settings(
            crate::service::config::ConfigManagerSettings {
                path_manager: Some(path_manager),
                auto_save: false,
                backup_count: 0,
            },
        )
        .await
        .expect("config service"),
    );
    let mcp_config_service = Arc::new(
        crate::service::mcp::config::MCPConfigService::new(config_service)
            .expect("MCP config service"),
    );
    let manager = super::MCPServerManager::assemble(mcp_config_service, None);
    let server_id = "external-mcp-failed-start";

    manager
        .install_external_ephemeral_server(
            MCPServerConfig {
                id: server_id.to_string(),
                name: "Unavailable external MCP".to_string(),
                server_type: MCPServerType::Local,
                transport: None,
                command: Some("openbitfun-command-that-does-not-exist".to_string()),
                args: Vec::new(),
                env: HashMap::new(),
                working_directory: None,
                inherit_parent_environment: Some(false),
                headers: HashMap::new(),
                url: None,
                auto_start: true,
                enabled: true,
                location: ConfigLocation::User,
                capabilities: Vec::new(),
                settings: HashMap::new(),
                oauth: None,
                oauth_enabled: None,
                xaa: None,
                timeouts: MCPServerTimeouts::default(),
            },
            "workspace-a".to_string(),
        )
        .await
        .expect("registration is asynchronous");

    let reason = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if let Some(failure) = manager.external_server_start_failure(server_id).await {
                break failure;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("failed startup should publish a reason");
    assert_eq!(reason, super::MCPServerStartFailure::CommandUnavailable);
}

#[tokio::test]
async fn oauth_credentials_follow_the_manager_injected_data_dir() {
    let root = tempfile::tempdir().expect("tempdir");
    let path_manager = Arc::new(
        crate::infrastructure::PathManager::with_user_root_for_tests(root.path().join("config")),
    );
    let config_service = Arc::new(
        crate::service::config::ConfigService::with_settings(
            crate::service::config::ConfigManagerSettings {
                path_manager: Some(path_manager),
                auto_save: false,
                backup_count: 0,
            },
        )
        .await
        .expect("config service"),
    );
    let mcp_config_service = Arc::new(
        crate::service::mcp::config::MCPConfigService::new(config_service)
            .expect("MCP config service"),
    );
    let oauth_data_dir = root.path().join("oauth");
    let manager =
        super::MCPServerManager::assemble(mcp_config_service, Some(oauth_data_dir.clone()));
    let vault = MCPRemoteOAuthCredentialVault::new(oauth_data_dir);
    let credentials: StoredCredentials = serde_json::from_value(serde_json::json!({
        "client_id": "client-123",
        "token_response": {
            "access_token": "access-token",
            "token_type": "bearer"
        }
    }))
    .expect("stored credentials");

    vault
        .store("server-a", &credentials)
        .await
        .expect("store credentials");

    assert!(manager
        .has_remote_oauth_credentials("server-a")
        .await
        .expect("query credentials"));

    manager
        .clear_remote_oauth_credentials("server-a")
        .await
        .expect("clear credentials");
    assert!(vault
        .load("server-a")
        .await
        .expect("load credentials after clear")
        .is_none());
}
