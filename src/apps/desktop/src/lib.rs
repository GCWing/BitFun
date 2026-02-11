#![allow(non_snake_case)]
//! BitFun Desktop - Tauri-based desktop application with TransportAdapter architecture

pub mod api;
pub mod logging;
pub mod macos_menubar;
pub mod theme;

use bitfun_core::infrastructure::ai::AIClientFactory;
use bitfun_core::infrastructure::{
    get_path_manager_arc,
    get_workspace_path,
    try_get_path_manager_arc,
};
use bitfun_transport::{TauriTransportAdapter, TransportAdapter};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use tauri::Manager;
use tauri_plugin_log::{RotationStrategy, TimezoneStrategy};
#[cfg(target_os = "macos")]
use tauri::Emitter;

// Re-export API
pub use api::*;

use api::ai_rules_api::*;
use api::clipboard_file_api::*;
use api::commands::*;
use api::config_api::*;
use api::conversation_api::*;
use api::diff_api::*;
use api::git_agent_api::*;
use api::git_api::*;
use api::i18n_api::*;
use api::lsp_api::*;
use api::lsp_workspace_api::*;
use api::mcp_api::*;
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

/// Tauri application entry point
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub async fn run() {
    let in_debug = cfg!(debug_assertions) || std::env::var("DEBUG").unwrap_or_default() == "1";
    let log_config = logging::LogConfig::new(in_debug);
    let log_targets = logging::build_log_targets(&log_config);

    eprintln!("=== BitFun Desktop Starting ===");

    bitfun_core::service::config::initialize_global_config()
        .await
        .expect("Failed to initialize global config service");

    AIClientFactory::initialize_global()
        .await
        .expect("Failed to initialize global AIClientFactory");

    let (coordinator, event_queue, event_router, ai_client_factory) = init_agentic_system()
        .await
        .expect("Failed to initialize agentic system");

    init_function_agents(ai_client_factory.clone())
        .await
        .expect("Failed to initialize function agents");

    let app_state = AppState::new_async()
        .await
        .expect("Failed to initialize AppState");

    let coordinator_state = CoordinatorState {
        coordinator: coordinator.clone(),
    };

    let terminal_state = api::terminal_api::TerminalState::new();

    let path_manager = get_path_manager_arc();

    setup_panic_hook();

    tauri::Builder::default()
        .plugin(
            tauri_plugin_log::Builder::new()
                .level(log_config.level)
                .level_for("ignore", log::LevelFilter::Off)
                .level_for("ignore::walk", log::LevelFilter::Off)
                .level_for("globset", log::LevelFilter::Off)
                .level_for("hyper_util", log::LevelFilter::Info)
                .level_for("h2", log::LevelFilter::Info)
                .targets(log_targets)
                .rotation_strategy(RotationStrategy::KeepSome(30))
                .max_file_size(10 * 1024 * 1024)
                .timezone_strategy(TimezoneStrategy::UseLocal)
                .clear_format()
                .build(),
        )
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .on_menu_event(|app, event| {
            #[cfg(target_os = "macos")]
            {
                let event_name = if event.id() == "bitfun.open_project" {
                    Some("bitfun_menu_open_project")
                } else if event.id() == "bitfun.new_project" {
                    Some("bitfun_menu_new_project")
                } else if event.id() == "bitfun.go_home" {
                    Some("bitfun_menu_go_home")
                } else if event.id() == "bitfun.about" {
                    Some("bitfun_menu_about")
                } else {
                    None
                };

                if let Some(event_name) = event_name {
                    // App-wide emit: delivers to frontend listeners even if window is not ready or label changed
                    let _ = app.emit(event_name, ());
                }
            }

            #[cfg(not(target_os = "macos"))]
            {
                let _ = (app, event);
            }
        })
        .manage(app_state)
        .manage(coordinator_state)
        .manage(path_manager)
        .manage(coordinator)
        .manage(terminal_state)
        .setup(move |app| {
            let app_handle = app.handle().clone();
            theme::create_main_window(&app_handle);

            #[cfg(target_os = "macos")]
            {
                let app_handle_for_menu = app.handle().clone();
                let app_state: tauri::State<'_, api::app_state::AppState> = app.state();
                let config_service = app_state.config_service.clone();
                let workspace_path = app_state.workspace_path.clone();

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

                    let _ = crate::macos_menubar::set_macos_menubar_with_mode(
                        &app_handle_for_menu,
                        &language,
                        mode,
                    );
                });
            }

            let transport = Arc::new(TauriTransportAdapter::new(app_handle.clone()));

            start_event_loop_with_transport(event_queue, event_router, transport);

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

            init_services(app_handle.clone());

            logging::spawn_log_cleanup_task();

            log::info!("BitFun Desktop started successfully");
            Ok(())
        })
        .on_window_event({
            static CLEANUP_DONE: AtomicBool = AtomicBool::new(false);

            move |window, event| {
                if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                    if window.label() == "main" {
                        if CLEANUP_DONE
                            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
                            .is_ok()
                        {
                            log::info!("Main window close requested, cleaning up");
                            bitfun_core::util::process_manager::cleanup_all_processes();

                            window.app_handle().exit(0);
                        } else {
                            api.prevent_close();
                        }
                    }
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            theme::show_main_window,
            api::agentic_api::create_session,
            api::agentic_api::start_dialog_turn,
            api::agentic_api::cancel_dialog_turn,
            api::agentic_api::delete_session,
            api::agentic_api::restore_session,
            api::agentic_api::list_sessions,
            api::agentic_api::get_session_messages,
            api::agentic_api::confirm_tool_execution,
            api::agentic_api::reject_tool_execution,
            api::agentic_api::cancel_tool,
            api::agentic_api::generate_session_title,
            api::agentic_api::get_available_modes,
            api::image_analysis_api::analyze_images,
            api::image_analysis_api::send_enhanced_message,
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
            initialize_ai,
            set_agent_model,
            get_agent_models,
            refresh_model_client,
            fix_mermaid_code,
            get_app_state,
            update_app_status,
            read_file_content,
            write_file_content,
            check_path_exists,
            get_file_metadata,
            rename_file,
            reveal_in_explorer,
            get_file_tree,
            get_directory_children,
            get_directory_children_paginated,
            search_files,
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
            set_config,
            reset_config,
            export_config,
            import_config,
            validate_config,
            reload_config,
            sync_config_to_global,
            get_global_config_health,
            get_mode_configs,
            get_mode_config,
            set_mode_config,
            reset_mode_config,
            get_subagent_configs,
            set_subagent_config,
            list_subagents,
            delete_subagent,
            create_subagent,
            reload_subagents,
            list_agent_tool_names,
            update_subagent_config,
            get_skill_configs,
            set_skill_enabled,
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
            get_session_files,
            get_session_turns,
            get_turn_files,
            get_file_diff,
            get_operation_diff,
            get_operation_summary,
            get_session_stats,
            get_snapshot_system_stats,
            get_snapshot_sessions,
            cleanup_snapshot_data,
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
            get_ai_rules,
            get_ai_rule,
            create_ai_rule,
            update_ai_rule,
            delete_ai_rule,
            get_ai_rules_stats,
            build_ai_rules_system_prompt,
            reload_ai_rules,
            toggle_ai_rule,
            // Conversation API
            get_conversation_sessions,
            load_conversation_history,
            save_dialog_turn,
            save_session_metadata,
            delete_conversation_history,
            touch_conversation_session,
            load_session_metadata,
            // AI Memory API
            api::ai_memory_api::get_all_memories,
            api::ai_memory_api::add_memory,
            api::ai_memory_api::update_memory,
            api::ai_memory_api::delete_memory,
            api::ai_memory_api::toggle_memory,
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
            get_mcp_servers,
            start_mcp_server,
            stop_mcp_server,
            restart_mcp_server,
            get_mcp_server_status,
            load_mcp_json_config,
            save_mcp_json_config,
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
            open_workspace,
            close_workspace,
            get_current_workspace,
            scan_workspace_info,
            api::prompt_template_api::get_prompt_template_config,
            api::prompt_template_api::save_prompt_template_config,
            api::prompt_template_api::export_prompt_templates,
            api::prompt_template_api::import_prompt_templates,
            api::prompt_template_api::reset_prompt_templates,
            api::config_api::sync_tool_configs,
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
            check_command_exists,
            check_commands_exist,
            run_system_command,
            i18n_get_current_language,
            i18n_set_language,
            i18n_get_supported_languages,
            i18n_get_config,
            i18n_set_config,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

async fn init_agentic_system() -> anyhow::Result<(
    Arc<bitfun_core::agentic::coordination::ConversationCoordinator>,
    Arc<bitfun_core::agentic::events::EventQueue>,
    Arc<bitfun_core::agentic::events::EventRouter>,
    Arc<AIClientFactory>,
)> {
    use bitfun_core::agentic::*;

    let ai_client_factory = AIClientFactory::get_global().await?;

    let event_queue = Arc::new(events::EventQueue::new(Default::default()));
    let event_router = Arc::new(events::EventRouter::new());

    let path_manager = try_get_path_manager_arc()?;
    let persistence_manager = Arc::new(persistence::PersistenceManager::new(path_manager.clone())?);

    let history_manager = Arc::new(session::MessageHistoryManager::new(
        persistence_manager.clone(),
        session::HistoryConfig {
            enable_persistence: false,
            ..Default::default()
        },
    ));

    let compression_manager = Arc::new(session::CompressionManager::new(
        persistence_manager.clone(),
        session::CompressionConfig {
            enable_persistence: false,
            ..Default::default()
        },
    ));

    let session_manager = Arc::new(session::SessionManager::new(
        history_manager,
        compression_manager,
        persistence_manager,
        Default::default(),
    ));

    let tool_registry = tools::registry::get_global_tool_registry();
    let tool_state_manager = Arc::new(tools::pipeline::ToolStateManager::new(event_queue.clone()));
    let image_context_provider = Arc::new(api::context_upload_api::create_image_context_provider());

    let tool_pipeline = Arc::new(tools::pipeline::ToolPipeline::new(
        tool_registry,
        tool_state_manager,
        Some(image_context_provider),
    ));

    let stream_processor = Arc::new(execution::StreamProcessor::new(event_queue.clone()));
    let round_executor = Arc::new(execution::RoundExecutor::new(
        stream_processor,
        event_queue.clone(),
        tool_pipeline.clone(),
    ));
    let execution_engine = Arc::new(execution::ExecutionEngine::new(
        round_executor,
        event_queue.clone(),
        session_manager.clone(),
        Default::default(),
    ));

    let coordinator = Arc::new(coordination::ConversationCoordinator::new(
        session_manager,
        execution_engine,
        tool_pipeline,
        event_queue.clone(),
        event_router.clone(),
    ));

    coordination::ConversationCoordinator::set_global(coordinator.clone());

    log::info!("Agentic system initialized");
    Ok((coordinator, event_queue, event_router, ai_client_factory))
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

        if message.contains("WSAStartup") || message.contains("10093") || message.contains("hyper")
        {
            log::error!("Network-related crash detected, possible solutions:");
            log::error!("  1) Restart the application");
            log::error!("  2) Check Windows network service status");
            log::error!("  3) Run as administrator");
        }

        std::process::exit(1);
    }));
}

