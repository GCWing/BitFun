# Subscription Auth as Model API Design

Date: 2026-07-21

## Goal

Align BitFun with OpenCode’s “use another product’s subscription as a model API”
capability. Users sign in to Codex (ChatGPT), Antigravity (Google), or OpenCode
Zen inside BitFun; the resulting OAuth tokens authenticate AI requests. No
upgrade path for the previous Codex/Gemini CLI disk-scan import.

## Non-goals

- Reusing `~/.codex/auth.json` / `~/.gemini/*`
- Migrating `AuthConfig::{CodexCli, GeminiCli}`
- Embedding the OpenCode Node/Effect runtime
- GitHub Copilot or other OpenCode auth plugins in this round

## Architecture

```
UI (AIModelConfig)
  -> Tauri: list / start_login / cancel_login / logout / status
  -> bitfun-ai-adapters::subscription_auth
       store: {user_data_dir}/subscription_auth.json (0600)
       providers: codex | antigravity | opencode
  -> apply_subscription_auth(AuthConfig, &mut AIConfig)
  -> existing provider adapters (codex_chatgpt / gemini-code-assist / opencode zen)
```

### AuthConfig

```rust
enum AuthConfig {
  ApiKey,
  Subscription { provider: SubscriptionProvider }, // codex | antigravity | opencode
}
```

Model configs store only the subscription reference. Tokens live only in the
subscription auth store and are refreshed on resolve.

### Providers

| ID | Login | Runtime |
|---|---|---|
| `codex` | ChatGPT PKCE on `localhost:1455/auth/callback` | Bearer + ChatGPT-Account-Id → `chatgpt.com/backend-api/codex/responses` |
| `antigravity` | Google OAuth PKCE on `localhost:51121/oauth-callback` | Bearer + Antigravity Client-Metadata → `cloudcode-pa` (daily→prod fallback) |
| `opencode` | Device code against `console.opencode.ai` | Bearer → Zen `/api/config` models + provider API |

### UI

Replace the “local CLI accounts” section with a “Subscription accounts” panel:
status, Login, Logout, and “Use as model” which creates an `AIModelConfig` with
`auth: { type: subscription, provider }`, suggested format/base_url, empty
`api_key`.

### Remote policy

Login/logout/list commands are local-machine only (`LocalOnly`).

## Verification

- `cargo test -p bitfun-ai-adapters --features subscription-auth subscription_auth`
- `cargo check -p bitfun-desktop`
- `pnpm run type-check:web`
- Manual: login each provider, import model, run a short chat turn
