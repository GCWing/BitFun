use crate::agentic::tools::{ToolPathPolicy, ToolRuntimeRestrictions};
use std::collections::BTreeSet;

pub fn build_auto_memory_runtime_restrictions(memory_dir: &str) -> ToolRuntimeRestrictions {
    ToolRuntimeRestrictions {
        allowed_tool_names: ["Read", "Glob", "Grep", "Write", "Edit", "Delete", "Bash"]
            .into_iter()
            .map(str::to_string)
            .collect::<BTreeSet<_>>(),
        denied_tool_names: BTreeSet::new(),
        path_policy: ToolPathPolicy {
            write_roots: vec![memory_dir.to_string()],
            edit_roots: vec![memory_dir.to_string()],
            delete_roots: vec![memory_dir.to_string()],
        },
        disable_snapshot_tracking: true,
    }
}
