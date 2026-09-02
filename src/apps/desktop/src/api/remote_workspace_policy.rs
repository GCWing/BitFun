//! Desktop closure test for the Product Operation Registry.
//!
//! Every Tauri command registered in `lib.rs` (`tauri::generate_handler!`)
//! must have exactly one row in
//! `bitfun_product_domains::remote_surface`, which declares how the command
//! behaves for remote SSH/Docker workspaces and in Peer Device Mode. Remote SSH
//! workspaces have no central command router: each handler adapts itself
//! (usually through `resolve_desktop_path_target` / `lookup_remote_connection`
//! / `is_remote_path`), so the registry is what forces a new command to declare
//! its remote behavior at all. Historically the lack of that declaration
//! produced silent local/remote feature gaps (for example the PR reviewer
//! opening to a blank panel in remote workspaces).
//!
//! The registry owns the stances, the frozen `Unaudited` backlog, and the
//! ratchet tests; this module only proves the desktop registration set and the
//! registry's `TauriCommand` rows are the same set. See
//! `docs/architecture/remote-surface-contract.md`.

pub use bitfun_product_domains::remote_surface::RemoteWorkspaceStance as RemoteWorkspacePolicy;

/// The declared remote-workspace stance of a registered Tauri command.
pub fn remote_workspace_policy(command: &str) -> Option<RemoteWorkspacePolicy> {
    bitfun_product_domains::remote_surface::operation(command).map(|op| op.remote_workspace)
}

#[cfg(test)]
mod tests {
    use super::*;
    use bitfun_product_domains::remote_surface::{operations, OperationSurface};
    use std::collections::BTreeSet;

    /// Extracts the command names registered in `tauri::generate_handler!`.
    pub(crate) fn registered_commands() -> BTreeSet<String> {
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
    fn every_registered_command_has_exactly_one_registry_row() {
        let registered = registered_commands();
        assert!(
            registered.len() > 400,
            "generate_handler! parsing looks broken; only {} commands found",
            registered.len()
        );

        let declared: BTreeSet<String> = operations()
            .iter()
            .filter(|op| op.surface == OperationSurface::TauriCommand)
            .map(|op| op.id.to_string())
            .collect();

        let missing: Vec<_> = registered.difference(&declared).cloned().collect();
        assert!(
            missing.is_empty(),
            "commands registered in generate_handler! without a Product Operation Registry row \
             (add one `op(...)` row in src/crates/contracts/product-domains/src/remote_surface/table.rs; \
             new commands must not use Unaudited): {missing:?}"
        );

        let stale: Vec<_> = declared.difference(&registered).cloned().collect();
        assert!(
            stale.is_empty(),
            "registry rows declared as Tauri commands that are no longer registered \
             (delete the row, or mark it HostInvokeOnly if a peer alias must survive): {stale:?}"
        );
    }

    #[test]
    fn host_invoke_only_rows_are_not_registered_tauri_commands() {
        let registered = registered_commands();
        let leaked: Vec<_> = operations()
            .iter()
            .filter(|op| op.surface == OperationSurface::HostInvokeOnly)
            .filter(|op| registered.contains(op.id))
            .map(|op| op.id)
            .collect();
        assert!(
            leaked.is_empty(),
            "these rows are marked HostInvokeOnly but the desktop registers them; change the row to `op(...)`: {leaked:?}"
        );
    }

    #[test]
    fn token_usage_statistics_are_scoped_to_the_current_bitfun_host() {
        assert_eq!(
            remote_workspace_policy("get_token_usage_statistics"),
            Some(RemoteWorkspacePolicy::Agnostic),
            "token usage is recorded by the current BitFun runtime and does not follow the workspace filesystem to an SSH host"
        );
    }

    #[test]
    fn frontend_update_decisions_are_local_desktop_only() {
        for command in [
            "frontend_update_candidate_ready",
            "get_frontend_update_status",
            "confirm_frontend_update",
            "rollback_frontend_update",
        ] {
            assert_eq!(
                remote_workspace_policy(command),
                Some(RemoteWorkspacePolicy::LocalOnly),
                "{command} must stay with the immutable controller-side confirmation window"
            );
        }
    }

    #[test]
    fn external_mcp_import_commands_explicitly_reject_remote_workspaces() {
        for command in [
            "plan_external_mcp_import_command",
            "apply_external_mcp_import_command",
        ] {
            assert_eq!(
                remote_workspace_policy(command),
                Some(RemoteWorkspacePolicy::Unsupported),
                "{command} must never fall back to the controller's local MCP config"
            );
        }
    }

    #[test]
    fn workspace_reference_snapshot_explicitly_rejects_remote_workspaces() {
        assert_eq!(
            remote_workspace_policy("get_workspace_reference_snapshot"),
            Some(RemoteWorkspacePolicy::Unsupported),
            "workspace references must never scan controller-local OpenCode config for a remote workspace"
        );
    }

    #[test]
    fn external_hook_import_commands_explicitly_reject_remote_workspaces() {
        for command in [
            "get_external_hook_import_snapshot",
            "plan_external_hook_import_command",
            "apply_external_hook_import_command",
            "mutate_external_hook_import_command",
        ] {
            assert_eq!(
                remote_workspace_policy(command),
                Some(RemoteWorkspacePolicy::Unsupported),
                "{command} must never use local imported Hooks for a remote workspace"
            );
        }
    }

    #[test]
    fn complete_rollback_commands_explicitly_reject_remote_workspaces() {
        for command in ["rollback_session", "rollback_session_to_turn"] {
            assert_eq!(
                remote_workspace_policy(command),
                Some(RemoteWorkspacePolicy::Unsupported),
                "{command} must not offer message-only rollback without remote file snapshots"
            );
        }
    }

    #[test]
    fn external_source_control_command_is_registered() {
        const COMMAND: &str = "get_external_source_control_snapshot";
        assert!(
            registered_commands().contains(COMMAND),
            "Desktop must register the external-source control command"
        );
    }
}
