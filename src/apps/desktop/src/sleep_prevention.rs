//! Desktop host integration for the app-wide sleep-prevention preference.
//!
//! The preference is intentionally independent of agent/session activity. While
//! enabled, the inhibitor lives for the desktop process and is released when
//! the preference is disabled or the process exits.

use std::sync::{mpsc, Arc};

use bitfun_core::service::config::{subscribe_config_updates, ConfigService, ConfigUpdateEvent};
use serde::Deserialize;
use tauri::{AppHandle, Manager, State};

use crate::api::app_state::AppState;

const PREVENT_SLEEP_CONFIG_PATH: &str = "app.prevent_sleep";

enum SleepPreventionRequest {
    SetEnabled {
        enabled: bool,
        response: tokio::sync::oneshot::Sender<Result<(), String>>,
    },
}

#[derive(Clone)]
pub struct SleepPreventionState {
    worker: Arc<tokio::sync::Mutex<Option<mpsc::Sender<SleepPreventionRequest>>>>,
}

impl SleepPreventionState {
    async fn set_enabled(&self, enabled: bool) -> Result<(), String> {
        let mut worker = self.worker.lock().await;
        if !enabled && worker.is_none() {
            return Ok(());
        }

        if worker.is_none() {
            *worker = Some(start_worker()?);
        }

        let (response, result) = tokio::sync::oneshot::channel();
        let send_result = worker
            .as_ref()
            .ok_or_else(|| "Sleep-prevention worker failed to initialize".to_string())?
            .send(SleepPreventionRequest::SetEnabled { enabled, response });
        if send_result.is_err() {
            worker.take();
            return Err("Sleep-prevention worker is unavailable".to_string());
        }

        let outcome = result
            .await
            .map_err(|_| "Sleep-prevention worker stopped unexpectedly".to_string())
            .and_then(|outcome| outcome);

        // An inactive or failed worker owns no useful resources. Dropping the
        // final sender lets the thread exit instead of keeping one alive for
        // the entire application lifetime while the preference is off.
        if !enabled || outcome.is_err() {
            worker.take();
        }

        outcome
    }
}

impl Default for SleepPreventionState {
    fn default() -> Self {
        Self {
            worker: Arc::new(tokio::sync::Mutex::new(None)),
        }
    }
}

fn start_worker() -> Result<mpsc::Sender<SleepPreventionRequest>, String> {
    let (sender, receiver) = mpsc::channel();
    std::thread::Builder::new()
        .name("bitfun-sleep-prevention".to_string())
        .spawn(move || run_worker(receiver))
        .map_err(|error| format!("Failed to start sleep-prevention worker: {}", error))?;
    Ok(sender)
}

fn run_worker(receiver: mpsc::Receiver<SleepPreventionRequest>) {
    let mut inhibitor = None;

    while let Ok(request) = receiver.recv() {
        match request {
            SleepPreventionRequest::SetEnabled { enabled, response } => {
                let _ = response.send(set_inhibitor_enabled(&mut inhibitor, enabled));
            }
        }
    }
}

fn set_inhibitor_enabled(
    inhibitor: &mut Option<keepawake::KeepAwake>,
    enabled: bool,
) -> Result<(), String> {
    if enabled {
        if inhibitor.is_none() {
            let guard = keepawake::Builder::default()
                .idle(true)
                .reason("Prevent sleep is enabled in BitFun")
                .app_name("BitFun")
                .app_reverse_domain("com.bitfun.desktop")
                .create()
                .map_err(|error| {
                    let message = format!("Failed to prevent system sleep: {}", error);
                    #[cfg(target_os = "linux")]
                    {
                        format!(
                            "{}. Linux sleep prevention requires the system D-Bus and \
                             org.freedesktop.login1",
                            message
                        )
                    }
                    #[cfg(not(target_os = "linux"))]
                    {
                        message
                    }
                })?;
            *inhibitor = Some(guard);
            log::info!("System sleep prevention enabled");
        }
    } else if inhibitor.take().is_some() {
        log::info!("System sleep prevention disabled");
    }

    Ok(())
}

