# TUI Session and Context Status Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make CLI TUI session navigation, fresh-session aliases, and model-context status truthful in both Embedded and Shared runtimes.

**Architecture:** Keep the change inside the CLI adapter. Project the existing `TokenUsageUpdated` event into a small UI-owned snapshot for the latest primary-model request, render that snapshot without claiming it is cumulative session usage, and reuse the existing session picker for history. No new runtime owner, transport request, or persistence contract is introduced.

**Tech Stack:** Rust, Ratatui, existing BitFun CLI action registry and agentic event contracts.

## Global Constraints

- Do not modify Relay, shared IPC, Core runtime behavior, Web UI, or extension/customization paths.
- Preserve the existing `/usage` command as the authoritative cumulative report where supported.
- Keep `/status` transient; it must not add a persisted conversation message.
- Treat subagent usage and last-round usage as distinct from session totals.
- Keep the total PR below 8k changed lines and prefer one final commit.

---

### Task 1: Correct slash-command semantics

**Files:**
- Modify: `src/apps/cli/src/actions.rs`
- Modify: `src/apps/cli/src/modes/chat/commands.rs`
- Modify: `src/apps/cli/src/ui/startup.rs`

- [x] Add failing registry tests proving `/status` is available in Shared TUI, `/sessions` exposes OpenCode-compatible `/resume` and `/continue` aliases, `/history` resolves to the existing session picker for compatibility, and `/clear` resolves to the same new-session action as `/new`.
- [x] Run the focused action tests and confirm they fail for the missing behavior.
- [x] Add the `Status` handler, route `/history` through `Sessions`, add the OpenCode-compatible session aliases, and remove the misleading history-statistics action.
- [x] Remove the TUI-only clear-conversation action and make `/clear` an alias for `/new`, matching OpenCode's fresh-session semantics without inventing a new clear-screen command.
- [x] Run the focused action tests and confirm they pass.

### Task 2: Preserve truthful primary-model context facts

**Files:**
- Modify: `src/apps/cli/src/chat_state.rs`
- Modify: `src/apps/cli/src/modes/chat/run.rs`
- Add: `src/apps/cli/src/ui/chat/status.rs`
- Modify: `src/apps/cli/src/modes/chat.rs`
- Modify: `src/apps/cli/src/ui/chat.rs`

- [x] Add failing unit tests for the latest primary-model usage snapshot and status text, including unknown context, known context percentage, and no session-total claim.
- [x] Run the focused tests and confirm they fail for the missing projection and formatter.
- [x] Replace `ChatMetadata::total_tokens` with a UI-owned `ModelTokenUsageSnapshot` containing only stable event facts used by the TUI.
- [x] Ignore subagent usage and unrelated turn events while retaining the latest primary-model request facts.
- [x] Add a pure transient `/status` formatter covering session, runtime, workspace, approval mode, and observed context facts.
- [x] Run the focused tests and confirm they pass.

### Task 3: Wire the popup and status bar, then verify the PR

**Files:**
- Modify: `src/apps/cli/src/modes/chat/commands.rs`
- Modify: `src/apps/cli/src/ui/chat/render.rs`
- Modify: `src/apps/cli/src/ui/command_menu.rs`
- Modify: `src/apps/cli/src/ui/command_palette.rs`

- [x] Add failing tests for compact status-bar text with known, unknown, and missing context-window values.
- [x] Run the focused tests and confirm they fail for the old token-total display.
- [x] Wire `/status` to the existing transient info popup in both runtime modes.
- [x] Replace all `Tokens:` session-total labels with truthful latest-request context text, omitting unavailable data rather than showing zero.
- [x] Run `cargo test -p bitfun-cli`, `cargo check -p bitfun-cli`, and `git diff --check`; run `cargo fmt -p bitfun-cli -- --check` and record its unrelated baseline drift.
- [x] Adversarially review the full diff for runtime parity, active-turn behavior, misleading copy, accidental persistence, duplicate aliases, and out-of-scope files; fix all actionable findings.
