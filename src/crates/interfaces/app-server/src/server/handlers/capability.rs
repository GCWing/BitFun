use agent_client_protocol::Error;
use bitfun_app_server_protocol::app::CapabilityAvailability;
use bitfun_app_server_protocol::error::{AppServerErrorData, AppServerErrorKind};
use bitfun_app_server_protocol::external_source::ExternalSourceErrorData;
use bitfun_product_domains::external_sources::{
    ExternalSourceOperationError, ExternalSourceOperationErrorCode,
};

use crate::management::{AppManagementError, AppManagementErrorKind, AppManagementService};

macro_rules! management_handler {
    ($management:ident, $capability:expr, $request:ty, $method:ident) => {{
        let management = $management.clone();
        async move |request: $request, responder, _cx| {
            let management = crate::server::handlers::capability::require_management(
                management.as_deref(),
                $capability,
            )?;
            responder.respond_with_result(management.$method(request).await.map_err(|error| {
                crate::server::handlers::capability::management_error($capability, error)
            }))
        }
    }};
}
pub(super) use management_handler;

pub(super) fn require_management<'a>(
    management: Option<&'a AppManagementService>,
    capability: &str,
) -> Result<&'a AppManagementService, Error> {
    let management = management.ok_or_else(|| unsupported(capability))?;
    match management.capabilities().availability(capability) {
        Some(CapabilityAvailability::Available) => Ok(management),
        Some(CapabilityAvailability::Unavailable { .. }) | None => Err(unsupported(capability)),
    }
}

fn unsupported(capability: &str) -> Error {
    error_with_data(
        AppServerErrorKind::Unsupported,
        capability,
        "The Host does not provide this management capability",
    )
}

pub(super) fn management_error(capability: &str, error: AppManagementError) -> Error {
    if let Some(external) = ExternalSourceOperationError::decode(&error.message) {
        return external_management_error(capability, external);
    }
    match error.kind {
        AppManagementErrorKind::Unsupported => {
            error_with_data(AppServerErrorKind::Unsupported, capability, error.message)
        }
        AppManagementErrorKind::InvalidRequest => Error::invalid_params().data(error.message),
        AppManagementErrorKind::NotFound => Error::resource_not_found(None).data(error.message),
        AppManagementErrorKind::Internal => Error::internal_error().data(error.message),
    }
}

fn external_management_error(capability: &str, error: ExternalSourceOperationError) -> Error {
    let kind = match error.code {
        ExternalSourceOperationErrorCode::StaleRevision => AppServerErrorKind::StaleRevision,
        ExternalSourceOperationErrorCode::HostCapabilityUnavailable
        | ExternalSourceOperationErrorCode::Unsupported => AppServerErrorKind::Unsupported,
        ExternalSourceOperationErrorCode::InvalidRequest => AppServerErrorKind::InvalidRequest,
        ExternalSourceOperationErrorCode::Timeout | ExternalSourceOperationErrorCode::Cancelled => {
            AppServerErrorKind::Internal
        }
        _ => AppServerErrorKind::Internal,
    };
    let app = AppServerErrorData {
        kind,
        retryable: error.retryable,
        outcome_unknown: matches!(kind, AppServerErrorKind::OutcomeUnknown),
        capability: Some(capability.to_string()),
        request_id: error.correlation_id.clone(),
    };
    Error::new(kind.json_rpc_code() as i32, error.detail.clone()).data(
        serde_json::to_value(ExternalSourceErrorData { app, error })
            .unwrap_or(serde_json::Value::Null),
    )
}

fn error_with_data(
    kind: AppServerErrorKind,
    capability: &str,
    message: impl Into<String>,
) -> Error {
    Error::new(kind.json_rpc_code() as i32, message.into()).data(
        serde_json::to_value(AppServerErrorData {
            kind,
            retryable: false,
            outcome_unknown: false,
            capability: Some(capability.to_string()),
            request_id: None,
        })
        .unwrap_or(serde_json::Value::Null),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::management::{EXTERNAL_SOURCES_CAPABILITY, MODELS_CAPABILITY};

    #[test]
    fn missing_management_service_returns_structured_unsupported_without_fallback() {
        let error = match require_management(None, MODELS_CAPABILITY) {
            Ok(_) => panic!("management service should be required"),
            Err(error) => error,
        };
        let data: AppServerErrorData = serde_json::from_value(
            error
                .data
                .expect("unsupported error should carry structured data"),
        )
        .expect("parse app server error data");

        assert_eq!(data.kind, AppServerErrorKind::Unsupported);
        assert_eq!(data.capability.as_deref(), Some(MODELS_CAPABILITY));
        assert!(!data.retryable);
        assert!(!data.outcome_unknown);
    }

    #[test]
    fn external_stale_error_preserves_domain_recovery_contract() {
        let domain = ExternalSourceOperationError::new(
            ExternalSourceOperationErrorCode::StaleRevision,
            "The external source catalog changed",
            false,
        )
        .with_correlation_id("external-source-ref-4")
        .with_default_recovery_actions();
        let error = external_management_error(EXTERNAL_SOURCES_CAPABILITY, domain.clone());
        let data: ExternalSourceErrorData = serde_json::from_value(
            error
                .data
                .expect("external source error should carry structured data"),
        )
        .expect("parse external source error data");

        assert_eq!(data.app.kind, AppServerErrorKind::StaleRevision);
        assert_eq!(data.app.retryable, domain.retryable);
        assert!(!data.app.outcome_unknown);
        assert_eq!(
            data.app.request_id.as_deref(),
            Some("external-source-ref-4")
        );
        assert_eq!(
            data.error.code,
            ExternalSourceOperationErrorCode::StaleRevision
        );
        assert_eq!(data.error.recovery_actions, domain.recovery_actions);
    }

    #[test]
    fn owner_timeout_is_not_misreported_as_unknown_transport_outcome() {
        let domain = ExternalSourceOperationError::new(
            ExternalSourceOperationErrorCode::Timeout,
            "External source discovery timed out",
            true,
        );
        let error = external_management_error(EXTERNAL_SOURCES_CAPABILITY, domain);
        let data: ExternalSourceErrorData = serde_json::from_value(
            error
                .data
                .expect("external source error should carry structured data"),
        )
        .expect("parse external source error data");

        assert_eq!(data.app.kind, AppServerErrorKind::Internal);
        assert!(data.app.retryable);
        assert!(!data.app.outcome_unknown);
        assert_eq!(data.error.code, ExternalSourceOperationErrorCode::Timeout);
    }
}
