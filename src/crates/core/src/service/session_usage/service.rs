use crate::agentic::persistence::PersistenceManager;
use crate::service::session::{DialogTurnData, DialogTurnKind, ToolItemData, TurnStatus};
use crate::service::session_usage::classifier::classify_tool_usage;
use crate::service::session_usage::redaction::{
    display_workspace_relative_path, redact_usage_label,
};
use crate::service::session_usage::types::*;
use crate::service::snapshot::get_snapshot_manager_for_workspace;
use crate::service::snapshot::types::FileOperation;
use crate::service::token_usage::{
    TimeRange, TokenUsageQuery, TokenUsageRecord, TokenUsageService,
};
use crate::util::errors::{BitFunError, BitFunResult};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SessionUsageReportRequest {
    pub session_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_connection_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_ssh_host: Option<String>,
    #[serde(default)]
    pub include_hidden_subagents: bool,
}

pub async fn generate_session_usage_report(
    persistence_manager: &PersistenceManager,
    token_usage_service: Option<&TokenUsageService>,
    request: SessionUsageReportRequest,
) -> BitFunResult<SessionUsageReport> {
    let workspace_path = request
        .workspace_path
        .clone()
        .ok_or_else(|| BitFunError::validation("Workspace path is required for usage reports"))?;
    let turns = persistence_manager
        .load_session_turns(Path::new(&workspace_path), &request.session_id)
        .await?;
    let token_records = if let Some(service) = token_usage_service {
        service
            .query_records(TokenUsageQuery {
                model_id: None,
                session_id: Some(request.session_id.clone()),
                time_range: TimeRange::All,
                limit: None,
                offset: None,
                include_subagent: request.include_hidden_subagents,
            })
            .await
            .map_err(|error| {
                BitFunError::service(format!("Failed to query token usage records: {}", error))
            })?
    } else {
        Vec::new()
    };

    let snapshot_facts = load_snapshot_facts(&request).await;

    Ok(build_session_usage_report_from_sources(
        request,
        &turns,
        &token_records,
        &snapshot_facts,
        Utc::now().timestamp_millis(),
    ))
}

pub fn build_session_usage_report_from_turns(
    request: SessionUsageReportRequest,
    turns: &[DialogTurnData],
    token_records: &[TokenUsageRecord],
    generated_at: i64,
) -> SessionUsageReport {
    build_session_usage_report_from_sources(
        request,
        turns,
        token_records,
        &UsageSnapshotFacts::default(),
        generated_at,
    )
}

pub fn build_session_usage_report_from_sources(
    request: SessionUsageReportRequest,
    turns: &[DialogTurnData],
    token_records: &[TokenUsageRecord],
    snapshot_facts: &UsageSnapshotFacts,
    generated_at: i64,
) -> SessionUsageReport {
    let mut report = SessionUsageReport::partial_unavailable(&request.session_id, generated_at);
    report.report_id = format!("usage-{}-{}", request.session_id, generated_at);
    report.workspace = build_workspace(&request);
    report.scope = build_scope(turns, request.include_hidden_subagents);
    report.coverage = build_coverage(&request, turns, token_records, snapshot_facts);
    report.time = build_time_breakdown(turns);
    report.tokens = build_token_breakdown(token_records);
    report.models = build_model_breakdown(token_records);
    report.tools = build_tool_breakdown(turns);
    report.files = build_file_breakdown(request.workspace_path.as_deref(), turns, snapshot_facts);
    report.compression = build_compression_breakdown(turns);
    report.errors = build_error_breakdown(turns);
    report.slowest = build_slowest_spans(turns);
    report.privacy = UsagePrivacy {
        prompt_content_included: false,
        tool_inputs_included: false,
        command_outputs_included: false,
        file_contents_included: false,
        redacted_fields: collect_redacted_fields(&report),
    };
    report
}

async fn load_snapshot_facts(request: &SessionUsageReportRequest) -> UsageSnapshotFacts {
    if request.remote_connection_id.is_some() || request.remote_ssh_host.is_some() {
        return UsageSnapshotFacts::default();
    }

    let Some(workspace_path) = request.workspace_path.as_deref() else {
        return UsageSnapshotFacts::default();
    };

    let Some(manager) = get_snapshot_manager_for_workspace(Path::new(workspace_path)) else {
        return UsageSnapshotFacts::default();
    };

    match manager.get_session(&request.session_id).await {
        Ok(session) => UsageSnapshotFacts {
            source_available: true,
            operations: session
                .operations
                .into_iter()
                .map(snapshot_operation_from_file_operation)
                .collect(),
        },
        Err(_) => UsageSnapshotFacts::default(),
    }
}

