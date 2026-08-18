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

Build `bitfun-sdk-host`, then pass its absolute path while the platform-native
package layout is still pending:

```typescript
import { AgentClient } from "@bitfun/agent-sdk";

await using client = await AgentClient.start({
  cwd: process.cwd(),
  hostPath: process.env.BITFUN_SDK_HOST_PATH,
});

await using query = await client.query({ prompt: "Summarize this repository" });
for await (const item of query) {
  if (item.type === "assistant_text_delta") {
    process.stdout.write(item.text);
  }
}
const result = await query.result();
```

`BITFUN_SDK_HOST_PATH` is also read directly when `hostPath` is omitted. The
eventual installable package must bundle or resolve a matching signed Host; it
must not require a separately installed BitFun CLI.

## Development

```bash
pnpm --dir sdk/typescript test
pnpm --dir sdk/typescript type-check
```

The internal TypeScript wire bindings are generated from the Rust SDK Host
protocol with `ts-rs`. Runtime validators are generated from those same
bindings, while only JSON-RPC envelope and cross-field semantic checks remain
hand-written. The generated sources are deliberately not the public API and
are not checked in.
