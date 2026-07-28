# PR-2 Progress — Cfg Gate + Compile Verification

## Status: ✅ Complete

## Changes Made

### 1. session.rs (execution/agent-runtime/src/session.rs)
- **`is_daemon`** in `SessionConfig`: Added `#[cfg(feature = "taiji")]` to field + default value
- **`max_context_tokens`** default in `SessionConfig::default()`: Added cfg gated values (1_048_576 with taiji, 128_128 without)
- **`parent_session_id`** in `SessionSummary`: Added `#[cfg(feature = "taiji")]` gate
- **`is_daemon`** in `SessionSummary`: Added `#[cfg(feature = "taiji")]` gate
- Updated test assertions to conditionally match the correct default value

### 2. config/types.rs (assembly/core/src/service/config/types.rs)
- Split `DEFAULT_MODEL_CONTEXT_WINDOW_TOKENS` into two versions:
  - `#[cfg(feature = "taiji")]` → `1_048_576`
  - `#[cfg(not(feature = "taiji"))]` → `128_128`

### 3. rbac_poke_integration.rs (assembly/core/tests/rbac_poke_integration.rs)
- Added `#![cfg(feature = "taiji")]` at top

### 4. New module files — Added `#![cfg(feature = "taiji")]` to:
- `warden/mod.rs`
- `warden/poisson.rs`
- `warden/punishment_executor.rs`
- `poke.rs` (tool-contracts/src/poke.rs)
- `review_propagation.rs` (assembly/core/src/agentic/coordination/review_propagation.rs)
- `session_tree.rs` (core-types/src/session_tree.rs)

### 5. Module declarations gated:
- `pub mod poke;` in tool-contracts/src/lib.rs
- `mod review_propagation;` + `pub use` in coordination/mod.rs
- `pub mod session_tree;` in core-types/src/lib.rs
- `pub mod tree;` in services-core/src/session/mod.rs
- Added `taiji = []` feature to services-core's Cargo.toml

### 6. Feature propagation:
- `bitfun-core`: Added `bitfun-services-core/taiji` to taiji feature list
- `bitfun-agent-runtime`: Added `bitfun-services-core/taiji` to taiji feature list

### 7. Pre-existing fixes (struct field gaps in PR branch):
- Added `depth: None` to two `SubagentParentInfo` constructors in coordinator.rs
- Added `execution_target: None, project_workspace_path: None` to `make_meta` in review_propagation.rs

## Compilation Verification

| Crate | Result |
|---|---|
| `cargo check -p bitfun-agent-tools --features taiji` | ✅ Passed |
| `cargo check -p bitfun-core --features taiji` | ✅ Passed |
| `cargo check -p bitfun-agent-runtime --features taiji` | ✅ Passed |

## Test Results

| Test Suite | Result |
|---|---|
| `cargo test -p bitfun-agent-tools --features taiji` | ✅ 91 passed, 1 pre-existing failure (delegation_policy_tool_restrictions_block_recursive_subagents — not caused by our changes) |
| `cargo test -p bitfun-core --features taiji --test rbac_poke_integration` | ✅ All 7 passed |
| `cargo test -p bitfun-agent-runtime --lib --features taiji -- session` | ✅ All 5 session tests passed |

## Notes
- One pre-existing test failure in `bitfun-agent-tools`: `framework::tests::delegation_policy_tool_restrictions_block_recursive_subagents` — this is a bug in the PR branch where `spawn_child()` creates depth=1 which is < MAX_FISSION_DEPTH(10), so `allow_subagent_spawn` is `true`, contradicting the test assertion. Not caused by cfg gate changes.
