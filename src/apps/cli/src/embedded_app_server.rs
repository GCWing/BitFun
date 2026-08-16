//! Private in-process App Server assembly for the Embedded interactive TUI.

use std::sync::Arc;

use crate::runtime::CliRuntimeContext;
use crate::tui_backend::{AppServerTuiBackend, TuiBackend};
use anyhow::{Context, Result};
use bitfun_app_server::{AppManagementService, BitfunAppRuntime, BitfunAppServer};
use bitfun_app_server_protocol::app::{ClientInfo, HealthStatus, InitializeRequest};
use bitfun_app_server_protocol::PROTOCOL_VERSION;

pub(crate) struct EmbeddedAppServerHost {
    backend: Arc<dyn TuiBackend>,
    shutdown_tx: Option<tokio::sync::oneshot::Sender<()>>,
    server_thread: Option<std::thread::JoinHandle<()>>,
}

impl EmbeddedAppServerHost {
    pub(crate) async fn start(runtime: &CliRuntimeContext) -> Result<Self> {
        let (server_transport, client_transport) =
            bitfun_app_server_protocol::transport::in_memory_channel_pair();
        let app_runtime = BitfunAppRuntime::new(
            runtime.agent_runtime().clone(),
            runtime.agent_event_source(),
        )
        .with_context_reload(Arc::new(runtime.compatibility().clone()))
        .with_product_search(Arc::new(runtime.compatibility().clone()));
        let management = Arc::new(
            AppManagementService::load_for_local_host(Some(runtime.account_runtime().clone()))
                .await?,
        );
        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
        let server_thread = std::thread::Builder::new()
            .name("bitfun-embedded-app-server".to_string())
            .stack_size(16 * 1024 * 1024)
            .spawn(move || {
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .expect("failed to build Embedded App Server runtime");
                let local = tokio::task::LocalSet::new();
                runtime.block_on(local.run_until(async move {
                    tokio::select! {
                        result = BitfunAppServer::new(app_runtime)
                            .with_management(management)
                            .serve(server_transport) => {
                            if let Err(error) = result {
                                tracing::warn!("Embedded App Server stopped with an error: {error}");
                            }
                        }
                        _ = shutdown_rx => {}
                    }
                }));
            })
            .context("Failed to start the Embedded App Server thread")?;

        let client = match bitfun_app_server_client::connect(client_transport).await {
            Ok(client) => client,
            Err(error) => {
                let _ = shutdown_tx.send(());
                let _ = server_thread.join();
                return Err(error).context("Failed to connect the Embedded TUI App Server");
            }
        };
        let backend: Arc<dyn TuiBackend> = Arc::new(AppServerTuiBackend::new(client));
        let initialized = backend
            .initialize(InitializeRequest {
                protocol_version: PROTOCOL_VERSION,
                client: ClientInfo {
                    name: "bitfun-tui".to_string(),
                    version: env!("CARGO_PKG_VERSION").to_string(),
                },
            })
            .await
            .context("Failed to initialize the Embedded TUI App Server")?;
        if initialized.protocol_version != PROTOCOL_VERSION {
            let _ = shutdown_tx.send(());
            let _ = server_thread.join();
            anyhow::bail!(
                "Embedded App Server negotiated protocol {}, expected {}",
                initialized.protocol_version,
                PROTOCOL_VERSION
            );
        }
        let health = backend
            .health()
            .await
            .context("Embedded TUI App Server health request failed")?;
        if health.status != HealthStatus::Ready {
            let _ = shutdown_tx.send(());
            let _ = server_thread.join();
            anyhow::bail!("Embedded TUI App Server is not ready");
        }

        Ok(Self {
            backend,
            shutdown_tx: Some(shutdown_tx),
            server_thread: Some(server_thread),
        })
    }

    pub(crate) fn backend(&self) -> Arc<dyn TuiBackend> {
        self.backend.clone()
    }
}

impl Drop for EmbeddedAppServerHost {
    fn drop(&mut self) {
        if let Some(shutdown_tx) = self.shutdown_tx.take() {
            let _ = shutdown_tx.send(());
        }
        if let Some(server_thread) = self.server_thread.take() {
            let _ = server_thread.join();
        }
    }
}
