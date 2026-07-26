[中文](AGENTS-CN.md) | **English**

# Interface Layer

This layer owns Rust protocol or host-facing entrypoints that expose assembled
product behavior. UI apps and delivery hosts remain under `src/apps`,
`src/web-ui`, `src/mobile-web`, and `BitFun-Installer` with their nearest local
`AGENTS.md`.

## Modules

| Crate | Responsibility | Local doc |
|---|---|---|
| `acp` | Agent Client Protocol interface over the assembled product runtime | [AGENTS.md](acp/AGENTS.md) |
| `sdk-host` | Versioned local Agent SDK Host protocol and connection use cases; process bootstrap and stdio framing remain in `src/apps/sdk-host` | — |

## Placement Rules

- Put protocol entrypoints here when they depend on `assembly/core` or an
  assembled product profile.
- Keep transport/protocol adapters in `adapters`.
- Keep reusable OS, filesystem, terminal, MCP, remote, and git implementations
  in `services`.

## Dependency Boundaries

- Interface crates may depend on `assembly/core` to expose a selected delivery
  profile.
- The portable `sdk-host` protocol crate is narrower: it depends only on stable
  Runtime/contracts and must not depend on `bitfun-core`, `terminal-core`,
  concrete services, the SDK Host app, or CLI. Concrete Host assembly stays in
  `src/apps/sdk-host`.
- Interface crates must not own product policy, reusable services, protocol
  transport internals, or execution primitives.
