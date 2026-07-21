[中文](AGENTS-CN.md) | **English**

# Service Layer

This layer owns reusable concrete implementations that touch local systems or
runtime infrastructure: filesystem, git, file watch, terminal, MCP, LSP plugin
registry, remote connectivity, process lifecycle, session persistence primitives, MiniApp concrete runtime IO, and similar
OS/network capabilities.

## Modules

| Crate | Responsibility | Local doc |
|---|---|---|
| `services-core` | Reusable local service primitives, managed process-tree lifecycle, filesystem helpers, LSP plugin registry rules, session storage layout/indexing/deletion, metadata store CRUD/index rebuild, metadata construction/counter/index/field mutation/lineage rules, and JSON file IO without product assembly decisions | [AGENTS.md](services-core/AGENTS.md) |
| `services-integrations` | Concrete MCP, git, remote, file-watch, MiniApp runtime, review-platform provider service, product-domain port implementations, and platform-neutral Remote Connect primitives | [AGENTS.md](services-integrations/AGENTS.md) |
| `relay-service` | Reusable Remote Connect relay state, storage, and HTTP/WebSocket routes shared by standalone and embedded hosts | [AGENTS.md](relay-service/AGENTS.md) |
| `page-function-runtime` | Embedded JS Page Function runtime (rquickjs) for BitFun Pages | [AGENTS.md](page-function-runtime/AGENTS.md) |
| `terminal` | PTY, shell integration, and terminal session infrastructure | [AGENTS.md](terminal/AGENTS.md) |

## Placement Rules

- Put concrete OS, process, filesystem, git, terminal, MCP, LSP registry, remote SSH,
  file-watch, MiniApp runtime IO, and network service implementations here.
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
