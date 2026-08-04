# BitFun Web Server (development paused)

`bitfun-server` is BitFun's Web Server product surface. Product development is
currently paused; the product is not deprecated or retired. The current partial
host is local and loopback-only, embeds the existing Agent Runtime, and exposes:

- `GET /health` and `GET /api/v1/health`;
- `GET /api/v1/info`;
- `GET /ws`, which serves the App Server JSON-RPC surface after exact browser
  origin validation.

The current implementation is not yet a production Web backend and does not
provide Desktop parity. Dispatch and external-source handlers are preserved for
a later App Server migration but are not registered on the current WebSocket
path. External-source method names keep the App Server's typed
`host_capability_unavailable` response instead of falling back locally.
The embedded Runtime retains the `RemoteExecPort` interface selected by its
bootstrap. The current host does not initialize the required global SSH manager,
so remote execution remains unavailable; the retained port is not an end-to-end
remote capability. Resuming Web Server development must connect these
capabilities through App Server and the existing lower-layer owners rather than
restore a parallel WebSocket command path.

`--workspace` requires an absolute, canonicalizable local path and is treated as
an authoritative operator request; validation, open, or Runtime ownership
failure stops startup instead of silently selecting another workspace. Without
that argument, persisted history is only an advisory startup hint. Server loads
the history metadata without preparing the restored workspace. A local history
entry gets an ownership-aware open; an unusable local entry or a Remote entry
(the current host has no SSH manager) logs a warning and falls back to the
default Assistant workspace. The default workspace uses the same ownership
boundary, including its first directory creation; failure there still stops
startup. This safety gate does not make the paused product production-complete.
The deferred host does not migrate legacy
Assistant directories before ownership; when the current default directory is
absent it keeps using the legacy default in place, classified as an Assistant
workspace. Normal product startup remains the owner of the directory migration.

The repository preserves four inactive Server source references:

- `src/ai_relay.rs`, an unregistered AI API proxy draft that is distinct from
  Remote Connect Relay;
- `src/rpc_dispatcher.rs`, the pre-App-Server dispatcher reference;
- `src/routes/dispatch.rs` and `src/routes/external_sources.rs`, the former
  host-local handlers awaiting App Server migration.

These sources are not in the default Rust module graph and do not represent
delivered runtime capabilities. The non-default
`paused-web-server-source-check` feature only keeps them and their existing
unit tests compilable; it does not register a route, add a CLI option, or
initialize an SSH manager.

For Remote Connect self-hosted relay deployment, use the
[Relay Server README](../relay-server/README.md). The relay service and this
paused Web Server are different products.

## Verification

```bash
cargo check --locked -p bitfun-server
cargo test --locked -p bitfun-server
cargo test --locked -p bitfun-server --features paused-web-server-source-check
```
