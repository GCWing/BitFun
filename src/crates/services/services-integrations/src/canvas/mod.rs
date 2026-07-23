//! Canvas runtime service.
//!
//! This module implements the Canvas product-domain storage port, TSX compiler
//! wiring, last-known-good payload handling, and session-scoped file
//! persistence behind the `canvas-runtime` integration feature.

mod compiler;

use bitfun_product_domains::canvas::{
    is_safe_canvas_ref_segment, CanvasArtifact, CanvasCompileResult, CanvasCompiledPayload,
    CanvasDiagnostic, CanvasDiagnosticSeverity, CanvasId, CanvasPortError, CanvasPortErrorKind,
    CanvasPortFuture, CanvasPortResult, CanvasSessionId, CanvasSnapshot, CanvasSource, CanvasState,
    CanvasStatus, CanvasStoragePort,
};
pub use compiler::{compile_canvas_component_js, compile_canvas_html, compile_canvas_source};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

pub type CanvasService = CanvasMemoryStore;

#[derive(Debug, Clone, Default)]
pub struct CanvasMemoryStore {
    inner: Arc<Mutex<CanvasMemoryStoreState>>,
    sessions_dir: Option<PathBuf>,
}

#[derive(Debug, Default)]
struct CanvasMemoryStoreState {
    snapshots: BTreeMap<(CanvasSessionId, CanvasId), CanvasSnapshot>,
}

