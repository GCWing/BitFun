# Evidence Package

## Metadata

- Task: Fix Monaco-related Vitest gap exposed by pre-merge verification
- Date: 2026-05-25
- Risk Level: L2
- Status: Complete

## Intent Record

`.agent/intents/intent-20260525-monaco-vitest-gap.md`

## Summary

Fixed the full web test failure caused by Vitest resolving real Monaco modules in a Node test environment. Added a test-only `monaco-editor` alias in `vite.config.ts` and a lightweight Monaco mock under `src/web-ui/src/test/`. The previously failing `EventHandlerModule.test.ts` now passes, and the full web test suite is green.

## Provenance Chain

- Original request: continue after pre-merge verification exposed a web test gap.
- Context inputs: failing Vitest output, `src/web-ui/AGENTS.md`, `EventHandlerModule.test.ts`, `vite.config.ts`, Monaco import paths.
- Intent Record: `.agent/intents/intent-20260525-monaco-vitest-gap.md`.
- Acceptance: focused failing test, full web tests, lint/type-check, workflow checker.
- Execution: added a test-only Monaco alias and mock; kept runtime Monaco behavior unchanged.
- Verification: focused test, full web test suite, lint, type-check, and workflow structure check passed.
- Repair loop: first focused mock exposed more Monaco import paths; switched to test-only alias for stable isolation.
- Review escalation: not required.
- Evidence Package: `.agent/evidence/evidence-20260525-monaco-vitest-gap.md`.

## Files Changed

- `.agent/intents/intent-20260525-monaco-vitest-gap.md`
- `.agent/evidence/evidence-20260525-monaco-vitest-gap.md`
- `src/web-ui/vite.config.ts`
- `src/web-ui/src/test/monaco-editor.mock.ts`
- `src/web-ui/src/flow_chat/services/flow-chat-manager/EventHandlerModule.test.ts`

## Verification

- `pnpm --dir src/web-ui run test:run src/flow_chat/services/flow-chat-manager/EventHandlerModule.test.ts`: passed, 19 tests
- `pnpm --dir src/web-ui run test:run`: passed, 147 test files and 771 tests
- `pnpm run lint:web`: passed
- `pnpm run type-check:web`: passed
- Workflow structure check: `pnpm run agent:check`: passed

## Repair Loop

- Failure classes: test environment/dependency resolution
- Repair attempts: 2
- Final repair status: complete
- Remaining verification gaps: none

## Risk Handling

- Final risk level: L2
- Risk factors: test alias could mask Monaco behavior if applied outside test mode.
- Verification matched expected level: yes.
- Skipped verification: full Rust workspace checks were already covered in the previous pre-merge verification slice.
- Review escalation: not required.

## Accepted Checks

- [x] Focused failing test passes.
- [x] Full web test suite passes.
- [x] Web lint/type-check pass.
- [x] Workflow structure check passes.

## Accepted Tests

- [x] `pnpm --dir src/web-ui run test:run src/flow_chat/services/flow-chat-manager/EventHandlerModule.test.ts`
- [x] `pnpm --dir src/web-ui run test:run`
- [x] `pnpm run lint:web`
- [x] `pnpm run type-check:web`
- [x] `pnpm run agent:check`

## Acceptance Coverage Result

- Automated: focused failing test, full web tests, lint, and type-check passed.
- Manual: reviewed alias condition so it only applies during Vitest/test mode.
- Coverage gaps: no product runtime Monaco test added; this slice fixes Node test isolation only.

## Risks

- The Monaco mock is intentionally lightweight and should not be used to validate editor behavior.
- Tests that genuinely exercise Monaco editor behavior should use browser/component infrastructure or explicit Monaco-aware setup.

## Human Review Focus

- Confirm the test-only alias in `vite.config.ts` is the preferred shared solution over per-test mocks.
- Confirm the Monaco mock surface is narrow enough for non-editor tests.

## Metrics

- intent_created: true
- questions_asked: 0
- tests_or_checks_created: 4 checks, 5 verification commands
- verification_passed: true
- rework_needed: false
