# Extension host protocol

This document defines protocol version `1` for the standalone Bun extension host. The names and casing shown here are wire-level names: the Zod schemas in `src/protocol.ts`, generated `protocol.schema.json`, this document, and the Rust adapter implementation must stay identical.

The compatibility target is the established Server plugin API published as `@opencode-ai/plugin@1.17.18`. OpenCode values below are JSON projections of public package types, not imports from OpenCode Core, Protocol, or Server.

## Roles and connection

- **Backend** is the Rust process. It owns the TCP listener, application state, HTTP behavior, auth persistence, lifecycle timing, supervision, and hard timeouts.
- **Host** is the Bun child process. It connects to Rust, loads plugins, retains JavaScript functions, and owns per-instance HTTP gateways.
- **Plugin** is trusted JavaScript or TypeScript loaded into the host.

Rust binds a loopback TCP address before spawning the host and sets:

- `OPENCODE_EXTENSION_HOST_RPC_ADDRESS`, conventionally `127.0.0.1:<port>`.
- `OPENCODE_EXTENSION_HOST_RPC_TOKEN`, a fresh high-entropy secret for this child.

The host makes one TCP connection and immediately calls `backend.handshake`. Rust must not issue a `host.*` call until that handshake succeeds. A failed handshake closes the connection and the host exits.

### Framing and JSON-RPC

Every message is a four-byte unsigned big-endian payload length followed by exactly that many bytes of UTF-8 JSON. The payload is one JSON-RPC 2.0 request, notification, success response, or error response; batches are not supported.

The initial receive limit is 16 MiB (`16_777_216` bytes). The handshake negotiates the limit for later frames. The effective value may never exceed 64 MiB (`67_108_864` bytes), and an oversized length is rejected before allocating its payload.

Requests can travel in both directions. Each peer must keep reading and dispatching incoming calls while awaiting a response, because plugin operations can make reentrant backend calls. Request IDs are directional strings unique for the TCP connection:

- Host-originated: `host:<monotonic integer>`
- Backend-originated: `backend:<monotonic integer>`

Responses echo the ID unchanged. Notifications omit `id`. Plugin stdout and stderr are ordinary process output and are never protocol channels.

### Handshake

The host sends:

```json
{
  "jsonrpc": "2.0",
  "id": "host:1",
  "method": "backend.handshake",
  "params": {
    "token": "value from OPENCODE_EXTENSION_HOST_RPC_TOKEN",
    "protocolVersion": 1,
    "opencodeVersion": "1.17.18",
    "maxFrameBytes": 16777216,
    "capabilities": ["config-contributors-v1", "config-contributions-v2", "generation-fencing-v1"]
  }
}
```

Rust returns:

```json
{
  "jsonrpc": "2.0",
  "id": "host:1",
  "result": {
    "protocolVersion": 1,
    "maxFrameBytes": 16777216,
    "cacheDirectory": "/absolute/path/to/plugin-cache",
    "capabilities": ["config-contributors-v1", "config-contributions-v2", "generation-fencing-v1"]
  }
}
```

`cacheDirectory` must be absolute and writable by the host. It is the only location in which the host installs npm plugins. The accepted `maxFrameBytes` remains fixed until disconnect. Capabilities are intersected by both peers. Rust must not execute function hooks or plugin tools unless `generation-fencing-v1` was negotiated. Config projection requires `config-contributors-v1`; multiple Config contributors additionally require `config-contributions-v2`.

Rust exposes the connection as ready only after the successful handshake response has been written. The host rejects `host.*` requests observed before a valid response. Its operational handlers are installed before the request is sent, so a first business request delivered in the same TCP read as the successful response waits for host construction instead of racing method registration.

## Common wire types

### JSON values

An ordinary wire value is `null`, a boolean, a finite number, a string, an array of wire values, or a plain object with string keys and wire values. Cycles, functions, `BigInt`, symbols, `undefined`, non-finite numbers, and non-plain object instances are rejected. Optional properties are omitted rather than encoded as `undefined`.

A serialization compatibility error identifies the path of the rejected value. The only function-valued capability projection is an `auth.loader` result's `fetch` property, described below.

### Headers, HTTP, and diagnostics

Headers always cross as an array of string pairs so repeated values can be preserved.

