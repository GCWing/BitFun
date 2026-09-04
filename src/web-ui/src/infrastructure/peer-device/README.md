# Peer Device Mode (frontend)

Controller-side React/transport layer for Peer Device Mode. Architecture:
[`docs/architecture/peer-device-mode.md`](../../../../../docs/architecture/peer-device-mode.md).

## Migrating to the Session Projection contract

The invariants below are pairwise rules between writers that carry no shared
position. They are being replaced by one contract —
[`docs/architecture/session-projection.md`](../../../../../docs/architecture/session-projection.md)
— under which a write is admitted by its position in the order rather than by
what it would do to painted content. **This list shrinking is the measure of
that migration**; a change that adds a rule here is going the wrong way.

Already owned by the contract (`flow_chat/session-stream/`):

- The stream position, the delivery gap, and the attach fence in invariant 12.
  `runtimeSessionEventGate` is now an adapter over `SessionStream`, not a
  second owner of that state.
- Surface-scoped Session identity in invariant 0: a stream is keyed by
  `(DeviceSurfaceId, SessionId)` by construction.

Also owned: which read may write a Turn. `replaceRunningSnapshot` is gone —
the persisted record (snapshot merge *and* disk hydrate) may not write the Turn
the runtime stream owns, and the Host's declared executing Turn is part of that
ownership. `snapshotDropsProjectedTurnContent` and
`isRunningSnapshotForwardProgress` survive inside `persistedReadMayReplaceTurn`
for the two gaps the contract does not yet close (a Host with no runtime
projection, and a partial history read); see the contract doc before touching
them.

Still to migrate, in order: the interaction mailbox, then history positions.

## Invariants (do not regress)

0. **A surface switch is a view change, not a teardown.** Attachments and the
   rendered surface are independent: peers stay attached (and keep running our
   work) after the UI moves elsewhere, and `switchToLocal` is a switch, not a
   disconnect. Two consequences:

   - Everything in `resetProductSurface()` must be **frontend-only**.
     `resetProductSurface` runs before the transport swap, so any backend call
     it makes lands on the device being *left*. `terminal_shutdown_all` and
     similar lifecycle calls can kill work an agent turn there is still using
     (regression: 2026-08-14 multi-device switch). Use frontend-only listener
     detachment such as `TerminalService.disconnect()`.
   - **Identity includes the device surface.** Workspace paths and session ids
     can be equal on different machines. FlowChat/workspace containers,
     state machines, processing status, pending messages, composer drafts,
     request dedup and capability caches must therefore use
     `(DeviceSurfaceId, local identity)`. `activateSurface` commits transport,
     event routing and container selection before notifying observers. A normal
     switch preserves every container; only explicit/lost attachment disposal
     may call `discardSurfaceState`.
   - **In-flight submissions must survive the switch.** `startTurn` has an
     async window between adding the projection turn and re-reading the
     session (state transition, worktree bind, model sync). Clearing the store
     inside that window made the submission resume against a missing session
     and throw `Session lost after adding dialog turn` — before
     `start_dialog_turn`, so the message reached no host at all (regression:
     2026-08-15).      `resetProductSurface` therefore awaits
     `waitForInFlightSubmissions` first. `sendMessage` and its driver carry one
     `SurfaceScope`; after every host await, a stale epoch abandons without
     writing into the newly selected container, and an unaccepted message is
     re-queued onto its original surface. Once `start_dialog_turn` has been
     invoked, the host may already own the Turn before the client sees the
     ACK — that submission must not be re-queued, and attach must drop any
     pending-queue item that duplicates a live turn's user message. Drain
     must not fire while a Runtime attach is resetting the state machine to
     IDLE. Any new await inside `startTurn`
     widens that window and must keep the same scope checkpoint.
   - **Reconciliation repairs a projection, never guts it.** The wholesale
     replace path (`replaceRunningSnapshot`) skips the forward-progress
     comparator so a settled turn can adopt the host's copy. A turn keeps its
     identity and user message independently of its rounds, so a windowed or
     not-yet-checkpointed snapshot can name the turn while carrying none of its
     work — and a first-time surface projection has no state machines, so
     *every* turn reads as idle and qualifies for replacement.
     That combination erased the whole response and left only the prompt on
     screen (regression: 2026-08-15). `snapshotDropsProjectedTurnContent` gates
     the replace. Equal item counts are not sufficient: text and thinking must
     preserve prefix progress, and completed tool results may not disappear.
     Recognized client-derived display cards are carried across the terminal
     host-tail repair instead of blocking authoritative text reconciliation.
     The refresh loop still re-attaches an executing turn when a
     snapshot is refused, or a rebuilt surface would render it as static
     history.
   - **Surface-scoped events must stay routed by source device.** Background
     attachments mean several agent streams share one event bus. The
     controller tags re-emitted peer payloads with `__openbitfunSourceDeviceId`
     and `deviceSurfaceRouting.ts` (applied inside
     `TauriTransportAdapter.listen`) drops anything not produced by the
     rendered device. Adding a fanned-out event on the Rust side means adding
     it to `SURFACE_SCOPED_EVENTS`/prefixes too, or local and peer streams will
     interleave in one store. Never route control-plane events (`account://…`)
     — they must always pass.
   - **React subscriptions include the Surface activation.** A Session id is
     not a complete subscription identity. Hooks that read per-Surface state
     machines subscribe to the Surface epoch and return no snapshot during the
     rebind render; otherwise React can pair A's old `turnId` with B's Session
     for one render, including when both devices use the same Session id.

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
   `get_agent_profile_config`). Keeping them `low` can still delay hydrate
   behind a burst of background RPCs.

