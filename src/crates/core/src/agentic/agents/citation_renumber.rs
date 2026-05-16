//! Citation renumbering hook for finalized deep-research reports.
//!
//! Triggered from `execute_dialog_turn_impl` after a DeepResearch agent's
//! dialog turn completes successfully. Reads
//! `<workspace>/.bitfun/sessions/<session_id>/research/report.md`, walks the
//! body in order, assigns consecutive display numbers `[1]`, `[2]`, ... to
//! each unique `cit_XXX` reference, and rewrites the report in place.
//! Citations marked `status=REJECTED` in the sibling `citations.md` registry
//! are skipped from numbering (and produce a warning if they still appear in
//! the report body). The display_map.json sidecar is written into the same
//! directory alongside `citations.md`.
//!
//! Both the report and audit files share one per-session WORK_DIR, so the
//! hook has zero path ambiguity — no scanning, no slug inference, no
//! pointer files.
//!
//! The hook is best-effort: any I/O or parse failure logs a warning and
//! leaves the report untouched. It is idempotent — a re-run on a report
//! containing only `[N]` references is a no-op.
//!
//! Why a hook (not a tool): renumbering must be a deterministic
//! post-processing step under engineering control, independent of whether
//! the model remembers to invoke it.

use crate::util::errors::{BitFunError, BitFunResult};
use log::{debug, info, warn};
use regex::Regex;
use serde_json::json;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;
use tokio::fs;

/// Outcome summary returned to the caller for logging / telemetry.
#[derive(Debug, Default, Clone)]
pub struct RenumberStats {
    pub citations_renumbered: usize,
    pub rejected_refs_in_body: usize,
}

static CIT_ID_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\bcit_\d+\b").unwrap());
static REGISTRY_ROW_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(cit_\d+)\b").unwrap());
static REGISTRY_STATUS_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"status\s*=\s*([A-Za-z_]+)").unwrap());
static CITATION_INDEX_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?m)^#{1,6}\s*(Citation Index|引用索引|引用列表)\s*$").unwrap());
static BRACKETED_GROUP_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\[(cit_\d+(?:\s*,\s*cit_\d+)*)\]").unwrap());

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
) -> BitFunResult<RenumberStats> {
    if !report_path.exists() {
        return Ok(RenumberStats::default());
    }

    let report = fs::read_to_string(report_path)
        .await
        .map_err(|e| BitFunError::tool(format!("read report failed: {}", e)))?;

    let registry_path = work_dir.join("citations.md");
    let registry_status = if registry_path.exists() {
        match fs::read_to_string(&registry_path).await {
            Ok(content) => parse_registry_status(&content),
            Err(e) => {
                warn!(
                    "citation_renumber: failed to read citations.md ({}): {}",
                    registry_path.display(),
                    e
                );
                HashMap::new()
            }
        }
    } else {
        HashMap::new()
    };

    let (body, index_section) = split_at_citation_index(&report);

    let (display_map, order, rejected_refs_in_body) = build_display_map(body, &registry_status);

    if display_map.is_empty() {
        debug!(
            "citation_renumber: no eligible cit_XXX references in {}",
            report_path.display()
        );
        return Ok(RenumberStats {
            citations_renumbered: 0,
            rejected_refs_in_body,
        });
    }

    let new_body = renumber_body(body, &display_map);
    let new_index = match index_section {
        Some(idx) => renumber_index_section(idx, &display_map),
        None => String::new(),
    };

    let final_report = if new_index.is_empty() {
        new_body
    } else {
        // Preserve original separator style: body usually ends with a trailing
        // newline + horizontal rule; we keep the existing whitespace as-is.
        format!("{}{}", new_body, new_index)
    };

    fs::write(report_path, &final_report)
        .await
        .map_err(|e| BitFunError::tool(format!("write report failed: {}", e)))?;

    // The display_map sidecar lives next to citations.md in WORK_DIR — both
    // are audit-trail artifacts for the same logical layer (internal cit_XXX
    // ↔ display [N]). The report's Citation Index table already shows the
    // mapping to human readers; display_map.json is for tooling.
    let _ = write_display_map_sidecar(work_dir, report_path, &order).await;

    Ok(RenumberStats {
        citations_renumbered: order.len(),
        rejected_refs_in_body,
    })
}

