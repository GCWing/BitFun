[中文](AGENTS-CN.md) | **English**

# Adapter Layer

This layer owns protocol, transport, external-provider, and host-facing adapter
crates. Adapters translate between product/runtime contracts and concrete
protocols; they should not become owners of product policy or reusable OS
services.

## Modules

| Crate | Responsibility | Local doc |
|---|---|---|
| `ai-adapters` | AI provider request/response adapters and stream protocol glue | [AGENTS.md](ai-adapters/AGENTS.md) |
| `api-layer` | Backend API adapter surface shared by product hosts | [AGENTS.md](api-layer/AGENTS.md) |
| `opencode-adapter` | Fixture-only OpenCode config and local plugin source projection contracts | [AGENTS.md](opencode-adapter/AGENTS.md) |
| `transport` | Event transport emitters and host transport adapters | [AGENTS.md](transport/AGENTS.md) |
| `webdriver` | Embedded WebDriver protocol and browser automation adapter | [AGENTS.md](webdriver/AGENTS.md) |

## Placement Rules

- Put protocol serialization, transport projection, external provider request
  shaping, and host communication adapters here.
- Keep OS, filesystem, terminal, MCP, remote, git, and watch implementations in
  `services` unless the code is purely protocol translation.
- Keep delivery-profile selection and adapter registration in `assembly`.

## Dependency Boundaries

- Adapters may depend on `contracts`, `execution`, and narrowly on `services`
  when an adapter must expose a service capability through a protocol.
- Adapters must not depend on `assembly/core`, product UI code, app command
  handlers, or Tauri APIs unless the crate is explicitly feature-gated for that
  host boundary.
- Prefer stable contracts over adapter-to-adapter coupling. Cross-adapter
  dependencies require a clear boundary reason.
