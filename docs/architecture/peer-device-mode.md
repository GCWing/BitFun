# Peer Device Mode

Peer Device Mode switches the desktop (and mobile control target) data plane
onto another same-account online OpenBitFun device. The React shell stays local;
product invokes and agentic events come from the peer. The peer may be Desktop
or CLI: both speak the same HostInvoke / DeviceEvent protocol.

## Product goal

After login, clicking an online peer device **B** from controller **A** must make
A's workspace list, sessions, assistants, chat, and tools behave like using
OpenBitFun on B's machine. The authority is **B's live local OpenBitFun state** via
HostInvoke / DeviceEvent fan-out — not a merged cloud session history.

## Attachment vs rendered surface

Two concepts, deliberately independent:

| | Attachment | Rendered surface |
|---|---|---|
| What it is | A live control link to a peer | The one device this window draws |
| How many | Any number, concurrently | Exactly one |
| Ends when | Explicit disconnect, peer offline, logout | Replaced by the next switch |
| Effect on the peer's agent | Keeps it running and fanning out | None |

This split is what makes several devices usable at once: dispatch a turn on B,
switch the UI back to A, dispatch another turn on A, and both keep running.
The frontend entry points are `switchToDevice` / `switchToLocal` /
`disconnectDevice` on `PeerDeviceContext`; the sidebar `DeviceSurfaceSwitcher`
lists this machine plus every online peer.

Two rules follow, and both are load-bearing:

- **A surface switch never mutates the device being left.** Everything in
  `resetProductSurface()` is frontend-only. Sending `terminal_shutdown_all`
  during a switch lands on the *previous* transport and
  kills work an agent there still depends on.
- **Product events are routed by their source device.** The controller re-emits
  peer DeviceEvents under their original event name, so with peers attached in
  the background one bus carries several agent streams. The desktop controller
  tags each re-emitted payload with `__openbitfunSourceDeviceId`
  (`remote_connect_api::PEER_EVENT_SOURCE_KEY`; non-object payloads are wrapped
  under `__openbitfunSourcePayload`), and `deviceSurfaceRouting.ts` — applied inside
  `TauriTransportAdapter.listen` — delivers a surface-scoped event only when its
  producing device is the rendered one. Untagged events are local by definition.
  Control-plane events (`account://…`, window chrome, updater) are never scoped
  and always pass.

### Surface identity and activation

The rendered device is a first-class `DeviceSurfaceId` (`local` or a peer
device id), not an implicit property of one mutable global transport. Cache,
request, capability, workspace, session-state-machine, processing-status,
pending-message, and composer-draft identity includes that surface. FlowChat
and workspace state are stored in per-surface containers: switching selects a
container immediately, then reconciles it with its host; it does not erase the
container belonging to the device being left.

Every surface activation creates a monotonic epoch and `AbortSignal`.
Product invokes capture that epoch, including through `ApiClient`; a response
or retry that outlives it raises `SurfaceChangedError` and is abandoned as
control flow. Controller-plane commands are exempt because their authority
remains the controller regardless of the rendered surface. Transport/event
routing and container selection commit synchronously in `activateSurface` so
no observer can see B's state while requests still target A.

`PeerDeviceSurfaceController` serializes activation outside React. Rapid
requests coalesce to the last target, a committed-but-superseded hydrate is
invalidated before the next target proceeds, and a real activation failure
rolls back to the previously rendered reachable surface. Separately,
`PeerConnectionManager` owns each attachment's
`connecting`/`ready`/`degraded`/`lost` lifecycle, keepalive and bounded backoff;
React only subscribes to snapshots. Attachment disposal is the only operation
that discards a peer's cached surface state.

Because the local surface can now miss its own events while another device is
rendered, Session attachment is no longer Peer-only. After this window's first
surface switch, `isSurfaceReconcileEnabled()` attaches whichever surface is
rendered, local included.

### Running-Turn attachment

