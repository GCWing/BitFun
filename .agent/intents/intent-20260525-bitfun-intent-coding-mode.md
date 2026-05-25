# Intent Record

## Metadata

- Task: Implement BitFun Intent Coding MVP
- Date: 2026-05-25
- Owner: Coding Agent
- Status: Accepted

## Original User Request

Implement the intent-aligned Coding Agent workflow from the referenced article in the BitFun project, following the recommendation to start with a separate Intent Coding mode and workspace `.agent/` files.

## Agent Understanding

Add a first BitFun-native MVP for intent alignment without building the full five-phase platform. The new workflow should be available as a separate coding mode, load workspace `.agent/rules` as lightweight Context Compiler input, and instruct the Agent to produce Intent Records, clarification questions, Accepted Checks/Tests, verification, and Evidence Packages before considering a coding task complete.

## In Scope

- Add an independent Intent Coding mode in core.
- Add an embedded prompt for the new mode.
- Include `.agent/rules/*.md` in request context where workspace instruction context is built.
- Add or update frontend mode labels/locales so users can select the mode.
- Keep persistent Intent/Evidence artifacts as workspace `.agent/` markdown files for P0.
- Add focused tests where practical.

## Out of Scope

- No full Disagreement Detector with multi-candidate execution.
- No Beads task scheduler.
- No OPA/Rego policy engine.
- No automatic merge.
- No formal L3/L4 verification.
- No deep UI workflow for approving Intent Records.
- No new dependencies.

## Acceptance Criteria

- Intent Coding appears as a built-in mode with its own prompt template.
- The mode has coding tools plus `AskUserQuestion` and planning capability.
- The prompt requires Intent Record before code edits, up to 3 high-risk clarification questions, Accepted Checks/Tests, verification, and Evidence Package.
- Workspace `.agent/rules/*.md` files are loaded into the agent request context when present.
- Existing Agentic and Plan behavior remain available.
- Focused verification passes or skipped verification is documented.

## Accepted Checks

- [x] New core mode is registered.
- [x] New prompt file is embedded and referenced.
- [x] `.agent/rules` context builder is covered by a focused test or equivalent check.
- [x] Frontend mode labels include Intent Coding.
- [x] No new dependencies are added.

## Accepted Tests

- Run focused Rust tests for prompt/request context changes.
- Run focused web tests if locale/mode UI logic changes include nearby tests.

## Clarification Questions

1. Should the first version be a separate mode or default Code Agent behavior?
2. Should Intent/Evidence persist first in workspace `.agent/` or session storage?

## User Confirmations

- Use the recommended approach.
- Implement as a separate mode.
- Use workspace `.agent/` files first.

## Execution Contract

Agent must:

- Read relevant mode, prompt builder, registry, and frontend mode files before editing.
- Reuse BitFun's existing Agent mode, prompt, and request-context patterns.
- Keep changes limited to the MVP workflow surface.
- Run focused verification.
- Report skipped broad verification.

Agent must not:

- Add dependencies.
- Change existing Agentic mode semantics.
- Build a full platform, gate pipeline, Beads scheduler, or formal verification layer in this task.
- Modify auth, billing, deployment, release, or database migration files.

## Metrics

- intent_created: true
- questions_asked: 2 answered by user direction
- tests_or_checks_created: 5 checks
- verification_passed: true
- rework_needed: false
