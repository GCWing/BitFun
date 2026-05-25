# Change Note

## Task

Intent Coding MVP rollout status.

## Date

2026-05-25

## Context

The implementation is intentionally staged. The current MVP combines a new Intent Coding mode, workspace `.agent` workflow files, bounded context loading, risk/review/repair/provenance guidance, and focused tests. It does not yet enforce the full article architecture at runtime.

## Decisions

- Keep Intent Coding as a separate mode instead of changing default Agentic behavior.
- Persist Intent Records and Evidence Packages as workspace Markdown files first.
- Load `.agent/rules`, `.agent/knowledge`, and `.agent/changes` through existing workspace instruction context.
- Apply deterministic context budget limits before adding retrieval or reranking.
- Use prompt/template/rule guidance for risk labels, review escalation, repair loops, provenance, and acceptance coverage before runtime enforcement.

## Follow-Up Constraints

- Do not implement auto-merge without a reviewed Gate Pipeline design.
- Do not auto-trigger Deep Review until product UX and interruption behavior are designed.
- Do not add token-budget or retrieval logic without tests for omitted/truncated context behavior.
- Keep Evidence Packages compact; they should link or summarize evidence instead of copying full logs.

## Verification Gaps

- No rendered ChatInput mode-switch UI test yet.
- No runtime validation that every Intent Coding task writes an Intent Record and Evidence Package.
- No structured session provenance store yet.
- No automatic accepted-check status validator yet.
- No automatic risk classifier yet.

## Delete When

Delete or replace this note when Intent Coding has a structured product rollout document or session-level workflow state that supersedes these Markdown notes.

