[中文](AGENTS-CN.md) | **English**

# Product Surfaces Layer

This layer owns Rust crates that expose assembled product behavior through an
external protocol or host-facing entrypoint. UI apps and delivery hosts still
keep their nearest local `AGENTS.md`.

## Modules

| Crate | Responsibility | Local doc |
|---|---|---|
| `acp` | Agent Client Protocol surface over the assembled product runtime | [AGENTS.md](acp/AGENTS.md) |

## Placement Rules

- Put protocol entrypoints here when they depend on `facade/core` or a product
  assembly plan.
- Keep low-level provider DTOs, transport emitters, and platform adapters in
  `integrations`.
- Keep reusable service implementations in `services`.

## Dependency Boundaries

- Surface crates may depend on `facade/core` to expose a selected delivery
  profile.
- Surface crates must not become owners of product policy, reusable services, or
  execution primitives.