fn snapshot_operation_from_file_operation(
    operation: FileOperation,
) -> UsageSnapshotOperationSummary {
    UsageSnapshotOperationSummary {
        operation_id: operation.operation_id,
        session_id: operation.session_id,
        turn_index: operation.turn_index,
        file_path: operation.file_path.to_string_lossy().to_string(),
        lines_added: operation.diff_summary.lines_added as u64,
        lines_removed: operation.diff_summary.lines_removed as u64,
    }
}

fn build_workspace(request: &SessionUsageReportRequest) -> UsageWorkspace {
    UsageWorkspace {
        kind: if request.remote_connection_id.is_some() || request.remote_ssh_host.is_some() {
            UsageWorkspaceKind::RemoteSsh
        } else if request.workspace_path.is_some() {
            UsageWorkspaceKind::Local
        } else {
            UsageWorkspaceKind::Unknown
        },
        path_label: request
            .workspace_path
            .as_deref()
            .map(|path| redact_usage_label(path, 120).value),
        workspace_id: None,
        remote_connection_id: request.remote_connection_id.clone(),
        remote_ssh_host: request.remote_ssh_host.clone(),
    }
}

fn build_scope(turns: &[DialogTurnData], includes_subagents: bool) -> UsageScope {
    UsageScope {
        kind: UsageScopeKind::EntireSession,
        turn_count: turns.len(),
        from_turn_id: turns.first().map(|turn| turn.turn_id.clone()),
        to_turn_id: turns.last().map(|turn| turn.turn_id.clone()),
        includes_subagents,
    }
}

fn build_coverage(
    request: &SessionUsageReportRequest,
    turns: &[DialogTurnData],
    token_records: &[TokenUsageRecord],
    snapshot_facts: &UsageSnapshotFacts,
) -> UsageCoverage {
    let mut available = vec![UsageCoverageKey::WorkspaceIdentity];
    if !token_records.is_empty() {
        available.push(UsageCoverageKey::SubagentScope);
    }
    if turns
        .iter()
        .flat_map(|turn| turn.model_rounds.iter())
        .any(|round| round.end_time.is_some())
    {
        available.push(UsageCoverageKey::ModelRoundTiming);
    }
    if token_records
        .iter()
        .any(|record| record.cached_tokens_available)
    {
        available.push(UsageCoverageKey::CachedTokens);
    }
    if token_records
        .iter()
        .any(|record| record.token_details.is_some())
    {
        available.push(UsageCoverageKey::TokenDetailBreakdown);
    }
    if snapshot_facts.source_available {
        available.push(UsageCoverageKey::FileLineStats);
    }

    let mut missing = vec![
        UsageCoverageKey::ToolPhaseTiming,
        UsageCoverageKey::CachedTokens,
        UsageCoverageKey::TokenDetailBreakdown,
        UsageCoverageKey::FileLineStats,
        UsageCoverageKey::CostEstimates,
    ];
    if !available.contains(&UsageCoverageKey::ModelRoundTiming) {
        missing.push(UsageCoverageKey::ModelRoundTiming);
    }
    for available_key in &available {
        missing.retain(|key| key != available_key);
    }

    if request.remote_connection_id.is_some() || request.remote_ssh_host.is_some() {
        if snapshot_facts.source_available {
            available.push(UsageCoverageKey::RemoteSnapshotStats);
        } else {
            missing.push(UsageCoverageKey::RemoteSnapshotStats);
        }
    }

    available.sort_by_key(|key| format!("{:?}", key));
    available.dedup();
    missing.sort_by_key(|key| format!("{:?}", key));
    missing.dedup();

    let mut notes = vec![
        "Report is based on persisted turns, token records, and cached snapshot summaries that already exist."
            .to_string(),
    ];
    if missing.contains(&UsageCoverageKey::CachedTokens) {
        notes.push(
            "Cached token source is unavailable when provider events do not report cache counts."
                .to_string(),
        );
    }
    if snapshot_facts.source_available {
        notes.push(
            "File line stats use cached snapshot operation summaries and do not read file bodies."
                .to_string(),
        );
    } else if request.remote_connection_id.is_some() || request.remote_ssh_host.is_some() {
        notes.push(
            "Remote snapshot summaries are unavailable for this workspace, so file line stats remain partial."
                .to_string(),
        );
    }

    UsageCoverage {
        level: UsageCoverageLevel::Partial,
        available,
        missing,
        notes,
    }
}

