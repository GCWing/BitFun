# Model Request Cache Reuse In BitFun

This note summarizes the mechanisms BitFun uses to improve model-side prompt
cache reuse and to avoid unnecessary cache invalidation across long-running
agent sessions.

The implementation is mostly in the shared Rust runtime under
`src/crates/assembly/core/src/agentic/`.

## The Core Idea

BitFun tries to keep the model-visible request prefix as stable as possible.
It does that in four complementary ways:

1. Cache stable prompt fragments at the session level.
2. Reuse or clone existing conversation prefixes when creating derived
   sessions.
3. Keep compatible coding modes on the same prompt-cache identity.
4. Move frequently changing surfaces, such as skill and subagent listings, out
   of the cached base prompt and update them incrementally.

There is also explicit observability for provider-reported cache reads versus
cache writes so BitFun can measure whether these strategies are actually
working.

## 1. System Prompt And User Context Are Cached Separately

BitFun does not treat the whole request prefix as one opaque string.
Instead, it separates two relatively stable layers:

- the system prompt
- the user-context reminder

The cache model lives in
`src/crates/execution/agent-runtime/src/prompt_cache.rs`.

Core still exposes `src/crates/assembly/core/src/agentic/session/prompt_cache.rs`, but
that file is now a compatibility facade that re-exports the owner types from
`bitfun-agent-runtime`.

Key details:

- `SystemPromptCacheIdentity` keys the cached system prompt.
- `UserContextCacheIdentity` keys the cached user-context reminder.
- `SessionPromptCache` stores both entries independently.
- `PromptCachePolicy` supports both in-memory TTL and persistence TTL.
- the default persistence TTL is one day
  (`DEFAULT_PROMPT_CACHE_PERSISTENCE_TTL`)

Session-level loading, saving, invalidation, and cloning live in
`src/crates/assembly/core/src/agentic/session/session_manager.rs`.

The test coverage in that file also explicitly exercises:

- persistence across session restore
- persisted invalidation cleanup
- prompt-cache cloning between sessions

Persistence details:

- prompt cache is persisted as `prompt_cache.json`
- the file is stored under the session directory in `.bitfun/sessions/...`
- restore-time TTL cleanup happens before the cache is accepted back into
  memory

Request assembly uses the cache in
`src/crates/assembly/core/src/agentic/execution/execution_engine.rs`:

- `resolve_cached_system_prompt(...)` reuses a cached system prompt when the
  scope key still matches
- `build_cached_prepended_prompt_reminders(...)` reuses the cached
  user-context reminder when the user-context scope key still matches

One important design choice is documented directly in
`prompt_builder_impl.rs`: workspace context is intentionally injected outside
the system prompt cache. That lets BitFun keep the global instruction template
stable while still handling workspace-dependent context separately.

## 2. Derived Sessions Reuse Existing Prefixes Instead Of Starting Cold

### Session branching

Persisted session branching is implemented in
`src/crates/assembly/core/src/agentic/persistence/session_branch.rs`.

When BitFun branches a session from an existing turn, it copies more than turn
text:

- the branched turns up to the selected turn
- persisted turn context snapshots
- persisted skill/subagent listing snapshots
- persisted `skill-agent-baseline-override.json` when the source session has one
- the source session's prompt cache
- compression state and lineage metadata

This means a branch can continue from a pre-built prefix instead of paying to
reconstruct everything from scratch.

### `/btw` side questions

`/btw` is implemented as a hidden child session, not as an ad hoc detached
request.

Relevant code:

- desktop adapter:
  `src/apps/desktop/src/api/btw_api.rs`
- child-session creation:
  `ConversationCoordinator::ensure_hidden_btw_session(...)` in
  `src/crates/assembly/core/src/agentic/coordination/coordinator.rs`
- side-question prompt wrapper:
  `src/crates/assembly/core/src/agentic/side_question.rs`

The parent context snapshot is not limited to a hot in-memory session. When
needed, `load_session_context_messages(...)` restores the parent session from
persistence first and then captures the context snapshot.

The child session is initialized from a captured parent context snapshot:

- same parent agent type
- same workspace and model config
- inherited message context
- cloned session-level prompt cache
- seeded skill/agent listing baselines derived from the parent session

That gives the side thread an already-built conversational prefix.

In other words, `/btw` now reuses both:

