//! Remote workspace command policy registry.
//!
//! Every Tauri command registered in `lib.rs` (`tauri::generate_handler!`)
//! must declare how it behaves when the active workspace is a remote SSH
//! workspace (`WorkspaceKind::Remote`). This registry exists because remote
//! SSH workspaces have no central command router: each handler adapts itself
//! (usually through `resolve_desktop_path_target` / `lookup_remote_connection`
//! / `is_remote_path`), so nothing else forces a new command to consider
//! remote workspaces at all. Historically that produced silent local/remote
//! feature gaps (for example the PR reviewer opening to a blank panel in
//! remote workspaces).
//!
//! Rules enforced by the contract tests in this module:
//!
//! - Every registered command has exactly one policy entry, and every entry
//!   matches a registered command.
//! - `LegacyUnaudited` is a frozen backlog: entries may leave it after their
//!   remote behavior has been audited, but no command may enter it. New
//!   commands must ship with an explicit policy.
//!
//! Policy semantics:
//!
//! - `RemoteRouted`: the handler detects remote workspace paths/sessions and
//!   executes on the remote host (or is itself part of the remote SSH
//!   machinery). This is the target state for workspace-facing features.
//! - `RemoteUnsupported`: the handler explicitly rejects remote workspaces
//!   with a clear, user-visible error or gated UI state. Silent fake-success
//!   or empty payloads do not qualify.
//! - `LocalOnly`: the command intentionally operates on the local host
//!   regardless of workspace (windowing, tray, local browser, devtools, OS
//!   automation).
//! - `WorkspaceAgnostic`: behavior does not depend on where the workspace
//!   filesystem lives (accounts, i18n, SSH connection management, Remote
//!   Connect, announcements, app lifecycle metadata).
//! - `LegacyUnaudited`: pre-existing command whose remote behavior has not
//!   been audited yet. Auditing one means reading the handler, fixing or
//!   gating remote behavior if needed, and moving it to a real policy.

/// How a Tauri command behaves for remote SSH workspaces.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RemoteWorkspacePolicy {
    /// Routed to the remote host for remote workspace paths/sessions.
    RemoteRouted,
    /// Explicitly rejected for remote workspaces with a clear error.
    RemoteUnsupported,
    /// Intentionally local-host behavior regardless of workspace.
    LocalOnly,
    /// Independent of workspace filesystem location.
    WorkspaceAgnostic,
    /// Frozen backlog; must not grow. See module docs.
    LegacyUnaudited,
}

