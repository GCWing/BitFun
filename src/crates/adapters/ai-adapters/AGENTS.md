# AI Adapters Agent Guide

Scope: this guide applies to `src/crates/adapters/ai-adapters`.

`openbitfun-ai-adapters` owns provider-specific request/response mapping, stream
protocol parsing, subscription auth (in-app OAuth login and credential
resolution), and provider/model selection helpers that are independent of core
config IO. Keep provider quirks here, then convert stream chunks into the
provider-neutral contracts owned by `openbitfun-agent-stream`.

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

## Subscription protocol references

Compared on 2026-09-08 against [OpenCode v1.18.29](https://github.com/anomalyco/opencode/tree/16747470f976aca3d362ad730bcd3fe82ecc2c9a)
(`account/account.ts`, `plugin/openai/codex.ts`, `plugin/xai.ts`, and
`session/llm/request.ts` under `packages/opencode/src`) and
[Hermes Agent](https://github.com/NousResearch/hermes-agent/tree/6e2b8e070d28b1a3381a3fb290b6b8d6cce13cef)
(`hermes_cli/auth_nous.py`, `hermes_cli/providers.py`, `agent/codex_headers.py`,
`agent/opencode_affinity.py`, and `agent/transports/codex.py`).

- OpenCode's account catalog chooses each model's protocol within its plan;
  users select a plan/model, not a wire format. Preserve unknown legacy/manual
  routes, and pin catalog-derived endpoints to OpenCode's production origin.
- Hermes currently defaults even `anthropic/*` to Chat Completions while the
  Portal native Messages cache issue is unresolved. Preserve the Nous bearer
  and `x-nous-refresh-token` refresh contract, including rotated-token storage.
- Subscription credentials own authentication and account headers regardless
  of saved replace mode or header casing. Use OpenBitFun attribution for Codex
  and OpenCode; retain provider-required compatibility headers for xAI and
  Antigravity. Public API-key configurations retain their existing behavior.
- Additional subscription request policy is enabled only by an explicit runtime
  subscription identity attached after resolving AuthConfig::Subscription; URLs
  and model names never opt ordinary API-key clients into it.
- Request affinity comes from `ModelRequestContext` on each call, never from a
  random ID on a cached client. Standalone OpenCode calls without runtime context
  still require `x-opencode-session`: generate one opaque identity per logical
  call and reuse it across all retries, including aggregate stream retries.
  Only the matching provider origin receives it.
  Client caches also compare the durable credential revision so login, logout,
  refresh, and account catalog changes invalidate old credentials/routes.

## Verification

Subscription model discovery must use the authenticated account catalog.
Antigravity uses `v1internal:fetchAvailableModels`; preserve returned wire IDs
and restrict alias translation to known legacy names. Codex's `supported_in_api`
flag describes the public API, not subscription availability. OpenCode catalog
models must stay grouped by plan and wire format. Never mask a failed account
lookup with a static catalog or another application's local model cache.

For the auth/discovery path, use `cargo test -p openbitfun-ai-adapters --features
subscription-auth --lib`. Device-grant timing tests use the dev-only Tokio
test clock and synthetic tokens; they do not authorize real accounts.

```bash
cargo test -p openbitfun-agent-stream
cargo test -p openbitfun-ai-adapters
cargo test -p openbitfun-ai-adapters --features subscription-auth subscription_auth
```

If stream behavior affects core integration, also run the relevant tests in
`src/crates/assembly/core/tests`.
