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
    "frontend_update_candidate_ready",
    "get_frontend_update_status",
    "confirm_frontend_update",
    "rollback_frontend_update",
    "quit_app",
    "minimize_to_tray",
    "initialize_tray_after_startup",
    "startup_window_control",
    "toggle_main_window_fullscreen",
    "set_main_window_transient_geometry",
    "get_prevent_sleep_enabled",
    "set_prevent_sleep_enabled",
    "restart_app",
    "check_for_updates",
    "install_update",
    "appearance_market_browse",
    "appearance_market_download_release",
    "appearance_market_get_listing",
    "appearance_market_get_review_submission",
    "appearance_market_list_review_submissions",
    "appearance_market_list_submissions",
    "appearance_market_review_submission",
    "appearance_market_submit_package",
    "appearance_market_withdraw_submission",
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
    "browser_webview_set_agent_target_state",
    "mark_bitfun_control_surface_ready",
    "mark_bitfun_control_surface_unready",
    "report_bitfun_control_result",
    "relay_deploy_preflight",
    "relay_deploy_install_docker",
    "relay_deploy_start",
    "relay_deploy_poll",
    "relay_deploy_cancel",
    "relay_deploy_register",
    "relay_deploy_verify",
    "dispatch_list_targets",
    "dispatch_probe_target",
    "dispatch_install_cli_start",
    "dispatch_install_cli_poll",
    "dispatch_install_cli_cancel",
    "dispatch_provision_target",
    "dispatch_sync_model_config",
    "dispatch_submit",
    "dispatch_status",
    "dispatch_query",
    "dispatch_sync_result",
    "dispatch_cancel",
    "dispatch_list_jobs",
    "dispatch_answer",
    "dispatch_append",
    "dispatch_continue",
    "dispatch_load_transcript",
    "dispatch_save_transcript",
    "speech_list_models",
    "speech_download_model",
    "speech_cancel_model_download",
    "speech_delete_model",
    "speech_verify_model",
    "speech_start_input_session",
    "speech_append_audio_chunk",
    "speech_finish_input_session",
    "speech_cancel_input_session",
    "speech_start_realtime_session",
    "speech_append_realtime_audio",
    "speech_commit_realtime_audio",
    "speech_send_realtime_tool_result",
    "speech_speak_realtime_text",
    "speech_cancel_realtime_response",
    "speech_close_realtime_session",
    "speech_get_realtime_config",
    "speech_save_realtime_config",
    // Granting Git ownership trust writes this user's global Git configuration
    // and tells Git to run hooks from a tree they do not own. That decision
    // belongs to the person at this machine, so refuse it explicitly rather
    // than relying on the command being unimplemented here.
    "git_trust_repository",
    // Controller app-shell state mirrored from the FE deny list. An older or
    // non-Web-UI controller can still HostInvoke these onto this peer, so the
    // CLI peer host must refuse them independently of the FE optimization.
    // Keep in sync with `src/web-ui/.../adapters/peer-device-adapter.ts`
    // LOCAL_ONLY_COMMANDS and `src/apps/desktop/src/api/peer_host_invoke.rs`.
    // These controller-owned commands are not implemented here either, but
    // being unimplemented is not the boundary — refuse explicitly.
    "i18n_get_current_language",
    "i18n_set_language",
    "i18n_get_supported_languages",
    "i18n_get_config",
    "i18n_set_config",
    "get_pending_announcements",
    "get_announcement_tips",
    "mark_announcement_seen",
    "dismiss_announcement",
    "never_show_announcement",
    "trigger_announcement",
    "list_agent_companion_pets",
    "import_agent_companion_pet_package",
    "delete_agent_companion_pet_package",
    "generate_insights",
    "get_latest_insights",
    "load_insights_report",
    "has_insights_data",
    "cancel_insights_generation",
    "report_ide_control_result",
    "browser_control_launch",
    "browser_control_list_browsers",
    "browser_control_get_status",
    "browser_control_restart_with_cdp",
    "browser_control_enable_default_cdp",
    "browser_control_disconnect",
    "browser_webview_create",
    "browser_webview_eval",
    "browser_webview_navigate",
    "browser_webview_reload",
    "browser_webview_set_bounds",
    "computer_use_get_status",
    "debug_devtools_available",
    "debug_open_devtools",
    "resize_agent_companion_desktop_pet",
    "show_agent_companion_desktop_pet",
    "hide_agent_companion_desktop_pet",
    "append_flow_chat_diagnostics",
];

