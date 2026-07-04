[中文](AGENTS-CN.md) | **English**

# Contracts Layer

This layer owns stable contracts and product-domain models that can be shared by
execution, services, adapters, assembly, and interfaces without pulling
implementation details upward.

## Modules

| Crate | Responsibility | Local doc |
|---|---|---|
| `core-types` | Shared DTOs, errors, session/surface data, LSP protocol/plugin DTOs, and small value types | [AGENTS.md](core-types/AGENTS.md) |
| `events` | Event payloads and emitter contracts | [AGENTS.md](events/AGENTS.md) |
| `product-domains` | Product domain DTOs, rules, policies, and narrow ports | [AGENTS.md](product-domains/AGENTS.md) |
| `runtime-ports` | Runtime-facing traits and ports used by owner crates | [AGENTS.md](runtime-ports/AGENTS.md) |

## Placement Rules

- Add a type here only when it is stable across more than one owner layer.
- Keep contracts behavior-light: validation helpers are acceptable; runtime,
  filesystem, network, UI, or platform behavior is not.
- Prefer narrow DTOs or traits over broad facade objects.
- If a type is only needed by one runtime or product crate, keep it with that
  crate until a second owner needs it.

## Dependency Boundaries

- This layer may depend on workspace primitives and other contract crates.
- It must not depend on `execution`, `services`, `adapters`, `assembly`,
  `interfaces`, `src/apps`, frontend packages, Tauri, or OS-specific adapters.
- New dependencies must stay minimal and justified by contract shape, not by
  implementation convenience.
