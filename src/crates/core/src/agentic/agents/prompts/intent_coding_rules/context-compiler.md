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

Evidence Packages should record key context inputs in `## Provenance Chain`.
At minimum, mention the built-in Intent Coding rules and any workspace
instructions or module documents that affected the implementation.

## Future Upgrade Path

A later Context Compiler can replace this deterministic policy with retrieval,
ranking, and context-budget controls. It must preserve the same reviewable
property: reviewers can see which context inputs influenced the task.
