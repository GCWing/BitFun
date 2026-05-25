# Intent Record

## Metadata

- Task: Add MVP context budget limits for `.agent` context loading
- Date: 2026-05-25
- Owner: Coding Agent
- Status: Accepted

## Original User Request

Continue implementing the intent-aligned Coding Agent workflow in BitFun.

## Agent Understanding

The next useful productization slice is to add deterministic context budget limits to the simplified Context Compiler. BitFun currently loads shallow Markdown files from `.agent/rules`, `.agent/knowledge`, and `.agent/changes`; this should be bounded by file count and per-file size so future knowledge growth does not inflate prompts unpredictably.

## In Scope

- Add a durable `.agent/rules/context-budget.md` rule.
- Enforce a shallow file count limit per `.agent` context directory.
- Enforce a per-file byte limit with UTF-8 safe truncation.
- Add focused tests for file count and truncation behavior.
- Update Intent Coding prompt to mention budgeted context loading.

## Out of Scope

- No token counting.
- No retrieval/reranking.
- No UI for context budget.
- No nested directory traversal.
- No new dependencies.

## Risk Level

- Level: L2
- Reason: Runtime prompt context behavior changes, but scoped to `.agent` context injection.
- Risk factors: Prompt context completeness can affect Agent behavior.
- Verification expectation: Focused Rust tests for context limits plus IntentCoding prompt embedding test.
- Review escalation: Not required for L2, but human review should check the chosen defaults.

## Acceptance Criteria

- `.agent/rules/context-budget.md` exists.
- `.agent` context loading limits files per context directory.
- Oversized `.agent` context files are truncated safely.
- Focused tests cover file-count limiting and truncation.
- Intent Coding prompt references budgeted Context Compiler input.

## Accepted Checks

- [x] Context budget rule exists.
- [x] Loader has a file count limit.
- [x] Loader has a UTF-8 safe file size limit.
- [x] Focused Rust tests pass.
- [x] Intent Coding prompt mentions budgeted context.

## Accepted Tests

- `workspace_instruction_context_limits_agent_context_file_count`
- `workspace_instruction_context_truncates_large_agent_context_files`
- `cargo test -p bitfun-core intent_coding -- --nocapture`

## Clarification Questions

No blocking question. Assumption: deterministic limits are acceptable before retrieval/reranking exists.

## User Confirmations

- User asked to continue after Provenance Chain MVP.

## Provenance Anchors

- Context inputs: `.agent/rules/provenance-chain.md`, existing context loader, Intent Coding prompt.
- User decisions: Continue the MVP implementation path.
- Related change notes: None.

## Execution Contract

Agent must:

- Keep limits deterministic and easy to review.
- Preserve existing local-only context loading behavior.
- Avoid new dependencies.
- Run focused verification.

Agent must not:

- Add vector retrieval or token counting.
- Change remote workspace prompt overlay behavior.
- Traverse nested `.agent` directories.

## Metrics

- intent_created: true
- questions_asked: 0
- tests_or_checks_created: 5 checks, 3 verification commands
- verification_passed: true
- rework_needed: false
