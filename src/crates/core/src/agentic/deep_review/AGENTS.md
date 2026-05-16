# AGENTS.md

## Scope

This file applies to DeepReview runtime internals in this directory.

## Local rules

- Keep this code platform-agnostic; use shared events, config, and tool context.
- Keep policy, manifest admission, queue state, retry metadata, task adapter,
  and report enrichment aligned.
- Keep default team/runtime contracts aligned with `deep_review_policy.rs` and
  reviewer agents in `src/crates/core/src/agentic/agents`.
- Reviewer subagents stay read-only; `ReviewFixer` is not part of the review
  pass.
- When queue or report fields change, update the matching frontend DTOs and
  DeepReview UI state.
