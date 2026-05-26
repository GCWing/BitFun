# Context Compiler Rules

Intent Coding uses a lightweight context compiler for this MVP. It is not a
retrieval or ranking engine yet; it is a deterministic context policy for what
must be considered before coding.

## Built-In Context

The IntentCoding mode always loads a manifest of built-in rules before the rule
documents. The manifest states why each rule is included so reviewers can audit
which long-lived constraints influenced the task.

Built-in rules are product-owned prompt context. They are not loaded from
workspace `.agent` artifacts.

## Workspace Context

Before implementation, also read the nearest applicable workspace instructions:

- Repository-level `AGENTS.md` or `AGENTS-CN.md`.
- Nearest module `AGENTS.md` or `AGENTS-CN.md` for changed paths.
- Relevant architecture or contribution documents referenced by those files.

More specific workspace instructions override broader instructions when they
conflict.

## Task Context

Use task-local context to narrow implementation:

- User confirmations and clarified assumptions.
- Intent Record scope, out-of-scope items, and accepted checks.
- Existing code patterns near the files being changed.
- Verification commands required by repository or module guidance.

Do not broaden scope because a built-in rule mentions a capability that the user
did not request.

## Provenance Requirement

Evidence Packages must record key context inputs in `## Context Inputs`.

When available, generate initial context input candidates with:

```bash
pnpm run agent:context-compile -- --evidence <path>
```

Use one line per input:

```text
- [builtin_rule] intent_coding_rules/risk-classification.md: risk level selection
- [workspace_instruction] AGENTS.md: repository verification guidance
- [module_doc] src/crates/core/AGENTS.md: core ownership rules
- [source_file] src/crates/core/src/example.rs: matched existing implementation pattern
- [user_confirmation] chat: confirmed boundary behavior
- [verification_guidance] AGENTS.md: selected cargo test command
- [not_available] module_doc: reason: no nearer module guide exists
```

Valid types:

- `builtin_rule`
- `workspace_instruction`
- `module_doc`
- `source_file`
- `user_confirmation`
- `verification_guidance`
- `not_available`

Use `not_available` only with `reason: <reason>`.

The `## Provenance Chain` section should still link the Intent Record, Evidence
Package, session/turn anchors, and durable provenance record when available.

## Future Upgrade Path

A later Context Compiler can replace this deterministic policy with retrieval,
ranking, and context-budget controls. It must preserve the same reviewable
property: reviewers can see which context inputs influenced the task.
