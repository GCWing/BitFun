---
name: pr-review-canvas
description: 'Create a BitFun Canvas for reviewing a pull request, branch diff, or change set with Cursor-style diff cards, review maps, risk callouts, and focused reviewer flow. Use when the user asks for a PR review canvas, diff walkthrough, change-set overview, or visual review summary.'
---

# PR Review Canvas

Use this skill to produce a session-scoped BitFun Canvas that helps a reviewer understand a PR quickly. The artifact should reorganize the diff for reviewer comprehension, not mirror file-tree order. It should look and feel like a Cursor Canvas review: compact metadata, focused diff stats, pill filters, file diff cards, tables, callouts, traces, and reviewer-facing notes.

Read and follow `bitfun-canvas` first. It defines the Canvas tool workflow, source rules, SDK surface, and design constraints.

## Inputs

Gather the change set before writing TSX:

- If the user gives a GitHub PR URL or number, use `gh pr view` and `gh pr diff` when available.
- If the user explicitly asks for current local changes, use `git diff`, `git diff --stat`, and `git diff --name-status`.
- If the user gives a branch/range, use that exact range.
- If the diff source is ambiguous, ask which PR, branch, or local diff to review. Do not guess from the current branch.

Collect:

- PR title, repo, number/link, author, base/head, status, and update time when available.
- File stats, additions/deletions, generated/mechanical files, test files, and risky files.
- Core hunks with enough context to understand behavior.
- Verification commands or CI checks when visible.

## Canvas Structure

Organize by review importance, not file-tree order:

1. Review map: PR identity, scope, diff stats, state, top risks.
2. Core logic: behavior changes first, with file cards and `DiffView` snippets.
3. Wiring and integration: routes, registration, config, dependency injection, feature flags.
4. Tests and verification: added/changed tests, missing coverage, commands run or expected.
5. Mechanical changes: imports, renames, generated files, formatting. Summarize in compact lists or tables instead of dumping.
6. Reviewer checklist: concrete questions, risk callouts, suggested review focus.

The first screen should already be useful to a reviewer. Lead with the behavior that matters most and one compact map of the change: risk summary, file groups, or a before/after flow. Do not make the reviewer scroll past metadata before seeing the core change.

Use Cursor-style compositions:

```tsx
<Card>
  <CardHeader trailing={<DiffStats additions={12} deletions={3} />}>
    src/example.ts
  </CardHeader>
  <CardBody style={{ padding: 0 }}>
    <DiffView path="src/example.ts" lines={lines} />
  </CardBody>
</Card>
```

Use `Pill` groups for filters or selected file tabs, `Table` for checks and file summaries, `Callout` for subtle or risky behavior, and `TextArea` only for local review notes.

## Review Guidance

- Lead with the behavior that matters most.
- Add pseudocode when dense logic is easier to review that way.
- Add a concrete before/after trace for tricky state transitions.
- Mark surprising hunks with short callout labels: `Subtle`, `Breaking`, `Race condition`, `Performance`, `Test gap`.
- Keep comments reviewer-facing: explain why a change matters and how files interact.
- Keep raw diff snippets focused. Prefer 5-30 meaningful lines per file card, not full-file dumps.
- Separate boilerplate from core logic. Put generated/mechanical/test fixture churn behind `CollapsibleSection` unless it is itself the risk.
- For refactors, show the old data flow and new data flow side by side or as a compact call graph.
- For bug fixes, show the failing input, old outcome, new outcome, and the exact hunk that changes it.
- For new features, show the request path or state machine before listing files.
- Use a small table for file grouping: core, wiring, tests, mechanical. Then expand only the core group with diff cards.

## Creative Representations

Pick the representation that fits the diff instead of always producing the same card stack:

- State transition diagram for workflow or lifecycle changes.
- Before/after call graph for refactors and dependency changes.
- Input -> output matrix for parser, validation, or transformation changes.
- Timeline for migrations and multi-step jobs.
- Risk heat table for broad PRs with many files.
- One large callout with most files collapsed for tiny but dangerous changes.

Use `computeDAGLayout` plus inline SVG when a call graph or flow map is clearer than a table. Use `DiffView` only where code context is necessary.

## Do Not

- Do not present the PR in file-tree order when that hides the main behavior.
- Do not dump every changed file into a diff card.
- Do not create repository files unless the user explicitly asks.
- Do not include unsupported imports or external packages.
- Do not make a marketing page. The first screen should already be useful to a reviewer.
- Do not fake CI, authorship, or PR metadata. Mark unavailable facts as unavailable.
- Do not wrap every section in a `Card`. Use cards for file-level diffs and high-signal framed summaries only.

## Output Self-Check

Before calling `CreateCanvas`, verify:

- The sections are ordered by reviewer value, not file path.
- Core logic appears before mechanical churn.
- Dense logic has pseudocode, a trace, or a flow/call graph if that would help.
- Raw diff snippets are focused and not full-file dumps.
- The first screen includes the main behavior/risk, not only metadata.
- The design passes the `bitfun-canvas` slop-pattern check.

## Output

Call `CreateCanvas` with a concise title and the complete TSX source. In the final response, give the returned `bitfun-canvas://...` artifact reference and mention the diff source used.
