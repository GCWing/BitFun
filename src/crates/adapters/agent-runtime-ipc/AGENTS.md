[中文](AGENTS-CN.md) | **English**

# Agent Runtime IPC

Scope: `src/crates/adapters/agent-runtime-ipc`.

This non-published crate is a private pre-integration seam for a future
first-party Shared Agent Runtime adapter. It currently proves discovery,
one-instance locking, bounded framing, authenticated initialization, Health,
connection bounds, and cleanup. It is not a public SDK or Runtime owner, and it
has no production consumer yet.

## Pre-integration contract

- First consumer: the separately reviewed first-party interactive TUI attach
  adapter. GUI, Remote, Headless CLI, and SDK Host are not implied consumers.
- Stable test contract: platform-local endpoint, strict initialize-first
  handshake, 64 KiB frame limit, Health, bounded connections, and owner-checked
  discovery cleanup.
- Integration check: the consumer must reuse existing Agent Runtime owners and
  prove Embedded/Shared behavior equivalence without depending on SDK Host.
- Removal condition: delete this seam if the first consumer chooses another
  transport or Shared deployment is abandoned before product activation.

## Boundaries

- Keep all Rust items crate-internal until the first production consumer proves
  the exact API it needs. Do not publish this crate.
- Health is the only operation. Do not add Session, Turn, Tool, MCP, Permission,
  UserInput, Hook, event replay, controller lease, or product configuration.
- Do not depend on `bitfun-core`, Agent Runtime, SDK Host, services, CLI/TUI,
  Tauri, product domains, terminal, tool runtime, or remote transports.
- Use only Windows Named Pipes or Unix Domain Sockets. Do not add TCP, HTTP,
  WebSocket, browser access, or remote fallback.
- Treat this as same-user local isolation, not a sandbox. Product composition
  must supply a user-private runtime directory.

## Verification

```bash
cargo test -p bitfun-agent-runtime-ipc
node scripts/check-core-boundaries.mjs
```
