# Intent Record

## Metadata

- Task: Add MVP provenance chain fields
- Date: 2026-05-25
- Owner: Coding Agent
- Status: Accepted

## Original User Request

Continue implementing the intent-aligned Coding Agent workflow in BitFun.

## Agent Understanding

The next useful slice is a lightweight Provenance Chain. Intent Coding should preserve a compact audit trail from original request to Intent Record, context inputs, verification, repair attempts, review escalation, and Evidence Package. This prepares for future session-level provenance without adding event storage now.

## In Scope

- Add `.agent/rules/provenance-chain.md`.
- Add provenance fields to Intent and Evidence templates.
- Update Intent Coding prompt to require provenance links in evidence.
- Keep this file/template/prompt based.

## Out of Scope

- No runtime event store.
- No database or session schema changes.
- No UI visualization.
- No automatic tool-call provenance export.
- No new dependencies.

## Risk Level

- Level: L1
- Reason: Workflow prompt/template/rule change only.
- Risk factors: Changes Agent reporting expectations but not runtime behavior.
- Verification expectation: Focused text checks and IntentCoding prompt embedding test.
- Review escalation: Not required for L1.

## Acceptance Criteria

- Provenance rule defines minimum chain entries.
- Intent template records provenance anchors.
- Evidence template records provenance chain.
- Intent Coding prompt requires provenance in Evidence Package.
- Focused checks pass.

## Accepted Checks

- [x] Provenance rule exists.
- [x] Intent template includes `Provenance Anchors`.
- [x] Evidence template includes `Provenance Chain`.
- [x] Intent Coding prompt references provenance.
- [x] No runtime event store is added.

## Accepted Tests

- Text checks with `rg`.
- `cargo test -p bitfun-core intent_coding -- --nocapture`

## Clarification Questions

No blocking question. Assumption: provenance starts as compact markdown anchors before product event storage exists.

## User Confirmations

- User asked to continue after repair-loop evidence was added.

## Execution Contract

Agent must:

- Keep this slice scoped to prompt/template/rule guidance.
- Avoid runtime schema changes.
- Avoid dependencies.
- Run focused verification.

Agent must not:

- Add an event store.
- Modify session persistence.
- Add UI visualization.

## Metrics

- intent_created: true
- questions_asked: 0
- tests_or_checks_created: 5 checks, 2 verification commands
- verification_passed: true
- rework_needed: false
