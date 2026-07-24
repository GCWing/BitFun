# Subscription Auth as Model API Implementation Plan

> **For agentic workers:** Implement task-by-task. No CLI-disk-scan compatibility.

**Goal:** Let BitFun sign in to Codex / Antigravity / OpenCode subscriptions and use them as model API auth, aligned with OpenCode.

**Architecture:** `subscription_auth` store + OAuth login sessions in `bitfun-ai-adapters`; `AuthConfig::Subscription`; desktop Tauri + AIModelConfig UI.

**Tech Stack:** Rust (tokio, reqwest, sha2), Tauri commands, React settings UI.

## Tasks

- [x] Spec: `docs/superpowers/specs/2026-07-21-subscription-auth-as-model-api-design.md`
- [x] Replace `cli_credentials` with `subscription_auth` (store, oauth, providers)
- [x] Wire `AuthConfig`, client_factory, desktop commands, remote policy
- [x] Replace Web UI CLI scan section with subscription login panel + i18n
- [x] Verify: cargo check/tests + type-check:web + i18n:audit
