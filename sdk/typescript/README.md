# BitFun Agent SDK for TypeScript

This package is a repository-internal vertical slice. It is private, reports
`not_delivered` through the Host handshake, and must not be published or
described as a Preview SDK yet.

The slice validates the intended public object model:

- one application-level `AgentClient` owns one managed native
  `bitfun-sdk-host` process and one Host connection;
- `client.query()` uses a Host-managed transient Session;
- `client.sessions.create()` creates an explicit Session whose Turns reuse the
  same connection and existing Agent Runtime owner;
- `Query` is an ordered async stream with idempotent cancellation, cached final
  `Result`, and explicit close semantics;
- protocol and process failures use `SdkError`, including outcome certainty.

Lifecycle cleanup is bounded. The Windows Host contains descendants in a
kill-on-close Job Object, while Unix managed Hosts run in an isolated process
group. A cleanup result whose outcome is unknown makes the connection
unusable and triggers Host reclamation.

It does not start the CLI or the Node/Bun Plugin Host, and it does not implement
another Agent Runtime. The managed native Host adapts this package to the
existing `agent-runtime::sdk` API.

## Repository usage

Build `bitfun-sdk-host`, then pass its absolute path and one process-lifetime
model configuration while the platform-native package layout is still pending.
The Host path must be explicit in this slice:

```typescript
import { AgentClient } from "@bitfun/agent-sdk";

const apiKey = await trustedSecretStore.read("openai");
await using client = await AgentClient.start({
  cwd: process.cwd(),
  hostPath: "/absolute/path/to/bitfun-sdk-host",
  model: {
    provider: "openai",
    model: "gpt-5.4",
    apiKey,
    baseUrl: "https://api.openai.com/v1",
  },
});

await using query = await client.query({ prompt: "Summarize this repository" });
for await (const item of query) {
  if (item.type === "assistant_text_delta") {
    process.stdout.write(item.text);
  }
}
const result = await query.result();
```

This repository-local package is private and unpublished. Node 24.14.1 and Bun
1.4.0 are the locally verified runners for this slice; they are not bundled
executables or a final minimum-version policy. The eventual installable package
must bundle or resolve a matching signed Host; it must not require a separately
installed BitFun CLI.

Browser and mobile runtimes cannot launch the local native Host. Custom
functions, permission and user-input callbacks, structured output, usage,
Session resume, Python support, and native package staging remain deferred.

## Development

```bash
pnpm --dir sdk/typescript test
pnpm --dir sdk/typescript type-check
pnpm --dir sdk/typescript smoke:node
pnpm --dir sdk/typescript smoke:bun
```

The internal TypeScript wire bindings are generated from the Rust SDK Host
protocol with `ts-rs`. Runtime validators are generated from those same
bindings, while only JSON-RPC envelope and cross-field semantic checks remain
hand-written. The generated sources are deliberately not the public API and
are not checked in.
