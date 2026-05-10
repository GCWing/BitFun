#![allow(non_snake_case)]
//! BitFun Desktop - Tauri-based desktop application with TransportAdapter architecture

pub mod api;
pub mod computer_use;
pub mod logging;
pub mod macos_menubar;
pub mod theme;
pub mod tray;

use bitfun_core::agentic::tools::computer_use_capability::set_computer_use_desktop_available;
use bitfun_core::agentic::tools::computer_use_host::ComputerUseHostRef;
use bitfun_core::infrastructure::ai::AIClientFactory;
use bitfun_core::infrastructure::{get_path_manager_arc, try_get_path_manager_arc};
use bitfun_core::service::search::get_global_workspace_search_service;
use bitfun_core::service::workspace::get_global_workspace_service;
use bitfun_core::util::{elapsed_ms, TimingCollector};
use bitfun_transport::{TauriTransportAdapter, TransportAdapter};
use serde::Deserialize;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::Instant;
use tauri::Emitter;
use tauri::Manager;

// Re-export API
pub use api::*;

use api::acp_client_api::*;
use api::clipboard_file_api::*;
use api::commands::*;
use api::computer_use_api::*;
use api::config_api::*;
use api::cron_api::*;
use api::diff_api::*;
use api::git_agent_api::*;
use api::git_api::*;
use api::i18n_api::*;
use api::lsp_api::*;
use api::lsp_workspace_api::*;
use api::mcp_api::*;
use api::runtime_api::*;
use api::search_api::*;
use api::session_api::*;
use api::skill_api::*;
use api::snapshot_service::*;
use api::startchat_agent_api::*;
use api::storage_commands::*;
use api::subagent_api::*;
use api::system_api::*;
use api::tool_api::*;

/// Agentic Coordinator state
#[derive(Clone)]
pub struct CoordinatorState {
    pub coordinator: Arc<bitfun_core::agentic::coordination::ConversationCoordinator>,
}

/// Dialog scheduler state (primary entry point for user messages)
#[derive(Clone)]
pub struct SchedulerState {
    pub scheduler: Arc<bitfun_core::agentic::coordination::DialogScheduler>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WebdriverBridgeResultRequest {
    payload: serde_json::Value,
}

#[cfg(target_os = "macos")]
static MAIN_WINDOW_HIDDEN_ON_MACOS: AtomicBool = AtomicBool::new(false);

#[cfg(target_os = "macos")]
static MAIN_WINDOW_CLOSE_PENDING_ON_MACOS: AtomicBool = AtomicBool::new(false);

const MAIN_WINDOW_CLOSE_REQUESTED_EVENT: &str = "bitfun_main_window_close_requested";

#[cfg(target_os = "macos")]
const MAIN_WINDOW_CLOSE_FALLBACK_HIDE_MS: u64 = 2_500;

// ─── Close-button behavior ────────────────────────────────────────────────────
// The close-button behavior is owned by the frontend; the Rust window-event
// handler only emits a notification event and the frontend decides what to do.
// No per-platform caching needed here.

#[cfg(target_os = "macos")]
pub(crate) fn mark_main_window_hidden_on_macos(hidden: bool) {
    MAIN_WINDOW_HIDDEN_ON_MACOS.store(hidden, Ordering::SeqCst);
}

#[cfg(target_os = "macos")]
pub(crate) fn cancel_main_window_close_request_on_macos() {
    MAIN_WINDOW_CLOSE_PENDING_ON_MACOS.store(false, Ordering::SeqCst);
}

#[cfg(target_os = "macos")]
fn begin_main_window_close_request_on_macos() -> bool {
    MAIN_WINDOW_CLOSE_PENDING_ON_MACOS
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_ok()
}

#[cfg(target_os = "macos")]
fn take_main_window_close_request_on_macos() -> bool {
    MAIN_WINDOW_CLOSE_PENDING_ON_MACOS
        .compare_exchange(true, false, Ordering::SeqCst, Ordering::SeqCst)
        .is_ok()
}

#[cfg(target_os = "macos")]
fn hide_main_window_on_macos(app: &tauri::AppHandle, reason: &str) -> Result<(), String> {
    let Some(main_window) = app.get_webview_window("main") else {
        mark_main_window_hidden_on_macos(false);
        return Err("Main window not found".to_string());
    };

    main_window.hide().map_err(|error| {
        mark_main_window_hidden_on_macos(false);
        log::warn!(
            "Failed to hide main window on macOS close request: reason={}, error={}",
            reason,
            error
        );
        format!("Failed to hide main window: {}", error)
    })?;

    mark_main_window_hidden_on_macos(true);
    log::info!(
        "Main window close requested on macOS; hid window instead of exiting: reason={}",
        reason
    );
    Ok(())
}

#[cfg(target_os = "macos")]
fn show_main_window_on_macos(app: &tauri::AppHandle, reason: &str) {
    cancel_main_window_close_request_on_macos();

    let Some(main_window) = app.get_webview_window("main") else {
        log::warn!(
            "Failed to show main window on macOS reopen event: reason={}, error=main window not found",
            reason
        );
        return;
    };

    let _ = main_window.unminimize();
    if let Err(error) = main_window.show() {
        mark_main_window_hidden_on_macos(false);
        log::warn!(
            "Failed to show main window on macOS reopen event: reason={}, error={}",
            reason,
            error
        );
        return;
    }

    mark_main_window_hidden_on_macos(false);
    if let Err(error) = main_window.set_focus() {
        log::warn!(
            "Failed to focus main window on macOS reopen event: reason={}, error={}",
            reason,
            error
        );
    }
}

#[tauri::command]
async fn hide_main_window_after_close_request(app: tauri::AppHandle) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        if take_main_window_close_request_on_macos() {
            hide_main_window_on_macos(&app, "frontend_ack")?;
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = app;
    }

    Ok(())
}

#[tauri::command]
async fn webdriver_bridge_result(request: WebdriverBridgeResultRequest) -> Result<(), String> {
    log::debug!("webdriver_bridge_result command invoked");
    bitfun_webdriver::handle_bridge_result(request.payload)
}

