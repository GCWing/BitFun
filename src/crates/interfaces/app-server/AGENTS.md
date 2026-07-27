[中文](AGENTS-CN.md) | **English**

# App-Server Protocol Surface Guide

Scope: this guide applies to `src/crates/interfaces/app-server`.

`bitfun-app-server` owns a protocol-agnostic JSON-RPC server/client scaffold
built on `agent_client_protocol` custom roles. The role/transport layer is
schema-free; consumers register their own `JsonRpcRequest` /
`JsonRpcNotification` types. The optional `agent` / `schema` / `server`
modules are the Phase 2 wiring that exposes a ready set of agent kernel
operations over a host-injected `AgentRuntime` using the generic `AppServer`
role, unlike `bitfun-acp` which uses the built-in ACP `Agent` role.

## Guardrails

- Keep the role/transport/transport-helper layer schema-free. Do not hard-code
  domain methods or business logic in the role/transport helpers. The Phase 2
  `schema` module is the one place agent kernel JSON-RPC messages live, and it
  must only map to `bitfun_agent_runtime` SDK types, not invent new kernel
  behavior.
- `AppServer` / `AppClient` are generic counterparts; do not reuse the built-in
  `Agent` / `Client` ACP roles here. `HasPeer` is per-role on itself because
  `ConnectionTo::send_request` requires `Counterpart: HasPeer<Counterpart>`.
- The `client` module (`AppServerClient`, `FrontendEvent`, `connect`) is the
  **transport-agnostic** app-server client: it drives an `AppClient` over a
  host-supplied transport and fans projected `agent/event` notifications out
  through a broadcast channel. It is the counterpart of `BitfunAppServer::serve`,
  which likewise takes a host-supplied transport. Hosts pick the transport
  (in-memory pair, stdio, websocket, ...) and own the server half; `connect`
  only owns the client half of one connection. Do not add a server-constructing
  `spawn` here -- server construction is a host concern. Host-specific fan-out,
  field normalization, and JSON-RPC error-code mapping belong in the host, not
  here. Add a new host by depending on this crate, serving `BitfunAppServer` on
  the server half of a transport, and calling `bitfun_app_server::client::connect`
  on the client half.
- Transport constructors must pin `ByteStreams::new(outgoing, incoming)`
  direction; never expose a swap-prone API.
- This crate owns the **full backend contract** under option C: the app-server
  schema is the single JSON-RPC surface the frontend faces, covering both agent
  kernel operations (delegated to `bitfun-agent-runtime` SDK) and host services
  (git/mcp/config/cron/snapshot/fs/workspace/...). To cover host services it
  depends directly on `assembly/core` (`bitfun-core`, `features = "product-full"`)
  -- the same pattern `bitfun-acp` already follows (`bitfun-acp/Cargo.toml`).
  Product assembly constructs the `AgentRuntime` and the host service singletons
  and injects both via `BitfunAppRuntime`; schema handlers for host services call
  `bitfun_core::service::*` the same way the Desktop host does (static/global
  accessors), so `BitfunAppRuntime` does not need a host-services field per
  service. Do not describe this crate as Core-independent for host-service
  operations; the agent-kernel handlers remain backed by the SDK facade only.
- Handlers offload runtime calls to background tasks or return immediately;
  do not call `SentRequest::block_task` inside a handler callback (upstream
  `DEADLOCK` note in `jsonrpc.rs`). Reply through `responder.respond_with_result`.
## Event delivery

Runtime events are part of the app-server protocol surface, not a host-side
subscription. The flow is one-directional over the transport:

- The **server** holds an injected `AgentEventSource` (built from the same
  `EventQueue` the host coordinator publishes to) and its `serve` main_fn
  drains it, forwarding each `AgenticEventEnvelope` to the client as an
  `agent/event` notification (`SessionEventNotification`) over the channel
  transport.
- The **client** registers `on_receive_notification(SessionEventNotification)`
  to receive them, then projects and fans them out to its own consumers
  (websocket connections, Tauri event bridge, ...).
- Hosts must NOT subscribe to the runtime `EventQueue` from the client side.
  The client never touches `AgentRuntime::subscribe_events` or the
  `EventQueue` directly; doing so bypasses the app-server surface and breaks
  the "all agent interfaces go through app-server" contract.

## Verification (continued)

Map `RuntimeError` to JSON-RPC `Error` at the boundary (see
`BitfunAppRuntime::runtime_error` / `session_runtime_error`); do not leak
runtime internals through the wire.

## Verification

```bash
cargo check -p bitfun-app-server --offline
cargo test -p bitfun-app-server --offline
```