The live WebView/DeviceEvent broadcast is a low-latency delivery path, not the
owner of a running Turn. Desktop and CLI Peer Runtime Hosts keep a materialized
projection of each eligible current Turn even when no client is subscribed. Events enter
that projection after the host's ordering/coalescing boundary and receive a
per-Session monotonic cursor plus a Runtime-process `streamId`. Text and
thinking chunks are materialized without collapsing segments across tool
boundaries; noisy tool progress is compacted.

`restore_session_view` returns this additive `runtimeEventSnapshot`; the CLI
Peer Host applies its existing Peer-owned-Turn filter before recording or
returning the projection. The persisted Session record of an executing Turn is
a lagging checkpoint, not the live projection: `loadSessionHistory` and
`refreshPeerSessionSnapshot` must not paint that checkpoint's in-progress
tool rows. A restore that races a live store update must still return the
journal so attach can replay. Delivery of a live event to a product listener
is not acceptance — a dropped ToolEvent / TextChunk marks the projection
stale so the next attach replays instead of treating the cursor as current.
`finish()` covers those cursors only after the painted projection has caught
up with journal terminal tools; a matching cursor alone is not enough.
Overlapping attach transfers the in-flight fence rather than delivering it
onto a state machine that is about to reset. Hidden-document and
fresh-TextChunk liveness skips apply only to the 3s poll, not to a dirty
projection. During an attach, the frontend fences live events for
`(DeviceSurfaceId, SessionId)`,
replays the snapshot into an empty current-Turn projection, and then releases
only events newer than the snapshot cursor. A different `streamId` is a new
Runtime process and its cursors are never compared with the old stream. The
Surface epoch rejects a response from a device that is no longer rendered.
This makes attach independent of client-written intermediate checkpoints and
closes the snapshot/live race without restarting, cancelling, or moving the
Turn. Older Hosts may omit the field and use the persisted-snapshot fallback.
Controller presence is an admission boundary, not the lifetime owner: after a
Peer Host accepts a Turn, that Host continues executing and materializing it
while zero controllers are attached. A later controller attaches to the same
Runtime projection; controller loss alone must not cancel or interrupt the
Turn. Actual host event-stream loss remains a fail-closed continuity error.

### Blocking-interaction reattachment

A push event is a notification, not the owner of an interaction that can block
an Agent turn. The owning Runtime keeps every native `AskUserQuestion` and
interactive permission request in a live mailbox until it is answered or
cancelled; an `AskUserQuestion` registration is also removed if its owning Tool
future is dropped. `restore_session_view` returns an additive
`interactionSnapshot` containing the Session-filtered mailbox and monotonic
revisions. Desktop and CLI Peer Hosts expose the same field; older Hosts may
omit it and remain on the event-only compatibility path.

The frontend projects that mailbox into the active Surface container. Permission
requests are retained for inactive Surfaces by source device, while missed
`AskUserQuestion` cards are reconstructed in their exact Dialog Turn and model
round. Snapshot responses are fenced by the Surface epoch and by event/revision
ordering, so an old response cannot erase a newer request or revive one that was
already answered. Reattachment only repairs presentation state: it never
restarts, cancels, or moves the Session, Dialog Turn, or Tool future.

Rendering a mailbox entry and answering it are separate compatibility
contracts. A Peer Host that accepts `submit_user_answers` advertises
`peer_mode_ping.capabilities.user_question_response`. Older Desktop hosts are
compatible because they already exposed the command; older CLI hosts are not,
so controllers must leave the card visible but disabled with an explicit
upgrade/unsupported state instead of sending a mutation that cannot complete.
Current controllers include the owning Session id, and the host rejects an
answer when that Session no longer owns the pending Tool id. New hosts retain
the legacy process-wide Tool-id form for older controllers that omit Session id.

This is the contract for any new blocking interaction: its execution owner must
retain replayable request state and expose it through an attach/snapshot path.
A one-shot frontend event plus an unresolved channel is not a complete
multi-device implementation.

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

