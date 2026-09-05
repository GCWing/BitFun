use std::sync::Arc;

use reqwest::Url;
use tokio::sync::{oneshot, Mutex};
use tokio::time::{timeout, Duration};

use crate::service::config::app_language::get_app_language_code;
use crate::service::mcp::auth::{
    map_auth_error, MCPRemoteOAuthSessionSnapshot, MCPRemoteOAuthStatus,
};
use crate::service::mcp::server::MCPServerType;
use crate::util::errors::{OpenBitFunError, OpenBitFunResult};
use openbitfun_services_integrations::mcp::auth::{
    clear_stored_oauth_credentials, has_stored_oauth_credentials,
    prepare_remote_oauth_authorization,
};

use super::auth_callback::build_oauth_callback_router;
use super::{ActiveRemoteOAuthSession, MCPServerManager};

const OAUTH_CALLBACK_TIMEOUT: Duration = Duration::from_secs(300);

impl MCPServerManager {
    fn oauth_data_dir(&self) -> OpenBitFunResult<std::path::PathBuf> {
        self.oauth_data_dir.clone().map(Ok).unwrap_or_else(|| {
            Ok(crate::infrastructure::try_get_path_manager_arc()?.user_data_dir())
        })
    }

    pub(super) async fn set_oauth_snapshot(
        session: &Arc<ActiveRemoteOAuthSession>,
        snapshot: MCPRemoteOAuthSessionSnapshot,
    ) {
        *session.snapshot.write().await = snapshot;
    }

    pub(super) async fn update_oauth_snapshot<F>(
        session: &Arc<ActiveRemoteOAuthSession>,
        update: F,
    ) -> MCPRemoteOAuthSessionSnapshot
    where
        F: FnOnce(&mut MCPRemoteOAuthSessionSnapshot),
    {
        let mut snapshot = session.snapshot.write().await;
        update(&mut snapshot);
        snapshot.clone()
    }

    pub(super) async fn insert_oauth_session(
        &self,
        server_id: &str,
        session: Arc<ActiveRemoteOAuthSession>,
    ) -> Option<Arc<ActiveRemoteOAuthSession>> {
        self.oauth_sessions
            .write()
            .await
            .insert(server_id.to_string(), session)
    }

    pub(super) async fn shutdown_oauth_session(session: &Arc<ActiveRemoteOAuthSession>) {
        if let Some(shutdown_tx) = session.shutdown_tx.lock().await.take() {
            let _ = shutdown_tx.send(());
        }
    }

    async fn fail_oauth_session(
        session: &Arc<ActiveRemoteOAuthSession>,
        message: String,
    ) -> MCPRemoteOAuthSessionSnapshot {
        let snapshot = MCPServerManager::update_oauth_snapshot(session, |snapshot| {
            snapshot.status = MCPRemoteOAuthStatus::Failed;
            snapshot.message = Some(message);
        })
        .await;
        Self::shutdown_oauth_session(session).await;
        snapshot
    }

