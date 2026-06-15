//! Embedded mini relay server for LAN / ngrok modes.
//!
//! Runs inside the desktop process, reusing the same relay logic as the
//! standalone relay-server binary. Uses `MemoryAssetStore` for in-memory
//! mobile-web file storage (no disk I/O for uploaded assets).

use bitfun_relay_server::{build_relay_router, MemoryAssetStore, RoomManager};
use log::info;
use std::sync::Arc;

/// Start the embedded relay and return a shutdown handle.
///
/// If `static_dir` is provided, the server also serves mobile-web static files
/// as a fallback for requests that don't match any API or WebSocket route.
pub async fn start_embedded_relay(
    port: u16,
    static_dir: Option<&str>,
) -> anyhow::Result<EmbeddedRelayHandle> {
    let room_manager = RoomManager::new();
    let asset_store = Arc::new(MemoryAssetStore::new());
    let start_time = std::time::Instant::now();

    let cleanup_rm = room_manager.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(60)).await;
            cleanup_rm.cleanup_stale_rooms(300);
        }
    });

    let mut app = build_relay_router(room_manager, asset_store, start_time);

    if let Some(dir) = static_dir {
        info!("Embedded relay: serving static files from {dir}");
        let serve_dir =
            tower_http::services::ServeDir::new(dir).append_index_html_on_directories(true);
        let static_app = axum::Router::<()>::new()
            .fallback_service(serve_dir)
            .layer(axum::middleware::from_fn(static_cache_headers));
        app = app.fallback_service(static_app);
    }

    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{port}"))
        .await
        .map_err(|e| anyhow::anyhow!("failed to bind embedded relay on port {port}: {e}"))?;

    info!("Embedded relay started on 0.0.0.0:{port}");

    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();
    tokio::spawn(async move {
        axum::serve(listener, app)
            .with_graceful_shutdown(async {
                let _ = shutdown_rx.await;
            })
            .await
            .ok();
    });

    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

    Ok(EmbeddedRelayHandle {
        _shutdown: Some(shutdown_tx),
    })
}

async fn static_cache_headers(
    request: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    let path = request.uri().path().to_string();
    let mut response = next.run(request).await;
    let headers = response.headers_mut();
    if path == "/" || path.ends_with(".html") {
        headers.insert(
            axum::http::header::CACHE_CONTROL,
            axum::http::HeaderValue::from_static("no-cache, no-store, must-revalidate"),
        );
        headers.insert(
            axum::http::header::PRAGMA,
            axum::http::HeaderValue::from_static("no-cache"),
        );
    } else if path.starts_with("/assets/") {
        headers.insert(
            axum::http::header::CACHE_CONTROL,
            axum::http::HeaderValue::from_static("public, max-age=31536000, immutable"),
        );
    }
    response
}

pub struct EmbeddedRelayHandle {
    _shutdown: Option<tokio::sync::oneshot::Sender<()>>,
}

impl EmbeddedRelayHandle {
    pub fn stop(&mut self) {
        if let Some(tx) = self._shutdown.take() {
            let _ = tx.send(());
            info!("Embedded relay stopped");
        }
    }
}

impl Drop for EmbeddedRelayHandle {
    fn drop(&mut self) {
        self.stop();
    }
}