- the parent message context snapshot
- the parent session's prompt cache via `SessionManager::clone_prompt_cache(...)`
- the parent session's full skill/agent listing baseline via
  `SessionManager::seed_forked_skill_agent_listing_baselines(...)`

### `fork_context` subagents

Forked subagents reuse even more.

Relevant code:

- snapshot model:
  `src/crates/assembly/core/src/agentic/fork_agent/mod.rs`
- request validation and tool contract:
  `src/crates/assembly/core/src/agentic/tools/implementations/task_tool.rs`
- execution and prompt-cache cloning:
  `src/crates/assembly/core/src/agentic/coordination/coordinator.rs`

When `Task` is called with `fork_context=true`, BitFun:

- captures the parent session's model-visible message context
- creates an isolated child session with the parent session's agent type,
  workspace, remote metadata, and model selection
- clones the parent session's prompt cache into the child session via
  `SessionManager::clone_prompt_cache(...)`
- seeds a fork-aware skill/agent listing baseline split via
  `SessionManager::seed_forked_skill_agent_listing_baselines(...)`

That seed step intentionally keeps two different baselines:

- the parent's turn-0 skill/agent snapshot is preserved as a prompt/listing
  baseline override so the child can reuse the same full-listing prefix on its
  first request
- the parent's latest snapshot at fork time becomes the child's own turn-0
  snapshot so later child turns diff against the fork-time surface, not forever
  against the parent's original turn-0 baseline

The `Task` tool also forbids fields that would make the fork drift away from
the inherited prefix, including `subagent_type`, `workspace_path`, `model_id`,
and DeepReview retry fields.

That restriction is not just validation polish; it protects cache reuse by
keeping the forked child aligned with the parent session shape.

## 3. The Shared Coding Modes Intentionally Reuse The Same Cache Identity

BitFun's four shared coding modes are:

- `agentic`
- `Plan`
- `debug`
- `Multitask`

They are intentionally configured to share the same stable prompt base.

Relevant code:

- shared constants and tests:
  `src/crates/execution/agent-runtime/src/agents.rs`
- mode definitions:
  `src/crates/assembly/core/src/agentic/agents/definitions/modes/{agentic,plan,debug,multitask}.rs`

Why they reuse cache:

- all four modes use the same prompt template:
  `SHARED_CODING_MODE_PROMPT_TEMPLATE = "agentic_mode"`
- all four modes use the same user-context policy:
  `shared_coding_mode_user_context_policy()`
- `Agent::system_prompt_cache_identity(...)` is derived from
  `prompt_template_name(...)`
- `Agent::user_context_cache_identity(...)` is derived from the user-context
  policy scope key

The test `shared_template_modes_share_system_prompt_cache_identity()` asserts
that these shared modes intentionally produce the same system-prompt and
user-context cache identities.

Mode-specific behavior is added through `system_reminder` text, not by swapping
the cached base template:

- `Plan`, `Debug`, and `Multitask` provide first-entry and ongoing reminder
  templates
- reminders are injected immediately before the current user message in
  `ExecutionEngine::build_ai_messages_for_send(...)`

This is the key reason mode switches between these four coding modes do not
force a base prompt cache reset.

The frontend also knows about this compatibility:

- `AgentInfo.prompt_cache_scope_key` is produced in
  `src/crates/assembly/core/src/agentic/agents/registry/types.rs`
- `ChatInput.tsx` only shows a prompt-cache warning when the next mode's scope
  key differs from the last submitted mode's scope key

In other words, BitFun aligns backend prompt identities and frontend mode-switch
UX around the same cache-compatibility contract.

## 4. Skill And Subagent Listings Are Hot-Updated Without Rebuilding The Base Prompt

A major source of cache churn in agent systems is dynamic capability listing.
BitFun explicitly avoids baking that surface into one permanently rebuilt prompt.

Relevant code:

- snapshot and diff model:
  `src/crates/assembly/core/src/agentic/skill_agent_snapshot.rs`
- sparse snapshot store:
  `src/crates/assembly/core/src/agentic/session/turn_skill_agent_snapshot_store.rs`
- fork baseline override persistence:
  `snapshots/skill-agent-baseline-override.json` via
  `src/crates/assembly/core/src/agentic/persistence/manager.rs`
- turn-time diff injection:
  `ConversationCoordinator::wrap_user_input(...)`
- baseline reminder reuse:
  `ExecutionEngine::build_cached_prepended_prompt_reminders(...)`
