# Intent Record

## Metadata

- Task: Skip `.agent` bucket README files during context injection
- Date: 2026-05-25
- Owner: Coding Agent
- Status: Accepted

## Original User Request

Continue implementing the intent-aligned Coding Agent workflow in BitFun.

## Agent Understanding

The simplified Context Compiler now loads bounded `.agent` context files and marks omitted files. The next useful refinement is to avoid injecting bucket README files (`.agent/knowledge/README.md`, `.agent/changes/README.md`, etc.) because they explain directory usage rather than task-relevant knowledge. Skipping README files keeps context focused while leaving README files available for humans.

## In Scope

- Skip `README.md` files in `.agent/rules`, `.agent/knowledge`, and `.agent/changes` context loading.
- Add focused test coverage.
- Update context budget rule to document the behavior.

## Out of Scope

- No nested traversal.
- No retrieval/reranking.
- No UI changes.
- No new dependencies.

## Risk Level

- Level: L2
- Reason: Runtime prompt-context behavior changes.
- Risk factors: Context completeness changes for README files.
- Verification expectation: Focused Rust tests for skip behavior and existing context loading tests.
- Review escalation: Not required for L2.

## Acceptance Criteria

- Loader skips shallow `README.md` files in `.agent` context buckets.
- Skipped README files do not count toward the 20-file budget.
- Focused test verifies README skip behavior.
- Context budget rule documents README skip behavior.

## Accepted Checks

- [x] README files are skipped.
- [x] README files do not consume context file budget.
- [x] Context budget rule documents README skip behavior.
- [x] Focused Rust tests pass.

## Accepted Tests

- `workspace_instruction_context_skips_agent_context_readmes`
- Existing focused context tests as needed.

## Clarification Questions

No blocking question. Assumption: bucket README files are human guidance and should not be injected by default.

## User Confirmations

- User asked to continue after context budget omission markers were added.

## Provenance Anchors

- Context inputs: context loader, `.agent/rules/context-budget.md`, `.agent/knowledge/README.md`, `.agent/changes/README.md`.
- User decisions: Continue the MVP implementation path.
- Related change notes: None.

## Execution Contract

Agent must:

- Keep skip behavior limited to `.agent` context buckets.
- Preserve loading of root `AGENTS.md` and `CLAUDE.md`.
- Run focused verification.

Agent must not:

- Skip arbitrary files outside `.agent` context buckets.
- Remove README files from the repo.
- Change context limits.

## Metrics

- intent_created: true
- questions_asked: 0
- tests_or_checks_created: 4 checks, focused tests
- verification_passed: true
- rework_needed: false
