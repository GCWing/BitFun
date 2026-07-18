use std::path::Path;

use agent_client_protocol::schema::{
    CurrentModeUpdate, ListSessionsRequest, ListSessionsResponse, LoadSessionRequest,
    LoadSessionResponse, NewSessionRequest, NewSessionResponse, SessionId, SessionInfo,
    SessionMode, SessionModeState, SessionUpdate, SetSessionModeRequest, SetSessionModeResponse,
};
use agent_client_protocol::{Client, ConnectionTo, Error, Result};
use bitfun_agent_runtime::sdk::{
    AgentSessionCreateRequest, AgentSessionListRequest, AgentSessionModeUpdateRequest,
    SessionStoragePathRequest,
};
use bitfun_core::agentic::agents::get_agent_registry;
use chrono::{DateTime, Utc};

use super::events::send_update;
use super::model::{
    build_session_config_options, build_session_model_state, normalize_session_model_id,
};
use super::replay::replay_session_history;
use super::{AcpSessionState, BitfunAcpRuntime};

impl BitfunAcpRuntime {
    pub(super) async fn create_session(
        &self,
        request: NewSessionRequest,
        connection: ConnectionTo<Client>,
    ) -> Result<NewSessionResponse> {
        let cwd = request.cwd.to_string_lossy().to_string();
        let mcp_servers = request.mcp_servers;
        let session = self
            .agent_runtime
            .create_session(AgentSessionCreateRequest {
                session_name: format!(
                    "ACP Session - {}",
                    chrono::Local::now().format("%Y-%m-%d %H:%M:%S")
                ),
                agent_type: "agentic".to_string(),
                workspace_path: Some(cwd.clone()),
                remote_connection_id: None,
                remote_ssh_host: None,
                metadata: serde_json::Map::new(),
            })
            .await
            .map_err(Self::runtime_error)?;

        let acp_session = AcpSessionState {
            acp_session_id: session.session_id.clone(),
            bitfun_session_id: session.session_id.clone(),
            cwd,
            mode_id: session.agent_type.clone(),
            model_id: normalize_session_model_id(None),
            mcp_server_ids: self
                .provision_mcp_servers(&session.session_id, mcp_servers)
                .await?,
        };
        self.sessions
            .insert(acp_session.acp_session_id.clone(), acp_session.clone());
        self.connections
            .insert(acp_session.acp_session_id.clone(), connection);

        let modes = build_session_modes(Some(session.agent_type.as_str())).await;
        let models = build_session_model_state(Some(&acp_session.model_id)).await?;
        let config_options =
            build_session_config_options(Some(&acp_session.model_id), Some(&acp_session.mode_id))
                .await?;
        Ok(
            NewSessionResponse::new(SessionId::new(acp_session.acp_session_id))
                .modes(modes)
                .models(models)
                .config_options(config_options),
        )
    }

    pub(super) async fn restore_session(
        &self,
        request: LoadSessionRequest,
        connection: ConnectionTo<Client>,
    ) -> Result<LoadSessionResponse> {
        let cwd = request.cwd.to_string_lossy().to_string();
        let session_id = request.session_id.to_string();
        let mcp_servers = request.mcp_servers;
        // ACP history replay and model selection must come from one persisted
        // snapshot. Keep this compatibility path until the runtime contract can
        // return the rich turn data ACP actually projects.
        let (session, turns) = self
            .compatibility
            .restore_session_with_turns_for_workspace(
                SessionStoragePathRequest {
                    workspace_path: Path::new(&cwd).to_path_buf(),
                    remote_connection_id: None,
                    remote_ssh_host: None,
                },
                &session_id,
                false,
            )
            .await
            .map_err(Self::internal_error)?;

        let acp_session = AcpSessionState {
            acp_session_id: session.session_id.clone(),
            bitfun_session_id: session.session_id.clone(),
            cwd,
            mode_id: session.agent_type.clone(),
            model_id: normalize_session_model_id(session.config.model_id.as_deref()),
            mcp_server_ids: self
                .provision_mcp_servers(&session.session_id, mcp_servers)
                .await?,
        };
        self.sessions
            .insert(acp_session.acp_session_id.clone(), acp_session.clone());
        self.connections
            .insert(acp_session.acp_session_id.clone(), connection.clone());

        // Replay the restored transcript so the client can rebuild its UI.
        // Failures to send individual notifications shouldn't abort the load;
        // the runtime context is already restored and the client can still
        // issue prompts against the session.
        if let Err(error) = replay_session_history(&connection, &acp_session.acp_session_id, &turns)
        {
            log::warn!(
                "ACP session history replay interrupted: session_id={}, error={}",
                acp_session.acp_session_id,
                error
            );
        }

        let modes = build_session_modes(Some(session.agent_type.as_str())).await;
        let models = build_session_model_state(Some(&acp_session.model_id)).await?;
        let config_options =
            build_session_config_options(Some(&acp_session.model_id), Some(&acp_session.mode_id))
                .await?;
        Ok(LoadSessionResponse::new()
            .modes(modes)
            .models(models)
            .config_options(config_options))
    }

