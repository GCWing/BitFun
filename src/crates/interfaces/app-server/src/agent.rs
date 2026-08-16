//! BitFun agent runtime adapter for the generic app-server surface.
//!
//! This is the Phase 2 wiring point: [`BitfunAppRuntime`] holds an externally
//! assembled [`AgentRuntime`] (built by product assembly from `AgenticSystem`,
//! the same way `bitfun_acp::BitfunAcpRuntime` receives its runtime) and exposes
//! the agent kernel operations the JSON-RPC handlers delegate to. Like the ACP
//! runtime boundary, this crate does not own product assembly or the
//! compatibility facade; the host injects a ready runtime.

use std::sync::Arc;

use agent_client_protocol::{Error, Result};
use bitfun_agent_runtime::sdk::{AgentEventSource, AgentRuntime, PortErrorKind, RuntimeError};
use bitfun_core::service::git::GitError;
use bitfun_runtime_ports::{AgentContextReloadPort, ProductSearchPort};

/// Host-injected BitFun agent runtime exposed over the app-server surface.
///
/// Construct with [`BitfunAppRuntime::new`] from a product-assembled
/// `AgentRuntime` and the runtime's [`AgentEventSource`] (so the server can
/// forward runtime events to the client as `agent/event` notifications), then
/// pass a clone to [`crate::server::BitfunAppServer::new`].
#[derive(Clone)]
pub struct BitfunAppRuntime {
    runtime: Arc<AgentRuntime>,
    event_source: AgentEventSource,
    context_reload: Option<Arc<dyn AgentContextReloadPort>>,
    product_search: Option<Arc<dyn ProductSearchPort>>,
}

impl std::fmt::Debug for BitfunAppRuntime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BitfunAppRuntime").finish_non_exhaustive()
    }
}

impl BitfunAppRuntime {
    /// Wrap an externally assembled agent runtime together with the runtime
    /// event source the server uses to forward `agent/event` notifications.
    /// Both are shared by reference across handlers, so callers usually pass a
    /// freshly built `AgentRuntime` and the matching `AgentEventSource`
    /// (constructed from the same `EventQueue` the runtime publishes to).
    pub fn new(runtime: AgentRuntime, event_source: AgentEventSource) -> Self {
        Self {
            runtime: Arc::new(runtime),
            event_source,
            context_reload: None,
            product_search: None,
        }
    }

    pub fn with_context_reload(mut self, context_reload: Arc<dyn AgentContextReloadPort>) -> Self {
        self.context_reload = Some(context_reload);
        self
    }

    pub fn with_product_search(mut self, product_search: Arc<dyn ProductSearchPort>) -> Self {
        self.product_search = Some(product_search);
        self
    }

    /// Shared reference to the underlying agent runtime, for handlers that
    /// need to call SDK methods directly (for example subscribing to events).
    pub fn runtime(&self) -> &AgentRuntime {
        &self.runtime
    }

    /// Clone of the injected runtime event source. The server's event
    /// forwarder subscribes through this handle to drain runtime events and
    /// push them to the client as `agent/event` notifications over the
    /// transport. Keeping this on the server side means the client never
    /// subscribes to the runtime queue directly.
    pub fn event_source(&self) -> AgentEventSource {
        self.event_source.clone()
    }

    pub fn context_reload(&self) -> Option<&Arc<dyn AgentContextReloadPort>> {
        self.context_reload.as_ref()
    }

    pub fn product_search(&self) -> Option<&Arc<dyn ProductSearchPort>> {
        self.product_search.as_ref()
    }

    /// Map a `RuntimeError` to a JSON-RPC `Error`, mirroring the ACP runtime
    /// boundary: `PortErrorKind::NotFound` becomes `resource_not_found`,
    /// `InvalidRequest` becomes `invalid_params`, everything else stays
    /// `internal_error` with the message surfaced as data.
    pub fn runtime_error(error: RuntimeError) -> Error {
        match error {
            RuntimeError::Port(error) => match error.kind {
                PortErrorKind::InvalidRequest => Error::invalid_params().data(error.message),
                PortErrorKind::NotFound => Error::resource_not_found(None),
                _ => Self::internal_error(error.message),
            },
            other => Self::internal_error(other.into_message()),
        }
    }

    /// Map a `RuntimeError` that carries a session id, so the resource id is
    /// surfaced on the `resource_not_found` response.
    pub fn session_runtime_error(session_id: &str, error: RuntimeError) -> Error {
        match error {
            RuntimeError::Port(error) if error.kind == PortErrorKind::NotFound => {
                Error::resource_not_found(Some(session_id.to_string()))
            }
            other => Self::runtime_error(other),
        }
    }

    fn internal_error(message: impl std::fmt::Display) -> Error {
        Error::internal_error().data(serde_json::json!(message.to_string()))
    }
}

