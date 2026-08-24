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
- the same stream reports safe Tool lifecycle facts and permission requests;
  `Query.respondPermission()` supports allow once, allow always, or reject;
- protocol and process failures use `SdkError`, including outcome certainty.

Lifecycle cleanup is bounded. The Windows Host contains descendants in a
kill-on-close Job Object, while Unix managed Hosts run in an isolated process
group. A cleanup result whose outcome is unknown makes the connection
unusable and triggers Host reclamation.

It does not start the CLI or the Node/Bun Plugin Host, and it does not implement
another Agent Runtime. The managed native Host adapts this package to the
existing `agent-runtime::sdk` API.

## Repository usage

Build the private SDK and `bitfun-sdk-host`, then stage that already-built Host
into the local package. This does not install BitFun or publish anything:

```bash
cargo build -p bitfun-sdk-host-app
pnpm --dir sdk/typescript build
pnpm --dir sdk/typescript stage:host -- ../../target/debug/bitfun-sdk-host.exe
```

Use `bitfun-sdk-host` without `.exe` on macOS and Linux. The staging command
copies only the current platform's executable into the package build under
`dist/sdk/typescript/native/<platform>-<arch>/`.

The trusted application then supplies one process-lifetime model configuration;
the SDK finds and manages the staged native Host automatically:

```typescript
import { AgentClient } from "@bitfun/agent-sdk";

const apiKey = await trustedSecretStore.read("openai");
await using client = await AgentClient.start({
  cwd: process.cwd(),
  model: {
    provider: "openai",
    model: "gpt-5.4",
    apiKey,
    baseUrl: "https://api.openai.com/v1",
  },
});

await using query = await client.query({ prompt: "Summarize this repository" });
for await (const item of query) {
  switch (item.type) {
    case "assistant_text_delta":
      process.stdout.write(item.text);
      break;
    case "tool_event":
      console.log(item.toolName, item.status);
      break;
    case "permission_request":
      await query.respondPermission(item.requestId, { decision: "allow_once" });
      break;
  }
}
const result = await query.result();
```

An explicit absolute `hostPath` remains available as a development override.
The SDK never searches `PATH` or an environment variable for the Host.

This repository-local package is private and unpublished. Node 24.14.1 is
locally verified for this slice. Bun uses the same ESM build but remains a
release-verification target when a Bun runner is available; neither runtime is
a bundled executable or a final minimum-version policy. `pnpm --dir sdk/typescript pack`
can produce a local tarball containing the staged Host. An application installs
that tarball as an ordinary dependency; it does not install BitFun or a CLI
separately. This PR does not publish the package. A future registry release
still needs platform packages, signing, and release verification.

Browser and mobile runtimes cannot launch the local native Host. Custom
functions, general user-input callbacks, structured output, usage, Session
resume, Python support, platform package publication, signing, and downloads
remain deferred.

## Development

```bash
pnpm --dir sdk/typescript test
pnpm --dir sdk/typescript type-check
pnpm --dir sdk/typescript smoke:node
pnpm --dir sdk/typescript smoke:bun
pnpm --dir sdk/typescript smoke:consumer
```

The internal TypeScript wire bindings are generated from the Rust SDK Host
protocol with `ts-rs`. Runtime validators are generated from those same
bindings, while only JSON-RPC envelope and cross-field semantic checks remain
hand-written. The generated sources are deliberately not the public API and
are not checked in.