    pub async fn start_remote_oauth_authorization(
        &self,
        server_id: &str,
    ) -> OpenBitFunResult<MCPRemoteOAuthSessionSnapshot> {
        let config = self
            .config_service
            .get_server_config(server_id)
            .await?
            .ok_or_else(|| {
                OpenBitFunError::NotFound(format!("MCP server config not found: {}", server_id))
            })?;

        if config.server_type != MCPServerType::Remote {
            return Err(OpenBitFunError::Validation(format!(
                "MCP server '{}' is not a remote server",
                server_id
            )));
        }

        if let Some(existing) = self.oauth_sessions.write().await.remove(server_id) {
            Self::shutdown_oauth_session(&existing).await;
        }

        let prepared = prepare_remote_oauth_authorization(self.oauth_data_dir()?, &config)
            .await
            .map_err(map_auth_error)?;
        let callback_path = Url::parse(&prepared.redirect_uri)
            .map_err(|error| {
                OpenBitFunError::MCPError(format!(
                    "Invalid OAuth redirect URI for server '{}': {}",
                    server_id, error
                ))
            })?
            .path()
            .to_string();

        let initial_snapshot = MCPRemoteOAuthSessionSnapshot::new(
            server_id.to_string(),
            MCPRemoteOAuthStatus::AwaitingBrowser,
            Some(prepared.authorization_url.clone()),
            Some(prepared.redirect_uri.clone()),
            Some("Open the authorization URL to continue OAuth sign-in.".to_string()),
        );

        let (callback_tx, callback_rx) = oneshot::channel();
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        let session = Arc::new(ActiveRemoteOAuthSession {
            snapshot: Arc::new(tokio::sync::RwLock::new(initial_snapshot.clone())),
            shutdown_tx: Mutex::new(Some(shutdown_tx)),
        });

        if let Some(previous) = self.insert_oauth_session(server_id, session.clone()).await {
            Self::shutdown_oauth_session(&previous).await;
        }

        let router =
            build_oauth_callback_router(&callback_path, callback_tx, get_app_language_code().await);
        let callback_server_session = session.clone();
        let callback_server_id = server_id.to_string();
        tokio::spawn(async move {
            let server =
                axum::serve(prepared.listener, router).with_graceful_shutdown(async move {
                    let _ = shutdown_rx.await;
                });

            if let Err(error) = server.await {
                let _ =
                    MCPServerManager::update_oauth_snapshot(&callback_server_session, |snapshot| {
                        if matches!(
                            snapshot.status,
                            MCPRemoteOAuthStatus::Authorized | MCPRemoteOAuthStatus::Cancelled
                        ) {
                            return;
                        }
                        snapshot.status = MCPRemoteOAuthStatus::Failed;
                        snapshot.message = Some(format!(
                            "OAuth callback listener failed for server '{}': {}",
                            callback_server_id, error
                        ));
                    })
                    .await;
            }
        });

        let manager = self.clone();
        let callback_session = session.clone();
        let callback_server_id = server_id.to_string();
        let authorization_url = prepared.authorization_url.clone();
        let redirect_uri = prepared.redirect_uri.clone();
        let mut oauth_state = prepared.state;
        tokio::spawn(async move {
            let _ = MCPServerManager::update_oauth_snapshot(&callback_session, |snapshot| {
                snapshot.status = MCPRemoteOAuthStatus::AwaitingCallback;
                snapshot.message = Some(
                    "Waiting for the OAuth provider to redirect back to OpenBitFun.".to_string(),
                );
            })
            .await;

            let callback = match timeout(OAUTH_CALLBACK_TIMEOUT, callback_rx).await {
                Ok(Ok(callback)) => callback,
                Ok(Err(_)) => {
                    let _ =
                        MCPServerManager::update_oauth_snapshot(&callback_session, |snapshot| {
                            snapshot.status = MCPRemoteOAuthStatus::Cancelled;
                            snapshot.message =
                                Some("OAuth authorization was cancelled.".to_string());
                        })
                        .await;
                    Self::shutdown_oauth_session(&callback_session).await;
                    return;
                }
                Err(_) => {
                    let _ = MCPServerManager::fail_oauth_session(
                        &callback_session,
                        "OAuth authorization timed out before the provider redirected back."
                            .to_string(),
                    )
                    .await;
                    return;
                }
            };

            if let Some(error) = callback.error {
                let description = callback
                    .error_description
                    .map(|value| format!(": {}", value))
                    .unwrap_or_default();
                let _ = MCPServerManager::fail_oauth_session(
                    &callback_session,
                    format!("OAuth provider returned '{}{}'", error, description),
                )
                .await;
                return;
            }

            let code = match callback.code {
                Some(code) => code,
                None => {
                    let _ = MCPServerManager::fail_oauth_session(
                        &callback_session,
                        "OAuth callback did not include an authorization code.".to_string(),
                    )
                    .await;
                    return;
                }
            };

            let state = match callback.state {
                Some(state) => state,
                None => {
                    let _ = MCPServerManager::fail_oauth_session(
                        &callback_session,
                        "OAuth callback did not include a state token.".to_string(),
                    )
                    .await;
                    return;
                }
            };

            let _ = MCPServerManager::update_oauth_snapshot(&callback_session, |snapshot| {
                snapshot.status = MCPRemoteOAuthStatus::ExchangingToken;
                snapshot.message =
                    Some("Exchanging the authorization code for an access token.".to_string());
            })
            .await;

            match oauth_state.handle_callback(&code, &state).await {
                Ok(_) => {
                    let _ = MCPServerManager::set_oauth_snapshot(
                        &callback_session,
                        MCPRemoteOAuthSessionSnapshot::new(
                            callback_server_id.clone(),
                            MCPRemoteOAuthStatus::Authorized,
                            Some(authorization_url.clone()),
                            Some(redirect_uri.clone()),
                            Some(
                                "OAuth authorization completed. Reconnecting MCP server."
                                    .to_string(),
                            ),
                        ),
                    )
                    .await;

                    if let Some(shutdown_tx) = callback_session.shutdown_tx.lock().await.take() {
                        let _ = shutdown_tx.send(());
                    }

                    manager.clear_reconnect_state(&callback_server_id).await;
                    let _ = manager.stop_server(&callback_server_id).await;
                    if let Err(error) = manager.start_server(&callback_server_id).await {
                        let _ = MCPServerManager::update_oauth_snapshot(
                            &callback_session,
                            |snapshot| {
                                snapshot.message = Some(format!(
                                    "OAuth token saved, but reconnect failed: {}",
                                    error
                                ));
                            },
                        )
                        .await;
                    }
                }
                Err(error) => {
                    let _ = MCPServerManager::fail_oauth_session(
                        &callback_session,
                        map_auth_error(error).to_string(),
                    )
                    .await;
                }
            }
        });

        Ok(initial_snapshot)
    }

    pub async fn get_remote_oauth_session(
        &self,
        server_id: &str,
    ) -> Option<MCPRemoteOAuthSessionSnapshot> {
        let session = self.oauth_sessions.read().await.get(server_id).cloned()?;
        let snapshot = session.snapshot.read().await.clone();
        Some(snapshot)
    }

    pub async fn has_remote_oauth_credentials(&self, server_id: &str) -> OpenBitFunResult<bool> {
        has_stored_oauth_credentials(self.oauth_data_dir()?, server_id)
            .await
            .map_err(map_auth_error)
    }

    pub async fn cancel_remote_oauth_authorization(&self, server_id: &str) -> OpenBitFunResult<()> {
        let session = self.oauth_sessions.write().await.remove(server_id);
        if let Some(session) = session {
            let _ = MCPServerManager::update_oauth_snapshot(&session, |snapshot| {
                snapshot.status = MCPRemoteOAuthStatus::Cancelled;
                snapshot.message = Some("OAuth authorization was cancelled.".to_string());
            })
            .await;
            Self::shutdown_oauth_session(&session).await;
        }
        Ok(())
    }

    pub async fn clear_remote_oauth_credentials(&self, server_id: &str) -> OpenBitFunResult<()> {
        self.cancel_remote_oauth_authorization(server_id).await?;
        clear_stored_oauth_credentials(self.oauth_data_dir()?, server_id)
            .await
            .map_err(map_auth_error)
    }
}
