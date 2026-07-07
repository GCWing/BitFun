use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

pub const CANVAS_SOURCE_LANGUAGE_TSX: &str = "tsx";
pub const CANVAS_ARTIFACT_REF_SCHEME: &str = "bitfun-canvas";
pub const CANVAS_CURRENT_SOURCE_SCHEMA_VERSION: u32 = 1;
pub const CANVAS_CURRENT_STATE_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CanvasId(pub String);

impl CanvasId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CanvasRevision(pub String);

impl CanvasRevision {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CanvasSessionId(pub String);

impl CanvasSessionId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CanvasWorkspaceId(pub String);

impl CanvasWorkspaceId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CanvasScope {
    Session,
}

impl Default for CanvasScope {
    fn default() -> Self {
        Self::Session
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CanvasStatus {
    SourceSaved,
    Compiled,
    CompileFailed,
    RuntimeFailed,
    Unsupported,
}

impl Default for CanvasStatus {
    fn default() -> Self {
        Self::SourceSaved
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CanvasDiagnosticSeverity {
    Error,
    Warning,
    Info,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CanvasDiagnosticCategory {
    TypeScript,
    ImportPolicy,
    Compile,
    Runtime,
    HostBridge,
    State,
    Unsupported,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CanvasDiagnostic {
    pub severity: CanvasDiagnosticSeverity,
    pub category: CanvasDiagnosticCategory,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub column: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggested_fix: Option<String>,
}

impl CanvasDiagnostic {
    pub fn error(
        category: CanvasDiagnosticCategory,
        message: impl Into<String>,
        code: impl Into<String>,
    ) -> Self {
        Self {
            severity: CanvasDiagnosticSeverity::Error,
            category,
            message: message.into(),
            code: Some(code.into()),
            line: None,
            column: None,
            suggested_fix: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CanvasArtifact {
    pub id: CanvasId,
    #[serde(default)]
    pub scope: CanvasScope,
    pub session_id: CanvasSessionId,
    pub workspace_id: CanvasWorkspaceId,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub source_revision: CanvasRevision,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latest_compiled_revision: Option<CanvasRevision>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_known_good_revision: Option<CanvasRevision>,
    #[serde(default)]
    pub status: CanvasStatus,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CanvasSource {
    pub canvas_id: CanvasId,
    pub revision: CanvasRevision,
    pub filename: String,
    pub language: String,
    pub source: String,
    pub sdk_version: String,
    pub created_at: i64,
}

impl CanvasSource {
    pub fn new_tsx(
        canvas_id: CanvasId,
        revision: CanvasRevision,
        filename: impl Into<String>,
        source: impl Into<String>,
        sdk_version: impl Into<String>,
        created_at: i64,
    ) -> Self {
        Self {
            canvas_id,
            revision,
            filename: filename.into(),
            language: CANVAS_SOURCE_LANGUAGE_TSX.to_string(),
            source: source.into(),
            sdk_version: sdk_version.into(),
            created_at,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CanvasCompiledPayload {
    pub canvas_id: CanvasId,
    pub source_revision: CanvasRevision,
    pub sdk_version: String,
    pub runtime_version: String,
    pub html: String,
    pub content_hash: String,
    pub diagnostics: Vec<CanvasDiagnostic>,
    pub compiled_at: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CanvasState {
    pub canvas_id: CanvasId,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_revision_seen: Option<CanvasRevision>,
    #[serde(default)]
    pub values: BTreeMap<String, Value>,
    pub updated_at: i64,
    #[serde(default = "default_state_schema_version")]
    pub schema_version: u32,
}

fn default_state_schema_version() -> u32 {
    CANVAS_CURRENT_STATE_SCHEMA_VERSION
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CanvasArtifactRef {
    pub scheme: String,
    pub session_id: CanvasSessionId,
    pub canvas_id: CanvasId,
}

impl CanvasArtifactRef {
    pub fn new(session_id: CanvasSessionId, canvas_id: CanvasId) -> Self {
        Self {
            scheme: CANVAS_ARTIFACT_REF_SCHEME.to_string(),
            session_id,
            canvas_id,
        }
    }

    pub fn to_uri(&self) -> String {
        format!(
            "{}://session/{}/canvas/{}",
            self.scheme,
            percent_encode_segment(self.session_id.as_str()),
            percent_encode_segment(self.canvas_id.as_str())
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CanvasCapabilityStatus {
    pub supported: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

impl CanvasCapabilityStatus {
    pub fn supported() -> Self {
        Self {
            supported: true,
            reason: None,
        }
    }

    pub fn unsupported(reason: impl Into<String>) -> Self {
        Self {
            supported: false,
            reason: Some(reason.into()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CanvasSnapshot {
    pub artifact: CanvasArtifact,
    pub source: CanvasSource,
    #[serde(default)]
    pub diagnostics: Vec<CanvasDiagnostic>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compiled_payload: Option<CanvasCompiledPayload>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<CanvasState>,
}

fn percent_encode_segment(value: &str) -> String {
    value
        .bytes()
        .flat_map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                vec![byte as char]
            }
            _ => format!("%{byte:02X}").chars().collect(),
        })
        .collect()
}
