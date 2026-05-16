# AGENTS.md

## Scope

This file applies to DeepReview launch, report, queue, and action UI code.

## Local rules

- The frontend resolves targets, builds the review-team manifest, and owns
  consent/action UI.
- The backend validates and executes the manifest; do not duplicate runtime
  policy in components.
- Keep `src/shared/services/review-team`, launch services, `AgentAPI`, action
  state, report rendering, and locales in sync.
- Work packets and evidence packs are metadata-only; do not embed file contents,
  full diffs, raw provider bodies, or model output.
- Use infrastructure APIs such as `agentAPI`; do not call Tauri directly from UI
  components.
