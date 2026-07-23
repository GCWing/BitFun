//! Desktop-owned embedded relay host for LAN and Ngrok Remote Connect modes.

use bitfun_core::service::remote_connect::embedded_relay_host::EmbeddedRelayHost;
use bitfun_relay_service::{build_relay_router, MemoryAssetStore, RoomManager};
use log::{info, warn};
use std::sync::Arc;
use tokio::sync::Mutex;

const SHUTDOWN_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(2);

#[derive(Default)]
pub(crate) struct DesktopEmbeddedRelayHost {
    runtime: Mutex<Option<EmbeddedRelayRuntime>>,
    #[cfg(test)]
    start_candidate_ready: tokio::sync::Notify,
}

struct EmbeddedRelayRuntime {
    shutdown: Option<tokio::sync::oneshot::Sender<()>>,
    server_task: Option<tokio::task::JoinHandle<()>>,
    cleanup_task: Option<tokio::task::JoinHandle<()>>,
}

impl EmbeddedRelayRuntime {
    fn signal_shutdown(&mut self) {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }

        if let Some(cleanup_task) = self.cleanup_task.take() {
            cleanup_task.abort();
        }
    }

    async fn stop(mut self) {
        self.signal_shutdown();

        if let Some(mut server_task) = self.server_task.take() {
            match tokio::time::timeout(SHUTDOWN_TIMEOUT, &mut server_task).await {
                Ok(Ok(())) => {}
                Ok(Err(error)) if error.is_cancelled() => {}
                Ok(Err(error)) => {
                    warn!("Embedded relay server task failed during shutdown: {error}");
                }
                Err(_) => {
                    warn!("Embedded relay shutdown timed out; aborting server task");
                    server_task.abort();
                    let _ = server_task.await;
                }
            }
        }

        info!("Embedded relay stopped");
    }

    fn abort(&mut self) {
        self.signal_shutdown();
        if let Some(server_task) = self.server_task.take() {
            server_task.abort();
        }
    }
}

impl Drop for EmbeddedRelayRuntime {
    fn drop(&mut self) {
        self.abort();
    }
}

impl Drop for DesktopEmbeddedRelayHost {
    fn drop(&mut self) {
        if let Some(mut runtime) = self.runtime.get_mut().take() {
            runtime.abort();
        }
    }
}

#[async_trait::async_trait]
impl EmbeddedRelayHost for DesktopEmbeddedRelayHost {
    async fn start(&self, port: u16, static_dir: Option<String>) -> anyhow::Result<()> {
        let mut runtime = self.runtime.lock().await;
        if runtime.is_some() {
            anyhow::bail!("embedded relay is already running");
        }

        let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{port}"))
            .await
            .map_err(|error| {
                anyhow::anyhow!("failed to bind embedded relay on port {port}: {error}")
            })?;

        let room_manager = RoomManager::new();
        let asset_store = Arc::new(MemoryAssetStore::new());
        let start_time = std::time::Instant::now();

        let mut app = build_relay_router(
            room_manager.clone(),
            asset_store,
            start_time,
            None,
            env!("CARGO_PKG_VERSION"),
        );

        if let Some(dir) = static_dir.as_deref() {
            info!("Embedded relay: serving static files from {dir}");
            let serve_dir =
                tower_http::services::ServeDir::new(dir).append_index_html_on_directories(true);
            let static_app = axum::Router::<()>::new()
                .fallback_service(serve_dir)
                .layer(axum::middleware::from_fn(static_cache_headers));
            app = app.fallback_service(static_app);
        }
        app = app.layer(axum::middleware::from_fn(
            bitfun_relay_service::relay_security_headers,
        ));

        info!("Embedded relay started on 0.0.0.0:{port}");

        let cleanup_room_manager = room_manager.clone();
        let cleanup_task = tokio::spawn(async move {
            loop {
                tokio::time::sleep(std::time::Duration::from_secs(60)).await;
                cleanup_room_manager.cleanup_stale_rooms(300);
            }
        });

        let (shutdown, shutdown_rx) = tokio::sync::oneshot::channel::<()>();
        let server_task = tokio::spawn(async move {
            axum::serve(
                listener,
                app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
            )
            .with_graceful_shutdown(async {
                let _ = shutdown_rx.await;
            })
            .await
            .ok();
        });

        // Keep the candidate local until readiness completes. If the start
        // future is cancelled, Drop aborts both tasks and releases the bound
        // listener instead of leaving a hidden active runtime in the host.
        let candidate = EmbeddedRelayRuntime {
            shutdown: Some(shutdown),
            server_task: Some(server_task),
            cleanup_task: Some(cleanup_task),
        };

        #[cfg(test)]
        self.start_candidate_ready.notify_one();

        // Preserve the existing readiness grace period before product
        // orchestration connects the local relay WebSocket.
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;

        *runtime = Some(candidate);

        Ok(())
    }

