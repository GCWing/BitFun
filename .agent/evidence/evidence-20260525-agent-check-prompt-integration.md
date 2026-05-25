# Evidence Package

## Metadata

- Task: Integrate agent workflow checker into Intent Coding prompt
- Date: 2026-05-25
- Risk Level: L1
- Status: Complete

## Intent Record

`.agent/intents/intent-20260525-agent-check-prompt-integration.md`

## Summary

Connected the local workflow checker back into the Intent Coding workflow. The prompt now instructs Agents to run `pnpm run agent:check` after Intent/Evidence artifacts are written, while keeping product verification as a separate requirement. The Evidence template now has a workflow structure check slot, and a durable rule documents the checker's scope and limits.

## Provenance Chain

- Original request: continue implementing the intent-aligned Coding Agent workflow in BitFun.
- Context inputs: Intent Coding prompt, prompt unit test, Evidence template, existing workflow checker.
- Intent Record: `.agent/intents/intent-20260525-agent-check-prompt-integration.md`.
- Acceptance: prompt instruction, Evidence template slot, durable rule, focused tests.
- Execution: updated prompt/template/rule/test.
- Verification: focused Rust prompt test and workflow structure check passed.
- Repair loop: no failures so far.
- Review escalation: not required for L1.
- Evidence Package: `.agent/evidence/evidence-20260525-agent-check-prompt-integration.md`.

## Files Changed

- `.agent/intents/intent-20260525-agent-check-prompt-integration.md`
- `.agent/evidence/evidence-20260525-agent-check-prompt-integration.md`
- `.agent/rules/workflow-check.md`
- `.agent/templates/evidence-template.md`
- `src/crates/core/src/agentic/agents/prompts/intent_coding_mode.md`
- `src/crates/core/src/agentic/agents/definitions/modes/intent_coding.rs`

## Verification

- `cargo test -p bitfun-core intent_coding_prompt_embeds_acceptance_and_evidence_workflow -- --nocapture`: passed
- Workflow structure check: `pnpm run agent:check`: passed

## Repair Loop

- Failure classes: none so far
- Repair attempts: 0
- Final repair status: complete
- Remaining verification gaps: none

## Risk Handling

- Final risk level: L1
- Risk factors: Prompt wording could imply workflow check replaces product verification.
- Verification matched expected level: yes.
- Skipped verification: none so far.
- Review escalation: not required.

## Accepted Checks

- [x] Prompt requires the workflow structure check when available.
- [x] Evidence template records the workflow structure check.
- [x] Durable rule explains the checker scope and limits.

## Accepted Tests

- [x] `cargo test -p bitfun-core intent_coding_prompt_embeds_acceptance_and_evidence_workflow -- --nocapture`
- [x] `pnpm run agent:check`

## Acceptance Coverage Result

- Automated: focused Rust prompt test and workflow structure check passed.
- Manual: reviewed wording so the checker is explicitly not a substitute for product verification.
- Coverage gaps: no runtime enforcement or CI integration.

## Risks

- Prompt-level guidance depends on Agent compliance until a future runtime or CI gate exists.
- The workflow checker remains structural and does not validate product behavior.

## Human Review Focus

- Confirm `agent:check` should be a delivery step for Intent Coding tasks that write `.agent` artifacts.
- Confirm the wording keeps product verification mandatory.

## Metrics

- intent_created: true
- questions_asked: 0
- tests_or_checks_created: 3 checks, 2 verification commands
- verification_passed: true
- rework_needed: false
