//! JSON-RPC 2.0 implementation
//!
//! Helper functions and types for the JSON-RPC protocol.

use super::types::*;

pub use bitfun_services_integrations::mcp::protocol::{
    create_initialize_request, create_ping_request, create_prompts_get_request,
    create_prompts_list_request, create_resources_list_request, create_resources_read_request,
    create_tools_call_request, create_tools_list_request,
};

/// Parses the response result.
pub fn parse_response_result<T>(response: &MCPResponse) -> crate::util::errors::BitFunResult<T>
where
    T: serde::de::DeserializeOwned,
{
    if let Some(error) = &response.error {
        return Err(crate::util::errors::BitFunError::MCPError(format!(
            "MCP Error {}: {}",
            error.code, error.message
        )));
    }

    let result = response.result.as_ref().ok_or_else(|| {
        crate::util::errors::BitFunError::MCPError("Missing result in MCP response".to_string())
    })?;

    serde_json::from_value(result.clone()).map_err(|e| {
        crate::util::errors::BitFunError::Deserialization(format!(
            "Failed to parse MCP response: {}",
            e
        ))
    })
}
