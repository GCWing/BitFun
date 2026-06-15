//! Deep Review product assembly adapters.
//!
//! Provider-neutral policy, manifest, queue, budget, and shared-context state
//! are owned by `bitfun-agent-runtime::deep_review`. This module keeps old
//! core paths compatible while concrete report shaping and product tool side
//! effects remain in core until separately migrated.

pub use bitfun_agent_runtime::deep_review::{
    budget, concurrency_policy, constants, diagnostics, execution_policy, incremental_cache,
    manifest, queue, shared_context, team_definition, tool_context,
};

pub mod report;
pub mod task_adapter;
pub mod tool_measurement;
