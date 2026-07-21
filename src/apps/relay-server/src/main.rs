//! BitFun Relay Server
//!
//! Standalone binary that runs the relay as a network service.
//! Uses `DiskAssetStore` for filesystem-backed mobile-web file storage.

use anyhow::Context;
use std::sync::Arc;
use tracing::info;

mod config;

use bitfun_relay_service::{DiskAssetStore, RoomManager, WebAssetStore};
use config::RelayConfig;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let cfg = RelayConfig::from_env();
    info!("BitFun Relay Server v{}", env!("CARGO_PKG_VERSION"));

    let room_manager = RoomManager::new();
    let asset_store = Arc::new(DiskAssetStore::new_with_max_bytes(
        &cfg.room_web_dir,
        cfg.asset_store_max_bytes,
    ));

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

    let db = if let Some(path) = &cfg.db_path {
        let pool = bitfun_relay_service::db::connect(path)
            .await
            .with_context(|| {
                format!("failed to initialize configured account database at {path}")
            })?;
        Some(Arc::new(pool))
    } else {
        info!("RELAY_DB_PATH not set — account features disabled (pure relay mode)");
        None
    };
    if db.is_some() && cfg.cors_allow_origins.iter().any(|origin| origin == "*") {
        anyhow::bail!(
            "RELAY_CORS_ALLOW_ORIGINS=* is not allowed when RELAY_DB_PATH enables account APIs"
        );
    }

    let page_data_dir = std::path::PathBuf::from(&cfg.room_web_dir).join("page-data");
    let mut app = bitfun_relay_service::build_relay_router_with_page_data_and_origins(
        room_manager,
        asset_store,
        start_time,
        db,
        env!("CARGO_PKG_VERSION"),
        Some(page_data_dir),
        cfg.cors_allow_origins.clone(),
    );

    if let Some(static_dir) = &cfg.static_dir {
        info!("Serving static files from: {static_dir}");
        app = app.fallback_service(
            tower_http::services::ServeDir::new(static_dir).append_index_html_on_directories(true),
        );
    }
    // Re-apply after installing the optional fallback so static files receive
    // the same browser hardening as relay API responses.
    app = app.layer(axum::middleware::from_fn(
        bitfun_relay_service::relay_security_headers,
    ));

    info!("Room web upload dir: {}", cfg.room_web_dir);
    info!("Asset store capacity: {} bytes", cfg.asset_store_max_bytes);

    let listener = tokio::net::TcpListener::bind(cfg.listen_addr).await?;
    info!("Relay server listening on {}", cfg.listen_addr);
    info!("WebSocket endpoint: ws://{}/ws", cfg.listen_addr);

    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .await?;
    Ok(())
}
