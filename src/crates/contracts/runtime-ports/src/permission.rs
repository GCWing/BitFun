use crate::{PortResult, RuntimeServicePort};
use async_trait::async_trait;
use bitfun_product_domains::tool_permissions::{
    PermissionAuditRecord, PermissionGrant, PermissionGrantKey,
};

/// Persistent remembered-grant storage. Pending requests do not belong here.
#[async_trait]
pub trait PermissionGrantStorePort: RuntimeServicePort {
    async fn list_project_grants(&self, project_id: &str) -> PortResult<Vec<PermissionGrant>>;

    async fn add_project_grants(&self, grants: Vec<PermissionGrant>) -> PortResult<()>;

    async fn remove_project_grant(&self, key: PermissionGrantKey) -> PortResult<bool>;

    async fn clear_project_grants(&self, project_id: &str) -> PortResult<usize>;
}

/// Append-only permission audit persistence over presentation-safe DTOs.
#[async_trait]
pub trait PermissionAuditStorePort: RuntimeServicePort {
    async fn append_permission_audit(&self, record: PermissionAuditRecord) -> PortResult<()>;

    async fn list_project_permission_audit(
        &self,
        project_id: &str,
    ) -> PortResult<Vec<PermissionAuditRecord>>;
}

/// Atomically commits the durable effects of one permission reply.
#[async_trait]
pub trait PermissionReplyStorePort: RuntimeServicePort {
    async fn commit_permission_reply(
        &self,
        grants: Vec<PermissionGrant>,
        audit: Vec<PermissionAuditRecord>,
    ) -> PortResult<()>;
}
