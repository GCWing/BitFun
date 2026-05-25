# Intent Record

## Metadata

- Task: Add Intent Coding usage guide
- Date: 2026-05-25
- Owner: Coding Agent
- Status: Accepted

## Original User Request

Continue implementing the intent-aligned Coding Agent workflow in BitFun.

## Agent Understanding

The MVP now has the core workflow files, mode, prompt, context loading, and structure checker. The next useful slice is a concise human-facing guide that explains how to use and review the Intent Coding workflow in this repository.

## In Scope

- Add `.agent/README.md` as the workflow entry point.
- Explain when to use Intent Coding and the task lifecycle.
- Document required artifacts, verification commands, and review focus.
- Keep the guide separate from automatically injected `.agent` context.

## Out of Scope

- No product runtime changes.
- No CI integration.
- No screenshots or UI walkthroughs.
- No rewrite of existing rules/templates.

## Acceptance Criteria

- `.agent/README.md` explains the Intent Coding MVP workflow.
- The guide points to templates, rules, knowledge, changes, intents, and evidence.
- The guide documents `pnpm run agent:check`.
- The guide makes clear that workflow structure checks do not replace product verification.
- `pnpm run agent:check` passes.

## Risk Level

- Level: L1
- Reason: Documentation-only repository workflow guide.
- Risk factors: Documentation could imply stronger enforcement than currently exists.
- Verification expectation: `pnpm run agent:check`.
- Review escalation: Not required for L1.

## Accepted Checks

- [x] Guide documents task lifecycle from request to Evidence Package.
- [x] Guide documents `pnpm run agent:check`.
- [x] Guide distinguishes workflow structure validation from product verification.

## Accepted Tests

- `pnpm run agent:check`

## Acceptance Coverage Plan

- Automated: workflow structure check.
- Manual: review guide content for accuracy against current MVP.
- Coverage gaps: no rendered product walkthrough.

## Clarification Questions

No blocking question. Assumption: `.agent/README.md` is the best first entry point because `.agent` README files are intentionally skipped from automatic context injection.

## User Confirmations

- User asked to continue after estimating remaining MVP work.

## Provenance Anchors

- Context inputs: `.agent/knowledge/intent-coding-mvp.md`, `.agent/changes/intent-coding-rollout.md`, `.agent/templates/*`, `.agent/rules/*`.
- User decisions: Continue toward final MVP completion.
- Related change notes: `.agent/changes/intent-coding-rollout.md`.

## Execution Contract

Agent must:

- Keep the guide concise and operational.
- Avoid overstating runtime enforcement.
- Run `pnpm run agent:check`.

Agent must not:

- Add new tooling or dependencies.
- Change runtime behavior.
- Duplicate every rule file in the guide.

## Metrics

- intent_created: true
- questions_asked: 0
- tests_or_checks_created: 3 checks, 1 verification command
- verification_passed: true
- rework_needed: false
