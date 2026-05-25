# Intent Record

## Metadata

- Task: Agent intent alignment MVP workflow scaffold
- Date: 2026-05-25
- Owner: Coding Agent
- Status: Accepted

## Original User Request

Define an MVP workflow where Coding Agent tasks produce an Intent Record before coding, clarify key ambiguity, generate acceptance checks or tests, run verification, and finish with an Evidence Package. Establish the initial `.agent/` directory structure with rules, intents, evidence, and templates.

## Agent Understanding

Create the repository-local workflow scaffold for a lightweight intent alignment loop. The first version should be document-based and enforceable by convention, not a full platform, policy engine, multi-agent workflow, or runtime integration.

## In Scope

- Add `.agent/rules/` with long-lived coding, architecture, and security constraints.
- Add `.agent/templates/` with reusable Intent Record and Evidence Package templates.
- Add this task's Intent Record under `.agent/intents/`.
- Add this task's Evidence Package under `.agent/evidence/`.
- Keep the change limited to documentation and workflow scaffolding.

## Out of Scope

- No runtime changes.
- No UI changes.
- No new dependencies.
- No OPA/Rego policy engine.
- No multi-agent Beads workflow.
- No automatic merge or repair router.
- No formal L3/L4 verification.

## Acceptance Criteria

- `.agent/rules/` contains stable repository constraints for coding style, architecture, and security.
- `.agent/templates/intent-template.md` captures user request, scope, clarifications, acceptance criteria/checks/tests, execution contract, and metrics.
- `.agent/templates/evidence-template.md` captures intent link, changed files, verification, accepted checks/tests, risks, review focus, and metrics.
- This task has an Intent Record and Evidence Package.
- Verification confirms the expected files exist and are visible in git status.

## Accepted Checks

- [x] `.agent/rules/coding-style.md` exists.
- [x] `.agent/rules/architecture.md` exists.
- [x] `.agent/rules/security.md` exists.
- [x] `.agent/templates/intent-template.md` exists.
- [x] `.agent/templates/evidence-template.md` exists.
- [x] `.agent/intents/intent-20260525-agent-intent-alignment-mvp.md` exists.
- [x] `.agent/evidence/evidence-20260525-agent-intent-alignment-mvp.md` exists.

## Accepted Tests

- Not applicable for this documentation-only scaffold.

## Clarification Questions

1. Should this MVP be implemented as documentation/workflow first, or wired into product runtime immediately?
2. Should the rules be English-only to match the repository docs, or bilingual?
3. Should future tasks require user confirmation for every Intent Record, or only when unresolved ambiguity remains?

## User Confirmations

- Proceeded with a documentation-first MVP because the requested P0 scope is `.agent/` directory and templates.
- Used English because the repository's root workflow documentation is English-first.
- Treated confirmation as required when ambiguity affects scope, safety, or acceptance.

## Execution Contract

Agent must:

- Read relevant files before editing.
- Reuse existing repository conventions and AGENTS guidance.
- Keep changes limited to `.agent/` workflow files.
- Run lightweight verification for file existence and diff review.
- Report any skipped verification.

Agent must not:

- Change product runtime, frontend UI, backend services, auth, billing, deployment, or database migration files.
- Introduce new dependencies.
- Broaden the MVP beyond the accepted intent.

## Metrics

- intent_created: true
- questions_asked: 3 recorded as design clarifications
- tests_or_checks_created: 7 checks
- verification_passed: true
- rework_needed: false
