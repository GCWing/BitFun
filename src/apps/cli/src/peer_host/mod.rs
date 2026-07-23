//! CLI Peer Device Mode host.
//!
//! Desktop controllers reach this process over the same HostInvoke / DeviceEvent
//! envelopes used for Desktop peers. Execution goes through Core services
//! (no webview / Tauri bridge).

mod args;
mod bootstrap;
mod commands;
mod control;
mod deny;
mod dispatch;
mod fanout;
mod state;
mod workspace_dto;

pub(crate) use bootstrap::ensure_peer_host_ready;
pub(crate) use dispatch::{handle_device_event_command, handle_host_invoke};

/// Fan out an `account://settings-applied` DeviceEvent to attached Peer Mode
/// controllers so they refresh their config cache after this host's settings
/// changed (cloud pull or local edit). No-op when no controller is attached.
pub(crate) fn notify_controllers_settings_changed() {
    tokio::spawn(async {
        fanout::fanout_peer_device_event(
            "account://settings-applied".to_string(),
            serde_json::json!({ "applied": true }),
        )
        .await;
    });
}

pub(crate) async fn update_controller_presence(online_device_ids: Vec<String>) {
    let lost_last_controller =
        control::retain_online_controllers(online_device_ids.iter().map(String::as_str)).await;
    if !lost_last_controller {
        return;
    }
    if let Some(state) = state::try_peer_host_state() {
        if let Err(error) = state
            .cancel_and_drain_peer_turns("last Peer controller went offline")
            .await
        {
            tracing::warn!("Peer work was not fully cancelled after controller loss: {error}");
        }
    }
}
