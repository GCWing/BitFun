# Evidence Package

## Metadata

- Task: Add MVP context budget limits for `.agent` context loading
- Date: 2026-05-25
- Owner: Coding Agent
- Status: Complete

## Intent Record

`.agent/intents/intent-20260525-agent-context-budget-mvp.md`

## Summary

Added deterministic budget limits to simplified Context Compiler loading. `.agent/rules`, `.agent/knowledge`, and `.agent/changes` now load at most 20 shallow Markdown files per directory, and each file is truncated to 12,000 bytes on a UTF-8 character boundary.

## Provenance Chain

- Original request: User asked to continue implementing the intent-aligned Coding Agent workflow.
- Context inputs: `src/crates/core/src/service/agent_memory/instruction_context.rs`, `src/crates/core/src/agentic/agents/prompts/intent_coding_mode.md`, `.agent/rules/context-budget.md`.
- Intent Record: `.agent/intents/intent-20260525-agent-context-budget-mvp.md`.
- Acceptance: Add context budget rule, enforce file count and file size limits, update prompt, and verify with focused tests.
- Execution: Added constants and truncation helper in the context loader, plus tests for count and truncation behavior.
- Verification: Text check with `rg`; focused Rust tests for budget behavior and prompt embedding.
- Repair loop: No failures; repair status `not_needed`.
- Review escalation: Not required for L2, but human review should check chosen defaults.
- Evidence Package: `.agent/evidence/evidence-20260525-agent-context-budget-mvp.md`.

## Files Changed

- `.agent/evidence/evidence-20260525-agent-context-budget-mvp.md`
- `.agent/intents/intent-20260525-agent-context-budget-mvp.md`
- `.agent/rules/context-budget.md`
- `src/crates/core/src/agentic/agents/prompts/intent_coding_mode.md`
- `src/crates/core/src/service/agent_memory/instruction_context.rs`

## Verification

- `rg -n "Context Budget|Load at most 20|12,000 bytes|context is budgeted|truncated to 12000" .agent/rules/context-budget.md src/crates/core/src/service/agent_memory/instruction_context.rs src/crates/core/src/agentic/agents/prompts/intent_coding_mode.md`: passed.
- `cargo test -p bitfun-core workspace_instruction_context_limits_agent_context_file_count -- --nocapture`: passed.
- `cargo test -p bitfun-core workspace_instruction_context_truncates_large_agent_context_files -- --nocapture`: passed.
- `cargo test -p bitfun-core workspace_instruction_context_includes_agent_context_files -- --nocapture`: passed.
- `cargo test -p bitfun-core intent_coding -- --nocapture`: passed.

## Repair Loop

- Failure classes: none observed.
- Repair attempts: 0.
- Final repair status: not_needed.
- Remaining verification gaps: full workspace tests were not run.

## Risk Handling

- Final risk level: L2
- Risk factors: Runtime prompt-context completeness changes for `.agent` context files.
- Verification matched expected level: yes, focused Rust tests cover the changed behavior.
- Skipped verification: full workspace tests were not run because the change is limited to context loading and prompt guidance.
- Review escalation: Not required for L2.

## Accepted Checks

- [x] Context budget rule exists.
- [x] Loader has a file count limit.
- [x] Loader has a UTF-8 safe file size limit.
- [x] Focused Rust tests pass.
- [x] Intent Coding prompt mentions budgeted context.

## Accepted Tests

- `workspace_instruction_context_limits_agent_context_file_count`
- `workspace_instruction_context_truncates_large_agent_context_files`
- `workspace_instruction_context_includes_agent_context_files`
- `intent_coding_mode_uses_dedicated_prompt_and_planning_tools`

## Risks

- Limits are byte-based, not token-based.
- When more than 20 files exist in one context directory, later alphabetical files are omitted from automatic context.
- Large files are truncated with a marker, but the Agent must explicitly read the full file if omitted context may matter.

## Human Review Focus

- Whether 20 files per directory and 12,000 bytes per file are the right defaults.
- Whether README files should count toward the 20-file limit.
- Whether future structured provenance should record omitted/truncated context explicitly.

## Metrics

- intent_created: true
- questions_asked: 0
- tests_or_checks_created: 5 checks, 5 verification commands
- verification_passed: true
- rework_needed: false

