# Context Budget Rules

The simplified Context Compiler loads shallow Markdown files from `.agent/rules`, `.agent/knowledge`, and `.agent/changes`. Keep this context compact and stable.

## Current MVP Limits

- Load only shallow `*.md` files from each context directory.
- Skip `README.md` files in context directories; they are human guidance and do not count toward the context budget.
- Load at most 20 files per context directory.
- Read at most 12,000 bytes from each context file.
- Truncate oversized files on a UTF-8 character boundary.
- When files are omitted by the file count limit, BitFun injects a `__context_budget__.md` marker for that directory.

## Authoring Guidance

- Prefer several focused notes over one large catch-all file.
- Keep durable facts in `.agent/knowledge`.
- Keep task-specific notes in `.agent/changes`.
- Keep enforcement-style constraints in `.agent/rules`.
- Put the highest-value files first alphabetically if a directory may exceed the file count limit.

## Evidence Requirement

When context budget limits affect a task, the Evidence Package should mention:

- Which context directory was likely truncated or capped.
- Whether missing context could affect the result.
- Any follow-up recommendation to split or shorten context files.
- Whether omitted files listed in `__context_budget__.md` were inspected manually.
