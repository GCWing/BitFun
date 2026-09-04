# OpenCode extension host

This directory is OpenBitFun's standalone Bun process for running established OpenCode Server plugins outside the OpenCode server. It targets the public `@opencode-ai/plugin` and `@opencode-ai/sdk` contract at version `1.17.18` and is supervised by the Rust backend.

The host is a compatibility process, not an OpenCode server. Rust owns application state, persistence, HTTP behavior, lifecycle timing, and process supervision. The Bun process owns plugin resolution, JavaScript execution, and the function-valued capabilities that cannot cross a JSON boundary.

## Compatibility boundary

The host supports:

- Public OpenCode Server plugin entrypoints and hook types from `@opencode-ai/plugin@1.17.18`.
- npm package specs and local files or directories.
- Package `exports["./server"]`, package `main`, and Bun-loadable `index.ts`, `index.tsx`, `index.js`, `index.mjs`, or `index.cjs` files.
- Object-form `{ id, server }` modules and legacy function exports.
- Plugin tools, auth providers and OAuth callbacks, provider model callbacks, and experimental workspace adapters.
- Multiple independent directory-scoped instances in one host process.
- The official OpenCode SDK, raw HTTP, and SSE through a per-instance loopback gateway.

It deliberately does **not** support:

- TUI plugins or the v2 plugin API.
- OpenCode's built-in plugins.
- Discovery or loading of `opencode.json` and other OpenCode configuration files.
- WebSocket proxying. A gateway request that attempts a WebSocket upgrade is rejected.
- Isolation from a malicious plugin. Plugins execute as trusted native extensions with the host user's filesystem, environment, network, and subprocess authority.

Local plugins are imported in place and must already be able to resolve their runtime dependencies. npm plugins are installed into the cache directory supplied by Rust during the handshake, with lifecycle scripts disabled. The host never installs into or edits a local plugin project.

