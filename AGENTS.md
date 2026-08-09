[中文](AGENTS-CN.md) | **English**

# AGENTS.md

BitFun is a Rust workspace plus React frontends.

Repository rule: **keep product logic platform-agnostic, then expose it through platform adapters**.

## Quick start

1. Read `README.md` and `CONTRIBUTING.md` before architecture-sensitive changes.
2. Use the primary product loop below for normal development. Surface-specific
   alternatives belong in the nearest app guide.
3. After Rust file changes, prefer `pnpm run fmt:rs` to format only changed or staged `.rs` files. Use `cargo fmt` only when you intentionally want broader formatting coverage.
4. After changes, use the nearest local `AGENTS.md` for the focused verification
   command. The repository-level verification section below only covers
   cross-cutting checks.
5. Workspace Rust dependencies own compatible versions, not broad capability
   unions. Each crate must select the dependency features it actually uses;
   keep test-only features in dev-dependencies and attach feature-gated service
   capabilities to the owning crate feature. `tokio/full` is forbidden in the
   root workspace and workspace members.

## Layered Module Index

Dependencies flow top to bottom. This table is the physical crate layout, not
the full conceptual architecture. For Product Surface / Product Assembly /
Product Feature / Agent Kernel / Execution / Extension / Cross-platform Adapter /
Stable Contracts and Security Control Plane boundaries, read
[`docs/architecture/product-architecture.md`](docs/architecture/product-architecture.md).
Keep crate dependencies inside each layer to the smallest set needed.

| # | Layer | Path | Owns | Modules / entries | Layer doc |
|---|---|---|---|---|---|
| 1 | Interfaces and entrypoints | `src/apps/*`, `src/web-ui`, `src/mobile-web`, `BitFun-Installer`, `tests/e2e`, `src/crates/interfaces` | Product hosts, commands, UI entrypoints, protocol interfaces, and cross-surface tests | desktop, CLI, server, relay, Web UI, mobile web, installer, E2E, `acp`, `sdk-host` | nearest local `AGENTS.md`; [interfaces](src/crates/interfaces/AGENTS.md) |
| 2 | Product assembly | `src/crates/assembly` | Compatibility exports, product capability selection, product-full wiring, immutable built-in Agent content, adapter/service registration, and ecosystem-neutral source coordination | `agent-content`, `core`, `external-sources`, `product-capabilities` | [AGENTS.md](src/crates/assembly/AGENTS.md) |
| 3 | Adapters | `src/crates/adapters` | AI/transport/WebDriver protocol adapters, external AI work source adapters (OpenCode/Claude Code/Codex), and external-provider translation | `agent-runtime-ipc`, `ai-adapters`, `opencode-adapter`, `claude-code-adapter`, `codex-adapter`, `static-hook-support`, `transport`, `webdriver` | [AGENTS.md](src/crates/adapters/AGENTS.md) |
| 4 | Services | `src/crates/services` | Reusable OS, filesystem, terminal, MCP, remote, git, watch, process, LSP plugin registry, session persistence primitives, MiniApp runtime IO, and network implementations | `services-core`, `services-integrations`, `miniapp-market-service`, `relay-service`, `page-function-runtime`, `terminal` | [AGENTS.md](src/crates/services/AGENTS.md) |
| 5 | Execution primitives | `src/crates/execution` | Portable agent, harness, stream, DeepReview policy/report, plugin runtime client, typed-service, tool-contract, tool-group, and tool-execution building blocks | `agent-runtime`, `agent-stream`, `tool-contracts`, `harness`, `plugin-runtime-client`, `runtime-services`, `tool-provider-groups`, `tool-execution`, `tool-call-jsonrepair` | [AGENTS.md](src/crates/execution/AGENTS.md) |
| 6 | Stable contracts and product domains | `src/crates/contracts` | Shared DTOs, event shapes, runtime ports, LSP protocol/plugin DTOs, and product domain contracts/policies | `core-types`, `events`, `runtime-ports`, `product-domains` | [AGENTS.md](src/crates/contracts/AGENTS.md) |

