# Docs layout migration map (docs-0818)

Purpose: old path → new path for the GCWing docs-governance alignment, following
OpenBitFun commit `ac9b9ff8c` (four buckets + merge sdlc-harness + externalize non-code docs).

## Core layout

| Old path | New path |
|---|---|
| `docs/development/*` | `docs/guideline/*` |
| `docs/features/*` | `docs/specs/*` |
| `docs/superpowers/specs/*` | `docs/specs/*` |
| `docs/superpowers/plans/*` | `docs/plans/*` (`-plan` suffix) |

## sdlc-harness merge

| Old path | New path |
|---|---|
| `docs/sdlc-harness/design.md` | `docs/architecture/sdlc-governance-architecture.md` |
| `docs/sdlc-harness/architecture/*` | `docs/architecture/*` (flat) |
| `docs/sdlc-harness/features/*` | `docs/specs/*` (`opencode-compatibility` → `opencode-compatibility-sdlc.md`) |
| `docs/sdlc-harness/product-requirements.md` | `docs/specs/sdlc-governance-product-requirements.md` |
| `docs/sdlc-harness/product-requirements-agent-workflow-adjustment.md` | `docs/specs/sdlc-governance-agent-workflow-adjustment.md` |
| `docs/sdlc-harness/traceability-matrix.md` | `docs/specs/sdlc-governance-traceability-matrix.md` |
| `docs/sdlc-harness/governance/metrics-spec.md` | `docs/specs/sdlc-governance-metrics-spec.md` |
| `docs/sdlc-harness/governance/self-governance-notes.md` | `docs/guideline/self-governance-notes.md` |
| `docs/sdlc-harness/implementation-plan.md` | `docs/plans/sdlc-governance-implementation-plan.md` |
| `docs/sdlc-harness/agent-workflow-staged-plan.md` | `docs/plans/sdlc-governance-agent-workflow-staged-plan.md` |
| `docs/sdlc-harness/README.md` | folded into `sdlc-governance-architecture.md` (removed) |
| `docs/sdlc-harness/research/*` | externalized to `bitfun_doc` (removed from code repo) |

## Externalized to bitfun_doc (removed from code repo)

| Old path | External |
|---|---|
| `docs/verify-downloads*.md` | **returned to code repo** as `docs/guideline/verify-downloads*.md` (linked from root README) |
| `docs/performance/*` | `bitfun_doc` / 技术调研/性能/ |
| `docs/remote-connect/feishu-bot-setup*.md` | **returned to code repo** as `docs/guideline/feishu-bot-setup*.md` (product link in `RemoteConnectDialog.tsx`) |
| other `docs/remote-connect/*` (if any) | historically mirrored under `bitfun_doc` / 开发指南/远程连接/ |

Product links: root README verify → `docs/guideline/verify-downloads*.md`; Feishu setup → `docs/guideline/feishu-bot-setup*.md`.
