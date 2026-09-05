//! Desktop host integration for the app-wide sleep-prevention preference.
//!
//! The preference is intentionally independent of agent/session activity. While
//! enabled, the inhibitor lives for the desktop process and is released when
//! the preference is disabled or the process exits.

use std::sync::{mpsc, Arc};

use openbitfun_core::service::config::{
    subscribe_config_updates, ConfigService, ConfigUpdateEvent,
};
use serde::Deserialize;
use tauri::{AppHandle, Manager, State};

use crate::api::app_state::AppState;

const PREVENT_SLEEP_CONFIG_PATH: &str = "app.prevent_sleep";
/// Ceiling for one worker round-trip. Acquiring an OS inhibitor is a local call
/// on every platform; anything slower than this is a stuck session manager, not
/// a slow one.
const WORKER_REQUEST_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

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

        // Bounded on purpose. The worker's `keepawake` call is a blocking D-Bus
        // round-trip to logind on Linux, with no timeout of its own; a hung or
        // slow session manager would otherwise hold this mutex forever and wedge
        // every later toggle, including the one that turns the feature off.
        let outcome = match tokio::time::timeout(WORKER_REQUEST_TIMEOUT, result).await {
            Ok(received) => received
                .map_err(|_| "Sleep-prevention worker stopped unexpectedly".to_string())
                .and_then(|outcome| outcome),
            Err(_) => {
                // The thread is still stuck inside the OS call, so it cannot be
                // reused; dropping the sender retires it once that call returns.
                Err(format!(
                    "Sleep-prevention request timed out after {} seconds",
                    WORKER_REQUEST_TIMEOUT.as_secs()
                ))
            }
        };

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
        .name("openbitfun-sleep-prevention".to_string())
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
                .reason("Prevent sleep is enabled in OpenBitFun")
                .app_name("OpenBitFun")
                .app_reverse_domain("com.openbitfun.desktop")
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

/// Events that can change `app.prevent_sleep`.
///
/// `ConfigReloaded` covers a config import or an external edit: the preference
/// can flip without any command running, and without re-reading it the runtime
/// state would silently disagree with the saved one until the next restart.
fn config_event_requires_sync(event: &ConfigUpdateEvent) -> bool {
    matches!(
        event,
        ConfigUpdateEvent::AppUpdated | ConfigUpdateEvent::ConfigReloaded
    )
}

