//! CLI peer host command policy, derived from the Product Operation Registry.
//!
//! The registry (`openbitfun_product_domains::remote_surface`) is the single owner
//! of which HostInvoke commands a CLI peer host may run on a controller's
//! behalf, which it must refuse because the controller keeps them, and which
//! it cannot implement. This module only exposes the CLI-side predicates the
//! dispatcher and its tests use; the frontend and the desktop host derive their
//! tables from the same registry, so the three surfaces cannot drift apart.
//! See `docs/architecture/remote-surface-contract.md`.

use openbitfun_product_domains::remote_surface::{
    peer_host_verdict, peer_stance, retired_reason, PeerHostKind, PeerHostVerdict, PeerRefusal,
    PeerStance,
};

/// Whether this host must refuse the command on a controller's behalf
/// (`ControllerLocal` and `OperatorOnly` stances).
pub(crate) fn is_local_only_command(command: &str) -> bool {
    matches!(
        peer_stance(command),
        Some(PeerStance::ControllerLocal | PeerStance::OperatorOnly)
    )
}

/// Commands kept as protocol tombstones after their runtime owner was removed.
pub(crate) fn is_retired_command(command: &str) -> bool {
    retired_reason(command).is_some()
}

/// Whether the registry declares the command unsupported on a CLI peer host
/// (as opposed to refused because the controller keeps it, or unknown).
pub(crate) fn is_cli_unsupported_command(command: &str) -> bool {
    matches!(
        peer_host_verdict(command, PeerHostKind::Cli),
        PeerHostVerdict::Refuse(PeerRefusal::HostUnsupported { .. })
    )
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
            "mark_openbitfun_control_surface_ready",
            "mark_openbitfun_control_surface_unready",
            "report_openbitfun_control_result",
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

    /// Previously the CLI list lacked this entry while the desktop and the
    /// frontend refused it, and a CI exception hid the drift. One registry row
    /// now answers for all three surfaces.
    #[test]
    fn cancelling_a_pending_login_stays_on_the_controller_device() {
        assert!(is_local_only_command("account_cancel_pending_login"));
    }

    /// Browser and OS automation are controller-local in the frontend adapter
    /// but Desktop peers can run them for a controller; the CLI host simply
    /// has no such runtime, so it reports "unsupported here", not "local-only".
    #[test]
    fn browser_and_computer_use_are_unsupported_not_local_only_on_cli() {
        for command in [
            "browser_control_launch",
            "computer_use_get_status",
            "computer_use_request_permissions",
        ] {
            assert!(!is_local_only_command(command), "{command}");
            assert!(is_cli_unsupported_command(command), "{command}");
        }
    }
}
