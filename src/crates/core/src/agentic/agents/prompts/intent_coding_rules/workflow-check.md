# Agent Workflow Check Rule

Intent Coding tasks should run the local workflow structure checker when the workspace provides one.

## Command

```bash
pnpm run agent:check
```

## When to Run

- After the Intent Record and Evidence Package have been written or updated.
- Before the final response for any coding task that changes Intent Record or Evidence Package artifacts.
- Alongside product verification such as Rust tests, web tests, type-checks, lint, or builds.

## Scope

The checker validates structural workflow hygiene:

- Intent Records and Evidence Packages exist and pair 1:1 by task slug.
- Intent Records contain required MVP sections.
- Evidence Packages contain required MVP sections.
- Evidence Packages reference existing Intent Records.
- Intent Records and Evidence Packages are paired by task slug.

## Limits

This check does not prove that the code is correct, the acceptance criteria are strong, or the product behavior works. It must not replace the smallest matching product verification command.