/// Apply the new runtime state, then persist it, undoing the runtime change if
/// persisting fails.
///
/// Order matters: applying first means a rejected inhibitor never reaches disk,
/// and rolling back means a failed save never leaves the running app in a state
/// the config does not describe. Written against injected closures so the
/// rollback branch can be tested without a failing filesystem.
async fn apply_then_persist<Apply, ApplyFut, Persist, PersistFut>(
    previous: bool,
    enabled: bool,
    apply: Apply,
    persist: Persist,
) -> Result<(), String>
where
    Apply: Fn(bool) -> ApplyFut,
    ApplyFut: std::future::Future<Output = Result<(), String>>,
    Persist: FnOnce() -> PersistFut,
    PersistFut: std::future::Future<Output = Result<(), String>>,
{
    apply(enabled).await?;

    let Err(error) = persist().await else {
        return Ok(());
    };
    if let Err(rollback_error) = apply(previous).await {
        return Err(format!(
            "Failed to save prevent-sleep preference: {}; runtime rollback also failed: {}",
            error, rollback_error
        ));
    }
    Err(format!(
        "Failed to save prevent-sleep preference: {}",
        error
    ))
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
                Ok(event) if config_event_requires_sync(&event) => {
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

/// Reads the preference. Deliberately free of side effects: the runtime state is
/// owned by [`spawn_config_listener`], which applies it at startup and on every
/// config update. Re-applying it from a getter meant reading the setting could
/// start a thread and take an OS inhibitor — and, on Linux, block on D-Bus.
#[tauri::command]
pub async fn get_prevent_sleep_enabled(
    app_state: State<'_, AppState>,
    sleep_prevention: State<'_, SleepPreventionState>,
    request: GetPreventSleepEnabledRequest,
) -> Result<bool, String> {
    let _ = request;
    let _ = sleep_prevention;
    configured_enabled(&app_state.config_service).await
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetPreventSleepEnabledRequest {
    pub enabled: bool,
}

#[tauri::command]
pub async fn set_prevent_sleep_enabled(
    app: AppHandle,
    request: SetPreventSleepEnabledRequest,
) -> Result<(), String> {
    crate::openbitfun_control_host::configure_option_from_gui(
        &app,
        "setting.application.general",
        "prevent-sleep",
        serde_json::Value::Bool(request.enabled),
    )
    .await
    .map(|_| ())
}

pub(crate) async fn set_prevent_sleep_enabled_from_host(
    app: &AppHandle,
    enabled: bool,
) -> Result<(), String> {
    let app_state = app.state::<AppState>();
    let sleep_prevention = app.state::<SleepPreventionState>();
    set_prevent_sleep_enabled_impl(&app_state.config_service, sleep_prevention.inner(), enabled)
        .await
}

async fn set_prevent_sleep_enabled_impl(
    config_service: &ConfigService,
    sleep_prevention: &SleepPreventionState,
    enabled: bool,
) -> Result<(), String> {
    let previous = configured_enabled(config_service).await?;
    apply_then_persist(
        previous,
        enabled,
        |enabled| sleep_prevention.set_enabled(enabled),
        || async {
            config_service
                .set_config(PREVENT_SLEEP_CONFIG_PATH, enabled)
                .await
                .map_err(|error| error.to_string())
        },
    )
    .await?;

    crate::api::remote_connect_api::notify_settings_changed();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        apply_then_persist, config_event_requires_sync, start_worker, SleepPreventionState,
    };
    use openbitfun_core::service::config::ConfigUpdateEvent;
    use std::sync::{Arc, Mutex};

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

    #[tokio::test]
    async fn a_failed_save_puts_the_runtime_state_back() {
        let applied = Arc::new(Mutex::new(Vec::new()));
        let recorder = Arc::clone(&applied);

        let error = apply_then_persist(
            false,
            true,
            move |enabled| {
                let recorder = Arc::clone(&recorder);
                async move {
                    recorder.lock().unwrap().push(enabled);
                    Ok(())
                }
            },
            || async { Err("disk is full".to_string()) },
        )
        .await
        .expect_err("a failed save must surface as an error");

        assert!(error.contains("disk is full"), "unexpected error: {error}");
        assert!(
            !error.contains("rollback also failed"),
            "rollback succeeded, so it must not be reported as failed: {error}"
        );
        assert_eq!(
            *applied.lock().unwrap(),
            vec![true, false],
            "the new state must be applied, then reverted to the previous one"
        );
    }

    #[tokio::test]
    async fn a_failed_save_and_a_failed_rollback_report_both() {
        let error = apply_then_persist(
            false,
            true,
            |enabled| async move {
                if enabled {
                    Ok(())
                } else {
                    Err("inhibitor stuck".to_string())
                }
            },
            || async { Err("disk is full".to_string()) },
        )
        .await
        .expect_err("a failed save must surface as an error");

        assert!(error.contains("disk is full"), "unexpected error: {error}");
        assert!(
            error.contains("inhibitor stuck"),
            "a failed rollback must be reported too: {error}"
        );
    }

    #[tokio::test]
    async fn a_successful_save_does_not_roll_back() {
        let applied = Arc::new(Mutex::new(Vec::new()));
        let recorder = Arc::clone(&applied);

        apply_then_persist(
            false,
            true,
            move |enabled| {
                let recorder = Arc::clone(&recorder);
                async move {
                    recorder.lock().unwrap().push(enabled);
                    Ok(())
                }
            },
            || async { Ok(()) },
        )
        .await
        .expect("saving succeeded");

        assert_eq!(*applied.lock().unwrap(), vec![true]);
    }

    #[test]
    fn config_reload_re_reads_the_preference() {
        assert!(config_event_requires_sync(
            &ConfigUpdateEvent::ConfigReloaded
        ));
        assert!(config_event_requires_sync(&ConfigUpdateEvent::AppUpdated));
        assert!(!config_event_requires_sync(
            &ConfigUpdateEvent::ModelConfigurationUpdated
        ));
    }
}
