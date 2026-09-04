//! Host-injected connection policy: identity, canonical workspace scope,
//! transport limits, and the explicit method allowlist enforced before any
//! request reaches a domain handler.
//!
//! The policy carries no product behavior: the Host constructs it from its own
//! scope and limits, and the server only enforces it fail-closed. Hosts that
//! do not inject a policy keep the pre-existing open surface.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use agent_client_protocol::{Builder, ConnectionTo, Dispatch, Error, HandleDispatchFrom, Handled};
use openbitfun_app_server_protocol::error::{AppServerErrorData, AppServerErrorKind};
use serde_json::Value;

use crate::role::{AppClient, AppServer};

/// Default transport limits advertised by hosts that do not inject their own.
pub const DEFAULT_MAX_FRAME_BYTES: u64 = 16 * 1024 * 1024;
pub const DEFAULT_EVENT_BUFFER_CAPACITY: u32 = 1024;

/// Host-transport disconnect signal shared by the Host reader and the server
/// event forwarder.
///
/// The stored flag makes the signal loss-free: the Host transport may observe
/// EOF before the event forwarder registers its waiter, and `wait` still
/// returns immediately in that case.
#[derive(Debug, Default)]
pub struct AppServerDisconnect {
    signaled: std::sync::atomic::AtomicBool,
    notify: tokio::sync::Notify,
}

impl AppServerDisconnect {
    /// Mark the transport as disconnected and wake any waiters.
    pub fn signal(&self) {
        self.signaled
            .store(true, std::sync::atomic::Ordering::Release);
        self.notify.notify_waiters();
    }

    pub fn is_signaled(&self) -> bool {
        self.signaled.load(std::sync::atomic::Ordering::Acquire)
    }

    /// Wait until the transport signals a disconnect. Returns immediately if
    /// the signal was already raised.
    pub async fn wait(&self) {
        loop {
            let notified = self.notify.notified();
            if self.is_signaled() {
                return;
            }
            notified.await;
        }
    }
}

/// Host-selected transport limits advertised in `app/initialize` and enforced
/// by the Host transport where it owns the reader.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AppServerHostLimits {
    pub max_frame_bytes: u64,
    pub event_buffer_capacity: u32,
}

impl AppServerHostLimits {
    /// Limits for the local stdio Server Host (`openbitfun server`).
    pub const fn local_stdio() -> Self {
        Self {
            max_frame_bytes: DEFAULT_MAX_FRAME_BYTES,
            event_buffer_capacity: DEFAULT_EVENT_BUFFER_CAPACITY,
        }
    }
}

impl Default for AppServerHostLimits {
    fn default() -> Self {
        Self::local_stdio()
    }
}

/// Wire keys that carry filesystem paths in App Server requests.
const PATH_KEYS: &[&str] = &[
    "workspacePath",
    "workspace_path",
    "repositoryPath",
    "repository_path",
    "projectWorkspacePath",
    "project_workspace_path",
];

/// Wire keys that request remote execution, which a local Host cannot honor.
const REMOTE_KEYS: &[&str] = &[
    "remoteConnectionId",
    "remote_connection_id",
    "remoteSshHost",
    "remote_ssh_host",
];

/// Immutable Host identity, canonical workspace scope, and method allowlist
/// for one App Server deployment.
#[derive(Debug, Clone)]
pub struct AppServerHostPolicy {
    identity: String,
    workspace_root: PathBuf,
    allowed_methods: BTreeSet<String>,
}

/// Fail-closed violation reported before a request reaches a domain owner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostPolicyViolation {
    MethodNotAllowed { method: String },
    PathOutsideWorkspace { method: String, path: String },
    RemoteExecutionNotSupported { method: String },
    NonLocalExecutionTarget { method: String, kind: String },
}

impl HostPolicyViolation {
    pub fn method(&self) -> &str {
        match self {
            Self::MethodNotAllowed { method }
            | Self::PathOutsideWorkspace { method, .. }
            | Self::RemoteExecutionNotSupported { method }
            | Self::NonLocalExecutionTarget { method, .. } => method,
        }
    }

