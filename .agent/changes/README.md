# Agent Changes

Use this directory for task-level change context that should be visible to future Coding Agent runs.

Good candidates:

- Important decisions made during a task.
- Follow-up constraints discovered during implementation.
- Known verification gaps that need future attention.
- Migration notes while a feature is in progress.

Avoid:

- Full logs or large command output.
- General domain knowledge that belongs in `.agent/knowledge/`.
- Evidence packages, which belong in `.agent/evidence/`.
- Intent records, which belong in `.agent/intents/`.

Files should be Markdown and should state when the note can be deleted.

