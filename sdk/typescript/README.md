# OpenBitFun Agent SDK for TypeScript

This package is a repository-internal vertical slice. It is private, reports
`not_delivered` through the Host handshake, and must not be published or
described as a Preview SDK yet.

The slice validates the intended public object model:

- one application-level `AgentClient` owns one managed native
  `openbitfun-sdk-host` process and one Host connection;
- `client.query()` uses a Host-managed transient Session;
- `client.sessions.create()` creates a durable Session whose Turns reuse the
  same connection, while `client.sessions.resume(id)` attaches it to a later
  Host process;
- `Query` is an ordered async stream with idempotent cancellation, cached final
  `Result`, and explicit close semantics;
- a prompt can be a string or ordered `text` / `local_image` parts; local image
  paths are resolved against the Session workspace and reuse Runtime image
  attachments;
- a terminal `Result` reports aggregate token usage when the provider supplied
  usage for that Turn;
- a Turn may request JSON output with `outputSchema`; its terminal `Result`
  keeps the raw text and exposes the parsed value as `structuredOutput`;
- the same stream reports safe Tool lifecycle facts and permission requests;
  `Query.respondPermission()` supports allow once, allow always, or reject;
- protocol and process failures use `SdkError`, including outcome certainty.

Lifecycle cleanup is bounded. The Windows Host contains descendants in a
kill-on-close Job Object, while Unix managed Hosts run in an isolated process
group. A cleanup result whose outcome is unknown makes the connection
unusable and triggers Host reclamation.

Durable Sessions are persisted by the existing Agent Runtime. Closing a
Session or its client unloads it without deleting its history, so a later
client using the same workspace can resume it by ID. Existing OS-level Session
locks reject a second writer while another Host process owns that same Session;
different Sessions remain independent.

It does not start the CLI or the Node/Bun Plugin Host, and it does not implement
another Agent Runtime. The managed native Host adapts this package to the
existing `agent-runtime::sdk` API.

## Repository usage

Build the private SDK and `openbitfun-sdk-host`, then stage that already-built Host
into the local package. This does not install OpenBitFun or publish anything:

```bash
cargo build -p openbitfun-sdk-host-app
pnpm --dir sdk/typescript build
pnpm --dir sdk/typescript stage:host -- ../../target/debug/openbitfun-sdk-host.exe
```

Use `openbitfun-sdk-host` without `.exe` on macOS and Linux. The staging command
copies only the current platform's executable into the package build under
`dist/sdk/typescript/native/<platform>-<arch>/`.

The trusted application then supplies one process-lifetime model configuration;
the SDK finds and manages the staged native Host automatically:

```typescript
import { AgentClient } from "@openbitfun/agent-sdk";

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

await using query = await client.query({
  prompt: [
    { type: "text", text: "Explain this screenshot in repository context" },
    { type: "local_image", path: "screenshots/failure.png" },
  ],
});
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
console.log(result.usage?.totalTokens);
```

Structured output is scoped to one Query or Session Turn:

```typescript
const result = await (
  await client.query({
    prompt: "Summarize the repository",
    outputSchema: {
      type: "object",
      properties: { summary: { type: "string" } },
      required: ["summary"],
      additionalProperties: false,
    },
  })
).result();

console.log(result.structuredOutput);
```

The Host requires an object schema and maps it to the selected OpenAI Chat,
OpenAI Responses, Anthropic, or Gemini protocol. It parses the final response
as JSON; invalid JSON produces a failed Result while preserving `outputText`.

`local_image` accepts local PNG, JPEG, GIF, and WebP paths. Image bytes and
remote URLs are intentionally outside this local Host protocol.

Use an explicit Session when the application needs continuity across client or
Host restarts. Here `options` is the same trusted `AgentClientOptions` value
shown above:

```typescript
const firstClient = await AgentClient.start(options);
const session = await firstClient.sessions.create({ sessionName: "review" });
const sessionId = session.id;
await (await session.startTurn({ prompt: "Inspect the current changes" })).result();
await firstClient.close(); // unloads the Session and exits its managed Host

const nextClient = await AgentClient.start(options);
const resumed = await nextClient.sessions.resume(sessionId);
await (await resumed.startTurn({ prompt: "Now summarize the risks" })).result();
await nextClient.close();
```

An explicit absolute `hostPath` remains available as a development override.
The SDK never searches `PATH` or an environment variable for the Host.

This repository-local package is private and unpublished. Node 24.14.1 is
locally verified for this slice. Bun uses the same ESM build but remains a
release-verification target when a Bun runner is available; neither runtime is
a bundled executable or a final minimum-version policy. `pnpm --dir sdk/typescript pack`
can produce a local tarball containing the staged Host. An application installs
that tarball as an ordinary dependency; it does not install OpenBitFun or a CLI
separately. This PR does not publish the package. A future registry release
still needs platform packages, signing, and release verification.

Browser and mobile runtimes cannot launch the local native Host. Custom
functions, general user-input callbacks, Python support, platform package
publication, signing, and downloads
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