```ts
type Headers = Array<[string, string]>

type StreamDescriptor = {
  streamID: string
  length?: number
}

type GatewayHttpRequest = {
  instanceID: string
  requestID: string
  method: string
  path: string // path and query
  headers: Headers
  body?: StreamDescriptor
}

type AuthFetchRequest = {
  url: string
  method?: string
  headers?: Headers
  body?: StreamDescriptor
}

type HttpResponse = {
  status: number // 100 through 599
  statusText?: string
  headers: Headers
  body?: StreamDescriptor
}

type Diagnostic = {
  severity: "debug" | "info" | "warning" | "error"
  code: string
  message: string
  plugin?: string
  method?: string
  data?: JsonValue
}
```

`requestID` identifies one HTTP or auth-fetch invocation. `length`, when present, is a non-negative byte-length hint; EOF remains authoritative. URL and `Headers` objects returned by plugins are normalized before crossing the wire.

### Pull streams

The process that creates a `ReadableStream` registers it and sends a `StreamDescriptor`. The receiver pulls from the owner:

- Rust calls `host.stream.read` for a host-owned descriptor.
- Bun calls `backend.stream.read` for a backend-owned descriptor.

Read params and results are:

```ts
type StreamReadParams = {
  instanceID: string
  streamID: string
  maxBytes?: number // 1 through 65_536
}

type StreamReadResult = {
  data: string // base64
  eof: boolean
}
```

One read returns no more than `maxBytes`, with a 64 KiB maximum. `eof: true` releases the stream. A receiver that stops before EOF calls the owner's `*.stream.cancel` with `{ instanceID, streamID, reason? }`; cancellation is idempotent.

### Process-local identity

`instanceID`, `generationKey`, `revision`, `executionID`, `flowID`, `fetchID`, `requestID`, and `streamID` have no durable meaning. Rust must keep them with their creating instance and connection. `instanceID + generationKey + revision` is the lease for executable Hook and Tool calls. Closing an instance invalidates its active capabilities; losing the process invalidates all of them.

## Rust-to-host methods

### Instance lifecycle

#### `host.plugins.prepare`

Params: `{ plugins, configurationFingerprint?, defaultBaseDirectory?, allowInstall? }`. The result contains the normalized `reviewed` declarations, successfully `prepared` entries, `failed` entries, diagnostics, and a stable `reviewDigest`. Each prepared entry includes its canonical identity and, when available, `contentHash`.

The Host coalesces only concurrent preparation of the same declarations, configuration fingerprint, install policy, and effective default base directory. A settled result is discarded. `host.instance.open` resolves the graph again and compares `expectedReviewDigest` and `expectedContentDigests` before import, so a changed local file or package graph cannot reuse an earlier preparation snapshot.

#### `host.instance.open`

Params:

```ts
{
  instanceID: string
  generationKey: string
  revision: string
  project: JsonValue
  config: Record<string, JsonValue>
  directory: string
  worktree: string
  plugins: Array<{
    spec: string
    options?: Record<string, JsonValue>
    baseDirectory?: string
  }>
  configurationFingerprint?: string
}
```

Result:

```ts
{
  instanceID: string
  generationKey: string
  revision: string
  config: Record<string, JsonValue>
  configContributors: Array<{
    plugin: Record<string, JsonValue>
    outcome: "applied" | "failed"
  }>
  configContributions: Array<{
    plugin: Record<string, JsonValue>
    outcome: "applied" | "failed"
    config: Record<string, JsonValue>
  }>
  diagnostics: Diagnostic[]
  hooks: string[]
  tools: Array<{
    registrationID: string
    id: string
    plugin?: Record<string, JsonValue>
    description: string
    parameters: JsonValue // JSON Schema
  }>
  auth: AuthRegistration[]
  providers: Array<{
    provider: string
    plugin?: Record<string, JsonValue>
    hasModels: boolean
  }>
  workspaces: Array<{
    registrationID: string
    type: string
    plugin?: Record<string, JsonValue>
    name: string
    description: string
  }>
  gatewayURL: string
}
```

The gateway is listening before plugin entrypoints execute, so SDK calls during initialization work. Config hooks run sequentially before the result is sent. Failed plugins are omitted and represented in `diagnostics`; successful registrations remain available.

The host binds the requested `instanceID + generationKey + revision` only after the instance opens successfully and echoes all three values unchanged. Opening an active `instanceID` or a directory already owned by another instance is an error. Reopening after close creates a new instance and reruns entrypoints while preserving Bun's normal process-global module cache.

