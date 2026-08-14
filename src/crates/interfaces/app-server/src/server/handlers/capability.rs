use agent_client_protocol::Error;
use bitfun_app_server_protocol::error::{AppServerErrorData, AppServerErrorKind};

macro_rules! unsupported_management_handler {
    ($capability:expr, $request:ty) => {{
        async move |_: $request, responder, _cx| {
            responder.respond_with_result(Err(crate::server::handlers::capability::unsupported(
                $capability,
            )))
        }
    }};
}
pub(super) use unsupported_management_handler;

pub(super) fn unsupported(capability: &str) -> Error {
    Error::new(
        AppServerErrorKind::Unsupported.json_rpc_code() as i32,
        "The Host does not provide this management capability",
    )
    .data(
        serde_json::to_value(AppServerErrorData {
            kind: AppServerErrorKind::Unsupported,
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
    fn unavailable_management_method_returns_structured_error_without_fallback() {
        let error = unsupported(MODELS_CAPABILITY);
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
