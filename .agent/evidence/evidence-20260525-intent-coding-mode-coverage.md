# Evidence Package

## Metadata

- Task: Add Intent Coding mode registration and display coverage
- Date: 2026-05-25
- Owner: Coding Agent
- Status: Complete

## Intent Record

`.agent/intents/intent-20260525-intent-coding-mode-coverage.md`

## Summary

Added focused coverage so the new Intent Coding mode remains registered in core and resolves correctly in frontend agent utilities.

## Provenance Chain

- Original request: User asked to continue implementing the intent-aligned Coding Agent workflow.
- Context inputs: `src/crates/core/src/agentic/agents/registry/tests.rs`, `src/web-ui/src/app/scenes/agents/utils.ts`, `src/web-ui/src/app/scenes/agents/agentsStore.ts`.
- Intent Record: `.agent/intents/intent-20260525-intent-coding-mode-coverage.md`.
- Acceptance: Core mode registry coverage, frontend utility coverage, focused verification.
- Execution: Added Rust registry assertions and a new Vitest file for frontend mode utility behavior.
- Verification: Focused Rust tests, focused Vitest test, and web type-check.
- Repair loop: One command invocation error from passing two Cargo test names at once; repaired by running the tests as separate commands.
- Review escalation: Not required for L1.
- Evidence Package: `.agent/evidence/evidence-20260525-intent-coding-mode-coverage.md`.

## Files Changed

- `.agent/evidence/evidence-20260525-intent-coding-mode-coverage.md`
- `.agent/intents/intent-20260525-intent-coding-mode-coverage.md`
- `src/crates/core/src/agentic/agents/registry/tests.rs`
- `src/web-ui/src/app/scenes/agents/utils.test.ts`

## Verification

- `cargo test -p bitfun-core intent_coding_is_registered_as_top_level_mode -- --nocapture`: passed.
- `cargo test -p bitfun-core top_level_modes_default_to_auto -- --nocapture`: passed.
- `pnpm --dir src/web-ui run test:run src/app/scenes/agents/utils.test.ts`: passed.
- `pnpm run type-check:web`: passed.

## Repair Loop

- Failure classes: command_error.
- Repair attempts: 1.
- Final repair status: repaired.
- Remaining verification gaps: full workspace test suites were not run.

## Risk Handling

- Final risk level: L1
- Risk factors: Test coverage change only.
- Verification matched expected level: yes.
- Skipped verification: full workspace tests were not run because focused coverage and type-check covered the touched surfaces.
- Review escalation: Not required for L1.

## Accepted Checks

- [x] Core registry coverage includes `IntentCoding`.
- [x] Frontend utility coverage includes `IntentCoding`.
- [x] No product behavior changes beyond tests/exports needed for tests.

## Accepted Tests

- `intent_coding_is_registered_as_top_level_mode`
- `top_level_modes_default_to_auto`
- `src/app/scenes/agents/utils.test.ts`

## Risks

- Frontend coverage targets utility behavior, not a rendered mode dropdown.
- Core coverage confirms registration and tools, not prompt content.

## Human Review Focus

- Whether `IntentCoding` should be grouped near Agentic or Plan in future presentation ordering.
- Whether a rendered ChatInput mode-switch test should be added later.

## Metrics

- intent_created: true
- questions_asked: 0
- tests_or_checks_created: 3 checks, 4 verification commands
- verification_passed: true
- rework_needed: false