- reminder ordering owner:
  `src/crates/execution/agent-runtime/src/prompt.rs`

The strategy is:

1. On the first turn, BitFun saves a full skill/subagent snapshot.
2. On later turns, it resolves the current snapshot again.
3. It diffs the latest prior snapshot against the new one.
4. Only the diff is injected as a reminder when something changed.

This keeps the base cached prefix stable while still letting the model see
fresh capability changes.

Forked child sessions add one extra wrinkle: prompt/listing reuse and later diff
correctness do not always want the same baseline. In those cases BitFun can keep
the child's turn-0 snapshot as the diff baseline while reading the separate
baseline-override snapshot first when rebuilding the full skill/agent listing
reminder.

What gets updated dynamically:

- skills
- visible subagents / agents
- collapsed tool listing sections

Why this is effectively "hot update":

- local custom subagents are reloaded during request wrapping via
  `load_custom_subagents(...)`
- skills are rescanned for the current workspace via
  `get_resolved_skills_for_workspace(...)`
- the desktop app also exposes an explicit `reload_subagents` command in
  `src/apps/desktop/src/api/subagent_api.rs`

So capability changes can reach the next request without restarting the session
and without rebuilding the main prompt template.

### Current gap: per-agent tool changes still churn request-prefix cache

This is not fully solved yet.

BitFun does allow tool customization at the agent/profile level:

- built-in modes resolve their effective tools from default tools plus user
  `added_tools` / `removed_tools`
- custom subagents can define their own tool lists directly

Relevant code:

- mode/profile tool overrides:
  `src/crates/assembly/core/src/service/config/types.rs`
  and `src/crates/assembly/core/src/service/config/mode_config_canonicalizer.rs`
- runtime resolution of effective agent tool policy:
  `src/crates/assembly/core/src/agentic/agents/registry/query.rs`

However, tool manifests are still recomputed per turn/request, not diff-patched
like skill and subagent listings.

More precisely:

- `ExecutionEngine` resolves `get_agent_tool_policy(...)` at turn start
- it then resolves `resolve_tool_manifest(...)`
- the resulting `tool_definitions` are attached to the model request for that
  turn
- all model rounds inside the same turn reuse that resolved tool surface

Relevant code:

- request-time tool manifest resolution:
  `src/crates/assembly/core/src/agentic/execution/execution_engine.rs`
- tool definitions included in token estimation:
  `src/crates/assembly/core/src/util/token_counter.rs`
- provider request-body tool attachment:
  `src/crates/adapters/ai-adapters/src/providers/openai/responses.rs`
  and `src/crates/adapters/ai-adapters/src/providers/openai/codex_chatgpt.rs`

This is why tool changes cannot currently be handled the same way as
skill/subagent listing updates.

Skill and subagent listings are prompt-visible descriptive surfaces, so BitFun
can:

- keep a baseline
- send only a diff reminder when they change

But tool availability is different. For many tool-calling providers, the model
is only allowed to call tools that are present in the current request's tool
definitions/schema payload.

That creates an asymmetry:

- removing a tool or changing its description can be partially explained with a
  reminder
- adding a new callable tool cannot be expressed by reminder alone
- the new tool must exist in the turn's actual `tool_definitions`

So as long as the callable tool set is request-attached in this provider style,
new tool additions will necessarily mutate the request prefix surface and
typically break prompt-cache reuse for that turn.

One plausible future direction would be to expose only a single stable
"tool-dispatch" entry in the provider request and let that entry accept:

- the real tool name
- the real tool arguments

In that design, the provider-visible tool schema could stay nearly constant
even when the product's actual tool catalog changes underneath it.

However, BitFun does not currently plan to implement this.

Reasons:

- it would be a fairly large architectural change across tool schema exposure,
  execution, validation, safety checks, and provider adapters
- the cache misses caused by tool-set changes are relatively low-frequency in
  practice
- those misses are usually user-driven, because changing an agent's tool set is
  an explicit configuration action rather than a high-churn runtime event

So while the direction is technically viable, it is not currently attractive
enough relative to its implementation cost.

## 5. Compression Rebuilds A Smaller Stable Prefix

Compression is not a pure cache-preserving operation, but it is still part of
BitFun's cache-hit strategy.

Relevant code:

- compression flow:
  `src/crates/assembly/core/src/agentic/execution/execution_engine.rs`
- compressor:
  `src/crates/assembly/core/src/agentic/session/compression/`

