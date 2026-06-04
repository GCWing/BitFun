# agent-runtime Agent Guide

Scope: this guide applies to `src/crates/agent-runtime`.

`bitfun-agent-runtime` owns portable agent runtime decisions that can be built
and tested without `bitfun-core`.

## Guardrails

- Do not depend on `bitfun-core`, app crates, Tauri, ACP protocol, web UI,
  concrete service crates, or product-domain implementations.
- Keep concrete scheduler/session lifecycle execution, session metadata IO, and
  product `Tool` adapters in `bitfun-core` until a reviewed owner migration
  proves behavior equivalence.
- Prefer pure facts and decisions first: queue policy, background delivery,
  dialog-turn queue state, active-turn facts, cancellation routing and
  suppression state, background running-turn injection construction, steering action
  planning, agent-session reply planning, thread-goal accounting/mutation/continuation decisions,
  scheduled-job lifecycle state transitions, runtime event facts,
  registry visibility/availability, custom subagent schema/default decisions,
  custom subagent markdown front-matter IO, custom subagent discovery/loading,
  post-call hook routing/executor orchestration,
  tool confirmation planning/failure mapping, round-boundary yield/injection state, turn-outcome
  queue decisions, registry source/profile facts, prompt-loop user-context
  policy, prompt listing reminder ordering, prompt-cache policy/identity/store,
  finish-reason labels, session-state event labels, and turn-outcome event facts.
- Keep concrete prompt assembly, workspace context IO, prompt-cache persistence
  wiring, dynamic environment collection, concrete non-custom agent definition
  loading, concrete hook side effects, permission UI/tool side effects, and
  channel/state mutation outside this crate until a reviewed migration proves
  behavior equivalence.
- Add focused tests before moving any runtime decision into this crate.

## Verification

```bash
cargo test -p bitfun-agent-runtime
node scripts/check-core-boundaries.mjs
cargo check -p bitfun-core --features product-full
```
