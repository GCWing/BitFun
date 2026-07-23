# Peer Device Mode (frontend)

Controller-side React/transport layer for Peer Device Mode. Architecture:
[`docs/architecture/peer-device-mode.md`](../../../../../docs/architecture/peer-device-mode.md).

## Invariants (do not regress)

1. **Cloud session/turn APIs stay on the controller** (`LOCAL_ONLY` in
   `peer-device-adapter.ts`). Peer history comes from HostInvoke
   (`restore_session_view`, list sessions, …), not from
   `account_fetch_session_turns`.

2. **Fail-closed cloud import must skip Peer Mode.**
   `FlowChatStore.loadSessionHistory` calls `accountFetchSessionTurns` and
   throws on failure for incomplete relay imports. In Peer Mode that command is
   paused — **skip the call** when `isPeerDeviceModeActive()` is true, then
   restore via the peer. Do not reintroduce “throw on any fetch error” without
   a Peer Mode gate (regression: 2026-07-19 session harden commit).

3. **Backend peer pauses must soft-succeed for hydrate paths.** Prefer
   `Ok(false)` / empty success over hard `Err` for
   `account_fetch_session_turns` / `account_auto_sync` while the controller is
   in Peer Mode, so accidental callers do not abort UI restore.

4. **Clear `FlowChatManager.currentWorkspacePath` on peer switch.** Stale
   controller paths (e.g. Windows) must not be reused for `create_session` on a
   peer host (e.g. Mac). `initialize()` failure must **throw**, never return
   `false` (callers treat `false` as “no history → create session”).

5. **Create-session always passes the live workspace path**
   (`flowChatSessionConfigForWorkspace`). Empty `{}` configs are unsafe after
   peer switch.

6. **Config / mode HostInvokes are high priority** during peer hydrate
   (`get_config`, `get_configs`, `get_available_modes`,
   `get_agent_profile_config`). Keeping them `low` starves hydrate under the
   concurrency limit of 2.

7. **Account identity commands are LOCAL_ONLY** and must stay denied on the
   peer host (`account_login`, `account_finalize_login`, logout, device RPC,
   …). Keep FE adapter, desktop `peer_host_invoke`, and CLI `peer_host/deny`
   lists aligned.

8. **`relay_deploy_*` is LOCAL_ONLY.** One-click deploy SSHes from the
   controller to a user-owned host; do not HostInvoke it onto the peer.

9. **Clear workspace before peer flag emit.** `resetProductSurface` must call
   `workspaceManager.clearForPeerModeSwitch()` so SessionModule cannot prefer
   a stale controller path while rebootstrap is in flight. Never pass `{}` to
   `createChatSession` when a live workspace exists — use
   `flowChatSessionConfigForCurrentWorkspace`.

10. **Download destinations stay on the controller.** Native dialogs select a
    path on A. Read file chunks from B with direct Peer commands, then write
    them through A's local filesystem adapter. Do not HostInvoke
    `export_local_file_to_path` with A's path. Directory downloads must preserve
    the tree and reject traversal-like entry names.

11. **Terminal traffic stays interactive and observable.** All `terminal_*`
    commands are high priority, low-priority polling leaves one transport slot
    available, and both local and SSH-backed PTY events on B must fan out to A.
    Remote `SIGINT` / `SIGTSTP` map to PTY control bytes instead of silently
    succeeding without affecting the process.

## Related account-login guards

Incomplete login (cloud vs local settings choice) must not persist a session
until `account_finalize_login`. See comments on
`PENDING_SYNC_CHOICE` in `src/apps/desktop/src/api/remote_connect_api.rs`.
