# Intent Record

## Metadata

- Task: Add context budget omission marker
- Date: 2026-05-25
- Owner: Coding Agent
- Status: Accepted

## Original User Request

Continue implementing the intent-aligned Coding Agent workflow in BitFun.

## Agent Understanding

The simplified Context Compiler now enforces file count and file size budgets. The next useful refinement is to avoid silent omission: when a `.agent` context directory exceeds the file count budget, inject a compact marker into the prompt context so the Agent knows additional files exist and can explicitly read them if needed.

## In Scope

- Add an omission marker when a `.agent` context directory has more files than the load limit.
- Update context budget rule and Intent Coding prompt wording.
- Add focused test coverage.

## Out of Scope

- No token counting.
- No retrieval/reranking.
- No nested traversal.
- No UI changes.
- No new dependencies.

## Risk Level

- Level: L2
- Reason: Runtime prompt context behavior changes.
- Risk factors: Agent awareness of omitted context changes, but actual loaded files remain bounded.
- Verification expectation: Focused Rust test for omission marker plus existing context budget tests.
- Review escalation: Not required for L2.

## Acceptance Criteria

- Loader emits a marker document when a context directory exceeds the file count limit.
- Marker states the directory, loaded file count, omitted file count, and omitted file names.
- Focused test verifies omitted files are not loaded as documents but are disclosed by marker.
- Context budget rule and Intent Coding prompt mention omission markers.

## Accepted Checks

- [x] Omitted context marker is emitted.
- [x] Omitted files are not loaded as full documents.
- [x] Rule documents marker behavior.
- [x] Prompt mentions omitted/truncated context markers.

## Accepted Tests

- `workspace_instruction_context_marks_omitted_agent_context_files`
- Existing context budget tests as needed.

## Clarification Questions

No blocking question. Assumption: exposing omitted Markdown file names is acceptable because these are workspace-local context filenames, not file contents.

## User Confirmations

- User asked to continue after Accepted Checks/Tests rule.

## Provenance Anchors

- Context inputs: context loader, `.agent/rules/context-budget.md`, Intent Coding prompt.
- User decisions: Continue the MVP implementation path.
- Related change notes: None.

## Execution Contract

Agent must:

- Keep marker compact.
- Avoid loading omitted file contents.
- Preserve deterministic ordering.
- Run focused verification.

Agent must not:

- Add retrieval/reranking.
- Add UI.
- Change context directory limits.

## Metrics

- intent_created: true
- questions_asked: 0
- tests_or_checks_created: 4 checks, focused tests
- verification_passed: true
- rework_needed: false
