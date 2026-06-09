//! Provider-neutral DeepResearch report post-processing.
//!
//! This module owns deterministic citation renumbering for finalized research
//! reports. It does not read or write files; callers provide the report body
//! and optional citation registry content, then persist the returned report and
//! display map in their own storage boundary.

use regex::Regex;
use std::collections::HashMap;
use std::sync::LazyLock;

static CIT_ID_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\bcit_\d+\b").unwrap());
static REGISTRY_ROW_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(cit_\d+)\b").unwrap());
static REGISTRY_STATUS_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"status\s*=\s*([A-Za-z_]+)").unwrap());
static CITATION_INDEX_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?m)^#{1,6}\s*(Citation Index|引用索引|引用列表)\s*$").unwrap());
static BRACKETED_GROUP_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\[(cit_\d+(?:\s*,\s*cit_\d+)*)\]").unwrap());

/// Outcome summary returned to the storage adapter for logging or telemetry.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ResearchCitationRenumberStats {
    pub citations_renumbered: usize,
    pub rejected_refs_in_body: usize,
    pub rejected_index_rows_dropped: usize,
}

/// One entry in the persisted display-map sidecar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResearchCitationDisplayMapEntry {
    pub display: String,
    pub internal: String,
}

/// Pure output of DeepResearch report renumbering.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResearchCitationRenumberOutput {
    pub report: String,
    pub display_map: Vec<ResearchCitationDisplayMapEntry>,
    pub stats: ResearchCitationRenumberStats,
}

/// Renumber internal `cit_XXX` references in a finalized DeepResearch report.
///
/// The numbering order is based on first appearance in the report body. Any
/// citation marked `status=REJECTED` in the registry remains visible as a
/// warning marker in body text but is removed from the user-facing Citation
/// Index. This function is idempotent for reports that no longer contain
/// internal citation ids.
pub fn renumber_research_report(
    report: &str,
    registry_content: Option<&str>,
) -> ResearchCitationRenumberOutput {
    let registry_status = registry_content
        .map(parse_registry_status)
        .unwrap_or_default();
    let (body, index_section) = split_at_citation_index(report);
    let (display_map, order, rejected_refs_in_body) = build_display_map(body, &registry_status);

    if display_map.is_empty() {
        return ResearchCitationRenumberOutput {
            report: report.to_string(),
            display_map: Vec::new(),
            stats: ResearchCitationRenumberStats {
                citations_renumbered: 0,
                rejected_refs_in_body,
                rejected_index_rows_dropped: 0,
            },
        };
    }

    let new_body = renumber_body(body, &display_map);
    let (new_index, rejected_index_rows_dropped) = match index_section {
        Some(section) => renumber_index_section(section, &display_map),
        None => (String::new(), 0),
    };
    let report = if new_index.is_empty() {
        new_body
    } else {
        format!("{}{}", new_body, new_index)
    };
    let display_map = order
        .iter()
        .enumerate()
        .map(|(index, internal)| ResearchCitationDisplayMapEntry {
            display: format!("[{}]", index + 1),
            internal: internal.clone(),
        })
        .collect::<Vec<_>>();

    ResearchCitationRenumberOutput {
        report,
        display_map,
        stats: ResearchCitationRenumberStats {
            citations_renumbered: order.len(),
            rejected_refs_in_body,
            rejected_index_rows_dropped,
        },
    }
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
            .and_then(|captures| captures.get(1).map(|m| m.as_str().to_string()))
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
    let pass1 = BRACKETED_GROUP_RE.replace_all(body, |caps: &regex::Captures| {
        let inside = &caps[1];
        let mapped = CIT_ID_RE
            .find_iter(inside)
            .map(|m| {
                let cit = m.as_str();
                match display_map.get(cit) {
                    Some(n) => format!("{}", n),
                    None => format!("{} (rejected)", cit),
                }
            })
            .collect::<Vec<_>>();
        format!("[{}]", mapped.join(", "))
    });

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

fn renumber_index_section(section: &str, display_map: &HashMap<String, usize>) -> (String, usize) {
    let marked = CIT_ID_RE
        .replace_all(section, |caps: &regex::Captures| {
            let cit = caps.get(0).unwrap().as_str();
            match display_map.get(cit) {
                Some(n) => format!("[{}] {}", n, cit),
                None => format!("[REJECTED] {}", cit),
            }
        })
        .to_string();

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

    let mut out = lines.join("\n");
    if section.ends_with('\n') && !out.ends_with('\n') {
        out.push('\n');
    }
    (out, dropped_rejected)
}

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

fn is_index_data_row(line: &str) -> bool {
    let trimmed = line.trim_start();
    if !trimmed.starts_with('|') {
        return false;
    }
    if trimmed.contains("---") {
        return false;
    }
    let after_pipe = trimmed[1..].trim_start();
    after_pipe.starts_with('[')
}

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