fn parse_registry_status(content: &str) -> HashMap<String, String> {
    let mut out = HashMap::new();
    for line in content.lines() {
        let trimmed = line.trim_start_matches(|c: char| c == '|' || c.is_whitespace());
        let Some(id_m) = REGISTRY_ROW_RE.captures(trimmed) else {
            continue;
        };
        let id = id_m.get(1).unwrap().as_str().to_string();
        let status = REGISTRY_STATUS_RE
            .captures(line)
            .and_then(|c| c.get(1).map(|m| m.as_str().to_string()))
            .unwrap_or_else(|| "ACCEPTED".to_string());
        out.insert(id, status.to_ascii_uppercase());
    }
    out
}

fn split_at_citation_index(report: &str) -> (&str, Option<&str>) {
    match CITATION_INDEX_RE.find(report) {
        Some(m) => (&report[..m.start()], Some(&report[m.start()..])),
        None => (report, None),
    }
}

fn build_display_map(
    body: &str,
    registry_status: &HashMap<String, String>,
) -> (HashMap<String, usize>, Vec<String>, usize) {
    let mut display_map: HashMap<String, usize> = HashMap::new();
    let mut order: Vec<String> = Vec::new();
    let mut rejected_refs_in_body = 0usize;

    for m in CIT_ID_RE.find_iter(body) {
        let cit_id = m.as_str();
        if display_map.contains_key(cit_id) {
            continue;
        }
        if let Some(status) = registry_status.get(cit_id) {
            if status == "REJECTED" {
                rejected_refs_in_body += 1;
                continue;
            }
        }
        let n = order.len() + 1;
        display_map.insert(cit_id.to_string(), n);
        order.push(cit_id.to_string());
    }

    (display_map, order, rejected_refs_in_body)
}

fn renumber_body(body: &str, display_map: &HashMap<String, usize>) -> String {
    // Pass 1: collapse [cit_X, cit_Y, ...] groups into a single bracket of
    // display numbers, so we do not end up with `[[1], [2]]`.
    let pass1 = BRACKETED_GROUP_RE.replace_all(body, |caps: &regex::Captures| {
        let inside = &caps[1];
        let mapped: Vec<String> = CIT_ID_RE
            .find_iter(inside)
            .map(|m| {
                let cit = m.as_str();
                match display_map.get(cit) {
                    Some(n) => format!("{}", n),
                    None => format!("{} (rejected)", cit),
                }
            })
            .collect();
        format!("[{}]", mapped.join(", "))
    });

    // Pass 2: bare `cit_XXX` references become `[N]`. By now any cit_XXX
    // inside `[...]` is already replaced; the remaining matches are
    // free-standing tokens (e.g. inline "see cit_007 for the source").
    CIT_ID_RE
        .replace_all(&pass1, |caps: &regex::Captures| {
            let cit = caps.get(0).unwrap().as_str();
            match display_map.get(cit) {
                Some(n) => format!("[{}]", n),
                None => format!("[{} (rejected)]", cit),
            }
        })
        .to_string()
}

