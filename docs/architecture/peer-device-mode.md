# Peer Device Mode

Peer Device Mode switches the desktop (and mobile control target) data plane
onto another same-account online BitFun device. The React shell stays local;
product invokes and agentic events come from the peer. The peer may be Desktop
or CLI: both speak the same HostInvoke / DeviceEvent protocol.

## Product goal

After login, clicking an online peer device **B** from controller **A** must make
A's workspace list, sessions, assistants, chat, and tools behave like using
BitFun on B's machine. The authority is **B's live local BitFun state** via
HostInvoke / DeviceEvent fan-out — not a merged cloud session history.

## Cloud account sync vs Peer Remote

| Concern | Account cloud sync | Peer Device Mode |
|---|---|---|
| Purpose | Settings preference sync; optional session **backup upload** | Live full-client remote on another device |
| Session list on A | Local disk only (cloud sessions are **not** imported) | Peer's live session store via HostInvoke |
| Settings | May pull/apply cloud settings to this device | Reloaded from peer after enter (via peer transport) |
| Offline peer | N/A | Must exit Peer Mode; UI must not keep a stale Remote label |

Do **not** treat cloud session blobs as the Remote data plane. Do **not** merge
cloud session metadata into local disk on login or periodic pull — that pollutes
A and conflicts with Peer Mode.

Settings sync is continuous on every logged-in host (Desktop, interactive CLI,
and the CLI daemon): local changes upload after a ~5s debounce (content-hash
deduped); cloud changes are pulled at process start and then every ~30s. After
applying or uploading settings, a host fans out `account://settings-applied`
to attached controllers; the controller re-emits it locally so the frontend
config cache and model selectors refresh without reconnecting.

SSH `WorkspaceKind.Remote` remains a separate path (local session mirror + remote
FS) and must not be mixed with Peer Device Mode.

## Boundaries

- Not SSH `WorkspaceKind.Remote` (local session mirror + remote FS).
- Enter via Account Login → Online Devices → click peer.
- Exit via sidebar Peer Remote status row `Disconnect` (device name + disconnect).
- Local-only commands (window chrome, updater, account login/logout, peer
  control plane) never execute on the peer on behalf of a controller.
- Unsupported or denied commands fail loudly; they must not fall back to the
  local host (that would leak local content).

## Transport

- Controller: `PeerDeviceTransportAdapter` wraps product `invoke` as
  `RemoteCommand::HostInvoke` over `account_device_rpc`.
- HostInvoke on the controller is **priority-queued** (effectively unbounded
  concurrency, `i32::MAX` in flight). Session restore / session-list / dialog /
  workspace-startup commands outrank background `git_*` / `ssh_*` / `lsp_*` /
  `search_*` / FS / canvas / editor RPCs so hydrate is not starved into relay
  HTTP 504s. Terminal commands are always interactive priority, and one slot is
  kept free from low-priority background work so input cannot be trapped behind
  slow polling requests.
- While Peer Mode is active, background noise is reduced further:
  - controller-local SSH heartbeats and remote-workspace auto-reconnect pause
  - Git / FilesPanel window-focus refresh pauses
  - editor disk sync poll slows to 15s (from 1s)
  - canvas snapshot poll slows to 15s (from 2s)
  - workspace search-index poll slows to 30s idle / 5s active
- Peer: decrypt → allow/deny → execute on the peer host:
  - Desktop: webview bridge `peer-host-invoke://request` → same Tauri handlers
    as local UI → `peer_host_invoke_complete`
  - CLI: the invocation-scoped CLI product runtime handles dialog submit/cancel
    through the Agent Runtime SDK and session/snapshot gaps through one Core
    compatibility facade — no webview and no second scheduler, persistence
    manager, or event queue. Desktop-only surfaces (MiniApp / cron / ACP list)
    return empty or no-op so hydrate does not fail.
- Events: peer agentic projection (and other product events such as terminal /
  FS / MCP interaction) fan-out as `RemoteCommand::DeviceEvent` to attached
  controllers; controller re-emits the same event names locally. This includes
  SSH-backed remote PTY Ready / Data / Exit events created on B, not only B's
  local terminal service events.
- Because DeviceEvent delivery has no ACK/replay contract, the controller also
  reconciles its active chat session from the Peer Host every 3s, immediately
  after session/visibility changes, and after detecting a dropped data event.
  Realtime events remain the primary path; snapshot reconciliation repairs a
  controller that attached after turn/round lifecycle events or crossed a
  transient relay gap. The host overlays its authoritative in-memory session
  state onto the persisted view so an executing turn is not misclassified as
  interrupted history.
