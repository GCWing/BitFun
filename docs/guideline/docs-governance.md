# In-repo docs governance

Purpose: how to place, write, and index documentation inside the BitFun **code** repository.
Scope: `docs/`, root `AGENTS` / `CONTRIBUTING`, and nearest module `AGENTS.md`.
Status: stable (target layout fixed: `architecture/`, `guideline/`, `specs/`, `plans/`).
Authority language: Chinese — see [`docs-governance.zh-CN.md`](docs-governance.zh-CN.md). This file is the English summary for AI / ops readers.

## Non-negotiable preservation rules

1. A documentation reorganization must preserve normative meaning. Splitting,
   merging, renaming, and re-indexing may change presentation, but must not
   change owners, requirements, current/target status, failure behavior, or
   acceptance criteria.
2. Before moving or renaming a page, inventory inbound references from Markdown,
   source code, configuration, tests, packaging, and product-facing URLs. Update
   every reference in the same change; do not leave compatibility to memory.
3. A page may leave the code repository only after proving that no code,
   runtime behavior, build/package step, test, or user-facing product link
   depends on its repository path. Otherwise retain it, or migrate the
   dependency and its tests together and verify the replacement URL is stable.
4. Preserve current/proposed/completed labels exactly when consolidating text.
   Moving a statement does not authorize changing its maturity or authority.
5. Record an old-to-new content map in the PR when a reorganization deletes or
   merges authorities. A link checker proves reachability, not semantic parity;
   human review remains required for the map.
6. If this repository enables product-level 4+1 / architecture doc check scripts, approved Authority migrations must update the check targets and related maps in the same change; do not only delete headings or treat a local L1 as product L0. If this repository does not yet ship `docs:architecture:check`, use manual review against the indexes and this guideline.

## Process artifacts

- Do not add ephemeral planning drafts under tracked `docs/`. Put durable
  specs/designs in `docs/specs/`, implementation plans in `docs/plans/`, and
  keep temporary process artifacts local (or untracked `*.local.md`). Move
  durable architecture facts into `docs/architecture/`, and put user-facing
  guidance in the owning app README.
- Do not add new authority content under retired paths such as
  `docs/superpowers/**`, `docs/features/**`, or `docs/development/**`. A minimal
  compatibility stub may remain only when a released product or durable public
  URL still targets the old path; it must link to the canonical page and be
  listed in `docs/README.md`.

## In-repo documentation scope

This code repository should track:

- Boundaries and ops needed to change this codebase
- Architecture constraints, verification matrix, command catalog
- In-progress / stable specs and implementation plans that evolve with PRs
- Indexed, durable research or technical audits that support a tracked spec or
  plan; these are non-normative references and must record their evidence date
- Nearest `AGENTS.md` / `LOGGING.md`

Tracking in-progress specs and implementation plans is an intentional current
workflow policy. Ephemeral prompts, research scratch, review drafts, and
personal notes are not repository documentation: keep them untracked and use a
`.local.md` suffix when a local filename helps.

Feishu remote-connect setup and release signature verification are explicit
code-coupled operational-guide exceptions. They stay in this code repo under
[`docs/guideline/feishu-bot-setup.md`](feishu-bot-setup.md)
([中文](feishu-bot-setup.zh-CN.md)) and
[`docs/guideline/verify-downloads.md`](verify-downloads.md)
([中文](verify-downloads.zh-CN.md)). `RemoteConnectDialog.tsx` and root
`README.md` link to those in-repo paths.

## Target `docs/` layout

```text
docs/
  README.md         # Directory map and placement router; no policy body
  architecture/     # Stable architecture; ADRs live here (no top-level ADR dir)
  guideline/        # Dev ops: commands, verification, host/remote, agent-loop, this doc
  specs/            # Specs + designs (what & why); see README index
    README.md
    templates/
  plans/            # Implementation plans + closeout (how & when)
    README.md
    templates/
```

These four directories are the only authoritative documentation buckets.
Compatibility stubs under retired paths own no content and only point to a
canonical page.

## Directory boundaries

