//! Connection initialization, capability, and health wire schemas.

use agent_client_protocol::{JsonRpcRequest, JsonRpcResponse};
use serde::{Deserialize, Serialize};

use crate::{MIN_PROTOCOL_VERSION, PROTOCOL_VERSION};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientInfo {
    pub name: String,
    pub version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerInfo {
    pub name: String,
    pub version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransportLimits {
    pub max_frame_bytes: u64,
    pub event_buffer_capacity: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "availability", rename_all = "camelCase")]
pub enum CapabilityAvailability {
    Available,
    Unavailable { reason: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CapabilityDescriptor {
    pub id: String,
    pub availability: CapabilityAvailability,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub methods: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcRequest)]
#[request(method = "app/initialize", response = InitializeResponse)]
#[serde(rename_all = "camelCase")]
pub struct InitializeRequest {
    pub protocol_version: u32,
    pub client: ClientInfo,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcResponse)]
#[serde(rename_all = "camelCase")]
pub struct InitializeResponse {
    pub protocol_version: u32,
    pub minimum_protocol_version: u32,
    pub server: ServerInfo,
    pub capabilities: Vec<CapabilityDescriptor>,
    pub limits: TransportLimits,
}

impl InitializeResponse {
    pub fn new(
        server: ServerInfo,
        capabilities: Vec<CapabilityDescriptor>,
        limits: TransportLimits,
    ) -> Self {
        Self {
            protocol_version: PROTOCOL_VERSION,
            minimum_protocol_version: MIN_PROTOCOL_VERSION,
            server,
            capabilities,
            limits,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcRequest)]
#[request(method = "app/health", response = HealthResponse)]
pub struct HealthRequest {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonRpcResponse)]
#[serde(rename_all = "camelCase")]
pub struct HealthResponse {
    pub status: HealthStatus,
    pub protocol_version: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum HealthStatus {
    Ready,
}

#[cfg(test)]
mod tests {
    use super::super::method::INITIALIZE;
    use super::{ClientInfo, InitializeRequest};

    #[test]
    fn initialize_method_is_stable() {
        assert_eq!(INITIALIZE, "app/initialize");
        let request = InitializeRequest {
            protocol_version: 1,
            client: ClientInfo {
                name: "tui".to_string(),
                version: "test".to_string(),
            },
        };
        assert_eq!(request.protocol_version, 1);
    }
}
