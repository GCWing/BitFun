# Evidence Package

## Metadata

- Task: Skip `.agent` bucket README files during context injection
- Date: 2026-05-25
- Owner: Coding Agent
- Status: Complete

## Intent Record

`.agent/intents/intent-20260525-agent-context-readme-skip.md`

## Summary

Updated simplified Context Compiler loading so shallow `README.md` files inside `.agent/rules`, `.agent/knowledge`, and `.agent/changes` are skipped. These README files remain available for humans but no longer consume prompt context budget.

## Provenance Chain

- Original request: User asked to continue implementing the intent-aligned Coding Agent workflow.
- Context inputs: `src/crates/core/src/service/agent_memory/instruction_context.rs`, `.agent/rules/context-budget.md`.
- Intent Record: `.agent/intents/intent-20260525-agent-context-readme-skip.md`.
- Acceptance: Skip bucket README files, ensure they do not count toward budget, update rule, focused tests pass.
- Execution: Added `is_agent_context_readme` filter and a focused skip/budget test.
- Verification: Text check with `rg`; focused Rust tests for README skip and omission marker behavior.
- Repair loop: No failures; repair status `not_needed`.
- Review escalation: Not required for L2.
- Evidence Package: `.agent/evidence/evidence-20260525-agent-context-readme-skip.md`.

## Files Changed

- `.agent/evidence/evidence-20260525-agent-context-readme-skip.md`
- `.agent/intents/intent-20260525-agent-context-readme-skip.md`
- `.agent/rules/context-budget.md`
- `src/crates/core/src/service/agent_memory/instruction_context.rs`

## Verification

- `rg -n "README.md|is_agent_context_readme|Human guidance|context budget" .agent/rules/context-budget.md src/crates/core/src/service/agent_memory/instruction_context.rs`: passed.
- `cargo test -p bitfun-core workspace_instruction_context_skips_agent_context_readmes -- --nocapture`: passed.
- `cargo test -p bitfun-core workspace_instruction_context_marks_omitted_agent_context_files -- --nocapture`: passed.

## Repair Loop

- Failure classes: none observed.
- Repair attempts: 0.
- Final repair status: not_needed.
- Remaining verification gaps: full workspace tests were not run.

## Risk Handling

- Final risk level: L2
- Risk factors: Runtime prompt-context behavior changed for `.agent` README files.
- Verification matched expected level: yes, focused Rust tests cover README skip and existing omission marker behavior.
- Skipped verification: full workspace tests were not run because the behavior is localized to context loading.
- Review escalation: Not required for L2.

## Accepted Checks

- [x] README files are skipped.
- [x] README files do not consume context file budget.
- [x] Context budget rule documents README skip behavior.
- [x] Focused Rust tests pass.

## Accepted Tests

- `workspace_instruction_context_skips_agent_context_readmes`
- `workspace_instruction_context_marks_omitted_agent_context_files`

## Acceptance Coverage Result

- Automated: Focused Rust tests and text checks.
- Manual: Reviewed skip scope to ensure root `AGENTS.md`/`CLAUDE.md` remain unaffected.
- Coverage gaps: No full workspace test run.

## Risks

- If a team intentionally stores important Agent context in a bucket README, it will no longer be injected automatically.
- Teams should move durable facts into named Markdown notes instead of README files.

## Human Review Focus

- Whether skipping README should apply to `.agent/rules` as well as knowledge/changes.
- Whether skipped README behavior should be mentioned in `.agent/knowledge/README.md` and `.agent/changes/README.md`.

## Metrics

- intent_created: true
- questions_asked: 0
- tests_or_checks_created: 4 checks, 3 verification commands
- verification_passed: true
- rework_needed: false

