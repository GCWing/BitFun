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
- The closed operation budget is Health, Session list/create/restore (including transcript), Turn submit/cancel, pending/respond Permission,
  and UserInput answers. Disconnect cleanup is internal lifecycle, not a detach operation. Do not add delete, fork, replay, observer,
  controller transfer, Tool/MCP/Hook management, or product configuration incidentally.
- Stable Event, Product Domain, and Runtime Port DTOs may be reused. Do not
  depend on `bitfun-core`, Agent Runtime implementations, SDK Host, services,
  Tauri, terminal, tool runtime, or remote transports.
- Use only Windows Named Pipes or Unix Domain Sockets. Do not add TCP, HTTP,
  WebSocket, browser access, or remote fallback.
- Treat this as same-user local isolation, not a sandbox. Product composition
  must supply a user-private runtime directory.

## Verification

```bash
cargo test -p bitfun-agent-runtime-ipc
node scripts/check-core-boundaries.mjs
```