async fn configured_enabled(config_service: &ConfigService) -> Result<bool, String> {
    config_service
        .get_config::<bool>(Some(PREVENT_SLEEP_CONFIG_PATH))
        .await
        .map_err(|error| format!("Failed to read prevent-sleep preference: {}", error))
}

async fn sync_from_config(config_service: &ConfigService, sleep_prevention: &SleepPreventionState) {
    match configured_enabled(config_service).await {
        Ok(enabled) => {
            if let Err(error) = sleep_prevention.set_enabled(enabled).await {
                log::warn!(
                    "Failed to apply prevent-sleep preference: enabled={}, error={}",
                    enabled,
                    error
                );
            }
        }
        Err(error) => {
            log::warn!("{}", error);
        }
    }
}

/// Applies the saved preference at startup and after config imports/reloads.
pub fn spawn_config_listener(app: AppHandle) {
    let app_state: State<'_, AppState> = app.state();
    let config_service = app_state.config_service.clone();
    let sleep_prevention = app.state::<SleepPreventionState>().inner().clone();

    tokio::spawn(async move {
        let Some(mut receiver) = subscribe_config_updates() else {
            log::warn!("Config update subscription unavailable for sleep-prevention listener");
            sync_from_config(&config_service, &sleep_prevention).await;
            return;
        };

        sync_from_config(&config_service, &sleep_prevention).await;

        loop {
            match receiver.recv().await {
                Ok(ConfigUpdateEvent::AppUpdated) | Ok(ConfigUpdateEvent::ConfigReloaded) => {
                    sync_from_config(&config_service, &sleep_prevention).await;
                }
                Ok(_) => {}
                Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                    log::warn!("Sleep-prevention config listener channel closed");
                    break;
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(count)) => {
                    log::warn!(
                        "Sleep-prevention config listener lagged by {} messages",
                        count
                    );
                    sync_from_config(&config_service, &sleep_prevention).await;
                }
            }
        }
    });
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct GetPreventSleepEnabledRequest {}

#[tauri::command]
pub async fn get_prevent_sleep_enabled(
    app_state: State<'_, AppState>,
    sleep_prevention: State<'_, SleepPreventionState>,
    request: GetPreventSleepEnabledRequest,
) -> Result<bool, String> {
    let _ = request;
    let enabled = configured_enabled(&app_state.config_service).await?;
    sleep_prevention.set_enabled(enabled).await?;
    Ok(enabled)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetPreventSleepEnabledRequest {
    pub enabled: bool,
}

#[tauri::command]
pub async fn set_prevent_sleep_enabled(
    app_state: State<'_, AppState>,
    sleep_prevention: State<'_, SleepPreventionState>,
    request: SetPreventSleepEnabledRequest,
) -> Result<(), String> {
    let previous = configured_enabled(&app_state.config_service).await?;
    sleep_prevention.set_enabled(request.enabled).await?;

    if let Err(error) = app_state
        .config_service
        .set_config(PREVENT_SLEEP_CONFIG_PATH, request.enabled)
        .await
    {
        if let Err(rollback_error) = sleep_prevention.set_enabled(previous).await {
            return Err(format!(
                "Failed to save prevent-sleep preference: {}; runtime rollback also failed: {}",
                error, rollback_error
            ));
        }
        return Err(format!(
            "Failed to save prevent-sleep preference: {}",
            error
        ));
    }

    crate::api::remote_connect_api::notify_settings_changed();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{start_worker, SleepPreventionState};

    #[tokio::test]
    async fn inactive_state_does_not_start_a_worker() {
        let state = SleepPreventionState::default();
        assert!(state.worker.lock().await.is_none());

        state.set_enabled(false).await.unwrap();
        state.set_enabled(false).await.unwrap();

        assert!(state.worker.lock().await.is_none());
    }

    #[tokio::test]
    async fn disabling_stops_an_existing_worker() {
        let state = SleepPreventionState::default();
        *state.worker.lock().await = Some(start_worker().unwrap());

        state.set_enabled(false).await.unwrap();

        assert!(state.worker.lock().await.is_none());
    }
}
