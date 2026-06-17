//! MCP client identity and capability helper contracts.

use rmcp::model::{ClientCapabilities, ClientInfo, Implementation, ProtocolVersion};

pub fn create_mcp_client_info(
    client_name: impl Into<String>,
    client_version: impl Into<String>,
) -> ClientInfo {
    ClientInfo::new(
        ClientCapabilities::builder()
            .enable_roots()
            .enable_sampling()
            .enable_elicitation()
            .enable_elicitation_schema_validation()
            .build(),
        Implementation::new(client_name, client_version),
    )
    .with_protocol_version(ProtocolVersion::LATEST)
}
