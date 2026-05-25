# Intent Coding Workflow

This directory contains BitFun's MVP workflow for intent-aligned Coding Agent tasks.

The goal is not to recreate a full five-phase agent platform yet. The goal is a hard delivery constraint:

1. Capture the user's intent before coding.
2. Clarify only high-risk ambiguity.
3. Turn intent into accepted checks or tests.
4. Execute narrowly.
5. Run verification.
6. Deliver an Evidence Package.

## When to Use

Use the `IntentCoding` mode when a task needs code changes and the cost of misunderstanding the request is meaningful.

Good fits:

- Product behavior changes.
- Shared runtime, agent loop, tool, or prompt changes.
- UI flows where acceptance criteria matter.
- Refactors with scope boundaries.
- Risky fixes that need clear evidence.

Plain conversation, quick code explanation, or one-off inspection does not need a persisted Intent Record unless the user asks for one.

## Directory Map

- `rules/`: durable constraints and workflow rules.
- `templates/`: reusable Markdown templates for records and notes.
- `intents/`: task Intent Records, named `intent-YYYYMMDD-short-task-name.md`.
- `evidence/`: task Evidence Packages, named `evidence-YYYYMMDD-short-task-name.md`.
- `knowledge/`: stable project facts for the simplified Context Compiler.
- `changes/`: temporary rollout or task-context notes.

`README.md` files under `.agent/` are for humans and are skipped during automatic context injection. Put Agent-readable facts in named Markdown files under `rules/`, `knowledge/`, or `changes/`.

## Task Lifecycle

1. Read relevant repository files and nearest `AGENTS.md`.
2. Load relevant `.agent/rules`, `.agent/knowledge`, and `.agent/changes` context.
3. Create or update an Intent Record before editing code.
4. Ask at most 3 clarification questions when ambiguity is high-risk.
5. Record risk level, accepted checks/tests, scope, and execution contract.
6. Make scoped changes.
7. Run the smallest matching product verification command.
8. Write an Evidence Package.
9. Run the workflow structure check.
10. Summarize evidence and any remaining gaps in the final response.

## Required Verification

Run product verification that matches the touched surface. Examples:

- Frontend: `pnpm run lint:web`, `pnpm run type-check:web`, or focused Vitest commands.
- Core Rust: `cargo check --workspace`, `cargo test --workspace`, or focused package tests.
- Desktop integration: desktop-specific Rust checks or nearest E2E smoke flow.

Then run:

```bash
pnpm run agent:check
```

`agent:check` validates workflow structure only. It does not prove product behavior, replace tests, or validate that acceptance criteria are strong enough.

## Review Checklist

When reviewing an Intent Coding task, check:

- The Intent Record matches the user's request.
- Scope-in and scope-out sections are clear.
- Accepted checks/tests are specific enough to verify.
- Verification commands match the changed surface.
- The Evidence Package links to the Intent Record and records outcomes.
- Risks and human review focus call out meaningful gaps.
- `pnpm run agent:check` passed.

## Current MVP Limits

- No runtime enforcement that every task writes records.
- No CI gate for `agent:check` yet.
- No automatic risk classifier.
- No automatic accepted-check status validator.
- No structured session provenance store.
- No automatic Deep Review trigger for L3/L4 tasks.

These are deliberate P1/P2 follow-ups, not blockers for the MVP.
