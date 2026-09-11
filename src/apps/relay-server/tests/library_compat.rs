use openbitfun_relay_server::{
    admin, build_relay_router, db, relay, routes, AppState, DiskAssetStore, MemoryAssetStore,
    WebAssetStore,
};
use std::sync::Arc;
use std::time::Instant;

#[tokio::test]
async fn openbitfun_library_path_exposes_supported_relay_api() {
    let _: fn(Arc<dyn WebAssetStore>, Instant, Arc<db::DbPool>) -> axum::Router =
        build_relay_router;
    let _ = admin::list_users;
    let _ = db::connect;
    let _ = DiskAssetStore::new;
    let _ = MemoryAssetStore::new;
    let _ = routes::api::health_check;
    // Pin the symbol, not a call: `server_info` is async, and `let _ =` on the
    // returned future would drop it unpolled.
    let _ = routes::api::server_info;
    let _ = std::mem::size_of::<AppState>();
    let _ = AppState {
        start_time: Instant::now(),
        asset_store: Arc::new(MemoryAssetStore::new()),
        db: Arc::new(db::connect(":memory:").await.unwrap()),
        page_data: None,
        page_access_manager: Arc::new(routes::pages::PageAccessManager::new()),
        page_upload_manager: Arc::new(routes::pages::PageUploadManager::new()),
        page_execution_guard: Arc::new(
            openbitfun_relay_server::page_execution::PageExecutionGuard::new(),
        ),
        login_rate_limiter: Arc::new(routes::auth::LoginRateLimiter::new()),
        device_manager: relay::DeviceManager::new(),
        cors_allow_origins: Arc::new(Vec::new()),
        page_browser_auth: None,
    };

    fn require_store<T: WebAssetStore>() {}
    require_store::<MemoryAssetStore>();
}
