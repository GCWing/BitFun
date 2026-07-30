[中文](AGENTS-CN.md) | **English**

# Agent Runtime IPC Migration

Scope: `src/crates/adapters/agent-runtime-ipc`.

This crate is the legacy private Shared TUI protocol. It is migration-only and
is not a target architecture boundary. The final architecture uses one App
Server wire for interactive TUI, Desktop, Electron, VS Code, and Web across
Embedded and Shared deployments. Shared instances are isolated by
`client_kind`; those client forms never connect to one instance. Read
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
- Interactive TUI must migrate to `AppServerClient`: default TUI uses an
  exclusive Embedded App Server and `--shared` connects a TUI-only Shared App
  Server.
- Remove this crate, or reduce it to an App Server-internal transport
  implementation with no independent wire semantics, once the old consumer is
  gone.

## Current Production Contract

Until the last Shared TUI consumer migrates and the deletion gates pass, this
remains a production contract even though it is not a target boundary:

- The only consumer is the first-party interactive TUI adapter in
  `src/apps/cli`. GUI, Remote, Peer, ACP, Headless CLI, and SDK Host are not
  consumers.
- Use only a local Windows Named Pipe or Unix Domain Socket. Windows rejects
  remote clients and requires the bearer handshake, but the current code does
  not explicitly install an owner-only pipe DACL or verify the peer SID; do not
  claim same-user isolation until that is implemented and tested. Unix keeps
  owner-only discovery/socket permissions. Do not add TCP, HTTP, WebSocket,
  browser access, or remote fallback; local transport plus authentication is
  not a sandbox.
- Require initialize as the first frame and use separate handshake/request
  deadlines. Initialization validates protocol version, instance identity,
  bearer token, client ID, and client version; token and discovery secrets stay
  redacted. Invalid token, wrong instance, protocol mismatch, pre-initialize
  requests, and unauthenticated connection exhaustion remain rejection tests.
- Reject unknown fields and operations. Request frames are limited to 128 KiB;
  response/event frames are limited to 8 MiB. Connections, queues, pending
  requests, and serialized buffers remain bounded and backpressured.
- The closed operation budget is Health; Session list/create/restore including
  transcript; current-Session rename and Agent mode/model update; Turn
  submit/cancel; pending/respond Permission; and UserInput answers. Do not add
  delete, fork, replay, Observer, controller transfer, Tool/MCP/Hook management,
  or product configuration.
- Keep one Controller per Session and one active Turn per connection. Preserve
  disconnect cancellation, `outcome_unknown`, sticky event-stream invalidation,
  30-second idle exit, and owner-checked discovery cleanup.
- Legacy in-process and Headless Direct Runtime callers continue to invoke the
  typed Agent Runtime directly and do not initialize this transport. Shared
  frames are encoded once before write; throughput work must not relax strict
  decoding, bounds, or backpressure.

Migration may change a limit only after the equivalent App Server limit has a
measured rationale, conformance and overload tests, and an explicit compatibility
decision. Until then, carry the current value forward unchanged.

## Verification

```bash
cargo test -p bitfun-agent-runtime-ipc
node scripts/check-core-boundaries.mjs
```
