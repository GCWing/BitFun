# AI Adapters Agent Guide

Scope: this guide applies to `src/crates/adapters/ai-adapters`.

`bitfun-ai-adapters` owns provider-specific request/response mapping, stream
protocol parsing, subscription auth (in-app OAuth login and credential
resolution), and provider/model selection helpers that are independent of core
config IO. Keep provider quirks here, then convert stream chunks into the
provider-neutral contracts owned by `bitfun-agent-stream`.

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
- Subscription auth (Codex/Antigravity codex CLI user-agent probing) may reuse
  lower-layer service command helpers for PATH and process-platform behavior; do
  not introduce host framework calls.
- Keep `subscription-auth` optional so standalone protocol adapters do not pull
  service/process dependencies by default. Never scan or reuse third-party CLI
  credential files on disk; tokens come only from the in-app OAuth store.

## Verification

```bash
cargo test -p bitfun-agent-stream
cargo test -p bitfun-ai-adapters
cargo test -p bitfun-ai-adapters --features subscription-auth subscription_auth
```

If stream behavior affects core integration, also run the relevant tests in
`src/crates/assembly/core/tests`.