| Directory | Must contain | Must not contain |
|---|---|---|
| `docs/architecture/` | Stable cross-module architecture boundaries, owner/dependency rules, accepted design authorities, ADRs | Implementation task lists, temporary review notes, user setup guides, benchmark dumps, module-local coding rules |
| `docs/guideline/` | Repository operations and code-change rules, plus explicitly indexed code-coupled operational guides | Product requirements, feature implementation plans, general user manuals, stable product architecture duplicated from `architecture/` |
| `docs/specs/` | Draft/in-progress specs, feature designs, stable single-feature designs, indexed non-normative research and technical audits | A second stable cross-cutting architecture authority, personal scratch files, raw generated evidence, user/operator guides, implementation task lists |
| `docs/plans/` | Independently executable implementation plans and `-completed.md` closeout records | Requirements, design bodies, personal scratch files, stable architecture |
| `docs/` root / retired paths | `README.md`, local untracked `*.local.md`, and indexed compatibility stubs only | New authority content, tracked topical articles, tracked `.local.md` files, duplicate indexes, generated output |

The nearest directory README owns the exact article list and local boundary.
Stable conclusions discovered in a spec move to the existing architecture authority;
the source document links to that authority instead of retaining a competing rule body.

## Two-hop index

```text
AGENTS.md  →  directory README / single authority  →  (at most one more hop) body
```

- At most two hops from the matching entry/index to the authoritative body.
- Every maintained documentation directory with multiple articles needs a README
  that states scope, exclusions, and a complete article index.
- Every governed page except templates must have at least one inbound index or
  task route; new/renamed pages must update the nearest index in the same change.
- Compatibility stubs are indexed separately in `docs/README.md` and contain
  only a canonical destination plus their compatibility reason.
- High-frequency single pages may be linked directly from AGENTS (for example
  `product-architecture.md`, `verification.md`).
- Indexes contain routing summaries only; do not fork normative bodies.

## Language

| Kind | Language | Bilingual |
|---|---|---|
| Human-facing narrative | Chinese authority | English not required by default |
| Root `AGENTS` / `CONTRIBUTING` | — | Both required; semantics must stay in sync |
| AI / code-change ops constraints (for example `guideline/*`, module `AGENTS`) | English authority | Chinese copy not required by default |
| Logs | English only | No Chinese or bilingual logs |

## Format

- Page header: purpose, scope, status (`draft`/`stable`/`reference`), authority language, related links.
- Link to authorities instead of pasting long bodies into indexes.
- Filenames: English kebab-case.
- Ordinary locale pairs use `<name>.md` and `<name>.zh-CN.md`. Root and module
  standards keep the repository convention `AGENTS.md` / `AGENTS-CN.md`; the root
  contribution pair remains `CONTRIBUTING.md` / `CONTRIBUTING_CN.md`.
- Standalone implementation plans end in `-plan.md`; closeout records end in `-completed.md`.

## Spec / Design / Plan

- Specs and designs (what & why): [`docs/specs/README.md`](../specs/README.md)
- Spec templates: [`docs/specs/templates/`](../specs/templates/)
- Implementation plans and closeout (how & when): [`docs/plans/`](../plans/)
- Plan templates: [`docs/plans/templates/`](../plans/templates/)

## Root entrypoints

| File | Location | Role |
|---|---|---|
| `AGENTS.md` / `AGENTS-CN.md` | Repository root | Code-change norms entry; progressive disclosure with outbound links |
| `CONTRIBUTING.md` / `CONTRIBUTING_CN.md` | Repository root | Human contribution flow; commands/verification link to `guideline/*`, norms link to AGENTS |

They cross-link each other; CONTRIBUTING must not maintain a third full command encyclopedia.

## Related

- Commands: [`common-commands.md`](common-commands.md)
- Verification: [`verification.md`](verification.md)
- Development docs index: [`README.md`](README.md)
- Documentation map: [`docs/README.md`](../README.md)
- Norms entry: [`AGENTS.md`](../../AGENTS.md) / [`AGENTS-CN.md`](../../AGENTS-CN.md)
- Contributing: [`CONTRIBUTING.md`](../../CONTRIBUTING.md) / [`CONTRIBUTING_CN.md`](../../CONTRIBUTING_CN.md)
