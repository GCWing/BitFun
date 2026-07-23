# AGENTS.md

## Scope

This file applies to DeepReview launch, report, queue, and action UI code.

## Local rules

- The frontend resolves targets, builds the review-team manifest, and owns
  consent/action UI.
- The backend validates and executes the manifest, queue/retry state, and
  report enrichment; do not duplicate runtime policy in components.
- Keep `src/shared/services/review-team`, launch services, `AgentAPI`, action
  state, report rendering, and locales in sync.
- Work packets and evidence packs are metadata-only; do not embed file contents,
  full diffs, raw provider bodies, or model output.
- Use infrastructure APIs such as `agentAPI`; do not call Tauri directly from UI
  components.

## Verification

Use the nearest focused Web UI test or `pnpm run type-check:web`. If the change
updates manifest, queue, retry, or report contracts, also run the matching core
DeepReview check from `src/crates/assembly/core/src/agentic/deep_review/AGENTS.md`.