/// Tauri application entry point
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub async fn run() {
    let startup_started = Instant::now();
    let mut startup_timings = TimingCollector::default();
    let in_debug = cfg!(debug_assertions) || std::env::var("DEBUG").unwrap_or_default() == "1";
    let log_config = logging::LogConfig::new(in_debug);
    let log_targets = logging::build_log_targets(&log_config);
    let session_log_dir = log_config.session_log_dir.clone();

    eprintln!("=== BitFun Desktop Starting ===");

    let step_started = Instant::now();
    if let Err(e) = bitfun_core::service::config::initialize_global_config().await {
        log::error!("Failed to initialize global config service: {}", e);
        return;
    }
    startup_timings.record_elapsed("initialize_global_config", step_started);

    // Initialize global I18nService so bot/remote-connect language is always in sync.
    {
        use bitfun_core::service::config::get_global_config_service;
        use bitfun_core::service::i18n::initialize_global_i18n_service;
        let step_started = Instant::now();
        match get_global_config_service().await {
            Ok(config_service) => {
                if let Err(e) = initialize_global_i18n_service(Some(config_service)).await {
                    log::error!("Failed to initialize global I18nService: {}", e);
                }
            }
            Err(e) => {
                log::error!("Failed to get config service for I18nService init: {}", e);
            }
        }
        startup_timings.record_elapsed("initialize_global_i18n_service", step_started);
    }

    let startup_log_level = resolve_runtime_log_level(log_config.level).await;

    let step_started = Instant::now();
    if let Err(e) = AIClientFactory::initialize_global().await {
        log::error!("Failed to initialize global AIClientFactory: {}", e);
        return;
    }
    startup_timings.record_elapsed("initialize_global_ai_client_factory", step_started);

    let step_started = Instant::now();
    let (coordinator, scheduler, event_queue, event_router, ai_client_factory, token_usage_service) =
        match init_agentic_system().await {
            Ok(state) => state,
            Err(e) => {
                log::error!("Failed to initialize agentic system: {}", e);
                return;
            }
        };
    startup_timings.record_elapsed("init_agentic_system", step_started);

    let step_started = Instant::now();
    if let Err(e) = init_function_agents(ai_client_factory.clone()).await {
        log::error!("Failed to initialize function agents: {}", e);
        return;
    }
    startup_timings.record_elapsed("init_function_agents", step_started);

    let workspace_search_enabled =
        bitfun_core::service::search::workspace_search_feature_enabled().await;
    let startup_flashgrep_path = configure_workspace_search_daemon_env();

    let step_started = Instant::now();
    let app_state = match AppState::new_async(token_usage_service).await {
        Ok(state) => state,
        Err(e) => {
            log::error!("Failed to initialize AppState: {}", e);
            return;
        }
    };
    startup_timings.record_elapsed("initialize_app_state", step_started);

    let coordinator_state = CoordinatorState {
        coordinator: coordinator.clone(),
    };

    let scheduler_state = SchedulerState {
        scheduler: scheduler.clone(),
    };

    let terminal_state = api::terminal_api::TerminalState::new();

    let path_manager = get_path_manager_arc();

    setup_panic_hook();

    let app = tauri::Builder::default()
        .plugin(logging::build_log_plugin(log_targets))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(
            tauri_plugin_autostart::Builder::new()
                .app_name("BitFun")
                .build(),
        )
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(app_state)
        .manage(coordinator_state)
        .manage(scheduler_state)
        .manage(path_manager)
        .manage(coordinator)
        .manage(scheduler)
        .manage(terminal_state)
        .setup(move |app| {
            let setup_started = Instant::now();
            #[cfg(target_os = "macos")]
            {
                app.on_menu_event(|app, event| {
                    let event_name =
                        crate::macos_menubar::menu_event_name_for_id(event.id().as_ref());

                    if let Some(event_name) = event_name {
                        let _ = app.emit(event_name, ());
                    }
                });
            }

            logging::register_runtime_log_state(startup_log_level, session_log_dir.clone());
            for step in startup_timings.steps() {
                log::debug!(
                    "Desktop startup step completed: step={}, duration_ms={}",
                    step.name,
                    step.duration_ms
                );
            }

            if workspace_search_enabled {
                let flashgrep_path = startup_flashgrep_path.clone().or_else(|| {
                    let binary_names =
                        bitfun_core::service::search::workspace_search_daemon_binary_names();
                    for binary_name in binary_names {
                        let primary = format!("flashgrep/{}", binary_name);
                        if let Ok(path) = app
                            .path()
                            .resolve(&primary, tauri::path::BaseDirectory::Resource)
                        {
                            if path.exists() {
                                return Some(path);
                            }
                        }
                    }

                    if let Ok(resource_dir) = app.path().resource_dir() {
                        for binary_name in binary_names {
                            for candidate in [
                                resource_dir.join("flashgrep").join(binary_name),
                                resource_dir.join("resources").join("flashgrep").join(binary_name),
                                resource_dir.join(binary_name),
                            ] {
                                if candidate.exists() {
                                    return Some(candidate);
                                }
                            }
                        }
                    }

                    None
                });
                if let Some(path) = flashgrep_path {
                    std::env::set_var("FLASHGREP_DAEMON_BIN", &path);
                    log::info!(
                        "Workspace search daemon startup check passed: path={}",
                        path.display()
                    );
                } else {
                    log::warn!(
                        "Workspace search daemon startup check failed: {}",
                        bitfun_core::service::search::workspace_search_daemon_missing_hint()
                    );
                }
            }

            // Register bundled mobile-web resource path for remote connect.
            // tauri.conf.json maps "../../mobile-web/dist" -> "mobile-web/dist",
            // so the primary candidate is "mobile-web/dist". Additional fallbacks
            // handle legacy or non-standard bundle layouts.
            {
                let candidates = ["mobile-web/dist", "mobile-web", "dist"];
                let mut found = false;
                for candidate in &candidates {
                    if let Ok(p) = app
                        .path()
                        .resolve(candidate, tauri::path::BaseDirectory::Resource)
                    {
                        if p.join("index.html").exists() {
                            log::info!("Found bundled mobile-web at: {}", p.display());
                            api::remote_connect_api::set_mobile_web_resource_path(p);
                            found = true;
                            break;
                        }
                    }
                }
                if !found {
                    // Last resort: scan the resource root for any index.html
                    if let Ok(res_dir) = app.path().resource_dir() {
                        for sub in &["mobile-web/dist", "mobile-web", "dist", ""] {
                            let p = if sub.is_empty() {
                                res_dir.clone()
                            } else {
                                res_dir.join(sub)
                            };
                            if p.join("index.html").exists() {
                                log::info!(
                                    "Found mobile-web via resource root scan: {}",
                                    p.display()
                                );
                                api::remote_connect_api::set_mobile_web_resource_path(p);
                                break;
                            }
                        }
                    }
                }
            }

            let app_handle = app.handle().clone();
            let window_started = Instant::now();
            theme::create_main_window(&app_handle);
            log::debug!(
                "Desktop startup step completed: step=create_main_window, duration_ms={}",
                elapsed_ms(window_started)
            );
            bitfun_webdriver::maybe_start(app_handle.clone());
            log::debug!(
                "Desktop startup timing: phase=tauri_setup, duration_ms={}, since_process_start_ms={}",
                elapsed_ms(setup_started),
                elapsed_ms(startup_started)
            );

            #[cfg(target_os = "macos")]
            {
                let app_handle_for_menu = app.handle().clone();
                let app_state: tauri::State<'_, api::app_state::AppState> = app.state();
                let config_service = app_state.config_service.clone();
                let workspace_path = app_state.workspace_path.clone();
                let macos_edit_menu_mode = app_state.macos_edit_menu_mode.clone();

                tokio::spawn(async move {
                    let language = config_service
                        .get_config::<String>(Some("app.language"))
                        .await
                        .unwrap_or_else(|_| "zh-CN".to_string());

                    let has_workspace = workspace_path.read().await.is_some();
                    let mode = if has_workspace {
                        crate::macos_menubar::MenubarMode::Workspace
                    } else {
                        crate::macos_menubar::MenubarMode::Startup
                    };
                    let edit_mode = *macos_edit_menu_mode.read().await;

                    let _ = crate::macos_menubar::set_macos_menubar_with_mode(
                        &app_handle_for_menu,
                        &language,
                        mode,
                        edit_mode,
                    );
                });
            }

            let transport = Arc::new(TauriTransportAdapter::new(app_handle.clone()));

            start_event_loop_with_transport(event_queue, event_router, transport);

            // Eagerly initialize the remote connect service so previously
            // paired bots start listening immediately on app startup.
            api::remote_connect_api::init_on_startup();

            {
                let _terminal_state: tauri::State<'_, api::terminal_api::TerminalState> =
                    app.state();
                let terminal_state_inner = api::terminal_api::TerminalState::new();
                let app_handle_clone = app_handle.clone();
                tokio::spawn(async move {
                    api::terminal_api::start_terminal_event_loop(
                        terminal_state_inner,
                        app_handle_clone,
                    );
                });
            }

            init_mcp_servers(app_handle.clone());
            init_acp_clients(app_handle.clone());

            init_services(app_handle.clone(), startup_log_level);

            logging::spawn_log_cleanup_task();

            // Set up system tray icon.
            if let Err(error) = crate::tray::setup_tray(app) {
                log::warn!("Failed to set up system tray: {}", error);
            }

            log::info!("BitFun Desktop started successfully");
            Ok(())
        })
        .on_window_event({
            move |window, event| {
                if let tauri::WindowEvent::CloseRequested { api: _api, .. } = event {
                    if window.label() == "main" {
                        #[cfg(target_os = "macos")]
                        {
                            _api.prevent_close();
                            if !begin_main_window_close_request_on_macos() {
                                return;
                            }

                            if let Err(error) = window.emit(MAIN_WINDOW_CLOSE_REQUESTED_EVENT, ()) {
                                log::warn!(
                                    "Failed to emit macOS main window close request event: {}",
                                    error
                                );
                            }

                            let app_handle = window.app_handle().clone();
                            tauri::async_runtime::spawn(async move {
                                tokio::time::sleep(std::time::Duration::from_millis(
                                    MAIN_WINDOW_CLOSE_FALLBACK_HIDE_MS,
                                ))
                                .await;

                                if take_main_window_close_request_on_macos() {
                                    if let Err(error) =
                                        hide_main_window_on_macos(&app_handle, "frontend_timeout")
                                    {
                                        log::warn!(
                                            "macOS close fallback hide failed after frontend timeout: {}",
                                            error
                                        );
                                    }
                                }
                            });
                        }
                    }
                }

                #[cfg(not(target_os = "macos"))]
                if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                    if window.label() == "main" {
                        // Prevent the OS from closing the window; let the frontend
                        // decide whether to minimize to tray, show a dialog, or quit.
                        api.prevent_close();
                        if let Err(error) = window.emit(MAIN_WINDOW_CLOSE_REQUESTED_EVENT, ()) {
                            log::warn!(
                                "Failed to emit main window close request event: {}",
                                error
                            );
                        }
                    }
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            theme::show_main_window,
            hide_main_window_after_close_request,
            api::agentic_api::create_session,
            api::agentic_api::update_session_model,
            api::agentic_api::update_session_title,
            api::agentic_api::ensure_coordinator_session,
            api::agentic_api::start_dialog_turn,
            api::agentic_api::compact_session,
            api::agentic_api::ensure_assistant_bootstrap,
            api::agentic_api::cancel_dialog_turn,
            api::agentic_api::steer_dialog_turn,
            api::agentic_api::control_deep_review_queue,
            api::agentic_api::cancel_session,
            api::agentic_api::set_subagent_timeout,
            api::agentic_api::delete_session,
            api::agentic_api::restore_session,
            webdriver_bridge_result,
            api::agentic_api::list_sessions,
            api::agentic_api::confirm_tool_execution,
            api::agentic_api::reject_tool_execution,
            api::agentic_api::cancel_tool,
            api::agentic_api::generate_session_title,
            api::agentic_api::get_available_modes,
            api::agentic_api::get_default_review_team_definition,
            api::btw_api::btw_ask_stream,
            api::btw_api::btw_cancel,
            api::editor_ai_api::editor_ai_stream,
            api::editor_ai_api::editor_ai_cancel,
            api::context_upload_api::upload_image_contexts,
            get_all_tools_info,
            get_readonly_tools_info,
            get_tool_info,
            validate_tool_input,
            execute_tool,
            is_tool_enabled,
            submit_user_answers,
            initialize_global_state,
            get_available_tools,
            report_ide_control_result,
            get_health_status,
            get_statistics,
            test_ai_connection,
            test_ai_config_connection,
            list_ai_models_by_config,
            discover_cli_credentials,
            refresh_cli_credential,
            initialize_ai,
            set_agent_model,
            get_agent_models,
            refresh_model_client,
            get_app_state,
            update_app_status,
            theme::show_agent_companion_desktop_pet,
            theme::hide_agent_companion_desktop_pet,
            theme::resize_agent_companion_desktop_pet,
            list_agent_companion_pets,
            import_agent_companion_pet_package,
            delete_agent_companion_pet_package,
            read_file_content,
            write_file_content,
            reset_workspace_persona_files,
            check_path_exists,
            get_file_metadata,
            get_file_editor_sync_hash,
            rename_file,
            export_local_file_to_path,
            reveal_in_explorer,
            get_file_tree,
            explorer_get_file_tree,
            get_directory_children,
            explorer_get_children,
            get_directory_children_paginated,
            explorer_get_children_paginated,
            search_files,
            search_filenames,
            search_file_contents,
            search_get_repo_status,
            search_build_index,
            search_rebuild_index,
            start_search_filenames_stream,
            start_search_file_contents_stream,
            cancel_search,
            delete_file,
            delete_directory,
            create_file,
            create_directory,
            list_directory_files,
            start_file_watch,
            stop_file_watch,
            get_watched_paths,
            get_clipboard_files,
            paste_files,
            get_config,
            computer_use_get_status,
            computer_use_request_permissions,
            computer_use_open_system_settings,
            set_config,
            reset_config,
            export_config,
            import_config,
            validate_config,
            reload_config,
            sync_config_to_global,
            get_global_config_health,
            get_runtime_logging_info,
            get_runtime_capabilities,
            get_mode_configs,
            get_mode_config,
            set_mode_config,
            reset_mode_config,
            get_subagent_configs,
            set_subagent_config,
            list_subagents,
            get_subagent_detail,
            delete_subagent,
            create_subagent,
            update_subagent,
            reload_subagents,
            list_agent_tool_names,
            update_subagent_config,
            get_skill_configs,
            get_mode_skill_configs,
            list_skill_market,
            search_skill_market,
            download_skill_market,
            set_mode_skill_disabled,
            replace_mode_skill_selection,
            reset_mode_skill_selection,
            validate_skill_path,
            add_skill,
            delete_skill,
            git_is_repository,
            git_get_repository,
            git_get_status,
            git_get_branches,
            git_get_enhanced_branches,
            git_get_commits,
            git_add_files,
            git_commit,
            git_push,
            git_pull,
            git_checkout_branch,
            git_create_branch,
            git_delete_branch,
            git_get_diff,
            git_get_changed_files,
            git_reset_files,
            git_reset_to_commit,
            git_get_file_content,
            git_get_graph,
            git_cherry_pick,
            git_cherry_pick_abort,
            git_cherry_pick_continue,
            git_list_worktrees,
            git_add_worktree,
            git_remove_worktree,
            generate_commit_message,
            quick_commit_message,
            save_git_repo_history,
            load_git_repo_history,
            preview_commit_message,
            analyze_work_state,
            quick_analyze_work_state,
            generate_greeting_only,
            get_work_state_summary,
            compute_diff,
            apply_patch,
            save_merged_diff_content,
            initialize_snapshot,
            record_file_change,
            rollback_session,
            rollback_to_turn,
            accept_session,
            accept_file,
            reject_file,
            get_session_files,
            get_session_turns,
            get_turn_files,
            get_file_diff,
            get_operation_diff,
            get_session_file_diff_stats,
            get_operation_summary,
            get_session_operations,
            accept_operation,
            reject_operation,
            get_session_stats,
            get_snapshot_system_stats,
            get_snapshot_sessions,
            check_git_isolation,
            get_file_change_history,
            get_all_modified_files,
            get_baseline_snapshot_diff,
            get_storage_paths,
            get_project_storage_paths,
            cleanup_storage,
            cleanup_storage_with_policy,
            get_storage_statistics,
            initialize_project_storage,
            // Session persistence API
            list_persisted_sessions,
            load_session_turns,
            get_session_usage_report,
            save_session_turn,
            save_session_metadata,
            export_session_transcript,
            delete_persisted_session,
            touch_session_activity,
            load_persisted_session_metadata,
            fork_session,
            api::project_context_api::get_document_statuses,
            api::project_context_api::toggle_document_enabled,
            api::project_context_api::create_context_document,
            api::project_context_api::generate_context_document,
            api::project_context_api::cancel_context_document_generation,
            api::project_context_api::get_project_context_config,
            api::project_context_api::save_project_context_config,
            api::project_context_api::create_project_category,
            api::project_context_api::delete_project_category,
            api::project_context_api::get_all_categories,
            api::project_context_api::import_project_document,
            api::project_context_api::delete_imported_document,
            api::project_context_api::toggle_imported_document_enabled,
            api::project_context_api::delete_context_document,
            initialize_mcp_servers,
            api::mcp_api::initialize_mcp_servers_non_destructive,
            get_mcp_servers,
            api::mcp_api::list_mcp_resources,
            api::mcp_api::read_mcp_resource,
            api::mcp_api::list_mcp_prompts,
            api::mcp_api::get_mcp_prompt,
            start_mcp_server,
            stop_mcp_server,
            restart_mcp_server,
            get_mcp_server_status,
            load_mcp_json_config,
            save_mcp_json_config,
            get_mcp_tool_ui_uri,
            fetch_mcp_app_resource,
            send_mcp_app_message,
            submit_mcp_interaction_response,
            update_mcp_remote_auth,
            clear_mcp_remote_auth,
            api::mcp_api::delete_mcp_server,
            api::mcp_api::start_mcp_remote_oauth,
            api::mcp_api::get_mcp_remote_oauth_session,
            api::mcp_api::cancel_mcp_remote_oauth,
            initialize_acp_clients,
            get_acp_clients,
            probe_acp_client_requirements,
            predownload_acp_client_adapter,
            install_acp_client_cli,
            stop_acp_client,
            load_acp_json_config,
            save_acp_json_config,
            submit_acp_permission_response,
            create_acp_flow_session,
            start_acp_dialog_turn,
            cancel_acp_dialog_turn,
            get_acp_session_options,
            set_acp_session_model,
            lsp_initialize,
            lsp_start_server_for_file,
            lsp_stop_server,
            lsp_stop_all_servers,
            lsp_did_open,
            lsp_did_change,
            lsp_did_save,
            lsp_did_close,
            lsp_get_completions,
            lsp_get_hover,
            lsp_goto_definition,
            lsp_find_references,
            lsp_format_document,
            lsp_install_plugin,
            lsp_uninstall_plugin,
            lsp_list_plugins,
            lsp_get_plugin,
            lsp_get_server_capabilities,
            lsp_get_supported_extensions,
            lsp_open_workspace,
            lsp_close_workspace,
            lsp_open_document,
            lsp_change_document,
            lsp_save_document,
            lsp_close_document,
            lsp_get_completions_workspace,
            lsp_get_hover_workspace,
            lsp_goto_definition_workspace,
            lsp_find_references_workspace,
            lsp_get_code_actions_workspace,
            lsp_format_document_workspace,
            lsp_get_inlay_hints_workspace,
            lsp_rename_workspace,
            lsp_get_document_highlight_workspace,
            lsp_get_document_symbols_workspace,
            lsp_get_semantic_tokens_workspace,
            lsp_get_semantic_tokens_range_workspace,
            lsp_get_server_state,
            lsp_get_all_server_states,
            lsp_stop_server_workspace,
            lsp_list_workspaces,
            lsp_detect_project,
            lsp_prestart_server,
            reload_global_config,
            get_global_config_status,
            subscribe_config_updates,
            get_model_configs,
            get_recent_workspaces,
            remove_recent_workspace,
            cleanup_invalid_workspaces,
            get_opened_workspaces,
            open_workspace,
            open_remote_workspace,
            create_assistant_workspace,
            delete_assistant_workspace,
            reset_assistant_workspace,
            close_workspace,
            set_active_workspace,
            reorder_opened_workspaces,
            get_current_workspace,
            scan_workspace_info,
            list_cron_jobs,
            create_cron_job,
            update_cron_job,
            delete_cron_job,
            api::config_api::canonicalize_mode_configs,
            api::terminal_api::terminal_get_shells,
            api::terminal_api::terminal_create,
            api::terminal_api::terminal_get,
            api::terminal_api::terminal_list,
            api::terminal_api::terminal_close,
            api::terminal_api::terminal_write,
            api::terminal_api::terminal_resize,
            api::terminal_api::terminal_signal,
            api::terminal_api::terminal_ack,
            api::terminal_api::terminal_execute,
            api::terminal_api::terminal_send_command,
            api::terminal_api::terminal_has_shell_integration,
            api::terminal_api::terminal_shutdown_all,
            api::terminal_api::terminal_get_history,
            get_system_info,
            get_app_version,
            check_for_updates,
            install_update,
            restart_app,
            send_system_notification,
            api::system_api::quit_app,
            api::system_api::minimize_to_tray,
            check_command_exists,
            check_commands_exist,
            run_system_command,
            set_macos_edit_menu_mode,
            i18n_get_current_language,
            i18n_set_language,
            i18n_get_supported_languages,
            i18n_get_config,
            i18n_set_config,
            // Remote Connect
            api::remote_connect_api::remote_connect_get_device_info,
            api::remote_connect_api::remote_connect_get_lan_ip,
            api::remote_connect_api::remote_connect_get_lan_network_info,
            api::remote_connect_api::remote_connect_get_methods,
            api::remote_connect_api::remote_connect_start,
            api::remote_connect_api::remote_connect_stop,
            api::remote_connect_api::remote_connect_stop_bot,
            api::remote_connect_api::remote_connect_status,
            api::remote_connect_api::remote_connect_get_form_state,
            api::remote_connect_api::remote_connect_set_form_state,
            api::remote_connect_api::remote_connect_configure_custom_server,
            api::remote_connect_api::remote_connect_configure_bot,
            api::remote_connect_api::remote_connect_weixin_qr_start,
            api::remote_connect_api::remote_connect_weixin_qr_poll,
            api::remote_connect_api::remote_connect_get_bot_verbose_mode,
            api::remote_connect_api::remote_connect_set_bot_verbose_mode,
            // MiniApp API
            api::miniapp_api::list_miniapps,
            api::miniapp_api::get_miniapp,
            api::miniapp_api::create_miniapp,
            api::miniapp_api::update_miniapp,
            api::miniapp_api::delete_miniapp,
            api::miniapp_api::get_miniapp_versions,
            api::miniapp_api::rollback_miniapp,
            api::miniapp_api::get_miniapp_storage,
            api::miniapp_api::set_miniapp_storage,
            api::miniapp_api::grant_miniapp_workspace,
            api::miniapp_api::grant_miniapp_path,
            api::miniapp_api::miniapp_runtime_status,
            api::miniapp_api::miniapp_worker_call,
            api::miniapp_api::miniapp_host_call,
            api::miniapp_api::miniapp_worker_stop,
            api::miniapp_api::miniapp_worker_list_running,
            api::miniapp_api::miniapp_install_deps,
            api::miniapp_api::miniapp_recompile,
            api::miniapp_api::miniapp_dialog_message,
            api::miniapp_api::miniapp_import_from_path,
            api::miniapp_api::miniapp_sync_from_fs,
            api::miniapp_api::miniapp_ai_complete,
            api::miniapp_api::miniapp_ai_chat,
            api::miniapp_api::miniapp_ai_cancel,
            api::miniapp_api::miniapp_ai_list_models,
            // Browser API (embedded webview)
            api::browser_api::browser_webview_eval,
            api::browser_api::browser_get_url,
            // Browser Control API (CDP-based user browser control)
            api::browser_control_api::browser_control_list_browsers,
            api::browser_control_api::browser_control_get_status,
            api::browser_control_api::browser_control_launch,
            api::browser_control_api::browser_control_restart_with_cdp,
            api::browser_control_api::browser_control_create_launcher,
            // Insights API
            api::insights_api::generate_insights,
            api::insights_api::get_latest_insights,
            api::insights_api::load_insights_report,
            api::insights_api::has_insights_data,
            api::insights_api::cancel_insights_generation,
            // SSH Remote API
            api::ssh_api::ssh_list_saved_connections,
            api::ssh_api::ssh_save_connection,
            api::ssh_api::ssh_delete_connection,
            api::ssh_api::ssh_has_stored_password,
            api::ssh_api::ssh_connect,
            api::ssh_api::ssh_disconnect,
            api::ssh_api::ssh_disconnect_all,
            api::ssh_api::ssh_is_connected,
            api::ssh_api::ssh_get_server_info,
            api::ssh_api::ssh_get_config,
            api::ssh_api::ssh_list_config_hosts,
            api::ssh_api::remote_read_file,
            api::ssh_api::remote_write_file,
            api::ssh_api::remote_exists,
            api::ssh_api::remote_read_dir,
            api::ssh_api::remote_get_tree,
            api::ssh_api::remote_create_dir,
            api::ssh_api::remote_remove,
            api::ssh_api::remote_rename,
            api::ssh_api::remote_download_to_local_path,
            api::ssh_api::remote_upload_from_local_path,
            api::ssh_api::remote_execute,
            api::ssh_api::remote_open_workspace,
            api::ssh_api::remote_close_workspace,
            api::ssh_api::remote_remove_workspace,
            api::ssh_api::remote_get_workspace_info,
            // Announcement / feature-demo / tips API
            api::announcement_api::get_pending_announcements,
            api::announcement_api::mark_announcement_seen,
            api::announcement_api::dismiss_announcement,
            api::announcement_api::never_show_announcement,
            api::announcement_api::trigger_announcement,
            api::announcement_api::get_announcement_tips,
            // Debug API (no-op stubs in release builds)
            api::debug_api::debug_element_picked,
            api::debug_api::debug_open_devtools,
            api::debug_api::debug_close_devtools,
        ])
        .build(tauri::generate_context!());

    match app {
        Ok(app) => {
            app.run(|_app_handle, event| match event {
                tauri::RunEvent::ExitRequested { .. } | tauri::RunEvent::Exit => {
                    perform_process_exit_cleanup();
                }
                #[cfg(target_os = "macos")]
                tauri::RunEvent::Reopen {
                    has_visible_windows,
                    ..
                } => {
                    let reason = if has_visible_windows {
                        "dock_reopen_with_visible_aux_window"
                    } else {
                        "dock_reopen_no_visible_windows"
                    };
                    show_main_window_on_macos(_app_handle, reason);
                }
                _ => {}
            });
        }
        Err(e) => {
            log::error!("Error while running tauri application: {}", e);
        }
    }
}

async fn init_agentic_system() -> anyhow::Result<(
    Arc<bitfun_core::agentic::coordination::ConversationCoordinator>,
    Arc<bitfun_core::agentic::coordination::DialogScheduler>,
    Arc<bitfun_core::agentic::events::EventQueue>,
    Arc<bitfun_core::agentic::events::EventRouter>,
    Arc<AIClientFactory>,
    Arc<bitfun_core::service::token_usage::TokenUsageService>,
)> {
    use bitfun_core::agentic::*;

    let ai_client_factory = AIClientFactory::get_global().await?;

    let event_queue = Arc::new(events::EventQueue::new(Default::default()));
    let event_router = Arc::new(events::EventRouter::new());

    let path_manager = try_get_path_manager_arc()?;
    let persistence_manager = Arc::new(persistence::PersistenceManager::new(path_manager.clone())?);

    let context_store = Arc::new(session::SessionContextStore::new());
    let context_compressor = Arc::new(session::ContextCompressor::new(Default::default()));

    let session_manager = Arc::new(session::SessionManager::new(
        context_store,
        persistence_manager,
        Default::default(),
    ));

    let tool_registry = tools::registry::get_global_tool_registry();
    let tool_state_manager = Arc::new(tools::pipeline::ToolStateManager::new(event_queue.clone()));

    let computer_use_host: ComputerUseHostRef =
        Arc::new(computer_use::DesktopComputerUseHost::new());
    set_computer_use_desktop_available(true);

    let tool_pipeline = Arc::new(tools::pipeline::ToolPipeline::new(
        tool_registry,
        tool_state_manager,
        Some(computer_use_host),
    ));

    let stream_processor = Arc::new(execution::StreamProcessor::new(event_queue.clone()));
    let round_executor = Arc::new(execution::RoundExecutor::new(
        stream_processor,
        event_queue.clone(),
        tool_pipeline.clone(),
    ));

    // Get execution config from global settings
    let exec_config = match bitfun_core::service::config::get_global_config_service().await {
        Ok(config_service) => {
            match config_service
                .get_config::<bitfun_core::service::config::types::GlobalConfig>(None)
                .await
            {
                Ok(global_config) => execution::ExecutionEngineConfig {
                    max_rounds: global_config.ai.max_rounds,
                    ..Default::default()
                },
                Err(_) => Default::default(),
            }
        }
        Err(_) => Default::default(),
    };

    let execution_engine = Arc::new(execution::ExecutionEngine::new(
        round_executor,
        event_queue.clone(),
        session_manager.clone(),
        context_compressor,
        exec_config,
    ));

    let coordinator = Arc::new(coordination::ConversationCoordinator::new(
        session_manager.clone(),
        execution_engine,
        tool_pipeline,
        event_queue.clone(),
        event_router.clone(),
    ));

    coordination::ConversationCoordinator::set_global(coordinator.clone());

    // Initialize token usage service and register subscriber
    let token_usage_service = Arc::new(
        bitfun_core::service::token_usage::TokenUsageService::new(path_manager.clone())
            .await
            .map_err(|e| anyhow::anyhow!("Failed to initialize token usage service: {}", e))?,
    );
    let token_usage_subscriber = Arc::new(
        bitfun_core::service::token_usage::TokenUsageSubscriber::new(token_usage_service.clone()),
    );
    event_router.subscribe_internal("token_usage".to_string(), token_usage_subscriber);

    log::info!("Token usage service initialized and subscriber registered");

    // Create the DialogScheduler and wire up the outcome notification channel
    let scheduler =
        coordination::DialogScheduler::new(coordinator.clone(), session_manager.clone());
    coordinator.set_scheduler_notifier(scheduler.outcome_sender());
    coordinator.set_round_preempt_source(scheduler.preempt_monitor());
    coordinator.set_round_steering_source(scheduler.steering_monitor());
    coordination::set_global_scheduler(scheduler.clone());

    let cron_service =
        bitfun_core::service::cron::CronService::new(path_manager.clone(), scheduler.clone())
            .await
            .map_err(|e| anyhow::anyhow!("Failed to initialize cron service: {}", e))?;
    bitfun_core::service::cron::set_global_cron_service(cron_service.clone());
    let cron_subscriber = Arc::new(bitfun_core::service::cron::CronEventSubscriber::new(
        cron_service.clone(),
    ));
    event_router.subscribe_internal("cron_jobs".to_string(), cron_subscriber);
    cron_service.start();

    log::info!("Cron service initialized and subscriber registered");
    log::info!("Agentic system initialized");
    Ok((
        coordinator,
        scheduler,
        event_queue,
        event_router,
        ai_client_factory,
        token_usage_service,
    ))
}

async fn init_function_agents(ai_client_factory: Arc<AIClientFactory>) -> anyhow::Result<()> {
    let _ = bitfun_core::function_agents::git_func_agent::GitFunctionAgent::new(
        ai_client_factory.clone(),
    );

    let _ = bitfun_core::function_agents::startchat_func_agent::StartchatFunctionAgent::new(
        ai_client_factory.clone(),
    );

    Ok(())
}

fn init_mcp_servers(app_handle: tauri::AppHandle) {
    tokio::spawn(async move {
        let _ = app_handle;
    });
}

fn init_acp_clients(app_handle: tauri::AppHandle) {
    tokio::spawn(async move {
        let state: tauri::State<'_, api::AppState> = app_handle.state();
        if let Some(service) = state.acp_client_service.as_ref() {
            if let Err(error) = service.initialize_all().await {
                log::warn!("Failed to initialize ACP clients: {}", error);
            }
        }
    });
}

fn setup_panic_hook() {
    std::panic::set_hook(Box::new(move |panic_info| {
        let location = panic_info
            .location()
            .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
            .unwrap_or_else(|| "unknown location".to_string());

        let message = panic_info
            .payload()
            .downcast_ref::<&str>()
            .copied()
            .or_else(|| {
                panic_info
                    .payload()
                    .downcast_ref::<String>()
                    .map(String::as_str)
            })
            .unwrap_or("unknown panic message");

        log::error!("Application panic at {}: {}", location, message);

        // Known wry bug: WKWebView.URL() returns nil after navigating to an
        // invalid address, causing url_from_webview to panic on unwrap().
        // This is non-fatal — the webview is still alive — so we log and
        // continue instead of killing the process.
        // See: https://github.com/tauri-apps/wry/pull/1554
        if location.contains("wry") && location.contains("wkwebview") {
            log::warn!("Suppressed non-fatal wry/wkwebview panic, application continues");
            return;
        }

        if message.contains("WSAStartup") || message.contains("10093") || message.contains("hyper")
        {
            log::error!("Network-related crash detected, possible solutions:");
            log::error!("  1) Restart the application");
            log::error!("  2) Check Windows network service status");
            log::error!("  3) Run as administrator");
        }

        perform_process_exit_cleanup();
        std::process::exit(1);
    }));
}

pub(crate) fn perform_process_exit_cleanup() -> bool {
    static CLEANUP_DONE: AtomicBool = AtomicBool::new(false);

    if CLEANUP_DONE
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return false;
    }

    if let Some(search_service) = get_global_workspace_search_service() {
        search_service.shutdown_blocking();
    }
    bitfun_core::util::process_manager::cleanup_all_processes();
    api::remote_connect_api::cleanup_on_exit();
    true
}

