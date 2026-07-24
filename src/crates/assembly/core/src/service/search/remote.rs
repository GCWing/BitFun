use crate::service::config::{get_global_config_service, types::WorkspaceConfig, ConfigService};
use crate::service::remote_ssh::workspace_state::{
    get_remote_workspace_manager, lookup_remote_connection, lookup_remote_connection_with_hint,
    RemoteWorkspaceEntry,
};
use crate::service::remote_ssh::{RemoteFileService, SSHConnectionManager};
use crate::service::search::{
    ContentSearchRequest, ContentSearchResult, GlobSearchRequest, GlobSearchResult,
    IndexTaskHandle, WorkspaceIndexStatus,
};
use async_trait::async_trait;
use bitfun_services_integrations::remote_ssh::workspace_search::{
    RemoteCommandOutput, RemoteWorkspaceSearchProvider,
    RemoteWorkspaceSearchService as ServiceRemoteWorkspaceSearchService,
    RemoteWorkspaceSearchStdioProtocol,
};
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::mpsc;

const REMOTE_FLASHGREP_LOG_TARGET: &str = "flashgrep";

#[derive(Clone)]
pub struct RemoteWorkspaceSearchService {
    inner: ServiceRemoteWorkspaceSearchService,
}

impl RemoteWorkspaceSearchService {
    pub fn new(
        ssh_manager: SSHConnectionManager,
        remote_file_service: RemoteFileService,
        config_service: Arc<ConfigService>,
    ) -> Self {
        let provider = Arc::new(CoreRemoteWorkspaceSearchProvider {
            ssh_manager,
            remote_file_service,
            config_service,
        });
        Self {
            inner: ServiceRemoteWorkspaceSearchService::new(provider),
        }
    }

    pub fn with_preferred_connection_id(mut self, preferred_connection_id: Option<String>) -> Self {
        self.inner = self
            .inner
            .with_preferred_connection_id(preferred_connection_id);
        self
    }

    pub async fn get_index_status(&self, root_path: &str) -> Result<WorkspaceIndexStatus, String> {
        self.inner.get_index_status(root_path).await
    }

    pub async fn build_index(&self, root_path: &str) -> Result<IndexTaskHandle, String> {
        self.inner.build_index(root_path).await
    }

    pub async fn rebuild_index(&self, root_path: &str) -> Result<IndexTaskHandle, String> {
        self.inner.rebuild_index(root_path).await
    }

    pub async fn search_content(
        &self,
        request: ContentSearchRequest,
    ) -> Result<ContentSearchResult, String> {
        self.inner.search_content(request).await
    }

    pub async fn glob(&self, request: GlobSearchRequest) -> Result<GlobSearchResult, String> {
        self.inner.glob(request).await
    }

    pub async fn resolve_remote_workspace_entry(
        &self,
        root_path: &str,
    ) -> Result<RemoteWorkspaceEntry, String> {
        self.inner.resolve_remote_workspace_entry(root_path).await
    }
}

#[derive(Clone)]
struct CoreRemoteWorkspaceSearchProvider {
    ssh_manager: SSHConnectionManager,
    remote_file_service: RemoteFileService,
    config_service: Arc<ConfigService>,
}

#[async_trait]
impl RemoteWorkspaceSearchProvider for CoreRemoteWorkspaceSearchProvider {
    async fn resolve_workspace_entry(
        &self,
        root_path: &str,
        preferred_connection_id: Option<&str>,
    ) -> Result<RemoteWorkspaceEntry, String> {
        if let Some(entry) =
            lookup_remote_connection_with_hint(root_path, preferred_connection_id).await
        {
            return Ok(entry);
        }
        lookup_remote_connection(root_path)
            .await
            .ok_or_else(|| format!("Remote workspace is not registered for path: {root_path}"))
    }

    async fn cached_server_os_type(&self, connection_id: &str) -> Option<String> {
        self.ssh_manager
            .get_server_info(connection_id)
            .await
            .map(|info| info.os_type)
    }

    async fn execute_command(
        &self,
        connection_id: &str,
        command: &str,
    ) -> Result<RemoteCommandOutput, String> {
        self.ssh_manager
            .execute_command(connection_id, command)
            .await
            .map(|(stdout, stderr, exit_code)| RemoteCommandOutput {
                stdout,
                stderr,
                exit_code,
            })
            .map_err(|error| error.to_string())
    }

    async fn create_dir_all(&self, connection_id: &str, path: &str) -> Result<(), String> {
        self.remote_file_service
            .create_dir_all(connection_id, path)
            .await
            .map_err(|error| error.to_string())
    }

    async fn write_file(
        &self,
        connection_id: &str,
        path: &str,
        contents: &[u8],
    ) -> Result<(), String> {
        self.remote_file_service
            .write_file(connection_id, path, contents)
            .await
            .map_err(|error| error.to_string())
    }

