//! Session persistence service

pub use bitfun_services_core::session::types;
pub use bitfun_services_core::session::*;

pub fn effective_tool_identity(item: &ToolItemData) -> (&str, &serde_json::Value) {
    bitfun_agent_tools::effective_tool_invocation(&item.tool_name, &item.tool_call.input)
}

pub trait ToolItemIdentityExt {
    fn effective_name(&self) -> &str;
    fn effective_input(&self) -> &serde_json::Value;
}

impl ToolItemIdentityExt for ToolItemData {
    fn effective_name(&self) -> &str {
        effective_tool_identity(self).0
    }

    fn effective_input(&self) -> &serde_json::Value {
        effective_tool_identity(self).1
    }
}