Configured plugin declarations are therefore an explicit trusted-code opt-in, not a sandbox or a fine-grained activation grant. The current reliability controls (loopback authentication, generation fencing, deadlines, cancellation, and process-tree cleanup) remain enabled, while immutable import snapshots, provenance, activation/permission UX, credential isolation, per-plugin isolation, and resource quotas are tracked as follow-up security work in [`opencode-extension-compatibility.md`](../../docs/architecture/extensions/opencode-extension-compatibility.md#21-当前受管-package-plugin-运行切片).

## Architecture

Rust first binds a loopback TCP listener and then launches the host. The host connects to the address in `OPENCODE_EXTENSION_HOST_RPC_ADDRESS` and authenticates its first request with `OPENCODE_EXTENSION_HOST_RPC_TOKEN`.

```text
OpenCode plugin
  |-- hooks, tools, auth, provider, workspace --> Bun extension host
  |-- official SDK / raw HTTP / SSE ----------> per-instance 127.0.0.1 gateway
                                                     |
Rust backend <====== framed bidirectional JSON-RPC ==+
```

Control traffic uses JSON-RPC 2.0 messages framed by a four-byte big-endian length. Requests can travel in either direction and may be reentrant; plugin stdout and stderr are never used as protocol channels. HTTP and fetch bodies use pull-based stream handles so the receiver controls backpressure instead of embedding unbounded bodies in JSON.

Each `host.instance.open` call creates one logical plugin instance and one HTTP gateway. The host resolves and imports retained plugin declarations concurrently, executes successful entrypoints in declaration order, runs their config hooks, and returns the resulting registrations. The open result includes ordered cumulative Config snapshots so Rust can attribute Agent, permission, and Skill changes across multiple Config hooks without re-executing plugin code. Operational hook calls are ordered within one invocation, but unrelated invocations and unrelated instances may overlap.

Closing an instance rejects new work, cancels active tools and fetches, closes its gateway, and invokes every registered disposer once. Losing the RPC connection applies the same cleanup to all instances and terminates the host. Rust is responsible for restarting the process and deciding whether any application work should be retried.

See [PROTOCOL.md](./PROTOCOL.md) for the complete method, error, and wire contract. `protocol.schema.json` is the generated machine-readable form consumed by a future Rust client; the Zod schemas in the implementation are canonical.

## Build and launch

The directory has its own dependency lock and does not rely on workspace-internal OpenCode packages.
Official Desktop, CLI, and app-server development/build entry points run
`pnpm run plugin-host:prepare`; Desktop and CLI packages plus app-server release
output stage the resulting `extension-host.js` under `resources/ext-host`. The current execution backend
still requires a compatible `bun` command at runtime (or an explicit
`OPENBITFUN_BUN_COMMAND`). Bundling a signed Bun sidecar is separate distribution
work and must be completed before claiming a self-contained installer.

```sh
cd src/apps/extension-host
bun install --frozen-lockfile
bun typecheck
bun test
bun run build
```

The Rust supervisor should bind its listener before spawning the built host and inherit or redirect stdout and stderr normally:

```sh
OPENCODE_EXTENSION_HOST_RPC_ADDRESS=127.0.0.1:48731 \
OPENCODE_EXTENSION_HOST_RPC_TOKEN="$ONE_TIME_RANDOM_TOKEN" \
bun ./dist/extension-host.js
```

The first host-to-Rust call is `backend.handshake`. Rust verifies the token, negotiates a frame limit, and supplies the npm plugin cache directory. Do not send instance requests until that handshake succeeds. The default negotiated frame limit is 16 MiB and neither peer may negotiate more than 64 MiB.

At startup the host appends `127.0.0.1`, `localhost`, and `::1` to both `NO_PROXY` and `no_proxy`. This keeps the injected SDK and raw `serverUrl` traffic on the per-instance loopback gateway even when the supervisor environment defines an HTTP proxy; all other proxy settings remain visible to plugins.

A normal supervisor sequence is:

1. Bind the Rust-owned loopback listener, generate a fresh token, and spawn the host with the two environment variables.
2. Accept the connection and complete `backend.handshake`.
3. Open one or more instances with explicit project/config values and ordered plugin declarations.
4. Invoke hooks, tools, auth flows, provider callbacks, workspace adapters, and HTTP forwarding as application state requires.
5. Close individual instances when their directories are released.
6. Call `host.shutdown` for an orderly process shutdown, or close the TCP connection to force global cleanup.

Rust should impose its own startup and request deadlines. The host intentionally does not provide durable recovery: instance IDs, registrations, active executions, stream handles, and auth-flow handles are process-local.

## Loading plugins

Rust passes declarations directly to `host.instance.open`:

```ts
type PluginDeclaration = {
  spec: string
  options?: Record<string, unknown>
  baseDirectory?: string
}
```

`baseDirectory` anchors a relative local `spec`; it does not change the plugin's process working directory. A declaration without `baseDirectory` resolves a relative path from the instance directory.

Declarations are deduplicated by npm package identity or canonical local file URL, retaining the last declaration. Resolution and import happen concurrently, but successful entrypoints execute sequentially in retained order. Later registrations replace earlier tools with the same tool ID, auth hooks for the same provider, provider hooks for the same provider ID, and workspace adapters for the same type.

For npm packages, `engines.opencode` is checked against `1.17.18`. Install, entrypoint, compatibility, import, and entrypoint-execution failures are isolated to that plugin and returned as structured diagnostics. Config-hook and dispose-hook failures are also isolated; mutations completed before a config-hook failure remain visible.

[`examples/example-plugin.ts`](./examples/example-plugin.ts) demonstrates the public plugin shape and the injected SDK, raw gateway URL, Bun shell, tool context, hook, and workspace APIs.

## Gateway and streams

Every instance gets a distinct `127.0.0.1` HTTP URL before its plugin entrypoints run. The injected SDK client uses that URL, and plugins may also use the injected `serverUrl` directly. The gateway forwards method, path and query, headers, and a streaming request body through `backend.http.request`, then reconstructs the backend's status, headers, and streaming response body. SSE therefore remains an ordinary streamed HTTP response.

The side that creates a stream owns it. The other side repeatedly calls that owner's `*.stream.read` method, which returns at most 64 KiB encoded as base64, and then cancels or consumes the stream to EOF. Stream IDs, like every other opaque handle, are scoped to an instance and become invalid when that instance closes.

## Serialization and diagnostics

Only JSON-compatible data crosses the control channel. Cycles, functions, `BigInt`, and non-finite numbers produce a compatibility error that identifies the failing value path. Two deliberate projections handle public plugin values that are not natively serializable:

- Tool parameter schemas cross as JSON Schema, including the `tool.definition` hook's `output.parameters` value.
- A function-valued `auth.loader` result named `fetch` remains in Bun and crosses as an opaque fetch handle. Other function-valued loader results are rejected.

Plugin load failures, fire-and-forget event failures, and isolated lifecycle failures are published with `backend.diagnostic.publish`. Request failures use structured JSON-RPC errors; see [PROTOCOL.md](./PROTOCOL.md#error-model) for the stable error envelope.

## Keeping the host self-contained

Keep this entire directory together rather than moving only `dist/extension-host.js`. `package.json`, `bun.lock`, `protocol.schema.json`, and the protocol documentation must stay versioned as one unit with the JavaScript and Rust sides. After dependency or protocol changes, run the validation commands above and a subprocess handshake smoke test.

Do not replace the pinned public packages with imports from this monorepo's Core, Protocol, Server, or generated internal modules. The dependency boundary is intentional: the extracted host must remain runnable without an OpenCode source checkout.
