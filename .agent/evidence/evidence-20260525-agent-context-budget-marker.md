# Evidence Package

## Metadata

- Task: Add context budget omission marker
- Date: 2026-05-25
- Owner: Coding Agent
- Status: Complete

## Intent Record

`.agent/intents/intent-20260525-agent-context-budget-marker.md`

## Summary

Added an omission marker for `.agent` context directories that exceed the file count budget. BitFun still loads only the first 20 shallow Markdown files per context directory, but now injects a `__context_budget__.md` marker listing omitted files so the Agent can explicitly inspect them when relevant.

## Provenance Chain

- Original request: User asked to continue implementing the intent-aligned Coding Agent workflow.
- Context inputs: `src/crates/core/src/service/agent_memory/instruction_context.rs`, `.agent/rules/context-budget.md`, `src/crates/core/src/agentic/agents/prompts/intent_coding_mode.md`.
- Intent Record: `.agent/intents/intent-20260525-agent-context-budget-marker.md`.
- Acceptance: Emit omission marker, avoid loading omitted contents, update rule/prompt, focused tests pass.
- Execution: Added omitted-path tracking, marker rendering, and tests for marker behavior.
- Verification: Text check with `rg`; focused Rust tests for marker, count limit, and IntentCoding prompt.
- Repair loop: Initial Rust compile failed because `files` vector was missing after refactor; added `let mut files = Vec::new()` and reran tests successfully.
- Review escalation: Not required for L2.
- Evidence Package: `.agent/evidence/evidence-20260525-agent-context-budget-marker.md`.

## Files Changed

- `.agent/evidence/evidence-20260525-agent-context-budget-marker.md`
- `.agent/intents/intent-20260525-agent-context-budget-marker.md`
- `.agent/rules/context-budget.md`
- `src/crates/core/src/agentic/agents/prompts/intent_coding_mode.md`
- `src/crates/core/src/service/agent_memory/instruction_context.rs`

## Verification

- `rg -n "__context_budget__|omitted files|Omitted files|loaded the first 20|truncation marker" .agent/rules/context-budget.md src/crates/core/src/service/agent_memory/instruction_context.rs src/crates/core/src/agentic/agents/prompts/intent_coding_mode.md`: passed.
- `cargo test -p bitfun-core workspace_instruction_context_marks_omitted_agent_context_files -- --nocapture`: passed.
- `cargo test -p bitfun-core workspace_instruction_context_limits_agent_context_file_count -- --nocapture`: passed.
- `cargo test -p bitfun-core intent_coding -- --nocapture`: passed.

## Repair Loop

- Failure classes: type_error.
- Repair attempts: 1.
- Final repair status: repaired.
- Remaining verification gaps: full workspace tests were not run.

## Risk Handling

- Final risk level: L2
- Risk factors: Runtime prompt-context behavior changed.
- Verification matched expected level: yes, focused Rust tests cover the changed context-loading behavior.
- Skipped verification: full workspace tests were not run because this change is localized to workspace instruction context loading and prompt guidance.
- Review escalation: Not required for L2.

## Accepted Checks

- [x] Omitted context marker is emitted.
- [x] Omitted files are not loaded as full documents.
- [x] Rule documents marker behavior.
- [x] Prompt mentions omitted/truncated context markers.

## Accepted Tests

- `workspace_instruction_context_marks_omitted_agent_context_files`
- `workspace_instruction_context_limits_agent_context_file_count`
- `intent_coding_mode_uses_dedicated_prompt_and_planning_tools`

## Acceptance Coverage Result

- Automated: Focused Rust tests and text checks.
- Manual: Reviewed marker text and prompt wording.
- Coverage gaps: No full workspace test run.

## Risks

- Marker lists omitted file names, not contents.
- File-name disclosure is assumed acceptable for workspace-local `.agent` context files.
- The marker itself consumes prompt space when a bucket exceeds the file count limit.

## Human Review Focus

- Whether omitted filenames should be listed or only counted.
- Whether marker naming `__context_budget__.md` is the right convention.
- Whether the marker should include a stronger instruction for L2+ tasks.

## Metrics

- intent_created: true
- questions_asked: 0
- tests_or_checks_created: 4 checks, 4 verification commands
- verification_passed: true
- rework_needed: false

