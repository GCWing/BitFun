use bitfun_product_domains::privacy::{
    AcceptPrivacyRequest, ApplyPrivacyCollectionPolicyRequest, EnterPrivacyNotAcceptedRequest,
    GetPrivacyStatusRequest, InitializePrivacyRequest, MarkPrivacyViewedRequest,
    PrivacyEffectiveMode, PrivacyError, PrivacyStatus,
};
use bitfun_services_integrations::privacy::{PrivacyCollectionPolicy, PrivacyService};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::OnceLock;
use tauri::State;
use tokio::sync::Mutex;

pub struct PrivacyServiceState {
    service: Option<Mutex<PrivacyService>>,
    collection_policy: Arc<PrivacyCollectionPolicy>,
}

static COLLECTION_POLICY: OnceLock<Arc<PrivacyCollectionPolicy>> = OnceLock::new();
fn shared_collection_policy(initially_allowed: bool) -> Arc<PrivacyCollectionPolicy> {
    COLLECTION_POLICY
        .get_or_init(|| Arc::new(PrivacyCollectionPolicy::new(initially_allowed)))
        .clone()
}

impl PrivacyServiceState {
    pub fn enabled(storage_dir: PathBuf, locale: &str) -> Self {
        Self {
            service: Some(Mutex::new(PrivacyService::new(storage_dir, locale))),
            collection_policy: shared_collection_policy(false),
        }
    }

    pub fn disabled() -> Self {
        Self {
            service: None,
            collection_policy: shared_collection_policy(true),
        }
    }

    pub fn collection_allowed(&self) -> bool {
        self.collection_policy.collection_allowed()
    }

    fn enter_full_mode(&self) -> Result<(), PrivacyError> {
        self.collection_policy.apply(PrivacyEffectiveMode::Full)
    }

    fn enter_not_accepted_mode(&self) -> Result<(), PrivacyError> {
        self.collection_policy
            .apply(PrivacyEffectiveMode::PrivacyNotAccepted)
    }

    async fn with_service<T>(
        &self,
        operation: impl for<'a> FnOnce(
            &'a PrivacyService,
        ) -> std::pin::Pin<
            Box<dyn std::future::Future<Output = Result<T, PrivacyError>> + Send + 'a>,
        >,
    ) -> Result<T, PrivacyError> {
        let service = self.service.as_ref().ok_or_else(|| {
            PrivacyError::new(
                "PRIVACY_SERVICE_UNAVAILABLE",
                "Privacy service is unavailable",
            )
        })?;
        let guard = service.lock().await;
        operation(&guard).await
    }

    async fn status_with_effective_mode(
        &self,
        mut status: PrivacyStatus,
    ) -> Result<PrivacyStatus, PrivacyError> {
        status.effective_mode = self.collection_policy.effective_mode();
        if status.lifecycle_state == bitfun_product_domains::privacy::PrivacyLifecycleState::Full
            && status.effective_mode != PrivacyEffectiveMode::Full
        {
            status.configuration_error = Some("PRIVACY_POLICY_NOT_APPLIED".to_string());
        }
        Ok(status)
    }

    async fn initialize(&self, app_version: &str) -> Result<PrivacyStatus, PrivacyError> {
        let Some(service) = self.service.as_ref() else {
            return Ok(PrivacyStatus::disabled());
        };
        let status = service.lock().await.initialize(app_version).await?;
        let application = if status.effective_mode == PrivacyEffectiveMode::Full {
            self.enter_full_mode()
        } else {
            self.enter_not_accepted_mode()
        };
        if let Err(error) = application {
            let mut failed_status = status;
            failed_status.effective_mode = PrivacyEffectiveMode::PrivacyNotAccepted;
            failed_status.configuration_error = Some(error.code);
            return Ok(failed_status);
        }
        self.status_with_effective_mode(status).await
    }
}