fn build_time_breakdown(turns: &[DialogTurnData]) -> UsageTimeBreakdown {
    if turns.is_empty() {
        return UsageTimeBreakdown {
            accounting: UsageTimeAccounting::Unavailable,
            denominator: UsageTimeDenominator::Unavailable,
            wall_time_ms: None,
            active_turn_ms: None,
            model_ms: None,
            tool_ms: None,
            idle_gap_ms: None,
        };
    }

    // These are persisted lifecycle spans. They intentionally describe recorded
    // session/turn/model-round boundaries, not pure provider streaming
    // throughput such as first-token latency or tokens per second.
    let start = turns.iter().map(|turn| turn.start_time).min().unwrap_or(0);
    let end = turns
        .iter()
        .map(|turn| turn.end_time.unwrap_or(turn.start_time))
        .max()
        .unwrap_or(start);
    let wall_time_ms = end.saturating_sub(start);
    let active_turn_ms: u64 = turns
        .iter()
        .map(|turn| {
            turn.duration_ms
                .or_else(|| turn.end_time.map(|end| end.saturating_sub(turn.start_time)))
                .unwrap_or(0)
        })
        .sum();
    let tool_ms: u64 = turns
        .iter()
        .flat_map(|turn| turn.model_rounds.iter())
        .flat_map(|round| round.tool_items.iter())
        .map(tool_duration_ms)
        .sum();
    let model_round_durations: Vec<u64> = turns
        .iter()
        .flat_map(|turn| turn.model_rounds.iter())
        .filter_map(|round| {
            round
                .end_time
                .map(|end| end.saturating_sub(round.start_time))
        })
        .collect();
    let model_ms = (!model_round_durations.is_empty()).then(|| model_round_durations.iter().sum());

    UsageTimeBreakdown {
        accounting: UsageTimeAccounting::Approximate,
        denominator: UsageTimeDenominator::SessionWallTime,
        wall_time_ms: Some(wall_time_ms),
        active_turn_ms: Some(active_turn_ms),
        model_ms,
        tool_ms: Some(tool_ms),
        idle_gap_ms: Some(wall_time_ms.saturating_sub(active_turn_ms)),
    }
}

fn build_token_breakdown(token_records: &[TokenUsageRecord]) -> UsageTokenBreakdown {
    if token_records.is_empty() {
        return UsageTokenBreakdown {
            source: UsageTokenSource::Unavailable,
            input_tokens: None,
            output_tokens: None,
            total_tokens: None,
            cached_tokens: None,
            cache_coverage: UsageCacheCoverage::Unavailable,
        };
    }

    UsageTokenBreakdown {
        source: UsageTokenSource::TokenUsageRecords,
        input_tokens: Some(
            token_records
                .iter()
                .map(|record| record.input_tokens as u64)
                .sum(),
        ),
        output_tokens: Some(
            token_records
                .iter()
                .map(|record| record.output_tokens as u64)
                .sum(),
        ),
        total_tokens: Some(
            token_records
                .iter()
                .map(|record| record.total_tokens as u64)
                .sum(),
        ),
        cached_tokens: token_records
            .iter()
            .any(|record| record.cached_tokens_available)
            .then(|| {
                token_records
                    .iter()
                    .filter(|record| record.cached_tokens_available)
                    .map(|record| record.cached_tokens as u64)
                    .sum()
            }),
        cache_coverage: if token_records
            .iter()
            .all(|record| record.cached_tokens_available)
        {
            UsageCacheCoverage::Available
        } else if token_records
            .iter()
            .any(|record| record.cached_tokens_available)
        {
            UsageCacheCoverage::Partial
        } else {
            UsageCacheCoverage::Unavailable
        },
    }
}

fn build_model_breakdown(token_records: &[TokenUsageRecord]) -> Vec<UsageModelBreakdown> {
    let mut by_model: HashMap<String, UsageModelBreakdown> = HashMap::new();
    for record in token_records {
        let row = by_model
            .entry(record.model_id.clone())
            .or_insert_with(|| UsageModelBreakdown {
                model_id: record.model_id.clone(),
                call_count: 0,
                input_tokens: Some(0),
                output_tokens: Some(0),
                total_tokens: Some(0),
                cached_tokens: None,
                duration_ms: None,
            });

        row.call_count += 1;
        row.input_tokens = Some(row.input_tokens.unwrap_or(0) + record.input_tokens as u64);
        row.output_tokens = Some(row.output_tokens.unwrap_or(0) + record.output_tokens as u64);
        row.total_tokens = Some(row.total_tokens.unwrap_or(0) + record.total_tokens as u64);
        if record.cached_tokens_available {
            row.cached_tokens = Some(row.cached_tokens.unwrap_or(0) + record.cached_tokens as u64);
        }
    }

    let mut rows: Vec<_> = by_model.into_values().collect();
    rows.sort_by(|a, b| a.model_id.cmp(&b.model_id));
    rows
}

