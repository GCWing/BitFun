# Third-party notices

This package is a fork of parts of [DeepSeek Harness](https://github.com/deepseek-ai/deepseek-harness),
which is distributed under the MIT License. Copyright (c) DeepSeek.

| File here | Upstream source | Relationship |
| --- | --- | --- |
| `src/codec.ts` | `packages/acp/acp/src/codec.ts` (`@deepseek-ai/dsh-acp`) | verbatim copy |
| `src/bridge.ts` | `packages/acp/acp/src/index.ts` (`@deepseek-ai/dsh-acp`) | session, turn, approval, and teardown machinery kept; the `session/event` dispatch replaced |
| `src/app.ts` | `packages/examples/acp-demo/src/index.ts` (`@deepseek-ai/dsh-acp-demo`) | composition kept; the transport swapped for `src/bridge.ts` |
| `src/bin.ts` | `packages/examples/acp-demo/src/bin.ts` (`@deepseek-ai/dsh-acp-demo`) | copy with a different bin name |
| `cordis.yml` | `examples/acp-agent/cordis.yml` | trimmed for an IDE launch |

The MIT License text ships with the upstream project; this fork carries the same
terms.

## Why fork rather than depend

`@deepseek-ai/dsh-acp` is automation-only by design. Its `session/event` handler
publishes committed assistant text and nothing else:

> Emit only committed assistant text. Raw chunks, reasoning, tools, plans,
> titles, and retry markers are presentation or trace data and stay off the
> automation wire.

There is no configuration flag that changes this, so an IDE client sees no tool
cards, no reasoning, and no plan — and receives permission requests for tool
calls it was never told about. This fork exists to publish exactly that withheld
data. Upstreaming it as a verbosity level in `@deepseek-ai/dsh-acp` remains the
better long-term outcome.
