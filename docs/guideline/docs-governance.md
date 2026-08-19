# In-repo docs governance

Purpose: how to place, write, and index documentation inside the BitFun **code** repository.  
Scope: `docs/`, root `AGENTS` / `CONTRIBUTING`, and nearest module `AGENTS.md`.  
Status: stable (target layout fixed: `architecture/`, `guideline/`, `specs/`, `plans/`; non-code content in `bitfun_doc`)  
Authority language: Chinese — see [`docs-governance.zh-CN.md`](docs-governance.zh-CN.md). This file is the English summary for AI / ops readers.

Rules for a separate product/docs site (user manuals, onboarding guides) are out of scope.

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
- Do not add new files under retired paths such as `docs/superpowers/**`,
  `docs/features/**`, or `docs/development/**`.

## Split: code repo vs docs site

| Keep in code repo | Put in separate docs site |
|---|---|
| Boundaries and ops needed to change this codebase | User manuals, integration guides, external narratives |
| Architecture constraints, verification matrix, command catalog | Training, marketing, long prose weakly tied to implementation |
| In-progress / stable specs and implementation plans that evolve with PRs | Pure historical archive, deployment/operator setup guides |
| Nearest `AGENTS.md` / `LOGGING.md` | — |

Tracking in-progress specs and implementation plans is an intentional current
workflow policy. Ephemeral prompts, research scratch, review drafts, and
personal notes are not repository documentation: keep them untracked and use a
`.local.md` suffix when a local filename helps.

