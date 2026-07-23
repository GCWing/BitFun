# BitFun CLI Agent Guide

Scope: this guide applies to `src/apps/cli`.

Read [`docs/architecture/cli-product-line-design.md`](../../../docs/architecture/cli-product-line-design.md),
[`docs/architecture/product-architecture.md`](../../../docs/architecture/product-architecture.md), and
[`docs/architecture/product-customization-blueprint.md`](../../../docs/architecture/product-customization-blueprint.md)
before product-definition, TUI layout, branding, packaging, runtime, or plugin architecture changes.

## Ownership

- This app owns Clap commands, TUI state and rendering, terminal input/lifecycle,
  CLI-local settings, structured output projection, and user-facing CLI diagnostics.
- Peer Device Mode **host** support lives in `src/peer_host/`: after `/login`
  (same Auth Server / Username / Password flow and `~/.bitfun` session/hint
  files as Desktop), device routing stays up so Desktop controllers can
  HostInvoke this process. CLI is not a Peer Mode controller. Same-machine
  Desktop+CLI share one `device_id`; last AuthConnect wins.
- Shared session, turn, task, tool, permission, context, checkpoint, Subagent,
  Harness, MCP, plugin, and capability facts belong to their runtime owners.
- Existing `bitfun-core/product-full` compatibility paths may remain during a
  reviewed migration. Do not add new concrete managers, global mutable services,
  or CLI-only copies of shared product behavior.

## Product and extension boundaries

- Assemble CLI behavior through `DeliveryProfile::Cli`, capability plans, typed
  services, and capability availability. Hiding a command is not a backend
  capability restriction.
- The target CLI consumes product identity, theme resources, data namespaces,
  bundled product extensions, update channels, and TUI layout IDs from the
  validated product assembly result. Resolved Product Manifest and TUI Blueprint
  are retired design terms, not migration inputs. Do not read authoring product
  definitions at runtime, add hard-coded branding/source
  rewrites, or treat user plugins as product assembly inputs. Runtime capability
  hiding does not prove code was physically removed.
- Product assembly may expose only the immutable protection IDs allowed by the
  customization design. CLI must not turn them into user/source plugin policy or
  store plugin activation, update, permission, or health state in the assembly result.
- OpenCode Prompt Commands from standard user and project configuration are
  read-only live sources. CLI may execute only the expanded prompt through the
  existing agent owner; it must re-confirm changed conflict participants and
  must not execute shell/file directives that the prompt-command contract marks
  unsupported.
- OpenCode standalone JavaScript tools may execute only through the shared
  external-source approval, conflict, Tool Runtime, and script-worker owners.
  CLI/TUI consumes typed snapshots and actions; it must not import modules,
  spawn tool workers, bypass a pending decision, or implement a second approval
  store. TypeScript, dependency loading, package plugins, and hooks remain
  non-executable until their own reviewed capability slice lands.
- OpenCode external subagents may execute only through the shared source
  decision and existing Subagent owner. TUI consumes typed summaries and
  generation-checked actions; it must not parse agent files, inject source
  prompts directly, invent model fallbacks, or offer follow-up for the current
  fresh single-run compatibility slice.
- The managed-package OpenCode adapter remains a static-preview path. Other
  OpenCode plugin capabilities, Codex, and Claude remain import/reference sources
  unless their own reviewed adapter design explicitly changes. Never copy
  credentials or silently ignore unsupported fields.
- Keep native instruction references, explicit import records, executable plugin
  sources, and credentials as separate asset classes. Importing non-executable
  config must not establish executable-source policy. CLI consumes the external
  source status and typed actions; it must not add another activation layer on top
  of the source/target decision or claim that post-import confirmation can undo
  candidate-module side effects.
- CLI plugin screens consume capability services, read-only status, and typed
  diagnostics. They must not depend on Plugin Runtime Host ABI or raw ecosystem
  payloads.
- Non-interactive commands return `action-required` only when the current operation
  actually depends on a pending external asset. Unrelated confirmations remain in
  structured status or `stderr` summaries and must not block the command.
- External ACP agents, external config import, and managed plugins are separate
  capabilities with separate trust and lifecycle state.

## TUI and automation

- Keep terminal session restore, event normalization, state transitions, effects,
  command dispatch, and rendering independently testable. Reducers and views do
  not perform filesystem, network, config, or Agent operations directly.
- Slash commands, palette actions, and root CLI commands should map to the same
  stable capability requests instead of reimplementing behavior per entrypoint.
- `json` is one result document; `stream-json` is one complete event per line.
  Keep protocol stdout free of logs and preserve schema/exit-code compatibility.
- Keep `src/modes/exec.rs` as the stable module facade. The current private split
  keeps lifecycle/event settlement in `exec/lifecycle.rs` and Patch capture/write
  behavior in `exec/patch.rs`; further private splits are allowed when they keep
  one executor, one output schema, and one lifecycle owner.
- Approval policy is invocation-scoped: interactive TUI defaults to ask;
  non-interactive execution fails when confirmation is required unless an
  explicit argument or managed policy approves it. Do not mutate a global
  confirmation flag to implement an entrypoint default.
- Shell shortcuts, file references, background work, compact, checkpoint, and
  rewind must use shared Tool/Agent Runtime, permission, cancellation, artifact,
  and audit paths.
- Always restore raw mode, alternate screen, mouse capture, and paste mode after
  normal exit, cancellation, initialization failure, or panic.

## Verification

Run the smallest checks matching the change:

```bash
cargo check -p bitfun-cli
cargo test -p bitfun-cli
```

Also run focused protocol/PTY tests when structured output, terminal lifecycle,
input, session control, config import, plugin management, or product assembly
behavior changes. Theme/color changes require `pnpm run theme:color-audit:all`.
Packaging or branding changes require the CLI package smoke path and a clean-tree
two-product build assertion.

## Install for end users

Use [`install.ps1`](install.ps1), [`install.sh`](install.sh), and [`README.md`](README.md) for
platform-native per-user installation. Document `bitfun` as primary; ship `bitfun-cli` only as the
deprecated compatibility entrypoint, and use `bitfun` in all new examples and integrations.
