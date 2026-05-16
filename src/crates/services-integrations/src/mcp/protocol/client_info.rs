//! MCP client identity and capability helper contracts.

use rmcp::model::{
    ClientCapabilities, ClientInfo, ElicitationCapability, Implementation, ProtocolVersion,
};

pub fn create_mcp_client_info(
    client_name: impl Into<String>,
    client_version: impl Into<String>,
) -> ClientInfo {
    ClientInfo {
        protocol_version: ProtocolVersion::LATEST,
        capabilities: ClientCapabilities::builder()
            .enable_roots()
            .enable_sampling()
            .enable_elicitation_with(ElicitationCapability {
                schema_validation: Some(true),
            })
            .build(),
        client_info: Implementation {
            name: client_name.into(),
            title: None,
            version: client_version.into(),
            icons: None,
            website_url: None,
        },
    }
}
