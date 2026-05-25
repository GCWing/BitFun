# Evidence Package

## Metadata

- Task: Add lightweight agent workflow checker
- Date: 2026-05-25
- Risk Level: L1
- Status: Complete

## Intent Record

`.agent/intents/intent-20260525-agent-workflow-check.md`

## Summary

Added a dependency-free local checker for the `.agent/` MVP workflow. The checker validates required directories, required templates, Intent Record sections, Evidence Package sections, Evidence-to-Intent references, and matching Intent/Evidence task slugs.

## Files Changed

- `package.json`
- `scripts/check-agent-workflow.mjs`
- `.agent/intents/intent-20260525-agent-workflow-check.md`
- `.agent/evidence/evidence-20260525-agent-workflow-check.md`

## Verification

- `pnpm run agent:check`: passed

## Accepted Checks

- `agent:check` script is available in `package.json`.
- Checker validates required `.agent/` directories/templates.
- Checker validates required Intent/Evidence sections.
- Checker validates Evidence-to-Intent references.

## Acceptance Coverage Result

- Automated coverage: `pnpm run agent:check` passed.
- Manual coverage: script reviewed for structural, dependency-free validation.
- Coverage gap: does not validate task-specific acceptance criteria semantics or checkbox truth.

## Repair Loop

- Failures observed: none.
- Fix iterations: 0.
- Error class: not applicable.

## Risks

- The checker is intentionally structural and may not catch weak acceptance criteria.
- The checker is not wired into CI in this slice.

## Human Review Focus

- Confirm required sections are strict enough for MVP but not too strict for normal iteration.
- Confirm `agent:check` should remain manual until the workflow stabilizes.

## Provenance Chain

- User request: continue implementing the intent-aligned Coding Agent workflow in BitFun.
- Context reviewed: existing `.agent/` artifacts, `package.json`, and repository script style.
- Intent captured: `.agent/intents/intent-20260525-agent-workflow-check.md`.
- Implementation: added `scripts/check-agent-workflow.mjs` and `pnpm run agent:check`.
- Verification: `pnpm run agent:check` passed.