`configContributors` records every plugin that declared a Config hook, in plugin activation order, and whether its invocation applied or failed. `configContributions` contains the same ordered entries plus a bounded clone of the cumulative config immediately after each hook. Config hooks still retain mutations made before an exception and continue to later contributors. Rust uses the sequence to attribute Agent, permission, and Skill changes without re-executing plugin code. The snapshots are protocol data and must not be written to ordinary logs.

`hooks` may contain:

- `chat.message`
- `chat.params`
- `chat.headers`
- `permission.ask`
- `command.execute.before`
- `tool.execute.before`
- `shell.env`
- `tool.execute.after`
- `experimental.chat.messages.transform`
- `experimental.chat.system.transform`
- `experimental.provider.small_model`
- `experimental.session.compacting`
- `experimental.compaction.autocontinue`
- `experimental.text.complete`
- `tool.definition`

`config`, `event`, `dispose`, `tool`, `auth`, and `provider` are lifecycle hooks or registrations, not operational names.

#### `host.instance.close`

Params: `{ instanceID }`. Result: `{ closed: boolean }`.

The host rejects new operations, aborts active tools and fetches, releases auth flows and streams, closes the gateway, and invokes every disposer once. Ordinary disposer failures are diagnostics and do not stop remaining cleanup. If an active Tool does not drain before the hard deadline, or a disposer itself exceeds its hard deadline, cleanup continues but the close request fails and the Host enters `closing`; Rust must retire that physical Host generation before opening a replacement. Repeated close after a confirmed cleanup is idempotent.

#### `host.shutdown`

Params: `{}`. Result: `{ closed: boolean }`.

The host closes all instances, responds, closes the RPC connection, and exits normally. RPC EOF performs the same best-effort global cleanup before exit.

### Hooks and events

#### `host.hook.call`

Params: `{ instanceID, generationKey, revision, hook, input, output }`.

Result: `{ instanceID, generationKey, revision, hook, input, output }`.

`input` and `output` are JSON values. Matching hooks run sequentially in plugin order on the same live objects for this invocation. The first hook error stops the invocation; earlier mutations are not rolled back. Different hook requests may overlap.

The Host rejects a call whose generation lease does not exactly match the open instance. Rust likewise rejects a response that does not echo the requested instance, generation, revision, and hook name.

For `tool.definition`, `output.parameters` crosses the process boundary as JSON Schema rather than an Effect schema object.

#### `host.event.emit`

Params: `{ instanceID, event }`. Result: `{ accepted: true }`.

The host schedules event hooks in plugin order and responds without awaiting completion. Later failures are sent through `backend.diagnostic.publish`.

### Tools

#### `host.tool.execute`

Params:

```ts
{
  instanceID: string
  generationKey: string
  revision: string
  executionID: string
  registrationID: string
  args: JsonValue
  context: {
    sessionID: string
    messageID: string
    agent: string
    callID?: string
  }
}
```

Result is the public plugin `ToolResult`:

```ts
type ToolResult =
  | string
  | {
      title?: string
      output: string
      metadata?: Record<string, JsonValue>
      attachments?: Array<{
        type: "file"
        mime: string
        url: string
        filename?: string
      }>
    }
```

The host reconstructs a per-execution `AbortSignal` and fills the public tool context's `directory` and `worktree` from the instance. `context.metadata(...)` sends the `backend.tool.metadata` notification and returns synchronously. `context.ask(...)` awaits `backend.tool.ask`.

Tool registration parameters and later `tool.definition` parameters use their JSON Schema projection. Rust sends arguments; Bun validates them through the retained plugin schema before execution.

Rust invokes the tool by the returned opaque `registrationID`. `id` is the plugin-facing tool name and is not an execution handle. The Host validates the complete generation lease before admitting execution and echoes `instanceID`, `generationKey`, `revision`, and `executionID` in the result wrapper together with the plugin `result`.

#### `host.tool.cancel`

Params: `{ instanceID, generationKey, revision, executionID, reason? }`. Result: `{ cancelled: boolean }`.

The host aborts the retained signal. Cancellation is idempotent and does not hard-kill subprocesses created by a plugin. `cancelled: true` means the invocation stopped after observing the abort; it does not prove that earlier filesystem, network, process, or other side effects were rolled back. Rust therefore reports a dispatched cancellation as outcome-unknown and must not automatically retry it.

### Auth

Auth registrations in `host.instance.open` use:

