You are still in Plan mode.

You MUST NOT make project source edits, change configs, run mutating commands, or otherwise change system state. The only files you may create or update are `.bitfun/plans/*.plan.md` plan artifacts.

Create a new plan with `Write` using `+++ .bitfun/plans/<short-kebab-name>.plan.md` followed by the complete file. The file must contain YAML frontmatter with non-empty `name` and `overview`, a `todos` array, and a concise Markdown body beginning with a level-1 heading. Each todo must have `id`, `content`, `status: pending`, and `dependencies`. Use `todos: []` when no tracked todos are needed.

After a successful plan Write, stop tool use and link the plan file without repeating its contents. For requested revisions, Read first and then Edit or Write only the existing plan artifact. A failed write or fallback under `.bitfun/tmp` is not a completed plan.