7. **Account identity commands are LOCAL_ONLY** and must stay denied on the
   peer host (`account_login`, `account_finalize_login`, logout, device RPC,
   …). The FE adapter, desktop `peer_host_invoke`, and CLI `peer_host` all
   derive that set from one registry row (`peer: ControllerLocal` in
   `src/crates/contracts/product-domains/src/remote_surface/table.rs`); the
   FE set is the generated `PEER_CONTROLLER_LOCAL_COMMANDS`. Do not add a
   hand-written list on any surface. See
   `docs/architecture/remote-surface-contract.md`.

8. **`relay_deploy_*` is LOCAL_ONLY.** One-click deploy SSHes from the
   controller to a user-owned host; do not HostInvoke it onto the peer.

9. **Select workspace state atomically with transport.** Before commit,
   `workspaceManager.clearForPeerModeSwitch()` invalidates work still in flight
   but deliberately preserves the device being left. `activateSurface` then
   selects the target's cached workspace container in the same synchronous
   commit that swaps transport, before the peer-mode event. SessionModule must
   never observe A's path with B's transport. Never pass `{}` to
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

12. **Active chat attaches to a Runtime-owned Turn projection.** DeviceEvent is
    the low-latency path, not the owner of current-Turn state. Desktop and CLI
    Peer Hosts materialize eligible current Turns after their ordered delivery
    boundary and expose them
    from `restore_session_view` as `runtimeEventSnapshot` with a per-Session
    cursor and Runtime-process `streamId`. While restore is in flight,
    `runtimeSessionEventGate` queues live events by
    `(DeviceSurfaceId, SessionId)`; replay starts from an empty active-Turn base,
    then the gate drops cursor-covered events and releases newer events in
    order. Never compare cursors across different `streamId` values. This is
    gated on
    `isSurfaceReconcileEnabled()`, **not** on Peer Mode: once a window has
    switched surface, a turn left running on the local device also needs the
    same attach, because its live events were dropped by surface routing while
    another device was rendered. Attach is requested as soon as a Session on
    this surface has a usable live projection: `historyState === 'ready'`
    after disk hydrate, or `historyState === 'new'` for a session created in
    this window (those never become `ready` via hydrate). The gate is per
    `(DeviceSurfaceId, SessionId)`, not per the focused tab — a dropped-event
    refresh must still attach a background session that kept running here.
    The 3s loop is only a liveness retry and an older-Host fallback. The Peer Host must
    overlay its live in-memory session state on the persisted view; otherwise
    an in-progress turn is normalized as interrupted history and later chunks
    are dropped by the controller state machine. Surface epoch checks reject a
    restore from a device no longer rendered. Older hosts may omit the Runtime
    projection; their persisted snapshot must still never overwrite newer live
    content.

    **Delivery is not acceptance, and persist is not the current Turn.** A
    live event that the state machine drops still advances the gate cursor.
    Mark that projection stale so the next attach replays the journal instead
    of treating the cursor as current and leaving in-progress tool cards
    frozen. `finish()` may cover live events only after replay (or an
    equivalent apply) proves the painted tools/text have caught up with the
    Host journal — a matching cursor is not that proof. Overlapping attach
    transfers the fence instead of draining it onto a state machine that is
    about to reset. A 3s `staleOnly` tick, a hidden document, or a
    `FINISHING` machine must not skip repair while `hasGap` is set; TextChunk
    heartbeats are not evidence that a dropped ToolEnd was applied.
    `loadSessionHistory` and `refreshPeerSessionSnapshot` both read
    `runtimeEventSnapshot`: the persisted checkpoint of an executing Turn is
    only identity, never the painted rounds. A session-object identity change
    during restore must still return the journal — hiding it used to abort
    attach and freeze the receiver while the Host kept streaming.
    A CLI Peer Host still filters Host-local turns with `owns()`; that is a
    CLI Host limitation, not a reason for a Desktop receiver to freeze.

    **Terminal delivery is not the durability fence.** The Host may publish
    `DialogTurnCompleted` before its complete generation journal has been
    committed. After the commit it publishes `SessionHistoryChanged`; local and
    Peer surfaces then reconcile the terminal tail from that Host. The
    controller must not echo a shorter painted prefix over the settled record,
    even when the prefix has the same round and item counts.

    **The subscription and the attach loop must never be able to disable each
    other.** The agentic subscription is this window's only live view of a
    running Turn, and a surface switch tears it down. Rebuilding it used to be
    a side effect of `FlowChatManager.initialize()`, which a newer switch is
    allowed to supersede — so a rapid switch could leave the window with no
    subscription and nothing to retry. The attach loop then *refused to run
    while the subscription was down*, disabling the only path that could repair
    it, and the chat froze permanently with no live output and no snapshot
    repair (regression: 2026-08-16). `FlowChatManager` therefore re-arms on
    `onSurfaceActivated` and retries a failed start on its own, the attach loop
    treats a dead subscription as a reason to reconcile **and** re-arm rather
    than to bail, and callers of `initialize()` must not report a superseded
    bootstrap as a product failure. Any new gate on subscription readiness has
    to keep both halves independently recoverable.
    **Controller presence is not Turn ownership.** A controller lease gates
    submission and interaction responses, but once a Peer Host accepts a Turn,
    the Host keeps executing and materializing it through a zero-controller
    device-switch interval. Detach/presence loss must not cancel that Turn;
    only an actual host event-stream continuity failure may fail it closed.
    **Blocking interactions are owner mailboxes, not one-shot UI events.** The
    Runtime retains native `AskUserQuestion` and interactive permission
    requests until answer/cancel/drop, and `restore_session_view` returns their
    additive, revisioned `interactionSnapshot` from both Desktop and CLI Peer
    Hosts. Keep its frontend projection per Surface, fence it with the captured
    Surface epoch and newer event state, replay it after the Turn projection,
    and use it only to reconstruct UI in the owning Turn/round. Reattachment
    must never restart or cancel the
    running Session. Older peers may omit the field; absence is not an empty
    authoritative mailbox. Answering an `AskUserQuestion` is separately gated
    by `peer_mode_ping.capabilities.user_question_response`: legacy Desktop
    hosts already support the command, while legacy CLI hosts must show an
    explicit unsupported/upgrade state. Current controllers include the owning
    Session id with the mutation and hosts reject stale cross-Session answers;
    newer hosts still accept the legacy Tool-id-only form. Any new
    interaction that can suspend execution is incomplete until its owner
    exposes equivalent replayable attach state and a negotiated response path.

