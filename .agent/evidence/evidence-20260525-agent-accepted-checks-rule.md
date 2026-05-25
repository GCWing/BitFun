# Evidence Package

## Metadata

- Task: Add Accepted Checks/Tests rule for Intent Coding
- Date: 2026-05-25
- Owner: Coding Agent
- Status: Complete

## Intent Record

`.agent/intents/intent-20260525-agent-accepted-checks-rule.md`

## Summary

Added durable guidance for Accepted Checks and Accepted Tests. Intent and Evidence templates now record acceptance coverage plans/results, and the Intent Coding prompt distinguishes automated tests from manual checks.

## Provenance Chain

- Original request: User asked to continue implementing the intent-aligned Coding Agent workflow.
- Context inputs: `.agent/templates/intent-template.md`, `.agent/templates/evidence-template.md`, `src/crates/core/src/agentic/agents/prompts/intent_coding_mode.md`, `src/crates/core/src/agentic/agents/definitions/modes/intent_coding.rs`.
- Intent Record: `.agent/intents/intent-20260525-agent-accepted-checks-rule.md`.
- Acceptance: Add acceptance rule, update templates, update prompt, add prompt embedding coverage.
- Execution: Added `.agent/rules/accepted-checks.md`, template fields, prompt guidance, and prompt-content test assertions.
- Verification: Text check with `rg`; `cargo test -p bitfun-core intent_coding -- --nocapture`.
- Repair loop: No failures; repair status `not_needed`.
- Review escalation: Not required for L1.
- Evidence Package: `.agent/evidence/evidence-20260525-agent-accepted-checks-rule.md`.

## Files Changed

- `.agent/evidence/evidence-20260525-agent-accepted-checks-rule.md`
- `.agent/intents/intent-20260525-agent-accepted-checks-rule.md`
- `.agent/rules/accepted-checks.md`
- `.agent/templates/evidence-template.md`
- `.agent/templates/intent-template.md`
- `src/crates/core/src/agentic/agents/definitions/modes/intent_coding.rs`
- `src/crates/core/src/agentic/agents/prompts/intent_coding_mode.md`

## Verification

- `rg -n "Accepted Checks and Tests|Acceptance Coverage Plan|Acceptance Coverage Result|accepted-checks|acceptance coverage result" .agent/rules/accepted-checks.md .agent/templates src/crates/core/src/agentic/agents/prompts/intent_coding_mode.md src/crates/core/src/agentic/agents/definitions/modes/intent_coding.rs`: passed.
- `cargo test -p bitfun-core intent_coding -- --nocapture`: passed.

## Repair Loop

- Failure classes: none observed.
- Repair attempts: 0.
- Final repair status: not_needed.
- Remaining verification gaps: full workspace tests were not run.

## Risk Handling

- Final risk level: L1
- Risk factors: Prompt/template/rule guidance and test coverage change.
- Verification matched expected level: yes.
- Skipped verification: full workspace tests were not run because focused tests cover the changed prompt/mode surface.
- Review escalation: Not required for L1.

## Accepted Checks

- [x] Accepted Checks/Tests rule exists.
- [x] Intent template includes acceptance coverage plan.
- [x] Evidence template includes acceptance coverage result.
- [x] Intent Coding prompt references accepted checks/tests coverage.
- [x] Prompt embedding test covers Intent Coding prompt content.

## Accepted Tests

- Text checks with `rg`.
- `intent_coding_prompt_embeds_acceptance_and_evidence_workflow`
- `intent_coding_mode_uses_dedicated_prompt_and_planning_tools`

## Acceptance Coverage Result

- Automated: Focused Rust prompt/mode tests and text checks.
- Manual: Reviewed template/prompt wording while editing.
- Coverage gaps: No runtime enforcement for acceptance coverage yet.

## Risks

- Acceptance coverage is still prompt-guided.
- No automatic test generation or policy gate exists.
- Agents can still under-report coverage until runtime enforcement exists.

## Human Review Focus

- Whether the rule is strict enough for L2+ work.
- Whether manual checks should require user confirmation for higher-risk tasks.
- Whether Evidence Package generation should eventually validate that all Accepted Checks have statuses.

## Metrics

- intent_created: true
- questions_asked: 0
- tests_or_checks_created: 5 checks, 2 verification commands
- verification_passed: true
- rework_needed: false

