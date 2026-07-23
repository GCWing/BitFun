mod runtime;

use anyhow::{Context, Result};

async fn run_host() -> Result<()> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::WARN)
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .with_target(false)
        .init();

    let workspace_root = std::env::current_dir().context("Failed to resolve SDK workspace")?;
    runtime::SdkHostRuntime::select_process_profile()?;
    runtime::initialize_terminal_service().await;

    bitfun_core::service::config::initialize_global_config()
        .await
        .context("Failed to initialize global config service")?;
    bitfun_core::infrastructure::ai::AIClientFactory::initialize_global()
        .await
        .context("Failed to initialize global AI client factory")?;

    let host = runtime::SdkHostRuntime::build(&workspace_root)
        .await
        .context("Failed to assemble Agent SDK Host")?;
    bitfun_sdk_host_app::transport::serve_stdio(
        host.agent_runtime().clone(),
        host.workspace_root().to_string_lossy().into_owned(),
    )
    .await
    .context("Agent SDK Host transport failed")
}

fn main() {
    bitfun_sdk_host_app::initialize_process_runtime();

    let worker = bitfun_sdk_host_app::spawn_sdk_host_worker(|| {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("failed to build SDK Host Tokio runtime");
        runtime.block_on(run_host())
    })
    .expect("failed to spawn SDK Host worker thread");

    match worker.join() {
        Ok(Ok(())) => {}
        Ok(Err(error)) => {
            eprintln!("Error: {error:#}");
            std::process::exit(1);
        }
        Err(_) => {
            eprintln!("Error: SDK Host worker thread panicked");
            std::process::exit(1);
        }
    }
}