impl CanvasMemoryStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn persistent(sessions_dir: impl Into<PathBuf>) -> Self {
        Self {
            inner: Arc::new(Mutex::new(CanvasMemoryStoreState::default())),
            sessions_dir: Some(sessions_dir.into()),
        }
    }

    pub fn capability_status() -> bitfun_product_domains::canvas::CanvasCapabilityStatus {
        bitfun_product_domains::canvas::CanvasCapabilityStatus::supported()
    }

    fn key(session_id: CanvasSessionId, canvas_id: CanvasId) -> (CanvasSessionId, CanvasId) {
        (session_id, canvas_id)
    }

    fn missing(session_id: &CanvasSessionId, canvas_id: &CanvasId) -> CanvasPortError {
        CanvasPortError::new(
            CanvasPortErrorKind::NotFound,
            format!(
                "Canvas '{}' was not found in session '{}'",
                canvas_id.as_str(),
                session_id.as_str()
            ),
        )
    }

    fn validate_path_segment(kind: &str, value: &str) -> CanvasPortResult<()> {
        if is_safe_canvas_ref_segment(value) {
            return Ok(());
        }
        Err(CanvasPortError::new(
            CanvasPortErrorKind::InvalidInput,
            format!("Canvas {kind} contains unsafe path characters"),
        ))
    }

    fn canvas_dir_for(root: &Path, session_id: &CanvasSessionId) -> CanvasPortResult<PathBuf> {
        Self::validate_path_segment("session id", session_id.as_str())?;
        Ok(root.join(session_id.as_str()).join("canvases"))
    }

    fn canvas_path_for(
        root: &Path,
        session_id: &CanvasSessionId,
        canvas_id: &CanvasId,
    ) -> CanvasPortResult<PathBuf> {
        Self::validate_path_segment("canvas id", canvas_id.as_str())?;
        Ok(Self::canvas_dir_for(root, session_id)?
            .join(format!("{}.json", urlencoding::encode(canvas_id.as_str()))))
    }

    async fn persist_snapshot(&self, snapshot: &CanvasSnapshot) -> CanvasPortResult<()> {
        let Some(sessions_dir) = self.sessions_dir.as_ref() else {
            return Ok(());
        };
        let canvas_dir = Self::canvas_dir_for(sessions_dir, &snapshot.artifact.session_id)?;
        tokio::fs::create_dir_all(&canvas_dir)
            .await
            .map_err(|error| canvas_backend_error("create Canvas directory", &canvas_dir, error))?;
        let path = Self::canvas_path_for(
            sessions_dir,
            &snapshot.artifact.session_id,
            &snapshot.artifact.id,
        )?;
        let bytes = serde_json::to_vec_pretty(snapshot).map_err(|error| {
            CanvasPortError::new(
                CanvasPortErrorKind::Backend,
                format!("Failed to serialize Canvas snapshot: {}", error),
            )
        })?;
        tokio::fs::write(&path, bytes)
            .await
            .map_err(|error| canvas_backend_error("write Canvas snapshot", &path, error))?;
        Ok(())
    }

    async fn load_persisted_snapshot(
        &self,
        session_id: &CanvasSessionId,
        canvas_id: &CanvasId,
    ) -> CanvasPortResult<Option<CanvasSnapshot>> {
        let Some(sessions_dir) = self.sessions_dir.as_ref() else {
            return Ok(None);
        };
        let path = Self::canvas_path_for(sessions_dir, session_id, canvas_id)?;
        let bytes = match tokio::fs::read(&path).await {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => {
                return Err(canvas_backend_error("read Canvas snapshot", &path, error));
            }
        };
        let snapshot = serde_json::from_slice::<CanvasSnapshot>(&bytes).map_err(|error| {
            CanvasPortError::new(
                CanvasPortErrorKind::Backend,
                format!(
                    "Failed to parse Canvas snapshot '{}': {}",
                    path.display(),
                    error
                ),
            )
        })?;
        Ok(Some(snapshot))
    }

    async fn cache_snapshot(&self, snapshot: CanvasSnapshot) -> CanvasPortResult<CanvasSnapshot> {
        let key = Self::key(
            snapshot.artifact.session_id.clone(),
            snapshot.artifact.id.clone(),
        );
        let mut state = self.inner.lock().map_err(|_| {
            CanvasPortError::new(CanvasPortErrorKind::Backend, "Canvas store lock poisoned")
        })?;
        state.snapshots.insert(key, snapshot.clone());
        Ok(snapshot)
    }

    async fn update_diagnostics(
        &self,
        session_id: CanvasSessionId,
        canvas_id: CanvasId,
        diagnostics: Vec<CanvasDiagnostic>,
        status: CanvasStatus,
    ) -> CanvasPortResult<CanvasSnapshot> {
        let snapshot = {
            let mut state = self.inner.lock().map_err(|_| {
                CanvasPortError::new(CanvasPortErrorKind::Backend, "Canvas store lock poisoned")
            })?;
            let key = Self::key(session_id.clone(), canvas_id.clone());
            let Some(snapshot) = state.snapshots.get_mut(&key) else {
                return Err(Self::missing(&session_id, &canvas_id));
            };
            snapshot.diagnostics = diagnostics;
            snapshot.artifact.status = status;
            snapshot.clone()
        };
        self.persist_snapshot(&snapshot).await?;
        Ok(snapshot)
    }
}

fn canvas_backend_error(action: &str, path: &Path, error: std::io::Error) -> CanvasPortError {
    CanvasPortError::new(
        CanvasPortErrorKind::Backend,
        format!("Failed to {} '{}': {}", action, path.display(), error),
    )
}

