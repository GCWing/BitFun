[中文](AGENTS-CN.md) | **English**

# AGENTS.md

BitFun is a Rust workspace plus React frontends.

Repository rule: **keep product logic platform-agnostic, then expose it through platform adapters**.

This file is the **formal standards entry** (progressive disclosure): indexes to authoritative
docs, plus the smallest always-on navigation. Open linked docs when the task matches; do not
copy long rule bodies back into this entry.

## Quick start

1. Read [`README.md`](README.md) and [`CONTRIBUTING.md`](CONTRIBUTING.md) before architecture-sensitive changes. Humans contributing via PR start at CONTRIBUTING; code-change norms start here.
2. Desktop: prefer `pnpm run desktop:dev`. Use `pnpm run desktop:preview:debug` only for faster frontend-only cold start (no Rust auto-rebuild). See [`docs/guideline/common-commands.md`](docs/guideline/common-commands.md).
3. After Rust edits: `pnpm run fmt:rs` (changed/staged `.rs` only). Use `cargo fmt` only when you intentionally want broader formatting coverage.
4. Use **Route by task** / **Standards map**, then pick checks from [`docs/guideline/verification.md`](docs/guideline/verification.md).
5. Root workspace dependencies own compatible versions; consuming crates select only the features they use. Keep test-only features in `dev-dependencies`, attach feature-gated service capabilities to the owning crate feature, and disable third-party defaults in `[workspace.dependencies]` when they are not part of every consumer's contract. Internal crates whose guarded `default` is empty do not repeat `default-features = false` on every edge; narrow consumers of an intentional compatibility default such as ACP still disable it explicitly. Manifests copied into a standalone Docker build context keep explicit versions and default policy. `tokio/full` is forbidden in the root workspace and workspace members.

## How to use this file

1. Prefer the nearest module `AGENTS.md` / `AGENTS-CN.md` when editing under that directory.
2. **Standards map** = norm types. **Architecture index** = STD-01 subtopics. **Cross-cutting index** = host/logging/agent-loop topics. **Route by task** = change → read → verify.
3. Open linked authorities for detail. Keep this file and [`AGENTS-CN.md`](AGENTS-CN.md) in sync.
4. Use [`docs/README.md`](docs/README.md) for the complete documentation map; placement and migration rules remain in [`docs-governance.md`](docs/guideline/docs-governance.md).

## Language (repo docs)

Summary only; full rules in [`docs/guideline/docs-governance.md`](docs/guideline/docs-governance.md).

| Kind | Language |
|---|---|
| Human-facing narrative | Chinese authority (English optional). Spec workflow index [`docs/specs/README.md`](docs/specs/README.md) is Chinese-authority. |
| Root `AGENTS` / `CONTRIBUTING` | Bilingual; **must stay in sync** |
| AI / code-change ops (`docs/guideline/*`, module `AGENTS`) | English authority |
| Logs | English only |

## Standards map

| ID | Norm type | Read when | Authority (start here) |
|---|---|---|---|
| STD-01 | Repository & architecture | Layers, dependencies, product-line boundaries | **Layered Module Index** + **Architecture index** → linked design docs |
| STD-02/03 | Coding & language stacks | Style beyond local AGENTS | Nearest module `AGENTS.md` |
| STD-04 | Frontend & interaction | UI, state, adapter, i18n, theme | i18n/theme: [`docs/architecture/i18n.md`](docs/architecture/i18n.md), ops guide [`docs/guideline/i18n.md`](docs/guideline/i18n.md), [`docs/architecture/theme-token-optimization.md`](docs/architecture/theme-token-optimization.md), ops [`docs/guideline/theme-color-tokens.md`](docs/guideline/theme-color-tokens.md); UI/state/adapter: nearest surface `AGENTS.md` (e.g. [`src/web-ui/AGENTS.md`](src/web-ui/AGENTS.md)) |
| STD-05 | API & data contracts | DTO, events, Tauri command, persistence, error **shape** | DTO/events/contracts: [`src/crates/contracts/AGENTS.md`](src/crates/contracts/AGENTS.md) and child-module `AGENTS.md`; Tauri/host/remote: [`docs/guideline/host-platform-and-remote.md`](docs/guideline/host-platform-and-remote.md) |
| STD-06 | DFX | Retry, cancel, partial success, heterogeneous inputs, failure **UX**, logging, security | Logging: [`docs/guideline/logging.md`](docs/guideline/logging.md), [`src/web-ui/LOGGING.md`](src/web-ui/LOGGING.md), [`src/crates/LOGGING.md`](src/crates/LOGGING.md); remote: [`host-platform-and-remote.md`](docs/guideline/host-platform-and-remote.md); Agent loop: [`agent-loop-behavior.md`](docs/guideline/agent-loop-behavior.md); security: [`SECURITY.md`](SECURITY.md) |
| STD-07 | Docs & templates | Spec / design / plan; where docs live | [`docs/guideline/docs-governance.md`](docs/guideline/docs-governance.md); [`docs/specs/README.md`](docs/specs/README.md) |
| STD-08 | Testing & verification | Which check to run after a change | [`docs/guideline/verification.md`](docs/guideline/verification.md) |
| STD-09 | Git & delivery | Branch, PR, contribute | [`CONTRIBUTING.md`](CONTRIBUTING.md) ([中文](CONTRIBUTING_CN.md)); do not duplicate command or verification encyclopedias in CONTRIBUTING |
| STD-10 | AI collaboration | Same norms across agents/tools | This entry + nearest module `AGENTS.md`; do not fork tool-only rule copies |
| STD-11 | Automated protection | Audits, baselines, boundary checks | i18n/theme audit authorities + [`docs/guideline/theme-color-tokens.md`](docs/guideline/theme-color-tokens.md) + [`docs/guideline/verification.md`](docs/guideline/verification.md); never raise baselines to silence failures |
| STD-12 | Module norms | Package/crate-local rules | Nearest `AGENTS.md` / `AGENTS-CN.md` |

