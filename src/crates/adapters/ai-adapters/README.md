# BitFun AI Adapters

Shared AI protocol adapters used by both `bitfun-core` and the installer.

This crate owns the portable AI integration layer:

- provider request building
- provider-specific message conversion
- SSE / stream parsing into provider-neutral stream contracts
- shared AI-facing transport types
- provider model discovery
- connection health checks

This crate intentionally does **not** own BitFun runtime concerns such as:

- global config services
- client factories and caches
- application event systems
- agent/session orchestration

Those remain in `bitfun-core`, which maps app config into the shared `AIConfig`
and re-exports this crate where convenient.

## Module Guide

- `client`: shared HTTP transport, retries, aggregation, health checks
- `providers`: OpenAI / Anthropic / Gemini request and discovery adapters
- `stream`: provider SSE parsing into unified streaming events from `bitfun-agent-stream`
- `tool_call_accumulator`: compatibility re-export; canonical implementation lives in `bitfun-agent-stream`
- `types`: portable request/response/config/message types

## Design Rule

If a type or function must behave the same in both the main app and the
installer, it belongs here. If it depends on BitFun runtime state or services,
it should stay outside this crate.
