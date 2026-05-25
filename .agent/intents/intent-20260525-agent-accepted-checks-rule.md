# Intent Record

## Metadata

- Task: Add Accepted Checks/Tests rule for Intent Coding
- Date: 2026-05-25
- Owner: Coding Agent
- Status: Accepted

## Original User Request

Continue implementing the intent-aligned Coding Agent workflow in BitFun.

## Agent Understanding

The next useful slice is to formalize Accepted Checks/Tests as a durable workflow rule. Intent Coding already asks for acceptance criteria, but the repository should define when a manual check is acceptable, when automated tests are expected, and how coverage gaps should be recorded in Evidence Packages.

## In Scope

- Add `.agent/rules/accepted-checks.md`.
- Add acceptance coverage fields to Intent and Evidence templates.
- Update Intent Coding prompt with clearer accepted checks/tests guidance.
- Add focused core prompt embedding coverage for the Intent Coding prompt.

## Out of Scope

- No automatic test generation.
- No runtime enforcement.
- No UI changes.
- No CI gate changes.
- No new dependencies.

## Risk Level

- Level: L1
- Reason: Workflow prompt/template/rule change plus focused prompt test coverage.
- Risk factors: Changes Agent behavior expectations but not runtime execution.
- Verification expectation: Text checks, IntentCoding prompt embedding test, existing mode registration test.
- Review escalation: Not required for L1.

## Acceptance Criteria

- Accepted Checks/Tests rule exists.
- Intent template records acceptance coverage plan.
- Evidence template records acceptance coverage result.
- Intent Coding prompt distinguishes automated tests from manual checks.
- Focused prompt embedding test passes.

## Accepted Checks

- [x] Accepted Checks/Tests rule exists.
- [x] Intent template includes acceptance coverage plan.
- [x] Evidence template includes acceptance coverage result.
- [x] Intent Coding prompt references accepted checks/tests coverage.
- [x] Prompt embedding test covers Intent Coding prompt content.

## Accepted Tests

- Text checks with `rg`.
- `cargo test -p bitfun-core intent_coding -- --nocapture`

## Clarification Questions

No blocking question. Assumption: acceptance coverage starts as guidance and evidence, not enforcement.

## User Confirmations

- User asked to continue after Intent Coding mode coverage was added.

## Provenance Anchors

- Context inputs: `.agent/templates/intent-template.md`, `.agent/templates/evidence-template.md`, Intent Coding prompt and mode tests.
- User decisions: Continue the MVP implementation path.
- Related change notes: None.

## Execution Contract

Agent must:

- Keep this slice scoped to acceptance guidance and focused prompt coverage.
- Avoid runtime test generation or enforcement.
- Run focused verification.

Agent must not:

- Add dependencies.
- Modify CI.
- Change UI behavior.

## Metrics

- intent_created: true
- questions_asked: 0
- tests_or_checks_created: 5 checks, 2 verification commands
- verification_passed: true
- rework_needed: false
