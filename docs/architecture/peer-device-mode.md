# Peer Device Mode

Peer Device Mode switches the desktop (and mobile control target) data plane
onto another same-account online BitFun device. The React shell stays local;
product invokes and agentic events come from the peer.

## Boundaries

- Not SSH `WorkspaceKind.Remote` (local session mirror + remote FS).
- Enter via Account Login → Online Devices → click peer.
- Exit via sidebar footer `Remote: {device_name}` disconnect.
- Local-only commands (window chrome, updater, account login/logout, peer
  control plane) never execute on the peer on behalf of a controller.
- Unsupported or denied commands fail loudly; they must not fall back to the
  local host (that would leak local content).

## Transport

- Controller: `PeerDeviceTransportAdapter` wraps product `invoke` as
  `RemoteCommand::HostInvoke` over `account_device_rpc`.
- Peer: decrypt → allow/deny → webview bridge `peer-host-invoke://request` →
  same Tauri handlers as local UI → `peer_host_invoke_complete`.
- Events: peer agentic projection fan-out as `RemoteCommand::DeviceEvent` to
  attached controllers; controller re-emits the same event names locally.

## Ownership

- Desktop host invoke / fan-out: `src/apps/desktop/src/api/peer_host_invoke.rs`,
  `remote_connect_api.rs`
- Frontend mode + transport: `src/web-ui/src/infrastructure/peer-device/`,
  `adapters/peer-device-adapter.ts`
