//! Host callback for PagePublish (wired by desktop account APIs).

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, OnceLock};

use serde_json::Value;

#[derive(Debug, Clone)]
pub struct PagePublishHostRequest {
    pub slug: String,
    pub visibility: String,
    pub title: Option<String>,
    pub note: Option<String>,
    pub deploy: bool,
    pub directory: Option<String>,
    pub files: Option<HashMap<String, String>>,
}

pub type PagePublishFuture = Pin<Box<dyn Future<Output = Result<Value, String>> + Send>>;

pub type PagePublishHandler =
    Arc<dyn Fn(PagePublishHostRequest) -> PagePublishFuture + Send + Sync>;

static PAGE_PUBLISH_HANDLER: OnceLock<PagePublishHandler> = OnceLock::new();

/// Register the desktop handler that publishes page content via the account relay.
pub fn set_page_publish_handler(handler: PagePublishHandler) {
    let _ = PAGE_PUBLISH_HANDLER.set(handler);
}

pub async fn invoke_page_publish(request: PagePublishHostRequest) -> Result<Value, String> {
    let Some(handler) = PAGE_PUBLISH_HANDLER.get() else {
        return Err("PagePublish host is not available on this surface".to_string());
    };
    handler(request).await
}
