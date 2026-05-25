# Intent Record

## Metadata

- Task: Fix Monaco-related Vitest gap exposed by pre-merge verification
- Date: 2026-05-25
- Owner: Coding Agent
- Status: Accepted

## Original User Request

Continue implementing the intent-aligned Coding Agent workflow in BitFun.

## Agent Understanding

Broader verification exposed a full web test failure in `EventHandlerModule.test.ts`: the test imports a flow-chat event module that eventually resolves `MonacoThemeSync`, causing Vite/Vitest to resolve `monaco-editor` in a Node test environment. This slice should fix the test isolation gap without changing product runtime behavior.

## In Scope

- Add a focused test mock so `EventHandlerModule.test.ts` does not import Monaco theme synchronization.
- Rerun the previously failing test.
- Rerun full web tests if the focused test passes.
- Run web lint/type-check and workflow structure check.

## Out of Scope

- No product runtime change.
- No Monaco package or dependency changes.
- No broad Vitest config rewrite unless a focused test mock is insufficient.

## Acceptance Criteria

- `EventHandlerModule.test.ts` no longer fails on `monaco-editor` resolution.
- Full web test suite passes.
- Web lint and type-check pass.
- `pnpm run agent:check` passes after Evidence Package creation.

## Risk Level

- Level: L2
- Reason: Test infrastructure gap in shared frontend, with full web suite verification.
- Risk factors: Test mocks can accidentally hide meaningful behavior if too broad.
- Verification expectation: Focused failing test, full web tests, lint, type-check, workflow checker.
- Review escalation: Not required.

## Accepted Checks

- [x] Focused failing test passes.
- [x] Full web test suite passes.
- [x] Web lint/type-check pass.
- [x] Workflow structure check passes.

## Accepted Tests

- `pnpm --dir src/web-ui run test:run src/flow_chat/services/flow-chat-manager/EventHandlerModule.test.ts`
- `pnpm --dir src/web-ui run test:run`
- `pnpm run lint:web`
- `pnpm run type-check:web`
- `pnpm run agent:check`

## Acceptance Coverage Plan

- Automated: focused test, full web tests, lint, type-check, workflow checker.
- Manual: inspect mock scope to ensure it only isolates Monaco theme sync.
- Coverage gaps: none expected for this test-gap slice.

## Clarification Questions

No blocking question. Assumption: a focused test mock is preferred over changing product imports or Vite config.

## User Confirmations

- User asked to continue after pre-merge verification reported the web test gap.

## Provenance Anchors

- Context inputs: `src/web-ui/AGENTS.md`, failing Vitest output, `EventHandlerModule.test.ts`, `ThemeService.test.ts`.
- User decisions: Continue toward final MVP completion.
- Related change notes: `.agent/changes/intent-coding-rollout.md`.

## Execution Contract

Agent must:

- Keep the fix test-only unless evidence shows product config is actually broken.
- Keep mock scope narrow.
- Rerun the relevant frontend verification.

Agent must not:

- Modify Monaco dependencies.
- Hide unrelated failing tests.
- Change runtime theme behavior.

## Metrics

- intent_created: true
- questions_asked: 0
- tests_or_checks_created: 4 checks, 5 verification commands
- verification_passed: true
- rework_needed: false
