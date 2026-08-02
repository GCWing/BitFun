use std::time::Duration;

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