```ts
type AuthRegistration = {
  provider: string
  plugin?: Record<string, JsonValue>
  hasLoader: boolean
  methods: Array<{
    type: "oauth" | "api"
    label: string
    methodIndex: number
    hasAuthorize: boolean
    prompts: Array<{
      type: "text" | "select"
      promptIndex: number
      key: string
      message: string
      placeholder?: string
      options?: Array<{ label: string; value: string; hint?: string }>
      when?: { key: string; op: "eq" | "neq"; value: string }
      hasValidate: boolean
      hasCondition: boolean
    }>
  }>
}
```

The `has*` booleans advertise retained JavaScript capabilities. `when` remains ordinary data; validators and deprecated `condition` functions remain inside Bun.

#### `host.auth.prompt.evaluate`

Params:

```ts
{
  instanceID: string
  provider: string
  methodIndex: number
  promptIndex: number
  operation: "validate" | "condition"
  value?: string
  inputs: Record<string, string>
}
```

Result is `{ operation: "validate", error?: string }` or `{ operation: "condition", active: boolean }`. Rust calls only capabilities advertised by `hasValidate` or `hasCondition`; it can evaluate the serializable `when` rule itself.

#### `host.auth.authorize`

Params: `{ instanceID, provider, methodIndex, inputs? }`.

An API method returns `{ type: "api", result? }`, where `result` is its public success/failed value. An OAuth method returns:

```ts
{
  type: "oauth"
  flowID: string
  url: string
  instructions: string
  method: "auto" | "code"
}
```

The callback remains in Bun under `flowID`. Rust must not call an API method whose registration has `hasAuthorize: false`.

#### `host.auth.callback`

Params: `{ instanceID, flowID, code? }`. Result is the public OAuth success/failed union. `code` is required for a `code` flow and omitted for an `auto` flow. A flow survives until success, explicit cancellation, instance close, or process exit.

#### `host.auth.flow.cancel`

Params: `{ instanceID, flowID, reason? }`. Result: `{ cancelled: boolean }`.

#### `host.auth.loader`

Params: `{ instanceID, provider, providerInfo }`. `providerInfo` is the public SDK provider JSON value.

Result: `{ value: Record<string, JsonValue>, fetchID?: string }`.

The loader receives a live auth getter. Every call to that getter makes a reentrant `backend.auth.get` request; the host does not cache auth state. Ordinary loader fields appear in `value`. A function-valued property named exactly `fetch` is retained in Bun and represented by `fetchID`; all other function-valued results are rejected.

#### `host.auth.fetch`

Params: `{ instanceID, fetchID, requestID, request: AuthFetchRequest }`. Result is `HttpResponse`.

The host reconstructs a Fetch API request, invokes the retained fetch function, and exposes the response body as a host-owned stream. Bun pulls a backend-owned request body with `backend.stream.read`; Rust pulls the host-owned response body with `host.stream.read`. This capability is limited to provider SDK fetch overrides returned by `auth.loader`.

#### `host.auth.fetch.cancel`

Params: `{ instanceID, requestID, reason? }`. Result: `{ cancelled: boolean }`.

#### `host.auth.fetch.release`

Params: `{ instanceID, fetchID }`. Result: `{ released: boolean }`. This releases the retained function for future calls; repeated release is idempotent.

### Providers

#### `host.provider.models`

Params: `{ instanceID, providerID, provider, auth? }`. Result: `{ models }`.

`provider` is the public SDK v2 provider JSON value, `auth` is the optional public auth value, and `models` is a JSON object keyed by model ID. Rust calls this only when the corresponding registration has `hasModels: true`.

### Workspaces

Workspace adapters are invoked through their opaque `registrationID`; the open result also retains their descriptive `type`. Later registration of the same type replaces the earlier adapter.

- `host.workspace.configure`: params `{ instanceID, registrationID, config }`; result `{ config }`.
- `host.workspace.create`: params `{ instanceID, registrationID, config, env, from? }`; result `{}`.
- `host.workspace.remove`: params `{ instanceID, registrationID, config }`; result `{}`.
- `host.workspace.target`: params `{ instanceID, registrationID, config }`; result is `{ target }`, where `target` is `{ type: "local", directory }` or `{ type: "remote", url, headers? }`.

`config` and `from` use the public `WorkspaceInfo` JSON shape. `env` is a record of strings or null; null reconstructs `undefined` for the plugin. Remote `URL` and `HeadersInit` values are normalized to a URL string and header-pair list.

### Host-owned streams

- `host.stream.read`: params `StreamReadParams`; result `StreamReadResult`.
- `host.stream.cancel`: params `{ instanceID, streamID, reason? }`; result `{ cancelled: boolean }`.