fn configure_workspace_search_daemon_env() -> Option<std::path::PathBuf> {
    let path = bitfun_core::service::search::resolve_workspace_search_daemon_program_path();
    if let Some(path) = path.as_ref() {
        std::env::set_var("FLASHGREP_DAEMON_BIN", path);
    }
    path
}

fn start_event_loop_with_transport(
    event_queue: Arc<bitfun_core::agentic::events::EventQueue>,
    event_router: Arc<bitfun_core::agentic::events::EventRouter>,
    transport: Arc<TauriTransportAdapter>,
) {
    tokio::spawn(async move {
        loop {
            event_queue.wait_for_events().await;
            loop {
                let batch = event_queue.dequeue_configured_batch().await;
                if batch.is_empty() {
                    break;
                }

                for envelope in batch {
                    // Route to internal subscribers (e.g. RemoteSessionStateTracker)
                    // sequentially so that text chunks are appended in order.
                    if let Err(e) = event_router.route(envelope.clone()).await {
                        log::warn!("Internal event routing failed: {:?}", e);
                    }

                    if let Err(e) = transport.emit_event("", envelope.event).await {
                        log::error!("Failed to emit event: {:?}", e);
                    }
                }
            }
        }
    });
}

fn init_services(app_handle: tauri::AppHandle, default_log_level: log::LevelFilter) {
    use bitfun_core::{infrastructure, service};

    spawn_ingest_server_with_config_listener();
    spawn_runtime_log_level_listener(default_log_level);
    spawn_workspace_search_feature_listener(app_handle.clone());

    tokio::spawn(async move {
        let transport = Arc::new(TauriTransportAdapter::new(app_handle.clone()));
        let emitter = create_event_emitter(transport);
        let workspace_identity_watch_service = {
            let app_state: tauri::State<'_, api::app_state::AppState> = app_handle.state();
            app_state.workspace_identity_watch_service.clone()
        };

        service::snapshot::initialize_snapshot_event_emitter(emitter.clone());

        bitfun_core::service::initialize_file_watch_service(emitter.clone());

        if let Err(e) = workspace_identity_watch_service
            .set_event_emitter(emitter.clone())
            .await
        {
            log::error!(
                "Failed to initialize workspace identity watch service: {}",
                e
            );
        }

        if let Err(e) = service::lsp::initialize_global_lsp_manager().await {
            log::error!("Failed to initialize LSP manager: {}", e);
        }

        let event_system = infrastructure::events::get_global_event_system();
        event_system.set_emitter(emitter).await;
    });
}

