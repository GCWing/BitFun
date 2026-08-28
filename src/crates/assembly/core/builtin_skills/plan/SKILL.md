---
name: plan
description: Research and write an actionable implementation plan without implementing it. Use when the user asks to plan, design an approach, or inspect a change before coding; stop applying the planning restriction once the user explicitly asks to implement.
---

# Plan

Produce a concise, evidence-backed implementation plan and leave project source unchanged.

## Workflow

1. Inspect the relevant code, configuration, documentation, and repository instructions using read-only tools. Resolve important behavior and ownership questions before drafting the plan.
2. Ask the user only when missing information would materially change the approach. Present concrete options and put the recommended option first. Do not ask whether to proceed merely because research is complete.
3. Use `Task` only for read-only research that materially improves the plan. Keep delegated scopes bounded and synthesize the findings yourself.
4. Write one plan artifact at `.bitfun/plans/<short-kebab-name>.plan.md`. Use a concise descriptive filename, and do not overwrite a different existing plan blindly.
5. After a successful plan Write, stop tool use and link the plan file without repeating its contents.

## Plan Artifact

Use `Write` with a payload containing the path header followed by the complete file:

```text
+++ .bitfun/plans/<short-kebab-name>.plan.md
<complete plan file>
```

The file must start with YAML frontmatter in this shape. Always include `todos`; use `todos: []` for a simple plan. Every todo starts as `pending`, has a stable kebab-case ID, and includes `dependencies`, using `[]` when it has none.

```yaml
---
name: Short Plan Name
overview: One or two sentence overview
todos:
  - id: stable-todo-id
    content: Specific actionable task
    status: pending
    dependencies: []
---
```

Follow the frontmatter with a non-empty Markdown body whose first line is a level-1 heading. Keep the plan proportional to the request and cite specific workspace-relative files with Markdown links when useful.

## Rules

- Do not edit project source, change configuration, run mutating commands, or otherwise implement the task while this planning workflow applies. The only permitted mutation is a `.bitfun/plans/*.plan.md` artifact.
- If the plan Write fails or falls back under `.bitfun/tmp`, fix only the plan artifact write and retry; that fallback is not a completed plan.
- For a requested revision, Read the existing plan first and then use Edit or Write only on that same `.bitfun/plans/*.plan.md` file. Do not create another plan card merely to revise it.
- The successful plan Write is the final tool call for the planning turn.
- Once the user explicitly approves the plan or asks to implement, leave this planning workflow and perform the requested work normally. This skill does not require an Agent or mode switch.
