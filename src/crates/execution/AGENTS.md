[中文](AGENTS-CN.md) | **English**

# Execution Primitives Layer

This layer owns reusable agent, tool, harness, stream, and typed-service
execution primitives. It is not the complete Agent Runtime SDK and not the
assembled product runtime. Product assembly decides which primitives, tool
packs, harness providers, and service providers are active for a delivery form.

## Modules

| Crate | Responsibility | Local doc |
|---|---|---|
| `agent-runtime` | Agent registry, scheduler, prompt cache, hooks, goals, and runtime control contracts | [AGENTS.md](agent-runtime/AGENTS.md) |
| `agent-stream` | Provider stream normalization and stream replay contracts | [AGENTS.md](agent-stream/AGENTS.md) |
| `agent-tools` | Tool contracts, execution gates, input validation, and result presentation contracts | [AGENTS.md](agent-tools/AGENTS.md) |
| `harness` | Harness workflow contracts and registry primitives | [AGENTS.md](harness/AGENTS.md) |
| `runtime-services` | Typed runtime service assembly and service availability facts | [AGENTS.md](runtime-services/AGENTS.md) |
| `tool-packs` | Tool provider group facts and product-full tool-pack composition | [AGENTS.md](tool-packs/AGENTS.md) |
| `tool-runtime` | Low-level file/search/tool IO helpers | [AGENTS.md](tool-runtime/AGENTS.md) |

## Placement Rules

- Put portable execution orchestration, agent lifecycle contracts, tool
  contracts, and provider-neutral execution facts here.
- Keep concrete filesystem, git, terminal, MCP server, remote SSH, and OS
  behavior in `services` unless the code is a pure low-level tool primitive.
- Keep product feature selection and delivery-profile decisions in `product` or
  `facade`, not in execution primitives.
- Tool packs should describe provider groups and required services; concrete
  service access should flow through ports or typed runtime services.

## Dependency Boundaries

- Execution primitive crates may depend on `contracts` and narrowly scoped
  integration DTOs when needed for provider stream normalization.
- Execution primitive crates must not depend on `facade/core`, `src/apps`,
  frontend code, Tauri APIs, or product-surface lifecycle.
- Any new dependency on `services`, `product`, or `integrations` needs an
  explicit boundary reason in the nearest module doc or PR description.