/// Declared remote-workspace policy for every registered Tauri command.
pub const REMOTE_WORKSPACE_COMMAND_POLICIES: &[(&str, RemoteWorkspacePolicy)] = &[
    ("accept_file", RemoteWorkspacePolicy::LegacyUnaudited),
    ("accept_operation", RemoteWorkspacePolicy::LegacyUnaudited),
    ("accept_session", RemoteWorkspacePolicy::LegacyUnaudited),
    (
        "account_auto_sync",
        RemoteWorkspacePolicy::WorkspaceAgnostic,
    ),
    (
        "account_connect_devices",
        RemoteWorkspacePolicy::WorkspaceAgnostic,
    ),
    (
        "account_delegate_to_paired",
        RemoteWorkspacePolicy::WorkspaceAgnostic,
    ),
    (
        "account_delete_device",
        RemoteWorkspacePolicy::WorkspaceAgnostic,
    ),
    (
        "account_delete_synced_session",
        RemoteWorkspacePolicy::WorkspaceAgnostic,
    ),
    (
        "account_device_rpc",
        RemoteWorkspacePolicy::WorkspaceAgnostic,
    ),
    (
        "account_execute_on_device",
        RemoteWorkspacePolicy::WorkspaceAgnostic,
    ),
    (
        "account_export_all_sessions",
        RemoteWorkspacePolicy::WorkspaceAgnostic,
    ),
    (
        "account_export_local_session",
        RemoteWorkspacePolicy::WorkspaceAgnostic,
    ),
    (
        "account_fetch_session_turns",
        RemoteWorkspacePolicy::WorkspaceAgnostic,
    ),
    (
        "account_fetch_settings",
        RemoteWorkspacePolicy::WorkspaceAgnostic,
    ),
    (
        "account_fetch_synced_sessions",
        RemoteWorkspacePolicy::WorkspaceAgnostic,
    ),
    (
        "account_get_credential_hint",
        RemoteWorkspacePolicy::WorkspaceAgnostic,
    ),
    (
        "account_import_remote_sessions",
        RemoteWorkspacePolicy::WorkspaceAgnostic,
    ),
    (
        "account_list_devices",
        RemoteWorkspacePolicy::WorkspaceAgnostic,
    ),
    ("account_login", RemoteWorkspacePolicy::WorkspaceAgnostic),
    ("account_logout", RemoteWorkspacePolicy::WorkspaceAgnostic),
    (
        "account_online_devices",
        RemoteWorkspacePolicy::WorkspaceAgnostic,
    ),
    (
        "account_send_session_to_device",
        RemoteWorkspacePolicy::WorkspaceAgnostic,
    ),
    ("account_status", RemoteWorkspacePolicy::WorkspaceAgnostic),
    (
        "account_sync_session",
        RemoteWorkspacePolicy::WorkspaceAgnostic,
    ),
    (
        "account_sync_settings",
        RemoteWorkspacePolicy::WorkspaceAgnostic,
    ),
    (
        "account_token_expired",
        RemoteWorkspacePolicy::WorkspaceAgnostic,
    ),
    (
        "activate_session_goal",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    ("add_skill", RemoteWorkspacePolicy::LegacyUnaudited),
    ("analyze_work_state", RemoteWorkspacePolicy::LegacyUnaudited),
    ("apply_patch", RemoteWorkspacePolicy::LegacyUnaudited),
    (
        "archive_all_sessions",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    ("archive_session", RemoteWorkspacePolicy::LegacyUnaudited),
    (
        "browser_control_create_launcher",
        RemoteWorkspacePolicy::LocalOnly,
    ),
    (
        "browser_control_get_status",
        RemoteWorkspacePolicy::LocalOnly,
    ),
    ("browser_control_launch", RemoteWorkspacePolicy::LocalOnly),
    (
        "browser_control_list_browsers",
        RemoteWorkspacePolicy::LocalOnly,
    ),
    (
        "browser_control_restart_with_cdp",
        RemoteWorkspacePolicy::LocalOnly,
    ),
    ("browser_get_url", RemoteWorkspacePolicy::LocalOnly),
    ("browser_webview_create", RemoteWorkspacePolicy::LocalOnly),
    ("browser_webview_eval", RemoteWorkspacePolicy::LocalOnly),
    ("browser_webview_navigate", RemoteWorkspacePolicy::LocalOnly),
    ("browser_webview_reload", RemoteWorkspacePolicy::LocalOnly),
    (
        "browser_webview_set_bounds",
        RemoteWorkspacePolicy::LocalOnly,
    ),
    ("btw_ask_stream", RemoteWorkspacePolicy::LegacyUnaudited),
    ("btw_cancel", RemoteWorkspacePolicy::LegacyUnaudited),
    (
        "cancel_acp_dialog_turn",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    ("cancel_dialog_turn", RemoteWorkspacePolicy::LegacyUnaudited),
    (
        "cancel_insights_generation",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    (
        "cancel_mcp_remote_oauth",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    ("cancel_search", RemoteWorkspacePolicy::LegacyUnaudited),
    ("cancel_session", RemoteWorkspacePolicy::LegacyUnaudited),
    ("cancel_tool", RemoteWorkspacePolicy::LegacyUnaudited),
    ("cancel_transfer", RemoteWorkspacePolicy::LegacyUnaudited),
    (
        "canonicalize_agent_profile_configs",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    (
        "check_command_exists",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    (
        "check_commands_exist",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    (
        "check_for_updates",
        RemoteWorkspacePolicy::WorkspaceAgnostic,
    ),
    (
        "check_git_isolation",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    ("check_path_exists", RemoteWorkspacePolicy::LegacyUnaudited),
    (
        "choose_external_mcp_conflict_command",
        RemoteWorkspacePolicy::RemoteUnsupported,
    ),
    (
        "choose_external_subagent_conflict_command",
        RemoteWorkspacePolicy::RemoteUnsupported,
    ),
    (
        "cleanup_invalid_workspaces",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    ("cleanup_storage", RemoteWorkspacePolicy::LegacyUnaudited),
    (
        "cleanup_storage_with_policy",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    (
        "clear_mcp_remote_auth",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    (
        "clear_session_thread_goal",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    ("close_workspace", RemoteWorkspacePolicy::LegacyUnaudited),
    ("compact_session", RemoteWorkspacePolicy::LegacyUnaudited),
    ("compress_path", RemoteWorkspacePolicy::LegacyUnaudited),
    ("compute_diff", RemoteWorkspacePolicy::LegacyUnaudited),
    ("computer_use_get_status", RemoteWorkspacePolicy::LocalOnly),
    (
        "computer_use_open_system_settings",
        RemoteWorkspacePolicy::LocalOnly,
    ),
    (
        "computer_use_request_permissions",
        RemoteWorkspacePolicy::LocalOnly,
    ),
    (
        "confirm_tool_execution",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    (
        "control_background_command",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    (
        "control_deep_review_queue",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    (
        "create_acp_flow_session",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    (
        "create_assistant_workspace",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    ("create_cron_job", RemoteWorkspacePolicy::LegacyUnaudited),
    (
        "create_custom_agent",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    ("create_directory", RemoteWorkspacePolicy::LegacyUnaudited),
    ("create_file", RemoteWorkspacePolicy::LegacyUnaudited),
    ("create_miniapp", RemoteWorkspacePolicy::LegacyUnaudited),
    ("create_session", RemoteWorkspacePolicy::LegacyUnaudited),
    ("create_subagent", RemoteWorkspacePolicy::LegacyUnaudited),
    ("debug_close_devtools", RemoteWorkspacePolicy::LocalOnly),
    ("debug_devtools_available", RemoteWorkspacePolicy::LocalOnly),
    ("debug_element_picked", RemoteWorkspacePolicy::LocalOnly),
    ("debug_open_devtools", RemoteWorkspacePolicy::LocalOnly),
    ("decompress_path", RemoteWorkspacePolicy::LegacyUnaudited),
    (
        "delete_agent_companion_pet_package",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    (
        "delete_all_archived_sessions",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    (
        "delete_assistant_workspace",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    ("delete_cron_job", RemoteWorkspacePolicy::LegacyUnaudited),
    (
        "delete_custom_agent",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    ("delete_directory", RemoteWorkspacePolicy::LegacyUnaudited),
    ("delete_file", RemoteWorkspacePolicy::LegacyUnaudited),
    ("delete_mcp_server", RemoteWorkspacePolicy::LegacyUnaudited),
    ("delete_miniapp", RemoteWorkspacePolicy::LegacyUnaudited),
    (
        "delete_persisted_session",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    ("delete_session", RemoteWorkspacePolicy::LegacyUnaudited),
    ("delete_skill", RemoteWorkspacePolicy::LegacyUnaudited),
    ("delete_subagent", RemoteWorkspacePolicy::LegacyUnaudited),
    (
        "discover_cli_credentials",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    (
        "dismiss_announcement",
        RemoteWorkspacePolicy::WorkspaceAgnostic,
    ),
    (
        "download_skill_market",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    ("editor_ai_cancel", RemoteWorkspacePolicy::LegacyUnaudited),
    ("editor_ai_stream", RemoteWorkspacePolicy::LegacyUnaudited),
    (
        "ensure_assistant_bootstrap",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    (
        "ensure_coordinator_session",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    ("execute_tool", RemoteWorkspacePolicy::LegacyUnaudited),
    (
        "explorer_get_children",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    (
        "explorer_get_children_paginated",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    (
        "explorer_get_file_tree",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    ("export_config", RemoteWorkspacePolicy::LegacyUnaudited),
    (
        "export_diagnostics_bundle",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    (
        "export_local_file_to_path",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    (
        "export_session_transcript",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    (
        "fetch_mcp_app_resource",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    ("fork_session", RemoteWorkspacePolicy::LegacyUnaudited),
    (
        "generate_commit_message",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    (
        "generate_greeting_only",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    ("generate_insights", RemoteWorkspacePolicy::LegacyUnaudited),
    (
        "generate_session_title",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    ("get_acp_clients", RemoteWorkspacePolicy::LegacyUnaudited),
    (
        "get_acp_session_commands",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    (
        "get_acp_session_options",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    (
        "get_agent_profile_config",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    (
        "get_agent_profile_configs",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    (
        "get_all_modified_files",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    ("get_all_tools_info", RemoteWorkspacePolicy::LegacyUnaudited),
    (
        "get_announcement_tips",
        RemoteWorkspacePolicy::WorkspaceAgnostic,
    ),
    ("get_app_state", RemoteWorkspacePolicy::WorkspaceAgnostic),
    ("get_app_version", RemoteWorkspacePolicy::WorkspaceAgnostic),
    (
        "get_available_modes",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    (
        "get_available_tools",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    (
        "get_baseline_snapshot_diff",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    (
        "get_clipboard_files",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    ("get_config", RemoteWorkspacePolicy::LegacyUnaudited),
    ("get_configs", RemoteWorkspacePolicy::LegacyUnaudited),
    (
        "get_current_workspace",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    (
        "get_custom_agent_detail",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    (
        "get_default_review_team_definition",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    (
        "get_directory_children",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    (
        "get_directory_children_paginated",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    (
        "get_external_source_snapshot",
        RemoteWorkspacePolicy::RemoteUnsupported,
    ),
    (
        "get_file_change_history",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    ("get_file_diff", RemoteWorkspacePolicy::LegacyUnaudited),
    (
        "get_file_editor_sync_hash",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    ("get_file_metadata", RemoteWorkspacePolicy::LegacyUnaudited),
    ("get_file_tree", RemoteWorkspacePolicy::LegacyUnaudited),
    (
        "get_global_config_health",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    (
        "get_global_config_status",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    (
        "get_health_status",
        RemoteWorkspacePolicy::WorkspaceAgnostic,
    ),
    (
        "get_latest_insights",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    ("get_mcp_prompt", RemoteWorkspacePolicy::LegacyUnaudited),
    (
        "get_mcp_remote_oauth_session",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    (
        "get_mcp_server_status",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    ("get_mcp_servers", RemoteWorkspacePolicy::LegacyUnaudited),
    (
        "get_mcp_tool_ui_uri",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    ("get_memory_paths", RemoteWorkspacePolicy::LegacyUnaudited),
    ("get_miniapp", RemoteWorkspacePolicy::LegacyUnaudited),
    (
        "get_miniapp_draft_storage",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    (
        "get_miniapp_storage",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    (
        "get_miniapp_versions",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    (
        "get_mode_skill_configs",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    ("get_model_configs", RemoteWorkspacePolicy::LegacyUnaudited),
    (
        "get_opened_workspaces",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    ("get_operation_diff", RemoteWorkspacePolicy::LegacyUnaudited),
    (
        "get_operation_summary",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    (
        "get_pending_announcements",
        RemoteWorkspacePolicy::WorkspaceAgnostic,
    ),
    (
        "get_project_storage_paths",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    (
        "get_readonly_tools_info",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    (
        "get_recent_workspaces",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    (
        "get_runtime_capabilities",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    (
        "get_runtime_logging_info",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    (
        "get_session_file_diff_stats",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    ("get_session_files", RemoteWorkspacePolicy::LegacyUnaudited),
    (
        "get_session_operations",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    ("get_session_stats", RemoteWorkspacePolicy::LegacyUnaudited),
    (
        "get_session_thread_goal",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    ("get_session_turns", RemoteWorkspacePolicy::LegacyUnaudited),
    (
        "get_session_usage_report",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    ("get_skill_configs", RemoteWorkspacePolicy::LegacyUnaudited),
    (
        "get_snapshot_sessions",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    (
        "get_snapshot_system_stats",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    (
        "get_startup_native_trace",
        RemoteWorkspacePolicy::WorkspaceAgnostic,
    ),
    ("get_statistics", RemoteWorkspacePolicy::LegacyUnaudited),
    ("get_storage_paths", RemoteWorkspacePolicy::LegacyUnaudited),
    (
        "get_storage_statistics",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    (
        "get_subagent_detail",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    ("get_system_info", RemoteWorkspacePolicy::WorkspaceAgnostic),
    ("get_tool_info", RemoteWorkspacePolicy::LegacyUnaudited),
    ("get_turn_files", RemoteWorkspacePolicy::LegacyUnaudited),
    ("get_watched_paths", RemoteWorkspacePolicy::LegacyUnaudited),
    (
        "get_work_state_summary",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    ("git_add_files", RemoteWorkspacePolicy::RemoteRouted),
    ("git_add_worktree", RemoteWorkspacePolicy::RemoteUnsupported),
    ("git_checkout_branch", RemoteWorkspacePolicy::RemoteRouted),
    ("git_cherry_pick", RemoteWorkspacePolicy::RemoteRouted),
    ("git_cherry_pick_abort", RemoteWorkspacePolicy::RemoteRouted),
    (
        "git_cherry_pick_continue",
        RemoteWorkspacePolicy::RemoteRouted,
    ),
    ("git_commit", RemoteWorkspacePolicy::RemoteRouted),
    ("git_create_branch", RemoteWorkspacePolicy::RemoteRouted),
    ("git_delete_branch", RemoteWorkspacePolicy::RemoteRouted),
    ("git_get_branches", RemoteWorkspacePolicy::RemoteRouted),
    ("git_get_changed_files", RemoteWorkspacePolicy::RemoteRouted),
    ("git_get_commits", RemoteWorkspacePolicy::RemoteRouted),
    ("git_get_diff", RemoteWorkspacePolicy::RemoteRouted),
    (
        "git_get_enhanced_branches",
        RemoteWorkspacePolicy::RemoteRouted,
    ),
    ("git_get_file_content", RemoteWorkspacePolicy::RemoteRouted),
    ("git_get_graph", RemoteWorkspacePolicy::RemoteUnsupported),
    ("git_get_repository", RemoteWorkspacePolicy::RemoteRouted),
    (
        "git_get_repository_basic",
        RemoteWorkspacePolicy::RemoteRouted,
    ),
    ("git_get_status", RemoteWorkspacePolicy::RemoteRouted),
    ("git_is_repository", RemoteWorkspacePolicy::RemoteRouted),
    (
        "git_list_worktrees",
        RemoteWorkspacePolicy::RemoteUnsupported,
    ),
    ("git_pull", RemoteWorkspacePolicy::RemoteRouted),
    ("git_push", RemoteWorkspacePolicy::RemoteRouted),
    (
        "git_remove_worktree",
        RemoteWorkspacePolicy::RemoteUnsupported,
    ),
    ("git_reset_files", RemoteWorkspacePolicy::RemoteRouted),
    ("git_reset_to_commit", RemoteWorkspacePolicy::RemoteRouted),
    ("git_resolve_revision", RemoteWorkspacePolicy::RemoteRouted),
    ("grant_miniapp_path", RemoteWorkspacePolicy::LegacyUnaudited),
    (
        "grant_miniapp_workspace",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    ("has_insights_data", RemoteWorkspacePolicy::LegacyUnaudited),
    (
        "hide_agent_companion_desktop_pet",
        RemoteWorkspacePolicy::LocalOnly,
    ),
    (
        "hide_main_window_after_close_request",
        RemoteWorkspacePolicy::LocalOnly,
    ),
    ("i18n_get_config", RemoteWorkspacePolicy::WorkspaceAgnostic),
    (
        "i18n_get_current_language",
        RemoteWorkspacePolicy::WorkspaceAgnostic,
    ),
    (
        "i18n_get_supported_languages",
        RemoteWorkspacePolicy::WorkspaceAgnostic,
    ),
    ("i18n_set_config", RemoteWorkspacePolicy::WorkspaceAgnostic),
    (
        "i18n_set_language",
        RemoteWorkspacePolicy::WorkspaceAgnostic,
    ),
    (
        "import_agent_companion_pet_package",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    ("import_config", RemoteWorkspacePolicy::LegacyUnaudited),
    (
        "initialize_acp_clients",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    ("initialize_ai", RemoteWorkspacePolicy::LegacyUnaudited),
    (
        "initialize_mcp_servers",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    (
        "initialize_mcp_servers_non_destructive",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    (
        "initialize_project_storage",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    (
        "initialize_snapshot",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    (
        "initialize_tray_after_startup",
        RemoteWorkspacePolicy::LocalOnly,
    ),
    (
        "initialize_workspace_startup_state",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    (
        "install_acp_client_cli",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    ("install_update", RemoteWorkspacePolicy::WorkspaceAgnostic),
    (
        "list_agent_companion_pets",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    (
        "list_agent_tool_names",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    (
        "list_ai_models_by_config",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    (
        "list_archived_sessions",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    (
        "list_background_command_activities",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    ("list_cron_jobs", RemoteWorkspacePolicy::LegacyUnaudited),
    (
        "list_directory_files",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    (
        "list_manageable_subagents",
        RemoteWorkspacePolicy::RemoteRouted,
    ),
    ("list_mcp_prompts", RemoteWorkspacePolicy::LegacyUnaudited),
    ("list_mcp_resources", RemoteWorkspacePolicy::LegacyUnaudited),
    ("list_miniapps", RemoteWorkspacePolicy::LegacyUnaudited),
    (
        "list_persisted_sessions",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    (
        "list_persisted_sessions_page",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    ("list_sessions", RemoteWorkspacePolicy::LegacyUnaudited),
    ("list_skill_market", RemoteWorkspacePolicy::LegacyUnaudited),
    ("list_subagents", RemoteWorkspacePolicy::RemoteRouted),
    (
        "list_visible_subagents",
        RemoteWorkspacePolicy::RemoteRouted,
    ),
    (
        "load_acp_json_config",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    (
        "load_canvas_artifact",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    ("load_canvas_state", RemoteWorkspacePolicy::LegacyUnaudited),
    (
        "load_git_repo_history",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    (
        "load_insights_report",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    (
        "load_mcp_json_config",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    (
        "load_persisted_session_metadata",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    ("load_session_turns", RemoteWorkspacePolicy::LegacyUnaudited),
    (
        "lsp_change_document",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    ("lsp_close_document", RemoteWorkspacePolicy::LegacyUnaudited),
    (
        "lsp_close_workspace",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    ("lsp_detect_project", RemoteWorkspacePolicy::LegacyUnaudited),
    ("lsp_did_change", RemoteWorkspacePolicy::LegacyUnaudited),
    ("lsp_did_close", RemoteWorkspacePolicy::LegacyUnaudited),
    ("lsp_did_open", RemoteWorkspacePolicy::LegacyUnaudited),
    ("lsp_did_save", RemoteWorkspacePolicy::LegacyUnaudited),
    (
        "lsp_find_references",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    (
        "lsp_find_references_workspace",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    (
        "lsp_format_document",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    (
        "lsp_format_document_workspace",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    (
        "lsp_get_all_server_states",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    (
        "lsp_get_code_actions_workspace",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    (
        "lsp_get_completions",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    (
        "lsp_get_completions_workspace",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    (
        "lsp_get_document_highlight_workspace",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    (
        "lsp_get_document_symbols_workspace",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    ("lsp_get_hover", RemoteWorkspacePolicy::LegacyUnaudited),
    (
        "lsp_get_hover_workspace",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    (
        "lsp_get_inlay_hints_workspace",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    ("lsp_get_plugin", RemoteWorkspacePolicy::LegacyUnaudited),
    (
        "lsp_get_semantic_tokens_range_workspace",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    (
        "lsp_get_semantic_tokens_workspace",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    (
        "lsp_get_server_capabilities",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    (
        "lsp_get_server_state",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    (
        "lsp_get_supported_extensions",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    (
        "lsp_goto_definition",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    (
        "lsp_goto_definition_workspace",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    ("lsp_initialize", RemoteWorkspacePolicy::LegacyUnaudited),
    ("lsp_install_plugin", RemoteWorkspacePolicy::LegacyUnaudited),
    ("lsp_list_plugins", RemoteWorkspacePolicy::LegacyUnaudited),
    (
        "lsp_list_workspaces",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    ("lsp_open_document", RemoteWorkspacePolicy::LegacyUnaudited),
    ("lsp_open_workspace", RemoteWorkspacePolicy::LegacyUnaudited),
    (
        "lsp_prestart_server",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    (
        "lsp_rename_workspace",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    ("lsp_save_document", RemoteWorkspacePolicy::LegacyUnaudited),
    (
        "lsp_start_server_for_file",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    (
        "lsp_stop_all_servers",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    ("lsp_stop_server", RemoteWorkspacePolicy::LegacyUnaudited),
    (
        "lsp_stop_server_workspace",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    (
        "lsp_uninstall_plugin",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    (
        "mark_announcement_seen",
        RemoteWorkspacePolicy::WorkspaceAgnostic,
    ),
    (
        "miniapp_agent_cancel",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    (
        "miniapp_agent_cancel_stale_runs",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    ("miniapp_agent_run", RemoteWorkspacePolicy::LegacyUnaudited),
    (
        "miniapp_agent_turn_text",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    ("miniapp_ai_cancel", RemoteWorkspacePolicy::LegacyUnaudited),
    ("miniapp_ai_chat", RemoteWorkspacePolicy::LegacyUnaudited),
    (
        "miniapp_ai_complete",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    (
        "miniapp_ai_list_models",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    (
        "miniapp_apply_draft",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    (
        "miniapp_create_draft",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    (
        "miniapp_decline_builtin_update",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    (
        "miniapp_dialog_message",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    (
        "miniapp_discard_draft",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    (
        "miniapp_draft_host_call",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    (
        "miniapp_draft_worker_call",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    (
        "miniapp_draft_worker_stop",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    (
        "miniapp_get_customization_metadata",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    ("miniapp_get_draft", RemoteWorkspacePolicy::LegacyUnaudited),
    ("miniapp_host_call", RemoteWorkspacePolicy::LegacyUnaudited),
    (
        "miniapp_import_from_path",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    (
        "miniapp_install_deps",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    (
        "miniapp_permission_diff_for_draft",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    ("miniapp_recompile", RemoteWorkspacePolicy::LegacyUnaudited),
    (
        "miniapp_render_slide_page",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    (
        "miniapp_runtime_status",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    (
        "miniapp_set_draft_permissions",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    (
        "miniapp_sync_draft_from_fs",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    (
        "miniapp_sync_from_fs",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    (
        "miniapp_worker_call",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    (
        "miniapp_worker_list_running",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    (
        "miniapp_worker_stop",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    ("minimize_to_tray", RemoteWorkspacePolicy::LocalOnly),
    (
        "never_show_announcement",
        RemoteWorkspacePolicy::WorkspaceAgnostic,
    ),
    (
        "notify_cron_host_ready",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    (
        "open_html_file_in_browser",
        RemoteWorkspacePolicy::LocalOnly,
    ),
    ("open_remote_workspace", RemoteWorkspacePolicy::RemoteRouted),
    ("open_workspace", RemoteWorkspacePolicy::LegacyUnaudited),
    ("paste_files", RemoteWorkspacePolicy::LegacyUnaudited),
    (
        "peer_control_attach",
        RemoteWorkspacePolicy::WorkspaceAgnostic,
    ),
    (
        "peer_control_detach",
        RemoteWorkspacePolicy::WorkspaceAgnostic,
    ),
    (
        "peer_controller_set_active",
        RemoteWorkspacePolicy::WorkspaceAgnostic,
    ),
    (
        "peer_host_invoke_complete",
        RemoteWorkspacePolicy::WorkspaceAgnostic,
    ),
    ("peer_mode_ping", RemoteWorkspacePolicy::WorkspaceAgnostic),
    (
        "predownload_acp_client_adapter",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    (
        "preview_commit_message",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    (
        "probe_acp_client_requirements",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    (
        "quick_analyze_work_state",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    (
        "quick_commit_message",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    ("quit_app", RemoteWorkspacePolicy::LocalOnly),
    (
        "read_background_command_output",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    ("read_file_content", RemoteWorkspacePolicy::LegacyUnaudited),
    ("read_mcp_resource", RemoteWorkspacePolicy::LegacyUnaudited),
    ("record_file_change", RemoteWorkspacePolicy::LegacyUnaudited),
    (
        "refresh_cli_credential",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    (
        "refresh_model_client",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    ("reject_file", RemoteWorkspacePolicy::LegacyUnaudited),
    ("reject_operation", RemoteWorkspacePolicy::LegacyUnaudited),
    (
        "reject_tool_execution",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    ("reload_config", RemoteWorkspacePolicy::LegacyUnaudited),
    (
        "reload_custom_agents",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    (
        "reload_global_config",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    ("reload_subagents", RemoteWorkspacePolicy::LegacyUnaudited),
    (
        "relay_deploy_install_docker",
        RemoteWorkspacePolicy::WorkspaceAgnostic,
    ),
    (
        "relay_deploy_poll",
        RemoteWorkspacePolicy::WorkspaceAgnostic,
    ),
    (
        "relay_deploy_preflight",
        RemoteWorkspacePolicy::WorkspaceAgnostic,
    ),
    (
        "relay_deploy_register",
        RemoteWorkspacePolicy::WorkspaceAgnostic,
    ),
    (
        "relay_deploy_start",
        RemoteWorkspacePolicy::WorkspaceAgnostic,
    ),
    (
        "relay_deploy_verify",
        RemoteWorkspacePolicy::WorkspaceAgnostic,
    ),
    (
        "remote_close_workspace",
        RemoteWorkspacePolicy::RemoteRouted,
    ),
    (
        "remote_connect_configure_bot",
        RemoteWorkspacePolicy::WorkspaceAgnostic,
    ),
    (
        "remote_connect_configure_custom_server",
        RemoteWorkspacePolicy::WorkspaceAgnostic,
    ),
    (
        "remote_connect_get_bot_verbose_mode",
        RemoteWorkspacePolicy::WorkspaceAgnostic,
    ),
    (
        "remote_connect_get_device_info",
        RemoteWorkspacePolicy::WorkspaceAgnostic,
    ),
    (
        "remote_connect_get_form_state",
        RemoteWorkspacePolicy::WorkspaceAgnostic,
    ),
    (
        "remote_connect_get_lan_ip",
        RemoteWorkspacePolicy::WorkspaceAgnostic,
    ),
    (
        "remote_connect_get_lan_network_info",
        RemoteWorkspacePolicy::WorkspaceAgnostic,
    ),
    (
        "remote_connect_get_methods",
        RemoteWorkspacePolicy::WorkspaceAgnostic,
    ),
    (
        "remote_connect_set_bot_verbose_mode",
        RemoteWorkspacePolicy::WorkspaceAgnostic,
    ),
    (
        "remote_connect_set_form_state",
        RemoteWorkspacePolicy::WorkspaceAgnostic,
    ),
    (
        "remote_connect_start",
        RemoteWorkspacePolicy::WorkspaceAgnostic,
    ),
    (
        "remote_connect_status",
        RemoteWorkspacePolicy::WorkspaceAgnostic,
    ),
    (
        "remote_connect_stop",
        RemoteWorkspacePolicy::WorkspaceAgnostic,
    ),
    (
        "remote_connect_stop_bot",
        RemoteWorkspacePolicy::WorkspaceAgnostic,
    ),
    (
        "remote_connect_weixin_qr_poll",
        RemoteWorkspacePolicy::WorkspaceAgnostic,
    ),
    (
        "remote_connect_weixin_qr_start",
        RemoteWorkspacePolicy::WorkspaceAgnostic,
    ),
    ("remote_create_dir", RemoteWorkspacePolicy::RemoteRouted),
    (
        "remote_download_to_local_path",
        RemoteWorkspacePolicy::RemoteRouted,
    ),
    ("remote_execute", RemoteWorkspacePolicy::RemoteRouted),
    ("remote_exists", RemoteWorkspacePolicy::RemoteRouted),
    ("remote_get_tree", RemoteWorkspacePolicy::RemoteRouted),
    (
        "remote_get_workspace_info",
        RemoteWorkspacePolicy::RemoteRouted,
    ),
    ("remote_open_workspace", RemoteWorkspacePolicy::RemoteRouted),
    ("remote_read_dir", RemoteWorkspacePolicy::RemoteRouted),
    ("remote_read_file", RemoteWorkspacePolicy::RemoteRouted),
    ("remote_remove", RemoteWorkspacePolicy::RemoteRouted),
    (
        "remote_remove_workspace",
        RemoteWorkspacePolicy::RemoteRouted,
    ),
    ("remote_rename", RemoteWorkspacePolicy::RemoteRouted),
    (
        "remote_upload_from_local_path",
        RemoteWorkspacePolicy::RemoteRouted,
    ),
    ("remote_write_file", RemoteWorkspacePolicy::RemoteRouted),
    (
        "remove_recent_workspace",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    ("rename_file", RemoteWorkspacePolicy::LegacyUnaudited),
    (
        "reorder_opened_workspaces",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    (
        "replace_mode_skill_selection",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    (
        "report_canvas_runtime_error",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    (
        "report_ide_control_result",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    (
        "reset_agent_profile_config",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    (
        "reset_assistant_workspace",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    ("reset_config", RemoteWorkspacePolicy::LegacyUnaudited),
    ("reset_memory", RemoteWorkspacePolicy::LegacyUnaudited),
    (
        "reset_mode_skill_selection",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    (
        "reset_workspace_persona_files",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    (
        "resize_agent_companion_desktop_pet",
        RemoteWorkspacePolicy::LocalOnly,
    ),
    ("restart_app", RemoteWorkspacePolicy::LocalOnly),
    ("restart_mcp_server", RemoteWorkspacePolicy::LegacyUnaudited),
    ("restore_session", RemoteWorkspacePolicy::LegacyUnaudited),
    (
        "restore_session_view",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    (
        "restore_session_with_turns",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    ("reveal_in_explorer", RemoteWorkspacePolicy::LocalOnly),
    (
        "review_platform_clear_auth_token",
        RemoteWorkspacePolicy::WorkspaceAgnostic,
    ),
    (
        "review_platform_get_issue",
        RemoteWorkspacePolicy::RemoteRouted,
    ),
    (
        "review_platform_get_pull_request_ci_log",
        RemoteWorkspacePolicy::RemoteRouted,
    ),
    (
        "review_platform_get_pull_request_detail",
        RemoteWorkspacePolicy::RemoteRouted,
    ),
    (
        "review_platform_get_pull_request_detail_page",
        RemoteWorkspacePolicy::RemoteRouted,
    ),
    (
        "review_platform_get_pull_request_review_target",
        RemoteWorkspacePolicy::RemoteRouted,
    ),
    (
        "review_platform_get_pull_request_review_target_by_identity",
        RemoteWorkspacePolicy::RemoteRouted,
    ),
    (
        "review_platform_get_workspace_context",
        RemoteWorkspacePolicy::RemoteRouted,
    ),
    (
        "review_platform_get_workspace_snapshot",
        RemoteWorkspacePolicy::RemoteRouted,
    ),
    (
        "review_platform_update_auth_token",
        RemoteWorkspacePolicy::WorkspaceAgnostic,
    ),
    ("rollback_miniapp", RemoteWorkspacePolicy::LegacyUnaudited),
    ("rollback_session", RemoteWorkspacePolicy::LegacyUnaudited),
    ("rollback_to_turn", RemoteWorkspacePolicy::LegacyUnaudited),
    ("run_init_agents_md", RemoteWorkspacePolicy::LegacyUnaudited),
    ("run_system_command", RemoteWorkspacePolicy::LegacyUnaudited),
    (
        "save_acp_json_config",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    ("save_canvas_state", RemoteWorkspacePolicy::LegacyUnaudited),
    (
        "save_git_repo_history",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    (
        "save_mcp_json_config",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    (
        "save_merged_diff_content",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    (
        "save_session_metadata",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    ("save_session_turn", RemoteWorkspacePolicy::LegacyUnaudited),
    (
        "scan_workspace_info",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    ("search_build_index", RemoteWorkspacePolicy::RemoteRouted),
    (
        "search_file_contents",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    ("search_filenames", RemoteWorkspacePolicy::LegacyUnaudited),
    ("search_files", RemoteWorkspacePolicy::LegacyUnaudited),
    (
        "search_get_repo_status",
        RemoteWorkspacePolicy::RemoteRouted,
    ),
    ("search_rebuild_index", RemoteWorkspacePolicy::RemoteRouted),
    (
        "search_skill_market",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    (
        "send_background_command_input",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    (
        "send_mcp_app_message",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    ("send_system_notification", RemoteWorkspacePolicy::LocalOnly),
    (
        "set_acp_session_model",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    (
        "set_active_workspace",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    (
        "set_agent_profile_config",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    ("set_config", RemoteWorkspacePolicy::LegacyUnaudited),
    (
        "set_external_mcp_server_decision_command",
        RemoteWorkspacePolicy::RemoteUnsupported,
    ),
    (
        "set_external_source_conflict_choice_command",
        RemoteWorkspacePolicy::RemoteUnsupported,
    ),
    (
        "set_external_source_enabled_command",
        RemoteWorkspacePolicy::RemoteUnsupported,
    ),
    (
        "set_external_tool_conflict_choice_command",
        RemoteWorkspacePolicy::RemoteUnsupported,
    ),
    (
        "set_external_tool_target_decision_command",
        RemoteWorkspacePolicy::RemoteUnsupported,
    ),
    (
        "set_external_subagent_activation_command",
        RemoteWorkspacePolicy::RemoteUnsupported,
    ),
    ("set_macos_edit_menu_mode", RemoteWorkspacePolicy::LocalOnly),
    (
        "set_miniapp_draft_storage",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    (
        "set_miniapp_storage",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    (
        "set_mode_skill_disabled",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    (
        "set_session_memory_mode",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    (
        "set_session_thread_goal_status",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    (
        "set_subagent_timeout",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    (
        "show_agent_companion_desktop_pet",
        RemoteWorkspacePolicy::LocalOnly,
    ),
    ("show_main_window", RemoteWorkspacePolicy::LocalOnly),
    ("ssh_connect", RemoteWorkspacePolicy::WorkspaceAgnostic),
    (
        "ssh_delete_connection",
        RemoteWorkspacePolicy::WorkspaceAgnostic,
    ),
    ("ssh_disconnect", RemoteWorkspacePolicy::WorkspaceAgnostic),
    (
        "ssh_disconnect_all",
        RemoteWorkspacePolicy::WorkspaceAgnostic,
    ),
    ("ssh_get_config", RemoteWorkspacePolicy::WorkspaceAgnostic),
    (
        "ssh_get_server_info",
        RemoteWorkspacePolicy::WorkspaceAgnostic,
    ),
    (
        "ssh_has_stored_password",
        RemoteWorkspacePolicy::WorkspaceAgnostic,
    ),
    ("ssh_is_connected", RemoteWorkspacePolicy::WorkspaceAgnostic),
    (
        "ssh_list_config_hosts",
        RemoteWorkspacePolicy::WorkspaceAgnostic,
    ),
    (
        "ssh_list_saved_connections",
        RemoteWorkspacePolicy::WorkspaceAgnostic,
    ),
    (
        "ssh_save_connection",
        RemoteWorkspacePolicy::WorkspaceAgnostic,
    ),
    (
        "start_acp_dialog_turn",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    ("start_dialog_turn", RemoteWorkspacePolicy::LegacyUnaudited),
    ("start_file_watch", RemoteWorkspacePolicy::LegacyUnaudited),
    (
        "start_mcp_remote_oauth",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    ("start_mcp_server", RemoteWorkspacePolicy::LegacyUnaudited),
    (
        "start_search_file_contents_stream",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    (
        "start_search_filenames_stream",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    ("startup_window_control", RemoteWorkspacePolicy::LocalOnly),
    ("steer_dialog_turn", RemoteWorkspacePolicy::LegacyUnaudited),
    ("stop_acp_client", RemoteWorkspacePolicy::LegacyUnaudited),
    ("stop_file_watch", RemoteWorkspacePolicy::LegacyUnaudited),
    ("stop_mcp_server", RemoteWorkspacePolicy::LegacyUnaudited),
    (
        "submit_acp_permission_response",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    (
        "submit_mcp_interaction_response",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    (
        "submit_user_answers",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    (
        "subscribe_config_updates",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    (
        "sync_config_to_global",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    ("terminal_ack", RemoteWorkspacePolicy::RemoteRouted),
    ("terminal_close", RemoteWorkspacePolicy::RemoteRouted),
    ("terminal_create", RemoteWorkspacePolicy::RemoteRouted),
    ("terminal_execute", RemoteWorkspacePolicy::RemoteRouted),
    ("terminal_get", RemoteWorkspacePolicy::LegacyUnaudited),
    ("terminal_get_history", RemoteWorkspacePolicy::RemoteRouted),
    (
        "terminal_get_shells",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    (
        "terminal_has_shell_integration",
        RemoteWorkspacePolicy::RemoteRouted,
    ),
    ("terminal_list", RemoteWorkspacePolicy::LegacyUnaudited),
    ("terminal_resize", RemoteWorkspacePolicy::RemoteRouted),
    ("terminal_send_command", RemoteWorkspacePolicy::RemoteRouted),
    (
        "terminal_shutdown_all",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    ("terminal_signal", RemoteWorkspacePolicy::RemoteRouted),
    ("terminal_write", RemoteWorkspacePolicy::RemoteRouted),
    (
        "test_ai_config_connection",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    ("test_ai_connection", RemoteWorkspacePolicy::LegacyUnaudited),
    (
        "toggle_main_window_fullscreen",
        RemoteWorkspacePolicy::LocalOnly,
    ),
    (
        "touch_session_activity",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    (
        "trigger_announcement",
        RemoteWorkspacePolicy::WorkspaceAgnostic,
    ),
    ("unarchive_session", RemoteWorkspacePolicy::LegacyUnaudited),
    (
        "update_app_status",
        RemoteWorkspacePolicy::WorkspaceAgnostic,
    ),
    ("update_cron_job", RemoteWorkspacePolicy::LegacyUnaudited),
    (
        "update_custom_agent",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    (
        "update_mcp_remote_auth",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    ("update_miniapp", RemoteWorkspacePolicy::LegacyUnaudited),
    (
        "update_session_model",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    (
        "update_session_thread_goal_objective",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    (
        "update_session_title",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    ("update_subagent", RemoteWorkspacePolicy::LegacyUnaudited),
    (
        "update_subagent_config",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    (
        "update_workspace_info",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    (
        "upload_image_contexts",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    ("validate_config", RemoteWorkspacePolicy::LegacyUnaudited),
    (
        "validate_skill_path",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    (
        "validate_tool_input",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    (
        "webdriver_bridge_result",
        RemoteWorkspacePolicy::LegacyUnaudited,
    ),
    ("write_file_content", RemoteWorkspacePolicy::LegacyUnaudited),
];

pub fn remote_workspace_policy(command: &str) -> Option<RemoteWorkspacePolicy> {
    REMOTE_WORKSPACE_COMMAND_POLICIES
        .iter()
        .find(|(name, _)| *name == command)
        .map(|(_, policy)| *policy)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    /// Extracts the command names registered in `tauri::generate_handler!`.
    fn registered_commands() -> BTreeSet<String> {
        let source = include_str!("../lib.rs");
        let start = source
            .find("generate_handler![")
            .expect("lib.rs must register commands via tauri::generate_handler!")
            + "generate_handler![".len();
        let block = &source[start..];
        let end = block
            .find("])")
            .expect("generate_handler! block must terminate with `])`");
        block[..end]
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty() && !line.starts_with("//"))
            .map(|line| {
                let entry = line.trim_end_matches(',');
                entry
                    .rsplit("::")
                    .next()
                    .expect("command path segments are non-empty")
                    .to_string()
            })
            .collect()
    }

    #[test]
    fn every_registered_command_declares_a_remote_workspace_policy() {
        let registered = registered_commands();
        assert!(
            registered.len() > 400,
            "generate_handler! parsing looks broken; only {} commands found",
            registered.len()
        );

        let declared: BTreeSet<String> = REMOTE_WORKSPACE_COMMAND_POLICIES
            .iter()
            .map(|(name, _)| (*name).to_string())
            .collect();
        assert_eq!(
            declared.len(),
            REMOTE_WORKSPACE_COMMAND_POLICIES.len(),
            "remote workspace policy registry contains duplicate command entries"
        );

        let missing: Vec<_> = registered.difference(&declared).cloned().collect();
        assert!(
            missing.is_empty(),
            "commands registered in generate_handler! without a remote workspace policy \
             (declare one in REMOTE_WORKSPACE_COMMAND_POLICIES; new commands must not use \
             LegacyUnaudited): {missing:?}"
        );

        let stale: Vec<_> = declared.difference(&registered).cloned().collect();
        assert!(
            stale.is_empty(),
            "remote workspace policies declared for commands that are no longer registered: {stale:?}"
        );
    }

    /// `LegacyUnaudited` is a frozen backlog: commands may graduate out of it
    /// once their remote workspace behavior is audited, but no command may be
    /// added to it. Do not append to this list; give new commands a real
    /// policy instead.
    #[test]
    fn legacy_unaudited_backlog_must_not_grow() {
        let unaudited: BTreeSet<&str> = REMOTE_WORKSPACE_COMMAND_POLICIES
            .iter()
            .filter(|(_, policy)| *policy == RemoteWorkspacePolicy::LegacyUnaudited)
            .map(|(name, _)| *name)
            .collect();
        let frozen: BTreeSet<&str> = LEGACY_UNAUDITED_BASELINE.iter().copied().collect();

        let added: Vec<_> = unaudited.difference(&frozen).collect();
        assert!(
            added.is_empty(),
            "new commands must declare an explicit remote workspace policy instead of \
             LegacyUnaudited: {added:?}"
        );
    }

    /// Frozen at introduction time. Only removals are allowed.
    const LEGACY_UNAUDITED_BASELINE: &[&str] = &[
        "accept_file",
        "accept_operation",
        "accept_session",
        "activate_session_goal",
        "add_skill",
        "analyze_work_state",
        "apply_patch",
        "archive_all_sessions",
        "archive_session",
        "btw_ask_stream",
        "btw_cancel",
        "cancel_acp_dialog_turn",
        "cancel_dialog_turn",
        "cancel_insights_generation",
        "cancel_mcp_remote_oauth",
        "cancel_search",
        "cancel_session",
        "cancel_tool",
        "cancel_transfer",
        "canonicalize_agent_profile_configs",
        "check_command_exists",
        "check_commands_exist",
        "check_git_isolation",
        "check_path_exists",
        "cleanup_invalid_workspaces",
        "cleanup_storage",
        "cleanup_storage_with_policy",
        "clear_mcp_remote_auth",
        "clear_session_thread_goal",
        "close_workspace",
        "compact_session",
        "compress_path",
        "compute_diff",
        "confirm_tool_execution",
        "control_background_command",
        "control_deep_review_queue",
        "create_acp_flow_session",
        "create_assistant_workspace",
        "create_cron_job",
        "create_custom_agent",
        "create_directory",
        "create_file",
        "create_miniapp",
        "create_session",
        "create_subagent",
        "decompress_path",
        "delete_agent_companion_pet_package",
        "delete_all_archived_sessions",
        "delete_assistant_workspace",
        "delete_cron_job",
        "delete_custom_agent",
        "delete_directory",
        "delete_file",
        "delete_mcp_server",
        "delete_miniapp",
        "delete_persisted_session",
        "delete_session",
        "delete_skill",
        "delete_subagent",
        "discover_cli_credentials",
        "download_skill_market",
        "editor_ai_cancel",
        "editor_ai_stream",
        "ensure_assistant_bootstrap",
        "ensure_coordinator_session",
        "execute_tool",
        "explorer_get_children",
        "explorer_get_children_paginated",
        "explorer_get_file_tree",
        "export_config",
        "export_diagnostics_bundle",
        "export_local_file_to_path",
        "export_session_transcript",
        "fetch_mcp_app_resource",
        "fork_session",
        "generate_commit_message",
        "generate_greeting_only",
        "generate_insights",
        "generate_session_title",
        "get_acp_clients",
        "get_acp_session_commands",
        "get_acp_session_options",
        "get_agent_profile_config",
        "get_agent_profile_configs",
        "get_all_modified_files",
        "get_all_tools_info",
        "get_available_modes",
        "get_available_tools",
        "get_baseline_snapshot_diff",
        "get_clipboard_files",
        "get_config",
        "get_configs",
        "get_current_workspace",
        "get_custom_agent_detail",
        "get_default_review_team_definition",
        "get_directory_children",
        "get_directory_children_paginated",
        "get_file_change_history",
        "get_file_diff",
        "get_file_editor_sync_hash",
        "get_file_metadata",
        "get_file_tree",
        "get_global_config_health",
        "get_global_config_status",
        "get_latest_insights",
        "get_mcp_prompt",
        "get_mcp_remote_oauth_session",
        "get_mcp_server_status",
        "get_mcp_servers",
        "get_mcp_tool_ui_uri",
        "get_memory_paths",
        "get_miniapp",
        "get_miniapp_draft_storage",
        "get_miniapp_storage",
        "get_miniapp_versions",
        "get_mode_skill_configs",
        "get_model_configs",
        "get_opened_workspaces",
        "get_operation_diff",
        "get_operation_summary",
        "get_project_storage_paths",
        "get_readonly_tools_info",
        "get_recent_workspaces",
        "get_runtime_capabilities",
        "get_runtime_logging_info",
        "get_session_file_diff_stats",
        "get_session_files",
        "get_session_operations",
        "get_session_stats",
        "get_session_thread_goal",
        "get_session_turns",
        "get_session_usage_report",
        "get_skill_configs",
        "get_snapshot_sessions",
        "get_snapshot_system_stats",
        "get_statistics",
        "get_storage_paths",
        "get_storage_statistics",
        "get_subagent_detail",
        "get_tool_info",
        "get_turn_files",
        "get_watched_paths",
        "get_work_state_summary",
        "grant_miniapp_path",
        "grant_miniapp_workspace",
        "has_insights_data",
        "import_agent_companion_pet_package",
        "import_config",
        "initialize_acp_clients",
        "initialize_ai",
        "initialize_mcp_servers",
        "initialize_mcp_servers_non_destructive",
        "initialize_project_storage",
        "initialize_snapshot",
        "initialize_workspace_startup_state",
        "install_acp_client_cli",
        "list_agent_companion_pets",
        "list_agent_tool_names",
        "list_ai_models_by_config",
        "list_archived_sessions",
        "list_background_command_activities",
        "list_cron_jobs",
        "list_directory_files",
        "list_manageable_subagents",
        "list_mcp_prompts",
        "list_mcp_resources",
        "list_miniapps",
        "list_persisted_sessions",
        "list_persisted_sessions_page",
        "list_sessions",
        "list_skill_market",
        "list_subagents",
        "list_visible_subagents",
        "load_acp_json_config",
        "load_canvas_artifact",
        "load_canvas_state",
        "load_git_repo_history",
        "load_insights_report",
        "load_mcp_json_config",
        "load_persisted_session_metadata",
        "load_session_turns",
        "lsp_change_document",
        "lsp_close_document",
        "lsp_close_workspace",
        "lsp_detect_project",
        "lsp_did_change",
        "lsp_did_close",
        "lsp_did_open",
        "lsp_did_save",
        "lsp_find_references",
        "lsp_find_references_workspace",
        "lsp_format_document",
        "lsp_format_document_workspace",
        "lsp_get_all_server_states",
        "lsp_get_code_actions_workspace",
        "lsp_get_completions",
        "lsp_get_completions_workspace",
        "lsp_get_document_highlight_workspace",
        "lsp_get_document_symbols_workspace",
        "lsp_get_hover",
        "lsp_get_hover_workspace",
        "lsp_get_inlay_hints_workspace",
        "lsp_get_plugin",
        "lsp_get_semantic_tokens_range_workspace",
        "lsp_get_semantic_tokens_workspace",
        "lsp_get_server_capabilities",
        "lsp_get_server_state",
        "lsp_get_supported_extensions",
        "lsp_goto_definition",
        "lsp_goto_definition_workspace",
        "lsp_initialize",
        "lsp_install_plugin",
        "lsp_list_plugins",
        "lsp_list_workspaces",
        "lsp_open_document",
        "lsp_open_workspace",
        "lsp_prestart_server",
        "lsp_rename_workspace",
        "lsp_save_document",
        "lsp_start_server_for_file",
        "lsp_stop_all_servers",
        "lsp_stop_server",
        "lsp_stop_server_workspace",
        "lsp_uninstall_plugin",
        "miniapp_agent_cancel",
        "miniapp_agent_cancel_stale_runs",
        "miniapp_agent_run",
        "miniapp_agent_turn_text",
        "miniapp_ai_cancel",
        "miniapp_ai_chat",
        "miniapp_ai_complete",
        "miniapp_ai_list_models",
        "miniapp_apply_draft",
        "miniapp_create_draft",
        "miniapp_decline_builtin_update",
        "miniapp_dialog_message",
        "miniapp_discard_draft",
        "miniapp_draft_host_call",
        "miniapp_draft_worker_call",
        "miniapp_draft_worker_stop",
        "miniapp_get_customization_metadata",
        "miniapp_get_draft",
        "miniapp_host_call",
        "miniapp_import_from_path",
        "miniapp_install_deps",
        "miniapp_permission_diff_for_draft",
        "miniapp_recompile",
        "miniapp_render_slide_page",
        "miniapp_runtime_status",
        "miniapp_set_draft_permissions",
        "miniapp_sync_draft_from_fs",
        "miniapp_sync_from_fs",
        "miniapp_worker_call",
        "miniapp_worker_list_running",
        "miniapp_worker_stop",
        "notify_cron_host_ready",
        "open_workspace",
        "paste_files",
        "predownload_acp_client_adapter",
        "preview_commit_message",
        "probe_acp_client_requirements",
        "quick_analyze_work_state",
        "quick_commit_message",
        "read_background_command_output",
        "read_file_content",
        "read_mcp_resource",
        "record_file_change",
        "refresh_cli_credential",
        "refresh_model_client",
        "reject_file",
        "reject_operation",
        "reject_tool_execution",
        "reload_config",
        "reload_custom_agents",
        "reload_global_config",
        "reload_subagents",
        "remove_recent_workspace",
        "rename_file",
        "reorder_opened_workspaces",
        "replace_mode_skill_selection",
        "report_canvas_runtime_error",
        "report_ide_control_result",
        "reset_agent_profile_config",
        "reset_assistant_workspace",
        "reset_config",
        "reset_memory",
        "reset_mode_skill_selection",
        "reset_workspace_persona_files",
        "restart_mcp_server",
        "restore_session",
        "restore_session_view",
        "restore_session_with_turns",
        "rollback_miniapp",
        "rollback_session",
        "rollback_to_turn",
        "run_init_agents_md",
        "run_system_command",
        "save_acp_json_config",
        "save_canvas_state",
        "save_git_repo_history",
        "save_mcp_json_config",
        "save_merged_diff_content",
        "save_session_metadata",
        "save_session_turn",
        "scan_workspace_info",
        "search_file_contents",
        "search_filenames",
        "search_files",
        "search_skill_market",
        "send_background_command_input",
        "send_mcp_app_message",
        "set_acp_session_model",
        "set_active_workspace",
        "set_agent_profile_config",
        "set_config",
        "set_miniapp_draft_storage",
        "set_miniapp_storage",
        "set_mode_skill_disabled",
        "set_session_memory_mode",
        "set_session_thread_goal_status",
        "set_subagent_timeout",
        "start_acp_dialog_turn",
        "start_dialog_turn",
        "start_file_watch",
        "start_mcp_remote_oauth",
        "start_mcp_server",
        "start_search_file_contents_stream",
        "start_search_filenames_stream",
        "steer_dialog_turn",
        "stop_acp_client",
        "stop_file_watch",
        "stop_mcp_server",
        "submit_acp_permission_response",
        "submit_mcp_interaction_response",
        "submit_user_answers",
        "subscribe_config_updates",
        "sync_config_to_global",
        "terminal_get",
        "terminal_get_shells",
        "terminal_list",
        "terminal_shutdown_all",
        "test_ai_config_connection",
        "test_ai_connection",
        "touch_session_activity",
        "unarchive_session",
        "update_cron_job",
        "update_custom_agent",
        "update_mcp_remote_auth",
        "update_miniapp",
        "update_session_model",
        "update_session_thread_goal_objective",
        "update_session_title",
        "update_subagent",
        "update_subagent_config",
        "update_workspace_info",
        "upload_image_contexts",
        "validate_config",
        "validate_skill_path",
        "validate_tool_input",
        "webdriver_bridge_result",
        "write_file_content",
    ];
}
