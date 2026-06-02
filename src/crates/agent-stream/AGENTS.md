# agent-stream Agent Guide

Scope: this guide applies to `src/crates/agent-stream`.

`bitfun-agent-stream` owns provider stream normalization and replayable stream
processing contracts. It should preserve provider wire behavior while exposing a
portable stream surface to higher layers.

## Guardrails

- Do not depend on `bitfun-core`, app crates, Tauri, concrete services,
  transport adapters, terminal, tool-runtime, or product-domain implementations.
- Keep provider-specific parsing isolated to stream normalization. Do not add
  session lifecycle, tool execution, prompt policy, or product orchestration
  behavior here.
- Stream fixture changes must preserve ordering, tool-call reconstruction,
  reasoning/thinking fields, usage accounting, and malformed-chunk handling.
- New provider quirks need fixture coverage rather than broad catch-all parsing.

## Verification

```bash
cargo test -p bitfun-agent-stream
node scripts/check-core-boundaries.mjs
```

For documentation-only changes, run `git diff --check`.