fn renumber_index_section(section: &str, display_map: &HashMap<String, usize>) -> String {
    // 1. Mark each cit_XXX with its [N] prefix. Citations that have no entry
    //    in display_map (either never referenced by the body, or rejected in
    //    Phase 4) are tagged `[REJECTED]` here so the next step can prune
    //    those rows entirely.
    let marked = CIT_ID_RE
        .replace_all(section, |caps: &regex::Captures| {
            let cit = caps.get(0).unwrap().as_str();
            match display_map.get(cit) {
                Some(n) => format!("[{}] {}", n, cit),
                None => format!("[REJECTED] {}", cit),
            }
        })
        .to_string();

    // 2. Walk the section line-by-line:
    //    - Drop data rows whose tag is `[REJECTED]` — they should never
    //      surface in the user-facing report. The full audit lives in
    //      `<work_dir>/citations.md`.
    //    - Within each contiguous block of accepted data rows, sort by [N]
    //      so the reader sees [1] [2] [3] in order.
    //    - Pass everything else through unchanged.
    let mut lines: Vec<String> = marked.lines().map(|s| s.to_string()).collect();
    let mut dropped_rejected = 0usize;
    let mut i = 0;
    while i < lines.len() {
        if !is_index_data_row(&lines[i]) {
            i += 1;
            continue;
        }
        let start = i;
        while i < lines.len() && is_index_data_row(&lines[i]) {
            i += 1;
        }
        let mut kept: Vec<String> = lines
            .splice(start..i, std::iter::empty::<String>())
            .filter(|row| {
                let drop = row_is_rejected(row);
                if drop {
                    dropped_rejected += 1;
                }
                !drop
            })
            .collect();
        kept.sort_by_key(|line| extract_display_sort_key(line));
        let kept_len = kept.len();
        for (offset, row) in kept.into_iter().enumerate() {
            lines.insert(start + offset, row);
        }
        i = start + kept_len;
    }

    if dropped_rejected > 0 {
        warn!(
            "citation_renumber: dropped {} REJECTED row(s) from the Citation Index — the model copied audit-only entries into the user-facing report; this is normal cleanup, full registry remains in citations.md",
            dropped_rejected
        );
    }

    let mut out = lines.join("\n");
    // `str::lines()` strips a trailing newline; preserve it if the original had one.
    if section.ends_with('\n') && !out.ends_with('\n') {
        out.push('\n');
    }
    out
}

/// A row carries the `[REJECTED]` tag if and only if `display_map` had no
/// entry for its cit_XXX — i.e. the model copied an audit-only citation into
/// the user-facing index. Such rows are pruned before the final report goes
/// out.
fn row_is_rejected(line: &str) -> bool {
    let bytes = line.as_bytes();
    let Some(open) = bytes.iter().position(|&b| b == b'[') else {
        return false;
    };
    let Some(close_off) = bytes[open + 1..].iter().position(|&b| b == b']') else {
        return false;
    };
    &line[open + 1..open + 1 + close_off] == "REJECTED"
}

/// A data row in the Citation Index table starts with `| [` (the leading
/// `|`, optional whitespace, then the display-number bracket we just stamped
/// in). Separator rows like `|---|---|` are excluded.
fn is_index_data_row(line: &str) -> bool {
    let trimmed = line.trim_start();
    if !trimmed.starts_with('|') {
        return false;
    }
    if trimmed.contains("---") {
        return false;
    }
    // First non-whitespace cell content must start with `[` (our stamp).
    let after_pipe = trimmed[1..].trim_start();
    after_pipe.starts_with('[')
}

/// Pull the integer display number from the first `[...]` group on the line.
/// REJECTED rows (no number inside the brackets) sort to the end via
/// `usize::MAX`. Lines with no recognizable display tag also sort last —
/// which means this function is safe to call on any line `is_index_data_row`
/// already accepted.
fn extract_display_sort_key(line: &str) -> usize {
    let bytes = line.as_bytes();
    let Some(open) = bytes.iter().position(|&b| b == b'[') else {
        return usize::MAX;
    };
    let Some(close_offset) = bytes[open + 1..].iter().position(|&b| b == b']') else {
        return usize::MAX;
    };
    let inner = &line[open + 1..open + 1 + close_offset];
    inner.parse().unwrap_or(usize::MAX)
}