fn start_event_loop_with_transport(
    event_queue: Arc<bitfun_core::agentic::events::EventQueue>,
    event_router: Arc<bitfun_core::agentic::events::EventRouter>,
    transport: Arc<TauriTransportAdapter>,
) {
    tokio::spawn(async move {
        loop {
            event_queue.wait_for_events().await;
            let batch = event_queue.dequeue_batch(10).await;

            if !batch.is_empty() {
                for envelope in batch {
                    let router = event_router.clone();
                    let env_clone = envelope.clone();
                    tokio::spawn(async move {
                        if let Err(e) = router.route(env_clone).await {
                            log::warn!("Internal event routing failed: {:?}", e);
                        }
                    });

                    if let Err(e) = transport.emit_event("", envelope.event).await {
                        log::error!("Failed to emit event: {:?}", e);
                    }
                }
            }
        }
    });
}

fn init_services(app_handle: tauri::AppHandle) {
    use bitfun_core::{infrastructure, service};

    spawn_ingest_server_with_config_listener();

    tokio::spawn(async move {
        let transport = Arc::new(TauriTransportAdapter::new(app_handle.clone()));
        let emitter = create_event_emitter(transport);

        service::snapshot::initialize_snapshot_event_emitter(emitter.clone());

        infrastructure::initialize_file_watcher(emitter.clone());

        if let Err(e) = service::lsp::initialize_global_lsp_manager().await {
            log::error!("Failed to initialize LSP manager: {}", e);
        }

        let event_system = infrastructure::events::get_global_event_system();
        event_system.set_emitter(emitter).await;
    });
}

fn create_event_emitter(
    transport: Arc<TauriTransportAdapter>,
) -> Arc<dyn bitfun_core::infrastructure::events::EventEmitter> {
    use bitfun_core::infrastructure::events::TransportEmitter;
    Arc::new(TransportEmitter::new(transport))
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
                let workspace_path = get_workspace_path()
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
                        let workspace_path = get_workspace_path()
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
                                let workspace_path = get_workspace_path()
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
