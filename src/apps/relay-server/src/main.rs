//! OpenBitFun Relay Server
//!
//! Standalone binary that runs the relay as a network service.
//! Uses `DiskAssetStore` for filesystem-backed published Page assets.

use anyhow::Context;
use std::sync::Arc;
use tracing::info;

mod config;

use config::RelayConfig;
use openbitfun_relay_service::DiskAssetStore;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let cfg = RelayConfig::from_env()?;
    info!("OpenBitFun Relay Server v{}", env!("CARGO_PKG_VERSION"));

    let asset_store = Arc::new(DiskAssetStore::new_with_max_bytes(
        &cfg.asset_dir,
        cfg.asset_store_max_bytes,
    ));

    let start_time = std::time::Instant::now();

    let db = if let Some(path) = &cfg.db_path {
        let pool = openbitfun_relay_service::db::connect(path)
            .await
            .with_context(|| {
                format!("failed to initialize configured account database at {path}")
            })?;
        Arc::new(pool)
    } else {
        anyhow::bail!("RELAY_DB_PATH is required; anonymous relay mode is no longer supported")
    };
    if cfg.cors_allow_origins.iter().any(|origin| origin == "*") {
        anyhow::bail!(
            "RELAY_CORS_ALLOW_ORIGINS=* is not allowed when RELAY_DB_PATH enables account APIs"
        );
    }
    let page_browser_auth = match (
        cfg.page_public_base_url.as_deref(),
        cfg.page_auth_base_url.as_deref(),
    ) {
        (Some(public_base_url), Some(auth_base_url)) => Some(
            openbitfun_relay_service::PageBrowserAuthConfig::new(public_base_url, auth_base_url)
                .map_err(anyhow::Error::msg)?,
        ),
        (None, None) => {
            {
                tracing::warn!(
                    "RELAY_PAGE_PUBLIC_BASE_URL and RELAY_PAGE_AUTH_BASE_URL are not set; \
                     published Pages are disabled until isolated origins are configured"
                );
            }
            None
        }
        _ => anyhow::bail!(
            "RELAY_PAGE_PUBLIC_BASE_URL and RELAY_PAGE_AUTH_BASE_URL must be configured together"
        ),
    };

    let pages_enabled = page_browser_auth.is_some();
    let page_data_dir = std::path::PathBuf::from(&cfg.asset_dir).join("page-data");
    let mut app = openbitfun_relay_service::build_relay_router_with_page_data_origins_and_page_auth(
        asset_store,
        start_time,
        db,
        env!("CARGO_PKG_VERSION"),
        pages_enabled.then_some(page_data_dir),
        cfg.cors_allow_origins.clone(),
        page_browser_auth,
    );

    if let Some(static_dir) = &cfg.static_dir {
        info!("Serving static files from: {static_dir}");
        app = app.fallback_service(
            tower_http::services::ServeDir::new(static_dir).append_index_html_on_directories(true),
        );
    }
    if !pages_enabled {
        app = app.layer(axum::middleware::from_fn(require_isolated_page_origins));
    }
    // Re-apply after installing the optional fallback so static files receive
    // the same browser hardening as relay API responses.
    app = app.layer(axum::middleware::from_fn(host_security_headers));

    info!("Page asset directory: {}", cfg.asset_dir);
    info!("Asset store capacity: {} bytes", cfg.asset_store_max_bytes);

    let listener = tokio::net::TcpListener::bind(cfg.listen_addr).await?;
    info!("Relay server listening on {}", cfg.listen_addr);
    info!("Device WebSocket endpoint: ws://{}/ws", cfg.listen_addr);

    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .await?;
    Ok(())
}

// The trusted mobile document needs camera access for its QR scanner. Keep
// uploaded content and API responses under the shared restrictive policy.
async fn host_security_headers(
    request: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    let controller_document = matches!(request.uri().path(), "/" | "/index.html");
    let mut response = openbitfun_relay_service::relay_security_headers(request, next).await;
    if controller_document {
        response.headers_mut().insert(
            "permissions-policy",
            axum::http::HeaderValue::from_static("camera=(self), microphone=(), geolocation=()"),
        );
    }
    response
}

async fn require_isolated_page_origins(
    request: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    use axum::response::IntoResponse;
    if is_published_page_path(request.uri().path()) {
        return (
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            axum::Json(serde_json::json!({"error": "Published Pages require isolated public and sign-in origins"})),
        ).into_response();
    }
    next.run(request).await
}

fn is_published_page_path(path: &str) -> bool {
    ["/api/pages", "/api/page-auth", "/p"].iter().any(|prefix| {
        path == *prefix
            || path
                .strip_prefix(prefix)
                .is_some_and(|tail| tail.starts_with('/'))
    })
}

#[cfg(test)]
mod tests {
    use super::is_published_page_path;

    #[test]
    fn gate_all_published_page_routes_without_blocking_account_or_device_routes() {
        for path in [
            "/api/pages",
            "/api/pages/foo",
            "/api/page-auth/login",
            "/p",
            "/p/owner/page",
        ] {
            assert!(is_published_page_path(path), "{path}");
        }
        for path in [
            "/health",
            "/ws",
            "/api/devices",
            "/api/auth/login",
            "/privacy",
            "/api/pages-other",
        ] {
            assert!(!is_published_page_path(path), "{path}");
        }
    }
}
