use bitfun_services_integrations::mcp::server::{
    compute_mcp_backoff_delay, detect_mcp_list_changed_kind, MCPListChangedKind,
};
use std::time::Duration;

#[test]
fn backoff_delay_grows_exponentially_and_caps() {
    let base = Duration::from_secs(2);
    let max = Duration::from_secs(60);

    assert_eq!(
        compute_mcp_backoff_delay(base, max, 1),
        Duration::from_secs(2)
    );
    assert_eq!(
        compute_mcp_backoff_delay(base, max, 2),
        Duration::from_secs(4)
    );
    assert_eq!(
        compute_mcp_backoff_delay(base, max, 5),
        Duration::from_secs(32)
    );
    assert_eq!(
        compute_mcp_backoff_delay(base, max, 10),
        Duration::from_secs(60)
    );
}

#[test]
fn detect_list_changed_kind_supports_three_catalogs() {
    assert_eq!(
        detect_mcp_list_changed_kind("notifications/tools/list_changed"),
        Some(MCPListChangedKind::Tools)
    );
    assert_eq!(
        detect_mcp_list_changed_kind("notifications/prompts/list_changed"),
        Some(MCPListChangedKind::Prompts)
    );
    assert_eq!(
        detect_mcp_list_changed_kind("notifications/resources/list_changed"),
        Some(MCPListChangedKind::Resources)
    );
    assert_eq!(detect_mcp_list_changed_kind("notifications/unknown"), None);
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
