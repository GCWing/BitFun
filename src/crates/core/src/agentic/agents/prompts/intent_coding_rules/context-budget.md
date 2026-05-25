# Context Budget Rules

Intent Coding rules are built into the mode binary and injected into every task context automatically. No workspace-level `.agent/` directory is required.

## Current MVP Limits

- Rules are embedded in the IntentCoding binary — no filesystem loading needed.
- Skip `README.md` files in context directories; they are human guidance and do not count toward the context budget.
- Rules have no file count or size limit since they are embedded at compile time.
- Rules reside in `src/crates/core/src/agentic/agents/prompts/intent_coding_rules/` in the codebase.
- Keep rules compact — large rules bloat the binary and the prompt context.

## Evidence Requirement

When context budget limits affect a task, the Evidence Package should mention:

- Which context directory was likely truncated or capped.
- Whether missing context could affect the result.
- Any follow-up recommendation to split or shorten context files.
- Whether omitted files listed in `__context_budget__.md` were inspected manually.
