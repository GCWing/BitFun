# Intent Record

## Metadata

- Task: Add Intent Coding MVP knowledge and change notes
- Date: 2026-05-25
- Owner: Coding Agent
- Status: Accepted

## Original User Request

Continue implementing the intent-aligned Coding Agent workflow in BitFun.

## Agent Understanding

Now that `.agent/knowledge/README.md` and `.agent/changes/README.md` are skipped during context injection, the simplified Context Compiler needs actual task-relevant Markdown notes. Add a durable knowledge note describing the Intent Coding MVP architecture and a change note describing the current rollout state.

## In Scope

- Add `.agent/knowledge/intent-coding-mvp.md`.
- Add `.agent/changes/intent-coding-rollout.md`.
- Verify these files are eligible for context injection while README files remain skipped.

## Out of Scope

- No runtime code changes.
- No prompt changes.
- No UI changes.
- No new dependencies.

## Risk Level

- Level: L0
- Reason: Documentation/context note addition only.
- Risk factors: Notes influence future Agent context but do not alter runtime behavior.
- Verification expectation: Text checks and existing context loader README skip test.
- Review escalation: Not required for L0.

## Acceptance Criteria

- Durable knowledge note summarizes Intent Coding MVP architecture.
- Change note summarizes current rollout status and remaining productization gaps.
- Notes are concrete enough to help future Agent work.
- Focused verification passes.

## Accepted Checks

- [x] Knowledge note exists and names core implementation files.
- [x] Change note exists and names current rollout state.
- [x] README skip test still passes.

## Accepted Tests

- Text checks with `rg`.
- `cargo test -p bitfun-core workspace_instruction_context_skips_agent_context_readmes -- --nocapture`

## Acceptance Coverage Plan

- Automated: Text checks and focused Rust test.
- Manual: Review note content for clarity.
- Coverage gaps: No full workspace tests for documentation-only change.

## Clarification Questions

No blocking question. Assumption: knowledge/change notes should be concise and eligible for automatic context injection.

## User Confirmations

- User asked to continue after README skip behavior was added.

## Provenance Anchors

- Context inputs: `.agent/templates/knowledge-template.md`, `.agent/templates/change-template.md`, current Intent Coding implementation and evidence trail.
- User decisions: Continue the MVP implementation path.
- Related change notes: None.

## Execution Contract

Agent must:

- Keep notes concise.
- Avoid duplicating every Evidence Package.
- Avoid secrets or private local data.
- Run focused verification.

Agent must not:

- Add runtime behavior.
- Add dependencies.
- Modify source code for this slice.

## Metrics

- intent_created: true
- questions_asked: 0
- tests_or_checks_created: 3 checks, 2 verification commands
- verification_passed: true
- rework_needed: false
