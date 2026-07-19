//! Deny tables for CLI Peer Host.

/// Commands that must never run on a peer on behalf of a controller.
/// Mirrors desktop `peer_host_invoke::LOCAL_ONLY_COMMANDS` (minus control-plane
/// commands which are handled specially before this check).
///
/// Keep `account_finalize_login` and cloud session/turn commands here — they
/// are controller identity/hydrate APIs. See
/// `src/web-ui/src/infrastructure/peer-device/README.md`.
static LOCAL_ONLY_COMMANDS: &[&str] = &[
    "show_main_window",
    "hide_main_window_after_close_request",
    "quit_app",
    "minimize_to_tray",
    "initialize_tray_after_startup",
    "startup_window_control",
    "toggle_main_window_fullscreen",
    "restart_app",
    "check_for_updates",
    "install_update",
    "account_login",
    "account_finalize_login",
    "account_logout",
    "account_status",
    "account_get_credential_hint",
    "account_token_expired",
    "account_connect_devices",
    "account_online_devices",
    "account_list_devices",
    "account_delete_device",
    "account_device_rpc",
    "account_delegate_to_paired",
    "account_auto_sync",
    "account_sync_settings",
    "account_fetch_settings",
    "account_sync_session",
    "account_fetch_synced_sessions",
    "account_delete_synced_session",
    "account_export_local_session",
    "account_export_all_sessions",
    "account_import_remote_sessions",
    "account_fetch_session_turns",
    "account_send_session_to_device",
    "account_execute_on_device",
    "peer_host_invoke_complete",
    "peer_controller_set_active",
    "remote_connect_get_device_info",
    "remote_connect_get_lan_ip",
    "remote_connect_get_lan_network_info",
    "remote_connect_get_methods",
    "remote_connect_start",
    "remote_connect_stop",
    "remote_connect_stop_bot",
    "remote_connect_status",
    "remote_connect_get_form_state",
    "remote_connect_set_form_state",
    "remote_connect_configure_custom_server",
    "remote_connect_configure_bot",
    "remote_connect_weixin_qr_start",
    "remote_connect_weixin_qr_poll",
    "remote_connect_get_bot_verbose_mode",
    "remote_connect_set_bot_verbose_mode",
    "computer_use_request_permissions",
    "computer_use_open_system_settings",
    "relay_deploy_preflight",
    "relay_deploy_install_docker",
    "relay_deploy_start",
    "relay_deploy_poll",
    "relay_deploy_cancel",
    "relay_deploy_register",
    "relay_deploy_verify",
];

/// Desktop IDE surfaces that CLI Peer Host does not implement.
/// Prefix match is applied for `lsp_`, `canvas_`, `editor_`, `ssh_`,
/// `terminal_`, `search_` unless the command is explicitly allowlisted.
///
/// `git_*` is intentionally not prefix-denied: `git_is_repository` is
/// implemented; other git commands fall through to the registry miss path.
static CLI_UNSUPPORTED_EXACT: &[&str] = &[
    "open_remote_workspace",
    "remote_get_workspace_info",
    "explorer_get_file_tree",
    "explorer_get_children",
    "explorer_get_children_paginated",
];

pub(crate) fn is_local_only_command(command: &str) -> bool {
    LOCAL_ONLY_COMMANDS.iter().any(|denied| *denied == command)
}

pub(crate) fn is_cli_unsupported_command(command: &str) -> bool {
    if CLI_UNSUPPORTED_EXACT.iter().any(|c| *c == command) {
        return true;
    }
    let prefixes = [
        "lsp_",
        "canvas_",
        "editor_",
        "ssh_",
        "terminal_",
        "search_",
        "plugin_",
        "miniapps_",
        "review_platform_",
    ];
    prefixes.iter().any(|prefix| command.starts_with(prefix))
}
