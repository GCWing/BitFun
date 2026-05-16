#![cfg(feature = "announcement")]

use bitfun_services_integrations::announcement::{
    AnnouncementCard, AnnouncementState, AnnouncementStateStore, CardSource, CardType,
    CompletionAction, ModalConfig, ModalPage, ModalSize, PageLayout, ToastConfig, TriggerCondition,
    TriggerRule,
};

#[test]
fn announcement_card_deserialization_preserves_default_contract() {
    let card: AnnouncementCard = serde_json::from_value(serde_json::json!({
        "id": "feature_v1",
        "card_type": "feature",
        "source": "local",
        "trigger": {
            "condition": {
                "type": "app_nth_open",
                "n": 3
            }
        },
        "toast": {
            "icon": "sparkles",
            "title": "Feature",
            "description": "Try it"
        }
    }))
    .unwrap();

    assert_eq!(card.id, "feature_v1");
    assert_eq!(card.card_type, CardType::Feature);
    assert_eq!(card.source, CardSource::Local);
    assert_eq!(card.priority, 0);
    assert_eq!(card.app_version, None);
    assert!(card.modal.is_none());
    assert_eq!(card.expires_at, None);
    assert!(matches!(
        card.trigger.condition,
        TriggerCondition::AppNthOpen { n: 3 }
    ));
    assert_eq!(card.trigger.delay_ms, 0);
    assert!(card.trigger.once_per_version);
    assert_eq!(card.toast.action_label, "");
    assert!(card.toast.dismissible);
    assert_eq!(card.toast.auto_dismiss_ms, None);
}

#[test]
fn announcement_modal_serialization_preserves_snake_case_contract() {
    let modal = ModalConfig {
        size: ModalSize::Xl,
        closable: true,
        pages: vec![ModalPage {
            layout: PageLayout::FullscreenMedia,
            title: "Showcase".to_string(),
            body: "Details".to_string(),
            media: None,
        }],
        completion_action: CompletionAction::NeverShowAgain,
    };

    assert_eq!(
        serde_json::to_value(modal).unwrap(),
        serde_json::json!({
            "size": "xl",
            "closable": true,
            "pages": [{
                "layout": "fullscreen_media",
                "title": "Showcase",
                "body": "Details",
                "media": null
            }],
            "completion_action": "never_show_again"
        })
    );
}

#[test]
fn announcement_state_and_trigger_defaults_preserve_runtime_assumptions() {
    let trigger = TriggerRule::default();
    assert!(matches!(
        trigger.condition,
        TriggerCondition::VersionFirstOpen
    ));
    assert_eq!(trigger.delay_ms, 2000);
    assert!(trigger.once_per_version);

    let state = AnnouncementState::default();
    assert_eq!(state.last_seen_version, "");
    assert_eq!(state.app_open_count, 0);
    assert!(state.seen_ids.is_empty());
    assert!(state.dismissed_ids.is_empty());
    assert!(state.never_show_ids.is_empty());
    assert_eq!(state.last_remote_fetch_at, None);

    let toast = ToastConfig {
        icon: "tip".to_string(),
        title: "Tip".to_string(),
        description: "Use shortcuts".to_string(),
        action_label: String::new(),
        dismissible: true,
        auto_dismiss_ms: None,
    };
    assert!(toast.dismissible);
}

#[tokio::test]
async fn announcement_state_store_round_trips_state_and_defaults_missing_file() {
    let root = std::env::temp_dir().join(format!(
        "bitfun-announcement-state-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ));
    let store = AnnouncementStateStore::new(&root);

    let missing = store.load().await.expect("load missing state");
    assert_eq!(missing.last_seen_version, "");
    assert_eq!(missing.app_open_count, 0);
    assert!(missing.seen_ids.is_empty());
    assert!(missing.dismissed_ids.is_empty());
    assert!(missing.never_show_ids.is_empty());
    assert_eq!(missing.last_remote_fetch_at, None);

    let mut state = AnnouncementState::default();
    state.app_open_count = 7;
    state.seen_ids.insert("feature-a".to_string());
    state.dismissed_ids.insert("tip-b".to_string());
    store.save(&state).await.expect("save state");

    let loaded = store.load().await.expect("load saved state");
    assert_eq!(loaded.app_open_count, 7);
    assert!(loaded.seen_ids.contains("feature-a"));
    assert!(loaded.dismissed_ids.contains("tip-b"));

    let _ = std::fs::remove_dir_all(root);
}