    async fn repo_max_file_size(&self) -> u64 {
        match self
            .config_service
            .get_config::<WorkspaceConfig>(Some("workspace"))
            .await
        {
            Ok(workspace_config) => workspace_config.max_file_size,
            Err(error) => {
                log::warn!(
                    target: REMOTE_FLASHGREP_LOG_TARGET,
                    "Failed to read workspace config for remote flashgrep repo open, using default max_file_size: {}",
                    error
                );
                WorkspaceConfig::default().max_file_size
            }
        }
    }

    async fn spawn_stdio_daemon(
        &self,
        connection_id: &str,
        command: &str,
        write_rx: mpsc::Receiver<Vec<u8>>,
        protocol: RemoteWorkspaceSearchStdioProtocol,
    ) -> Result<(), String> {
        let transport = self
            .ssh_manager
            .open_workspace_stdio(connection_id, command)
            .await
            .map_err(|error| format!("Failed to start remote flashgrep stdio daemon: {error}"))?;
        spawn_remote_stdio_owner(connection_id.to_string(), transport, write_rx, protocol);
        Ok(())
    }
}

pub async fn remote_workspace_search_service_for_path(
    root_path: &str,
    preferred_connection_id: Option<String>,
) -> Result<RemoteWorkspaceSearchService, String> {
    let manager = get_remote_workspace_manager()
        .ok_or_else(|| "Remote workspace manager is unavailable".to_string())?;
    let preferred_connection_id = match preferred_connection_id {
        Some(connection_id) => Some(connection_id),
        None => lookup_remote_connection(root_path)
            .await
            .map(|entry| entry.connection_id),
    };

    Ok(RemoteWorkspaceSearchService::new(
        manager
            .get_ssh_manager()
            .await
            .ok_or_else(|| "SSH manager unavailable".to_string())?,
        manager
            .get_file_service()
            .await
            .ok_or_else(|| "Remote file service unavailable".to_string())?,
        get_global_config_service()
            .await
            .map_err(|error| format!("Config service unavailable: {error}"))?,
    )
    .with_preferred_connection_id(preferred_connection_id))
}

fn spawn_remote_stdio_owner(
    connection_id: String,
    transport: bitfun_services_integrations::remote_ssh::WorkspaceStdio,
    mut write_rx: mpsc::Receiver<Vec<u8>>,
    protocol: RemoteWorkspaceSearchStdioProtocol,
) {
    tokio::spawn(async move {
        let (mut writer, mut stdout, mut stderr, _control, _completion) = transport.into_parts();
        let mut read_buffer = Vec::<u8>::new();
        let stderr_protocol = protocol.clone();
        let stderr_connection_id = connection_id.clone();
        let stderr_task = tokio::spawn(async move {
            let mut buffer = vec![0u8; 16 * 1024];
            loop {
                match stderr.read(&mut buffer).await {
                    Ok(0) | Err(_) => break,
                    Ok(read) => {
                        let text = String::from_utf8_lossy(&buffer[..read]);
                        let log_context = format!("connection_id={stderr_connection_id}");
                        for line in text.lines() {
                            stderr_protocol.log_stderr_line_with_context(Some(&log_context), line);
                        }
                    }
                }
            }
        });
        let mut stdout_chunk = vec![0u8; 16 * 1024];

        loop {
            tokio::select! {
                outbound = write_rx.recv() => {
                    let Some(outbound) = outbound else {
                        let _ = writer.shutdown().await;
                        break;
                    };
                    if let Err(error) = writer.write_all(&outbound).await {
                        log::warn!(
                            target: REMOTE_FLASHGREP_LOG_TARGET,
                            "Failed to write remote flashgrep stdio request: connection_id={}, error={}",
                            connection_id,
                            error
                        );
                        protocol
                            .close_with_message("remote flashgrep stdio daemon write failed")
                            .await;
                        break;
                    }
                    if let Err(error) = writer.flush().await {
                        log::warn!(
                            target: REMOTE_FLASHGREP_LOG_TARGET,
                            "Failed to flush remote flashgrep stdio request: connection_id={}, error={}",
                            connection_id,
                            error
                        );
                        protocol
                            .close_with_message("remote flashgrep stdio daemon flush failed")
                            .await;
                        break;
                    }
                }

                read = stdout.read(&mut stdout_chunk) => {
                    match read {
                        Ok(0) => break,
                        Ok(read) => {
                            if let Err(error) = protocol.handle_stdout_chunk(&mut read_buffer, &stdout_chunk[..read]).await {
                                log::warn!(
                                    target: REMOTE_FLASHGREP_LOG_TARGET,
                                    "Failed to decode remote flashgrep stdio message: connection_id={}, error={}",
                                    connection_id,
                                    error
                                );
                                protocol
                                    .close_with_message(format!(
                                        "remote flashgrep stdio daemon decode failed: {error}"
                                    ))
                                    .await;
                                break;
                            }
                        }
                        Err(error) => {
                            log::warn!(
                                target: REMOTE_FLASHGREP_LOG_TARGET,
                                "Failed to read remote flashgrep stdio response: connection_id={}, error={}",
                                connection_id,
                                error
                            );
                            break;
                        }
                    }
                }
            }
        }

        stderr_task.abort();
        protocol
            .close_with_message("remote flashgrep stdio daemon closed before sending a response")
            .await;
    });
}