    pub(super) async fn list_sessions_for_cwd(
        &self,
        request: ListSessionsRequest,
    ) -> Result<ListSessionsResponse> {
        let cwd = request
            .cwd
            .or_else(|| std::env::current_dir().ok())
            .ok_or_else(|| Error::invalid_params().data("cwd is required"))?;
        let cursor = request
            .cursor
            .as_deref()
            .and_then(|value| value.parse::<u128>().ok());

        let mut summaries = self
            .agent_runtime
            .list_sessions(AgentSessionListRequest {
                workspace_path: cwd.to_string_lossy().to_string(),
                remote_connection_id: None,
                remote_ssh_host: None,
            })
            .await
            .map_err(Self::runtime_error)?;
        summaries.sort_by_key(|summary| std::cmp::Reverse(summary.last_active_at_ms));

        let limit = 100usize;
        let filtered = summaries
            .into_iter()
            .filter(|summary| {
                cursor
                    .map(|cursor| u128::from(summary.last_active_at_ms) < cursor)
                    .unwrap_or(true)
            })
            .collect::<Vec<_>>();

        let sessions = filtered
            .iter()
            .take(limit)
            .map(|summary| {
                SessionInfo::new(
                    SessionId::new(summary.session_id.clone()),
                    Path::new(&cwd).to_path_buf(),
                )
                .title(summary.session_name.clone())
                .updated_at(unix_ms_to_rfc3339(summary.last_active_at_ms))
            })
            .collect::<Vec<_>>();

        let next_cursor = if filtered.len() > limit {
            filtered
                .get(limit - 1)
                .map(|summary| summary.last_active_at_ms.to_string())
        } else {
            None
        };

        Ok(ListSessionsResponse::new(sessions).next_cursor(next_cursor))
    }

    pub(super) async fn update_session_mode(
        &self,
        request: SetSessionModeRequest,
    ) -> Result<SetSessionModeResponse> {
        let mode_id = request.mode_id.to_string();
        self.update_session_mode_inner(&request.session_id.to_string(), &mode_id)
            .await?;

        Ok(SetSessionModeResponse::new())
    }

    pub(super) async fn update_session_mode_inner(
        &self,
        session_id: &str,
        mode_id: &str,
    ) -> Result<()> {
        let acp_session = self
            .sessions
            .get(session_id)
            .ok_or_else(|| Error::resource_not_found(Some(session_id.to_string())))?;
        let bitfun_session_id = acp_session.bitfun_session_id.clone();
        drop(acp_session);

        validate_mode_id(mode_id).await?;

        self.agent_runtime
            .update_session_mode(AgentSessionModeUpdateRequest {
                session_id: bitfun_session_id,
                mode_id: mode_id.to_string(),
            })
            .await
            .map_err(Self::runtime_error)?;

        if let Some(mut state) = self.sessions.get_mut(session_id) {
            state.mode_id = mode_id.to_string();
        }

        if let Some(connection) = self.connections.get(session_id) {
            send_update(
                &connection,
                session_id,
                SessionUpdate::CurrentModeUpdate(CurrentModeUpdate::new(mode_id.to_string())),
            )?;
        }

        Ok(())
    }
}

async fn build_session_modes(preferred_mode_id: Option<&str>) -> SessionModeState {
    let available_modes = get_agent_registry()
        .get_modes_info()
        .await
        .into_iter()
        .map(|info| SessionMode::new(info.id, info.name).description(info.description))
        .collect::<Vec<_>>();

    let current_mode_id = preferred_mode_id
        .and_then(|preferred| {
            available_modes
                .iter()
                .find(|mode| mode.id.to_string() == preferred)
                .map(|mode| mode.id.clone())
        })
        .or_else(|| {
            available_modes
                .iter()
                .find(|mode| mode.id.to_string() == "agentic")
                .or_else(|| available_modes.first())
                .map(|mode| mode.id.clone())
        })
        .unwrap_or_else(|| "agentic".into());

    SessionModeState::new(current_mode_id, available_modes)
}

async fn validate_mode_id(mode_id: &str) -> Result<()> {
    let mode_exists = get_agent_registry()
        .get_modes_info()
        .await
        .into_iter()
        .any(|info| info.id == mode_id);

    if mode_exists {
        Ok(())
    } else {
        Err(Error::invalid_params().data(format!("unknown session mode: {}", mode_id)))
    }
}

fn unix_ms_to_rfc3339(time_ms: u64) -> String {
    let time_ms = i64::try_from(time_ms).unwrap_or(i64::MAX);
    DateTime::<Utc>::from_timestamp_millis(time_ms)
        .unwrap_or(DateTime::<Utc>::UNIX_EPOCH)
        .to_rfc3339()
}

#[cfg(test)]
mod tests {
    use super::{unix_ms_to_rfc3339, validate_mode_id};

    #[test]
    fn session_timestamps_remain_rfc3339_after_runtime_projection() {
        assert_eq!(unix_ms_to_rfc3339(0), "1970-01-01T00:00:00+00:00");
        assert_eq!(
            unix_ms_to_rfc3339(1_700_000_000_000),
            "2023-11-14T22:13:20+00:00"
        );
    }

    #[tokio::test]
    async fn unknown_session_mode_remains_an_invalid_request() {
        assert!(validate_mode_id("__missing_acp_mode__").await.is_err());
    }
}
