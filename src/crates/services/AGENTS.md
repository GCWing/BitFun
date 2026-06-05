[中文](AGENTS-CN.md) | **English**

# Services Layer

This layer owns reusable non-UI service implementations and service adapters:
filesystem, git, process/system, diagnostics, terminal, MCP, remote, and
persistence-adjacent capabilities. Generic services should be callable through
narrow APIs or ports rather than through product facades. Product-specific
adapters may implement product-domain ports, but must not own product policy.

## Modules

| Crate | Responsibility | Local doc |
|---|---|---|
| `services-core` | Core reusable services for filesystem, diff, diagnostics, session usage, token usage, system, and process concerns | [AGENTS.md](services-core/AGENTS.md) |
| `services-integrations` | Concrete integrations for announcement, file watch, function agents, git, MCP, remote connect, and remote SSH | [AGENTS.md](services-integrations/AGENTS.md) |
| `terminal` | Terminal API, PTY, shell integration, and persistent terminal sessions | [AGENTS.md](terminal/AGENTS.md) |

## Placement Rules

- Put concrete host/service behavior here when it is reusable by more than one
  product or runtime path.
- Keep UI state, product feature selection, and delivery assembly out of this
  layer.
- Prefer small service APIs over broad managers that mix unrelated concerns.
- If behavior depends on platform capabilities, isolate those details behind a
  service module or feature gate.

## Dependency Boundaries

- Generic service crates should not depend on product crates.
- A service adapter may depend on narrowly scoped `product-domains` port/DTO
  types only when it implements a product-owned port behind a feature gate.
  Current example: `services-integrations` implements function-agent Git
  snapshots for `product-domains` function-agent ports. Do not generalize this
  into service-owned product policy.
- Services must not depend on `facade/core`, `src/apps`, frontend code, or
  Tauri `AppHandle`.
- Remote and platform support must fail through typed service errors or clear
  unsupported-state handling, not generic string failures.
