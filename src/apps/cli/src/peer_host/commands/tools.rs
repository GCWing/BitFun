//! Tools HostInvoke handlers for CLI Peer Host.

use serde_json::Value;

use openbitfun_core::agentic::tools::product_runtime::build_all_tools_info;

/// Read-only tool catalog for the Agents / Assistant Defaults UI.
///
/// CLI Host assembles the same Core tool registry as Desktop; this returns the
/// identical DTO shape so a controller cannot tell "CLI Host doesn't support
/// catalog query" from "the runtime really has no tools". Without this, the
/// controller's `get_all_tools_info` call would fall into the unsupported
/// dispatch branch and the UI would silently render an empty tool list.
pub(crate) async fn get_all_tools_info() -> Result<Value, String> {
    let tools = build_all_tools_info().await;
    serde_json::to_value(tools).map_err(|error| format!("Failed to serialize tool info: {error}"))
}