The sync engine subscribes to successful local mutations at `ConfigService`,
in addition to legacy host notifications. This covers model, Skill, Agent
profile, and individual preference mutations through Desktop and CLI. Failed
writes, runtime-only credentials, reloads, and cloud imports do not emit this
local-change signal. Pending local edits take priority over the periodic pull;
a fetched blob is applied only if the local document still matches its
pre-fetch snapshot. The comparison and import share the config write lock.

Older settings snapshots may omit fixed fields introduced by a newer build.
Imports preserve those local fields instead of replacing them with defaults.
Supplied arrays and dynamic maps remain authoritative, so deleted models,
profiles and list entries are not resurrected. Optional/default-elided fields
retain their existing reset semantics; an explicit raw backup restore also
honors omitted default memory and AI preferences. Legacy renamed fields still
pass through their migrations before values at the new names are preserved.

Realtime voice credentials live in `app.voice_call` in the same persisted
configuration and export/backup format as model settings. Account settings
apply preserves the controller's existing voice fields when an older payload
omits them, and an empty voice API key from an unconfigured host does not erase
a configured local key. Non-empty synced keys still replace the local key.
Explicit file imports can restore or clear a supplied key; local voice saves
and resets can also clear it. A valid whole-config import creates a raw
`app_pre-import_*.json` backup before replacement, under the existing backup
retention policy. Config reload and model-reference reconciliation serialize
their reads and writes with local saves so stale snapshots cannot undo a
completed credential save. These rules do not change speech command routing:
capture, configuration and realtime connections remain on the controller.

Config mutations publish in-memory values and change notifications only after
atomic persistence succeeds. Model CRUD and Agent/Skill map edits use a shared
read/modify/write operation; startup profile canonicalization updates only its
map. User backups have unique names even within the same second. Web UI reads
resolve legacy model metadata without writing it back, model edits read fresh
host data inside the client mutation queue, and AI-experience controls save
only edited fields. An explicit empty quick-action list stays empty across
reloads; defaults are supplied only when absent or when explicitly reset.

SSH `WorkspaceKind.Remote` remains a separate path (local session mirror + remote
FS) and must not be mixed with Peer Device Mode.

## Boundaries

- Not SSH `WorkspaceKind.Remote` (local session mirror + remote FS).
- Switch via the sidebar device switcher, or Account Login → Online Devices →
  click a device. Both list this machine, so returning to it is a switch like
  any other.
- Selecting this machine only changes what is rendered; peers stay attached and
  keep working. `Disconnect` in the switcher is the separate, explicit action
  that ends a peer's control link and discards that peer's cached Surface state
  on the controller. It does not cancel a Turn the peer has already accepted;
  reconnecting later reattaches to the Host-owned Runtime projection. Pending
  controller-only interactions still follow their owner's mailbox or fail-closed
  policy.
- Local-only commands (window chrome, updater, account login/logout, peer
  control plane) never execute on the peer on behalf of a controller. Which
  commands those are is declared once, per command, in the Product Operation
  Registry (`openbitfun_product_domains::remote_surface`); the desktop host, the
  CLI host, and the Web UI transport adapter derive their tables from it. See
  [remote-surface-contract.md](remote-surface-contract.md).
- Unsupported or denied commands fail loudly; they must not fall back to the
  local host (that would leak local content). The CLI host distinguishes
  "controller-owned", "unsupported on a CLI host (reason)", "retired", and
  "unknown to this host version" so a controller can tell a policy refusal
  from a version mismatch.

## Transport

- Controller: `PeerDeviceTransportAdapter` wraps product `invoke` as
  `RemoteCommand::HostInvoke` over `account_device_rpc`.
- HostInvoke on the controller is **priority-queued** with four requests in
  flight. Session restore / session-list / dialog / workspace-startup commands
  outrank background `git_*` / `ssh_*` / `search_*` / FS / canvas /
  editor RPCs so hydrate is not starved into relay HTTP 504s. Terminal commands
  are always interactive priority, and one slot is kept free from normal and
  low-priority work so input cannot be trapped behind slow polling requests.
