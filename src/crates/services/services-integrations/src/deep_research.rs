//! Citation renumbering hook for finalized DeepResearch reports.
//!
//! This module owns the best-effort filesystem hook and sidecar persistence.
//! The deterministic report rewrite stays in `bitfun-agent-workflows`.

use bitfun_agent_workflows::deep_research::{
    renumber_research_report, ResearchCitationDisplayMapEntry,
};
use bitfun_runtime_ports::WorkspaceFileSystem;
use log::{debug, info, warn};
use serde_json::json;
use std::fmt;
use std::path::PathBuf;

#[derive(Debug)]
pub enum DeepResearchReportIoError {
    ReadReport(std::io::Error),
    WriteReport(std::io::Error),
    SerializeDisplayMap(serde_json::Error),
    WriteDisplayMap {
        path: PathBuf,
        source: std::io::Error,
    },
}

impl fmt::Display for DeepResearchReportIoError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ReadReport(source) => write!(f, "read report failed: {source}"),
            Self::WriteReport(source) => write!(f, "write report failed: {source}"),
            Self::SerializeDisplayMap(source) => {
                write!(f, "serialize display_map.json failed: {source}")
            }
            Self::WriteDisplayMap { path, source } => {
                write!(f, "write {} failed: {source}", path.display())
            }
        }
    }
}

impl std::error::Error for DeepResearchReportIoError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::ReadReport(source) | Self::WriteReport(source) => Some(source),
            Self::SerializeDisplayMap(source) => Some(source),
            Self::WriteDisplayMap { source, .. } => Some(source),
        }
    }
}

pub type DeepResearchReportIoResult<T> = Result<T, DeepResearchReportIoError>;

/// Outcome summary returned to the caller for logging / telemetry.
#[derive(Debug, Default, Clone)]
pub struct RenumberStats {
    pub citations_renumbered: usize,
    pub rejected_refs_in_body: usize,
}

/// Best-effort entry point. Logs and swallows errors so callers can safely
/// fire-and-await without affecting the surrounding agent flow.
///
/// Operates on the per-session WORK_DIR at
/// `<workspace>/.bitfun/sessions/<session_id>/research/`, where both the
/// report and the audit files live.
pub async fn run_for_session_workspace(
    fs: &dyn WorkspaceFileSystem,
    workspace_root: &str,
    session_id: &str,
) {
    let work_dir = fs.join_path(
        workspace_root,
        &[".bitfun", "sessions", session_id, "research"],
    );
    let report_path = fs.join_path(&work_dir, &["report.md"]);

    match fs.exists(&report_path).await {
        Ok(false) => {
            debug!(
                "citation_renumber: {} not found, nothing to renumber",
                report_path
            );
            return;
        }
        Err(error) => {
            warn!(
                "citation_renumber: skipped (best-effort failure): path={}, err=check report existence failed: {}",
                report_path, error
            );
            return;
        }
        Ok(true) => {}
    }

    match try_renumber_research_report(fs, &report_path, &work_dir).await {
        Ok(stats) if stats.citations_renumbered == 0 => {
            debug!(
                "citation_renumber: no cit_XXX references found in {}; skipping",
                report_path
            );
        }
        Ok(stats) => {
            info!(
                "citation_renumber: renumbered {} citations in {} ({} rejected refs in body)",
                stats.citations_renumbered, report_path, stats.rejected_refs_in_body
            );
        }
        Err(err) => {
            warn!(
                "citation_renumber: skipped (best-effort failure): path={}, err={}",
                report_path, err
            );
        }
    }
}