    /// Stable machine-readable reason surfaced in the wire error data.
    pub fn reason(&self) -> &'static str {
        match self {
            Self::MethodNotAllowed { .. } => "method_not_allowed_by_host_policy",
            Self::PathOutsideWorkspace { .. } => "path_outside_workspace_scope",
            Self::RemoteExecutionNotSupported { .. } => "remote_execution_not_supported",
            Self::NonLocalExecutionTarget { .. } => "non_local_execution_target",
        }
    }
}

impl std::fmt::Display for HostPolicyViolation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MethodNotAllowed { method } => {
                write!(f, "method `{method}` is not allowed by the Host policy")
            }
            Self::PathOutsideWorkspace { method, path } => write!(
                f,
                "method `{method}` carries a path outside the Host workspace scope: {path}"
            ),
            Self::RemoteExecutionNotSupported { method } => write!(
                f,
                "method `{method}` requests remote execution, which this Host does not provide"
            ),
            Self::NonLocalExecutionTarget { method, kind } => write!(
                f,
                "method `{method}` requests execution target kind `{kind}`, which this Host does not provide"
            ),
        }
    }
}

impl AppServerHostPolicy {
    /// Build a policy from a Host-selected canonical workspace root and the
    /// explicit list of methods this Host serves. Everything not listed here
    /// fails closed.
    pub fn new(
        identity: impl Into<String>,
        workspace_root: impl AsRef<Path>,
        allowed_methods: impl IntoIterator<Item = impl Into<String>>,
    ) -> Result<Self, std::io::Error> {
        let workspace_root = dunce::canonicalize(workspace_root.as_ref())?;
        Ok(Self {
            identity: identity.into(),
            workspace_root,
            allowed_methods: allowed_methods.into_iter().map(Into::into).collect(),
        })
    }

    pub fn identity(&self) -> &str {
        &self.identity
    }

    pub fn workspace_root(&self) -> &Path {
        &self.workspace_root
    }

    pub fn allows(&self, method: &str) -> bool {
        self.allowed_methods.contains(method)
    }

    pub fn allowed_methods(&self) -> impl Iterator<Item = &str> {
        self.allowed_methods.iter().map(String::as_str)
    }

    /// Fail-closed check for one incoming message.
    pub fn check(&self, method: &str, params: &Value) -> Result<(), HostPolicyViolation> {
        if !self.allows(method) {
            return Err(HostPolicyViolation::MethodNotAllowed {
                method: method.to_string(),
            });
        }
        self.validate_scope(method, params)
    }