There are two different cache-reuse stories around compression:

1. the compression request itself reuses the current session prefix
2. the completed compression result intentionally invalidates and rebuilds the
   session prompt cache

### Compression request path: reuses the existing prefix

BitFun does not send compression as a detached, prompt-from-scratch request.

Instead:

- `build_compression_request_messages(...)` calls
  `build_ai_messages_for_send(...)`
- that means the compression request is assembled through the same message-send
  path used for ordinary model rounds
- prepended reminders are preserved
- the current conversation messages are preserved

For automatic compression, the `runtime_messages` passed into
`build_compression_request_messages(...)` come from the active `messages`
buffer, which already includes the current system prompt at the front of the
request.

For manual compaction, the code explicitly rebuilds `runtime_messages` as:

- `system_prompt_message`
- followed by the current session messages

Only after that shared prefix is rebuilt does BitFun append the final
compression-specific instruction from
`context_compressor.build_compact_prompt(...)`.

So the compression model call itself is prefix-reusing by design: it keeps the
same stable prefix and adds one extra user message that asks the model to
compact the conversation.

There is even an explicit runtime comment documenting why an older pre-pass was
removed: the removed "microcompact" rewrite mutated already-sent prefixes and
"kill[ed] provider KV-cache hits on every round". The current compression path
keeps that concern front and center.

### Compression completion path: invalidate, then rebuild against a shorter prefix

When compression is applied, BitFun does three important things:

1. Replaces the live context messages with the compressed result.
2. Rebuilds the skill/subagent listing baseline to the latest snapshot.
3. Invalidates the session prompt cache because the mutable prefix changed.

That invalidation happens on purpose. After compression, the old cached prompt
fragments no longer match the new conversation prefix.

However, the next request can rebuild cache entries against a much shorter and
more stable post-compression prefix. In practice, compression is a "reset once,
reuse many times after" mechanism, not a permanent loss.

## 6. Cache Observability Is First-Class

BitFun also normalizes provider-specific cache telemetry so cache reuse can be
measured instead of guessed.

Relevant code:

- unified usage types:
  `src/crates/adapters/ai-adapters/src/stream/types/unified.rs`
- Anthropic mapping:
  `src/crates/adapters/ai-adapters/src/stream/types/anthropic.rs`
- OpenAI / DeepSeek mapping:
  `src/crates/adapters/ai-adapters/src/stream/types/openai.rs`
- runtime event emission:
  `src/crates/assembly/core/src/agentic/execution/round_executor.rs`

Two fields matter:

- `cached_content_token_count`: cache reads / hits
- `cache_creation_token_count`: cache writes / creation

BitFun keeps them separate on purpose. That avoids overstating hit rate and
makes it possible to answer both of these questions correctly:

- "How many input tokens were served from cache?"
- "How many input tokens were spent creating new cache entries?"

## Summary

BitFun improves request cache hit rate by combining prompt splitting, session
reuse, compatible mode identities, incremental capability-list updates, and
post-compression prefix rebuilding.

The most important implementation choices are:

- stable prompt pieces are cached separately and persisted per session
- derived sessions reuse parent prefixes instead of starting from zero
- shared coding modes keep the same cache identity and express differences as
  reminders
- dynamic capability surfaces are updated incrementally outside the base cached
  prompt
- provider cache-read and cache-write telemetry is tracked separately

## Implementation Map

- Prompt cache model:
  `src/crates/execution/agent-runtime/src/prompt_cache.rs`
- Prompt cache lifecycle:
  `src/crates/assembly/core/src/agentic/session/session_manager.rs`
- Request assembly and cache hits:
  `src/crates/assembly/core/src/agentic/execution/execution_engine.rs`
- Fork snapshot model:
  `src/crates/assembly/core/src/agentic/fork_agent/mod.rs`
- Session branching:
  `src/crates/assembly/core/src/agentic/persistence/session_branch.rs`
- Side-question child sessions:
  `src/crates/assembly/core/src/agentic/coordination/coordinator.rs`
  and `src/apps/desktop/src/api/btw_api.rs`
- Shared coding-mode identities:
  `src/crates/execution/agent-runtime/src/agents.rs`
- Dynamic skill/agent listing snapshots:
  `src/crates/assembly/core/src/agentic/skill_agent_snapshot.rs`
- Provider cache telemetry:
  `src/crates/adapters/ai-adapters/src/stream/types/`
