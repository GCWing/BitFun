[中文](AGENTS-CN.md) | **English**

# Product Layer

This layer owns product-domain facts and capability composition that are not tied
to a UI surface, app process, external protocol, or platform adapter.

## Modules

| Crate | Responsibility | Local doc |
|---|---|---|
| `product-domains` | Product-owned domains such as MiniApp and function-agent domain contracts | [AGENTS.md](product-domains/AGENTS.md) |
| `product-capabilities` | Capability packs, delivery-profile facts, tool provider groups, and harness registry facts | [AGENTS.md](product-capabilities/AGENTS.md) |

## Placement Rules

- Put product concepts, capability facts, and domain policies here when they are
  shared by multiple delivery forms.
- Keep UI copy, route state, protocol adapters, Tauri commands, and OS service
  implementations out of product crates.
- Product capabilities may describe required runtime services and tool packs,
  but should not instantiate concrete service implementations.
- When a product rule needs platform data, depend on a contract or service API;
  do not directly read host state from product code.

## Dependency Boundaries

- Product crates may depend on `contracts` and selected `runtime` facts needed
  to describe capabilities.
- Product crates must not depend on `facade/core`, `src/apps`, frontend code, or
  Tauri.
- Avoid dependencies on `services` unless the product crate owns the domain type
  required by that service; prefer keeping concrete behavior in services.
