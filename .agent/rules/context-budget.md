# Context Budget Rules

The simplified Context Compiler loads shallow Markdown files from `.agent/rules`. Keep this context compact and stable.

## Current MVP Limits

- Load only shallow `*.md` files from `.agent/rules`.
- Skip `README.md` files in context directories; they are human guidance and do not count toward the context budget.
- Load at most 20 files from `.agent/rules`.
- Read at most 12,000 bytes from each context file.
- Truncate oversized files on a UTF-8 character boundary.
- When files are omitted by the file count limit, BitFun injects a `__context_budget__.md` marker.

## Authoring Guidance

- Prefer several focused rules over one large catch-all file.
- Keep constraints in `.agent/rules`.
- Put the highest-value files first alphabetically if rules may exceed the file count limit.

## Evidence Requirement

When context budget limits affect a task, the Evidence Package should mention:

- Which context directory was likely truncated or capped.
- Whether missing context could affect the result.
- Any follow-up recommendation to split or shorten context files.
- Whether omitted files listed in `__context_budget__.md` were inspected manually.
