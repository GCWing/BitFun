# Evidence Package

## Metadata

- Task: Add MVP review escalation guidance
- Date: 2026-05-25
- Owner: Coding Agent
- Status: Complete

## Intent Record

`.agent/intents/intent-20260525-agent-review-escalation-mvp.md`

## Summary

Connected risk labels to review escalation guidance. L3/L4 Intent Coding tasks now need an explicit planned review path before coding, and Evidence Packages must state whether Deep Review or equivalent specialist review was completed, skipped by explicit user direction, or blocked by tooling.

## Files Changed

- `.agent/evidence/evidence-20260525-agent-review-escalation-mvp.md`
- `.agent/intents/intent-20260525-agent-review-escalation-mvp.md`
- `.agent/rules/risk-classification.md`
- `.agent/templates/evidence-template.md`
- `.agent/templates/intent-template.md`
- `src/crates/core/src/agentic/agents/prompts/intent_coding_mode.md`

## Verification

- `rg -n "Review Escalation|review escalation|Deep Review|L3 or L4|equivalent specialist review" .agent/templates .agent/rules/risk-classification.md src/crates/core/src/agentic/agents/prompts/intent_coding_mode.md`: passed.
- `cargo test -p bitfun-core intent_coding -- --nocapture`: passed.

## Risk Handling

- Final risk level: L1
- Risk factors: Prompt/template/rule guidance change.
- Verification matched expected level: yes.
- Skipped verification: full workspace tests were not run because no runtime gate, UI, or Deep Review behavior changed.
- Review escalation: Not required for this L1 change.

## Accepted Checks

- [x] Risk rule includes Deep Review or equivalent specialist review escalation.
- [x] Intent template includes `Review Escalation`.
- [x] Evidence template includes `Review Escalation`.
- [x] Intent Coding prompt mentions L3/L4 review escalation.
- [x] No automatic gate or UI behavior is added.

## Accepted Tests

- Text checks with `rg`.
- `intent_coding_mode_uses_dedicated_prompt_and_planning_tools`

## Risks

- Review escalation is still advisory and prompt-guided.
- No product enforcement exists yet for L3/L4 review completion.
- Deep Review is not auto-launched in this slice.

## Human Review Focus

- Whether Deep Review should be mandatory for all L3 code changes or only recommended when available.
- Whether L4 should require security-specific reviewer roles in the next slice.
- Whether skipped escalation should require explicit user confirmation.

## Metrics

- intent_created: true
- questions_asked: 0
- tests_or_checks_created: 5 checks, 2 verification commands
- verification_passed: true
- rework_needed: false

