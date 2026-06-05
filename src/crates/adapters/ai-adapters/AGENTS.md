# AI Adapters Agent Guide

Scope: this guide applies to `src/crates/adapters/ai-adapters`.

`bitfun-ai-adapters` owns provider-specific request/response mapping and stream
normalization. Keep provider quirks here instead of leaking them into core tool
contracts or product runtime logic.

## Guardrails

- OpenAI Responses and Codex ChatGPT flat tool schemas are adapter
  serialization behavior. Keep core/tool manifests provider-neutral.
- `cached_content_token_count` means cache reads/hits. Keep
  `cache_creation_token_count` separate, and preserve provider-specific mappings
  such as DeepSeek prompt-cache hits and Gemini's current lack of creation
  count.
- Do not change shared stream or usage semantics without updating the focused
  adapter tests and downstream usage expectations.

## Verification

```bash
cargo test -p bitfun-agent-stream
cargo test -p bitfun-ai-adapters
```

If stream behavior affects core integration, also run the relevant tests in
`src/crates/assembly/core/tests`.