Boundary rules:

- Interfaces and app entrypoints expose selected product behavior; reusable behavior moves down.
- Assembly wires lower layers and selects product capability facts; it must not implement concrete adapter, OS, or service details.
- Product features assemble user-facing commands, UI contributions, settings, and default policy on top of kernel capabilities; long-running task, scheduler, permission, session/workspace, memory, DFX, hook, and event facts stay in Agent Kernel owners.
- Adapters translate protocols and external-provider shapes; they should not own product capability selection or reusable OS service behavior.
- Services implement reusable concrete OS, process, terminal, MCP, remote, git, filesystem, LSP plugin registry, and MiniApp runtime IO capabilities.
- External systems are boundary resources, not repository layers. Only registered adapters/services/app-local providers should call them; other layers consume ports and stable contracts.
- Execution crates are portable runtime building blocks, not host-specific or delivery-profile owners.
- Contracts stay behavior-light and must not depend upward.


## Common commands

Keep this list to stable repository entry points. Surface- and crate-specific
test commands belong in the nearest local `AGENTS.md` and must not be copied here.

```bash
# Setup and primary product loop
pnpm install
pnpm run desktop:dev               # full hot-reload: Vite HMR + Rust auto-rebuild & restart

# Repository checks
pnpm run fmt:rs                    # format only changed / staged Rust files
pnpm run check:repo-hygiene        # repository content and filename rules
pnpm run check:github-config       # GitHub workflow/configuration rules
pnpm run check:core-boundaries     # Cargo/module ownership boundaries
```

For Web UI, mobile, CLI, Desktop, Installer, packaging, and focused test
commands, use the nearest local guide. The full script registry remains in
[`package.json`](package.json).

## Global rules

### Process artifacts

- Do not add or update files under `docs/superpowers/**`. Keep temporary
  planning, design, and implementation-process artifacts local. Move durable
  architecture or feature facts into the existing document for that area, and
  put user-facing guidance in the owning app README.

### Internationalization

- Locale ids, aliases, fallback rules, and surface defaults are owned by
  `src/shared/i18n/contract/locales.json`. Run `pnpm run i18n:generate`
  after editing it.
- Shared stable labels live in
  `src/shared/i18n/resources/shared/<locale>/terms.json`; workflow copy stays
  in the owning product surface.
- Do not import Web UI locale resources into smaller product surfaces such as
  `src/mobile-web` or `BitFun-Installer`. See `docs/architecture/i18n.md`.
- Static self-contained pages may use generated page-scoped shared-term files;
  they must not import Web UI locale catalogs.
- Web UI loads only bootstrap namespaces eagerly; use `useI18n(namespace)` for
  route or feature copy and keep direct `i18nService.t(...)` calls in bootstrap
  namespaces.
- Use shared i18n formatting helpers for user-visible dates, times, and
  numbers instead of direct `Intl.*` or `toLocale*` calls.
- `pnpm run i18n:audit` enforces key/placeholder parity, direct static key
  existence, dynamic key source proofs, literal fallback and locale-format
  no-growth baselines, shared-term/l10n governance baselines, non-blocking
  same-text locale inventory, and the no-hardcoded-CJK source budget.

### Theme and color tokens

- Theme and color-token baselines are ratchet contracts, not editable test
  expectations. Do not make a failing theme audit pass by raising values in
  `scripts/theme-color-governance-baseline*.json`, loosening fixture/assertion
  counts, adding broad allowlist entries, or removing CI audit coverage.
- Lower theme baselines when measured debt is removed. If a change truly needs a
  new color or key, add the smallest owner contract and document why existing
  semantic, component, or specialized-domain tokens cannot cover it.
- For theme, CSS variable, widget payload, mobile, installer, or CLI/TUI color
  changes, run `pnpm run theme:color-audit:all`.

### Logging

Logs must be English-only, with no emojis.

