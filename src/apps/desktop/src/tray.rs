//! System tray integration for BitFun Desktop.
//!
//! Creates a system tray icon with a context menu. On Windows and Linux the tray
//! icon is always visible while the process is running; on macOS the icon appears
//! in the macOS menu bar.
//!
//! Left-click  – toggles the main window (show / hide).
//! Right-click – opens a context menu with:
//!   • toggle desktop Agent companion pet (persisted via `app.ai_experience`)
//!   • "Show BitFun"
//!   • "Quit BitFun"
//!
//! The context menu is rebuilt every time the user left-clicks (for freshness),
//! periodically, and after locale changes.

use std::sync::OnceLock;

use tauri::menu::{CheckMenuItemBuilder, MenuBuilder, MenuItemBuilder};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager};

use bitfun_core::service::config::app_language::get_app_language;
use bitfun_core::service::config::types::AIExperienceConfig;
use bitfun_core::service::i18n::LocaleId;

use crate::api::app_state::AppState;

static TRAY_ICON: OnceLock<tauri::tray::TrayIcon> = OnceLock::new();

struct TrayStrings {
    show_app: &'static str,
    quit_app: &'static str,
    desktop_pet: &'static str,
}

const STRINGS_ZH_CN: TrayStrings = TrayStrings {
    show_app: "显示 BitFun",
    quit_app: "退出 BitFun",
    desktop_pet: "显示桌面宠物",
};

const STRINGS_ZH_TW: TrayStrings = TrayStrings {
    show_app: "顯示 BitFun",
    quit_app: "退出 BitFun",
    desktop_pet: "顯示桌面寵物",
};

const STRINGS_EN_US: TrayStrings = TrayStrings {
    show_app: "Show BitFun",
    quit_app: "Quit BitFun",
    desktop_pet: "Show desktop pet",
};

fn tray_strings(locale: &LocaleId) -> &'static TrayStrings {
    match locale {
        LocaleId::ZhCN => &STRINGS_ZH_CN,
        LocaleId::ZhTW => &STRINGS_ZH_TW,
        LocaleId::EnUS => &STRINGS_EN_US,
    }
}

fn desktop_pet_should_show(exp: &AIExperienceConfig) -> bool {
    exp.enable_agent_companion && exp.agent_companion_display_mode == "desktop"
}

async fn load_ai_experience(app: &AppHandle) -> Option<AIExperienceConfig> {
    let app_state = app.try_state::<AppState>()?;
    app_state
        .config_service
        .get_config(Some("app.ai_experience"))
        .await
        .ok()
}

pub async fn rebuild_tray_menu_public(app: &AppHandle) {
    rebuild_tray_menu(app).await;
}

async fn rebuild_tray_menu(app: &AppHandle) {
    let locale = get_app_language().await;
    let s = tray_strings(&locale);

    let tray = match TRAY_ICON.get() {
        Some(t) => t,
        None => return,
    };

    let pet_checked = load_ai_experience(app)
        .await
        .as_ref()
        .map(desktop_pet_should_show)
        .unwrap_or(false);

    let pet_item = match CheckMenuItemBuilder::with_id("toggle_desktop_pet", s.desktop_pet)
        .checked(pet_checked)
        .build(app)
    {
        Ok(i) => i,
        Err(_) => return,
    };

    let show_item = match MenuItemBuilder::with_id("show_window", s.show_app).build(app) {
        Ok(i) => i,
        Err(_) => return,
    };
    let quit_item = match MenuItemBuilder::with_id("quit", s.quit_app).build(app) {
        Ok(i) => i,
        Err(_) => return,
    };

    let menu = match MenuBuilder::new(app)
        .item(&pet_item)
        .separator()
        .item(&show_item)
        .separator()
        .item(&quit_item)
        .build()
    {
        Ok(m) => m,
        Err(e) => {
            log::warn!("Failed to build tray menu: {}", e);
            return;
        }
    };

    if let Err(e) = tray.set_menu(Some(menu)) {
        log::warn!("Failed to update tray menu: {}", e);
    }
}

async fn tray_toggle_desktop_pet(app: &AppHandle) -> Result<(), String> {
    let app_state = app
        .try_state::<AppState>()
        .ok_or_else(|| "AppState not available".to_string())?;
    let config_service = &app_state.config_service;

    let mut exp: AIExperienceConfig = config_service
        .get_config(Some("app.ai_experience"))
        .await
        .map_err(|e| e.to_string())?;

    let desktop_on = desktop_pet_should_show(&exp);

    if desktop_on {
        exp.enable_agent_companion = false;
    } else {
        exp.enable_agent_companion = true;
        exp.agent_companion_display_mode = "desktop".to_string();
    }

    config_service
        .set_config("app.ai_experience", &exp)
        .await
        .map_err(|e| e.to_string())?;

    let show = desktop_pet_should_show(&exp);
    if show {
        crate::theme::show_agent_companion_desktop_pet(app.clone()).await?;
    } else {
        crate::theme::hide_agent_companion_desktop_pet(app.clone()).await?;
    }

    Ok(())
}

/// Build and attach the system tray icon to the Tauri application.
pub fn setup_tray(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let pet_item = CheckMenuItemBuilder::with_id("toggle_desktop_pet", STRINGS_EN_US.desktop_pet)
        .checked(false)
        .build(app)?;
    let show_item = MenuItemBuilder::with_id("show_window", STRINGS_EN_US.show_app).build(app)?;
    let quit_item = MenuItemBuilder::with_id("quit", STRINGS_EN_US.quit_app).build(app)?;

    let initial_menu = MenuBuilder::new(app)
        .item(&pet_item)
        .separator()
        .item(&show_item)
        .separator()
        .item(&quit_item)
        .build()?;

    let icon = app
        .default_window_icon()
        .ok_or("No default window icon")?
        .clone();

    let tray = TrayIconBuilder::new()
        .icon(icon)
        .menu(&initial_menu)
        .tooltip("BitFun")
        .on_menu_event(|app, event| {
            let id = event.id.as_ref();
            if id == "show_window" {
                show_main_window(app);
            } else if id == "quit" {
                log::info!("Quit requested from tray menu");
                crate::perform_process_exit_cleanup();
                app.exit(0);
            } else if id == "toggle_desktop_pet" {
                let app_handle = app.clone();
                tauri::async_runtime::spawn(async move {
                    if let Err(e) = tray_toggle_desktop_pet(&app_handle).await {
                        log::warn!("Tray desktop pet toggle failed: {}", e);
                    }
                    rebuild_tray_menu(&app_handle).await;
                });
            }
        })
        .on_tray_icon_event(|tray, event| match event {
            TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } => {
                let app = tray.app_handle().clone();
                toggle_main_window(&app);
                tauri::async_runtime::spawn(async move {
                    rebuild_tray_menu(&app).await;
                });
            }
            _ => {}
        })
        .build(app)?;

    let _ = TRAY_ICON.set(tray);

    let app_handle = app.handle().clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        rebuild_tray_menu(&app_handle).await;

        let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));
        loop {
            interval.tick().await;
            rebuild_tray_menu(&app_handle).await;
        }
    });

    Ok(())
}

pub fn show_main_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
        log::info!("Main window shown via tray");
    } else {
        log::warn!("Tray: show_main_window called but main window not found");
    }
}

fn toggle_main_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let visible = window.is_visible().unwrap_or(false);
        if visible {
            let _ = window.hide();
            log::info!("Main window hidden via tray toggle");
        } else {
            let _ = window.show();
            let _ = window.unminimize();
            let _ = window.set_focus();
            log::info!("Main window shown via tray toggle");
        }
    }
}
