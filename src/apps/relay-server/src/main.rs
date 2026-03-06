//! BitFun Relay Server
//!
//! Standalone binary that runs the relay as a network service.
//! Uses `DiskAssetStore` for filesystem-backed mobile-web file storage.

use std::sync::Arc;
use tracing::info;

mod config;

use bitfun_relay_server::{build_relay_router, DiskAssetStore, RoomManager, WebAssetStore};
use config::RelayConfig;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .init();

    let cfg = RelayConfig::from_env();
    info!("BitFun Relay Server v{}", env!("CARGO_PKG_VERSION"));

    let room_manager = RoomManager::new();
    let asset_store = Arc::new(DiskAssetStore::new(&cfg.room_web_dir));

    let cleanup_rm = room_manager.clone();
    let cleanup_ttl = cfg.room_ttl_secs;
    let cleanup_store = asset_store.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(60)).await;
            let stale_ids = cleanup_rm.cleanup_stale_rooms(cleanup_ttl);
            for room_id in &stale_ids {
                cleanup_store.cleanup_room(room_id);
            }
        }
    });

    let start_time = std::time::Instant::now();
    let mut app = build_relay_router(room_manager, asset_store, start_time);

    if let Some(static_dir) = &cfg.static_dir {
        info!("Serving static files from: {static_dir}");
        app = app.fallback_service(
            tower_http::services::ServeDir::new(static_dir)
                .append_index_html_on_directories(true),
        );
    }

    info!("Room web upload dir: {}", cfg.room_web_dir);

    let listener = tokio::net::TcpListener::bind(cfg.listen_addr).await?;
    info!("Relay server listening on {}", cfg.listen_addr);
    info!("WebSocket endpoint: ws://{}/ws", cfg.listen_addr);

    axum::serve(listener, app).await?;
    Ok(())
}