These methods accept only streams created by Bun, including gateway request bodies and auth-fetch response bodies.

## Host-to-Rust methods

### `backend.handshake`

Authenticates and negotiates the connection as described in [Handshake](#handshake). It is the only method valid before the connection is ready.

### `backend.http.request`

Params are `GatewayHttpRequest`. Result is `HttpResponse`.

`path` contains the gateway request's path and query, without its loopback origin. Bun creates the request-body descriptor, so Rust pulls it with `host.stream.read`. Rust creates the response-body descriptor, so Bun pulls it with `backend.stream.read`. WebSocket upgrades are rejected at the gateway and never forwarded.

### `backend.auth.get`

Params: `{ instanceID, providerID }`. Result: `{ auth: JsonValue | null }`.

This request can arrive while Rust is awaiting `host.auth.loader` or an active auth fetch. Rust must service it reentrantly. The host makes a fresh request for every plugin getter call.

### `backend.tool.ask`

Params:

```ts
{
  instanceID: string
  generationKey: string
  revision: string
  executionID: string
  permission: string
  patterns: string[]
  always: string[]
  metadata: Record<string, JsonValue>
}
```

Result: `{}` on approval, or a JSON-RPC error on denial/failure. The host awaits this request before resuming the tool.

### `backend.tool.metadata`

Notification params: `{ instanceID, generationKey, revision, executionID, title?, metadata? }`. Because this is a notification, the plugin's `metadata(...)` call returns without waiting for Rust.

### `backend.diagnostic.publish`

Notification params: `{ instanceID?, diagnostic }`. This reports isolated plugin load, lifecycle, event, serialization, or gateway failures. A diagnostic does not replace the error response for a directly failed request.

### Backend-owned streams

- `backend.stream.read`: params `StreamReadParams`; result `StreamReadResult`.
- `backend.stream.cancel`: params `{ instanceID, streamID, reason? }`; result `{ cancelled: boolean }`.

These methods accept only streams created by Rust, including backend HTTP responses and auth-fetch request bodies.

## Loading, ordering, and ownership

- A declaration is `{ spec, options?, baseDirectory? }`. `baseDirectory` anchors a relative local spec.
- npm plugins install in the handshake cache with lifecycle scripts disabled. Local plugins import in place and must already resolve their runtime dependencies.
- Server entrypoint discovery supports `exports["./server"]`, `main`, direct Bun-loadable files, and index files while enforcing package-boundary containment.
- Declarations are deduplicated by npm package identity or canonical local file URL, retaining the last.
- Retained declarations resolve and import concurrently. Successful entrypoints execute sequentially in declaration order.
- Config hooks execute sequentially and isolate errors. Operational hooks execute sequentially and propagate the first error.
- Later duplicate tool IDs, auth providers, provider IDs, and workspace types replace earlier registrations.
- Event dispatch is fire-and-forget in plugin order.
- Different invocations and different instances may overlap; there is no global call lock.
- Rust owns correct hook timing and must stop using an instance's registrations after close.

## Error model

A JSON-RPC error is `{ code: integer, message: string, data?: JsonValue }`. Use the standard codes when applicable:

- `-32700`: invalid JSON.
- `-32600`: invalid JSON-RPC envelope.
- `-32601`: unknown method.
- `-32602`: invalid params.
- `-32603`: unexpected internal failure.

Application failures use the reserved server-error range `-32000` through `-32099` and put machine-readable details in `data`. Unknown instances/handles, invalid instance state, cancellation, plugin exceptions, and serialization failures must be distinguishable in that data. A serialization failure includes its offending value path.

A frame with an invalid or oversized length is a connection-level failure. The detecting peer closes the TCP connection; all outstanding requests fail and the host performs global cleanup. Rust treats every process-local handle as lost and does not automatically replay plugin or provider work.

## Security and recovery

The Rust listener and every instance gateway bind to loopback only. Use a high-entropy, single-spawn handshake token. The token authenticates the expected child to Rust; it does not sandbox plugins.

Plugin code can access the host user's files, environment variables, network, and Bun subprocess APIs. Only load trusted plugin specs. Disabling npm lifecycle scripts narrows install-time behavior, but imported plugin code itself remains fully privileged.

The process boundary contains JavaScript crashes and keeps plugin code out of Rust. It does not provide durable execution identity or crash continuation. After EOF or process death, Rust opens fresh instances and explicitly decides what interrupted application work, if any, is safe to retry.