/// Renumber `cit_XXX` references in `report_path` in place.
///
/// `work_dir` is the session's research/ directory; it is consulted for the
/// citation registry's `status=ACCEPTED|REJECTED` flags so REJECTED rows can
/// be skipped during numbering.
pub async fn try_renumber_research_report(
    fs: &dyn WorkspaceFileSystem,
    report_path: &str,
    work_dir: &str,
) -> DeepResearchReportIoResult<RenumberStats> {
    if !fs
        .exists(report_path)
        .await
        .map_err(|error| DeepResearchReportIoError::ReadReport(workspace_io_error(error)))?
    {
        return Ok(RenumberStats::default());
    }

    let report = fs
        .read_file_text(report_path)
        .await
        .map_err(|error| DeepResearchReportIoError::ReadReport(workspace_io_error(error)))?;

    let registry_path = fs.join_path(work_dir, &["citations.md"]);
    let registry_content = match fs.exists(&registry_path).await {
        Ok(true) => match fs.read_file_text(&registry_path).await {
            Ok(content) => Some(content),
            Err(error) => {
                warn!(
                    "citation_renumber: failed to read citations.md ({}): {}",
                    registry_path, error
                );
                None
            }
        },
        Ok(false) => None,
        Err(error) => {
            warn!(
                "citation_renumber: failed to inspect citations.md ({}): {}",
                registry_path, error
            );
            None
        }
    };

    let output = renumber_research_report(&report, registry_content.as_deref());

    if output.display_map.is_empty() {
        debug!(
            "citation_renumber: no eligible cit_XXX references in {}",
            report_path
        );
        return Ok(RenumberStats {
            citations_renumbered: output.stats.citations_renumbered,
            rejected_refs_in_body: output.stats.rejected_refs_in_body,
        });
    }

    fs.write_file(report_path, output.report.as_bytes())
        .await
        .map_err(|error| DeepResearchReportIoError::WriteReport(workspace_io_error(error)))?;

    if output.stats.rejected_index_rows_dropped > 0 {
        warn!(
            "citation_renumber: dropped {} REJECTED row(s) from the Citation Index; full registry remains in citations.md",
            output.stats.rejected_index_rows_dropped
        );
    }

    if let Err(error) =
        write_display_map_sidecar(fs, work_dir, report_path, &output.display_map).await
    {
        warn!("citation_renumber: {error}");
    }

    Ok(RenumberStats {
        citations_renumbered: output.stats.citations_renumbered,
        rejected_refs_in_body: output.stats.rejected_refs_in_body,
    })
}

async fn write_display_map_sidecar(
    fs: &dyn WorkspaceFileSystem,
    parent: &str,
    report_path: &str,
    display_map: &[ResearchCitationDisplayMapEntry],
) -> DeepResearchReportIoResult<String> {
    let map_path = fs.join_path(parent, &["display_map.json"]);
    let entries = display_map
        .iter()
        .map(|entry| {
            json!({
                "display": entry.display,
                "internal": entry.internal,
            })
        })
        .collect::<Vec<_>>();
    let body = json!({
        "version": 1,
        "report_path": report_path,
        "citation_count": display_map.len(),
        "entries": entries,
    });
    let serialized = serde_json::to_string_pretty(&body)
        .map_err(DeepResearchReportIoError::SerializeDisplayMap)?;
    fs.write_file(&map_path, serialized.as_bytes())
        .await
        .map_err(|source| DeepResearchReportIoError::WriteDisplayMap {
            path: PathBuf::from(&map_path),
            source: workspace_io_error(source),
        })?;
    Ok(map_path)
}

