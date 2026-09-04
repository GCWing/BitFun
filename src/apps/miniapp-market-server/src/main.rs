use anyhow::Context;
use openbitfun_miniapp_market_service::build_market_router;
use openbitfun_miniapp_market_service::config::MarketConfig;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .json()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                EnvFilter::new("openbitfun_miniapp_market=info,tower_http=info")
            }),
        )
        .init();

    let config = MarketConfig::from_env()?;
    let bind = config.bind;
    let app = build_market_router(config).await?;
    let listener = tokio::net::TcpListener::bind(bind)
        .await
        .with_context(|| format!("failed to bind MiniApp market server on {bind}"))?;
    tracing::info!(address = %bind, "MiniApp market server started");
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .context("MiniApp market server stopped unexpectedly")
}

async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    #[cfg(unix)]
    let terminate = async {
        use tokio::signal::unix::{signal, SignalKind};
        if let Ok(mut signal) = signal(SignalKind::terminate()) {
            signal.recv().await;
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {}
        _ = terminate => {}
    }
}