fn build_tool_breakdown(turns: &[DialogTurnData]) -> Vec<UsageToolBreakdown> {
    let mut by_tool: HashMap<String, UsageToolBreakdown> = HashMap::new();
    let mut durations_by_tool: HashMap<String, Vec<u64>> = HashMap::new();

    for tool in iter_tools(turns) {
        let label = redact_usage_label(&tool.tool_name, 80);
        let row = by_tool
            .entry(label.value.clone())
            .or_insert_with(|| UsageToolBreakdown {
                tool_name: label.value.clone(),
                category: classify_tool_usage(&tool.tool_name, Some(&tool.tool_call.input)),
                call_count: 0,
                success_count: 0,
                error_count: 0,
                duration_ms: Some(0),
                p95_duration_ms: None,
                queue_wait_ms: None,
                preflight_ms: None,
                confirmation_wait_ms: None,
                execution_ms: None,
                redacted: label.redacted,
            });
        row.call_count += 1;
        match tool.tool_result.as_ref().map(|result| result.success) {
            Some(true) => row.success_count += 1,
            Some(false) => row.error_count += 1,
            None => {}
        }
        let duration_ms = tool_duration_ms(tool);
        row.duration_ms = Some(row.duration_ms.unwrap_or(0) + duration_ms);
        if duration_ms > 0 {
            durations_by_tool
                .entry(label.value.clone())
                .or_default()
                .push(duration_ms);
        }
        row.redacted |= label.redacted;
    }

    let mut rows: Vec<_> = by_tool
        .into_values()
        .map(|mut row| {
            row.p95_duration_ms = durations_by_tool
                .get(&row.tool_name)
                .and_then(|durations| p95_duration_ms(durations));
            row
        })
        .collect();
    rows.sort_by(|a, b| {
        b.call_count
            .cmp(&a.call_count)
            .then_with(|| a.tool_name.cmp(&b.tool_name))
    });
    rows
}

fn p95_duration_ms(durations: &[u64]) -> Option<u64> {
    if durations.len() < 2 {
        return None;
    }

    let mut sorted = durations.to_vec();
    sorted.sort_unstable();
    let index = ((sorted.len() as f64) * 0.95).ceil() as usize;
    sorted.get(index.saturating_sub(1)).copied()
}

fn build_file_breakdown(
    workspace_root: Option<&str>,
    turns: &[DialogTurnData],
    snapshot_facts: &UsageSnapshotFacts,
) -> UsageFileBreakdown {
    if snapshot_facts.source_available {
        return build_file_breakdown_from_snapshot_operations(
            workspace_root,
            &snapshot_facts.operations,
        );
    }

    build_file_breakdown_from_tool_inputs(workspace_root, turns)
}

fn build_file_breakdown_from_snapshot_operations(
    workspace_root: Option<&str>,
    operations: &[UsageSnapshotOperationSummary],
) -> UsageFileBreakdown {
    let mut files: HashMap<String, UsageFileRow> = HashMap::new();
    let mut turn_indexes_by_path: HashMap<String, BTreeSet<usize>> = HashMap::new();
    let mut operation_ids_by_path: HashMap<String, BTreeSet<String>> = HashMap::new();

    for operation in operations {
        let label = display_workspace_relative_path(workspace_root, &operation.file_path);
        let row = files
            .entry(label.value.clone())
            .or_insert_with(|| UsageFileRow {
                path_label: label.value.clone(),
                operation_count: 0,
                added_lines: Some(0),
                deleted_lines: Some(0),
                session_id: Some(operation.session_id.clone()),
                turn_indexes: vec![],
                operation_ids: vec![],
                redacted: label.redacted,
            });
        row.operation_count += 1;
        row.added_lines = Some(row.added_lines.unwrap_or(0) + operation.lines_added);
        row.deleted_lines = Some(row.deleted_lines.unwrap_or(0) + operation.lines_removed);
        row.session_id
            .get_or_insert_with(|| operation.session_id.clone());
        row.redacted |= label.redacted;

        turn_indexes_by_path
            .entry(label.value.clone())
            .or_default()
            .insert(operation.turn_index);
        operation_ids_by_path
            .entry(label.value)
            .or_default()
            .insert(operation.operation_id.clone());
    }

    let mut rows: Vec<_> = files
        .into_iter()
        .map(|(path_label, mut row)| {
            row.turn_indexes = turn_indexes_by_path
                .remove(&path_label)
                .map(|values| values.into_iter().collect())
                .unwrap_or_default();
            row.operation_ids = operation_ids_by_path
                .remove(&path_label)
                .map(|values| values.into_iter().collect())
                .unwrap_or_default();
            row
        })
        .collect();
    rows.sort_by(|a, b| a.path_label.cmp(&b.path_label));

    UsageFileBreakdown {
        scope: UsageFileScope::SnapshotSummary,
        changed_files: Some(rows.len() as u64),
        added_lines: Some(rows.iter().map(|row| row.added_lines.unwrap_or(0)).sum()),
        deleted_lines: Some(rows.iter().map(|row| row.deleted_lines.unwrap_or(0)).sum()),
        files: rows,
    }
}