async fn resolve_runtime_log_level(default_level: log::LevelFilter) -> log::LevelFilter {
    use bitfun_core::service::config::get_global_config_service;

    if let Ok(config_service) = get_global_config_service().await {
        if let Ok(config_level) = config_service
            .get_config::<String>(Some("app.logging.level"))
            .await
        {
            if let Some(level) = logging::parse_log_level(&config_level) {
                return level;
            }
            log::warn!(
                "Invalid app.logging.level '{}', falling back to default={}",
                config_level,
                logging::level_to_str(default_level)
            );
        }
    }

    default_level
}

fn spawn_runtime_log_level_listener(default_level: log::LevelFilter) {
    use bitfun_core::service::config::{subscribe_config_updates, ConfigUpdateEvent};

    tokio::spawn(async move {
        if let Some(mut receiver) = subscribe_config_updates() {
            loop {
                match receiver.recv().await {
                    Ok(ConfigUpdateEvent::LogLevelUpdated { new_level }) => {
                        if let Some(level) = logging::parse_log_level(&new_level) {
                            logging::apply_runtime_log_level(level, "config_update_event");
                        } else {
                            log::warn!(
                                "Received invalid log level from config update event: {}",
                                new_level
                            );
                        }
                    }
                    Ok(ConfigUpdateEvent::ConfigReloaded) => {
                        let level = resolve_runtime_log_level(default_level).await;
                        logging::apply_runtime_log_level(level, "config_reloaded");
                    }
                    Ok(_) => {}
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                        log::warn!("Log-level listener channel closed, stopping listener");
                        break;
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                        log::warn!("Log-level listener lagged by {} messages", n);
                    }
                }
            }
        } else {
            log::warn!("Config update subscription unavailable for log-level listener");
        }
    });
}

