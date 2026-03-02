//! BitFun Relay Server
//!
//! WebSocket relay for Remote Connect. Manages rooms and forwards E2E encrypted
//! messages between desktop and mobile clients. Also serves mobile web static files.

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
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(60)).await;
            cleanup_rm.cleanup_stale_rooms(cleanup_ttl);
        }
    });

    let state = AppState {
        room_manager,
        start_time: std::time::Instant::now(),
    };

    let mut app = Router::new()
        .route("/health", get(api::health_check))
        .route("/api/info", get(api::server_info))
        .route("/api/rooms/{room_id}/join", post(api::join_room))
        .route("/api/rooms/{room_id}/message", post(api::relay_message))
        .route("/api/rooms/{room_id}/poll", get(api::poll_messages))
        .route("/api/rooms/{room_id}/ack", post(api::ack_messages))
        .route("/ws", get(websocket::websocket_handler))
        .layer(CorsLayer::permissive())
        .with_state(state);

    // Serve mobile web static files if configured
    if let Some(static_dir) = &cfg.static_dir {
        info!("Serving static files from: {static_dir}");
        app = app.nest_service(
            "/",
            tower_http::services::ServeDir::new(static_dir)
                .append_index_html_on_directories(true),
        );
    }

    let listener = tokio::net::TcpListener::bind(cfg.listen_addr).await?;
    info!("Relay server listening on {}", cfg.listen_addr);
    info!("WebSocket endpoint: ws://{}/ws", cfg.listen_addr);

    axum::serve(listener, app).await?;
    Ok(())
}
