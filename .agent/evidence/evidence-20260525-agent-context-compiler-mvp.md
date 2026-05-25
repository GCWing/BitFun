# Evidence Package

## Metadata

- Task: Add simplified Context Compiler directories
- Date: 2026-05-25
- Owner: Coding Agent
- Status: Complete

## Intent Record

`.agent/intents/intent-20260525-agent-context-compiler-mvp.md`

## Summary

Added the P1 simplified Context Compiler scaffold. BitFun now loads shallow Markdown context from `.agent/rules`, `.agent/knowledge`, and `.agent/changes` through the existing workspace instruction context. The Intent Coding prompt now names all three context buckets.

## Files Changed

- `.agent/changes/README.md`
- `.agent/evidence/evidence-20260525-agent-context-compiler-mvp.md`
- `.agent/intents/intent-20260525-agent-context-compiler-mvp.md`
- `.agent/knowledge/README.md`
- `.agent/templates/change-template.md`
- `.agent/templates/knowledge-template.md`
- `src/crates/core/src/agentic/agents/prompts/intent_coding_mode.md`
- `src/crates/core/src/service/agent_memory/instruction_context.rs`

## Verification

- `node -e "...JSON.parse(...)"`: passed for updated locale JSON files.
- `cargo test -p bitfun-core intent_coding -- --nocapture`: passed.
- `cargo test -p bitfun-core workspace_instruction_context_includes_agent_context_files -- --nocapture`: passed.

## Accepted Checks

- [x] `.agent/knowledge/README.md` exists.
- [x] `.agent/changes/README.md` exists.
- [x] `.agent/templates/knowledge-template.md` exists.
- [x] `.agent/templates/change-template.md` exists.
- [x] Context loader includes rules, knowledge, and changes.
- [x] Focused Rust test passes.

## Accepted Tests

- `workspace_instruction_context_includes_agent_context_files`

## Risks

- This is deterministic shallow loading, not retrieval or reranking.
- Large `.agent/knowledge` or `.agent/changes` directories could increase prompt size because P1 does not yet enforce a token budget.
- Remote workspace behavior keeps the existing prompt-builder branch: local instruction files are loaded only when no remote execution overlay is active.

## Human Review Focus

- Whether `.agent/changes` should be injected by default or only for Intent Coding mode.
- Whether README files should be excluded from context loading later if they become too verbose.
- Whether token limits should be added before teams put many files in `.agent/knowledge`.

## Metrics

- intent_created: true
- questions_asked: 0
- tests_or_checks_created: 6 checks, 1 focused test
- verification_passed: true
- rework_needed: false

