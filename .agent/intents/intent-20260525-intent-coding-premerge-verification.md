# Intent Record

## Metadata

- Task: Run broader pre-merge verification for Intent Coding MVP
- Date: 2026-05-25
- Owner: Coding Agent
- Status: Accepted

## Original User Request

Continue implementing the intent-aligned Coding Agent workflow in BitFun.

## Agent Understanding

Focused verification has passed. The next useful slice is broader pre-merge verification across web lint/tests and Rust workspace compilation, without adding new features.

## In Scope

- Run web lint.
- Run full web test suite.
- Run Rust workspace check.
- Run workflow structure check after Evidence Package creation.
- Record any failures and repair only if scoped to the Intent Coding MVP.

## Out of Scope

- No new feature work.
- No commit, push, or PR creation.
- No full `cargo test --workspace` unless the broader checks suggest it is necessary and feasible in this turn.

## Acceptance Criteria

- `pnpm run lint:web` passes.
- `pnpm --dir src/web-ui run test:run` passes.
- `cargo check --workspace` passes.
- `pnpm run agent:check` passes after Evidence Package creation.

## Risk Level

- Level: L2
- Reason: Verification spans frontend and Rust workspace compile surfaces.
- Risk factors: Existing repository tests may expose unrelated failures.
- Verification expectation: Broader pre-merge checks and workflow structure check.
- Review escalation: Not required; verification-only slice.

## Accepted Checks

- [x] Web lint passes.
- [ ] Full web tests pass.
- [x] Rust workspace check passes.
- [x] Workflow structure check passes.

## Accepted Tests

- `pnpm run lint:web`
- `pnpm --dir src/web-ui run test:run`
- `cargo check --workspace`
- `pnpm run agent:check`

## Acceptance Coverage Plan

- Automated: lint, full web tests, Rust workspace check, workflow checker.
- Manual: classify any failures as MVP-caused or unrelated.
- Coverage gaps: full `cargo test --workspace` remains optional for a final CI/PR pass.

## Clarification Questions

No blocking question. Assumption: broader but not maximal verification is the right next step after focused checks.

## User Confirmations

- User asked to continue after focused final verification completed.

## Provenance Anchors

- Context inputs: final focused verification evidence, package scripts, repository AGENTS verification table.
- User decisions: Continue toward final MVP completion.
- Related change notes: `.agent/changes/intent-coding-rollout.md`.

## Execution Contract

Agent must:

- Run the listed verification commands.
- Classify failures before attempting repairs.
- Keep repairs scoped to Intent Coding MVP if needed.

Agent must not:

- Hide unrelated failures.
- Start unrelated refactors.
- Commit or push.

## Metrics

- intent_created: true
- questions_asked: 0
- tests_or_checks_created: 4 checks, 4 verification commands
- verification_passed: false
- rework_needed: false
