# Evidence Package

## Metadata

- Task: Add Intent Coding usage guide
- Date: 2026-05-25
- Risk Level: L1
- Status: Complete

## Intent Record

`.agent/intents/intent-20260525-intent-coding-usage-guide.md`

## Summary

Added `.agent/README.md` as the human-facing entry point for BitFun's Intent Coding MVP. The guide explains when to use Intent Coding, the directory layout, the task lifecycle, required product verification, `pnpm run agent:check`, review focus, and current MVP limits.

## Provenance Chain

- Original request: continue implementing the intent-aligned Coding Agent workflow in BitFun.
- Context inputs: `.agent/knowledge/intent-coding-mvp.md`, `.agent/changes/intent-coding-rollout.md`, existing templates and rules.
- Intent Record: `.agent/intents/intent-20260525-intent-coding-usage-guide.md`.
- Acceptance: lifecycle documented, `agent:check` documented, product verification distinction documented.
- Execution: added `.agent/README.md`.
- Verification: workflow structure check passed.
- Repair loop: no failures so far.
- Review escalation: not required for L1.
- Evidence Package: `.agent/evidence/evidence-20260525-intent-coding-usage-guide.md`.

## Files Changed

- `.agent/README.md`
- `.agent/intents/intent-20260525-intent-coding-usage-guide.md`
- `.agent/evidence/evidence-20260525-intent-coding-usage-guide.md`

## Verification

- Workflow structure check: `pnpm run agent:check`: passed

## Repair Loop

- Failure classes: none so far
- Repair attempts: 0
- Final repair status: complete
- Remaining verification gaps: none

## Risk Handling

- Final risk level: L1
- Risk factors: Documentation could imply stronger enforcement than currently exists.
- Verification matched expected level: yes.
- Skipped verification: none so far.
- Review escalation: not required.

## Accepted Checks

- [x] Guide documents task lifecycle from request to Evidence Package.
- [x] Guide documents `pnpm run agent:check`.
- [x] Guide distinguishes workflow structure validation from product verification.

## Accepted Tests

- [x] `pnpm run agent:check`

## Acceptance Coverage Result

- Automated: workflow structure check passed.
- Manual: guide reviewed against current MVP facts and limits.
- Coverage gaps: no rendered product walkthrough.

## Risks

- The guide intentionally documents a manual MVP workflow, not runtime enforcement.
- The guide does not replace detailed rules under `.agent/rules/`.

## Human Review Focus

- Confirm the guide is concise enough to be used as the workflow entry point.
- Confirm the stated MVP limits match product expectations.

## Metrics

- intent_created: true
- questions_asked: 0
- tests_or_checks_created: 3 checks, 1 verification command
- verification_passed: true
- rework_needed: false
