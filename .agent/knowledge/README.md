# Agent Knowledge

Use this directory for durable knowledge that helps Coding Agents understand the product and repository.

Good candidates:

- Domain vocabulary and product concepts.
- Architecture decisions that are not already captured in ADRs.
- Known traps and historical mistakes.
- Invariants that should hold across many tasks.
- Review expectations that are stable over time.

Avoid:

- One-off task plans.
- Temporary investigation notes.
- Secrets, tokens, credentials, customer data, or private local configuration.
- Content that duplicates nearby `AGENTS.md` files without adding new context.

Files should be Markdown and concise enough to inject into Agent context.

