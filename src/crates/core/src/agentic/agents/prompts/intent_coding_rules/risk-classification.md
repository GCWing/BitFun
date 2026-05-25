# Risk Classification Rules

Intent Coding tasks must classify risk before code edits. Use the lowest level that honestly matches the changed surface.

## Intent Record Requirement

Every Intent Record must include a machine-checkable risk line in `## Metadata`:

- `Risk level: L0`
- `Risk level: L1`
- `Risk level: L2`
- `Risk level: L3`
- `Risk level: L4`

For L3 and L4 tasks, the Intent Record must also include:

- `Review escalation: <planned review path>`

## Levels

### L0 Exploration

Use for prototypes, notes, documentation drafts, and throwaway local experiments.

Minimum verification:

- Syntax or file-existence checks when applicable.
- Manual accepted checks are acceptable.

### L1 Routine

Use for small UI changes, CRUD behavior, copy changes, straightforward tests, and narrow non-critical refactors.

Minimum verification:

- Focused tests or checks for the touched behavior.
- Typecheck/lint when frontend or typed contracts change.
- Cargo check/test for touched Rust logic when practical.

### L2 Important

Use for core business logic, cross-module behavior, persistence, synchronization, remote workspace behavior, or changes that can silently lose user work.

Minimum verification:

- Focused tests for new behavior.
- Relevant regression tests for adjacent behavior.
- Broader typecheck/check commands for the affected surface.
- Evidence Package must call out remaining gaps.

### L3 Critical

Use for authentication, authorization, data integrity, migrations, payment, encryption, release/signing, protocol parsing, or runtime ownership boundaries.

Minimum verification:

- L2 verification.
- Human review focus must be explicit.
- Deep Review or equivalent specialist review should be run when available.
- Intent Record must state the planned review escalation.
- Evidence Package must state whether Deep Review or equivalent specialist review was run.
- No automatic merge.

### L4 Safety-Critical

Use for cryptography, protocol correctness, sandbox boundaries, privilege escalation surfaces, destructive filesystem operations, or high-impact security controls.

Minimum verification:

- L3 verification.
- Security-focused review is mandatory.
- Formal/spec/property testing should be considered.
- Intent Record must state the planned specialist review path before coding.
- Evidence Package must state review results or the explicit reason review was skipped.
- No automatic merge.

## Risk Factors

Increase risk when a task touches:

- Auth, permissions, tokens, credentials, billing, release, deployment, migrations, or data deletion.
- Shared runtime loops, agent tool execution, prompt/tool schema contracts, stream parsing, or session persistence.
- Remote workspace behavior, synchronization, or multi-client control.
- Multiple modules or public APIs.
- Areas with recent defects or unclear ownership.

## Evidence Requirement

Every Evidence Package must record:

- Final risk level as `Final risk level: L0|L1|L2|L3|L4` in `## Risks`.
- Why that level was selected.
- Verification commands run.
- Verification that was skipped and why.
- Human review focus for L2 and above.
- Review escalation result for L3 and L4 as `Review escalation status: <completed|skipped|blocked>` in `## Risks`.

## Review Escalation

For L3 and L4 tasks:

- Prefer BitFun Deep Review when the changed surface is code and a review session is available.
- Use equivalent specialist review when Deep Review is unavailable or the task is not code-review shaped.
- Do not claim completion without stating whether review escalation was completed, skipped by explicit user direction, or blocked by tooling.