- Frontend: [`src/web-ui/LOGGING.md`](src/web-ui/LOGGING.md)
- Backend: [`src/crates/LOGGING.md`](src/crates/LOGGING.md)

### Tauri commands

- Command names: `snake_case`
- TypeScript may wrap with `camelCase`, but invoke Rust with a structured `request`

```rust
#[tauri::command]
pub async fn your_command(
    state: State<'_, AppState>,
    request: YourRequest,
) -> Result<YourResponse, String>
```

```ts
await api.invoke('your_command', { request: { ... } });
```

### Platform boundaries

- Do not call Tauri APIs directly from UI components; go through the adapter/infrastructure layer.
- Desktop-only host adapters belong in `src/apps/desktop`, then flow through typed capability interfaces and, when event delivery is needed, the production transport adapter.
- In shared core, avoid host-specific APIs such as `tauri::AppHandle`; use shared abstractions such as `bitfun_events::EventEmitter`.

### Remote compatibility

- When adding features, consider remote workspace and remote control synchronization support from the start. Local-only behavior can silently leave remote scenarios incomplete.
- If a feature cannot reasonably support remote workspaces, gate it or show a clear unsupported-state message instead of letting it fail with a generic error.
- Every desktop Tauri command must declare its remote-workspace policy in
 `src/apps/desktop/src/api/remote_workspace_policy.rs`; the contract test there
 rejects new commands without an explicit policy and forbids growing the
 legacy-unaudited backlog.

### Agent loop behavior

- Do not add hard-coded limits or pattern checks to the agent loop as a first response to looping behavior, such as blocking repeated tool calls by string or count alone.
- Excessive hard-coding turns the agent loop into a brittle workflow engine. Investigate the root cause first: tool behavior, model interaction, session context packaging, prompt/tool schema design, or state synchronization issues.

### Agent hooks

- BitFun implements the Codex hook contract, so <https://learn.chatgpt.com/docs/hooks> is the reference for events, payload fields, and the decision schema. Do not fork that contract. [`docs/features/agent-hooks.md`](docs/features/agent-hooks.md) ([中文](docs/features/agent-hooks.zh-CN.md)) covers only the BitFun-specific parts — file locations, the `app.hooks` gates, and the deviations table — and must be updated whenever a deviation is added or closed.
- The portable engine (settings parsing, payload construction, process execution, decision merging) lives in `bitfun-agent-runtime::native_hooks`. `bitfun-core::native_hooks` owns config discovery, gating, and per-event dispatch helpers; dispatch sites call those helpers instead of executing hooks inline.
- Three separate things share the word "hook": these native user hooks, the internal compiled-in `post_call_hooks`, and the read-only external hook catalog of other AI applications (`external_hooks`). Keep them separate.

## Architecture

### Product architecture guardrails

For any `bitfun-core` decomposition, feature-boundary, dependency-boundary, or
Rust build-speed refactor, read both
[`docs/architecture/product-architecture.md`](docs/architecture/product-architecture.md)
and
[`docs/architecture/rust-build-dependency-boundaries.md`](docs/architecture/rust-build-dependency-boundaries.md)
before editing. Keep these files as entry points; put module-specific ownership
details in the nearest module `AGENTS.md`.

Repository-level decomposition rules:

- Do not confuse DTO/contract extraction with runtime owner migration.
- Product surfaces may diverge; share stable facts or ports, not UI, protocol,
  lifecycle, or platform implementation.
- Moving runtime ownership requires a reviewed port/provider design, old-path
  compatibility, behavior equivalence tests, and explicit confirmation when a
  behavior boundary could change.

For Agent Runtime deployment, multi-GUI/TUI/Remote instances, shared Session
control, or process-topology changes, also read
[`docs/architecture/agent-runtime-deployment-design.md`](docs/architecture/agent-runtime-deployment-design.md).
Do not key Rust Runtime or Node/Bun Plugin Host processes by client, workspace,
session, or plugin by default; use the responsible state module, execution and
security conditions, and measured capacity.

