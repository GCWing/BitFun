# Intent Record

## Metadata

- Task: Add simplified Context Compiler directories
- Date: 2026-05-25
- Owner: Coding Agent
- Status: Accepted

## Original User Request

Continue implementing the intent-aligned Coding Agent workflow in BitFun.

## Agent Understanding

The next useful slice is a simplified Context Compiler: keep durable rules, domain knowledge, and task/change notes in workspace `.agent/` directories, and inject those files through BitFun's existing workspace instruction context. This strengthens Phase A from the reference article without adding search, ranking, vector retrieval, or a full knowledge platform.

## In Scope

- Add `.agent/knowledge/` and `.agent/changes/` scaffold files and templates.
- Extend workspace instruction context loading from `.agent/rules/*.md` to also include `.agent/knowledge/*.md` and `.agent/changes/*.md`.
- Keep loading deterministic and shallow for P1.
- Update Intent Coding prompt to name the three context buckets.
- Add/update focused tests.

## Out of Scope

- No vector retrieval or BM25.
- No LLM reranking.
- No token-budget optimizer.
- No nested directory crawler.
- No UI for editing knowledge or changes.
- No new dependencies.

## Acceptance Criteria

- `.agent/knowledge/README.md` documents what belongs in domain knowledge.
- `.agent/changes/README.md` documents what belongs in task/change notes.
- Templates exist for knowledge and change notes.
- Workspace instruction context includes markdown files from `.agent/rules`, `.agent/knowledge`, and `.agent/changes`.
- Focused Rust test covers all three `.agent` context buckets.
- Intent Coding prompt references the simplified Context Compiler buckets.

## Accepted Checks

- [x] `.agent/knowledge/README.md` exists.
- [x] `.agent/changes/README.md` exists.
- [x] `.agent/templates/knowledge-template.md` exists.
- [x] `.agent/templates/change-template.md` exists.
- [x] Context loader includes rules, knowledge, and changes.
- [x] Focused Rust test passes.

## Accepted Tests

- `workspace_instruction_context_includes_agent_context_files`

## Clarification Questions

No blocking question. Assumption: P1 should remain file-based and deterministic instead of implementing retrieval/reranking.

## User Confirmations

- User asked to continue after the P0/P1 Intent Coding mode implementation.

## Execution Contract

Agent must:

- Keep this change limited to context scaffold and loader behavior.
- Reuse the existing workspace instruction context path.
- Avoid new dependencies.
- Run focused verification.

Agent must not:

- Build a full Context Compiler.
- Add a UI workflow.
- Change existing Agentic mode semantics.

## Metrics

- intent_created: true
- questions_asked: 0
- tests_or_checks_created: 6 checks, 1 focused test
- verification_passed: true
- rework_needed: false
