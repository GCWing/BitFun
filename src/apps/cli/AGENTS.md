# BitFun CLI Agent Guide

Scope: this guide applies to `src/apps/cli`.

Read [`docs/architecture/cli-product-line-design.md`](../../../docs/architecture/cli-product-line-design.md),
[`docs/architecture/product-architecture.md`](../../../docs/architecture/product-architecture.md), and
[`docs/architecture/product-customization-blueprint.md`](../../../docs/architecture/product-customization-blueprint.md)
before product-definition, TUI layout, branding, packaging, runtime, or plugin architecture changes.

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
- The target CLI consumes product identity, theme resources, data namespaces,
  bundled product extensions, update channels, and TUI layout IDs from the
  validated product assembly result. Existing Resolved Product Manifest and TUI
  Blueprint projections may remain only as reviewed migration inputs. Do not read
  authoring product definitions at runtime, add hard-coded branding/source
  rewrites, or treat user plugins as product assembly inputs. Runtime capability
  hiding does not prove code was physically removed.
- Product assembly may expose only the immutable protection IDs allowed by the
  customization design. CLI must not turn them into user/source plugin policy or
  store plugin activation, update, permission, or health state in the assembly result.
- Current OpenCode adapter code is dry-run/static preview only. After the matching
  OC-R phases are implemented, OpenCode standard config and plugin sources become
  read-only source files whose valid results can affect runtime behavior without
  a BitFun import or second activation. Codex and Claude remain import/reference
  sources unless their own design explicitly changes. Never copy credentials or
  silently ignore unsupported fields.
- Keep native instruction references, explicit import records, executable plugin
  sources, and credentials as separate asset classes. Importing non-executable
  config must not establish executable-source policy; live OpenCode execution is
  governed by the source/target policy resolved before module import.
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
input, session control, config import, plugin management, or product assembly
behavior changes. Theme/color changes require `pnpm run theme:color-audit:all`.
Packaging or branding changes require the CLI package smoke path and a clean-tree
two-product build assertion.