- Idempotent read HostInvokes use a 10s per-attempt deadline and at most four
  exponential-backoff retries. Mutating commands use a 30s deadline and are
  not replayed unless both ends share an explicit idempotency contract.
  `start_dialog_turn` and `start_acp_dialog_turn` use their stable
  `(sessionId, turnId)` identity for bounded retry: the controller reuses the
  exact payload, while the host coalesces concurrent attempts and caches the
  completed result for the retry window. The controller enables this exception
  only when the initial `peer_mode_ping` advertises
  `idempotent_dialog_submit`, so mixed-version peers remain single-shot.
  Other mutations remain single-shot because a timed-out outcome is unknown.
  Identity-based Session rollback is sent only when `peer_mode_ping` advertises
  `targeted_session_rollback`; older peers fail explicitly and never fall back
  to controller-local files, history, or the removed numeric rollback command.
  The desktop `account_device_rpc` command enforces the requested deadline
  around the native HTTP future; the controller's Promise deadline is not
  merely a UI timer. Failed session-list loads leave the spinner and expose an
  explicit retry action.
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
- Relay DeviceEvent delivery itself has no ACK/replay contract. The active chat
  therefore attaches immediately when the selected Session becomes hydrated,
  after Surface/visibility changes, and after a detected data gap. The Peer
  Host's `runtimeEventSnapshot` plus `(streamId, cursor)` is the resumable
  current-Turn contract: live events are fenced while the snapshot is in
  flight, the materialized Turn is replayed, and only later cursors are
  released. The 3s reconciliation remains a liveness retry and an older-Host
  persisted-snapshot fallback, not the source of Turn continuity. The host
  still overlays its authoritative in-memory Session state so an executing
  Turn is not misclassified as interrupted history. Native blocking
  interactions are reconciled from the Runtime-owned `interactionSnapshot`
  after event replay, because a Turn can wait indefinitely without emitting
  another text chunk or producing a newer persisted checkpoint.
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
  retain this Peer-only confirmation requirement. The host keeps tracked Turns
  running when the last controller detaches or goes offline and continues
  materializing their Runtime projection for a later attach. Actual agent-event
  subscription lag or closure remains a continuity failure: it cancels tracked
  Turns and projects the existing dialog-turn-failed terminal event. Terminal
  ownership remains tracked until the event reaches the delivery attempt, and a
  closed local delivery queue uses the same direct DeviceEvent path. Delivery
  targets are captured when an event is queued and rechecked against the
  currently attached set before each send. A per-target delivery lease serializes
  detach or offline removal with the local Relay enqueue attempt. An explicit
  disconnect still restores the local controller UI and reports a warning when
  the host does not confirm attachment teardown; that uncertainty concerns the
  control link and controller-scoped interaction cleanup, not cancellation of
  Host-accepted work. This boundary does not change the Relay envelope or add
  ACK or replay.
- Relay `POST /api/devices/:id/rpc` still permits up to **120s** for generic
  callers. Peer controllers normally cancel earlier through their per-command
  10s/30s deadlines; reverse proxies must still accommodate any other caller
  that relies on the Relay maximum.

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

- Command policy and peer capabilities (all surfaces):
  `src/crates/contracts/product-domains/src/remote_surface/`
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
- Surface routing / switcher: `deviceSurfaceRouting.ts`,
  `deviceSurfaceReconcile.ts`, `deviceActivity.ts`,
  `DeviceSurfaceSwitcher.tsx`, `useAccountDeviceRoster.ts`
- Peer directory picker: `pickWorkspaceDirectory.ts`, `PeerDirectoryBrowser.tsx`,
  `PeerDirectoryPickerHost.tsx`

## Regression guards (read before changing session/account paths)

Frontend invariants and known failure modes:
[`src/web-ui/src/infrastructure/peer-device/README.md`](../../src/web-ui/src/infrastructure/peer-device/README.md).

Especially: Peer Mode must not call fail-closed `account_fetch_session_turns`
during hydrate; clear stale `currentWorkspacePath` on peer switch; pass live
workspace into `create_session`; keep config HostInvokes high-priority.
