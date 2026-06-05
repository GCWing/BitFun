[中文](AGENTS-CN.md) | **English**

# Facade Layer

This layer owns compatibility exports and product assembly for legacy consumers.
It chooses product capabilities, delivery profiles, and provider registrations,
then wires lower layers together. It should not become the long-term owner of
new execution primitives, service implementations, product-domain policy, or
integration protocol behavior.

## Modules

| Crate | Responsibility | Local doc |
|---|---|---|
| `core` | Legacy `bitfun-core` facade, compatibility imports, and product-full assembly | [AGENTS.md](core/AGENTS.md) |

## Placement Rules

- Put old import compatibility, product-full wiring, and assembly shims here.
- Put provider selection and registration here when the decision is tied to a
  delivery profile or legacy `bitfun-core` compatibility.
- Move stable owner logic to `contracts`, `execution`, `services`, `product`, or
  `integrations` when a lower layer can own it.
- Preserve existing public import paths unless a migration explicitly removes
  them with compatibility notes and tests.
- Keep facade additions small and traceable; broad feature growth here is a sign
  that ownership has not been pushed down far enough.

## Dependency Boundaries

- `facade/core` may depend on lower owner layers to assemble the current product
  runtime.
- Facade may depend on integration adapters, but should not implement their
  protocol serialization, authentication, transport, or platform details.
- Avoid direct host APIs in facade code; Tauri support must remain feature-gated
  and should be owned by app or adapter code when possible.
- Product-facing protocol surfaces may call the facade, but low-level
  integration adapters must not. The facade should not absorb protocol
  implementation details.
