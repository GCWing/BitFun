# Intent Record

## Metadata

- Task: Add Intent Coding mode picker display coverage
- Date: 2026-05-25
- Owner: Coding Agent
- Status: Accepted

## Original User Request

Continue implementing the intent-aligned Coding Agent workflow in BitFun.

## Agent Understanding

The next useful slice is frontend coverage closer to the mode picker experience. Existing tests cover frontend utility mapping for `IntentCoding`; this slice should verify the mode display data used by the picker can resolve translated names and descriptions without rendering the full ChatInput.

## In Scope

- Inspect ChatInput mode-display logic.
- Extract or add a focused helper if needed.
- Add tests for `IntentCoding` mode name/description resolution.
- Run focused web tests and type-check.

## Out of Scope

- No ChatInput UI redesign.
- No large rendered integration test.
- No mode ordering change.
- No new dependencies.

## Risk Level

- Level: L1
- Reason: Frontend test/helper coverage only.
- Risk factors: Small refactor risk if a helper is extracted.
- Verification expectation: Focused Vitest test and web type-check.
- Review escalation: Not required for L1.

## Acceptance Criteria

- `IntentCoding` mode display name resolves to localized `Intent Coding`.
- `IntentCoding` mode description resolves to localized description.
- Fallback behavior still works when localization is missing.
- Focused web verification passes.

## Accepted Checks

- [x] Mode display helper/test covers localized name.
- [x] Mode display helper/test covers localized description.
- [x] Fallback behavior is preserved.

## Accepted Tests

- Focused Vitest test.
- `pnpm run type-check:web`

## Acceptance Coverage Plan

- Automated: Focused frontend test and type-check.
- Manual: Review helper scope and imports.
- Coverage gaps: No full rendered ChatInput test.

## Clarification Questions

No blocking question. Assumption: focused helper coverage is preferable to a brittle full ChatInput render test for this slice.

## User Confirmations

- User asked to continue after knowledge/change notes were added.

## Provenance Anchors

- Context inputs: `src/web-ui/src/flow_chat/components/ChatInput.tsx`, `src/web-ui/src/app/scenes/agents/utils.test.ts`.
- User decisions: Continue the MVP implementation path.
- Related change notes: `.agent/changes/intent-coding-rollout.md`.

## Execution Contract

Agent must:

- Keep frontend changes focused.
- Avoid broad ChatInput refactors.
- Run focused verification.

Agent must not:

- Change mode behavior.
- Add dependencies.
- Redesign the mode picker.

## Metrics

- intent_created: true
- questions_asked: 0
- tests_or_checks_created: 3 checks, 2 verification commands
- verification_passed: true
- rework_needed: false