**Also:** command dictionary → [`docs/guideline/common-commands.md`](docs/guideline/common-commands.md) (not a substitute for Verification; also linked from Quick start §2).

## Architecture index (STD-01)

When the task hits a row below, open that authority. Do not stop at the STD-01 map row alone.

| Topic | Open when | Authority |
|---|---|---|
| Product architecture | `bitfun-core` split, feature/dependency boundaries, build-speed refactors | [`docs/architecture/product-architecture.md`](docs/architecture/product-architecture.md) (see §1.1); Rust build dependencies: [`docs/architecture/rust-build-dependency-boundaries.md`](docs/architecture/rust-build-dependency-boundaries.md); topic map [`docs/architecture/README.md`](docs/architecture/README.md) |
| Agent Runtime deployment | Multi-GUI/TUI/Remote instances, shared Session control, process topology | [`docs/architecture/agent-runtime-deployment-design.md`](docs/architecture/agent-runtime-deployment-design.md) |
| Agent hooks | Native Codex-compatible hooks, BitFun deviations / gates | [`docs/specs/agent-hooks.md`](docs/specs/agent-hooks.md) ([中文](docs/specs/agent-hooks.zh-CN.md)); do not fork the Codex hook contract |
| Physical layers | Where a crate/app belongs, dependency direction | **Layered Module Index** in this file |
| CLI / TUI product line | CLI/TUI parity, non-interactive output, config import, plugin UX, CLI Agent, branded CLI | [`docs/architecture/cli-product-line-design.md`](docs/architecture/cli-product-line-design.md), [`src/apps/cli/AGENTS.md`](src/apps/cli/AGENTS.md) |
| HarmonyOS PC CLI/TUI | HarmonyOS PC terminal / CLI-TUI portability | [`docs/architecture/platform-portability-design.md`](docs/architecture/platform-portability-design.md) |
| Product customization | Product definition, branded distro, GUI/TUI layout selection, bundled extensions, customization builds | [`docs/architecture/product-customization-blueprint.md`](docs/architecture/product-customization-blueprint.md) |
| OpenCode compatibility | Live OpenCode config or plugin execution | [`docs/architecture/extensions/opencode-extension-compatibility.md`](docs/architecture/extensions/opencode-extension-compatibility.md) — **read current P0 runtime guardrail** (managed-package / static-preview; do not treat design targets as shipped) |
| SDLC quality harness | Lifecycle evidence, gates, Artifact Graph, Project Profile, Deep Review, target-project governance | [`docs/architecture/sdlc-governance-architecture.md`](docs/architecture/sdlc-governance-architecture.md); if module boundaries/behavior change, also matching docs under [`docs/architecture/`](docs/architecture/) (security-boundary, quality-data-plane, evidence-pack, artifact-graph, project-profile-integration, agent-workflow-design) or [`docs/specs/`](docs/specs/) (configurable-policy-profile, risk-classifier, pr-quality-gate, agent-evaluation, requirement-impact-analysis); do not hard-code BitFun-repo assumptions as target-project rules |

## Cross-cutting index

Condition-triggered rules. Open only when the task matches; details stay in the linked docs.

