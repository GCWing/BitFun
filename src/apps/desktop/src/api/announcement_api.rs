//! Announcement system Tauri commands.

use crate::api::app_state::AppState;
use bitfun_core::service::announcement::{AnnouncementCard, CardType};
use serde::Deserialize;
use tauri::State;

#[derive(Debug, Deserialize)]
pub struct AnnouncementIdRequest {
    pub id: String,
}

/// Return the ordered list of cards that should be displayed in this session.
///
/// This triggers the scheduler (updates open count / version state) and returns
/// cards that pass all filter rules.  Call once per application start.
/// Built-in tip cards are excluded when `app.notifications.enable_startup_tips` is false.
#[tauri::command]
pub async fn get_pending_announcements(
    state: State<'_, AppState>,
) -> Result<Vec<AnnouncementCard>, String> {
    let locale = state
        .config_service
        .get_config::<String>(Some("app.general.language"))
        .await
        .unwrap_or_else(|_| "zh-CN".to_string());

    let mut cards = state
        .announcement_scheduler
        .run(&locale)
        .await
        .map_err(|e| format!("Failed to get pending announcements: {}", e))?;

    // Respect the user preference for startup tips.
    let tips_enabled: bool = state
        .config_service
        .get_config::<bool>(Some("app.notifications.enable_startup_tips"))
        .await
        .unwrap_or(true); // default on if config is missing / unset

    if !tips_enabled {
        cards.retain(|c| c.card_type != CardType::Tip);
    }

    Ok(cards)
}

/// Mark a card as seen (the user opened its modal or acknowledged it).
///
/// Seen cards with `once_per_version = true` will not be shown again in the
/// current version cycle.
#[tauri::command]
pub async fn mark_announcement_seen(
    state: State<'_, AppState>,
    request: AnnouncementIdRequest,
) -> Result<(), String> {
    state
        .announcement_scheduler
        .mark_seen(&request.id)
        .await
        .map_err(|e| format!("Failed to mark announcement seen: {}", e))
}

/// Dismiss a card for the current version cycle (closed without acting).
///
/// Dismissed cards will not reappear until a version upgrade.
#[tauri::command]
pub async fn dismiss_announcement(
    state: State<'_, AppState>,
    request: AnnouncementIdRequest,
) -> Result<(), String> {
    state
        .announcement_scheduler
        .dismiss(&request.id)
        .await
        .map_err(|e| format!("Failed to dismiss announcement: {}", e))
}

/// Permanently suppress a card (user selected "don't show again").
#[tauri::command]
pub async fn never_show_announcement(
    state: State<'_, AppState>,
    request: AnnouncementIdRequest,
) -> Result<(), String> {
    state
        .announcement_scheduler
        .never_show(&request.id)
        .await
        .map_err(|e| format!("Failed to suppress announcement: {}", e))
}

/// Manually trigger a specific card by ID (e.g. from a "What's New" menu item).
///
/// Returns `None` if no card with the given ID is registered.
#[tauri::command]
pub async fn trigger_announcement(
    state: State<'_, AppState>,
    request: AnnouncementIdRequest,
) -> Result<Option<AnnouncementCard>, String> {
    let locale = state
        .config_service
        .get_config::<String>(Some("app.general.language"))
        .await
        .unwrap_or_else(|_| "zh-CN".to_string());

    Ok(state
        .announcement_scheduler
        .trigger_card(&request.id, &locale)
        .await)
}

/// Return all currently eligible tip cards (for a dedicated tips browser).
#[tauri::command]
pub async fn get_announcement_tips(
    state: State<'_, AppState>,
) -> Result<Vec<AnnouncementCard>, String> {
    let locale = state
        .config_service
        .get_config::<String>(Some("app.general.language"))
        .await
        .unwrap_or_else(|_| "zh-CN".to_string());

    // Re-use the scheduler run result but filter to tips only.
    let cards = state
        .announcement_scheduler
        .run(&locale)
        .await
        .map_err(|e| format!("Failed to get tips: {}", e))?;

    let tips = cards
        .into_iter()
        .filter(|c| {
            matches!(
                c.card_type,
                bitfun_core::service::announcement::types::CardType::Tip
            )
        })
        .collect();

    Ok(tips)
}
