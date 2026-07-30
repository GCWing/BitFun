[中文](AGENTS-CN.md) | **English**

# Agent Runtime IPC Migration

Scope: `src/crates/adapters/agent-runtime-ipc`.

This crate is the legacy private Shared TUI protocol. It is migration-only and
is not a target architecture boundary. The final architecture uses one App
Server wire for interactive TUI, Desktop, Electron, VS Code, and Web across
per-client and Shared deployments. Read
[`docs/architecture/app-server-architecture-design.md`](../../../../docs/architecture/app-server-architecture-design.md)
and
[`docs/architecture/agent-runtime-deployment-design.md`](../../../../docs/architecture/agent-runtime-deployment-design.md).

## Migration Rules

- Do not add operations, consumers, public exports, transports, or product
  capabilities to this protocol.
- Preserve current behavior only for compatibility and regression fixes while
  the App Server vertical slice is incomplete.
- Promote controller leases, authentication, frame bounds, disconnect
  cancellation, event invalidation, `outcome_unknown`, and cleanup into App
  Server conformance tests before removing their old implementations.
- Move only genuinely protocol-neutral Named Pipe/UDS, discovery, framing, or
  budget primitives into the App Server transport adapter. Do not carry the
  TUI-specific operation envelope or handler upward.
- Interactive TUI must migrate to `AppServerClient`: default TUI uses a
  per-client managed App Server and `--shared` connects a Shared App Server.
- Remove this crate, or reduce it to an App Server-internal transport
  implementation with no independent wire semantics, once the old consumer is
  gone.

## Temporary Safety Contract

Until deletion, do not weaken strict initialize-first authentication, request
and event size limits, bounded connections/queues, one Controller per Session,
disconnect cancellation, sticky event-stream invalidation, idle cleanup, or
owner-checked discovery cleanup.

## Verification

```bash
cargo test -p bitfun-agent-runtime-ipc
node scripts/check-core-boundaries.mjs
```
