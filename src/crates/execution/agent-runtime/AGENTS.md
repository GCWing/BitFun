# agent-runtime Agent Guide

Scope: this guide applies to `src/crates/execution/agent-runtime`.

`bitfun-agent-runtime` owns portable agent runtime decisions,
session/config/context facts, lifecycle helper state, and the narrow
port-backed `sdk` / `AgentRuntime` facade that can be built and tested without
`bitfun-core`.

## Guardrails

- Do not depend on `bitfun-core`, app crates, Tauri, ACP protocol, web UI,
  concrete service crates, or product-domain implementations.
- The `sdk` module may re-export only stable runtime request/response types,
  runtime-port contracts, and the service/tool/harness registry types needed
  for dependency injection.
- `AgentRuntime` may depend on stable ports plus injected `RuntimeServices`,
  tool registry, harness registry, and hook registry. Product assembly owns
  concrete registration; this crate must not create concrete managers, app
  state, filesystem, terminal, MCP, remote, or AI clients.
- Keep concrete scheduler/session lifecycle execution, session metadata IO,
  event emitter wiring, permission UI presentation, and product `Tool` adapter
  execution in `bitfun-core` until a reviewed owner migration proves behavior
  equivalence. Provider-neutral confirmation gate/wait-channel and user-question state
  may live here.
- Prefer pure facts and decisions first: queue policy, background delivery,
  dialog-turn queue state, active-turn facts, cancellation routing and
  suppression state, background running-turn injection construction, steering action
  planning, agent-session reply planning, thread-goal accounting/mutation/continuation decisions,
  scheduled-job lifecycle state transitions, runtime event facts,
  registry visibility/availability, custom subagent schema/default decisions,
  builtin agent definition catalog, skill catalog/root/mode/selection facts,
  thread-goal metadata / event payload /
  token usage / scheduler delivery plans, thread-goal tool wire contracts,
  session config/defaults/summary and persisted session-state sidecar shape,
  user-question validation/result/channel contracts, SessionControl input/cancel-route/result contracts, DeepReview
  policy/manifest/budget/queue/report/cache/shared-context/task-execution
  shaping decisions, DeepResearch citation renumbering,
  custom subagent markdown front-matter IO, custom subagent discovery/loading,
  post-call hook routing/executor orchestration,
  tool confirmation gate/planning/failure/wait-result/channel mapping, light checkpoint
  summary policy, dialog-turn cancellation token state,
  round-boundary yield/injection state, turn-outcome
  queue decisions, registry source/profile facts, prompt-loop user-context
  policy, prompt listing reminder ordering, prompt-cache policy/identity/store,
  prompt runtime/workspace/user-context rendering, turn skill/agent snapshot
  state, file-read session state, session evidence ledger projection,
  finish-reason labels, session-state event labels, and turn-outcome event
  facts.
- Keep concrete prompt fact collection, workspace context IO, prompt-cache
  persistence wiring, dynamic environment collection, concrete hook side
  effects, DeepReview task launch/provider wait/report persistence,
  DeepResearch storage IO/post-turn hook and concrete product tool execution
  outside this crate until a reviewed migration proves behavior equivalence.
- Add focused tests before moving any runtime decision into this crate.

## Verification

```bash
cargo test -p bitfun-agent-runtime
node scripts/check-core-boundaries.mjs
cargo check -p bitfun-core --features product-full
```
