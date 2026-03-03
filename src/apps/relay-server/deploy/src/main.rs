//! BitFun Relay Server
//!
//! WebSocket relay for Remote Connect. Manages rooms and forwards E2E encrypted
//! messages between desktop and mobile clients. Also serves mobile web static files.

use axum::extract::DefaultBodyLimit;
use axum::routing::{get, post};
use axum::Router;
use tower_http::cors::CorsLayer;
use tracing::info;

mod config;
mod relay;
mod routes;

use config::RelayConfig;
use relay::RoomManager;
use routes::api::{self, AppState};
use routes::websocket;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .init();

    let cfg = RelayConfig::from_env();
    info!("BitFun Relay Server v{}", env!("CARGO_PKG_VERSION"));

    let room_manager = RoomManager::new();

    let cleanup_rm = room_manager.clone();
    let cleanup_ttl = cfg.room_ttl_secs;
    let cleanup_room_web_dir = cfg.room_web_dir.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(60)).await;
            let stale_ids = cleanup_rm.cleanup_stale_rooms(cleanup_ttl);
            for room_id in &stale_ids {
                api::cleanup_room_web(&cleanup_room_web_dir, room_id);
            }
        }
    });

    let store_dir = std::path::PathBuf::from(&cfg.room_web_dir).join("_store");
    let _ = std::fs::create_dir_all(&store_dir);
    let content_store = std::sync::Arc::new(api::ContentStore::new(&store_dir));

    let state = AppState {
        room_manager,
        start_time: std::time::Instant::now(),
        room_web_dir: cfg.room_web_dir.clone(),
        content_store,
    };

    let mut app = Router::new()
        .route("/health", get(api::health_check))
        .route("/api/info", get(api::server_info))
        .route("/api/rooms/:room_id/join", post(api::join_room))
        .route("/api/rooms/:room_id/message", post(api::relay_message))
        .route("/api/rooms/:room_id/poll", get(api::poll_messages))
        .route("/api/rooms/:room_id/ack", post(api::ack_messages))
        .route(
            "/api/rooms/:room_id/upload-web",
            post(api::upload_web).layer(DefaultBodyLimit::max(10 * 1024 * 1024)),
        )
        .route(
            "/api/rooms/:room_id/check-web-files",
            post(api::check_web_files),
        )
        .route(
            "/api/rooms/:room_id/upload-web-files",
            post(api::upload_web_files).layer(DefaultBodyLimit::max(10 * 1024 * 1024)),
        )
        .route("/r/*rest", get(api::serve_room_web_catchall))
        .route("/ws", get(websocket::websocket_handler))
        .layer(CorsLayer::permissive())
        .with_state(state);

    // Serve mobile web static files as a fallback for requests that
    // don't match any API or WebSocket route.
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