/// Convenience for wrapping a fallible runtime call into a JSON-RPC `Result`.
pub fn runtime_call<T>(result: std::result::Result<T, RuntimeError>) -> Result<T> {
    result.map_err(BitfunAppRuntime::runtime_error)
}

/// Map a `bitfun_core::service::git::GitError` onto a JSON-RPC `Error`. Mirrors
/// the runtime boundary's intent: argument/path problems surface as
/// `invalid_params`, everything else as `internal_error` with the message in
/// `data`. Used by the `git/*` schema handlers.
pub fn git_service_error(error: GitError) -> Error {
    match error {
        GitError::RepositoryNotFound(_)
        | GitError::InvalidPath(_)
        | GitError::BranchNotFound(_) => {
            Error::invalid_params().data(serde_json::json!(error.to_string()))
        }
        other => Error::internal_error().data(serde_json::json!(other.to_string())),
    }
}

/// Map a `bitfun_core::BitFunError` (the unified core error type) onto a
/// JSON-RPC `Error`. `BitFunError` already derives `Serialize`, so the whole
/// enum is attached as `data` (carrying the variant + message) for the client
/// to inspect; the human-readable `Display` form goes into the message. Used
/// by host-service handlers (`config/*`, and the upcoming `workspace/*`,
/// `snapshot/*` batches) that call into `bitfun-core` service singletons.
pub fn bitfun_error(error: bitfun_core::BitFunError) -> Error {
    Error::internal_error().data(serde_json::to_value(&error).unwrap_or(serde_json::Value::Null))
}

/// Map a `BitFunError` from a `config/getConfig` / `config/getConfigs` call
/// onto a JSON-RPC `Error`.
///
/// The frontend `ConfigAPI.getConfig` swallows the error and returns
/// `undefined` only when `error.message` (lowercased) contains the substrings
/// `not found:`, `config path`, and `'<path>'` -- it does not inspect `data`.
/// [`bitfun_error`] leaves `message` as the static `"Internal error"`, so the
/// substring match never hits on a path-not-found. This helper instead puts
/// the human-readable `BitFunError` Display text into the JSON-RPC `message`
/// for the `NotFound` case, mirroring the desktop host's
/// `"Failed to get config: Not found: Config path '<path>' not found"` shape
/// (`desktop/src/api/config_api.rs` + `BitFunError::NotFound` Display =
/// `Not found: {0}`) byte-for-byte, so the frontend substring match works in
/// web mode the same way it does on desktop. The structured `BitFunError` is
/// still attached as `data` for callers that inspect it. Other config errors
/// fall back to the generic [`bitfun_error`] shape.
pub fn config_get_error(error: bitfun_core::BitFunError) -> Error {
    match &error {
        bitfun_core::BitFunError::NotFound(_) => Error::new(
            agent_client_protocol::ErrorCode::InternalError.into(),
            format!("Failed to get config: {}", error),
        )
        .data(serde_json::to_value(&error).unwrap_or(serde_json::Value::Null)),
        _ => bitfun_error(error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The frontend `ConfigAPI.getConfig` swallows the error into `undefined`
    /// only when `error.message` (lowercased) contains `not found:`, `config
    /// path`, and `'<path>'`. Pin the exact substrings the desktop host
    /// produces so a future refactor of `config_get_error` cannot silently
    /// break the frontend swallow in web mode.
    #[test]
    fn config_get_error_not_found_message_matches_frontend_substrings() {
        let path = "ai.review_teams.default";
        let error = bitfun_core::BitFunError::NotFound(format!("Config path '{}' not found", path));
        let mapped = config_get_error(error);

        let message = mapped.message.to_lowercase();
        assert!(
            message.contains("not found:"),
            "message must contain 'not found:' for the frontend match, got: {mapped:?}"
        );
        assert!(
            message.contains("config path"),
            "message must contain 'config path' for the frontend match, got: {mapped:?}"
        );
        assert!(
            message.contains(&format!("'{}'", path)),
            "message must contain the quoted path for the frontend match, got: {mapped:?}"
        );
        // The structured enum still rides along in `data` for callers that
        // inspect it.
        assert!(
            mapped.data.is_some(),
            "data must carry the BitFunError enum"
        );
    }

    /// Non-`NotFound` config errors must fall back to the generic `bitfun_error`
    /// shape (`message = "Internal error"`, structured enum in `data`), so they
    /// are NOT swallowed by the frontend substring match -- they surface as
    /// real errors.
    #[test]
    fn config_get_error_non_not_found_falls_back_to_internal_error_message() {
        let error = bitfun_core::BitFunError::config("something else broke");
        let mapped = config_get_error(error);

        assert_eq!(
            mapped.message, "Internal error",
            "non-NotFound errors must keep the generic message, got: {mapped:?}"
        );
        assert!(mapped.data.is_some());
    }
}
