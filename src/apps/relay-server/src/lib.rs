//! Compatibility import path for relay library consumers.
//!
//! Runtime ownership lives in `bitfun-relay-service`. New code should depend
//! on that crate directly; this facade preserves the existing import paths.

pub use bitfun_relay_service::{
    admin, db, relay, routes, AppState, DiskAssetStore, MemoryAssetStore, ResponsePayload,
    RoomManager, WebAssetStore,
};

/// Builds the shared relay router using this compatibility host's version.
pub fn build_relay_router(
    room_manager: std::sync::Arc<RoomManager>,
    asset_store: std::sync::Arc<dyn WebAssetStore>,
    start_time: std::time::Instant,
    db: Option<std::sync::Arc<db::DbPool>>,
) -> axum::Router {
    bitfun_relay_service::build_relay_router_with_page_data(
        room_manager,
        asset_store,
        start_time,
        db,
        env!("CARGO_PKG_VERSION"),
        None,
    )
}