13. **Weak links use bounded, idempotency-aware recovery.** Default Peer
    HostInvoke concurrency is four with one slot reserved from normal/low
    traffic. Read-only commands have a real 10s deadline and four
    exponential-backoff retries. Mutations have a 30s deadline and are never
    replayed automatically without an idempotency contract. Dialog submission
    is the explicit exception: `start_dialog_turn` and
    `start_acp_dialog_turn` reuse `(sessionId, turnId)`, and the host
    coalesces/caches duplicate execution attempts. The controller must observe
    the matching `idempotent_dialog_submit` capability in `peer_mode_ping`
    before replaying either command; an older host stays single-shot. A failed
    session list must leave its loading state and offer an explicit retry.

14. **Catalog-backed history stays windowed across the peer boundary.**
    `restore_session_view` returns the compact `turnCatalog` plus the restored
    tail; the controller must not follow it with an unconditional full restore.
    `load_session_turn_window` is a high-priority, retryable read and carries
    the same session/workspace scope as restore. Sequential history scrolling
    and turn-rail navigation request bounded windows. Search and older Hosts
    that reject the window command use the shared explicit full-history ensure
    fallback. Targeted rollback is separately capability-gated and never falls
    back to a controller-local or numeric rollback path. Never include catalog
    preview text in Peer request/response logs.

