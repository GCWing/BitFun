[中文](AGENTS-CN.md) | **English**

# Product Assembly Layer

This layer owns product assembly, compatibility exports, capability selection,
and runtime registration. It wires lower layers together for a delivery form;
it does not own concrete adapter behavior, reusable service implementations, OS
integration, or stable product-domain contracts.

## Modules

| Crate | Responsibility | Local doc |
|---|---|---|
| `core` | `bitfun-core` compatibility facade and product-full assembly | [AGENTS.md](core/AGENTS.md) |
| `external-sources` | Ecosystem-neutral lifecycle owner: capability-specific coordinators plus shared bounded discovery lanes | inherited |
| `product-capabilities` | Product capability profiles, tool group facts, service requirements, and harness selections | [AGENTS.md](product-capabilities/AGENTS.md) |

## Placement Rules

- Put product-full wiring, compatibility shims, capability profile selection,
  and adapter/service registration here.
- Keep product-domain rules in `contracts/product-domains`; assembly may select
  those facts but must not become their owner.
- Move stable owner logic to `contracts`, portable execution logic to
  `execution`, concrete protocol adaptation to `adapters`, and reusable
  implementation behavior to `services` when a lower layer can own it.
- Preserve existing public import paths unless a migration explicitly removes
  them with compatibility notes and tests.
- Keep assembly additions small and traceable; broad feature growth here is a
  sign that ownership has not been pushed down far enough.
- Keep external-source capability payloads in their typed contracts and owners.
  `ExternalSourceControlPlane` may share scheduling, generation fencing, and
  provider isolation, but must not become a generic asset registry or a second
  product-state owner.

## Dependency Boundaries

- `assembly/core` may depend on lower owner layers to assemble the current product
  runtime.
- Assembly crates must not depend on `src/apps/*`. Embedded-relay product
  orchestration depends on the narrow `EmbeddedRelayHost` capability; Desktop
  owns its TCP binding, static fallback, and task lifecycle. Do not move those
  concrete details back into assembly or treat the Desktop implementation as a
  reusable product surface.
- Assembly may depend on adapter and service crates for selected delivery forms,
  but should not implement their protocol serialization, authentication,
  transport, or platform details.
- Avoid direct host APIs in assembly code; Tauri support must remain feature-gated
  and should be owned by app or adapter code when possible.
- Interface crates may call assembly APIs, but adapters and services must not
  depend on assembly.
