# Contributing

[中文版](./CONTRIBUTING_CN.md)

Thanks for your interest in BitFun! BitFun is a multi-platform AI programming environment powered by Rust and TypeScript, with shared core logic across Desktop/CLI/Server. This guide explains how to contribute effectively.

## Code of Conduct

Be respectful, kind, and constructive. We welcome contributors of all backgrounds and experience levels.

## Quick Start

### Prerequisites

- Node.js 22.12+ (LTS recommended)
- pnpm 10.15.0 via Corepack
- Rust toolchain (install via [rustup](https://rustup.rs/))
- [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/) for desktop development

BitFun standardizes local JavaScript builds and CI on Node.js 22.12+. The GitHub
Actions upgrades in this repository use Node.js 24-compatible action runtimes,
but project scripts should run on Node.js 22.12+ unless a narrower local guide
says otherwise. After switching from an older Node.js version, rerun
`pnpm install`.

#### Windows: OpenSSL Setup

Most Windows contributors do not need to configure OpenSSL manually. Use
`pnpm run desktop:dev` or the normal `desktop:build*` scripts; they bootstrap a
pre-built OpenSSL package when needed.

Only handle OpenSSL yourself when the bootstrap fails, you are preparing CI, or
you intentionally use `pnpm run desktop:dev:raw`. In that case, run
`scripts/ci/setup-openssl-windows.ps1`, or set `OPENSSL_DIR` to a pre-built x64
OpenSSL directory and set `OPENSSL_STATIC=1`.

### Install dependencies

```bash
pnpm install
```

### Common commands

```bash
# Desktop (recommended for daily development)
pnpm run desktop:dev                # full hot-reload: Vite HMR + Rust auto-rebuild & restart

# Desktop (lightweight preview, no Rust auto-rebuild)
pnpm run desktop:preview:debug      # reuse pre-built binary + Vite HMR; Rust changes require manual restart

# Desktop (production build)
pnpm run desktop:build

# E2E
pnpm run e2e:test
```

> **`desktop:dev` vs `desktop:preview:debug`**: `desktop:dev` runs `tauri dev`, which provides **full hot-reload** — frontend changes apply instantly via Vite HMR, and Rust/backend changes trigger an incremental rebuild followed by an automatic app restart. This is the recommended workflow for active development. `desktop:preview:debug` launches a pre-built debug binary alongside a Vite dev server; frontend edits still get HMR, but **Rust-side changes are not auto-rebuilt** — you must stop and re-run the command (or use `--force-rebuild`). Use `desktop:preview:debug` when you only need to iterate on frontend code or want a faster cold-start without waiting for `tauri dev` initialization.

> For the full script list, see [`package.json`](package.json). For agent-specific commands, verification, and architecture rules, see [`AGENTS.md`](AGENTS.md).

### Desktop debugging tools

Desktop dev builds enable the `devtools` Cargo feature. Use `F12` for native
webview DevTools. `Cmd/Ctrl + Shift + I` toggles the BitFun element inspector,
and `Cmd/Ctrl + Shift + J` also opens native DevTools. These tools are disabled
in end-user `release` builds.

## Code Standards and Architecture Constraints

Use [`AGENTS.md`](AGENTS.md) as the canonical source for architecture-sensitive
rules, module boundaries, and the verification matrix. In contributor-facing
terms:

- Logs are English-only and should stay useful, not noisy.
- User-visible copy should use the project i18n flow; do not share Web UI
  locale catalogs with smaller surfaces.
- Shared core must stay platform-agnostic. Desktop/Tauri details belong in app
  adapters and flow through typed capability interfaces; use the production
  transport adapter when event delivery is needed.
- Tauri commands use `snake_case` command names and structured `request`
  payloads.
- Product architecture, feature-boundary, dependency-boundary, and build-speed
  work must follow `docs/architecture/product-architecture.md`.
- Feature-specific rules belong in the nearest module `AGENTS.md`.

## Key Contribution Focus Areas

1. Contribute good ideas/creativity (features, interactions, visuals, etc.) by opening issues
   > Product managers and UI designers are welcome to submit ideas quickly via PI. We will help refine them for development.
2. Improve the Agent system and overall quality
3. Improve system stability and strengthen foundational capabilities
4. Expand the ecosystem (Skills, MCP, LSP plugins, or better support for domain-specific development scenarios)

## Contribution Workflow and PR Expectations

### What to Contribute (Beyond Features and Fixes)

We welcome contributions beyond standard feature or bug-fix PRs. Examples include:

| Contribution area | Location / files | Example |
| --- | --- | --- |
| Prompts | `src/crates/assembly/core/src/agentic/agents/prompts/` | Add or refine prompts, and update related logic as needed |
| Tools | `src/crates/assembly/core/src/agentic/tools/implementations/`, `src/crates/assembly/core/src/agentic/tools/registry.rs` | Add tool implementations and register them in the tool registry |
| Subagents | `src/crates/assembly/core/src/agentic/agents/custom_subagents/`, `src/crates/assembly/core/src/agentic/agents/registry.rs` | Add subagent implementations and register them in the subagent registry |
| Mode contributions | `src/crates/assembly/core/src/agentic/agents/*_mode.rs`, `src/crates/assembly/core/src/agentic/agents/prompts/*_mode.md`, `src/web-ui/src/locales/*/settings/modes.json` | Add/improve agent modes (e.g. Plan/Debug/Agentic or custom modes) and keep prompts + UI copy in sync |
| Scenario guides for Code Agent and AIIde | `website/src/docs/` | Add workflows, playbooks, and real-world scenario docs (or link them from `README.md`) |

### Before you start

- Open an issue to describe the problem or proposal, especially for larger changes, to avoid duplication and design conflicts
- For new features or UI changes, discuss the design direction early to ensure it fits the product experience
- Use the issue and PR templates as a guide. Keep the PR focused and explain any skipped verification when it matters.

### PR title and description

We recommend using Conventional Commits for clearer history and better automation:

- `feat:` new feature
- `fix:` bug fix
- `docs:` documentation
- `chore:` maintenance/deps
- `refactor:` refactor without behavior change
- `test:` tests

UI changes should include before/after screenshots or a short recording for fast review.

If your work is AI-assisted, please note it in the PR and indicate testing level (untested/lightly tested/fully tested) to help reviewers assess risk.

Do not commit transient AI prompts, local absolute paths, generated scratch files, pairing secrets, tokens, certificates, or unrelated artifacts. Keep the PR focused on the intended product or maintenance change.

### Branch management

**The `main` branch is the default collaboration branch and accepts feature PRs.** Since this repo encourages product managers and developers to use AI-generated code for rapid validation or idea submission, **please open all PRs targeting the `main` branch**.

### Scope

Keep PRs small and focused. Avoid bundling unrelated changes.

## Testing and Verification

Run the smallest checks that match the changed files and behavior. CI covers
full builds and broad test suites; local prechecks should stay focused unless
the change affects build, packaging, release behavior, or a path CI cannot
protect.

Common local checks:

| Change type | Typical verification |
| --- | --- |
| Repository metadata or GitHub config | `pnpm run check:repo-hygiene && pnpm run check:github-config && git diff --check` |
| Frontend runtime or UI | `pnpm run type-check:web`, plus the nearest focused test when behavior changed |
| Mobile web | `pnpm --dir src/mobile-web run type-check` |
| Rust shared runtime or services | `cargo check --workspace`, plus a focused `cargo test` when behavior changed |
| Desktop/Tauri integration | `cargo check -p bitfun-desktop` |
| i18n resources or contract | use the matching i18n row in `AGENTS.md` |

For UI changes, include screenshots or a short recording when helpful. If you
cannot run a relevant check, explain why in the PR and provide a lower-risk
manual verification path.

## Security and Compliance

- Do not commit secrets, tokens, certificates, or any sensitive data
- When adding dependencies, ensure license compatibility and explain the purpose

## Thanks

Every contribution matters. Issues, PRs, and suggestions are all welcome!

## Taiji Module

This repository also hosts the **Taiji** multi-agent quant system under `src/crates/taiji/`.

### Crate layout

| Category | Count | Crates |
| --- | --- | --- |
| Active | 20 | `taiji-bar`, `taiji-cli`, `taiji-engine`, `taiji-engine-py`, `taiji-content`, `taiji-publisher`, `taiji-growth`, `taiji-alert`, `taiji-knowledge-graph`, `taiji-blog-gen`, `taiji-example`, `taiji-llm`, `taiji-backtest`, `taiji-executor`, `taiji-realtime`, `taiji-pattern`, `taiji-abnormal`, `taiji-sentiment`, `taiji-orderflow`, `taiji-strategen` |
| Closed-source (commented out) | 4 | `taiji-dvmi`, `taiji-magnet`, `taiji-thrust`, `taiji-risk` |

### Closed-source policy

The 4 closed-source crates listed above are **commented out** in `Cargo.toml` workspace members. **Do not uncomment or submit closed-source code to the public repository.** CI test jobs also exclude these crates.

### Before you start

- Read `docs/architecture/product-architecture.md` for architecture-sensitive rules and module boundaries.
- Familiarize yourself with the taiji crate dependency matrix: see `.bitfun/team/type-contract-phase8-10.md` for inter-crate dependencies and merge rules.

### Verification

Run per-crate checks before committing taiji changes:

```bash
cargo check -p <crate>
cargo test -p <crate>
```

For workspace-wide taiji verification, use:

```bash
cargo check --workspace
cargo test --workspace
```

Keep PRs focused on one functional area and note any skipped verification with a reason.
