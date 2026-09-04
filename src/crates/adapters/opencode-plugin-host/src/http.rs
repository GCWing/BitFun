use crate::{PluginHostClient, PluginHostError};
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::time::Duration;
use thiserror::Error;
use url::Url;

pub(crate) const MAX_HTTP_BODY_BYTES: usize = 1024 * 1024;
pub(crate) const MAX_STREAM_CHUNK_BYTES: usize = 64 * 1024;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BackendHttpRequest {
    #[serde(rename = "instanceID")]
    pub instance_id: String,
    #[serde(rename = "requestID")]
    pub request_id: String,
    pub method: String,
    pub path: String,
    pub headers: Vec<(String, String)>,
    pub body: Option<StreamDescriptor>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StreamDescriptor {
    #[serde(rename = "streamID")]
    pub stream_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub length: Option<usize>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BackendHttpResponse {
    pub status: u16,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status_text: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub headers: Vec<(String, String)>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<StreamDescriptor>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OpenCodeClientRoute {
    ProjectList,
    ProjectCurrent,
    PathGet,
    VcsGet,
    ConfigGet,
    ConfigProviders,
    ToolIds,
    ToolList,
    ProviderList,
    AppLog,
    AgentList,
    CommandList,
    SessionList,
    SessionCreate,
    SessionStatus,
    SessionDelete {
        session_id: String,
    },
    SessionGet {
        session_id: String,
    },
    SessionUpdate {
        session_id: String,
    },
    SessionChildren {
        session_id: String,
    },
    SessionTodo {
        session_id: String,
    },
    SessionFork {
        session_id: String,
    },
    SessionAbort {
        session_id: String,
    },
    SessionDiff {
        session_id: String,
    },
    SessionMessages {
        session_id: String,
    },
    SessionMessage {
        session_id: String,
        message_id: String,
    },
    PtyList,
    PtyCreate,
    PtyDelete {
        pty_id: String,
    },
    PtyGet {
        pty_id: String,
    },
    PtyUpdate {
        pty_id: String,
    },
    FindText,
    FindFiles,
    FileList,
    FileRead,
    FileStatus,
    McpStatus,
    LspStatus,
}

impl OpenCodeClientRoute {
    pub fn operation(&self) -> &'static str {
        match self {
            Self::ProjectList => "project.list",
            Self::ProjectCurrent => "project.current",
            Self::PathGet => "path.get",
            Self::VcsGet => "vcs.get",
            Self::ConfigGet => "config.get",
            Self::ConfigProviders => "config.providers",
            Self::ToolIds => "tool.ids",
            Self::ToolList => "tool.list",
            Self::ProviderList => "provider.list",
            Self::AppLog => "app.log",
            Self::AgentList => "app.agents",
            Self::CommandList => "command.list",
            Self::SessionList => "session.list",
            Self::SessionCreate => "session.create",
            Self::SessionStatus => "session.status",
            Self::SessionDelete { .. } => "session.delete",
            Self::SessionGet { .. } => "session.get",
            Self::SessionUpdate { .. } => "session.update",
            Self::SessionChildren { .. } => "session.children",
            Self::SessionTodo { .. } => "session.todo",
            Self::SessionFork { .. } => "session.fork",
            Self::SessionAbort { .. } => "session.abort",
            Self::SessionDiff { .. } => "session.diff",
            Self::SessionMessages { .. } => "session.messages",
            Self::SessionMessage { .. } => "session.message",
            Self::PtyList => "pty.list",
            Self::PtyCreate => "pty.create",
            Self::PtyDelete { .. } => "pty.remove",
            Self::PtyGet { .. } => "pty.get",
            Self::PtyUpdate { .. } => "pty.update",
            Self::FindText => "find.text",
            Self::FindFiles => "find.files",
            Self::FileList => "file.list",
            Self::FileRead => "file.read",
            Self::FileStatus => "file.status",
            Self::McpStatus => "mcp.status",
            Self::LspStatus => "lsp.status",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct HttpRouteMatch {
    pub route: OpenCodeClientRoute,
    pub path: String,
    pub query: HashMap<String, Vec<String>>,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub(crate) enum HttpRouteError {
    #[error("request path is invalid")]
    InvalidPath,
    #[error("OpenCode client route was not found")]
    NotFound,
    #[error("HTTP method is not allowed for this OpenCode client route")]
    MethodNotAllowed,
}

pub(crate) fn match_http_route(
    method: &str,
    path_and_query: &str,
) -> Result<HttpRouteMatch, HttpRouteError> {
    if !path_and_query.starts_with('/') || path_and_query.len() > 16 * 1024 {
        return Err(HttpRouteError::InvalidPath);
    }
    let url = Url::parse(&format!("http://127.0.0.1{path_and_query}"))
        .map_err(|_| HttpRouteError::InvalidPath)?;
    let path = url.path().trim_end_matches('/');
    let path = if path.is_empty() { "/" } else { path };
    let query = url.query_pairs().fold(
        HashMap::<String, Vec<String>>::new(),
        |mut query, (key, value)| {
            query
                .entry(key.into_owned())
                .or_default()
                .push(value.into_owned());
            query
        },
    );
    let method = method.trim().to_ascii_uppercase();
    let segments = path
        .split('/')
        .filter(|segment| !segment.is_empty())
        .map(decode_segment)
        .collect::<Result<Vec<_>, _>>()?;
    let route = match (method.as_str(), path, segments.as_slice()) {
        ("GET", "/project", _) => OpenCodeClientRoute::ProjectList,
        ("GET", "/project/current", _) => OpenCodeClientRoute::ProjectCurrent,
        ("GET", "/path", _) => OpenCodeClientRoute::PathGet,
        ("GET", "/vcs", _) => OpenCodeClientRoute::VcsGet,
        ("GET", "/config", _) => OpenCodeClientRoute::ConfigGet,
        ("GET", "/config/providers", _) => OpenCodeClientRoute::ConfigProviders,
        ("GET", "/experimental/tool/ids", _) => OpenCodeClientRoute::ToolIds,
        ("GET", "/experimental/tool", _) => OpenCodeClientRoute::ToolList,
        ("GET", "/provider", _) => OpenCodeClientRoute::ProviderList,
        ("POST", "/log", _) => OpenCodeClientRoute::AppLog,
        ("GET", "/agent", _) => OpenCodeClientRoute::AgentList,
        ("GET", "/command", _) => OpenCodeClientRoute::CommandList,
        ("GET", "/session", _) => OpenCodeClientRoute::SessionList,
        ("POST", "/session", _) => OpenCodeClientRoute::SessionCreate,
        ("GET", "/session/status", _) => OpenCodeClientRoute::SessionStatus,
        ("DELETE", _, [session, session_id]) if session == "session" && session_id != "status" => {
            OpenCodeClientRoute::SessionDelete {
                session_id: session_id.clone(),
            }
        }
        ("GET", _, [session, session_id]) if session == "session" && session_id != "status" => {
            OpenCodeClientRoute::SessionGet {
                session_id: session_id.clone(),
            }
        }
        ("PATCH", _, [session, session_id]) if session == "session" && session_id != "status" => {
            OpenCodeClientRoute::SessionUpdate {
                session_id: session_id.clone(),
            }
        }
        ("GET", _, [session, session_id, suffix])
            if session == "session" && session_id != "status" && suffix == "children" =>
        {
            OpenCodeClientRoute::SessionChildren {
                session_id: session_id.clone(),
            }
        }
        ("GET", _, [session, session_id, suffix])
            if session == "session" && session_id != "status" && suffix == "todo" =>
        {
            OpenCodeClientRoute::SessionTodo {
                session_id: session_id.clone(),
            }
        }
        ("POST", _, [session, session_id, suffix])
            if session == "session" && session_id != "status" && suffix == "fork" =>
        {
            OpenCodeClientRoute::SessionFork {
                session_id: session_id.clone(),
            }
        }
        ("POST", _, [session, session_id, suffix])
            if session == "session" && session_id != "status" && suffix == "abort" =>
        {
            OpenCodeClientRoute::SessionAbort {
                session_id: session_id.clone(),
            }
        }
        ("GET", _, [session, session_id, suffix])
            if session == "session" && session_id != "status" && suffix == "diff" =>
        {
            OpenCodeClientRoute::SessionDiff {
                session_id: session_id.clone(),
            }
        }
        ("GET", _, [session, session_id, suffix])
            if session == "session" && session_id != "status" && suffix == "message" =>
        {
            OpenCodeClientRoute::SessionMessages {
                session_id: session_id.clone(),
            }
        }
        ("GET", _, [session, session_id, message, message_id])
            if session == "session" && session_id != "status" && message == "message" =>
        {
            OpenCodeClientRoute::SessionMessage {
                session_id: session_id.clone(),
                message_id: message_id.clone(),
            }
        }
        ("GET", "/pty", _) => OpenCodeClientRoute::PtyList,
        ("POST", "/pty", _) => OpenCodeClientRoute::PtyCreate,
        ("DELETE", _, [pty, pty_id]) if pty == "pty" => OpenCodeClientRoute::PtyDelete {
            pty_id: pty_id.clone(),
        },
        ("GET", _, [pty, pty_id]) if pty == "pty" => OpenCodeClientRoute::PtyGet {
            pty_id: pty_id.clone(),
        },
        ("PUT", _, [pty, pty_id]) if pty == "pty" => OpenCodeClientRoute::PtyUpdate {
            pty_id: pty_id.clone(),
        },
        ("GET", "/find", _) => OpenCodeClientRoute::FindText,
        ("GET", "/find/file", _) => OpenCodeClientRoute::FindFiles,
        ("GET", "/file", _) => OpenCodeClientRoute::FileList,
        ("GET", "/file/content", _) => OpenCodeClientRoute::FileRead,
        ("GET", "/file/status", _) => OpenCodeClientRoute::FileStatus,
        ("GET", "/mcp", _) => OpenCodeClientRoute::McpStatus,
        ("GET", "/lsp", _) => OpenCodeClientRoute::LspStatus,
        _ if is_known_adapted_path(path, &segments) => {
            return Err(HttpRouteError::MethodNotAllowed)
        }
        _ => return Err(HttpRouteError::NotFound),
    };
    Ok(HttpRouteMatch {
        route,
        path: path.to_string(),
        query,
    })
}

fn decode_segment(segment: &str) -> Result<String, HttpRouteError> {
    let bytes = segment.as_bytes();
    for index in 0..bytes.len() {
        if bytes[index] == b'%'
            && (index + 2 >= bytes.len()
                || !bytes[index + 1].is_ascii_hexdigit()
                || !bytes[index + 2].is_ascii_hexdigit())
        {
            return Err(HttpRouteError::InvalidPath);
        }
    }
    let decoded = urlencoding::decode(segment).map_err(|_| HttpRouteError::InvalidPath)?;
    if decoded.is_empty() || decoded.contains('/') || decoded.contains('\\') {
        return Err(HttpRouteError::InvalidPath);
    }
    Ok(decoded.into_owned())
}

fn is_known_adapted_path(path: &str, segments: &[String]) -> bool {
    matches!(
        path,
        "/project"
            | "/project/current"
            | "/path"
            | "/vcs"
            | "/config"
            | "/config/providers"
            | "/experimental/tool/ids"
            | "/experimental/tool"
            | "/provider"
            | "/log"
            | "/agent"
            | "/command"
            | "/session"
            | "/session/status"
            | "/pty"
            | "/find"
            | "/find/file"
            | "/file"
            | "/file/content"
            | "/file/status"
            | "/mcp"
            | "/lsp"
    ) || matches!(segments, [root, _] if root == "session" || root == "pty")
        || matches!(
            segments,
            [root, _, suffix]
                if root == "session"
                    && matches!(
                        suffix.as_str(),
                        "children" | "todo" | "fork" | "abort" | "diff" | "message"
                    )
        )
        || matches!(segments, [root, _, message, _] if root == "session" && message == "message")
}

#[derive(Debug, Error)]
pub(crate) enum HostStreamReadError {
    #[error("request body exceeds the maximum allowed size")]
    BodyTooLarge,
    #[error("host stream returned invalid base64 data: {0}")]
    InvalidBase64(#[source] base64::DecodeError),
    #[error("host stream RPC failed: {0}")]
    Rpc(#[from] PluginHostError),
    #[error("host stream returned an invalid response")]
    InvalidResponse,
}

pub(crate) async fn read_host_stream(
    client: &PluginHostClient,
    instance_id: &str,
    descriptor: &StreamDescriptor,
    max_bytes: usize,
    deadline: Duration,
) -> Result<Vec<u8>, HostStreamReadError> {
    let result = read_host_stream_inner(client, instance_id, descriptor, max_bytes, deadline).await;
    if let Err(error) = &result {
        let reason = match error {
            HostStreamReadError::BodyTooLarge => "request body too large",
            HostStreamReadError::InvalidBase64(_) => "host stream returned invalid base64 data",
            HostStreamReadError::InvalidResponse => "host stream returned an invalid response",
            HostStreamReadError::Rpc(_) => "host stream RPC failed",
        };
        cancel_host_stream(client, instance_id, descriptor, reason).await;
    }
    result
}

async fn read_host_stream_inner(
    client: &PluginHostClient,
    instance_id: &str,
    descriptor: &StreamDescriptor,
    max_bytes: usize,
    deadline: Duration,
) -> Result<Vec<u8>, HostStreamReadError> {
    if descriptor.length.is_some_and(|length| length > max_bytes) {
        return Err(HostStreamReadError::BodyTooLarge);
    }
    let mut output = Vec::with_capacity(descriptor.length.unwrap_or(0).min(max_bytes));
    loop {
        let response = client
            .request(
                "host.stream.read",
                json!({
                    "instanceID": instance_id,
                    "streamID": descriptor.stream_id,
                    "maxBytes": MAX_STREAM_CHUNK_BYTES,
                }),
                deadline,
            )
            .await?;
        let data = response
            .get("data")
            .and_then(Value::as_str)
            .ok_or(HostStreamReadError::InvalidResponse)?;
        let eof = response
            .get("eof")
            .and_then(Value::as_bool)
            .ok_or(HostStreamReadError::InvalidResponse)?;
        let chunk = BASE64_STANDARD
            .decode(data)
            .map_err(HostStreamReadError::InvalidBase64)?;
        if output.len().saturating_add(chunk.len()) > max_bytes {
            return Err(HostStreamReadError::BodyTooLarge);
        }
        output.extend_from_slice(&chunk);
        if eof {
            return Ok(output);
        }
    }
}

async fn cancel_host_stream(
    client: &PluginHostClient,
    instance_id: &str,
    descriptor: &StreamDescriptor,
    reason: &str,
) {
    let _ = client
        .request(
            "host.stream.cancel",
            json!({
                "instanceID": instance_id,
                "streamID": descriptor.stream_id,
                "reason": reason,
            }),
            Duration::from_secs(2),
        )
        .await;
}

pub(crate) fn json_error_body(code: &str, message: &str, route: &str) -> Vec<u8> {
    serde_json::to_vec(&json!({
        "error": {
            "code": code,
            "message": message,
            "route": route,
        }
    }))
    .unwrap_or_else(|_| b"{\"error\":{\"code\":\"backend_failure\"}}".to_vec())
}

#[cfg(test)]
mod tests {
    use super::{match_http_route, HttpRouteError, OpenCodeClientRoute};

    fn assert_route(method: &str, path: &str, expected: OpenCodeClientRoute) {
        let matched = match_http_route(method, path)
            .unwrap_or_else(|error| panic!("route did not match: {method} {path}: {error}"));
        assert_eq!(
            matched.route, expected,
            "unexpected route for {method} {path}"
        );
    }

    #[test]
    fn adapted_route_matrix_covers_every_documented_a_route() {
        let cases = vec![
            ("GET", "/project", OpenCodeClientRoute::ProjectList),
            (
                "GET",
                "/project/current?directory=C%3A%5Cworkspace",
                OpenCodeClientRoute::ProjectCurrent,
            ),
            ("GET", "/path", OpenCodeClientRoute::PathGet),
            ("GET", "/vcs", OpenCodeClientRoute::VcsGet),
            ("GET", "/config", OpenCodeClientRoute::ConfigGet),
            (
                "GET",
                "/config/providers",
                OpenCodeClientRoute::ConfigProviders,
            ),
            (
                "GET",
                "/experimental/tool/ids",
                OpenCodeClientRoute::ToolIds,
            ),
            (
                "GET",
                "/experimental/tool?provider=openbitfun&model=primary",
                OpenCodeClientRoute::ToolList,
            ),
            ("GET", "/provider", OpenCodeClientRoute::ProviderList),
            ("POST", "/log", OpenCodeClientRoute::AppLog),
            ("GET", "/agent", OpenCodeClientRoute::AgentList),
            ("GET", "/command", OpenCodeClientRoute::CommandList),
            ("GET", "/session", OpenCodeClientRoute::SessionList),
            ("POST", "/session", OpenCodeClientRoute::SessionCreate),
            ("GET", "/session/status", OpenCodeClientRoute::SessionStatus),
            (
                "DELETE",
                "/session/session%3A1",
                OpenCodeClientRoute::SessionDelete {
                    session_id: "session:1".to_string(),
                },
            ),
            (
                "GET",
                "/session/session%3A1",
                OpenCodeClientRoute::SessionGet {
                    session_id: "session:1".to_string(),
                },
            ),
            (
                "PATCH",
                "/session/session%3A1",
                OpenCodeClientRoute::SessionUpdate {
                    session_id: "session:1".to_string(),
                },
            ),
            (
                "GET",
                "/session/session%3A1/children",
                OpenCodeClientRoute::SessionChildren {
                    session_id: "session:1".to_string(),
                },
            ),
            (
                "GET",
                "/session/session%3A1/todo",
                OpenCodeClientRoute::SessionTodo {
                    session_id: "session:1".to_string(),
                },
            ),
            (
                "POST",
                "/session/session%3A1/fork",
                OpenCodeClientRoute::SessionFork {
                    session_id: "session:1".to_string(),
                },
            ),
            (
                "POST",
                "/session/session%3A1/abort",
                OpenCodeClientRoute::SessionAbort {
                    session_id: "session:1".to_string(),
                },
            ),
            (
                "GET",
                "/session/session%3A1/diff?messageID=message%3A1",
                OpenCodeClientRoute::SessionDiff {
                    session_id: "session:1".to_string(),
                },
            ),
            (
                "GET",
                "/session/session%3A1/message?limit=10",
                OpenCodeClientRoute::SessionMessages {
                    session_id: "session:1".to_string(),
                },
            ),
            (
                "GET",
                "/session/session%3A1/message/message%3A2",
                OpenCodeClientRoute::SessionMessage {
                    session_id: "session:1".to_string(),
                    message_id: "message:2".to_string(),
                },
            ),
            ("GET", "/pty", OpenCodeClientRoute::PtyList),
            ("POST", "/pty", OpenCodeClientRoute::PtyCreate),
            (
                "DELETE",
                "/pty/pty%3A1",
                OpenCodeClientRoute::PtyDelete {
                    pty_id: "pty:1".to_string(),
                },
            ),
            (
                "GET",
                "/pty/pty%3A1",
                OpenCodeClientRoute::PtyGet {
                    pty_id: "pty:1".to_string(),
                },
            ),
            (
                "PUT",
                "/pty/pty%3A1",
                OpenCodeClientRoute::PtyUpdate {
                    pty_id: "pty:1".to_string(),
                },
            ),
            ("GET", "/find?pattern=needle", OpenCodeClientRoute::FindText),
            (
                "GET",
                "/find/file?query=needle",
                OpenCodeClientRoute::FindFiles,
            ),
            ("GET", "/file?path=src", OpenCodeClientRoute::FileList),
            (
                "GET",
                "/file/content?path=README.md",
                OpenCodeClientRoute::FileRead,
            ),
            ("GET", "/file/status", OpenCodeClientRoute::FileStatus),
            ("GET", "/mcp", OpenCodeClientRoute::McpStatus),
            ("GET", "/lsp", OpenCodeClientRoute::LspStatus),
        ];

        for (method, path, expected) in cases {
            assert_route(method, path, expected);
        }
    }

    #[test]
    fn normalizes_methods_paths_and_query_values() {
        let matched = match_http_route(
            " get ",
            "/project/current/?directory=C%3A%5Cworkspace&directory=D%3A%5Cignored",
        )
        .expect("normalized project route");

        assert_eq!(matched.route, OpenCodeClientRoute::ProjectCurrent);
        assert_eq!(matched.path, "/project/current");
        assert_eq!(
            matched
                .query
                .get("directory")
                .and_then(|values| values.first())
                .map(String::as_str),
            Some("C:\\workspace")
        );
        assert_eq!(
            matched.query.get("directory"),
            Some(&vec![
                "C:\\workspace".to_string(),
                "D:\\ignored".to_string()
            ])
        );
    }

    #[test]
    fn rejects_invalid_and_unsafe_route_paths() {
        let oversized = format!("/{}", "a".repeat(16 * 1024));
        for path in [
            "project/current",
            "/session/%ZZ",
            "/session/session%2Fescape",
            "/session/session%5Cescape",
            oversized.as_str(),
        ] {
            assert_eq!(
                match_http_route("GET", path),
                Err(HttpRouteError::InvalidPath),
                "invalid path unexpectedly matched: {path}"
            );
        }

        assert_eq!(
            match_http_route("GET", "/unknown"),
            Err(HttpRouteError::NotFound)
        );
    }

    #[test]
    fn postponed_and_excluded_routes_are_not_in_the_route_table() {
        for (method, path) in [
            ("GET", "/global/event"),
            ("GET", "/event"),
            ("POST", "/instance/dispose"),
            ("GET", "/provider/auth"),
            ("POST", "/provider/openai/oauth/authorize"),
            ("POST", "/provider/openai/oauth/callback"),
            ("GET", "/pty/pty-1/connect"),
            ("GET", "/find/symbol"),
            ("GET", "/formatter"),
            ("POST", "/session/s1/init"),
            ("POST", "/session/s1/summarize"),
            ("DELETE", "/session/s1/share"),
            ("POST", "/session/s1/share"),
            ("POST", "/session/s1/prompt_async"),
            ("POST", "/session/s1/command"),
            ("POST", "/session/s1/shell"),
            ("POST", "/session/s1/revert"),
            ("POST", "/session/s1/unrevert"),
            ("POST", "/mcp/server/connect"),
            ("POST", "/mcp/server/disconnect"),
            ("DELETE", "/mcp/server/auth"),
            ("POST", "/mcp/server/auth"),
            ("POST", "/mcp/server/auth/callback"),
            ("POST", "/mcp/server/auth/authenticate"),
            ("PUT", "/auth/server"),
            ("DELETE", "/auth/provider"),
            ("POST", "/auth/provider"),
            ("POST", "/auth/provider/callback"),
            ("POST", "/auth/provider/authenticate"),
            ("POST", "/tui/append-prompt"),
            ("POST", "/tui/open-help"),
            ("POST", "/tui/open-sessions"),
            ("POST", "/tui/open-themes"),
            ("POST", "/tui/open-models"),
            ("POST", "/tui/submit-prompt"),
            ("POST", "/tui/clear-prompt"),
            ("POST", "/tui/execute-command"),
            ("POST", "/tui/show-toast"),
            ("POST", "/tui/publish"),
            ("GET", "/tui/control/next"),
            ("POST", "/tui/control/response"),
            ("POST", "/session/s1/permissions/p1"),
        ] {
            assert_eq!(
                match_http_route(method, path),
                Err(HttpRouteError::NotFound),
                "excluded route unexpectedly matched: {method} {path}"
            );
        }

        for (method, path) in [
            ("POST", "/project/current"),
            ("PATCH", "/config"),
            ("DELETE", "/config"),
            ("DELETE", "/session/status"),
            ("POST", "/session/s1/message"),
            ("PATCH", "/pty/pty-1"),
            ("POST", "/mcp"),
            ("POST", "/file/status"),
        ] {
            assert_eq!(
                match_http_route(method, path),
                Err(HttpRouteError::MethodNotAllowed),
                "adapted route accepted the wrong method: {method} {path}"
            );
        }
    }
}
