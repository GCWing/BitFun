# AGENTS.md

## Scope

This file applies to DeepReview runtime internals in this directory.

## Local rules

- Keep this code platform-agnostic; use shared events, config, and tool context.
- Keep policy, manifest admission, queue state, retry metadata, task adapter,
  and report enrichment aligned.
- Frontend code owns target resolution and review-team manifest construction;
  this directory owns validation, queue/retry state, task adaptation, and
  report enrichment.
- Keep default team/runtime contracts aligned with `deep_review_policy.rs` and
  reviewer agents in `src/crates/assembly/core/src/agentic/agents`.
- Reviewer subagents stay read-only; `ReviewFixer` is not part of the review
  pass.
- `ReviewFixer` may only act as a session primary in the user-approved
  remediation phase of a review child session. The agent registry resolves it
  through the builtin primary-agent path without itself checking an approval
  flag, so product surfaces must obtain explicit user approval before starting
  remediation (mirroring `DeepReviewExecutionPolicy::classify_subagent`, which
  rejects `ReviewFixer` during review execution). Do not route `ReviewFixer`
  through review-phase manifest injection in the coordinator; its scope is
  carried by the remediation prompt.
- When queue or report fields change, update the matching frontend DTOs and
  DeepReview UI state.

## Verification

Use the nearest Web UI check for frontend-only behavior. For shared runtime
behavior, run:

```bash
cargo test -p openbitfun-core --no-default-features --features agent-runtime --lib deep_review -- --nocapture
```

Also run the relevant Rust or desktop check when the change touches backend
state, Tauri APIs, or desktop integration.
