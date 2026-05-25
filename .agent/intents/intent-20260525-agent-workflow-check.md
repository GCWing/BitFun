# Intent Record

## Metadata

- Task: Add lightweight agent workflow checker
- Date: 2026-05-25
- Owner: Coding Agent
- Status: Accepted

## Original User Request

Continue implementing the intent-aligned Coding Agent workflow in BitFun.

## Agent Understanding

The next useful slice is a local validation command that checks whether the `.agent/` workflow artifacts are structurally complete. This keeps the MVP lightweight while making Intent Records and Evidence Packages easier to audit before any future CI or gate integration.

## In Scope

- Add a Node script under `scripts/` to validate `.agent/` directories, templates, intents, and evidence files.
- Add a package script so the check is easy to run.
- Validate required Markdown sections and Evidence-to-Intent references.
- Run the new checker.

## Out of Scope

- No CI integration.
- No automatic creation or mutation of records.
- No strict semantic validation of every checkbox.
- No dependency additions.

## Acceptance Criteria

- `pnpm run agent:check` exists.
- The checker verifies required `.agent/` directories and templates.
- The checker verifies Intent Records and Evidence Packages contain required sections.
- The checker verifies Evidence Package Intent Record paths exist.
- The checker passes on the current MVP artifacts.

## Risk Level

- Level: L1
- Reason: Repository tooling only; no product runtime behavior.
- Risk factors: Overly strict checks could block valid historical records.
- Verification expectation: Run the new checker.
- Review escalation: Not required for L1.

## Accepted Checks

- [x] `agent:check` script is available in `package.json`.
- [x] Checker validates required `.agent/` directories/templates.
- [x] Checker validates required Intent/Evidence sections.
- [x] Checker validates Evidence-to-Intent references.

## Accepted Tests

- `pnpm run agent:check`

## Acceptance Coverage Plan

- Automated: Run the new checker against current repository artifacts.
- Manual: Review script scope to confirm it stays structural and lightweight.
- Coverage gaps: Does not validate task-specific acceptance criteria semantics.

## Clarification Questions

No blocking question. Assumption: a lightweight manual check is preferable before wiring this into CI.

## User Confirmations

- User asked to continue after the mode-picker coverage slice.

## Provenance Anchors

- Context inputs: `.agent/templates/*`, `.agent/intents/*`, `.agent/evidence/*`, `package.json`, existing `scripts/*.mjs` style.
- User decisions: Continue the MVP implementation path.
- Related change notes: `.agent/changes/intent-coding-rollout.md`.

## Execution Contract

Agent must:

- Keep the checker dependency-free.
- Report actionable file/section errors.
- Keep validation structural rather than policy-heavy.
- Run the new check before delivery.

Agent must not:

- Add new dependencies.
- Modify historical artifact content just to satisfy arbitrary strictness.
- Wire the check into CI in this slice.

## Metrics

- intent_created: true
- questions_asked: 0
- tests_or_checks_created: 4 checks, 1 verification command
- verification_passed: true
- rework_needed: false
