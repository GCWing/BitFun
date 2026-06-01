//! Product tool snapshot decoration boundary.

use crate::agentic::tools::framework::Tool;
use crate::agentic::tools::registry::ToolRef;
use bitfun_agent_tools::SnapshotToolWrapper;

#[derive(Debug, Clone)]
pub(super) struct ProductSnapshotToolWrapper;

impl SnapshotToolWrapper<dyn Tool> for ProductSnapshotToolWrapper {
    fn wrap_for_snapshot_tracking(&self, tool: ToolRef) -> ToolRef {
        crate::service::snapshot::wrap_tool_for_snapshot_tracking(tool)
    }
}
