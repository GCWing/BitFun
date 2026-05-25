# Evidence Package

## Metadata

- Task: Add Intent Coding mode picker display coverage
- Date: 2026-05-25
- Risk Level: L1
- Status: Complete

## Intent Record

`.agent/intents/intent-20260525-intent-coding-mode-picker-coverage.md`

## Summary

Added a small mode-display helper for ChatInput and focused tests proving the `IntentCoding` mode resolves localized picker labels and preserves backend fallbacks when localization or descriptions are missing.

## Files Changed

- `src/web-ui/src/flow_chat/components/ChatInput.tsx`
- `src/web-ui/src/flow_chat/components/modeDisplay.ts`
- `src/web-ui/src/flow_chat/components/modeDisplay.test.ts`
- `.agent/intents/intent-20260525-intent-coding-mode-picker-coverage.md`
- `.agent/evidence/evidence-20260525-intent-coding-mode-picker-coverage.md`

## Verification

- `pnpm --dir src/web-ui run test:run src/flow_chat/components/modeDisplay.test.ts`: passed
- `pnpm run type-check:web`: passed

## Accepted Checks

- `IntentCoding` localized name resolves to `Intent Coding`.
- `IntentCoding` localized description resolves from `chatInput.modeDescriptions.IntentCoding`.
- Missing localization falls back to backend `name` and `description`.
- Missing description falls back to backend `name`.

## Acceptance Coverage Result

- Automated coverage: focused Vitest test for localized display and fallback behavior.
- Manual coverage: reviewed helper extraction in `ChatInput.tsx`; behavior remains display-only.
- Coverage gap: no full rendered ChatInput mode-picker integration test in this slice.

## Repair Loop

- Failures observed: none.
- Fix iterations: 0.
- Error class: not applicable.

## Risks

- No behavior change intended beyond moving display-name and display-description resolution into a helper.
- Full picker rendering remains covered indirectly by existing component behavior, not by this focused test.

## Human Review Focus

- Confirm the helper name and location fit frontend conventions.
- Confirm focused helper coverage is enough before adding a heavier ChatInput render test.

## Provenance Chain

- User request: continue implementing the intent-aligned Coding Agent workflow in BitFun.
- Context reviewed: `src/web-ui/src/flow_chat/components/ChatInput.tsx` and existing frontend agent utility tests.
- Intent captured: `.agent/intents/intent-20260525-intent-coding-mode-picker-coverage.md`.
- Implementation: extracted display resolution helper and added focused tests.
- Verification: focused Vitest and web type-check passed.
