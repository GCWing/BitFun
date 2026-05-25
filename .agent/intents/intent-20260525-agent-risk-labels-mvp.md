# Intent Record

## Metadata

- Task: Add MVP risk labels for Intent Coding
- Date: 2026-05-25
- Owner: Coding Agent
- Status: Accepted

## Original User Request

Continue implementing the intent-aligned Coding Agent workflow in BitFun.

## Agent Understanding

The next useful slice is the P1 risk labeling layer. Before building a full Gate Pipeline, Intent Coding tasks should explicitly classify task risk and map that risk to verification expectations. This creates a lightweight bridge from Intent Record to Evidence Package and later Deep Review/Gate integration.

## In Scope

- Add a durable `.agent/rules/risk-classification.md` rule.
- Add risk level fields to Intent Record and Evidence Package templates.
- Update the Intent Coding prompt to require risk classification before coding.
- Keep the implementation prompt/documentation-based for this slice.

## Out of Scope

- No automatic static analysis risk scorer.
- No Deep Review auto-trigger.
- No CI gate pipeline.
- No OPA/Rego policy engine.
- No UI changes.
- No new dependencies.

## Risk Level

- Level: L1
- Reason: Prompt/template/rule change that affects Agent behavior but does not modify product runtime beyond prompt content.

## Acceptance Criteria

- `.agent/rules/risk-classification.md` defines L0-L4 levels and verification expectations.
- Intent template includes risk level, risk factors, and verification expectation.
- Evidence template includes final risk level and risk handling result.
- Intent Coding prompt requires risk classification before code edits.
- Focused verification confirms prompt/rule/template files contain the new risk fields.

## Accepted Checks

- [x] Risk classification rule exists.
- [x] Intent template includes `Risk Level`.
- [x] Evidence template includes `Risk Handling`.
- [x] Intent Coding prompt references risk classification.
- [x] No product UI or runtime gate behavior is added.

## Accepted Tests

- Text checks with `rg` for the new risk sections.
- `cargo test -p bitfun-core intent_coding -- --nocapture`

## Clarification Questions

No blocking question. Assumption: risk labels should be explicit and manual/prompt-guided before automatic scoring exists.

## User Confirmations

- User asked to continue after the simplified Context Compiler slice.

## Execution Contract

Agent must:

- Keep this slice focused on risk labels and verification expectations.
- Avoid adding dependencies.
- Avoid changing runtime gate behavior.
- Run focused checks.

Agent must not:

- Implement a full policy engine.
- Auto-trigger Deep Review.
- Block merges or modify CI.

## Metrics

- intent_created: true
- questions_asked: 0
- tests_or_checks_created: 5 checks, 2 verification commands
- verification_passed: true
- rework_needed: false
