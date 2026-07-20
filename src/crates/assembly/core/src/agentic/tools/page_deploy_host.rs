//! Host callback for PageDeploy (wired by desktop account APIs).

use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, OnceLock};

use serde_json::Value;

pub type PageDeployFuture = Pin<Box<dyn Future<Output = Result<Value, String>> + Send>>;

pub type PageDeployHandler = Arc<dyn Fn(String, String) -> PageDeployFuture + Send + Sync>;

static PAGE_DEPLOY_HANDLER: OnceLock<PageDeployHandler> = OnceLock::new();

/// Register the desktop handler that deploys a page version via the account relay.
pub fn set_page_deploy_handler(handler: PageDeployHandler) {
    let _ = PAGE_DEPLOY_HANDLER.set(handler);
}

pub async fn invoke_page_deploy(slug: String, version_id: String) -> Result<Value, String> {
    let Some(handler) = PAGE_DEPLOY_HANDLER.get() else {
        return Err("PageDeploy host is not available on this surface".to_string());
    };
    handler(slug, version_id).await
}