fn create_event_emitter(
    transport: Arc<TauriTransportAdapter>,
) -> Arc<dyn bitfun_core::infrastructure::events::EventEmitter> {
    use bitfun_core::infrastructure::events::TransportEmitter;
    Arc::new(TransportEmitter::new(transport))
}

fn spawn_workspace_search_feature_listener(app_handle: tauri::AppHandle) {
    use bitfun_core::service::config::{subscribe_config_updates, ConfigUpdateEvent};

    let app_state: tauri::State<'_, api::AppState> = app_handle.state();
    let workspace_search_service = app_state.workspace_search_service.clone();
    let workspace_path = app_state.workspace_path.clone();

    tokio::spawn(async move {
        let mut feature_enabled =
            bitfun_core::service::search::workspace_search_feature_enabled().await;

        let Some(mut receiver) = subscribe_config_updates() else {
            log::warn!("Config update subscription unavailable for workspace search listener");
            return;
        };

        loop {
            match receiver.recv().await {
                Ok(ConfigUpdateEvent::AppUpdated) | Ok(ConfigUpdateEvent::ConfigReloaded) => {
                    let next_enabled =
                        bitfun_core::service::search::workspace_search_feature_enabled().await;

                    if next_enabled == feature_enabled {
                        continue;
                    }

                    if !next_enabled {
                        workspace_search_service.stop_all_daemons().await;
                        log::info!(
                            "Workspace search feature disabled; stopped flashgrep daemon and cleared sessions"
                        );
                        feature_enabled = false;
                        continue;
                    }

                    let resolved_path = configure_workspace_search_daemon_env();
                    if !bitfun_core::service::search::workspace_search_daemon_available() {
                        log::warn!(
                            "Workspace search feature enabled but daemon is unavailable: path={:?}, hint={}",
                            resolved_path.as_ref().map(|path| path.display().to_string()),
                            bitfun_core::service::search::workspace_search_daemon_missing_hint()
                        );
                        feature_enabled = true;
                        continue;
                    }

                    let current_workspace = workspace_path.read().await.clone();
                    if let Some(current_workspace) = current_workspace {
                        let workspace_str = current_workspace.to_string_lossy().to_string();
                        if !bitfun_core::service::remote_ssh::workspace_state::is_remote_path(
                            workspace_str.trim(),
                        )
                        .await
                        {
                            match workspace_search_service.open_repo(&current_workspace).await {
                                Ok(_) => {
                                    log::info!(
                                        "Workspace search feature enabled; warmed current workspace: path={}",
                                        current_workspace.display()
                                    );
                                }
                                Err(error) => {
                                    log::warn!(
                                        "Workspace search feature enabled but failed to warm current workspace: path={}, error={}",
                                        current_workspace.display(),
                                        error
                                    );
                                }
                            }
                        }
                    }

                    feature_enabled = true;
                }
                Ok(_) => {}
                Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                    log::warn!("Workspace search feature listener channel closed");
                    break;
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                    log::warn!("Workspace search feature listener lagged by {} messages", n);
                }
            }
        }
    });
}

