# Intent Record

## Metadata

- Task: Integrate agent workflow checker into Intent Coding prompt
- Date: 2026-05-25
- Owner: Coding Agent
- Status: Accepted

## Original User Request

Continue implementing the intent-aligned Coding Agent workflow in BitFun.

## Agent Understanding

The previous slice added `pnpm run agent:check`, but Intent Coding does not yet instruct Agents to run it as part of delivery. This slice should connect the checker to the workflow through durable rules, templates, and prompt coverage.

## In Scope

- Add a durable `.agent` rule for the workflow structure checker.
- Update the Evidence Package template to record the workflow structure check.
- Update the Intent Coding prompt to run `pnpm run agent:check` when the checker is available.
- Add prompt test coverage for the new instruction.

## Out of Scope

- No CI integration.
- No changes to the checker behavior.
- No runtime enforcement or automatic command execution.

## Acceptance Criteria

- Intent Coding prompt mentions `pnpm run agent:check`.
- Prompt test covers the checker instruction.
- Evidence template includes a workflow structure check slot.
- `pnpm run agent:check` still passes.
- Focused core prompt test passes.

## Risk Level

- Level: L1
- Reason: Prompt/template/rule guidance plus focused test assertion only.
- Risk factors: Overstating the checker as a substitute for product verification.
- Verification expectation: Focused Rust prompt test and `agent:check`.
- Review escalation: Not required for L1.

## Accepted Checks

- [x] Prompt requires the workflow structure check when available.
- [x] Evidence template records the workflow structure check.
- [x] Durable rule explains the checker scope and limits.

## Accepted Tests

- `cargo test -p bitfun-core intent_coding_prompt_embeds_acceptance_and_evidence_workflow -- --nocapture`
- `pnpm run agent:check`

## Acceptance Coverage Plan

- Automated: Focused Rust prompt test and local agent workflow checker.
- Manual: Review prompt wording to ensure product verification remains required.
- Coverage gaps: No runtime enforcement.

## Clarification Questions

No blocking question. Assumption: prompt-level enforcement is the right MVP step before CI or runtime enforcement.

## User Confirmations

- User asked to continue after the workflow checker slice.

## Provenance Anchors

- Context inputs: `src/crates/core/src/agentic/agents/prompts/intent_coding_mode.md`, `src/crates/core/src/agentic/agents/definitions/modes/intent_coding.rs`, `.agent/templates/evidence-template.md`, `scripts/check-agent-workflow.mjs`.
- User decisions: Continue the MVP implementation path.
- Related change notes: `.agent/changes/intent-coding-rollout.md`.

## Execution Contract

Agent must:

- Keep the checker as a structural add-on, not a replacement for product verification.
- Update prompt/test/template consistently.
- Run focused verification.

Agent must not:

- Add CI integration.
- Modify checker behavior.
- Remove existing verification requirements.

## Metrics

- intent_created: true
- questions_asked: 0
- tests_or_checks_created: 3 checks, 2 verification commands
- verification_passed: true
- rework_needed: false
