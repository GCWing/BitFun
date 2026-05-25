# Accepted Checks and Tests Rules

Intent Coding tasks must translate user intent into verifiable acceptance before code edits.

## Minimum Acceptance

Every coding task should have:

- 1-3 Accepted Checks or Accepted Tests before implementation.
- At least one check that directly exercises the user's stated outcome.
- A clear statement of any behavior explicitly out of scope.

## Prefer Automated Tests When

Add or update automated tests when:

- The touched area already has nearby tests.
- The behavior is shared, reusable, or regression-prone.
- The task changes parsing, persistence, synchronization, API contracts, authorization, data integrity, or agent/tool execution.
- The task is L2 or higher.

## Manual Checks Are Acceptable When

Manual checks are acceptable when:

- The task is documentation-only.
- The project has no reasonable test harness for the touched surface.
- The change is visual/copy-only and a focused manual check is clearer than brittle automation.
- The user explicitly requests no test changes.

## Evidence Requirement

Every Evidence Package should record:

- Accepted Checks/Tests status.
- Which checks were automated.
- Which checks were manual.
- Any acceptance coverage gaps and why they remain.

## Good Accepted Checks

Good checks are specific and observable:

- "Selecting role=admin sends `role=admin` in the list request."
- "Clearing role filter removes the role query parameter."
- "`cargo test -p bitfun-core session_usage` passes."

Avoid vague checks:

- "Works correctly."
- "UI looks good."
- "Tests pass."

