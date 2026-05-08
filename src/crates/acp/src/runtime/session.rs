use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use agent_client_protocol::schema::{
    CurrentModeUpdate, ListSessionsRequest, ListSessionsResponse, LoadSessionRequest,
    LoadSessionResponse, NewSessionRequest, NewSessionResponse, SessionId, SessionInfo,
    SessionMode, SessionModeState, SessionUpdate, SetSessionModeRequest, SetSessionModeResponse,
};
use agent_client_protocol::{Client, ConnectionTo, Error, Result};
use bitfun_core::agentic::agents::get_agent_registry;
use bitfun_core::agentic::core::SessionConfig;
use chrono::{DateTime, Utc};

use super::events::send_update;
use super::model::{
    build_session_config_options, build_session_model_state, normalize_session_model_id,
};
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
            .agentic_system
            .coordinator
            .create_session(
                format!(
                    "ACP Session - {}",
                    chrono::Local::now().format("%Y-%m-%d %H:%M:%S")
                ),
                "agentic".to_string(),
                SessionConfig {
                    workspace_path: Some(cwd.clone()),
                    ..Default::default()
                },
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
        let session = self
            .agentic_system
            .coordinator
            .restore_session(Path::new(&cwd), &session_id)
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
            .insert(acp_session.acp_session_id.clone(), connection);

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
            .agentic_system
            .coordinator
            .list_sessions(&cwd)
            .await
            .map_err(Self::internal_error)?;
        summaries.sort_by(|a, b| b.last_activity_at.cmp(&a.last_activity_at));

        let limit = 100usize;
        let filtered = summaries
            .into_iter()
            .filter(|summary| {
                cursor
                    .map(|cursor| system_time_to_unix_ms(summary.last_activity_at) < cursor)
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
                .updated_at(system_time_to_rfc3339(summary.last_activity_at))
            })
            .collect::<Vec<_>>();

        let next_cursor = if filtered.len() > limit {
            filtered
                .get(limit - 1)
                .map(|summary| system_time_to_unix_ms(summary.last_activity_at).to_string())
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

        self.agentic_system
            .coordinator
            .update_session_agent_type(&bitfun_session_id, mode_id)
            .await
            .map_err(Self::internal_error)?;

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
        .filter(|info| info.enabled)
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
        .any(|info| info.enabled && info.id == mode_id);

    if mode_exists {
        Ok(())
    } else {
        Err(Error::invalid_params().data(format!("unknown session mode: {}", mode_id)))
    }
}

fn system_time_to_unix_ms(time: SystemTime) -> u128 {
    time.duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default()
}

fn system_time_to_rfc3339(time: SystemTime) -> String {
    DateTime::<Utc>::from(time).to_rfc3339()
}
