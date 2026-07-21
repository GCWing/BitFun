use bitfun_relay_server::{
    admin, build_relay_router, db, relay, routes, AppState, DiskAssetStore, MemoryAssetStore,
    ResponsePayload, RoomManager, WebAssetStore,
};
use std::sync::Arc;
use std::time::Instant;

#[test]
fn legacy_library_path_exposes_supported_relay_api() {
    let _: fn(
        Arc<RoomManager>,
        Arc<dyn WebAssetStore>,
        Instant,
        Option<Arc<db::DbPool>>,
    ) -> axum::Router = build_relay_router;
    let _ = admin::list_users;
    let _ = db::connect;
    let _ = DiskAssetStore::new;
    let _ = MemoryAssetStore::new;
    let _ = RoomManager::new;
    let _ = relay::room::RoomManager::new;
    let _ = routes::api::health_check;
    let _ = routes::api::server_info();
    let _: Option<ResponsePayload> = None;
    let _ = std::mem::size_of::<AppState>();
    let _ = AppState {
        room_manager: RoomManager::new(),
        start_time: Instant::now(),
        asset_store: Arc::new(MemoryAssetStore::new()),
        db: None,
        page_data: None,
        login_rate_limiter: Arc::new(routes::auth::LoginRateLimiter::new()),
        device_manager: relay::DeviceManager::new(),
        cors_allow_origins: Arc::new(Vec::new()),
    };

    fn require_store<T: WebAssetStore>() {}
    require_store::<MemoryAssetStore>();
}
