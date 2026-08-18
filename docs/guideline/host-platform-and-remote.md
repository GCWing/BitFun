# Host platform, Tauri, and remote workspace

> Companion to the root `AGENTS.md` entry (STD-05 / STD-06 related host rules).
> Open this when changing desktop commands, UI↔host boundaries, remote scenarios,
> or upgrade compatibility.
>
> [中文](host-platform-and-remote.zh-CN.md)

## Tauri commands

- Command names: `snake_case`.
- TypeScript may wrap with `camelCase`, but invoke Rust with a structured `request`:

```rust
#[tauri::command]
pub async fn your_command(
    state: State<'_, AppState>,
    request: YourRequest,
) -> Result<YourResponse, String>
```

```ts
await api.invoke('your_command', { request: { ... } });
```

Also follow [`src/apps/desktop/AGENTS.md`](../../src/apps/desktop/AGENTS.md) for desktop host scope.

## Platform boundaries

- Do not call Tauri APIs directly from UI components; go through the adapter/infrastructure layer.
- Desktop-only host adapters belong in `src/apps/desktop`, then flow through typed capability interfaces and, when event delivery is needed, the production transport adapter.
- In shared core, avoid host-specific APIs such as `tauri::AppHandle`; use shared abstractions such as `bitfun_events::EventEmitter`.

## Remote scenarios

BitFun is not a local-only desktop app. The workspace, the runtime that executes
a turn, and the person driving it can each sit on a different machine. Treat the
four scenarios below as first-class targets of every change, not as a later port.

| Scenario | What it means | Design entry point |
|---|---|---|
| Remote workspace | The active workspace lives on an SSH host, a jump-host chain, or a Docker container; files, terminal, search, and Agent subprocesses must execute there | [remote-workspace-transport.md](../architecture/remote-workspace-transport.md), [remote-workspaces.md](../specs/remote-workspaces.md) |
| Remote control | Mobile web, or a Feishu / Telegram / WeChat bot, drives a session on a Desktop or CLI host through the Remote Connect relay | [`src/mobile-web`](../../src/mobile-web/AGENTS.md), `remote_connect` in [services-integrations](../../src/crates/services/services-integrations/AGENTS.md), [relay-service](../../src/crates/services/relay-service/AGENTS.md) |
| Peer Device Mode | One same-account device becomes the data plane of another: the controller shell stays local, invokes and events come from the peer | [peer-device-mode.md](../architecture/peer-device-mode.md), [peer-device README](../../src/web-ui/src/infrastructure/peer-device/README.md) |
| Detached Dispatch | A controller submits a durable job to another BitFun host and may then disconnect; the target owns the job, session, worktree, event log, and permission mailbox | [detached-task-dispatch.md](../architecture/detached-task-dispatch.md) |

Rules that apply to all four:

- Design the remote path together with the feature. A capability that assumes UI,
  process, and filesystem share one machine is incomplete, not "phase one".
- Degrade loudly. When a scenario cannot be supported, gate the entry point or
  return a clear unsupported state. Silent local fallback, fake success, empty
  payloads, and generic errors are all regressions; local fallback additionally
  leaks local content to a remote controller.
- Keep blocking interaction answerable from a distance. New permission prompts,
  dialogs, and pickers must reach the driving surface through the existing dialog
  and permission-mailbox orchestration. A turn that only the desktop window can
  unblock deadlocks remote control and dispatch jobs.
- Survive disconnect. Remote surfaces reconnect, replay by cursor, and re-hydrate,
  so prefer resumable cursors and idempotent mutations over state that exists only
  while a client happens to be attached.
- Remote workspace paths are POSIX on every client OS. Do not split or join them
  with host `std::path` semantics, and do not reuse a controller-side path on a
  peer host.

Per-scenario obligations:

- **Remote workspace**: every desktop Tauri command declares its policy in
  [`remote_workspace_policy.rs`](../../src/apps/desktop/src/api/remote_workspace_policy.rs).
  The contract test there rejects new commands without an explicit policy and
  forbids growing the `LegacyUnaudited` backlog.
- **Remote control**: mobile web and IM bots reach sessions through the
  `RemoteCommand` wire protocol and the bot command router / menu, not through the
  Web UI. When a session-level capability is added or moved — workspace or
  assistant selection, session lifecycle, mode, model, approval, attachment —
  extend those surfaces or make them answer with an explicit unsupported reply.
- **Peer Device Mode**: product commands are proxied to the peer by default. A
  command that must stay on the controller (window chrome, updater, account
  identity, local OS automation) has to be denied in all three lists that are kept
  in sync: [`peer_host_invoke.rs`](../../src/apps/desktop/src/api/peer_host_invoke.rs),
  [`deny.rs`](../../src/apps/cli/src/peer_host/deny.rs), and
  [`peer-device-adapter.ts`](../../src/web-ui/src/infrastructure/api/adapters/peer-device-adapter.ts).
  Read the peer-device README invariants before changing session, account, or
  hydrate paths.
- **Detached Dispatch**: jobs run headless on the target under the CLI delivery
  profile, with no interactive host and no guaranteed controller connection. The
  controller is an observer, never a runtime or filesystem proxy. Do not add
  behavior that requires a live submitter, and treat the dispatch protocol version
  and required target capabilities as a compatibility contract — a new target-side
  requirement needs a negotiated capability, not an assumption.

State which remote scenarios a change was exercised in. Local-only tests are not
evidence of remote behavior.

## Upgrade compatibility

Users upgrade in place, and the remote scenarios above routinely put two
different BitFun versions on the same connection. Every change must keep
existing installs working without manual repair.

- **Persisted shapes are read by older and newer code.** Config, settings,
  sessions, connection profiles, worktree and dispatch records: add fields with
  defaults, keep deserialization tolerant, and never repurpose or narrow the
  meaning of a field that is already on disk. A field old data cannot supply
  must not become required.
- **Never delete or reset user data to recover from something you cannot
  parse.** Keep the record, degrade the feature, and surface a clear state.
  Missing credentials, an unreadable profile, a timeout, or an offline host are
  not reasons to drop a session, workspace, or connection. Destructive removal
  stays an explicit user action.
- **Cross-version boundaries negotiate; they do not assume.** Peer HostInvoke,
  the dispatch protocol, relay and mobile web, and IM bots all talk to a build
  you do not control. Advertise a capability and check it before using it —
  package version equality is not evidence of behavior — and keep the older
  side on a working path instead of failing it.
- **A rename is a migration.** Keep reading the old name, id, or record shape
  until no supported peer can still send it, and migrate referenced data
  (vault entries, workspace pointers) together with the thing being renamed.
- **Prove it with tests.** Cover legacy deserialization and an old-payload
  round trip, not just the new shape. A test that only exercises data written
  by the current code is not upgrade coverage.