fn workspace_io_error(error: anyhow::Error) -> std::io::Error {
    match error.downcast::<std::io::Error>() {
        Ok(error) => error,
        Err(error) => std::io::Error::other(error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bitfun_runtime_ports::{WorkspaceDirEntry, WorkspaceFileSystem, WorkspacePathKind};
    use std::collections::HashMap;
    use std::env;
    use std::path::Path;
    use std::path::PathBuf;
    use std::sync::{Arc, Mutex};

    struct HostWorkspaceFs;

    #[async_trait::async_trait]
    impl WorkspaceFileSystem for HostWorkspaceFs {
        async fn read_file(&self, path: &str) -> anyhow::Result<Vec<u8>> {
            Ok(std::fs::read(path)?)
        }

        async fn read_file_text(&self, path: &str) -> anyhow::Result<String> {
            Ok(std::fs::read_to_string(path)?)
        }

        async fn write_file(&self, path: &str, contents: &[u8]) -> anyhow::Result<()> {
            if let Some(parent) = Path::new(path).parent() {
                std::fs::create_dir_all(parent)?;
            }
            Ok(std::fs::write(path, contents)?)
        }

        async fn exists(&self, path: &str) -> anyhow::Result<bool> {
            Ok(Path::new(path).try_exists()?)
        }

        async fn is_file(&self, path: &str) -> anyhow::Result<bool> {
            Ok(std::fs::metadata(path)
                .map(|metadata| metadata.is_file())
                .unwrap_or(false))
        }

        async fn is_dir(&self, path: &str) -> anyhow::Result<bool> {
            Ok(std::fs::metadata(path)
                .map(|metadata| metadata.is_dir())
                .unwrap_or(false))
        }

        async fn read_dir(&self, path: &str) -> anyhow::Result<Vec<WorkspaceDirEntry>> {
            let mut entries = Vec::new();
            let read_dir = std::fs::read_dir(path)?;
            for entry in read_dir {
                let entry = entry?;
                let metadata = entry.metadata()?;
                entries.push(WorkspaceDirEntry {
                    name: entry.file_name().to_string_lossy().to_string(),
                    path: entry.path().to_string_lossy().to_string(),
                    is_dir: metadata.is_dir(),
                    is_symlink: metadata.file_type().is_symlink(),
                    modified: metadata.modified().ok(),
                });
            }
            Ok(entries)
        }
    }

    #[derive(Clone, Default)]
    struct RecordingWorkspaceFs {
        files: Arc<Mutex<HashMap<String, Vec<u8>>>>,
        operations: Arc<Mutex<Vec<String>>>,
    }

    impl RecordingWorkspaceFs {
        fn insert(&self, path: &str, contents: impl Into<Vec<u8>>) {
            self.files
                .lock()
                .unwrap()
                .insert(path.to_string(), contents.into());
        }

        fn text(&self, path: &str) -> Option<String> {
            self.files
                .lock()
                .unwrap()
                .get(path)
                .map(|contents| String::from_utf8(contents.clone()).unwrap())
        }

        fn observed_operations(&self) -> Vec<String> {
            self.operations.lock().unwrap().clone()
        }

        fn record(&self, operation: &str, path: &str) {
            self.operations
                .lock()
                .unwrap()
                .push(format!("{operation}:{path}"));
        }
    }

    #[async_trait::async_trait]
    impl WorkspaceFileSystem for RecordingWorkspaceFs {
        fn join_path(&self, root: &str, components: &[&str]) -> String {
            let mut path = root.trim_end_matches('/').to_string();
            if path.is_empty() && root.starts_with('/') {
                path.push('/');
            }
            for component in components {
                if !path.is_empty() && !path.ends_with('/') {
                    path.push('/');
                }
                path.push_str(component.trim_matches('/'));
            }
            path
        }

        async fn read_file(&self, path: &str) -> anyhow::Result<Vec<u8>> {
            self.record("read", path);
            self.files
                .lock()
                .unwrap()
                .get(path)
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("missing test file: {path}"))
        }

        async fn read_file_text(&self, path: &str) -> anyhow::Result<String> {
            Ok(String::from_utf8(self.read_file(path).await?)?)
        }

        async fn write_file(&self, path: &str, contents: &[u8]) -> anyhow::Result<()> {
            self.record("write", path);
            self.files
                .lock()
                .unwrap()
                .insert(path.to_string(), contents.to_vec());
            Ok(())
        }

        async fn exists(&self, path: &str) -> anyhow::Result<bool> {
            self.record("exists", path);
            Ok(self.files.lock().unwrap().contains_key(path))
        }

        async fn is_file(&self, path: &str) -> anyhow::Result<bool> {
            self.exists(path).await
        }

        async fn is_dir(&self, _path: &str) -> anyhow::Result<bool> {
            Ok(false)
        }

        async fn path_kind_no_follow(
            &self,
            path: &str,
        ) -> anyhow::Result<Option<WorkspacePathKind>> {
            Ok(self.exists(path).await?.then_some(WorkspacePathKind::File))
        }

        async fn read_dir(&self, _path: &str) -> anyhow::Result<Vec<WorkspaceDirEntry>> {
            Ok(Vec::new())
        }
    }

    #[test]
    fn workspace_io_error_preserves_standard_io_kind() {
        let error = workspace_io_error(anyhow::Error::new(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "denied",
        )));

        assert_eq!(error.kind(), std::io::ErrorKind::PermissionDenied);
    }

    /// Minimal tempdir helper to avoid pulling in the `tempfile` crate just
    /// for one test. Removes the dir on drop.
    struct ScratchDir(PathBuf);
    impl ScratchDir {
        fn new(label: &str) -> Self {
            let unique = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock before unix epoch")
                .as_nanos();
            let path =
                env::temp_dir().join(format!("bitfun-citation-renumber-{}-{}", label, unique));
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }
        fn path(&self) -> &Path {
            &self.0
        }
    }
    impl Drop for ScratchDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[tokio::test]
    async fn end_to_end_renumbers_report_and_writes_sidecar() {
        let dir = ScratchDir::new("e2e");
        let work_dir = dir.path().join("research");
        let report_dir = dir.path().join("report-out");
        std::fs::create_dir_all(&work_dir).unwrap();
        std::fs::create_dir_all(&report_dir).unwrap();

        let citations = "\
cit_001 | claim a | url=u1 | authority=high | status=ACCEPTED
cit_002 | claim b | url=u2 | authority=low | status=REJECTED | reason=contradicted
cit_005 | claim c | url=u3 | authority=medium
";
        std::fs::write(work_dir.join("citations.md"), citations).unwrap();

        let report = "\
# Deep Research Report

> Summary mentioning cit_005 first.

## Findings

- Cited claim with cit_001 here.
- A pair: [cit_005, cit_001].
- Rejected reference cit_002 should be flagged.

## Citation Index

| ID | Claim | Source |
|----|-------|--------|
| cit_001 | claim a | u1 |
| cit_002 | claim b | u2 |
| cit_005 | claim c | u3 |
";
        let report_path = report_dir.join("test-subject-2026-05-13.md");
        std::fs::write(&report_path, report).unwrap();

        let stats = try_renumber_research_report(
            &HostWorkspaceFs,
            &report_path.to_string_lossy(),
            &work_dir.to_string_lossy(),
        )
        .await
        .unwrap();
        assert_eq!(stats.citations_renumbered, 2);
        assert_eq!(stats.rejected_refs_in_body, 1);

        let after = std::fs::read_to_string(&report_path).unwrap();
        assert!(after.contains("mentioning [1] first"));
        assert!(after.contains("claim with [2] here"));
        assert!(after.contains("A pair: [1, 2]"));
        assert!(after.contains("cit_002 (rejected)"));
        assert!(after.contains("[2] cit_001"));
        assert!(after.contains("[1] cit_005"));

        let index_section = after.split("## Citation Index").nth(1).unwrap_or("");
        assert!(
            !index_section.contains("cit_002"),
            "REJECTED cit_002 must not appear in the Citation Index table"
        );

        let sidecar = work_dir.join("display_map.json");
        assert!(
            sidecar.exists(),
            "display_map.json must sit beside citations.md in WORK_DIR"
        );
        assert!(
            !report_dir.join("display_map.json").exists(),
            "display_map.json must NOT be written next to the report"
        );
        let map: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(sidecar).unwrap()).unwrap();
        assert_eq!(map["citation_count"], 2);
    }

    #[tokio::test]
    async fn run_for_session_is_no_op_when_session_has_no_report() {
        let dir = ScratchDir::new("no-session-report");
        let workspace_root = dir.path().to_string_lossy().to_string();
        run_for_session_workspace(&HostWorkspaceFs, &workspace_root, "missing-session").await;

        let work_dir = dir
            .path()
            .join(".bitfun")
            .join("sessions")
            .join("incomplete-session")
            .join("research");
        std::fs::create_dir_all(&work_dir).unwrap();
        run_for_session_workspace(&HostWorkspaceFs, &workspace_root, "incomplete-session").await;
        assert!(!work_dir.join("display_map.json").exists());
    }

    #[tokio::test]
    async fn run_for_session_renumbers_when_report_present() {
        let dir = ScratchDir::new("with-session-report");
        let session_id = "abc12345-test-session";

        let work_dir = dir
            .path()
            .join(".bitfun")
            .join("sessions")
            .join(session_id)
            .join("research");
        std::fs::create_dir_all(&work_dir).unwrap();

        let report_path = work_dir.join("report.md");
        let report = "\
# Deep Research Report

Para 1 references cit_005 first. Para 2 references cit_001.

## Citation Index

| ID | Claim | Source |
|----|-------|--------|
| cit_001 | claim a | u1 |
| cit_005 | claim c | u3 |
";
        std::fs::write(&report_path, report).unwrap();

        std::fs::write(
            work_dir.join("citations.md"),
            "cit_001 | claim a | url=u1 | authority=high | status=ACCEPTED\n\
             cit_005 | claim c | url=u3 | authority=medium\n",
        )
        .unwrap();

        let workspace_root = dir.path().to_string_lossy().to_string();
        run_for_session_workspace(&HostWorkspaceFs, &workspace_root, session_id).await;

        let after = std::fs::read_to_string(&report_path).unwrap();
        assert!(after.contains("references [1] first"));
        assert!(after.contains("references [2]."));
        assert!(after.contains("[2] cit_001"));
        assert!(after.contains("[1] cit_005"));
        assert!(work_dir.join("display_map.json").exists());
    }

    #[tokio::test]
    async fn remote_workspace_paths_are_processed_only_through_the_workspace_fs_port() {
        let fs = RecordingWorkspaceFs::default();
        let workspace_root = "/root/project";
        let session_id = "remote-session";
        let work_dir = format!("{workspace_root}/.bitfun/sessions/{session_id}/research");
        let report_path = format!("{work_dir}/report.md");
        let citations_path = format!("{work_dir}/citations.md");
        let display_map_path = format!("{work_dir}/display_map.json");
        fs.insert(
            &report_path,
            b"Finding cit_005 then cit_001.\n\n## Citation Index\n\n| ID | Claim |\n|---|---|\n| cit_001 | a |\n| cit_005 | b |\n".to_vec(),
        );
        fs.insert(
            &citations_path,
            b"cit_001 | claim a | status=ACCEPTED\ncit_005 | claim b | status=ACCEPTED\n".to_vec(),
        );

        run_for_session_workspace(&fs, workspace_root, session_id).await;

        let report = fs.text(&report_path).expect("rewritten remote report");
        assert!(report.contains("Finding [1] then [2]."));
        let display_map = fs
            .text(&display_map_path)
            .expect("remote display map sidecar");
        assert!(display_map.contains("\"citation_count\": 2"));
        assert_eq!(
            fs.observed_operations(),
            vec![
                format!("exists:{report_path}"),
                format!("exists:{report_path}"),
                format!("read:{report_path}"),
                format!("exists:{citations_path}"),
                format!("read:{citations_path}"),
                format!("write:{report_path}"),
                format!("write:{display_map_path}"),
            ]
        );
    }

    #[tokio::test]
    async fn missing_workspace_file_never_falls_back_to_the_host_filesystem() {
        let host_workspace = ScratchDir::new("no-host-fallback");
        let session_id = "remote-session";
        let host_work_dir = host_workspace
            .path()
            .join(".bitfun")
            .join("sessions")
            .join(session_id)
            .join("research");
        std::fs::create_dir_all(&host_work_dir).unwrap();
        let host_report = host_work_dir.join("report.md");
        let original_report = "Finding cit_001.\n";
        std::fs::write(&host_report, original_report).unwrap();

        let remote_fs = RecordingWorkspaceFs::default();
        let workspace_root = host_workspace.path().to_string_lossy().to_string();
        run_for_session_workspace(&remote_fs, &workspace_root, session_id).await;

        assert_eq!(
            std::fs::read_to_string(&host_report).unwrap(),
            original_report
        );
        assert!(!host_work_dir.join("display_map.json").exists());
        assert_eq!(
            remote_fs.observed_operations(),
            vec![format!(
                "exists:{}",
                remote_fs.join_path(
                    &workspace_root,
                    &[".bitfun", "sessions", session_id, "research", "report.md"]
                )
            )]
        );
    }
}