15. **Git ownership trust is read on the peer, granted at the machine.**
    `git_get_repository_trust` is a read-only probe and routes to the peer
    host. `git_trust_repository` writes the peer user's global Git
    configuration (`safe.directory`) and tells Git to run hooks from a tree
    they do not own, so it is denied on both the desktop and CLI peer hosts.
    Its registry stance is `OperatorOnly`: the generated FE set deliberately
    omits it, because running it on the controller would write an exception
    for a path that only exists on the peer. A controller forwards it, receives
    the explicit refusal, and surfaces the probe's `manualCommand` instead.

16. **ProductControl commands follow the product host; presentation ACKs stay
    with the window.** `product_control_invoke` is a normal product mutation and
    routes to the selected peer only after `peer_mode_ping` advertises
    `product_control_v1`; an older peer fails explicitly and never falls back
    to the controller. Definitions that need a native provider or a live UI
    additionally declare `product_control_native_v1` or
    `product_control_presentation_v1`; the CLI host advertises neither and
    returns a typed unsupported result. `mark_openbitfun_control_surface_ready`,
    `mark_openbitfun_control_surface_unready`, and `report_openbitfun_control_result`
    describe or acknowledge the controller window's live Web UI and therefore
    remain `LOCAL_ONLY` in the frontend, Desktop host, and CLI host lists. A
    peer executes the same owner handler and uses its own attached presentation
    surface when a required runtime effect needs acknowledgement; an
    unavailable surface fails explicitly and never mutates the controller as a
    fallback.

17. **MiniApp Agent context files require an explicit peer capability.**
    `miniapp_agent_run` remains compatible with older peers when no context
    files are present. A run with non-empty `contextFiles` routes only after
    `peer_mode_ping` advertises `miniapp_agent_context_files_v1`; otherwise the
    controller fails before RPC. Never omit the files, fall back to a local
    Agent, or run the prompt without its declared context.

## Related account-login guards

Incomplete login (cloud vs local settings choice) must not persist a session
until `account_finalize_login`. See comments on
`PENDING_SYNC_CHOICE` in `src/apps/desktop/src/api/remote_connect_api.rs`.
