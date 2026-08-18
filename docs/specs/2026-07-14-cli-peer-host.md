# CLI Peer Device Mode Host

## Goal

Desktop A can enter Peer Mode against a same-account CLI process on machine B
using the same HostInvoke / DeviceEvent wire protocol as Desktop B. The CLI is
a **host only** (not a Peer Mode controller).

## Non-goals

- CLI controlling other devices
- Remote-desktop / raw input mirroring
- Splitting same-machine Desktop + CLI into two device list entries
- Full IDE parity (LSP UI, canvas, window chrome) — deny with clear English errors

## Device identity and election

- `device_id = hash(hostname + MAC)` (shared across Desktop and CLI on one machine)
- Relay replaces the live connection for the same `(user_id, device_id)`
- Last successful `AuthConnect` wins as the Peer Host process

## Architecture

1. CLI decrypts `IncomingDeviceMessage` and **special-cases** `HostInvoke` /
   `DeviceEvent` (does not send HostInvoke through `RemoteServer::dispatch`).
2. Control plane in Rust: `peer_mode_ping`, `peer_control_attach`,
   `peer_control_detach`.
3. Product data plane: CLI HostInvoke registry calls Core services
   (`WorkspaceService`, `FileSystemService`, `PersistenceManager`,
   `ConversationCoordinator`, `DialogScheduler`, `ConfigService`, `GitService`)
   — same Tauri command names and `{ request: ... }` arg shape as Desktop.
4. Agentic (and selected UI) events fan out as `RemoteCommand::DeviceEvent` to
   attached controllers.
5. Denied / unsupported commands return
   `HostInvokeResult { ok: false, error }` — never fall back to the controller.

## Must-support command surface

Control: `peer_mode_ping`, `peer_control_attach`, `peer_control_detach`

Hydrate / chat / FS:

- Workspace: `initialize_workspace_startup_state`, `get_opened_workspaces`,
  `get_recent_workspaces`, `get_current_workspace`, `open_workspace`,
  `get_workspace_info`, `cleanup_invalid_workspaces`, `reload_config`
- Config: `get_config`, `get_configs`, `set_config`, `get_agent_profile_config`,
  `get_agent_profile_configs`
- Sessions: `list_persisted_sessions*`, `load_session_turns`,
  `restore_session_view`, `restore_session_with_turns`, `restore_session`,
  `create_session`, `delete_session`, `rename_session`, `archive_session`,
  `touch_session_activity`, `get_session_thread_goal`, `update_session_model`,
  `ensure_coordinator_session`, `get_available_modes`, `get_session_stats`,
  `save_session_turn`
- Snapshot: `rollback_to_turn`, `get_session_files`
- Dialog: `start_dialog_turn`, `cancel_dialog_turn`, `confirm_tool_execution`,
  `reject_tool_execution`
- FS / system: `get_directory_children`, `get_directory_children_paginated`,
  `list_files`, `check_path_exists`, `create_directory`, `get_system_info`
- Git (local): `git_is_repository`

### Soft empty / no-op (Desktop-only subsystems)

CLI does not run MiniApp runtime, Desktop cron host, or Peer-mounted ACP
client service. These commands succeed with empty / null payloads so Desktop
hydrate does not fail:

- `list_miniapps` → `[]`
- `miniapp_worker_list_running` → `[]`
- `get_acp_clients` → `[]`
- `notify_cron_host_ready` → null Ok
- `list_background_command_activities` → `{ activities: [] }`

## Ownership

- Implementation: `src/apps/cli/src/peer_host/`
- Routing: `src/apps/cli/src/account.rs`
- Product doc: `docs/architecture/peer-device-mode.md`