fn build_file_breakdown_from_tool_inputs(
    workspace_root: Option<&str>,
    turns: &[DialogTurnData],
) -> UsageFileBreakdown {
    let mut files: HashMap<String, UsageFileRow> = HashMap::new();

    for tool in iter_tools(turns) {
        if !is_file_modification_tool(&tool.tool_name) {
            continue;
        }

        let Some(path) = extract_file_path(tool) else {
            continue;
        };
        let label = display_workspace_relative_path(workspace_root, &path);
        let row = files
            .entry(label.value.clone())
            .or_insert_with(|| UsageFileRow {
                path_label: label.value.clone(),
                operation_count: 0,
                added_lines: None,
                deleted_lines: None,
                session_id: None,
                turn_indexes: vec![],
                operation_ids: vec![],
                redacted: label.redacted,
            });
        row.operation_count += 1;
        row.redacted |= label.redacted;
    }

    let mut rows: Vec<_> = files.into_values().collect();
    rows.sort_by(|a, b| a.path_label.cmp(&b.path_label));
    UsageFileBreakdown {
        scope: if rows.is_empty() {
            UsageFileScope::Unavailable
        } else {
            UsageFileScope::ToolInputsOnly
        },
        changed_files: if rows.is_empty() {
            None
        } else {
            Some(rows.len() as u64)
        },
        added_lines: None,
        deleted_lines: None,
        files: rows,
    }
}

fn build_compression_breakdown(turns: &[DialogTurnData]) -> UsageCompressionBreakdown {
    let manual_compaction_count = turns
        .iter()
        .filter(|turn| turn.kind == DialogTurnKind::ManualCompaction)
        .count() as u64;
    let automatic_compaction_count = iter_tools(turns)
        .filter(|tool| tool.tool_name.to_lowercase().contains("compaction"))
        .count() as u64;

    UsageCompressionBreakdown {
        compaction_count: manual_compaction_count + automatic_compaction_count,
        manual_compaction_count,
        automatic_compaction_count,
        saved_tokens: None,
    }
}

fn build_error_breakdown(turns: &[DialogTurnData]) -> UsageErrorBreakdown {
    let model_errors = turns
        .iter()
        .filter(|turn| turn.status == TurnStatus::Error)
        .count() as u64;
    let tool_errors = iter_tools(turns)
        .filter(|tool| {
            tool.tool_result
                .as_ref()
                .is_some_and(|result| !result.success)
        })
        .count() as u64;

    UsageErrorBreakdown {
        total_errors: model_errors + tool_errors,
        tool_errors,
        model_errors,
        examples: vec![],
    }
}

fn build_slowest_spans(turns: &[DialogTurnData]) -> Vec<UsageSlowSpan> {
    let mut spans = Vec::new();

    for turn in turns {
        if let Some(duration_ms) = turn
            .duration_ms
            .or_else(|| turn.end_time.map(|end| end.saturating_sub(turn.start_time)))
        {
            spans.push(UsageSlowSpan {
                label: format!("turn {}", turn.turn_index),
                kind: UsageSlowSpanKind::Turn,
                duration_ms,
                redacted: false,
            });
        }

        for tool in iter_turn_tools(turn) {
            let label = redact_usage_label(&tool.tool_name, 80);
            spans.push(UsageSlowSpan {
                label: label.value,
                kind: UsageSlowSpanKind::Tool,
                duration_ms: tool_duration_ms(tool),
                redacted: label.redacted,
            });
        }
    }

    spans.sort_by(|a, b| b.duration_ms.cmp(&a.duration_ms));
    spans.truncate(5);
    spans
}

fn collect_redacted_fields(report: &SessionUsageReport) -> Vec<String> {
    let mut fields = HashSet::new();
    if report.tools.iter().any(|tool| tool.redacted) {
        fields.insert("tools.toolName".to_string());
    }
    if report.files.files.iter().any(|file| file.redacted) {
        fields.insert("files.pathLabel".to_string());
    }
    if report.slowest.iter().any(|span| span.redacted) {
        fields.insert("slowest.label".to_string());
    }

    let mut fields: Vec<_> = fields.into_iter().collect();
    fields.sort();
    fields
}

fn iter_tools(turns: &[DialogTurnData]) -> impl Iterator<Item = &ToolItemData> {
    turns.iter().flat_map(iter_turn_tools)
}

fn iter_turn_tools(turn: &DialogTurnData) -> impl Iterator<Item = &ToolItemData> {
    turn.model_rounds
        .iter()
        .flat_map(|round| round.tool_items.iter())
}

fn tool_duration_ms(tool: &ToolItemData) -> u64 {
    tool.duration_ms
        .or_else(|| {
            tool.tool_result
                .as_ref()
                .and_then(|result| result.duration_ms)
        })
        .or_else(|| tool.end_time.map(|end| end.saturating_sub(tool.start_time)))
        .unwrap_or(0)
}

