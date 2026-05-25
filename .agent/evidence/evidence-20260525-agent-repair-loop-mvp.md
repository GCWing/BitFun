# Evidence Package

## Metadata

- Task: Add MVP repair loop evidence fields
- Date: 2026-05-25
- Owner: Coding Agent
- Status: Complete

## Intent Record

`.agent/intents/intent-20260525-agent-repair-loop-mvp.md`

## Summary

Added lightweight failure classification and repair-loop evidence guidance. Verification failures in Intent Coding should now be classified before repair, repair attempts should be recorded, and Evidence Packages include repair-loop status.

## Files Changed

- `.agent/evidence/evidence-20260525-agent-repair-loop-mvp.md`
- `.agent/intents/intent-20260525-agent-repair-loop-mvp.md`
- `.agent/rules/error-classification.md`
- `.agent/templates/evidence-template.md`
- `src/crates/core/src/agentic/agents/prompts/intent_coding_mode.md`

## Verification

- `rg -n "Error Classification|Failure Classes|Repair Loop|failure class|repair-loop|repair attempts|Final repair status" .agent/rules .agent/templates src/crates/core/src/agentic/agents/prompts/intent_coding_mode.md`: passed.
- `cargo test -p bitfun-core intent_coding -- --nocapture`: passed.

## Repair Loop

- Failure classes: none observed.
- Repair attempts: 0.
- Final repair status: not_needed.
- Remaining verification gaps: full workspace tests were not run for this prompt/template/rule-only slice.

## Risk Handling

- Final risk level: L1
- Risk factors: Prompt/template/rule guidance change.
- Verification matched expected level: yes.
- Skipped verification: full workspace tests were not run because no execution loop or tool runtime behavior changed.
- Review escalation: Not required for L1.

## Accepted Checks

- [x] Error classification rule exists.
- [x] Evidence template includes `Repair Loop`.
- [x] Intent Coding prompt references failure classification.
- [x] Intent Coding prompt references repair attempts.
- [x] No automatic Repair Router runtime is added.

## Accepted Tests

- Text checks with `rg`.
- `intent_coding_mode_uses_dedicated_prompt_and_planning_tools`

## Risks

- Failure classification is still prompt-guided and manual.
- No runtime retry cap or Repair Router exists yet.
- Evidence quality depends on the Agent following the prompt until runtime enforcement exists.

## Human Review Focus

- Whether the failure classes match BitFun's most common verification failures.
- Whether repeated-failure escalation should later become runtime-enforced.
- Whether repair-loop counters should be stored in session provenance instead of only Evidence Package markdown.

## Metrics

- intent_created: true
- questions_asked: 0
- tests_or_checks_created: 5 checks, 2 verification commands
- verification_passed: true
- rework_needed: false

