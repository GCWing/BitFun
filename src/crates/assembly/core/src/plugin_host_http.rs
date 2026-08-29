use async_trait::async_trait;
use bitfun_opencode_plugin_host::{
    BackendDiagnosticError, BackendDiagnosticEvent, BackendRouteFailure, BackendRouteRequest,
    OpenCodeBackendHandler, PluginHostBackendBridge, PluginHostClient,
};
use serde::de::DeserializeOwned;
use serde_json::{json, Value};
use std::sync::{Arc, OnceLock, RwLock};

static PLUGIN_HOST_BACKEND_BRIDGE: OnceLock<RwLock<Option<Arc<PluginHostBackendBridge>>>> =
    OnceLock::new();

struct CoreOpenCodeBackend;

#[async_trait]
impl OpenCodeBackendHandler for CoreOpenCodeBackend {
    async fn handle_route(
        &self,
        request: BackendRouteRequest,
    ) -> Result<Value, BackendRouteFailure> {
        let context = crate::plugin_host::plugin_host_instance_by_id(&request.instance_id)
            .await
            .ok_or_else(|| BackendRouteFailure::not_found("Plugin host instance was not found"))?;
        if !context.is_ready() {
            log::debug!(
                "Plugin client request admitted during activation: instance_id={}, operation={}",
                request.instance_id,
                request.route.operation()
            );
        }
        if let Some(directory) = request.query_first("directory") {
            validate_instance_directory(directory, &context.directory)?;
        }
        if let Some(directory) = request.headers.iter().find_map(|(name, value)| {
            name.eq_ignore_ascii_case("x-opencode-directory")
                .then_some(value.as_str())
        }) {
            validate_instance_directory(directory, &context.directory)?;
        }
        crate::plugin_host_http_routes::dispatch_route(
            &context,
            request.route,
            &request.query,
            &request.body,
        )
        .await
    }

    async fn publish_diagnostic(
        &self,
        event: BackendDiagnosticEvent,
    ) -> Result<(), BackendDiagnosticError> {
        crate::plugin_host::publish_plugin_host_diagnostic(event).await
    }
}

fn validate_instance_directory(
    requested: &str,
    owned: &std::path::Path,
) -> Result<(), BackendRouteFailure> {
    if crate::plugin_host::instance_directories_equal(requested, owned) {
        Ok(())
    } else {
        Err(BackendRouteFailure::forbidden(
            "Request directory does not belong to this plugin instance",
        ))
    }
}

pub(crate) async fn register_plugin_host_backend_handlers(
    client: PluginHostClient,
) -> crate::BitFunResult<Arc<PluginHostBackendBridge>> {
    let previous = PLUGIN_HOST_BACKEND_BRIDGE
        .get_or_init(|| RwLock::new(None))
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone();
    if let Some(previous) = previous {
        if !previous.begin_draining().await {
            return Err(crate::BitFunError::ProcessError(
                "Previous plugin Host backend requests did not stop before replacement".to_string(),
            ));
        }
    }
    let bridge = bitfun_opencode_plugin_host::register_backend_handlers(
        client,
        Arc::new(CoreOpenCodeBackend),
    )
    .await
    .map_err(|error| {
        crate::BitFunError::ProcessError(format!(
            "Failed to register plugin host backend handler: {error}"
        ))
    })?;
    *PLUGIN_HOST_BACKEND_BRIDGE
        .get_or_init(|| RwLock::new(None))
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(bridge.clone());
    Ok(bridge)
}

pub(crate) fn plugin_host_backend_bridge() -> Option<Arc<PluginHostBackendBridge>> {
    PLUGIN_HOST_BACKEND_BRIDGE
        .get()?
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone()
}

fn parse_body<T: DeserializeOwned>(body: &[u8]) -> Result<T, BackendRouteFailure> {
    if body.is_empty() {
        serde_json::from_value(json!({}))
            .map_err(|error| BackendRouteFailure::bad_request(error.to_string()))
    } else {
        serde_json::from_slice(body)
            .map_err(|error| BackendRouteFailure::bad_request(error.to_string()))
    }
}

pub(crate) fn body_as<T: DeserializeOwned>(body: &[u8]) -> Result<T, BackendRouteFailure> {
    parse_body(body)
}

pub(crate) type RouteResult = Result<Value, BackendRouteFailure>;
pub(crate) type Failure = BackendRouteFailure;