    fn validate_scope(&self, method: &str, params: &Value) -> Result<(), HostPolicyViolation> {
        match params {
            Value::Object(map) => {
                for (key, value) in map {
                    if PATH_KEYS.contains(&key.as_str()) {
                        self.validate_path(method, key, value)?;
                    }
                    if REMOTE_KEYS.contains(&key.as_str()) && !is_empty_string(value) {
                        return Err(HostPolicyViolation::RemoteExecutionNotSupported {
                            method: method.to_string(),
                        });
                    }
                    if matches!(key.as_str(), "executionTarget" | "execution_target")
                        && !is_local_execution_target(value)
                    {
                        return Err(HostPolicyViolation::NonLocalExecutionTarget {
                            method: method.to_string(),
                            kind: value
                                .get("kind")
                                .and_then(Value::as_str)
                                .unwrap_or("unknown")
                                .to_string(),
                        });
                    }
                    self.validate_scope(method, value)?;
                }
            }
            Value::Array(items) => {
                for item in items {
                    self.validate_scope(method, item)?;
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn validate_path(
        &self,
        method: &str,
        key: &str,
        value: &Value,
    ) -> Result<(), HostPolicyViolation> {
        let outside = |path: String| HostPolicyViolation::PathOutsideWorkspace {
            method: method.to_string(),
            path,
        };
        let Some(path) = value.as_str() else {
            return Err(outside(format!("{key}: {value}")));
        };
        let candidate = Path::new(path);
        if !candidate.is_absolute() {
            return Err(outside(path.to_string()));
        }
        let Ok(canonical) = dunce::canonicalize(candidate) else {
            return Err(outside(path.to_string()));
        };
        if canonical != self.workspace_root && !canonical.starts_with(&self.workspace_root) {
            return Err(outside(path.to_string()));
        }
        Ok(())
    }
}

/// Map a policy violation to the JSON-RPC error the client receives.
///
/// Method denial is `method_not_found` (the Host does not serve it); scope and
/// execution-target violations are `invalid_params` because the method exists
/// but the supplied arguments leave the Host's execution domain. The wire
/// `data` carries the stable `AppServerErrorData` shape plus the machine
/// readable `reason` and the denied `method`.
pub fn violation_error(violation: &HostPolicyViolation) -> Error {
    let (error, kind) = match violation {
        HostPolicyViolation::MethodNotAllowed { .. } => {
            (Error::method_not_found(), AppServerErrorKind::Unsupported)
        }
        HostPolicyViolation::PathOutsideWorkspace { .. } => {
            (Error::invalid_params(), AppServerErrorKind::InvalidRequest)
        }
        HostPolicyViolation::RemoteExecutionNotSupported { .. }
        | HostPolicyViolation::NonLocalExecutionTarget { .. } => {
            (Error::invalid_params(), AppServerErrorKind::Unsupported)
        }
    };
    let mut data = serde_json::to_value(AppServerErrorData {
        kind,
        retryable: false,
        outcome_unknown: false,
        capability: None,
        request_id: None,
    })
    .unwrap_or(Value::Null);
    if let Some(object) = data.as_object_mut() {
        object.insert(
            "reason".to_string(),
            Value::String(violation.reason().to_string()),
        );
        object.insert(
            "method".to_string(),
            Value::String(violation.method().to_string()),
        );
    }
    error.data(data)
}

/// First connection-builder layer: the Host policy guard.
///
/// Every incoming request and notification is checked fail-closed before any
/// domain handler sees it. Denied requests receive a typed error response,
/// denied notifications receive an error notification, and responses pass
/// through untouched. Without a Host policy the guard passes everything to the
/// existing open surface.
pub(super) fn builder(
    policy: Option<Arc<AppServerHostPolicy>>,
) -> Builder<AppServer, impl HandleDispatchFrom<AppClient>> {
    AppServer
        .builder()
        .name("host policy guard")
        .on_receive_dispatch(
            async move |message: Dispatch, cx: ConnectionTo<AppClient>| {
                let Some(policy) = policy.as_ref() else {
                    return Ok(Handled::No {
                        message,
                        retry: false,
                    });
                };
                match message {
                    Dispatch::Request(request, responder) => {
                        match policy.check(request.method(), request.params()) {
                            Ok(()) => Ok(Handled::No {
                                message: Dispatch::Request(request, responder),
                                retry: false,
                            }),
                            Err(violation) => {
                                tracing::warn!(
                                    method = violation.method(),
                                    reason = violation.reason(),
                                    "App-server host policy denied request"
                                );
                                responder.respond_with_error(violation_error(&violation))?;
                                Ok(Handled::Yes)
                            }
                        }
                    }
                    Dispatch::Notification(notification) => {
                        match policy.check(notification.method(), notification.params()) {
                            Ok(()) => Ok(Handled::No {
                                message: Dispatch::Notification(notification),
                                retry: false,
                            }),
                            Err(violation) => {
                                tracing::warn!(
                                    method = violation.method(),
                                    reason = violation.reason(),
                                    "App-server host policy denied notification"
                                );
                                cx.send_error_notification(violation_error(&violation))?;
                                Ok(Handled::Yes)
                            }
                        }
                    }
                    Dispatch::Response(result, router) => Ok(Handled::No {
                        message: Dispatch::Response(result, router),
                        retry: false,
                    }),
                }
            },
            agent_client_protocol::on_receive_dispatch!(),
        )
}

fn is_empty_string(value: &Value) -> bool {
    match value {
        Value::String(text) => text.is_empty(),
        Value::Null => true,
        _ => false,
    }
}

fn is_local_execution_target(value: &Value) -> bool {
    match value.get("kind").and_then(Value::as_str) {
        None => true,
        Some(kind) => kind == "local",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn policy(root: &Path) -> AppServerHostPolicy {
        AppServerHostPolicy::new(
            "test-host",
            root,
            ["app/initialize", "git/isRepository", "agent/createSession"],
        )
        .expect("build test policy")
    }

    fn outside_path(root: &Path) -> PathBuf {
        let candidate = root.parent().expect("root parent").join("outside.txt");
        std::fs::write(&candidate, "x").expect("write outside file");
        candidate
    }

    #[test]
    fn unlisted_methods_fail_closed() {
        let root = std::env::temp_dir();
        let policy = policy(&root);
        let violation = policy
            .check("config/setConfig", &serde_json::json!({}))
            .expect_err("unlisted method must be denied");
        assert_eq!(
            violation,
            HostPolicyViolation::MethodNotAllowed {
                method: "config/setConfig".to_string()
            }
        );
    }

    #[test]
    fn paths_must_canonicalize_inside_the_workspace_root() {
        let root = std::env::temp_dir();
        let policy = policy(&root);
        let outside = outside_path(&root);
        let violation = policy
            .check(
                "git/isRepository",
                &serde_json::json!({ "repositoryPath": outside.to_string_lossy() }),
            )
            .expect_err("path outside the workspace must be denied");
        assert_eq!(
            violation,
            HostPolicyViolation::PathOutsideWorkspace {
                method: "git/isRepository".to_string(),
                path: outside.to_string_lossy().to_string(),
            }
        );

        policy
            .check(
                "git/isRepository",
                &serde_json::json!({ "repositoryPath": root.to_string_lossy() }),
            )
            .expect("workspace root itself is in scope");
    }

    #[test]
    fn nested_and_relative_paths_fail_closed() {
        let root = std::env::temp_dir();
        let policy = policy(&root);
        let violation = policy
            .check(
                "agent/createSession",
                &serde_json::json!({ "workspacePath": "relative/workspace" }),
            )
            .expect_err("relative paths must be denied");
        assert!(matches!(
            violation,
            HostPolicyViolation::PathOutsideWorkspace { .. }
        ));

        let outside = outside_path(&root);
        let violation = policy
            .check(
                "agent/createSession",
                &serde_json::json!({
                    "executionTarget": { "kind": "local" },
                    "nested": { "projectWorkspacePath": outside.to_string_lossy() },
                }),
            )
            .expect_err("nested path keys must be checked");
        assert!(matches!(
            violation,
            HostPolicyViolation::PathOutsideWorkspace { .. }
        ));
    }

    #[test]
    fn remote_and_non_local_execution_fail_closed() {
        let root = std::env::temp_dir();
        let policy = policy(&root);
        let violation = policy
            .check(
                "agent/createSession",
                &serde_json::json!({ "remoteConnectionId": "peer-1" }),
            )
            .expect_err("remote connections must be denied");
        assert_eq!(
            violation,
            HostPolicyViolation::RemoteExecutionNotSupported {
                method: "agent/createSession".to_string()
            }
        );

        let violation = policy
            .check(
                "agent/createSession",
                &serde_json::json!({ "executionTarget": { "kind": "managedWorktree" } }),
            )
            .expect_err("non-local execution targets must be denied");
        assert_eq!(
            violation,
            HostPolicyViolation::NonLocalExecutionTarget {
                method: "agent/createSession".to_string(),
                kind: "managedWorktree".to_string(),
            }
        );

        policy
            .check(
                "agent/createSession",
                &serde_json::json!({
                    "executionTarget": { "kind": "local" },
                    "remoteConnectionId": null,
                    "remoteSshHost": "",
                }),
            )
            .expect("local execution without remote fields is in scope");
    }

    #[test]
    fn violation_error_carries_stable_kind_and_reason() {
        let violation = HostPolicyViolation::PathOutsideWorkspace {
            method: "git/isRepository".to_string(),
            path: "/outside".to_string(),
        };
        let error = violation_error(&violation);
        assert_eq!(error.code, agent_client_protocol::ErrorCode::InvalidParams);
        let data = error
            .data
            .expect("policy violation error must carry structured data");
        assert_eq!(data["kind"], Value::String("invalid_request".to_string()));
        assert_eq!(
            data["reason"],
            Value::String("path_outside_workspace_scope".to_string())
        );
        assert_eq!(
            data["method"],
            Value::String("git/isRepository".to_string())
        );

        let violation = HostPolicyViolation::MethodNotAllowed {
            method: "config/setConfig".to_string(),
        };
        let error = violation_error(&violation);
        assert_eq!(error.code, agent_client_protocol::ErrorCode::MethodNotFound);
        assert_eq!(
            error.data.expect("data")["kind"],
            Value::String("unsupported".to_string())
        );
    }
}
