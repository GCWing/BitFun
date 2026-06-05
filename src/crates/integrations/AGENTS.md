[中文](AGENTS-CN.md) | **English**

# Concrete Provider Adapters Layer

This layer owns low-level external protocol, provider, transport, and platform
adapters. Protocol surfaces that expose assembled product behavior belong in
`src/crates/surfaces`, not here.

## Modules

| Crate | Responsibility | Local doc |
|---|---|---|
| `ai-adapters` | AI provider DTOs and provider-facing adapter helpers | [AGENTS.md](ai-adapters/AGENTS.md) |
| `api-layer` | Platform-agnostic API handlers over transport abstractions | [AGENTS.md](api-layer/AGENTS.md) |
| `transport` | Cross-platform communication adapters and emitters | [AGENTS.md](transport/AGENTS.md) |
| `webdriver` | Embedded WebDriver protocol/runtime implementation | [AGENTS.md](webdriver/AGENTS.md) |

## Placement Rules

- Put protocol/framework/provider adapters here when their primary job is to
  translate between BitFun contracts and an external system.
- Low-level adapters should depend on contracts or narrow execution facts, not
  product assembly.
- Do not place product policy, reusable service implementation, or agent/tool
  orchestration here.

## Dependency Boundaries

- Integrations may depend on `contracts`, execution facts, and narrowly scoped
  provider dependencies.
- Integrations must not depend on `facade/core`; move product-facing protocol
  entrypoints to `src/crates/surfaces`.
- Platform-specific dependencies must be optional or isolated when possible so
  smaller delivery forms are not forced to compile unrelated adapters.
