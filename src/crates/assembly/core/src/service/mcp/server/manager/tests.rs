use std::sync::Arc;
use std::time::Duration;

use bitfun_services_integrations::mcp::auth::rmcp_compat::StoredCredentials;
use bitfun_services_integrations::mcp::auth::MCPRemoteOAuthCredentialVault;

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