### CLI product-line guardrails

For CLI/TUI parity work, non-interactive output contracts, external config
imports, plugin management UX, CLI Agent behavior, or branded CLI distributions,
read [`docs/architecture/cli-product-line-design.md`](docs/architecture/cli-product-line-design.md)
and [`src/apps/cli/AGENTS.md`](src/apps/cli/AGENTS.md). Keep CLI/TUI presentation
in the app; move reusable product behavior through Product Assembly, Agent
Runtime, Tool/Harness, Runtime Services, or the existing extension boundaries.

### HarmonyOS PC CLI/TUI guardrails

For changes that affect HarmonyOS PC CLI/TUI support, also read
[`docs/architecture/platform-portability-design.md`](docs/architecture/platform-portability-design.md).
This is a future platform target, not implemented support. The product target is
the real PC system terminal; HAP, `hdc shell`, the phone Remote App, and remote
execution are not substitutes. Design each concrete adaptation as a separate
topic and keep the current mobile capability unchanged.

### Product customization guardrails

For product definitions, branded distributions, GUI/TUI layout selection,
bundled product extensions, or customization build tasks, read
[`docs/architecture/product-customization-blueprint.md`](docs/architecture/product-customization-blueprint.md).
Keep product customization separate from user runtime configuration and plugins.
GUI and TUI may share stable product facts, but not layout, component, theme-key,
keybinding, or renderer schemas. Product assembly results and layout selections
may carry a small immutable list of product identity, data-isolation, recovery,
upgrade-integrity, or legal protection IDs. They must not carry user/source-level
plugin policy, installation, activation, update, permission, or dynamic health state.
Product Profile, Brand Pack, GUI/TUI Surface Blueprint, and Resolved Product Manifest are retired
design terms, not current production objects. Do not create compatibility formats
for them; implement only the smallest product-definition and assembly-result fields
used by a real build and runtime consumer.

For OpenCode live configuration or plugin execution, also read
[`docs/architecture/extensions/opencode-extension-compatibility.md`](docs/architecture/extensions/opencode-extension-compatibility.md).
The current P0 adapter remains a managed-package/static-preview path until the matching
OC-R phase is implemented and verified. Do not extend the legacy managed-package
path as the target OpenCode runtime model, and do not treat a design target as an
already available capability.

### SDLC quality guardrails

For lifecycle evidence, gates, Artifact Graph, Project Profile, Deep Review
policy, OpenCode compatibility, or target-project governance changes, read
[`docs/sdlc-harness/README.md`](docs/sdlc-harness/README.md)
first, then [`docs/sdlc-harness/design.md`](docs/sdlc-harness/design.md). If
module boundaries or behavior change, follow the matching design under
`docs/sdlc-harness/architecture/` or `docs/sdlc-harness/features/`.

Do not hard-code BitFun repository assumptions as target-project rules; keep
quality protection behavior target-aware, evidence-backed, risk-tiered,
cost-aware, and auditable.

## Verification

Choose verification at the owner, not from a repository-wide test matrix:

1. Read the nearest local `AGENTS.md` and run its narrowest command that covers
   the changed behavior.
2. Prefer one package, one test target or module filter, and the minimum feature
   set. Do not use `product-full`, `all-features`, or a workspace-wide suite as a
   shortcut.
3. Run a repository check only when its contract changed: repository hygiene for
   layout/content rules, GitHub config for workflow changes, and core boundaries
   for Cargo features, dependency direction, or test-target layout.
4. Leave broad builds, workspace suites, packaging, and platform matrices to
   existing CI unless the change affects those paths or reproduces a CI failure.

If a module lacks a useful focused command, add it to that module's guide rather
than expanding this file. Do not pre-emptively align every module's test list;
document a command only when a real workflow needs it.

## Agent-doc priority

Prefer the nearest matching `AGENTS.md` / `AGENTS-CN.md` for the directory you are changing. If local guidance conflicts with this file, follow the more specific, nearer document.
