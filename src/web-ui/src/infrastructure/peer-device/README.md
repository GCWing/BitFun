# Peer Device Mode (frontend)

Controller-side React/transport layer for Peer Device Mode. Architecture:
[`docs/architecture/peer-device-mode.md`](../../../../../docs/architecture/peer-device-mode.md).

## Invariants (do not regress)

0. **A surface switch is a view change, not a teardown.** Attachments and the
   rendered surface are independent: peers stay attached (and keep running our
   work) after the UI moves elsewhere, and `switchToLocal` is a switch, not a
   disconnect. Two consequences:

   - Everything in `resetProductSurface()` must be **frontend-only**.
     `resetProductSurface` runs before the transport swap, so any backend call
     it makes lands on the device being *left*. `terminal_shutdown_all` and
     `lsp_close_workspace` were exactly that bug: switching away killed the
     PTYs and language servers an agent turn there was still using
     (regression: 2026-08-14 multi-device switch). Use
     `TerminalService.disconnect()` and
     `WorkspaceLspManager.detachAllForSurfaceSwitch()`.
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
     2026-08-15). `resetProductSurface` therefore awaits
     `waitForInFlightSubmissions` first. `sendMessage` and its driver carry one
     `SurfaceScope`; after every host await, a stale epoch abandons without
     writing into the newly selected container, and an unaccepted message is
     re-queued onto its original surface. Any new await inside `startTurn`
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
     the replace; the refresh loop still re-attaches an executing turn when a
     snapshot is refused, or a rebuilt surface would render it as static
     history.
   - **Surface-scoped events must stay routed by source device.** Background
     attachments mean several agent streams share one event bus. The
     controller tags re-emitted peer payloads with `__bitfunSourceDeviceId`
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
   …). Keep FE adapter, desktop `peer_host_invoke`, and CLI `peer_host/deny`
   lists aligned.

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
    another device was rendered. Attach is requested as soon as active Session
    hydration becomes ready; the 3s loop is only a liveness retry and an
    older-Host fallback. The Peer Host must
    overlay its live in-memory session state on the persisted view; otherwise
    an in-progress turn is normalized as interrupted history and later chunks
    are dropped by the controller state machine. Surface epoch checks reject a
    restore from a device no longer rendered. Older hosts may omit the Runtime
    projection; their persisted snapshot must still never overwrite newer live
    content.

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
    authoritative mailbox. Any new interaction that can suspend execution is
    incomplete until its owner exposes equivalent replayable attach state.

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

## Related account-login guards

Incomplete login (cloud vs local settings choice) must not persist a session
until `account_finalize_login`. See comments on
`PENDING_SYNC_CHOICE` in `src/apps/desktop/src/api/remote_connect_api.rs`.
