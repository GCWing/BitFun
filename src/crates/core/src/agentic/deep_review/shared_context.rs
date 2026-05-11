//! Content-free duplicate tool-use tracking for shared reviewer context.
//!
//! This module measures duplicate `Read` and `GetFileDiff` usage without
//! storing tool results. It is an observability aid for future evidence/cache
//! decisions, not a programmatic full tool-result cache.

use serde::Serialize;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeepReviewSharedContextDuplicate {
    pub tool_name: String,
    pub file_path: String,
    pub call_count: usize,
    pub reviewer_count: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeepReviewSharedContextMeasurementSnapshot {
    pub total_calls: usize,
    pub duplicate_calls: usize,
    pub duplicate_context_count: usize,
    pub repeated_contexts: Vec<DeepReviewSharedContextDuplicate>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct DeepReviewSharedContextKey {
    pub(crate) tool_name: String,
    pub(crate) file_path: String,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct DeepReviewSharedContextUseRecord {
    pub(crate) call_count: usize,
    pub(crate) reviewer_types: HashSet<String>,
}

pub(crate) fn normalize_shared_context_tool_name(tool_name: &str) -> Option<&'static str> {
    let tool_name = tool_name.trim();
    if tool_name.eq_ignore_ascii_case("Read") {
        Some("Read")
    } else if tool_name.eq_ignore_ascii_case("GetFileDiff") {
        Some("GetFileDiff")
    } else {
        None
    }
}

pub(crate) fn normalize_shared_context_file_path(file_path: &str) -> Option<String> {
    let mut file_path = file_path.trim().replace('\\', "/");
    while file_path.starts_with("./") {
        file_path = file_path[2..].to_string();
    }
    (!file_path.is_empty()).then_some(file_path)
}

pub(crate) fn shared_context_measurement_snapshot_from_uses(
    uses: &HashMap<DeepReviewSharedContextKey, DeepReviewSharedContextUseRecord>,
) -> DeepReviewSharedContextMeasurementSnapshot {
    let total_calls = uses.values().map(|record| record.call_count).sum();
    let duplicate_calls = uses
        .values()
        .map(|record| record.call_count.saturating_sub(1))
        .sum();
    let mut repeated_contexts: Vec<DeepReviewSharedContextDuplicate> = uses
        .iter()
        .filter_map(|(key, record)| {
            (record.call_count > 1).then(|| DeepReviewSharedContextDuplicate {
                tool_name: key.tool_name.clone(),
                file_path: key.file_path.clone(),
                call_count: record.call_count,
                reviewer_count: record.reviewer_types.len(),
            })
        })
        .collect();
    repeated_contexts.sort_by(|left, right| {
        right
            .call_count
            .cmp(&left.call_count)
            .then_with(|| right.reviewer_count.cmp(&left.reviewer_count))
            .then_with(|| left.tool_name.cmp(&right.tool_name))
            .then_with(|| left.file_path.cmp(&right.file_path))
    });
    let duplicate_context_count = repeated_contexts.len();

    DeepReviewSharedContextMeasurementSnapshot {
        total_calls,
        duplicate_calls,
        duplicate_context_count,
        repeated_contexts,
    }
}