impl CanvasStoragePort for CanvasMemoryStore {
    fn save_source(
        &self,
        artifact: CanvasArtifact,
        source: CanvasSource,
        diagnostics: Vec<bitfun_product_domains::canvas::CanvasDiagnostic>,
    ) -> CanvasPortFuture<'_, CanvasSnapshot> {
        let store = self.clone();
        Box::pin(async move {
            if artifact.id != source.canvas_id {
                return Err(CanvasPortError::new(
                    CanvasPortErrorKind::InvalidInput,
                    "Canvas source id must match artifact id",
                ));
            }

            let snapshot = {
                let key = Self::key(artifact.session_id.clone(), artifact.id.clone());
                let mut state = store.inner.lock().map_err(|_| {
                    CanvasPortError::new(CanvasPortErrorKind::Backend, "Canvas store lock poisoned")
                })?;
                let previous = state.snapshots.get(&key);
                let mut artifact = artifact;
                artifact.status = if diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.severity == CanvasDiagnosticSeverity::Error)
                {
                    CanvasStatus::CompileFailed
                } else {
                    artifact.status
                };
                artifact.latest_compiled_revision = previous
                    .and_then(|snapshot| snapshot.artifact.latest_compiled_revision.clone())
                    .or(artifact.latest_compiled_revision);
                artifact.last_known_good_revision = previous
                    .and_then(|snapshot| snapshot.artifact.last_known_good_revision.clone())
                    .or(artifact.last_known_good_revision);
                let snapshot = CanvasSnapshot {
                    artifact,
                    source,
                    diagnostics,
                    compiled_payload: previous
                        .and_then(|snapshot| snapshot.compiled_payload.clone()),
                    state: previous.and_then(|snapshot| snapshot.state.clone()),
                };
                state.snapshots.insert(key, snapshot.clone());
                snapshot
            };
            store.persist_snapshot(&snapshot).await?;
            Ok(snapshot)
        })
    }

    fn load_snapshot(
        &self,
        session_id: CanvasSessionId,
        canvas_id: CanvasId,
    ) -> CanvasPortFuture<'_, CanvasSnapshot> {
        let store = self.clone();
        Box::pin(async move {
            if let Some(snapshot) = {
                let state = store.inner.lock().map_err(|_| {
                    CanvasPortError::new(CanvasPortErrorKind::Backend, "Canvas store lock poisoned")
                })?;
                state
                    .snapshots
                    .get(&Self::key(session_id.clone(), canvas_id.clone()))
                    .cloned()
            } {
                return Ok(snapshot);
            }

            if let Some(snapshot) = store
                .load_persisted_snapshot(&session_id, &canvas_id)
                .await?
            {
                return store.cache_snapshot(snapshot).await;
            }

            Err(Self::missing(&session_id, &canvas_id))
        })
    }

    fn list_session_artifacts(
        &self,
        session_id: CanvasSessionId,
    ) -> CanvasPortFuture<'_, Vec<CanvasArtifact>> {
        let store = self.clone();
        Box::pin(async move {
            let mut artifacts = {
                let state = store.inner.lock().map_err(|_| {
                    CanvasPortError::new(CanvasPortErrorKind::Backend, "Canvas store lock poisoned")
                })?;
                state
                    .snapshots
                    .iter()
                    .filter(|((stored_session_id, _), _)| stored_session_id == &session_id)
                    .map(|(_, snapshot)| snapshot.artifact.clone())
                    .collect::<Vec<_>>()
            };

            if let Some(sessions_dir) = store.sessions_dir.as_ref() {
                let canvas_dir = Self::canvas_dir_for(sessions_dir, &session_id)?;
                match tokio::fs::read_dir(&canvas_dir).await {
                    Ok(mut entries) => {
                        while let Some(entry) = entries.next_entry().await.map_err(|error| {
                            canvas_backend_error("read Canvas directory entry", &canvas_dir, error)
                        })? {
                            let path = entry.path();
                            if path.extension().and_then(|value| value.to_str()) != Some("json") {
                                continue;
                            }
                            let bytes = tokio::fs::read(&path).await.map_err(|error| {
                                canvas_backend_error("read Canvas snapshot", &path, error)
                            })?;
                            let snapshot = serde_json::from_slice::<CanvasSnapshot>(&bytes)
                                .map_err(|error| {
                                    CanvasPortError::new(
                                        CanvasPortErrorKind::Backend,
                                        format!(
                                            "Failed to parse Canvas snapshot '{}': {}",
                                            path.display(),
                                            error
                                        ),
                                    )
                                })?;
                            store.cache_snapshot(snapshot.clone()).await?;
                            if !artifacts
                                .iter()
                                .any(|artifact| artifact.id == snapshot.artifact.id)
                            {
                                artifacts.push(snapshot.artifact);
                            }
                        }
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                    Err(error) => {
                        return Err(canvas_backend_error(
                            "read Canvas directory",
                            &canvas_dir,
                            error,
                        ));
                    }
                }
            }

            artifacts.sort_by_key(|artifact| artifact.created_at);
            Ok(artifacts)
        })
    }

    fn compile_latest(
        &self,
        session_id: CanvasSessionId,
        canvas_id: CanvasId,
        compiled_at: i64,
    ) -> CanvasPortFuture<'_, CanvasCompileResult> {
        let store = self.clone();
        Box::pin(async move {
            let snapshot = store
                .load_snapshot(session_id.clone(), canvas_id.clone())
                .await?;
            let result = compile_canvas_source(&snapshot.source, compiled_at);
            if !result.compiled {
                store
                    .update_diagnostics(
                        session_id,
                        canvas_id,
                        result.diagnostics.clone(),
                        CanvasStatus::CompileFailed,
                    )
                    .await?;
                return Ok(result);
            }

            if let Some(payload) = result.payload.clone() {
                store.save_compiled_payload(session_id, payload).await?;
            }

            Ok(result)
        })
    }

    fn save_compiled_payload(
        &self,
        session_id: CanvasSessionId,
        payload: CanvasCompiledPayload,
    ) -> CanvasPortFuture<'_, CanvasSnapshot> {
        let store = self.clone();
        Box::pin(async move {
            let snapshot = {
                let mut state = store.inner.lock().map_err(|_| {
                    CanvasPortError::new(CanvasPortErrorKind::Backend, "Canvas store lock poisoned")
                })?;
                let key = Self::key(session_id.clone(), payload.canvas_id.clone());
                let Some(snapshot) = state.snapshots.get_mut(&key) else {
                    return Err(Self::missing(&session_id, &payload.canvas_id));
                };
                snapshot.compiled_payload = Some(payload.clone());
                snapshot.artifact.latest_compiled_revision = Some(payload.source_revision.clone());
                snapshot.artifact.last_known_good_revision = Some(payload.source_revision);
                snapshot.artifact.status = CanvasStatus::Compiled;
                snapshot.clone()
            };
            store.persist_snapshot(&snapshot).await?;
            Ok(snapshot)
        })
    }

    fn report_runtime_diagnostic(
        &self,
        session_id: CanvasSessionId,
        canvas_id: CanvasId,
        diagnostic: CanvasDiagnostic,
    ) -> CanvasPortFuture<'_, CanvasSnapshot> {
        let store = self.clone();
        Box::pin(async move {
            store
                .load_snapshot(session_id.clone(), canvas_id.clone())
                .await?;
            let snapshot = {
                let mut state = store.inner.lock().map_err(|_| {
                    CanvasPortError::new(CanvasPortErrorKind::Backend, "Canvas store lock poisoned")
                })?;
                let key = Self::key(session_id.clone(), canvas_id.clone());
                let Some(snapshot) = state.snapshots.get_mut(&key) else {
                    return Err(Self::missing(&session_id, &canvas_id));
                };
                snapshot
                    .diagnostics
                    .retain(|existing| existing.code.as_deref() != diagnostic.code.as_deref());
                snapshot.diagnostics.push(diagnostic);
                snapshot.artifact.status = CanvasStatus::RuntimeFailed;
                snapshot.clone()
            };
            store.persist_snapshot(&snapshot).await?;
            Ok(snapshot)
        })
    }

    fn load_state(
        &self,
        session_id: CanvasSessionId,
        canvas_id: CanvasId,
    ) -> CanvasPortFuture<'_, Option<CanvasState>> {
        let store = self.clone();
        Box::pin(async move {
            if let Some(state) = {
                let state = store.inner.lock().map_err(|_| {
                    CanvasPortError::new(CanvasPortErrorKind::Backend, "Canvas store lock poisoned")
                })?;
                state
                    .snapshots
                    .get(&Self::key(session_id.clone(), canvas_id.clone()))
                    .and_then(|snapshot| snapshot.state.clone())
            } {
                return Ok(Some(state));
            }

            Ok(store
                .load_persisted_snapshot(&session_id, &canvas_id)
                .await?
                .and_then(|snapshot| snapshot.state))
        })
    }

    fn save_state(
        &self,
        session_id: CanvasSessionId,
        canvas_state: CanvasState,
    ) -> CanvasPortFuture<'_, CanvasState> {
        let store = self.clone();
        Box::pin(async move {
            store
                .load_snapshot(session_id.clone(), canvas_state.canvas_id.clone())
                .await?;
            let snapshot = {
                let mut state = store.inner.lock().map_err(|_| {
                    CanvasPortError::new(CanvasPortErrorKind::Backend, "Canvas store lock poisoned")
                })?;
                let key = Self::key(session_id.clone(), canvas_state.canvas_id.clone());
                let Some(snapshot) = state.snapshots.get_mut(&key) else {
                    return Err(Self::missing(&session_id, &canvas_state.canvas_id));
                };
                snapshot.state = Some(canvas_state.clone());
                snapshot.clone()
            };
            store.persist_snapshot(&snapshot).await?;
            Ok(canvas_state)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bitfun_product_domains::canvas::{
        CanvasDiagnosticCategory, CanvasDiagnosticSeverity, CanvasRevision, CanvasScope,
        CanvasWorkspaceId,
    };
    use std::collections::BTreeMap;

    fn sample_artifact(canvas_id: &str, session_id: &str) -> CanvasArtifact {
        CanvasArtifact {
            id: CanvasId::new(canvas_id),
            scope: CanvasScope::Session,
            session_id: CanvasSessionId::new(session_id),
            workspace_id: CanvasWorkspaceId::new("workspace_1"),
            title: "Canvas".to_string(),
            description: None,
            source_revision: CanvasRevision::new("rev_1"),
            latest_compiled_revision: None,
            last_known_good_revision: None,
            status: CanvasStatus::SourceSaved,
            created_at: 1,
            updated_at: 1,
        }
    }

    fn sample_source(canvas_id: &str) -> CanvasSource {
        CanvasSource::new_tsx(
            CanvasId::new(canvas_id),
            CanvasRevision::new("rev_1"),
            "canvas.tsx",
            "import { Stack } from 'bitfun/canvas'; export default function C() { return <Stack />; }",
            "0.1.0",
            1,
        )
    }

    #[tokio::test]
    async fn canvas_store_saves_loads_and_lists_session_artifacts() {
        let store = CanvasMemoryStore::new();
        let artifact = sample_artifact("canvas_1", "session_1");
        let source = sample_source("canvas_1");

        let saved = store
            .save_source(artifact.clone(), source.clone(), Vec::new())
            .await
            .expect("source should save");

        assert_eq!(saved.artifact, artifact);
        assert_eq!(saved.source, source);

        let loaded = store
            .load_snapshot(CanvasSessionId::new("session_1"), CanvasId::new("canvas_1"))
            .await
            .expect("snapshot should load");
        assert_eq!(loaded.artifact.title, "Canvas");

        let listed = store
            .list_session_artifacts(CanvasSessionId::new("session_1"))
            .await
            .expect("session artifacts should list");
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].id.as_str(), "canvas_1");
    }

    #[tokio::test]
    async fn canvas_store_rejects_source_for_different_artifact() {
        let store = CanvasMemoryStore::new();
        let error = store
            .save_source(
                sample_artifact("canvas_1", "session_1"),
                sample_source("canvas_2"),
                Vec::new(),
            )
            .await
            .expect_err("mismatched source id must fail");

        assert_eq!(error.kind, CanvasPortErrorKind::InvalidInput);
    }

    #[tokio::test]
    async fn canvas_store_preserves_sidecar_state_across_source_update() {
        let store = CanvasMemoryStore::new();
        store
            .save_source(
                sample_artifact("canvas_1", "session_1"),
                sample_source("canvas_1"),
                Vec::new(),
            )
            .await
            .unwrap();

        let mut values = BTreeMap::new();
        values.insert("filter".to_string(), serde_json::json!("failed"));
        store
            .save_state(
                CanvasSessionId::new("session_1"),
                CanvasState {
                    canvas_id: CanvasId::new("canvas_1"),
                    source_revision_seen: Some(CanvasRevision::new("rev_1")),
                    values,
                    updated_at: 2,
                    schema_version: 1,
                },
            )
            .await
            .unwrap();

        let mut updated_artifact = sample_artifact("canvas_1", "session_1");
        updated_artifact.source_revision = CanvasRevision::new("rev_2");
        let mut updated_source = sample_source("canvas_1");
        updated_source.revision = CanvasRevision::new("rev_2");
        store
            .save_source(
                updated_artifact,
                updated_source,
                vec![CanvasDiagnostic {
                    severity: CanvasDiagnosticSeverity::Warning,
                    category: CanvasDiagnosticCategory::ImportPolicy,
                    message: "warning".to_string(),
                    code: None,
                    line: None,
                    column: None,
                    suggested_fix: None,
                }],
            )
            .await
            .unwrap();

        let state = store
            .load_state(CanvasSessionId::new("session_1"), CanvasId::new("canvas_1"))
            .await
            .unwrap()
            .expect("state should survive source update");
        assert_eq!(state.values["filter"], "failed");
    }

    #[tokio::test]
    async fn canvas_store_persists_snapshots_by_session() {
        let sessions_dir =
            std::env::temp_dir().join(format!("bitfun-canvas-store-test-{}", uuid::Uuid::new_v4()));
        let store = CanvasMemoryStore::persistent(&sessions_dir);
        store
            .save_source(
                sample_artifact("canvas_1", "session_1"),
                sample_source("canvas_1"),
                Vec::new(),
            )
            .await
            .expect("source should save");
        store
            .compile_latest(
                CanvasSessionId::new("session_1"),
                CanvasId::new("canvas_1"),
                2,
            )
            .await
            .expect("compile should save payload");

        let reloaded = CanvasMemoryStore::persistent(&sessions_dir)
            .load_snapshot(CanvasSessionId::new("session_1"), CanvasId::new("canvas_1"))
            .await
            .expect("persisted snapshot should load");

        assert_eq!(reloaded.artifact.id.as_str(), "canvas_1");
        assert!(reloaded.compiled_payload.is_some());

        let listed = CanvasMemoryStore::persistent(&sessions_dir)
            .list_session_artifacts(CanvasSessionId::new("session_1"))
            .await
            .expect("persisted session artifacts should list");
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].id.as_str(), "canvas_1");

        let mut values = BTreeMap::new();
        values.insert("mode".to_string(), serde_json::json!("compact"));
        CanvasMemoryStore::persistent(&sessions_dir)
            .save_state(
                CanvasSessionId::new("session_1"),
                CanvasState {
                    canvas_id: CanvasId::new("canvas_1"),
                    source_revision_seen: Some(CanvasRevision::new("rev_1")),
                    values,
                    updated_at: 3,
                    schema_version: 1,
                },
            )
            .await
            .expect("state should save through a fresh persistent service");
        let state = CanvasMemoryStore::persistent(&sessions_dir)
            .load_state(CanvasSessionId::new("session_1"), CanvasId::new("canvas_1"))
            .await
            .expect("state should load")
            .expect("state should exist");
        assert_eq!(state.values["mode"], "compact");

        let _ = std::fs::remove_dir_all(sessions_dir);
    }

    #[tokio::test]
    async fn persistent_canvas_store_rejects_unsafe_path_segments() {
        let sessions_dir =
            std::env::temp_dir().join(format!("bitfun-canvas-store-test-{}", uuid::Uuid::new_v4()));
        let store = CanvasMemoryStore::persistent(&sessions_dir);

        let error = store
            .save_source(
                sample_artifact("canvas_1", "../session_1"),
                sample_source("canvas_1"),
                Vec::new(),
            )
            .await
            .expect_err("unsafe session id should fail");
        assert_eq!(error.kind, CanvasPortErrorKind::InvalidInput);

        let error = store
            .load_snapshot(
                CanvasSessionId::new("session_1"),
                CanvasId::new("canvas/with/slash"),
            )
            .await
            .expect_err("unsafe canvas id should fail");
        assert_eq!(error.kind, CanvasPortErrorKind::InvalidInput);

        let _ = std::fs::remove_dir_all(sessions_dir);
    }

    #[tokio::test]
    async fn canvas_compile_latest_rejects_policy_errors_and_keeps_last_known_good() {
        let store = CanvasMemoryStore::new();
        store
            .save_source(
                sample_artifact("canvas_1", "session_1"),
                sample_source("canvas_1"),
                Vec::new(),
            )
            .await
            .unwrap();
        let first = store
            .compile_latest(
                CanvasSessionId::new("session_1"),
                CanvasId::new("canvas_1"),
                2,
            )
            .await
            .unwrap();
        assert!(first.compiled);
        assert!(first.payload.is_some());

        let mut updated_artifact = sample_artifact("canvas_1", "session_1");
        updated_artifact.source_revision = CanvasRevision::new("rev_2");
        let mut updated_source = sample_source("canvas_1");
        updated_source.revision = CanvasRevision::new("rev_2");
        updated_source.source =
            "import helper from './helper'; export default function C() { return null; }"
                .to_string();
        store
            .save_source(updated_artifact, updated_source, Vec::new())
            .await
            .unwrap();

        let failed = store
            .compile_latest(
                CanvasSessionId::new("session_1"),
                CanvasId::new("canvas_1"),
                3,
            )
            .await
            .unwrap();
        assert!(!failed.compiled);
        assert!(failed.payload.is_none());
        assert_eq!(
            failed.diagnostics[0].category,
            CanvasDiagnosticCategory::ImportPolicy
        );

        let snapshot = store
            .load_snapshot(CanvasSessionId::new("session_1"), CanvasId::new("canvas_1"))
            .await
            .unwrap();
        assert_eq!(snapshot.artifact.status, CanvasStatus::CompileFailed);
        assert_eq!(
            snapshot
                .artifact
                .last_known_good_revision
                .as_ref()
                .map(CanvasRevision::as_str),
            Some("rev_1")
        );
        assert_eq!(
            snapshot
                .compiled_payload
                .as_ref()
                .map(|payload| payload.source_revision.as_str()),
            Some("rev_1")
        );
    }

    #[tokio::test]
    async fn canvas_store_records_runtime_diagnostics() {
        let store = CanvasMemoryStore::new();
        store
            .save_source(
                sample_artifact("canvas_1", "session_1"),
                sample_source("canvas_1"),
                Vec::new(),
            )
            .await
            .expect("canvas should save");

        let snapshot = store
            .report_runtime_diagnostic(
                CanvasSessionId::new("session_1"),
                CanvasId::new("canvas_1"),
                CanvasDiagnostic {
                    severity: CanvasDiagnosticSeverity::Error,
                    category: CanvasDiagnosticCategory::Runtime,
                    message: "layout is not iterable".to_string(),
                    code: Some("canvas.runtime.error".to_string()),
                    line: None,
                    column: None,
                    suggested_fix: None,
                },
            )
            .await
            .expect("runtime diagnostic should save");

        assert_eq!(snapshot.artifact.status, CanvasStatus::RuntimeFailed);
        assert_eq!(snapshot.diagnostics.len(), 1);
        assert_eq!(
            snapshot.diagnostics[0].category,
            CanvasDiagnosticCategory::Runtime
        );
    }

    #[test]
    fn canvas_compile_source_builds_sandboxed_runtime_payload() {
        let mut source = sample_source("canvas_1");
        source.source =
            "export default function C() { return <Text>{\"</script><unsafe>\"}</Text>; }"
                .to_string();

        let result = compile_canvas_source(&source, 2);
        assert!(result.compiled, "{:?}", result.diagnostics);
        let html = result.payload.unwrap().html;

        assert!(html.contains("bitfun-canvas-root"));
        assert!(html.contains("BitfunCanvasRuntime.mount"));
        assert!(html.contains("connect-src 'none'"));
        assert!(!html.contains("</script><unsafe>"));
    }
}
