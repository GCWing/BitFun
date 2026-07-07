use crate::canvas::{
    runtime::CanvasCompileResult,
    types::{
        CanvasArtifact, CanvasCompiledPayload, CanvasDiagnostic, CanvasId, CanvasSessionId,
        CanvasSnapshot, CanvasSource, CanvasState,
    },
};
use serde::{Deserialize, Serialize};
use std::future::Future;
use std::pin::Pin;

pub type CanvasPortFuture<'a, T> = Pin<Box<dyn Future<Output = CanvasPortResult<T>> + Send + 'a>>;
pub type CanvasPortResult<T> = Result<T, CanvasPortError>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CanvasPortErrorKind {
    NotFound,
    InvalidInput,
    Unsupported,
    Serialization,
    Io,
    Backend,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CanvasPortError {
    pub kind: CanvasPortErrorKind,
    pub message: String,
}

impl CanvasPortError {
    pub fn new(kind: CanvasPortErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }
}

impl std::fmt::Display for CanvasPortError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}: {}", self.kind, self.message)
    }
}

impl std::error::Error for CanvasPortError {}

pub trait CanvasStoragePort: Send + Sync {
    fn save_source(
        &self,
        artifact: CanvasArtifact,
        source: CanvasSource,
        diagnostics: Vec<CanvasDiagnostic>,
    ) -> CanvasPortFuture<'_, CanvasSnapshot>;

    fn load_snapshot(
        &self,
        session_id: CanvasSessionId,
        canvas_id: CanvasId,
    ) -> CanvasPortFuture<'_, CanvasSnapshot>;

    fn list_session_artifacts(
        &self,
        session_id: CanvasSessionId,
    ) -> CanvasPortFuture<'_, Vec<CanvasArtifact>>;

    fn compile_latest(
        &self,
        session_id: CanvasSessionId,
        canvas_id: CanvasId,
        compiled_at: i64,
    ) -> CanvasPortFuture<'_, CanvasCompileResult>;

    fn save_compiled_payload(
        &self,
        session_id: CanvasSessionId,
        payload: CanvasCompiledPayload,
    ) -> CanvasPortFuture<'_, CanvasSnapshot>;

    fn report_runtime_diagnostic(
        &self,
        session_id: CanvasSessionId,
        canvas_id: CanvasId,
        diagnostic: CanvasDiagnostic,
    ) -> CanvasPortFuture<'_, CanvasSnapshot>;

    fn load_state(
        &self,
        session_id: CanvasSessionId,
        canvas_id: CanvasId,
    ) -> CanvasPortFuture<'_, Option<CanvasState>>;

    fn save_state(
        &self,
        session_id: CanvasSessionId,
        state: CanvasState,
    ) -> CanvasPortFuture<'_, CanvasState>;
}
