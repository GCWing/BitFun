use agent_client_protocol::Error;
use bitfun_app_server_protocol::app::CapabilityAvailability;
use bitfun_app_server_protocol::error::{AppServerErrorData, AppServerErrorKind};

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
    match error.kind {
        AppManagementErrorKind::Unsupported => {
            error_with_data(AppServerErrorKind::Unsupported, capability, error.message)
        }
        AppManagementErrorKind::InvalidRequest => Error::invalid_params().data(error.message),
        AppManagementErrorKind::NotFound => Error::resource_not_found(None).data(error.message),
        AppManagementErrorKind::Internal => Error::internal_error().data(error.message),
    }
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
    use crate::management::MODELS_CAPABILITY;

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
}
