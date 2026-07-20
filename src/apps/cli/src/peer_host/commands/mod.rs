//! Product HostInvoke command handlers for CLI Peer Host.

mod config;
mod dialog;
mod filesystem;
mod git;
mod session;
mod snapshot;
mod soft;
mod system;
mod workspace;

use serde_json::Value;

use super::state::PeerHostState;

pub(crate) async fn dispatch(
    command: &str,
    args: &Value,
    state: &PeerHostState,
) -> Result<Value, String> {
    match command {
        // Workspace / config
        "initialize_workspace_startup_state" => {
            workspace::initialize_workspace_startup_state(state).await
        }
        "get_opened_workspaces" => workspace::get_opened_workspaces(state).await,
        "get_recent_workspaces" => workspace::get_recent_workspaces(state).await,
        "get_current_workspace" | "get_workspace_info" => {
            workspace::get_current_workspace(state).await
        }
        "open_workspace" => workspace::open_workspace(state, args).await,
        "cleanup_invalid_workspaces" => workspace::cleanup_invalid_workspaces(state).await,
        "reload_config" => workspace::reload_config().await,
        "get_config" => config::get_config(args).await,
        "get_configs" => config::get_configs(args).await,
        "set_config" => config::set_config(args).await,
        "get_agent_profile_config" => config::get_agent_profile_config(args).await,
        "get_agent_profile_configs" => config::get_agent_profile_configs().await,

        // Filesystem
        "get_directory_children" | "list_files" => {
            filesystem::get_directory_children(state, args).await
        }
        "get_directory_children_paginated" => {
            filesystem::get_directory_children_paginated(state, args).await
        }
        "check_path_exists" => filesystem::check_path_exists(args).await,
        "create_directory" => filesystem::create_directory(state, args).await,

        // Sessions
        "list_persisted_sessions" => session::list_persisted_sessions(state, args).await,
        "list_persisted_sessions_page" => session::list_persisted_sessions_page(state, args).await,
        "list_persisted_sessions_count" => {
            session::list_persisted_sessions_count(state, args).await
        }
        "load_session_turns" => session::load_session_turns(state, args).await,
        "restore_session_view" => session::restore_session_view(state, args).await,
        "restore_session_with_turns" => session::restore_session_with_turns(state, args).await,
        "restore_session" => session::restore_session(state, args).await,
        "create_session" => session::create_session(state, args).await,
        "delete_session" => session::delete_session(state, args).await,
        "rename_session" => session::rename_session(state, args).await,
        "archive_session" => session::archive_session(state, args).await,
        "touch_session_activity" => session::touch_session_activity(state, args).await,
        "get_session_thread_goal" => session::get_session_thread_goal(state, args).await,
        "update_session_model" => session::update_session_model(state, args).await,
        "ensure_coordinator_session" => session::ensure_coordinator_session(state, args).await,
        "get_available_modes" => session::get_available_modes().await,
        "get_session_stats" => session::get_session_stats(state, args).await,
        "save_session_turn" => session::save_session_turn(state, args).await,

        // Snapshot / rollback
        "rollback_to_turn" => snapshot::rollback_to_turn(state, args).await,
        "get_session_files" => snapshot::get_session_files(state, args).await,

        // Dialog / tools
        "start_dialog_turn" => dialog::start_dialog_turn(state, args).await,
        "cancel_dialog_turn" => dialog::cancel_dialog_turn(state, args).await,
        "confirm_tool_execution" => dialog::confirm_tool_execution(state, args).await,
        "reject_tool_execution" => dialog::reject_tool_execution(state, args).await,

        // Git (local workspace only)
        "git_is_repository" => git::git_is_repository(args).await,

        // Soft empty / no-op for Desktop-only subsystems
        "notify_cron_host_ready" => soft::notify_cron_host_ready().await,
        "list_miniapps" => soft::list_miniapps().await,
        "miniapp_worker_list_running" => soft::miniapp_worker_list_running().await,
        "get_acp_clients" => soft::get_acp_clients().await,
        "list_background_command_activities" => soft::list_background_command_activities().await,

        // System
        "get_system_info" => system::get_system_info().await,

        other => Err(format!(
            "command '{other}' is not supported on CLI peer host"
        )),
    }
}
