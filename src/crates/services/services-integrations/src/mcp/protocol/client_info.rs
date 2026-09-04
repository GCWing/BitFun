//! MCP client identity and capability helper contracts.

use rmcp::model::{ClientCapabilities, ClientInfo, Implementation, ProtocolVersion};

pub fn create_mcp_client_info(
    client_name: impl Into<String>,
    client_version: impl Into<String>,
) -> ClientInfo {
    // SEP-2577 deprecates `roots` and `sampling`, but OpenBitFun still advertises
    // both so servers that gate features on them keep working; the handshake
    // shape is pinned by `mcp_remote_client_info_declares_supported_client_capabilities`.
    // Dropping them is a protocol-visible decision, not a lint cleanup — revisit
    // when rmcp actually removes the builders.
    #[allow(deprecated)]
    let capabilities = ClientCapabilities::builder()
        .enable_roots()
        .enable_sampling()
        .enable_elicitation()
        .build();
    ClientInfo::new(
        capabilities,
        Implementation::new(client_name, client_version),
    )
    .with_protocol_version(ProtocolVersion::LATEST)
}
