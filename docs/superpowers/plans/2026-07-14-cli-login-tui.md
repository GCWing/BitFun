# CLI Login TUI Implementation Plan

> **For agentic workers:** Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace cooked-mode `/login` prompts with a dedicated ratatui Login page.

**Architecture:** New `ui/login_form.rs` owns form state/render/keys. `account::login_with_credentials` performs Auth + persistence + Peer Host. Startup and Chat show the form and handle Submit/Cancel.

**Tech Stack:** ratatui, crossterm, existing CLI account client

---

### Task 1: Account API without stdin

- Modify: `src/apps/cli/src/account.rs`
- [ ] Replace `login_interactive` + echo helpers with `login_with_credentials(relay_url, username, password)`

### Task 2: Login form UI

- Create: `src/apps/cli/src/ui/login_form.rs`
- Modify: `src/apps/cli/src/ui/mod.rs`
- [ ] Full-viewport form with three empty fields + Login button
- [ ] Password masked as `*`
- [ ] Up/Down/Tab focus; Enter advances or submits

### Task 3: Wire startup + chat

- Modify: `src/apps/cli/src/ui/startup.rs`, `src/apps/cli/src/ui/chat/*`, `src/apps/cli/src/modes/chat.rs`
- [ ] `/login` and palette open the form; no raw-mode toggle
- [ ] Submit calls `login_with_credentials`; errors stay on form

### Task 4: Verify

- [ ] `cargo check -p bitfun-cli`
- [ ] Rebuild Ubuntu `~/bitfun-build/target/debug/bitfun-cli`
