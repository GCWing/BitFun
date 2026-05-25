# Evidence Package

## Metadata

- Task: Add MVP risk labels for Intent Coding
- Date: 2026-05-25
- Owner: Coding Agent
- Status: Complete

## Intent Record

`.agent/intents/intent-20260525-agent-risk-labels-mvp.md`

## Summary

Added lightweight risk labeling to the Intent Coding workflow. The repository now has a durable risk classification rule, templates require risk metadata, and the Intent Coding prompt asks the Agent to classify risk before coding and report risk handling in the Evidence Package.

## Files Changed

- `.agent/evidence/evidence-20260525-agent-risk-labels-mvp.md`
- `.agent/intents/intent-20260525-agent-risk-labels-mvp.md`
- `.agent/rules/risk-classification.md`
- `.agent/templates/evidence-template.md`
- `.agent/templates/intent-template.md`
- `src/crates/core/src/agentic/agents/prompts/intent_coding_mode.md`

## Verification

- `rg -n "Risk Level|Risk Handling|risk classification|L0 Exploration|L4 Safety-Critical" .agent/templates .agent/rules src/crates/core/src/agentic/agents/prompts/intent_coding_mode.md`: passed.
- `cargo test -p bitfun-core intent_coding -- --nocapture`: passed.

## Risk Handling

- Final risk level: L1
- Risk factors: Agent behavior prompt/template changes.
- Verification matched expected level: yes, focused text checks and prompt embedding test passed.
- Skipped verification: full workspace tests were not run because this slice did not change runtime gate behavior or frontend code.

## Accepted Checks

- [x] Risk classification rule exists.
- [x] Intent template includes `Risk Level`.
- [x] Evidence template includes `Risk Handling`.
- [x] Intent Coding prompt references risk classification.
- [x] No product UI or runtime gate behavior is added.

## Accepted Tests

- Text checks with `rg` for the new risk sections.
- `intent_coding_mode_uses_dedicated_prompt_and_planning_tools`

## Risks

- Risk labels are currently prompt-guided and manual, not automatically scored.
- No gate behavior changes were added, so this does not yet enforce Deep Review or CI escalation.
- Verification expectations depend on the Agent following the prompt until a runtime policy layer exists.

## Human Review Focus

- Whether the L0-L4 wording maps well to BitFun's actual release risk.
- Whether `.agent/rules/risk-classification.md` should become product default guidance for all coding modes or only Intent Coding.
- Whether L3/L4 should automatically recommend Deep Review in the next implementation slice.

## Metrics

- intent_created: true
- questions_asked: 0
- tests_or_checks_created: 5 checks, 2 verification commands
- verification_passed: true
- rework_needed: false

