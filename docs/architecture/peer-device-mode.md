# Peer Device Mode

Peer Device Mode switches the desktop (and mobile control target) data plane
onto another same-account online BitFun device. The React shell stays local;
product invokes and agentic events come from the peer.

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
- HostInvoke on the controller is **priority-queued** (max 2 in flight). Session
  restore / session-list commands outrank background `git_*` / `ssh_*` /
  editor RPCs so hydrate is not starved into relay HTTP 504s.
- While Peer Mode is active, controller-local SSH heartbeats and remote-workspace
  auto-reconnect are paused so they cannot flood the peer HostInvoke path.
- Peer: decrypt → allow/deny → webview bridge `peer-host-invoke://request` →
  same Tauri handlers as local UI → `peer_host_invoke_complete`.
- Events: peer agentic projection (and other product events such as terminal /
  FS / MCP interaction) fan-out as `RemoteCommand::DeviceEvent` to attached
  controllers; controller re-emits the same event names locally.
- Relay `POST /api/devices/:id/rpc` waits up to **120s** for the peer response;
  reverse proxies in front of the relay must use a matching (or higher) read
  timeout or they will return 504 first.

## Ownership

- Desktop host invoke / fan-out: `src/apps/desktop/src/api/peer_host_invoke.rs`,
  `remote_connect_api.rs`
- Frontend mode + transport: `src/web-ui/src/infrastructure/peer-device/`,
  `adapters/peer-device-adapter.ts`