/// Desktop IDE surfaces that CLI Peer Host does not implement.
/// Prefix match is applied for `canvas_`, `editor_`, `ssh_`,
/// `terminal_`, `search_` unless the command is explicitly allowlisted.
///
/// `git_*` is intentionally not prefix-denied: `git_is_repository` and the
/// read-only `git_get_repository_trust` are implemented; `git_trust_repository`
/// is refused as local-only above, and other git commands fall through to the
/// registry miss path.
static CLI_UNSUPPORTED_EXACT: &[&str] = &[
    "open_remote_workspace",
    "remote_get_workspace_info",
    "explorer_get_file_tree",
    "explorer_get_children",
    "explorer_get_children_paginated",
];

pub(crate) fn is_local_only_command(command: &str) -> bool {
    LOCAL_ONLY_COMMANDS.contains(&command)
}

/// Commands kept as protocol tombstones after their runtime owner was removed.
pub(crate) fn is_retired_command(command: &str) -> bool {
    command.starts_with("lsp_")
}

pub(crate) fn is_cli_unsupported_command(command: &str) -> bool {
    if command == "search_session_content" {
        return false;
    }
    if CLI_UNSUPPORTED_EXACT.contains(&command) {
        return true;
    }
    let prefixes = [
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

#[cfg(test)]
mod tests {
    use super::{is_cli_unsupported_command, is_local_only_command, is_retired_command};

    #[test]
    fn retired_lsp_commands_are_matched_without_overmatching_external_names() {
        assert!(is_retired_command("lsp_open_workspace"));
        assert!(!is_retired_command("custom_lsp_wrapper"));
    }

    #[test]
    fn session_content_search_executes_on_the_peer_host() {
        assert!(!is_cli_unsupported_command("search_session_content"));
        assert!(is_cli_unsupported_command("search_files"));
    }

    #[test]
    fn outbound_dispatch_control_plane_stays_local_only() {
        for command in [
            "dispatch_list_targets",
            "dispatch_probe_target",
            "dispatch_install_cli_start",
            "dispatch_install_cli_poll",
            "dispatch_install_cli_cancel",
            "dispatch_provision_target",
            "dispatch_sync_model_config",
            "dispatch_submit",
            "dispatch_status",
            "dispatch_query",
            "dispatch_sync_result",
            "dispatch_cancel",
            "dispatch_list_jobs",
            "dispatch_answer",
            "dispatch_append",
            "dispatch_continue",
            "dispatch_load_transcript",
            "dispatch_save_transcript",
        ] {
            assert!(is_local_only_command(command), "{command}");
        }
    }

    /// The controller-side FE deny list is an optimization, not the boundary.
    /// An older or non-FE controller still reaches this host.
    #[test]
    fn speech_capture_stays_on_the_controller_device() {
        for command in [
            "speech_list_models",
            "speech_download_model",
            "speech_cancel_model_download",
            "speech_delete_model",
            "speech_verify_model",
            "speech_start_input_session",
            "speech_append_audio_chunk",
            "speech_finish_input_session",
            "speech_cancel_input_session",
            "speech_start_realtime_session",
            "speech_append_realtime_audio",
            "speech_commit_realtime_audio",
            "speech_send_realtime_tool_result",
            "speech_speak_realtime_text",
            "speech_cancel_realtime_response",
            "speech_close_realtime_session",
            "speech_get_realtime_config",
            "speech_save_realtime_config",
        ] {
            assert!(is_local_only_command(command), "{command}");
        }
    }

    #[test]
    fn built_in_browser_target_lifecycle_stays_on_the_controller_device() {
        assert!(is_local_only_command(
            "browser_webview_set_agent_target_state"
        ));
    }

    #[test]
    fn frontend_update_decisions_stay_on_the_controller_device() {
        assert!(is_local_only_command("frontend_update_candidate_ready"));
        assert!(is_local_only_command("get_frontend_update_status"));
        assert!(is_local_only_command("confirm_frontend_update"));
        assert!(is_local_only_command("rollback_frontend_update"));
    }

    #[test]
    fn product_control_presentation_callbacks_stay_on_the_controller_device() {
        for command in [
            "mark_bitfun_control_surface_ready",
            "mark_bitfun_control_surface_unready",
            "report_bitfun_control_result",
        ] {
            assert!(is_local_only_command(command), "{command}");
        }
        assert!(!is_local_only_command("product_control_invoke"));
    }

    /// Reading why Git refuses a repository is safe to answer for a controller
    /// and is implemented here; granting the exception writes this user's
    /// global Git configuration and must be decided at this machine. Being
    /// unimplemented is not a boundary — say no explicitly.
    #[test]
    fn granting_git_ownership_trust_is_refused_on_the_peer() {
        assert!(is_local_only_command("git_trust_repository"));
        assert!(!is_local_only_command("git_get_repository_trust"));
    }
}
