# Evidence Package

## Metadata

- Task: Run broader pre-merge verification for Intent Coding MVP
- Date: 2026-05-25
- Risk Level: L2
- Status: Complete with verification gap

## Intent Record

`.agent/intents/intent-20260525-intent-coding-premerge-verification.md`

## Summary

Ran broader pre-merge verification after the focused Intent Coding checks. Web lint passed and Rust workspace compilation passed. The full web test suite ran 147 files: 146 files passed, 752 tests passed, and 1 suite failed before running its tests due to an existing Vitest/Vite resolution path for `monaco-editor` through `EventHandlerModule.test.ts` and `MonacoThemeSync`. This failure is outside the Intent Coding MVP change surface, so it is recorded as a verification gap rather than repaired in this slice.

## Provenance Chain

- Original request: continue implementing the intent-aligned Coding Agent workflow in BitFun.
- Context inputs: repository verification table, web package scripts, Vitest output, Rust workspace check output.
- Intent Record: `.agent/intents/intent-20260525-intent-coding-premerge-verification.md`.
- Acceptance: web lint, full web tests, Rust workspace check, workflow checker.
- Execution: ran broader checks and investigated the full web test failure path.
- Verification: lint, Rust check, and workflow structure check passed; full web tests failed on `monaco-editor` resolution.
- Repair loop: failure classified and not repaired because it is outside the accepted Intent Coding scope.
- Review escalation: not required for L2.
- Evidence Package: `.agent/evidence/evidence-20260525-intent-coding-premerge-verification.md`.

## Files Changed

- `.agent/intents/intent-20260525-intent-coding-premerge-verification.md`
- `.agent/evidence/evidence-20260525-intent-coding-premerge-verification.md`

## Verification

- `pnpm run lint:web`: passed
- `pnpm --dir src/web-ui run test:run`: failed
  - 146 test files passed.
  - 752 tests passed.
  - 1 suite failed: `src/flow_chat/services/flow-chat-manager/EventHandlerModule.test.ts`.
  - Failure class: test environment/dependency resolution.
  - Failure detail: Vite failed to resolve package entry for `monaco-editor` imported by `src/web-ui/src/infrastructure/theme/integrations/MonacoThemeSync.ts`.
- `cargo check --workspace`: passed
- Workflow structure check: `pnpm run agent:check`: passed

## Repair Loop

- Failure classes: test environment/dependency resolution
- Repair attempts: 0
- Final repair status: not repaired in this slice
- Remaining verification gaps: full web test suite has one monaco resolution failure

## Risk Handling

- Final risk level: L2
- Risk factors: broader checks span web and Rust workspace surfaces.
- Verification matched expected level: partial; lint and Rust check passed, full web suite exposed an out-of-scope test environment failure.
- Skipped verification: full `cargo test --workspace` was not run.
- Review escalation: not required.

## Accepted Checks

- [x] Web lint passes.
- [ ] Full web tests pass.
- [x] Rust workspace check passes.
- [x] Workflow structure check passes.

## Accepted Tests

- [x] `pnpm run lint:web`
- [ ] `pnpm --dir src/web-ui run test:run`
- [x] `cargo check --workspace`
- [x] `pnpm run agent:check`

## Acceptance Coverage Result

- Automated: web lint and Rust workspace check passed; full web tests mostly passed but have one out-of-scope monaco resolution failure.
- Manual: inspected Vitest config, monaco package presence, and failing test import path.
- Coverage gaps: full web suite is not green; full Rust workspace tests were not run.

## Risks

- A PR should either fix or explicitly waive the `monaco-editor` Vitest resolution failure before treating full web tests as green.
- The broader verification result should not be represented as fully passing.

## Human Review Focus

- Decide whether to fix the existing Monaco/Vitest test environment issue before PR.
- Decide whether to run full `cargo test --workspace` after the web test gap is resolved or waived.

## Metrics

- intent_created: true
- questions_asked: 0
- tests_or_checks_created: 4 checks, 4 verification commands
- verification_passed: false
- rework_needed: false