fn spawn_ingest_server_with_config_listener() {
    use bitfun_core::infrastructure::debug_log::IngestServerManager;
    use bitfun_core::service::config::{
        get_global_config_service, subscribe_config_updates, ConfigUpdateEvent,
    };

    tokio::spawn(async move {
        let initial_config = if let Ok(config_service) = get_global_config_service().await {
            if let Ok(config) = config_service
                .get_config::<bitfun_core::service::config::GlobalConfig>(None)
                .await
            {
                let debug_config = &config.ai.debug_mode_config;
                let workspace_path = get_global_workspace_service()
                    .and_then(|service| service.try_get_current_workspace_path())
                    .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());

                Some(bitfun_core::infrastructure::debug_log::IngestServerConfig::from_debug_mode_config(
                    debug_config.ingest_port,
                    workspace_path.join(&debug_config.log_path),
                ))
            } else {
                None
            }
        } else {
            None
        };

        let configured_port = if let Ok(config_service) = get_global_config_service().await {
            if let Ok(config) = config_service
                .get_config::<bitfun_core::service::config::GlobalConfig>(None)
                .await
            {
                Some(config.ai.debug_mode_config.ingest_port)
            } else {
                None
            }
        } else {
            None
        };

        let manager = IngestServerManager::global();
        if let Err(e) = manager.start(initial_config).await {
            log::error!("Failed to start Debug Log Ingest Server: {}", e);
        }

        let actual_port = manager.get_actual_port().await;
        if let Some(cfg_port) = configured_port {
            if actual_port != cfg_port {
                if let Ok(config_service) = get_global_config_service().await {
                    if let Err(e) = config_service
                        .set_config("ai.debug_mode_config.ingest_port", actual_port)
                        .await
                    {
                        log::error!("Failed to sync actual port to config: {}", e);
                    } else {
                        log::info!(
                            "Ingest Server port synced: actual_port={}, config_port={}",
                            actual_port,
                            cfg_port
                        );
                    }
                }
            }
        }

        if let Some(mut receiver) = subscribe_config_updates() {
            loop {
                match receiver.recv().await {
                    Ok(ConfigUpdateEvent::DebugModeConfigUpdated {
                        new_port,
                        new_log_path,
                    }) => {
                        let workspace_path = get_global_workspace_service()
                            .and_then(|service| service.try_get_current_workspace_path())
                            .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
                        let full_log_path = workspace_path.join(&new_log_path);

                        if let Err(e) = manager.update_port(new_port, full_log_path).await {
                            log::error!("Failed to update Ingest Server config: port={}, log_path={}, error={}", new_port, new_log_path, e);
                        }
                    }
                    Ok(ConfigUpdateEvent::ConfigReloaded) => {
                        if let Ok(config_service) = get_global_config_service().await {
                            if let Ok(config) = config_service
                                .get_config::<bitfun_core::service::config::GlobalConfig>(None)
                                .await
                            {
                                let debug_config = &config.ai.debug_mode_config;
                                let workspace_path = get_global_workspace_service()
                                    .and_then(|service| service.try_get_current_workspace_path())
                                    .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
                                let full_log_path = workspace_path.join(&debug_config.log_path);

                                if let Err(e) = manager
                                    .update_port(debug_config.ingest_port, full_log_path)
                                    .await
                                {
                                    log::error!("Failed to update Ingest Server after config reload: port={}, error={}", debug_config.ingest_port, e);
                                }
                            }
                        }
                    }
                    Ok(_) => {}
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                        log::warn!("Config update channel closed, stopping listener");
                        break;
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                        log::warn!("Config update listener lagged by {} messages", n);
                    }
                }
            }
        }
    });
}

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
