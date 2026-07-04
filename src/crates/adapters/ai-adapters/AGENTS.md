# AI Adapters Agent Guide

Scope: this guide applies to `src/crates/adapters/ai-adapters`.

`bitfun-ai-adapters` owns provider-specific request/response mapping, stream
protocol parsing, CLI credential resolution, and provider/model selection
helpers that are independent of core config IO. Keep provider quirks here, then
convert stream chunks into the provider-neutral contracts owned by
`bitfun-agent-stream`.

## Guardrails

- OpenAI Responses and Codex ChatGPT flat tool schemas are adapter
  serialization behavior. Keep core/tool manifests provider-neutral.
- `cached_content_token_count` means cache reads/hits. Keep
  `cache_creation_token_count` separate, and preserve provider-specific mappings
  such as DeepSeek prompt-cache hits and Gemini's current lack of creation
  count.
- Do not change shared stream or usage semantics without updating the focused
  adapter tests and downstream usage expectations.
- Do not move provider-neutral stream DTOs, replay policy, or tool-call
  accumulation ownership back into this crate.
- CLI credential probing may reuse lower-layer service command helpers for
  PATH and process-platform behavior; do not introduce host framework calls.
- Keep `cli-credentials` optional so standalone protocol adapters do not pull
  service/process dependencies by default.

## Verification

```bash
cargo test -p bitfun-agent-stream
cargo test -p bitfun-ai-adapters
cargo test -p bitfun-ai-adapters --features cli-credentials cli_credentials
```

If stream behavior affects core integration, also run the relevant tests in
`src/crates/assembly/core/tests`.
