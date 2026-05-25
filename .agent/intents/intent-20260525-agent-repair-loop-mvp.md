# Intent Record

## Metadata

- Task: Add MVP repair loop evidence fields
- Date: 2026-05-25
- Owner: Coding Agent
- Status: Accepted

## Original User Request

Continue implementing the intent-aligned Coding Agent workflow in BitFun.

## Agent Understanding

The next useful slice is lightweight failure classification and repair-loop evidence. When verification fails, Intent Coding should classify the failure, record repair attempts, and include final repair status in the Evidence Package. This prepares for a future Error Classifier and Repair Router without implementing automatic routing now.

## In Scope

- Add `.agent/rules/error-classification.md`.
- Add repair-loop fields to the Evidence Package template.
- Update Intent Coding prompt to require failure classification and repair attempt tracking.
- Keep this prompt/template/rule based.

## Out of Scope

- No automatic Error Classifier implementation.
- No Repair Router runtime.
- No retry limits enforced by code.
- No UI changes.
- No new dependencies.

## Risk Level

- Level: L1
- Reason: Workflow prompt/template/rule change only.
- Risk factors: Changes Agent behavior expectations but not tool execution runtime.
- Verification expectation: Focused text checks and IntentCoding prompt embedding test.
- Review escalation: Not required for L1.

## Acceptance Criteria

- Error classification rule defines common failure classes.
- Evidence template records verification failures, repair attempts, and final repair status.
- Intent Coding prompt asks the Agent to classify failed verification before repair.
- Focused checks pass.

## Accepted Checks

- [x] Error classification rule exists.
- [x] Evidence template includes `Repair Loop`.
- [x] Intent Coding prompt references failure classification.
- [x] Intent Coding prompt references repair attempts.
- [x] No automatic Repair Router runtime is added.

## Accepted Tests

- Text checks with `rg`.
- `cargo test -p bitfun-core intent_coding -- --nocapture`

## Clarification Questions

No blocking question. Assumption: repair-loop tracking should start as explicit evidence, not automatic runtime routing.

## User Confirmations

- User asked to continue after review escalation guidance was added.

## Execution Contract

Agent must:

- Keep changes limited to prompt/template/rule guidance.
- Avoid dependencies.
- Avoid runtime retry/router behavior.
- Run focused verification.

Agent must not:

- Add automatic retry limits.
- Modify agent execution loops.
- Change tool runtime behavior.

## Metrics

- intent_created: true
- questions_asked: 0
- tests_or_checks_created: 5 checks, 2 verification commands
- verification_passed: true
- rework_needed: false
