[中文](AGENTS-CN.md) | **English**

# Service Layer

This layer owns reusable concrete implementations that touch local systems or
runtime infrastructure: filesystem, git, file watch, terminal, MCP, remote
connectivity, process lifecycle, and similar OS/network capabilities.

## Modules

| Crate | Responsibility | Local doc |
|---|---|---|
| `services-core` | Reusable local service primitives without product assembly decisions | [AGENTS.md](services-core/AGENTS.md) |
| `services-integrations` | Concrete MCP, git, remote, file-watch, and product-domain port implementations | [AGENTS.md](services-integrations/AGENTS.md) |
| `terminal` | PTY, shell integration, and terminal session infrastructure | [AGENTS.md](terminal/AGENTS.md) |

## Placement Rules

- Put concrete OS, process, filesystem, git, terminal, MCP, remote SSH,
  file-watch, and network service implementations here.
- Implement `contracts`, `execution`, or `contracts/product-domains` ports here
  when the implementation needs concrete dependencies.
- Keep protocol/transport projection in `adapters`, and keep product capability
  selection in `assembly`.

## Dependency Boundaries

- Services may depend on `contracts` and narrowly on provider-neutral execution
  crates when implementing runtime ports.
- Services must not depend on `assembly/core`, interface crates, product UI code,
  or app command handlers.
- Service-to-service dependencies must stay narrow; reusable contracts should
  move to `contracts` or `execution` instead of creating broad coupling.
