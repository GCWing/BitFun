# Intent Record

## Metadata

- Task: Add MVP review escalation guidance
- Date: 2026-05-25
- Owner: Coding Agent
- Status: Accepted

## Original User Request

Continue implementing the intent-aligned Coding Agent workflow in BitFun.

## Agent Understanding

The next useful slice is to connect risk labels to human/specialist review expectations. This should remain prompt/template/rule guidance for now: L3/L4 tasks must explicitly recommend Deep Review or equivalent specialist review in the Intent Record and Evidence Package, without auto-triggering review sessions or modifying gate behavior.

## In Scope

- Update risk classification rules with review escalation expectations.
- Add review escalation fields to Intent and Evidence templates.
- Update Intent Coding prompt to require review escalation notes for L3/L4.
- Keep this slice documentation/prompt based.

## Out of Scope

- No automatic Deep Review launch.
- No UI workflow changes.
- No CI/gate enforcement.
- No policy engine.
- No new dependencies.

## Risk Level

- Level: L1
- Reason: Workflow prompt/template/rule change only.
- Risk factors: Changes Agent behavior expectations but does not modify execution or gate runtime.
- Verification expectation: Focused text checks and IntentCoding prompt embedding test.

## Acceptance Criteria

- Risk rule states L3/L4 require explicit review escalation handling.
- Intent template includes review escalation expectation.
- Evidence template includes review escalation result.
- Intent Coding prompt requires L3/L4 review escalation notes.
- Focused checks pass.

## Accepted Checks

- [x] Risk rule includes Deep Review or equivalent specialist review escalation.
- [x] Intent template includes `Review Escalation`.
- [x] Evidence template includes `Review Escalation`.
- [x] Intent Coding prompt mentions L3/L4 review escalation.
- [x] No automatic gate or UI behavior is added.

## Accepted Tests

- Text checks with `rg`.
- `cargo test -p bitfun-core intent_coding -- --nocapture`

## Clarification Questions

No blocking question. Assumption: review escalation should be explicit guidance first, not an automatic product action.

## User Confirmations

- User asked to continue after risk labels were added.

## Execution Contract

Agent must:

- Keep the change scoped to prompt/template/rule guidance.
- Avoid new dependencies.
- Avoid auto-triggering Deep Review.
- Run focused verification.

Agent must not:

- Modify CI gates.
- Add UI controls.
- Change Deep Review runtime behavior.

## Metrics

- intent_created: true
- questions_asked: 0
- tests_or_checks_created: 5 checks, 2 verification commands
- verification_passed: true
- rework_needed: false
