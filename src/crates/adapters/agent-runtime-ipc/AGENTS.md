[中文](AGENTS-CN.md) | **English**

# Agent Runtime IPC

Scope: `src/crates/adapters/agent-runtime-ipc`.

This non-published crate is the private local protocol used by the first-party Shared TUI adapter.
It provides discovery, one-instance locking, bounded framing, authenticated initialization, a closed interactive operation set,
session controller leases, event delivery, connection bounds, and cleanup. It is not a public SDK, remote protocol, service layer, or Runtime owner.

## Pre-integration contract

- Only consumer: the first-party interactive TUI adapter in `src/apps/cli`.
  GUI, Remote, Peer, ACP, Headless CLI, and SDK Host are not implied consumers.
- Stable test contract: platform-local endpoint, strict initialize-first handshake, separate handshake/request deadlines,
  128 KiB request and 8 MiB response/event limits, bounded connections, one controller per Session, one active Turn per connection,
  disconnect cancellation, sticky event-stream invalidation, 30-second idle exit, and owner-checked discovery cleanup.
- Integration check: the consumer must reuse existing Agent Runtime owners and
  prove Embedded/Shared behavior equivalence without depending on SDK Host.

## Boundaries

- Export only the exact workspace-private API needed by the CLI adapter. Do not
  publish this crate or expose its wire as an SDK contract.
- The closed operation budget is Health, Session list/create/restore/delete/fork (including transcript on restore/fork), current-Session rename, Agent mode/model update, manual context compaction, and Session undo/redo,
  declarative context reload, Turn submit/cancel, pending/respond Permission, and UserInput answers. Delete is limited to an idle Session not controlled by any client.
  Fork is a current-controller, idle-only operation. It either copies through the latest persisted Turn or stops immediately before an explicitly selected Turn. The encoded success result carries the authoritative new Session and transcript; only then may the server atomically switch the connection lease from the source Session to the fork.
  Manual compaction is a current-controller, idle-only Turn operation. The client supplies its exact Turn ID before admission so timeout or disconnect cleanup can cancel the same owned task; once Core begins the atomic context commit, a late cancellation does not expose a false idle state.
  Context reload may run during an active Turn, does not rewrite that Turn, and guards the cache so the next message reads invalidated instructions.
  Undo/redo is a current-controller operation that may enter during an active Turn because Core owns cancel-and-drain before mutation. Its success response carries the authoritative transcript and clears the connection's active-Turn projection. It is local-workspace only and does not expose a generic checkpoint protocol.
  Disconnect cleanup is internal lifecycle, not a detach operation.
  Model catalogs and defaults remain product configuration outside this wire. Do not add archive, replay, observer,
  general controller transfer, Tool/MCP/Hook management, or other product configuration incidentally.
- Stable Event, Product Domain, and Runtime Port DTOs may be reused. Do not
  depend on `bitfun-core`, Agent Runtime implementations, SDK Host, services,
  Tauri, terminal, tool runtime, or remote transports.
- Use only Windows Named Pipes or Unix Domain Sockets. Do not add TCP, HTTP,
  WebSocket, browser access, or remote fallback.
- Treat this as same-user local isolation, not a sandbox. Product composition
  must supply a user-private runtime directory.
- Embedded callers must continue to invoke the typed Agent Runtime directly and
  must not initialize this transport. Shared outgoing request, response, and
  event frames are encoded once before write; strict decoding, unknown-field
  rejection, frame limits, bounded queues, and backpressure must not be weakened
  for throughput.

## Verification

```bash
cargo test -p bitfun-agent-runtime-ipc
node scripts/check-core-boundaries.mjs
```
