# Flow Chat Deep Review Ownership

This directory owns Flow Chat integration for Deep Review. Keep the historical import paths under `src/web-ui/src/flow_chat/services`, `components/btw`, and `utils` as compatibility facades.

## Module Boundaries

| Area | Owns | Should not own |
|---|---|---|
| `launch/` | Slash command parsing, review target resolution, launch prompt formatting, launch error shaping, child-session launch orchestration. | Review Team manifest policy internals, direct Tauri calls, action-bar state. |
| `action-bar/` | Shared review action bar rendering, Deep Review queue notice, partial-results panel, recovery-plan preview, remediation selection, action controls, diagnostics text building, compact status/header formatting. | Launch target parsing, report markdown semantics, backend queue classification. |
| `report/` | Code review report types, retryable slice extraction, reliability notices, run-manifest markdown sections, report section normalization, markdown export. | UI rendering, session launch, raw source/diff/model output storage. |

## Guardrails

- Preserve the facade exports from `services/DeepReviewService.ts`, `components/btw/DeepReviewActionBar.tsx`, and `utils/codeReviewReport.ts`.
- Keep Deep Review-only UI gated by `reviewMode === 'deep'` or an explicit Deep Review queue/report context.
- Standard Code Review remediation and markdown export must keep focused regression coverage when report or action-bar helpers move.
- Diagnostics, markdown export, and evidence summaries must stay metadata-first and must not include source text, full diff text, reviewer output, provider raw bodies, or full file contents.
- Add behavior to the narrow helper module first. Only grow the orchestration files when the behavior actually coordinates multiple helpers or adapters.

## Focused Verification

```powershell
pnpm --dir src/web-ui exec vitest run src/flow_chat/services/DeepReviewService.test.ts src/flow_chat/deep-review/action-bar/PartialResultsPanel.test.tsx src/flow_chat/deep-review/action-bar/RecoveryPlanPreview.test.tsx src/flow_chat/deep-review/action-bar/RemediationSelectionPanel.test.tsx src/flow_chat/deep-review/action-bar/ReviewActionControls.test.tsx src/flow_chat/deep-review/action-bar/interruptionDiagnostics.test.ts src/flow_chat/components/btw/DeepReviewActionBar.test.tsx src/flow_chat/utils/codeReviewReport.test.ts
pnpm run type-check:web
pnpm run lint:web
```
