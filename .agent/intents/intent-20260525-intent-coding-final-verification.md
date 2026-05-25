# Intent Record

## Metadata

- Task: Run Intent Coding MVP final verification
- Date: 2026-05-25
- Owner: Coding Agent
- Status: Accepted

## Original User Request

Continue implementing the intent-aligned Coding Agent workflow in BitFun.

## Agent Understanding

The MVP now has the mode, workflow files, context loading, tests, checker, and usage guide. The next useful slice is a final verification and change-scope audit before declaring the MVP functionally complete.

## In Scope

- Run the workflow structure check.
- Run focused core tests for Intent Coding mode, prompt, registry, and context loading behavior.
- Run focused web tests for frontend Intent Coding mapping/display behavior.
- Run web type-check.
- Inspect git diff/stat for scope sanity.
- Produce an Evidence Package for final verification.

## Out of Scope

- No new feature work unless verification exposes a defect.
- No full workspace test suite unless focused verification indicates a broader issue.
- No commit, branch, or PR creation.

## Acceptance Criteria

- `pnpm run agent:check` passes.
- Focused core Intent Coding tests pass.
- Focused web Intent Coding tests pass.
- `pnpm run type-check:web` passes.
- Diff audit finds no unrelated/generated churn requiring cleanup.

## Risk Level

- Level: L2
- Reason: Final verification spans Rust core, frontend, and workflow artifacts.
- Risk factors: Multiple touched areas and many new workflow files.
- Verification expectation: Focused Rust/web checks plus web type-check and agent workflow check.
- Review escalation: Not required; no L3/L4 product risk.

## Accepted Checks

- [x] Workflow structure check passes.
- [x] Focused Rust tests pass.
- [x] Focused web tests and type-check pass.
- [x] Diff scope remains aligned with Intent Coding MVP.

## Accepted Tests

- `pnpm run agent:check`
- `cargo test -p bitfun-core intent_coding -- --nocapture`
- `cargo test -p bitfun-core workspace_instruction_context -- --nocapture`
- `pnpm --dir src/web-ui run test:run src/app/scenes/agents/utils.test.ts src/flow_chat/components/modeDisplay.test.ts`
- `pnpm run type-check:web`

## Acceptance Coverage Plan

- Automated: workflow checker, focused Rust tests, focused frontend tests, web type-check.
- Manual: inspect `git diff --stat` and relevant diff slices for scope.
- Coverage gaps: not running full `cargo test --workspace` or full web test suite in this slice.

## Clarification Questions

No blocking question. Assumption: focused verification is appropriate for final MVP confidence before any full CI pass or PR.

## User Confirmations

- User asked to continue after the usage guide slice.

## Provenance Anchors

- Context inputs: current git diff, `.agent/README.md`, `scripts/check-agent-workflow.mjs`, Intent Coding Rust and web tests.
- User decisions: Continue toward final MVP completion.
- Related change notes: `.agent/changes/intent-coding-rollout.md`.

## Execution Contract

Agent must:

- Run verification before claiming final readiness.
- Record skipped broader verification explicitly.
- Avoid unrelated cleanup.

Agent must not:

- Commit or push.
- Broaden scope into new runtime enforcement.
- Hide failed verification.

## Metrics

- intent_created: true
- questions_asked: 0
- tests_or_checks_created: 4 checks, 5 verification commands
- verification_passed: true
- rework_needed: false
