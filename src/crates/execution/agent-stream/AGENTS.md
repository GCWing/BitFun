# agent-stream Agent Guide

Scope: this guide applies to `src/crates/execution/agent-stream`.

`bitfun-agent-stream` owns provider-neutral stream DTOs, tool-call accumulation,
and replayable stream processing contracts. Provider wire parsing belongs in
`src/crates/adapters/ai-adapters`, which converts provider chunks into these
portable stream contracts.

## Guardrails

- Do not depend on `bitfun-core`, app crates, Tauri, concrete services,
  transport adapters, AI adapters, terminal, tool-runtime, or product-domain
  implementations.
- Keep provider-specific SSE or response parsing in `bitfun-ai-adapters`; this
  crate only owns provider-neutral stream assembly and replay behavior.
- Do not add session lifecycle, tool execution, prompt policy, or product
  orchestration behavior here.
- Stream contract changes must preserve ordering, tool-call reconstruction,
  reasoning/thinking fields, usage accounting, and malformed-chunk handling.

## Verification

```bash
cargo test -p bitfun-agent-stream
node scripts/check-core-boundaries.mjs
```

When provider fixture parsing changes, run the focused `bitfun-ai-adapters`
stream tests as well.

For documentation-only changes, run `git diff --check`.
