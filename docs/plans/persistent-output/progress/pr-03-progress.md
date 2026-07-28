# PR-3 Progress — Coordination Tools Layer: Cfg Gate + Compile Verification

## Status: ✅ Complete

## Changes Made

### 1. bitfun-events (`src/crates/contracts/events/`)

#### Cargo.toml
- Added `taiji = []` feature definition

#### agentic.rs
- **`SubagentCompletionStatus`** enum: Added `#[cfg(feature = "taiji")]` gate
- **`SubagentTurnCompleted`** variant in `AgenticEvent`: Added `#[cfg(feature = "taiji")]` gate
- **`ReviewPropagationNeeded`** variant in `AgenticEvent`: Added `#[cfg(feature = "taiji")]` gate
- **`session_id()`** match arms: Split gated variants into separate arms with `#[cfg(feature = "taiji")]`
- **`default_priority()`** match arm: Extracted `SubagentTurnCompleted` into separate gated arm
- **Test** `subagent_completion_status_serializes_snake_case`: Added `#[cfg(feature = "taiji")]` gate

#### frontend_projection.rs
- **`ReviewPropagationNeeded`** match arm: Added `#[cfg(feature = "taiji")]` gate
- **`SubagentTurnCompleted`** match arm: Added `#[cfg(feature = "taiji")]` gate

### 2. bitfun-core (`src/crates/assembly/core/`)

#### Cargo.toml
- Added `"bitfun-events/taiji"` to the `taiji` feature propagation list

#### coordination/background_outcomes.rs
- **`list_records()`** method: Added `#[cfg(feature = "taiji")]` gate

#### coordination/coordination_store.rs
- **`list_tasks()`** method: Added `#[cfg(feature = "taiji")]` gate

#### coordination/coordinator.rs
- **`SessionTreeManager`** import: Added `#[cfg(feature = "taiji")]` gate
- **`SubagentCompletionStatus`** import: Added `#[cfg(feature = "taiji")]` gate
- **`session_tree`** field in `ConversationCoordinator`: Added `#[cfg(feature = "taiji")]` gate

#### coordination/review_propagation.rs
- Already had `#![cfg(feature = "taiji")]` from PR-2

#### coordination/mod.rs
- Already had `#[cfg(feature = "taiji")]` for `review_propagation` from PR-2

#### tools/implementations/session_control_tool.rs
- **`SessionTreeManager`** import: Added `#[cfg(feature = "taiji")]` gate

## Compilation Verification

| Crate | Command | Result |
|---|---|---|
| `bitfun-events` | `cargo check -p bitfun-events --features taiji` | ✅ Passed |
| `bitfun-events` | `cargo check -p bitfun-events` (default feat.) | ✅ Passed |
| `bitfun-core` | `cargo check -p bitfun-core --features taiji` | ✅ Passed |
| `bitfun-core` | `cargo check -p bitfun-core` (default feat.) | ✅ Passed |

Note: `cargo check -p bitfun-core --no-default-features` fails with pre-existing errors in `external_subagents.rs` (gated behind `product-full` feature, not related to taiji changes).

## Test Results

| Suite | Result |
|---|---|
| `cargo test -p bitfun-events --features taiji` | ✅ 19 passed, 0 failed |
| `cargo test -p bitfun-core --features taiji` | ✅ 1576 passed, **4 pre-existing failures** |

### Pre-existing Failures (not caused by PR-3)

1. **`agentic::coordination::coordinator::tests::session_mode_port_rejects_unknown_mode_for_active_session`** — Lock contention in `AgentRegistry::read_agents()` using `try_read()` in async tokio test context. Caused by PR-2's `std::sync::RwLock` → `tokio::sync::RwLock` migration.

2. **`agentic::tools::restrictions::tests::update_restrictions_patch_overrides_role_template`** — "Should retain Executor's WriteFile" assertion failure in RBAC restrictions code from PR-2.

3. **`agentic::warden::tests::poke_message_example`** — Serde field name mismatch: JSON uses `pokeId` (camelCase) but struct expects `poke_id` with `#[serde(rename_all = "snake_case")]`. Pre-existing in PR-2 warden tests.

4. **`agentic::warden::tests::poke_response_example`** — Serde variant mismatch: JSON uses `Acknowledged` but `PokeStatus` expects `acknowledged` via `rename_all = "snake_case"`. Pre-existing in PR-2 warden tests.

## Files Modified

### bitfun-events crate
- `src/crates/contracts/events/Cargo.toml` — added taiji feature
- `src/crates/contracts/events/src/agentic.rs` — gated SubagentCompletionStatus, SubagentTurnCompleted, ReviewPropagationNeeded
- `src/crates/contracts/events/src/frontend_projection.rs` — gated SubagentTurnCompleted and ReviewPropagationNeeded arms

### bitfun-core crate
- `src/crates/assembly/core/Cargo.toml` — added bitfun-events/taiji propagation
- `src/crates/assembly/core/src/agentic/coordination/background_outcomes.rs` — gated list_records
- `src/crates/assembly/core/src/agentic/coordination/coordination_store.rs` — gated list_tasks
- `src/crates/assembly/core/src/agentic/coordination/coordinator.rs` — gated SessionTreeManager/SubagentCompletionStatus imports and session_tree field
- `src/crates/assembly/core/src/agentic/tools/implementations/session_control_tool.rs` — gated SessionTreeManager import

## Notes
- All taiji-specific additions from PR-2 are now protected behind `#[cfg(feature = "taiji")]`.
- Some deeply integrated changes (e.g., SQL batch optimizations in coordination_store.rs, RwLock migrations in registry files) are general infrastructure improvements, not taiji-specific — they do not need gating.
- The `taiji` feature remains in `bitfun-core`'s default features, so normal builds are unaffected.
