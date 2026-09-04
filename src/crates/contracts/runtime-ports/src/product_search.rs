use crate::PortResult;
use async_trait::async_trait;
use openbitfun_product_domains::product_search::{
    SessionContentSearchRequest, SessionContentSearchResponse,
};

/// Host-neutral entry point for searching authoritative persisted session content.
#[async_trait]
pub trait ProductSearchPort: Send + Sync {
    async fn search_session_content(
        &self,
        request: SessionContentSearchRequest,
    ) -> PortResult<SessionContentSearchResponse>;
}