- CLI Peer Host forwards only turns submitted through Peer Host and linked
  child turns. A background-result follow-up inherits ownership only when its
  Core-internal metadata identifies the exact tracked parent and source child
  turns; if an unrelated turn is running in the same session, the result queues
  behind it without losing Peer ownership. Completed source lineage uses a
  bounded, one-shot tombstone while delivery waits on session serialization;
  session drain or event-stream interruption clears it. Peer Host
  requires an attached controller before submit and binds tool confirmation to
  the exact observed tool and turn. Confirmable Peer tools always wait for the
  controller even when the host's global policy skips confirmation, so an Agent
  pauses until the controller responds; exact background-result follow-ups
  retain this Peer-only confirmation requirement. The host cancels tracked turns when the
  last controller detaches/goes offline or the agent-event subscription
  lags/closes; continuity loss also projects the existing dialog-turn-failed
  terminal event. Terminal ownership remains tracked until the event reaches the
  delivery attempt, and a closed local delivery queue uses the same direct
  DeviceEvent path. Delivery targets are captured when an event is queued and
  rechecked against the currently attached set before each send. A per-target
  delivery lease serializes detach or offline removal with the local Relay
  enqueue attempt. An explicit disconnect still restores the local controller
  UI, but reports a warning when host cancellation was not confirmed. This
  boundary does not change the Relay envelope or add ACK or replay.
- Relay `POST /api/devices/:id/rpc` waits up to **120s** for the peer response;
  reverse proxies in front of the relay must use a matching (or higher) read
  timeout or they will return 504 first.

## Workspace directory picking

Native `@tauri-apps/plugin-dialog` always opens on the **controller** machine.
In Peer Device Mode that would pick a path on A and then send it to B via
`open_workspace` / `create_directory` — wrong semantics.

Peer Mode therefore uses an in-app directory browser on A that lists B's
filesystem through HostInvoke (`get_directory_children`, etc.). Entry points
call `pickWorkspaceDirectory()`:

- Local mode → native plugin-dialog
- Peer Mode → `PeerDirectoryBrowser` via `peerDirectoryPickerStore`

Still use normal `openWorkspace` / create-workspace flows (not SSH
`openRemoteWorkspace` / `WorkspaceKind.Remote`).

## File download ownership

The native save/folder dialog always selects a destination on controller A,
while the workspace source belongs to peer B. A download is therefore a
split-endpoint operation: B returns file bytes through the existing
`GetFileInfo` / `ReadFileChunk` protocol and A writes those chunks through its
local filesystem adapter. Directory downloads enumerate B recursively and
create the corresponding tree on A. Never forward A's selected destination to
B through `export_local_file_to_path`; paths and permissions are host-specific
and may represent a different operating system.

## Ownership

- Desktop host invoke / fan-out: `src/apps/desktop/src/api/peer_host_invoke.rs`,
  `remote_connect_api.rs`
- CLI host invoke / fan-out: `src/apps/cli/src/peer_host/` (Core registry; no
  webview bridge). Device routing in `src/apps/cli/src/account.rs` special-cases
  `HostInvoke` / `DeviceEvent`. Same machine Desktop+CLI share one `device_id`;
  last `AuthConnect` wins.
- Shared account settings sync engine:
  `src/crates/assembly/core/src/service/remote_connect/settings_sync.rs`
  (debounced push, 30s pull, persisted cursor); app wiring in
  `src/apps/desktop/src/api/remote_connect_api.rs` and
  `src/apps/cli/src/account_sync.rs`.
- Frontend mode + transport: `src/web-ui/src/infrastructure/peer-device/`,
  `adapters/peer-device-adapter.ts`
- Peer directory picker: `pickWorkspaceDirectory.ts`, `PeerDirectoryBrowser.tsx`,
  `PeerDirectoryPickerHost.tsx`

## Regression guards (read before changing session/account paths)

Frontend invariants and known failure modes:
[`src/web-ui/src/infrastructure/peer-device/README.md`](../../src/web-ui/src/infrastructure/peer-device/README.md).

Especially: Peer Mode must not call fail-closed `account_fetch_session_turns`
during hydrate; clear stale `currentWorkspacePath` on peer switch; pass live
workspace into `create_session`; keep config HostInvokes high-priority.