| Topic | Open when | Authority |
|---|---|---|
| Logging | Adding or changing log output / observability text | Repository-wide policy: [`docs/guideline/logging.md`](docs/guideline/logging.md); frontend API: [`src/web-ui/LOGGING.md`](src/web-ui/LOGGING.md); Rust API: [`src/crates/LOGGING.md`](src/crates/LOGGING.md) |
| Tauri / platform / remote / upgrade | Desktop commands, UI↔host boundaries, remote scenarios, and cross-version behavior | [`docs/guideline/host-platform-and-remote.md`](docs/guideline/host-platform-and-remote.md); [`src/apps/desktop/AGENTS.md`](src/apps/desktop/AGENTS.md) |
| Agent loop | Agent loop, repeated tool calls, anti-loop safeguards | [`docs/guideline/agent-loop-behavior.md`](docs/guideline/agent-loop-behavior.md); nearest `src/crates/execution/*/AGENTS.md` |

## Route by task

| Task / change | Read first | Then verify |
|---|---|---|
| Unsure where code belongs | Layered Module Index + Product architecture row | [`verification.md`](docs/guideline/verification.md) matching row |
| Desktop Tauri / desktop-only API | [`host-platform-and-remote.md`](docs/guideline/host-platform-and-remote.md); [`src/apps/desktop/AGENTS.md`](src/apps/desktop/AGENTS.md); `remote_workspace_policy.rs` | Desktop row in Verification |
| Shared Rust (assembly/adapters/services/execution/contracts) | Layered Module Index + nearest crate `AGENTS.md` | Shared Rust row in Verification |
| Web UI (no locale contract change) | [`src/web-ui/AGENTS.md`](src/web-ui/AGENTS.md); platform section in [`host-platform-and-remote.md`](docs/guideline/host-platform-and-remote.md) | Frontend row in Verification |
| i18n / locales | [`docs/architecture/i18n.md`](docs/architecture/i18n.md); ops [`docs/guideline/i18n.md`](docs/guideline/i18n.md) | Locale / i18n rows in Verification |
| Theme / color tokens | [`docs/architecture/theme-token-optimization.md`](docs/architecture/theme-token-optimization.md), ops [`docs/guideline/theme-color-tokens.md`](docs/guideline/theme-color-tokens.md) | `pnpm run theme:color-audit:all` |
| Logging / log message text | Cross-cutting → Logging | Focused check for the touched surface |
| Agent loop / anti-loop changes | [`agent-loop-behavior.md`](docs/guideline/agent-loop-behavior.md) | Nearest execution/runtime tests |
| Mobile web pairing / reconnect | [`src/mobile-web/AGENTS.md`](src/mobile-web/AGENTS.md) | Mobile web row in Verification |
| CLI / TUI / HarmonyOS PC / customization / OpenCode / SDLC | Matching **Architecture index** row | Smallest surface check + module AGENTS |
| Installer | [`BitFun-Installer/AGENTS.md`](BitFun-Installer/AGENTS.md) | Installer rows in Verification |
| Failure UX / provider errors / remote unsupported | STD-05/06 authorities; remote section in [`host-platform-and-remote.md`](docs/guideline/host-platform-and-remote.md) | Contract/focused tests for the touched surface |
| Writing Spec / design | [`docs/specs/README.md`](docs/specs/README.md) + [`templates/`](docs/specs/templates/); governance [`docs-governance.md`](docs/guideline/docs-governance.md) | Human review; cite applicable STD rows |
| Opening a PR / contribution process | [`CONTRIBUTING.md`](CONTRIBUTING.md) | Smallest Verification row for touched files |

## Layered Module Index

Dependencies flow top to bottom. This table is the **physical** crate layout, not the full conceptual architecture — see Product architecture in the Architecture index. Keep crate dependencies inside each layer to the smallest set needed.

| # | Layer | Path | Owns | Modules / entries | Layer doc |
|---|---|---|---|---|---|
| 1 | Interfaces and entrypoints | `src/apps/*`, `src/web-ui`, `src/mobile-web`, `BitFun-Installer`, `tests/e2e`, `src/crates/interfaces` | Product hosts, commands, UI entrypoints, protocol interfaces, and cross-surface tests | desktop, CLI, server, relay, Web UI, mobile web, installer, E2E, `acp`, `app-server`, `sdk-host` | nearest local `AGENTS.md`; [interfaces](src/crates/interfaces/AGENTS.md) |
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

## Agent-doc priority

Prefer the nearest matching `AGENTS.md` / `AGENTS-CN.md` for the directory you are changing. If local guidance conflicts with this file, follow the more specific nearer document.