fn is_file_modification_tool(tool_name: &str) -> bool {
    matches!(
        tool_name,
        "write_file" | "edit_file" | "create_file" | "delete_file"
    )
}

fn extract_file_path(tool: &ToolItemData) -> Option<String> {
    let input = tool.tool_call.input.as_object()?;
    ["file_path", "path", "filePath"]
        .into_iter()
        .find_map(|key| input.get(key).and_then(|value| value.as_str()))
        .map(ToOwned::to_owned)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::service::session::{
        DialogTurnData, ModelRoundData, ToolCallData, ToolItemData, ToolResultData, UserMessageData,
    };
    use chrono::TimeZone;

    #[test]
    fn report_marks_cache_unavailable_for_zero_filled_cache_source() {
        let request = test_request(None);
        let records = vec![test_token_record("model-a", 100, 20, 0)];

        let report = build_session_usage_report_from_turns(
            request,
            &[test_turn("turn-1", 0, DialogTurnKind::UserDialog)],
            &records,
            1_778_347_200_000,
        );

        assert_eq!(report.tokens.total_tokens, Some(120));
        assert_eq!(report.tokens.cached_tokens, None);
        assert_eq!(
            report.tokens.cache_coverage,
            UsageCacheCoverage::Unavailable
        );
        assert!(report
            .coverage
            .missing
            .contains(&UsageCoverageKey::CachedTokens));
    }

    #[test]
    fn report_uses_cached_tokens_when_provider_reports_them() {
        let request = test_request(None);
        let mut records = vec![test_token_record("model-a", 100, 20, 12)];
        records[0].cached_tokens_available = true;

        let report = build_session_usage_report_from_turns(
            request,
            &[test_turn("turn-1", 0, DialogTurnKind::UserDialog)],
            &records,
            1_778_347_200_000,
        );

        assert_eq!(report.tokens.cached_tokens, Some(12));
        assert_eq!(report.tokens.cache_coverage, UsageCacheCoverage::Available);
        assert_eq!(report.models[0].cached_tokens, Some(12));
        assert!(report
            .coverage
            .available
            .contains(&UsageCoverageKey::CachedTokens));
    }

    #[test]
    fn report_marks_remote_snapshot_stats_partial() {
        let request = test_request(Some("ssh-1"));

        let report = build_session_usage_report_from_turns(
            request,
            &[test_turn("turn-1", 0, DialogTurnKind::UserDialog)],
            &[],
            1_778_347_200_000,
        );

        assert_eq!(report.workspace.kind, UsageWorkspaceKind::RemoteSsh);
        assert!(report
            .coverage
            .missing
            .contains(&UsageCoverageKey::RemoteSnapshotStats));
    }

    #[test]
    fn report_scopes_by_workspace_identity() {
        let request = test_request(None);

        let report = build_session_usage_report_from_turns(
            request,
            &[test_turn("turn-1", 0, DialogTurnKind::UserDialog)],
            &[],
            1_778_347_200_000,
        );

        assert_eq!(report.session_id, "session-1");
        assert_eq!(report.workspace.kind, UsageWorkspaceKind::Local);
        assert_eq!(
            report.workspace.path_label.as_deref(),
            Some("D:/workspace/bitfun")
        );
    }

    #[test]
    fn report_active_runtime_is_approximate_in_p0() {
        let request = test_request(None);

        let report = build_session_usage_report_from_turns(
            request,
            &[
                test_turn("turn-1", 0, DialogTurnKind::UserDialog),
                test_turn("turn-2", 1, DialogTurnKind::ManualCompaction),
            ],
            &[],
            1_778_347_200_000,
        );

        assert_eq!(report.time.accounting, UsageTimeAccounting::Approximate);
        assert_eq!(report.time.model_ms, Some(400));
        assert_eq!(report.compression.manual_compaction_count, 1);
    }

    #[test]
    fn report_counts_failed_and_cancelled_tool_duration_when_available() {
        let request = test_request(None);
        let turn = test_turn_with_tools(
            "turn-1",
            0,
            DialogTurnKind::UserDialog,
            vec![
                test_tool_item(
                    "tool-failed",
                    "write_file",
                    Some(false),
                    120,
                    "D:/workspace/bitfun/src/main.rs",
                ),
                test_tool_item(
                    "tool-cancelled",
                    "edit_file",
                    None,
                    80,
                    "D:/workspace/bitfun/src/lib.rs",
                ),
            ],
        );

        let report =
            build_session_usage_report_from_turns(request, &[turn], &[], 1_778_347_200_000);

        let failed = report
            .tools
            .iter()
            .find(|tool| tool.tool_name == "write_file")
            .expect("failed tool row");
        assert_eq!(failed.error_count, 1);
        assert_eq!(failed.duration_ms, Some(120));

        let cancelled = report
            .tools
            .iter()
            .find(|tool| tool.tool_name == "edit_file")
            .expect("cancelled tool row");
        assert_eq!(cancelled.call_count, 1);
        assert_eq!(cancelled.duration_ms, Some(80));
    }

    #[test]
    fn report_computes_tool_p95_only_with_multiple_duration_spans() {
        let request = test_request(None);
        let turn = test_turn_with_tools(
            "turn-1",
            0,
            DialogTurnKind::UserDialog,
            vec![
                test_tool_item(
                    "tool-1",
                    "write_file",
                    Some(true),
                    10,
                    "D:/workspace/bitfun/src/a.rs",
                ),
                test_tool_item(
                    "tool-2",
                    "write_file",
                    Some(true),
                    100,
                    "D:/workspace/bitfun/src/b.rs",
                ),
                test_tool_item(
                    "tool-3",
                    "write_file",
                    Some(true),
                    200,
                    "D:/workspace/bitfun/src/c.rs",
                ),
                test_tool_item(
                    "tool-4",
                    "edit_file",
                    Some(true),
                    60,
                    "D:/workspace/bitfun/src/d.rs",
                ),
            ],
        );

        let report =
            build_session_usage_report_from_turns(request, &[turn], &[], 1_778_347_200_000);

        let write = report
            .tools
            .iter()
            .find(|tool| tool.tool_name == "write_file")
            .expect("write tool row");
        assert_eq!(write.duration_ms, Some(310));
        assert_eq!(write.p95_duration_ms, Some(200));

        let edit = report
            .tools
            .iter()
            .find(|tool| tool.tool_name == "edit_file")
            .expect("edit tool row");
        assert_eq!(edit.p95_duration_ms, None);
    }

    #[test]
    fn aggregates_operation_summary_file_stats_without_reading_file_bodies() {
        let request = test_request(None);
        let snapshot_facts = test_snapshot_facts(vec![
            test_snapshot_operation("op-1", 0, "D:/workspace/bitfun/src/main.rs", 10, 2),
            test_snapshot_operation("op-2", 1, "D:/workspace/bitfun/src/main.rs", 5, 1),
            test_snapshot_operation("op-3", 1, "D:/workspace/bitfun/src/lib.rs", 4, 0),
        ]);

        let report = build_session_usage_report_from_sources(
            request,
            &[test_turn("turn-1", 0, DialogTurnKind::UserDialog)],
            &[],
            &snapshot_facts,
            1_778_347_200_000,
        );

        assert_eq!(report.files.scope, UsageFileScope::SnapshotSummary);
        assert_eq!(report.files.changed_files, Some(2));
        assert_eq!(report.files.added_lines, Some(19));
        assert_eq!(report.files.deleted_lines, Some(3));
        assert!(report
            .coverage
            .available
            .contains(&UsageCoverageKey::FileLineStats));
        assert!(!report
            .coverage
            .missing
            .contains(&UsageCoverageKey::FileLineStats));

        let main_row = report
            .files
            .files
            .iter()
            .find(|row| row.path_label == "src/main.rs")
            .expect("main.rs row");
        assert_eq!(main_row.operation_count, 2);
        assert_eq!(main_row.added_lines, Some(15));
        assert_eq!(main_row.deleted_lines, Some(3));
    }

    #[test]
    fn remote_workspace_without_snapshot_marks_file_stats_partial() {
        let request = test_request(Some("ssh-1"));

        let report = build_session_usage_report_from_sources(
            request,
            &[test_turn("turn-1", 0, DialogTurnKind::UserDialog)],
            &[],
            &UsageSnapshotFacts::default(),
            1_778_347_200_000,
        );

        assert_eq!(report.workspace.kind, UsageWorkspaceKind::RemoteSsh);
        assert_eq!(report.files.scope, UsageFileScope::ToolInputsOnly);
        assert_eq!(report.files.changed_files, Some(1));
        assert_eq!(report.files.added_lines, None);
        assert!(report
            .coverage
            .missing
            .contains(&UsageCoverageKey::FileLineStats));
        assert!(report
            .coverage
            .missing
            .contains(&UsageCoverageKey::RemoteSnapshotStats));
    }

    #[test]
    fn file_rows_preserve_operation_turn_and_session_scopes() {
        let request = test_request(None);
        let snapshot_facts = test_snapshot_facts(vec![
            test_snapshot_operation("op-9", 2, "D:/workspace/bitfun/src/main.rs", 1, 0),
            test_snapshot_operation("op-1", 0, "D:/workspace/bitfun/src/main.rs", 2, 1),
        ]);

        let report = build_session_usage_report_from_sources(
            request,
            &[test_turn("turn-1", 0, DialogTurnKind::UserDialog)],
            &[],
            &snapshot_facts,
            1_778_347_200_000,
        );

        let row = report
            .files
            .files
            .iter()
            .find(|row| row.path_label == "src/main.rs")
            .expect("main.rs row");

        assert_eq!(row.session_id.as_deref(), Some("session-1"));
        assert_eq!(row.turn_indexes, vec![0, 2]);
        assert_eq!(row.operation_ids, vec!["op-1", "op-9"]);
    }

    fn test_request(remote_connection_id: Option<&str>) -> SessionUsageReportRequest {
        SessionUsageReportRequest {
            session_id: "session-1".to_string(),
            workspace_path: Some("D:/workspace/bitfun".to_string()),
            remote_connection_id: remote_connection_id.map(ToOwned::to_owned),
            remote_ssh_host: remote_connection_id.map(|_| "host.example".to_string()),
            include_hidden_subagents: true,
        }
    }

    fn test_snapshot_facts(operations: Vec<UsageSnapshotOperationSummary>) -> UsageSnapshotFacts {
        UsageSnapshotFacts {
            source_available: true,
            operations,
        }
    }

    fn test_snapshot_operation(
        operation_id: &str,
        turn_index: usize,
        file_path: &str,
        lines_added: u64,
        lines_removed: u64,
    ) -> UsageSnapshotOperationSummary {
        UsageSnapshotOperationSummary {
            operation_id: operation_id.to_string(),
            session_id: "session-1".to_string(),
            turn_index,
            file_path: file_path.to_string(),
            lines_added,
            lines_removed,
        }
    }

    fn test_turn(turn_id: &str, turn_index: usize, kind: DialogTurnKind) -> DialogTurnData {
        test_turn_with_tools(
            turn_id,
            turn_index,
            kind,
            vec![test_tool_item(
                &format!("tool-{}", turn_index),
                "write_file",
                Some(true),
                100,
                "D:/workspace/bitfun/src/main.rs",
            )],
        )
    }

    fn test_turn_with_tools(
        turn_id: &str,
        turn_index: usize,
        kind: DialogTurnKind,
        tool_items: Vec<ToolItemData>,
    ) -> DialogTurnData {
        DialogTurnData {
            turn_id: turn_id.to_string(),
            turn_index,
            session_id: "session-1".to_string(),
            timestamp: 1_000 + turn_index as u64,
            kind,
            user_message: UserMessageData {
                id: format!("user-{}", turn_index),
                content: "hidden from report".to_string(),
                timestamp: 1_000 + turn_index as u64,
                metadata: None,
            },
            model_rounds: vec![ModelRoundData {
                id: format!("round-{}", turn_index),
                turn_id: turn_id.to_string(),
                round_index: 0,
                timestamp: 1_000 + turn_index as u64,
                text_items: vec![],
                tool_items,
                thinking_items: vec![],
                start_time: 1_000 + turn_index as u64,
                end_time: Some(1_200 + turn_index as u64),
                status: "completed".to_string(),
            }],
            start_time: 1_000 + turn_index as u64,
            end_time: Some(1_300 + turn_index as u64),
            duration_ms: Some(300),
            status: TurnStatus::Completed,
        }
    }

    fn test_tool_item(
        id: &str,
        tool_name: &str,
        success: Option<bool>,
        duration_ms: u64,
        file_path: &str,
    ) -> ToolItemData {
        ToolItemData {
            id: id.to_string(),
            tool_name: tool_name.to_string(),
            tool_call: ToolCallData {
                input: serde_json::json!({
                    "file_path": file_path
                }),
                id: format!("call-{}", id),
            },
            tool_result: success.map(|success| ToolResultData {
                result: serde_json::json!({}),
                success,
                result_for_assistant: None,
                error: (!success).then(|| "tool failed".to_string()),
                duration_ms: Some(duration_ms),
            }),
            ai_intent: None,
            start_time: 1_000,
            end_time: Some(1_000 + duration_ms),
            duration_ms: Some(duration_ms),
            order_index: None,
            is_subagent_item: None,
            parent_task_tool_id: None,
            subagent_session_id: None,
            status: Some(
                match success {
                    Some(true) => "completed",
                    Some(false) => "failed",
                    None => "cancelled",
                }
                .to_string(),
            ),
            interruption_reason: success.is_none().then(|| "cancelled".to_string()),
        }
    }

    fn test_token_record(
        model_id: &str,
        input_tokens: u32,
        output_tokens: u32,
        cached_tokens: u32,
    ) -> TokenUsageRecord {
        TokenUsageRecord {
            model_id: model_id.to_string(),
            session_id: "session-1".to_string(),
            turn_id: "turn-1".to_string(),
            timestamp: Utc.timestamp_millis_opt(1_778_347_200_000).unwrap(),
            input_tokens,
            output_tokens,
            cached_tokens,
            cached_tokens_available: false,
            total_tokens: input_tokens + output_tokens,
            token_details: None,
            is_subagent: false,
        }
    }
}