    async fn stop(&self) {
        let runtime = self.runtime.lock().await.take();
        if let Some(runtime) = runtime {
            runtime.stop().await;
        }
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::header::{CACHE_CONTROL, PRAGMA};

    async fn unused_port() -> u16 {
        let listener = tokio::net::TcpListener::bind("0.0.0.0:0")
            .await
            .expect("test should reserve an unused port");
        listener
            .local_addr()
            .expect("reserved listener should have an address")
            .port()
    }

    #[tokio::test]
    async fn bind_failure_does_not_create_an_active_runtime() {
        let occupied = tokio::net::TcpListener::bind("0.0.0.0:0")
            .await
            .expect("test should occupy a port");
        let port = occupied
            .local_addr()
            .expect("occupied listener should have an address")
            .port();
        let host = DesktopEmbeddedRelayHost::default();

        let error = host
            .start(port, None)
            .await
            .expect_err("starting on an occupied port should fail");

        assert!(error
            .to_string()
            .starts_with(&format!("failed to bind embedded relay on port {port}:")));
        assert!(host.runtime.lock().await.is_none());
    }

    #[tokio::test]
    async fn static_cache_headers_and_listener_lifecycle_are_preserved() {
        let static_dir = std::env::temp_dir().join(format!(
            "bitfun-embedded-relay-host-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(static_dir.join("assets"))
            .expect("test static directory should be created");
        std::fs::write(static_dir.join("index.html"), "embedded relay test")
            .expect("test index should be written");
        std::fs::write(static_dir.join("assets").join("app.js"), "test asset")
            .expect("test asset should be written");

        let port = unused_port().await;
        let host = DesktopEmbeddedRelayHost::default();
        host.start(port, Some(static_dir.to_string_lossy().into_owned()))
            .await
            .expect("embedded relay should start");

        let client = reqwest::Client::new();
        let index = client
            .get(format!("http://127.0.0.1:{port}/"))
            .send()
            .await
            .expect("index request should complete");
        assert!(index.status().is_success());
        assert_eq!(
            index
                .headers()
                .get(CACHE_CONTROL)
                .and_then(|v| v.to_str().ok()),
            Some("no-cache, no-store, must-revalidate")
        );
        assert_eq!(
            index.headers().get(PRAGMA).and_then(|v| v.to_str().ok()),
            Some("no-cache")
        );

        let asset = client
            .get(format!("http://127.0.0.1:{port}/assets/app.js"))
            .send()
            .await
            .expect("asset request should complete");
        assert!(asset.status().is_success());
        assert_eq!(
            asset
                .headers()
                .get(CACHE_CONTROL)
                .and_then(|v| v.to_str().ok()),
            Some("public, max-age=31536000, immutable")
        );

        drop(index);
        drop(asset);
        drop(client);
        host.stop().await;

        host.start(port, Some(static_dir.to_string_lossy().into_owned()))
            .await
            .expect("embedded relay should restart immediately on the same port");
        host.stop().await;

        let released = tokio::net::TcpListener::bind(("0.0.0.0", port))
            .await
            .expect("stop must release the listener before returning");
        drop(released);
        std::fs::remove_dir_all(&static_dir).expect("test static directory should be removed");
    }

    #[tokio::test]
    async fn cancelled_start_releases_listener_without_committing_runtime() {
        let port = unused_port().await;
        let host = Arc::new(DesktopEmbeddedRelayHost::default());
        let start_task = tokio::spawn({
            let host = host.clone();
            async move { host.start(port, None).await }
        });

        tokio::time::timeout(
            std::time::Duration::from_secs(1),
            host.start_candidate_ready.notified(),
        )
        .await
        .expect("start should create the candidate runtime before readiness completes");
        start_task.abort();
        let _ = start_task.await;

        assert!(host.runtime.lock().await.is_none());
        let released = tokio::net::TcpListener::bind(("0.0.0.0", port))
            .await
            .expect("cancelling start must release the listener");
        drop(released);
    }
}