Performance reports, external research, and other user guides that are not
tied to in-repo product links live in the separate
[`bitfun_doc`](https://gitcode.com/BitFun-Platform/bitfun_doc) repository.
Feishu remote-connect setup and release signature verification stay in this
code repo under [`docs/guideline/feishu-bot-setup.md`](feishu-bot-setup.md)
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

Non-code-repo content (user guides, performance reports, external research) lives in the
separate [`bitfun_doc`](https://gitcode.com/BitFun-Platform/bitfun_doc) repository.

## Directory boundaries

| Directory | Must contain | Must not contain |
|---|---|---|
| `docs/architecture/` | Stable cross-module architecture boundaries, owner/dependency rules, accepted design authorities, ADRs | Implementation task lists, temporary review notes, user setup guides, benchmark dumps, module-local coding rules |
| `docs/guideline/` | Repository operations and code-change rules: commands, verification, host/platform constraints, logging, i18n operations, test-id policy, docs governance, self-governance notes | Product requirements, feature implementation plans, user manuals, stable product architecture duplicated from `architecture/` |
| `docs/specs/` | Draft/in-progress specs, feature designs, stable single-feature designs | A second stable cross-cutting architecture authority, personal scratch files, generated evidence, user/operator guides, implementation task lists |
| `docs/plans/` | Independently executable implementation plans and `-completed.md` closeout records | Requirements, design bodies, personal scratch files, stable architecture |
| `docs/` root | `README.md` only; local untracked `*.local.md` scratch may exist in a developer workspace | Tracked topical articles, tracked `.local.md` files, duplicate indexes, generated output |

The nearest directory README owns the exact article list and local boundary.
Stable conclusions discovered in a spec move to the existing architecture authority;
the source document links to that authority instead of retaining a competing rule body.

## Two-hop index

```text
AGENTS.md  '‡ 搀椀爀攀挀琀漀爀礀 刀䔀䄀䐀䴀䔀 ⼀ 猀椀渀最氀攀 愀甀琀栀漀爀椀琀礀  鈀‡ ⠀愀琀 洀漀猀琀 漀渀攀 洀漀爀攀 栀漀瀀⤀ 戀漀搀礀਀怀怀怀਀਀ⴀ 䄀琀 洀漀猀琀 琀眀漀 栀漀瀀猀 昀爀漀洀 琀栀攀 洀愀琀挀栀椀渀最 攀渀琀爀礀⼀椀渀搀攀砀 琀漀 琀栀攀 愀甀琀栀漀爀椀琀愀琀椀瘀攀 戀漀搀礀⸀਀ⴀ 䔀瘀攀爀礀 洀愀椀渀琀愀椀渀攀搀 搀漀挀甀洀攀渀琀愀琀椀漀渀 搀椀爀攀挀琀漀爀礀 眀椀琀栀 洀甀氀琀椀瀀氀攀 愀爀琀椀挀氀攀猀 渀攀攀搀猀 愀਀  刀䔀䄀䐀䴀䔀 琀栀愀琀 猀琀愀琀攀猀 猀挀漀瀀攀Ⰰ 攀砀挀氀甀猀椀漀渀猀Ⰰ 愀渀搀 愀 挀漀洀瀀氀攀琀攀 愀爀琀椀挀氀攀 椀渀搀攀砀⸀਀ⴀ 䔀瘀攀爀礀 渀漀渀ⴀ琀攀洀瀀氀愀琀攀 最漀瘀攀爀渀攀搀 瀀愀最攀 洀甀猀琀 栀愀瘀攀 愀琀 氀攀愀猀琀 漀渀攀 椀渀戀漀甀渀搀 椀渀搀攀砀 漀爀 琀愀猀欀਀  爀漀甀琀攀⸀ 一攀眀 瀀愀最攀猀 愀渀搀 爀攀渀愀洀攀猀 甀瀀搀愀琀攀 琀栀攀 渀攀愀爀攀猀琀 椀渀搀攀砀 椀渀 琀栀攀 猀愀洀攀 挀栀愀渀最攀⸀਀ⴀ 䠀漀琀 猀椀渀最氀攀 瀀愀最攀猀 洀愀礀 戀攀 氀椀渀欀攀搀 搀椀爀攀挀琀氀礀 昀爀漀洀 䄀䜀䔀一吀匀 ⠀攀⸀最⸀ 怀瀀爀漀搀甀挀琀ⴀ愀爀挀栀椀琀攀挀琀甀爀攀⸀洀搀怀Ⰰ 怀瘀攀爀椀昀椀挀愀琀椀漀渀⸀洀搀怀⤀⸀਀ⴀ 䤀渀搀攀砀攀猀 挀漀渀琀愀椀渀 爀漀甀琀椀渀最 猀甀洀洀愀爀椀攀猀 漀渀氀礀⸀ 吀栀攀礀 洀甀猀琀 渀漀琀 昀漀爀欀 渀漀爀洀愀琀椀瘀攀 戀漀搀椀攀猀⸀਀਀⌀⌀ 䰀愀渀最甀愀最攀਀਀簀 䬀椀渀搀 簀 䰀愀渀最甀愀最攀 簀 䈀椀氀椀渀最甀愀氀 簀਀簀ⴀⴀⴀ簀ⴀⴀⴀ簀ⴀⴀⴀ簀਀簀 䠀甀洀愀渀ⴀ昀愀挀椀渀最 渀愀爀爀愀琀椀瘀攀 簀 䌀栀椀渀攀猀攀 愀甀琀栀漀爀椀琀礀 簀 䔀渀最氀椀猀栀 渀漀琀 爀攀焀甀椀爀攀搀 戀礀 搀攀昀愀甀氀琀 簀਀簀 刀漀漀琀 怀䄀䜀䔀一吀匀怀 ⼀ 怀䌀伀一吀刀䤀䈀唀吀䤀一䜀怀 簀 ᐀†簀 䈀漀琀栀 爀攀焀甀椀爀攀搀㬀 洀甀猀琀 猀琀愀礀 椀渀 猀礀渀挀 簀਀簀 䄀䤀 ⼀ 挀漀搀攀ⴀ挀栀愀渀最攀 漀瀀猀 挀漀渀猀琀爀愀椀渀琀猀 ⠀攀⸀最⸀ 怀最甀椀搀攀氀椀渀攀⼀⨀怀Ⰰ 洀漀搀甀氀攀 怀䄀䜀䔀一吀匀怀⤀ 簀 䔀渀最氀椀猀栀 愀甀琀栀漀爀椀琀礀 簀 䌀栀椀渀攀猀攀 挀漀瀀礀 渀漀琀 爀攀焀甀椀爀攀搀 戀礀 搀攀昀愀甀氀琀 簀਀簀 䰀漀最猀 簀 䔀渀最氀椀猀栀 漀渀氀礀 簀 一漀 䌀栀椀渀攀猀攀Ⰰ 渀漀 戀椀氀椀渀最甀愀氀 氀漀最猀 簀਀਀⌀⌀ 䘀漀爀洀愀琀਀਀ⴀ 倀愀最攀 栀攀愀搀攀爀㨀 瀀甀爀瀀漀猀攀Ⰰ 猀挀漀瀀攀Ⰰ 猀琀愀琀甀猀 ⠀怀搀爀愀昀琀怀⼀怀猀琀愀戀氀攀怀⤀Ⰰ 愀甀琀栀漀爀椀琀礀 氀愀渀最甀愀最攀Ⰰ 爀攀氀愀琀攀搀 氀椀渀欀猀⸀਀ⴀ 䰀椀渀欀 愀甀琀栀漀爀椀琀椀攀猀㬀 搀漀 渀漀琀 瀀愀猀琀攀 氀漀渀最 戀漀搀椀攀猀 椀渀琀漀 椀渀搀攀砀攀猀⸀਀ⴀ 䘀椀氀攀渀愀洀攀猀㨀 䔀渀最氀椀猀栀 欀攀戀愀戀ⴀ挀愀猀攀⸀਀ⴀ 伀爀搀椀渀愀爀礀 氀漀挀愀氀攀 瀀愀椀爀猀 甀猀攀 怀㰀渀愀洀攀㸀⸀洀搀怀 愀渀搀 怀㰀渀愀洀攀㸀⸀稀栀ⴀ䌀一⸀洀搀怀⸀ 刀漀漀琀 愀渀搀 洀漀搀甀氀攀਀  猀琀愀渀搀愀爀搀猀 攀渀琀爀礀瀀漀椀渀琀猀 欀攀攀瀀 琀栀攀 爀攀瀀漀猀椀琀漀爀礀 挀漀渀瘀攀渀琀椀漀渀 怀䄀䜀䔀一吀匀⸀洀搀怀 ⼀਀  怀䄀䜀䔀一吀匀ⴀ䌀一⸀洀搀怀㬀 琀栀攀 爀漀漀琀 挀漀渀琀爀椀戀甀琀椀漀渀 瀀愀椀爀 爀攀洀愀椀渀猀 怀䌀伀一吀刀䤀䈀唀吀䤀一䜀⸀洀搀怀 ⼀਀  怀䌀伀一吀刀䤀䈀唀吀䤀一䜀开䌀一⸀洀搀怀⸀਀ⴀ 匀琀愀渀搀愀氀漀渀攀 椀洀瀀氀攀洀攀渀琀愀琀椀漀渀 瀀氀愀渀猀 攀渀搀 椀渀 怀ⴀ瀀氀愀渀⸀洀搀怀㬀 挀氀漀猀攀漀甀琀 爀攀挀漀爀搀猀 攀渀搀 椀渀਀  怀ⴀ挀漀洀瀀氀攀琀攀搀⸀洀搀怀⸀਀਀⌀⌀ 匀瀀攀挀 ⼀ 䐀攀猀椀最渀 ⼀ 倀氀愀渀਀਀ⴀ 匀瀀攀挀猀 愀渀搀 搀攀猀椀最渀猀 ⠀眀栀愀琀 ☀ 眀栀礀⤀㨀 嬀怀搀漀挀猀⼀猀瀀攀挀猀⼀刀䔀䄀䐀䴀䔀⸀洀搀怀崀⠀⸀⸀⼀猀瀀攀挀猀⼀刀䔀䄀䐀䴀䔀⸀洀搀⤀਀ⴀ 匀瀀攀挀 琀攀洀瀀氀愀琀攀猀㨀 嬀怀搀漀挀猀⼀猀瀀攀挀猀⼀琀攀洀瀀氀愀琀攀猀⼀怀崀⠀⸀⸀⼀猀瀀攀挀猀⼀琀攀洀瀀氀愀琀攀猀⼀⤀਀ⴀ 䤀洀瀀氀攀洀攀渀琀愀琀椀漀渀 瀀氀愀渀猀 愀渀搀 挀氀漀猀攀漀甀琀 ⠀栀漀眀 ☀ 眀栀攀渀⤀㨀 嬀怀搀漀挀猀⼀瀀氀愀渀猀⼀怀崀⠀⸀⸀⼀瀀氀愀渀猀⼀⤀਀ⴀ 倀氀愀渀 琀攀洀瀀氀愀琀攀㨀 嬀怀搀漀挀猀⼀瀀氀愀渀猀⼀琀攀洀瀀氀愀琀攀猀⼀怀崀⠀⸀⸀⼀瀀氀愀渀猀⼀琀攀洀瀀氀愀琀攀猀⼀⤀਀਀⌀⌀ 刀漀漀琀 攀渀琀爀礀瀀漀椀渀琀猀਀਀簀 䘀椀氀攀 簀 䰀漀挀愀琀椀漀渀 簀 刀漀氀攀 簀਀簀ⴀⴀⴀ簀ⴀⴀⴀ簀ⴀⴀⴀ簀਀簀 怀䄀䜀䔀一吀匀⸀洀搀怀 ⼀ 怀䄀䜀䔀一吀匀ⴀ䌀一⸀洀搀怀 簀 刀攀瀀漀 爀漀漀琀 簀 䌀漀搀攀ⴀ挀栀愀渀最攀 渀漀爀洀猀 攀渀琀爀礀㬀 瀀爀漀最爀攀猀猀椀瘀攀 搀椀猀挀氀漀猀甀爀攀 簀਀簀 怀䌀伀一吀刀䤀䈀唀吀䤀一䜀⸀洀搀怀 ⼀ 怀䌀伀一吀刀䤀䈀唀吀䤀一䜀开䌀一⸀洀搀怀 簀 刀攀瀀漀 爀漀漀琀 簀 䠀漀眀 栀甀洀愀渀猀 挀漀渀琀爀椀戀甀琀攀㬀 氀椀渀欀 挀漀洀洀愀渀搀猀⼀瘀攀爀椀昀椀挀愀琀椀漀渀㬀 氀椀渀欀 䄀䜀䔀一吀匀 昀漀爀 渀漀爀洀猀 簀਀਀䌀爀漀猀猀ⴀ氀椀渀欀 戀漀琀栀⸀ 䌀伀一吀刀䤀䈀唀吀䤀一䜀 洀甀猀琀 渀漀琀 欀攀攀瀀 愀 琀栀椀爀搀 昀甀氀氀 挀漀洀洀愀渀搀 攀渀挀礀挀氀漀瀀攀搀椀愀⸀਀਀⌀⌀ 刀攀氀愀琀攀搀਀਀ⴀ 䌀漀洀洀愀渀搀猀㨀 嬀怀挀漀洀洀漀渀ⴀ挀漀洀洀愀渀搀猀⸀洀搀怀崀⠀挀漀洀洀漀渀ⴀ挀漀洀洀愀渀搀猀⸀洀搀⤀਀ⴀ 嘀攀爀椀昀椀挀愀琀椀漀渀㨀 嬀怀瘀攀爀椀昀椀挀愀琀椀漀渀⸀洀搀怀崀⠀瘀攀爀椀昀椀挀愀琀椀漀渀⸀洀搀⤀਀ⴀ 䐀攀瘀攀氀漀瀀洀攀渀琀 椀渀搀攀砀㨀 嬀怀刀䔀䄀䐀䴀䔀⸀洀搀怀崀⠀刀䔀䄀䐀䴀䔀⸀洀搀⤀਀ⴀ 䐀漀挀甀洀攀渀琀愀琀椀漀渀 洀愀瀀㨀 嬀怀搀漀挀猀⼀刀䔀䄀䐀䴀䔀⸀洀搀怀崀⠀⸀⸀⼀刀䔀䄀䐀䴀䔀⸀洀搀⤀਀ⴀ 一漀爀洀猀 攀渀琀爀礀㨀 嬀怀䄀䜀䔀一吀匀⸀洀搀怀崀⠀⸀⸀⼀⸀⸀⼀䄀䜀䔀一吀匀⸀洀搀⤀ ⼀ 嬀怀䄀䜀䔀一吀匀ⴀ䌀一⸀洀搀怀崀⠀⸀⸀⼀⸀⸀⼀䄀䜀䔀一吀匀ⴀ䌀一⸀洀搀⤀਀ⴀ 䌀漀渀琀爀椀戀甀琀椀渀最㨀 嬀怀䌀伀一吀刀䤀䈀唀吀䤀一䜀⸀洀搀怀崀⠀⸀⸀⼀⸀⸀⼀䌀伀一吀刀䤀䈀唀吀䤀一䜀⸀洀搀⤀ ⼀ 嬀怀䌀伀一吀刀䤀䈀唀吀䤀一䜀开䌀一⸀洀搀怀崀⠀⸀⸀⼀⸀⸀⼀䌀伀一吀刀䤀䈀唀吀䤀一䜀开䌀一⸀洀搀⤀਀
