# R-WF-15 rename legion to workflow wording (execution record)

> Branch: task/rwf15 | Base: main = 228dd7253 | Commit: `refactor(ui): rename legion to workflow wording` = 487d38813
> 2026-08-17 | Seventh CPO batch 7 workstation B

## Scope

1. i18n zh-CN/en/zh-TW: legion -> workflow (user-facing strings only)
2. Component copy all flows through i18n keys (CreateLegionPage/LegionCard/AgentsScene/GroupChatsSection menu) - no hardcoded copy found, nothing to change beyond locale values
3. Backend LegionPreset structure untouched (zero diff on *.rs)

## Files changed (6 modified + 1 added)

- src/web-ui/src/locales/zh-CN/scenes/agents.json
- src/web-ui/src/locales/zh-CN/settings/basics.json
- src/web-ui/src/locales/zh-TW/scenes/agents.json
- src/web-ui/src/locales/zh-TW/settings/basics.json
- src/web-ui/src/locales/en-US/scenes/agents.json
- src/web-ui/src/locales/en-US/settings/basics.json
- src/web-ui/src/test/i18n-legion-wording.test.ts (new: 3 zero-residual assertions)

Kept as structural identifiers: JSON keys (newLegion/legionsZone/legionPattern/legion), technical terms (LegionPreset/LegionControl).

## Verification (test-first)

- New test i18n-legion-wording.test.ts ran RED first (3 failed: zh-CN 8 / zh-TW 8 / en-US 14 hits)
- After implementation: 3 passed
- Related suites: scenes/agents + group-chats = 9 files / 43 tests green
- Full vitest: 502 files / 3697 tests green (first run had 4 RemoteConnectDialog file-level failures due to missing worktree node_modules links - @noble/curves unresolved; fixed by junction, unrelated to this change; main-repo baseline 501/3694 green too)
- Contract gates: tsc 0 errors / i18n:audit passed 0 warnings / appearance contract passed (287 surfaces) / eslint 0 errors

## Acceptance assertions

- Frontend i18n zh-CN/zh-TW "军团/軍團": zero hit in src/web-ui/src (only the test file references the term itself) OK
- en-US "legion" case-insensitive: zero hit in user-visible copy (only JSON keys remain) OK
- Backend LegionPreset present (team_presets.rs:15) and zero diff on *.rs OK

## S-85

Working tree clean after commit; diff symmetric 42+/42-; no CJK comments; backend untouched.