#[tauri::command]
pub async fn privacy_initialize(
    state: State<'_, PrivacyServiceState>,
    app: tauri::AppHandle,
    _request: InitializePrivacyRequest,
) -> Result<PrivacyStatus, PrivacyError> {
    state
        .initialize(&app.package_info().version.to_string())
        .await
}

#[tauri::command]
pub async fn privacy_get_status(
    state: State<'_, PrivacyServiceState>,
    app: tauri::AppHandle,
    request: GetPrivacyStatusRequest,
) -> Result<PrivacyStatus, PrivacyError> {
    if state.service.is_none() {
        return Ok(PrivacyStatus::disabled());
    }
    let app_version = app.package_info().version.to_string();
    let status = state
        .with_service(|service| {
            Box::pin(async move { service.status(&request.locale, &app_version).await })
        })
        .await?;
    state.status_with_effective_mode(status).await
}

#[tauri::command]
pub async fn privacy_accept(
    state: State<'_, PrivacyServiceState>,
    app: tauri::AppHandle,
    request: AcceptPrivacyRequest,
) -> Result<PrivacyStatus, PrivacyError> {
    if state.service.is_none() {
        return Ok(PrivacyStatus::disabled());
    }
    let app_version = app.package_info().version.to_string();
    let status = state
        .with_service(|service| {
            Box::pin(async move { service.accept(&request, &app_version).await })
        })
        .await?;
    state.enter_full_mode()?;
    state.status_with_effective_mode(status).await
}

#[tauri::command]
pub async fn privacy_enter_not_accepted(
    state: State<'_, PrivacyServiceState>,
    app: tauri::AppHandle,
    request: EnterPrivacyNotAcceptedRequest,
) -> Result<PrivacyStatus, PrivacyError> {
    if state.service.is_none() {
        return Ok(PrivacyStatus::disabled());
    }
    state.enter_not_accepted_mode()?;
    let app_version = app.package_info().version.to_string();
    let status = state
        .with_service(|service| {
            Box::pin(async move {
                service
                    .enter_not_accepted(&request.locale, &app_version)
                    .await
            })
        })
        .await?;
    state.status_with_effective_mode(status).await
}

#[tauri::command]
pub async fn privacy_mark_viewed(
    state: State<'_, PrivacyServiceState>,
    app: tauri::AppHandle,
    request: MarkPrivacyViewedRequest,
) -> Result<PrivacyStatus, PrivacyError> {
    if state.service.is_none() {
        return Ok(PrivacyStatus::disabled());
    }
    let app_version = app.package_info().version.to_string();
    let status = state
        .with_service(|service| {
            Box::pin(async move {
                service
                    .mark_viewed(&request.policy_updated_at, &app_version, &request.locale)
                    .await
            })
        })
        .await?;
    state.status_with_effective_mode(status).await
}

#[tauri::command]
pub async fn privacy_apply_collection_policy(
    state: State<'_, PrivacyServiceState>,
    app: tauri::AppHandle,
    request: ApplyPrivacyCollectionPolicyRequest,
) -> Result<PrivacyStatus, PrivacyError> {
    if state.service.is_none() {
        return Ok(PrivacyStatus::disabled());
    }
    if request.mode == PrivacyEffectiveMode::Full {
        let full_is_valid = state
            .with_service(|service| Box::pin(async move { service.can_apply_full_mode().await }))
            .await?;
        if !full_is_valid {
            return Err(PrivacyError::new(
                "PRIVACY_CONSENT_REQUIRED",
                "Full collection mode requires a valid saved consent",
            ));
        }
    }
    if request.mode == PrivacyEffectiveMode::Full {
        state.enter_full_mode()?;
    } else {
        state.enter_not_accepted_mode()?;
    }
    let app_version = app.package_info().version.to_string();
    let status = state
        .with_service(|service| {
            Box::pin(async move { service.status(&request.locale, &app_version).await })
        })
        .await?;
    state.status_with_effective_mode(status).await
}
