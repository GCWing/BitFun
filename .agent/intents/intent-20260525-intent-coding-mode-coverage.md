# Intent Record

## Metadata

- Task: Add Intent Coding mode registration and display coverage
- Date: 2026-05-25
- Owner: Coding Agent
- Status: Accepted

## Original User Request

Continue implementing the intent-aligned Coding Agent workflow in BitFun.

## Agent Understanding

The next useful productization slice is test coverage for the newly added Intent Coding mode. The mode is already registered in core and exposed in frontend labels/metadata; focused tests should ensure it remains discoverable and displays the expected translated description/capabilities.

## In Scope

- Add or update core tests so built-in registry coverage includes `IntentCoding`.
- Add or update frontend tests for mode description/capability utilities.
- Keep changes limited to coverage for existing Intent Coding behavior.

## Out of Scope

- No new mode behavior.
- No UI redesign.
- No runtime policy enforcement.
- No new dependencies.

## Risk Level

- Level: L1
- Reason: Test coverage and utility assertions for existing behavior.
- Risk factors: Frontend test utilities may require small export adjustments.
- Verification expectation: Focused Rust and web tests.
- Review escalation: Not required for L1.

## Acceptance Criteria

- Core test confirms `IntentCoding` is a built-in mode.
- Frontend test confirms Intent Coding description/capabilities resolve correctly.
- Focused verification passes.

## Accepted Checks

- [x] Core registry coverage includes `IntentCoding`.
- [x] Frontend utility coverage includes `IntentCoding`.
- [x] No product behavior changes beyond tests/exports needed for tests.

## Accepted Tests

- Focused Rust test for built-in agent specs or registry.
- Focused web test for agents utilities.

## Clarification Questions

No blocking question. Assumption: adding focused tests is the right next productization step before adding more runtime behavior.

## User Confirmations

- User asked to continue after Context Budget MVP.

## Provenance Anchors

- Context inputs: core registry files, `src/web-ui/src/app/scenes/agents/utils.ts`, nearby tests.
- User decisions: Continue the MVP implementation path.
- Related change notes: None.

## Execution Contract

Agent must:

- Read nearby tests before editing.
- Keep changes focused on coverage.
- Run focused verification.

Agent must not:

- Change Intent Coding behavior as part of test work.
- Add dependencies.
- Run broad refactors.

## Metrics

- intent_created: true
- questions_asked: 0
- tests_or_checks_created: 3 checks, focused tests
- verification_passed: true
- rework_needed: false
