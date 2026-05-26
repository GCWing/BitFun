# Agent Workflow Check Rule

Intent Coding tasks should run the local workflow structure checker when the workspace provides one.

## Command

```bash
pnpm run agent:check
```

For L3/L4 review routing handoff:

```bash
pnpm run agent:review-route -- --evidence .agent/evidence/evidence-YYYYMMDD-task.md
```

For session provenance record export:

```bash
pnpm run agent:provenance-record -- --evidence .agent/evidence/evidence-YYYYMMDD-task.md --session-id <id> --turn-id <id>
```

For context input candidate generation:

```bash
pnpm run agent:context-compile -- --evidence .agent/evidence/evidence-YYYYMMDD-task.md
```

## When to Run

- After the Intent Record and Evidence Package have been written or updated.
- Before the final response for any coding task that changes Intent Record or Evidence Package artifacts.
- Alongside product verification such as Rust tests, web tests, type-checks, lint, or builds.
- In CI as a lightweight structural gate when the repository provides the script.

## Scope

The checker validates structural workflow hygiene:

- Intent Records and Evidence Packages exist and pair 1:1 by task slug.
- Intent Records contain required MVP sections.
- Intent Records include a machine-checkable risk level.
- L3/L4 Intent Records include a planned review escalation path.
- Evidence Packages contain required MVP sections.
- Evidence Packages reference existing Intent Records.
- Intent Records and Evidence Packages are paired by task slug.
- Evidence Package context inputs include machine-checkable source types and reasons.
- Evidence Package accepted checks include explicit status markers.
- Evidence Package repair loops include attempt counts and final repair status.
- Evidence Package provenance chains include machine-checkable store, session, turn, Intent Record, Evidence Package, and durable record anchors.
- Evidence Package policy gates include built-in/configured gate profiles, machine-checkable statuses, and failure/skipped/blocked handling.
- Evidence Package risks include a final risk level.
- L3/L4 Intent Records include a machine-checkable review route.
- L3/L4 Evidence Packages include review route, trigger mode, and escalation status.
- L3/L4 review routes can be converted into a review handoff plan.
- Evidence Package changed files, risk-sensitive Evidence text, ownership-sensitive surfaces, and dependency-impact files produce an advisory risk-level suggestion.

## Limits

This check does not prove that the code is correct, the acceptance criteria are strong, or the product behavior works. It must not replace the smallest matching product verification command.
