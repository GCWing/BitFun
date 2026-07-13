# BitFun CLI Agent Guide

Scope: this guide applies to `src/apps/cli`.

Read [`docs/architecture/cli-product-line-design.md`](../../../docs/architecture/cli-product-line-design.md),
[`docs/architecture/product-architecture.md`](../../../docs/architecture/product-architecture.md), and the
matching runtime or plugin design before architecture-sensitive CLI changes.

## Ownership

- This app owns Clap commands, TUI state and rendering, terminal input/lifecycle,
  CLI-local settings, structured output projection, and user-facing CLI diagnostics.
- Shared session, turn, task, tool, permission, context, checkpoint, Subagent,
  Harness, MCP, plugin, and capability facts belong to their runtime owners.
- Existing `bitfun-core/product-full` compatibility paths may remain during a
  reviewed migration. Do not add new concrete managers, global mutable services,
  or CLI-only copies of shared product behavior.

## Product and extension boundaries

- Assemble CLI behavior through `DeliveryProfile::Cli`, capability plans, typed
  services, and capability availability. Hiding a command is not a backend
  capability restriction.
- Product names, logos, theme resources, data namespaces, bundled extensions,
  and update channels come from a validated Product Profile, Resolved Product
  Manifest, or generated resources. Do not add new hard-coded branding or
  source-rewrite scripts. Runtime capability hiding does not prove code was
  physically removed from an artifact.
- External OpenCode, Codex, or Claude configuration enters through a dry-run
  import adapter. Do not copy credentials, treat external config as live BitFun
  state, or silently ignore unsupported fields.
- Keep native instruction references, config candidates, managed Skill/plugin
  content, and credentials as separate asset classes. Config approval must not
  activate executable content or establish plugin trust.
- CLI plugin screens consume capability services, read-only status, and typed
  diagnostics. They must not depend on Plugin Runtime Host ABI or raw ecosystem
  payloads.
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
input, session control, config import, plugin management, or Product Profile
behavior changes. Theme/color changes require `pnpm run theme:color-audit:all`.
Packaging or branding changes require the CLI package smoke path and a clean-tree
two-profile build assertion.