async fn write_display_map_sidecar(
    parent: &Path,
    report_path: &Path,
    order: &[String],
) -> BitFunResult<PathBuf> {
    let map_path = parent.join("display_map.json");
    let entries: Vec<_> = order
        .iter()
        .enumerate()
        .map(|(i, cit)| {
            json!({
                "display": format!("[{}]", i + 1),
                "internal": cit,
            })
        })
        .collect();
    let body = json!({
        "version": 1,
        "report_path": report_path.to_string_lossy(),
        "citation_count": order.len(),
        "entries": entries,
    });
    let serialized = serde_json::to_string_pretty(&body).map_err(|e| {
        BitFunError::tool(format!("serialize display_map.json failed: {}", e))
    })?;
    fs::write(&map_path, serialized).await.map_err(|e| {
        BitFunError::tool(format!(
            "write {} failed: {}",
            map_path.display(),
            e
        ))
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
            let path = env::temp_dir().join(format!(
                "bitfun-citation-renumber-{}-{}",
                label,
                uuid::Uuid::new_v4()
            ));
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

    #[test]
    fn renumber_body_in_order_of_first_appearance() {
        let body = "Para 1 mentions cit_005 first.\n\nPara 2 mentions cit_001 and cit_005 again.";
        let mut map = HashMap::new();
        map.insert("cit_005".to_string(), 1);
        map.insert("cit_001".to_string(), 2);
        let out = renumber_body(body, &map);
        assert_eq!(
            out,
            "Para 1 mentions [1] first.\n\nPara 2 mentions [2] and [1] again."
        );
    }

    #[test]
    fn collapses_bracketed_groups() {
        let body = "*Sources: [cit_003, cit_007], [cit_001]*";
        let mut map = HashMap::new();
        map.insert("cit_003".to_string(), 1);
        map.insert("cit_007".to_string(), 2);
        map.insert("cit_001".to_string(), 3);
        let out = renumber_body(body, &map);
        assert_eq!(out, "*Sources: [1, 2], [3]*");
    }

    #[test]
    fn body_scan_skips_rejected() {
        let body = "cit_001 valid, cit_002 dropped, cit_003 valid.";
        let mut registry = HashMap::new();
        registry.insert("cit_002".to_string(), "REJECTED".to_string());
        let (map, order, rejected) = build_display_map(body, &registry);
        assert_eq!(map.get("cit_001"), Some(&1));
        assert_eq!(map.get("cit_003"), Some(&2));
        assert!(!map.contains_key("cit_002"));
        assert_eq!(order, vec!["cit_001".to_string(), "cit_003".to_string()]);
        assert_eq!(rejected, 1);
    }

    #[test]
    fn parse_registry_handles_pipe_prefix_and_missing_status() {
        let content = "\
cit_001 | claim a | url=u1 | authority=high | corroborated=true
| cit_002 | claim b | url=u2 | status=REJECTED | reason=paywalled
cit_003 | claim c | url=u3 | status=accepted
";
        let map = parse_registry_status(content);
        assert_eq!(map.get("cit_001").map(String::as_str), Some("ACCEPTED"));
        assert_eq!(map.get("cit_002").map(String::as_str), Some("REJECTED"));
        assert_eq!(map.get("cit_003").map(String::as_str), Some("ACCEPTED"));
    }

    #[test]
    fn split_at_citation_index_finds_section() {
        let report = "# Title\n\nbody...\n\n## Citation Index\n\n| ID | ... |";
        let (body, idx) = split_at_citation_index(report);
        assert_eq!(body, "# Title\n\nbody...\n\n");
        assert!(idx.unwrap().starts_with("## Citation Index"));
    }

    #[test]
    fn index_section_keeps_internal_id_with_display_prefix() {
        let section =
            "## Citation Index\n\n| ID | Claim | Source |\n|----|-------|--------|\n| cit_001 | ... | url1 |\n| cit_003 | ... | url3 |\n";
        let mut map = HashMap::new();
        map.insert("cit_001".to_string(), 1);
        map.insert("cit_003".to_string(), 2);
        let out = renumber_index_section(section, &map);
        assert!(out.contains("[1] cit_001"));
        assert!(out.contains("[2] cit_003"));
    }

    /// Reproduces the bug the user found: registry order was cit_001, cit_002,
    /// cit_003 but display numbers came out [8], [14], [16] (because each
    /// citation's first body appearance was scattered). The Index table must
    /// be reordered to [1], [2], [3] so reader scanning is monotonic.
    #[test]
    fn index_section_rows_are_sorted_by_display_number() {
        let section = "\
## Citation Index

| ID | Claim | Source |
|----|-------|--------|
| cit_001 | first registered | u1 |
| cit_002 | second registered | u2 |
| cit_003 | third registered | u3 |
";
        // Body appearance order: cit_003 (→[1]), cit_001 (→[2]), cit_002 (→[3])
        let mut map = HashMap::new();
        map.insert("cit_003".to_string(), 1);
        map.insert("cit_001".to_string(), 2);
        map.insert("cit_002".to_string(), 3);

        let out = renumber_index_section(section, &map);

        let lines: Vec<&str> = out.lines().collect();
        let data_rows: Vec<&str> = lines
            .into_iter()
            .filter(|l| l.trim_start().starts_with("| ["))
            .collect();
        assert_eq!(data_rows.len(), 3, "should have 3 data rows");
        assert!(
            data_rows[0].contains("[1] cit_003"),
            "first row should be [1] cit_003, got: {}",
            data_rows[0]
        );
        assert!(
            data_rows[1].contains("[2] cit_001"),
            "second row should be [2] cit_001, got: {}",
            data_rows[1]
        );
        assert!(
            data_rows[2].contains("[3] cit_002"),
            "third row should be [3] cit_002, got: {}",
            data_rows[2]
        );
    }

    /// REJECTED citations are audit-only and must not appear in the
    /// user-facing index. If the model copied them from citations.md into
    /// the Phase 6 table, the hook prunes those rows.
    #[test]
    fn index_section_rejected_rows_are_dropped() {
        let section = "\
| cit_001 | a | u1 |
| cit_002 | b (rejected) | u2 |
| cit_003 | c | u3 |
";
        // cit_002 was rejected (not in map). cit_003 → [1], cit_001 → [2].
        let mut map = HashMap::new();
        map.insert("cit_003".to_string(), 1);
        map.insert("cit_001".to_string(), 2);

        let out = renumber_index_section(section, &map);
        let lines: Vec<&str> = out.lines().collect();

        // Only the two accepted rows survive, in display order.
        let data_rows: Vec<&str> = lines
            .iter()
            .copied()
            .filter(|l| l.trim_start().starts_with("| ["))
            .collect();
        assert_eq!(data_rows.len(), 2, "REJECTED row should be dropped");
        assert!(data_rows[0].contains("[1] cit_003"));
        assert!(data_rows[1].contains("[2] cit_001"));

        // cit_002 must not appear anywhere in the rendered Index.
        assert!(!out.contains("cit_002"), "REJECTED cit_002 leaked into index");
        assert!(!out.contains("[REJECTED]"), "no REJECTED tag should remain");
    }

    #[tokio::test]
    async fn end_to_end_renumbers_report_and_writes_sidecar() {
        // `try_renumber_research_report` takes the report path and the audit
        // `work_dir` as independent arguments. In production both live under
        // the same session work_dir (`<work_dir>/report.md`); this test puts
        // them in separate scratch dirs to lock in the path-agnostic
        // contract so any future caller can pass them independently.
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
        // body: cit_005 → [1] (first appearance), cit_001 → [2]
        assert!(after.contains("mentioning [1] first"));
        assert!(after.contains("claim with [2] here"));
        assert!(after.contains("A pair: [1, 2]"));
        // A REJECTED cit appearing in the body keeps a `(rejected)` marker so
        // the prose stays readable but the reader sees a sourcing warning.
        assert!(after.contains("cit_002 (rejected)"));
        // index keeps internal IDs for the accepted rows
        assert!(after.contains("[2] cit_001"));
        assert!(after.contains("[1] cit_005"));
        // Citation Index must NOT carry the rejected row at all — full audit
        // remains in citations.md, not in the user-facing report.
        let index_section = after.split("## Citation Index").nth(1).unwrap_or("");
        assert!(
            !index_section.contains("cit_002"),
            "REJECTED cit_002 must not appear in the Citation Index table"
        );

        // sidecar lives in WORK_DIR next to citations.md, NOT next to the report
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
        // Session dir does not even exist.
        run_for_session_workspace(dir.path(), "missing-session").await;
        // No panic, no file created. (Verifies the early-return path.)

        // And when work_dir exists but report.md does not, still a no-op.
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
        // cit_005 appeared first → [1], cit_001 → [2]
        assert!(after.contains("references [1] first"));
        assert!(after.contains("references [2]."));
        // Index keeps internal IDs for traceability
        assert!(after.contains("[2] cit_001"));
        assert!(after.contains("[1] cit_005"));
        // Sidecar lives in the session's WORK_DIR
        let sidecar = work_dir.join("display_map.json");
        assert!(sidecar.exists());
    }
}
