//! Product-search App Server wire schema.

#[cfg(feature = "rpc")]
use agent_client_protocol::{JsonRpcRequest, JsonRpcResponse};
pub use openbitfun_product_domains::product_search::{
    SessionContentSearchRequest, SessionContentSearchResponse, SessionSearchDiagnostic,
    SessionSearchDiagnosticCode, SessionSearchHit, SessionSearchHitKind, SessionSearchMatchField,
    PRODUCT_SEARCH_CAPABILITY_ID,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "rpc", derive(JsonRpcRequest))]
#[cfg_attr(feature = "rpc", request(method = "search/sessionContent", response = SearchSessionContentResponse))]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(transparent)]
pub struct SearchSessionContentMessage(pub SessionContentSearchRequest);

impl std::fmt::Debug for SearchSessionContentMessage {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SearchSessionContentMessage")
            .field("workspace_path", &"<redacted>")
            .field("query_length", &self.0.query.chars().count())
            .field("remote", &self.0.remote_connection_id.is_some())
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "rpc", derive(JsonRpcResponse))]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(transparent)]
pub struct SearchSessionContentResponse(pub SessionContentSearchResponse);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schemas::method::{is_valid_method_name, SEARCH_SESSION_CONTENT};

    #[test]
    fn search_method_and_legacy_defaults_are_stable() {
        assert!(is_valid_method_name(SEARCH_SESSION_CONTENT));
        let message: SearchSessionContentMessage = serde_json::from_value(serde_json::json!({
            "workspacePath": "/workspace",
            "query": "architecture"
        }))
        .expect("search message");
        assert!(!message.0.include_archived);
        assert_eq!(message.0.limit, 40);
    }
}
