//! Citation renumbering hook for finalized DeepResearch reports.
//!
//! This module owns the best-effort filesystem hook and sidecar persistence.
//! The deterministic report rewrite stays in `bitfun-agent-runtime`.

use bitfun_agent_runtime::deep_research::{
    renumber_research_report, ResearchCitationDisplayMapEntry,
};
use log::{debug, info, warn};
use serde_json::json;
use std::fmt;
use std::path::{Path, PathBuf};
use tokio::fs;

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
pub async fn run_for_session_workspace(workspace_root: &Path, session_id: &str) {
    let work_dir = workspace_root
        .join(".bitfun")
        .join("sessions")
        .join(session_id)
        .join("research");
    let report_path = work_dir.join("report.md");

    if !report_path.exists() {
        debug!(
            "citation_renumber: {} not found, nothing to renumber",
            report_path.display()
        );
        return;
    }

    match try_renumber_research_report(&report_path, &work_dir).await {
        Ok(stats) if stats.citations_renumbered == 0 => {
            debug!(
                "citation_renumber: no cit_XXX references found in {}; skipping",
                report_path.display()
            );
        }
        Ok(stats) => {
            info!(
                "citation_renumber: renumbered {} citations in {} ({} rejected refs in body)",
                stats.citations_renumbered,
                report_path.display(),
                stats.rejected_refs_in_body
            );
        }
        Err(err) => {
            warn!(
                "citation_renumber: skipped (best-effort failure): path={}, err={}",
                report_path.display(),
                err
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
    report_path: &Path,
    work_dir: &Path,
) -> DeepResearchReportIoResult<RenumberStats> {
    if !report_path.exists() {
        return Ok(RenumberStats::default());
    }

    let report = fs::read_to_string(report_path)
        .await
        .map_err(DeepResearchReportIoError::ReadReport)?;

    let registry_path = work_dir.join("citations.md");
    let registry_content = if registry_path.exists() {
        match fs::read_to_string(&registry_path).await {
            Ok(content) => Some(content),
            Err(e) => {
                warn!(
                    "citation_renumber: failed to read citations.md ({}): {}",
                    registry_path.display(),
                    e
                );
                None
            }
        }
    } else {
        None
    };

    let output = renumber_research_report(&report, registry_content.as_deref());

    if output.display_map.is_empty() {
        debug!(
            "citation_renumber: no eligible cit_XXX references in {}",
            report_path.display()
        );
        return Ok(RenumberStats {
            citations_renumbered: output.stats.citations_renumbered,
            rejected_refs_in_body: output.stats.rejected_refs_in_body,
        });
    }

    fs::write(report_path, &output.report)
        .await
        .map_err(DeepResearchReportIoError::WriteReport)?;

    if output.stats.rejected_index_rows_dropped > 0 {
        warn!(
            "citation_renumber: dropped {} REJECTED row(s) from the Citation Index; full registry remains in citations.md",
            output.stats.rejected_index_rows_dropped
        );
    }

    let _ = write_display_map_sidecar(work_dir, report_path, &output.display_map).await;

    Ok(RenumberStats {
        citations_renumbered: output.stats.citations_renumbered,
        rejected_refs_in_body: output.stats.rejected_refs_in_body,
    })
}

async fn write_display_map_sidecar(
    parent: &Path,
    report_path: &Path,
    display_map: &[ResearchCitationDisplayMapEntry],
) -> DeepResearchReportIoResult<PathBuf> {
    let map_path = parent.join("display_map.json");
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
        "report_path": report_path.to_string_lossy(),
        "citation_count": display_map.len(),
        "entries": entries,
    });
    let serialized = serde_json::to_string_pretty(&body)
        .map_err(DeepResearchReportIoError::SerializeDisplayMap)?;
    fs::write(&map_path, serialized).await.map_err(|source| {
        DeepResearchReportIoError::WriteDisplayMap {
            path: map_path.clone(),
            source,
        }
    })?;
    Ok(map_path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

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
        fs::create_dir_all(&work_dir).await.unwrap();
        fs::create_dir_all(&report_dir).await.unwrap();

        let citations = "\
cit_001 | claim a | url=u1 | authority=high | status=ACCEPTED
cit_002 | claim b | url=u2 | authority=low | status=REJECTED | reason=contradicted
cit_005 | claim c | url=u3 | authority=medium
";
        fs::write(work_dir.join("citations.md"), citations)
            .await
            .unwrap();

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
        fs::write(&report_path, report).await.unwrap();

        let stats = try_renumber_research_report(&report_path, &work_dir)
            .await
            .unwrap();
        assert_eq!(stats.citations_renumbered, 2);
        assert_eq!(stats.rejected_refs_in_body, 1);

        let after = fs::read_to_string(&report_path).await.unwrap();
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
            serde_json::from_str(&fs::read_to_string(sidecar).await.unwrap()).unwrap();
        assert_eq!(map["citation_count"], 2);
    }

    #[tokio::test]
    async fn run_for_session_is_no_op_when_session_has_no_report() {
        let dir = ScratchDir::new("no-session-report");
        run_for_session_workspace(dir.path(), "missing-session").await;

        let work_dir = dir
            .path()
            .join(".bitfun")
            .join("sessions")
            .join("incomplete-session")
            .join("research");
        fs::create_dir_all(&work_dir).await.unwrap();
        run_for_session_workspace(dir.path(), "incomplete-session").await;
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
        fs::create_dir_all(&work_dir).await.unwrap();

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
        fs::write(&report_path, report).await.unwrap();

        fs::write(
            work_dir.join("citations.md"),
            "cit_001 | claim a | url=u1 | authority=high | status=ACCEPTED\n\
             cit_005 | claim c | url=u3 | authority=medium\n",
        )
        .await
        .unwrap();

        run_for_session_workspace(dir.path(), session_id).await;

        let after = fs::read_to_string(&report_path).await.unwrap();
        assert!(after.contains("references [1] first"));
        assert!(after.contains("references [2]."));
        assert!(after.contains("[2] cit_001"));
        assert!(after.contains("[1] cit_005"));
        assert!(work_dir.join("display_map.json").exists());
    }
}
