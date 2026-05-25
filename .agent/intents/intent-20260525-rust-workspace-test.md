# Intent Record

## Metadata

- Task: Run Rust workspace tests for Intent Coding MVP
- Date: 2026-05-25
- Owner: Coding Agent
- Status: Accepted

## Original User Request

Continue implementing the intent-aligned Coding Agent workflow in BitFun.

## Agent Understanding

The final MVP summary left one explicit verification gap: full `cargo test --workspace` had not been run. This slice should run it, record the result, and keep any repair scoped to failures caused by the Intent Coding MVP.

## In Scope

- Run `cargo test --workspace`.
- Classify failures if any appear.
- Run `pnpm run agent:check` after the Evidence Package is written.
- Update evidence with the final Rust workspace test result.

## Out of Scope

- No new feature work.
- No broad unrelated Rust fixes unless the failure is clearly caused by this MVP.
- No commit, push, or PR creation.

## Acceptance Criteria

- `cargo test --workspace` result is recorded.
- Any failure is classified and not hidden.
- `pnpm run agent:check` passes after Evidence Package creation.

## Risk Level

- Level: L2
- Reason: Workspace-wide Rust tests are broad verification across multiple crates.
- Risk factors: Existing unrelated tests may fail.
- Verification expectation: Full Rust workspace tests and workflow checker.
- Review escalation: Not required for verification-only slice.

## Accepted Checks

- [x] Rust workspace test result is recorded.
- [x] Failures, if any, are classified.
- [x] Workflow structure check passes.

## Accepted Tests

- `cargo test --workspace`
- `pnpm run agent:check`

## Acceptance Coverage Plan

- Automated: full Rust workspace tests and workflow checker.
- Manual: classify any Rust test failure against MVP scope.
- Coverage gaps: none expected for this verification slice.

## Clarification Questions

No blocking question. Assumption: running the full Rust workspace test suite is the right final verification step.

## User Confirmations

- User asked to continue after the MVP completion Evidence Package.

## Provenance Anchors

- Context inputs: final MVP completion evidence and current verification gaps.
- User decisions: Continue toward PR-ready validation.
- Related change notes: `.agent/changes/intent-coding-rollout.md`.

## Execution Contract

Agent must:

- Run `cargo test --workspace`.
- Record exact result.
- Avoid unrelated repairs.

Agent must not:

- Hide failures.
- Commit or push.
- Expand MVP scope.

## Metrics

- intent_created: true
- questions_asked: 0
- tests_or_checks_created: 3 checks, 2 verification commands
- verification_passed: true
- rework_needed: false
