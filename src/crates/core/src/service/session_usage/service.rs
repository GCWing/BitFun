use crate::agentic::persistence::PersistenceManager;
use crate::service::session::{
    DialogTurnData, DialogTurnKind, ModelRoundData, ToolItemData, TurnStatus,
};
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
    let reportable_turns: Vec<DialogTurnData> = turns
        .iter()
        .filter(|turn| is_reportable_usage_turn(turn))
        .cloned()
        .collect();
    let turns = reportable_turns.as_slice();
    let mut report = SessionUsageReport::partial_unavailable(&request.session_id, generated_at);
    report.report_id = format!("usage-{}-{}", request.session_id, generated_at);
    report.workspace = build_workspace(&request);
    report.scope = build_scope(turns, request.include_hidden_subagents);
    report.coverage = build_coverage(&request, turns, token_records, snapshot_facts);
    report.time = build_time_breakdown(turns);
    report.tokens = build_token_breakdown(token_records);
    report.models = build_model_breakdown(turns, token_records);
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

fn is_reportable_usage_turn(turn: &DialogTurnData) -> bool {
    turn.kind != DialogTurnKind::LocalCommand
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
        .any(has_model_timing_fact)
    {
        available.push(UsageCoverageKey::ModelRoundTiming);
    }
    if iter_tools(turns).any(has_tool_phase_timing_fact) {
        available.push(UsageCoverageKey::ToolPhaseTiming);
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
    let active_intervals = turns
        .iter()
        .filter_map(|turn| turn.end_time.map(|end| (turn.start_time, end)))
        .collect::<Vec<_>>();
    let active_turn_ms = (!active_intervals.is_empty())
        .then(|| duration_union_ms(&active_intervals))
        .or_else(|| {
            let summed: u64 = turns.iter().filter_map(|turn| turn.duration_ms).sum();
            (summed > 0).then_some(summed)
        });
    let tool_durations = turns
        .iter()
        .flat_map(|turn| turn.model_rounds.iter())
        .flat_map(|round| round.tool_items.iter())
        .filter_map(tool_duration_ms)
        .collect::<Vec<_>>();
    let tool_ms = Some(tool_durations.iter().sum());
    let model_round_durations: Vec<u64> = turns
        .iter()
        .flat_map(|turn| turn.model_rounds.iter())
        .filter_map(model_round_duration_ms)
        .collect();
    let model_ms = (!model_round_durations.is_empty()).then(|| model_round_durations.iter().sum());
    let has_incomplete_turn_span = turns.iter().any(|turn| turn.end_time.is_none());
    let has_legacy_model_span = turns
        .iter()
        .flat_map(|turn| turn.model_rounds.iter())
        .any(|round| round.duration_ms.is_none() && round.end_time.is_some());

    UsageTimeBreakdown {
        accounting: if has_incomplete_turn_span || has_legacy_model_span {
            UsageTimeAccounting::Approximate
        } else {
            UsageTimeAccounting::Exact
        },
        denominator: if active_turn_ms.is_some() {
            UsageTimeDenominator::ActiveTurnTime
        } else {
            UsageTimeDenominator::SessionWallTime
        },
        wall_time_ms: Some(wall_time_ms),
        active_turn_ms,
        model_ms,
        tool_ms,
        idle_gap_ms: active_turn_ms.map(|active| wall_time_ms.saturating_sub(active)),
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

fn build_model_breakdown(
    turns: &[DialogTurnData],
    token_records: &[TokenUsageRecord],
) -> Vec<UsageModelBreakdown> {
    let mut by_model: HashMap<String, UsageModelBreakdown> = HashMap::new();
    let mut span_counts_by_model: HashMap<String, u64> = HashMap::new();
    let turn_indexes_by_id: HashMap<&str, usize> = turns
        .iter()
        .map(|turn| (turn.turn_id.as_str(), turn.turn_index))
        .collect();
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
                sample_turn_id: None,
                sample_turn_index: None,
            });

        row.call_count += 1;
        row.input_tokens = Some(row.input_tokens.unwrap_or(0) + record.input_tokens as u64);
        row.output_tokens = Some(row.output_tokens.unwrap_or(0) + record.output_tokens as u64);
        row.total_tokens = Some(row.total_tokens.unwrap_or(0) + record.total_tokens as u64);
        if record.cached_tokens_available {
            row.cached_tokens = Some(row.cached_tokens.unwrap_or(0) + record.cached_tokens as u64);
        }
        set_turn_anchor_if_missing(
            &mut row.sample_turn_id,
            &mut row.sample_turn_index,
            &record.turn_id,
            turn_indexes_by_id.get(record.turn_id.as_str()).copied(),
        );
    }

    for turn in turns {
        for round in &turn.model_rounds {
            let Some(duration_ms) = model_round_duration_ms(round) else {
                continue;
            };
            let model_id = model_round_label(round);
            let row = by_model
                .entry(model_id.clone())
                .or_insert_with(|| UsageModelBreakdown {
                    model_id: model_id.clone(),
                    call_count: 0,
                    input_tokens: None,
                    output_tokens: None,
                    total_tokens: None,
                    cached_tokens: None,
                    duration_ms: Some(0),
                    sample_turn_id: None,
                    sample_turn_index: None,
                });

            row.duration_ms = Some(row.duration_ms.unwrap_or(0) + duration_ms);
            set_turn_anchor_if_missing(
                &mut row.sample_turn_id,
                &mut row.sample_turn_index,
                &turn.turn_id,
                Some(turn.turn_index),
            );
            *span_counts_by_model.entry(model_id).or_default() += 1;
        }
    }

    for (model_id, span_count) in span_counts_by_model {
        if let Some(row) = by_model.get_mut(&model_id) {
            row.call_count = row.call_count.max(span_count);
        }
    }

    let mut rows: Vec<_> = by_model.into_values().collect();
    rows.sort_by(|a, b| a.model_id.cmp(&b.model_id));
    rows
}

fn build_tool_breakdown(turns: &[DialogTurnData]) -> Vec<UsageToolBreakdown> {
    let mut by_tool: HashMap<String, UsageToolBreakdown> = HashMap::new();
    let mut durations_by_tool: HashMap<String, Vec<u64>> = HashMap::new();

    for turn in turns {
        for tool in iter_turn_tools(turn) {
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
                    sample_turn_id: None,
                    sample_turn_index: None,
                    sample_item_id: None,
                    redacted: label.redacted,
                });
            row.call_count += 1;
            match tool.tool_result.as_ref().map(|result| result.success) {
                Some(true) => row.success_count += 1,
                Some(false) => row.error_count += 1,
                None => {}
            }
            let duration_ms = tool_duration_ms(tool).unwrap_or(0);
            row.duration_ms = Some(row.duration_ms.unwrap_or(0) + duration_ms);
            if duration_ms > 0 {
                durations_by_tool
                    .entry(label.value.clone())
                    .or_default()
                    .push(duration_ms);
            }
            add_optional_duration(&mut row.queue_wait_ms, tool.queue_wait_ms);
            add_optional_duration(&mut row.preflight_ms, tool.preflight_ms);
            add_optional_duration(&mut row.confirmation_wait_ms, tool.confirmation_wait_ms);
            add_optional_duration(&mut row.execution_ms, tool.execution_ms);
            set_item_anchor_if_missing(
                &mut row.sample_turn_id,
                &mut row.sample_turn_index,
                &mut row.sample_item_id,
                &turn.turn_id,
                turn.turn_index,
                &tool.id,
            );
            row.redacted |= label.redacted;
        }
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
    let mut turn_indexes_by_path: HashMap<String, BTreeSet<usize>> = HashMap::new();
    let mut operation_ids_by_path: HashMap<String, BTreeSet<String>> = HashMap::new();

    for turn in turns {
        for tool in iter_turn_tools(turn) {
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

            turn_indexes_by_path
                .entry(label.value.clone())
                .or_default()
                .insert(turn.turn_index);
            operation_ids_by_path
                .entry(label.value)
                .or_default()
                .insert(tool.id.clone());
        }
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
    let mut examples = Vec::new();

    if model_errors > 0 {
        let sample_model_error_turn = turns.iter().find(|turn| turn.status == TurnStatus::Error);
        examples.push(UsageErrorExample {
            label: "Model/runtime turn errors".to_string(),
            count: model_errors,
            sample_turn_id: sample_model_error_turn.map(|turn| turn.turn_id.clone()),
            sample_turn_index: sample_model_error_turn.map(|turn| turn.turn_index),
            sample_item_id: None,
            redacted: false,
        });
    }

    let mut tool_error_counts: HashMap<String, UsageErrorExample> = HashMap::new();
    for turn in turns {
        for tool in iter_turn_tools(turn).filter(|tool| {
            tool.tool_result
                .as_ref()
                .is_some_and(|result| !result.success)
        }) {
            let label = redact_usage_label(&tool.tool_name, 80);
            let row = tool_error_counts
                .entry(label.value.clone())
                .or_insert_with(|| UsageErrorExample {
                    label: label.value.clone(),
                    count: 0,
                    sample_turn_id: None,
                    sample_turn_index: None,
                    sample_item_id: None,
                    redacted: label.redacted,
                });
            row.count += 1;
            set_item_anchor_if_missing(
                &mut row.sample_turn_id,
                &mut row.sample_turn_index,
                &mut row.sample_item_id,
                &turn.turn_id,
                turn.turn_index,
                &tool.id,
            );
            row.redacted |= label.redacted;
        }
    }

    let mut tool_examples: Vec<_> = tool_error_counts.into_values().collect();
    tool_examples.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.label.cmp(&b.label)));
    examples.extend(tool_examples.into_iter().take(4));

    UsageErrorBreakdown {
        total_errors: model_errors + tool_errors,
        tool_errors,
        model_errors,
        examples,
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
                turn_id: Some(turn.turn_id.clone()),
                turn_index: Some(turn.turn_index),
            });
        }

        for round in &turn.model_rounds {
            if let Some(duration_ms) = model_round_duration_ms(round) {
                spans.push(UsageSlowSpan {
                    label: model_round_label(round),
                    kind: UsageSlowSpanKind::Model,
                    duration_ms,
                    redacted: false,
                    turn_id: Some(turn.turn_id.clone()),
                    turn_index: Some(turn.turn_index),
                });
            }
        }

        for tool in iter_turn_tools(turn) {
            let label = redact_usage_label(&tool.tool_name, 80);
            if let Some(duration_ms) = tool_duration_ms(tool) {
                spans.push(UsageSlowSpan {
                    label: label.value,
                    kind: UsageSlowSpanKind::Tool,
                    duration_ms,
                    redacted: label.redacted,
                    turn_id: Some(turn.turn_id.clone()),
                    turn_index: Some(turn.turn_index),
                });
            }
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

fn model_round_duration_ms(round: &ModelRoundData) -> Option<u64> {
    round.duration_ms.or_else(|| {
        round
            .end_time
            .map(|end| end.saturating_sub(round.start_time))
    })
}

fn model_round_label(round: &ModelRoundData) -> String {
    round
        .model_id
        .as_deref()
        .or(round.model_alias.as_deref())
        .map(|value| redact_usage_label(value, 80).value)
        .unwrap_or_else(|| "unknown_model".to_string())
}

fn has_model_timing_fact(round: &ModelRoundData) -> bool {
    model_round_duration_ms(round).is_some()
        || round.first_chunk_ms.is_some()
        || round.first_visible_output_ms.is_some()
        || round.stream_duration_ms.is_some()
        || round.attempt_count.is_some()
        || round.failure_category.is_some()
}

fn has_tool_phase_timing_fact(tool: &ToolItemData) -> bool {
    tool.queue_wait_ms.is_some()
        || tool.preflight_ms.is_some()
        || tool.confirmation_wait_ms.is_some()
        || tool.execution_ms.is_some()
}

fn tool_duration_ms(tool: &ToolItemData) -> Option<u64> {
    tool.duration_ms
        .or_else(|| {
            tool.tool_result
                .as_ref()
                .and_then(|result| result.duration_ms)
        })
        .or_else(|| tool.end_time.map(|end| end.saturating_sub(tool.start_time)))
}

fn add_optional_duration(total: &mut Option<u64>, value: Option<u64>) {
    if let Some(value) = value {
        *total = Some(total.unwrap_or(0) + value);
    }
}

fn set_turn_anchor_if_missing(
    sample_turn_id: &mut Option<String>,
    sample_turn_index: &mut Option<usize>,
    turn_id: &str,
    turn_index: Option<usize>,
) {
    if sample_turn_id.is_none() {
        *sample_turn_id = Some(turn_id.to_string());
    }
    if sample_turn_index.is_none() {
        *sample_turn_index = turn_index;
    }
}

fn set_item_anchor_if_missing(
    sample_turn_id: &mut Option<String>,
    sample_turn_index: &mut Option<usize>,
    sample_item_id: &mut Option<String>,
    turn_id: &str,
    turn_index: usize,
    item_id: &str,
) {
    set_turn_anchor_if_missing(sample_turn_id, sample_turn_index, turn_id, Some(turn_index));
    if sample_item_id.is_none() {
        *sample_item_id = Some(item_id.to_string());
    }
}

fn duration_union_ms(intervals: &[(u64, u64)]) -> u64 {
    let mut normalized = intervals
        .iter()
        .filter_map(|(start, end)| (end > start).then_some((*start, *end)))
        .collect::<Vec<_>>();
    if normalized.is_empty() {
        return 0;
    }

    normalized.sort_unstable_by_key(|(start, end)| (*start, *end));
    let mut total = 0;
    let (mut current_start, mut current_end) = normalized[0];

    for (start, end) in normalized.into_iter().skip(1) {
        if start <= current_end {
            current_end = current_end.max(end);
        } else {
            total += current_end.saturating_sub(current_start);
            current_start = start;
            current_end = end;
        }
    }

    total + current_end.saturating_sub(current_start)
}

fn is_file_modification_tool(tool_name: &str) -> bool {
    matches!(
        tool_name,
        "Write"
            | "Edit"
            | "Delete"
            | "write_file"
            | "edit_file"
            | "create_file"
            | "delete_file"
            | "rename_file"
            | "move_file"
            | "search_replace"
    )
}

fn extract_file_path(tool: &ToolItemData) -> Option<String> {
    let input = tool.tool_call.input.as_object()?;
    ["file_path", "path", "filePath", "target_file", "filename"]
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
    fn report_active_runtime_uses_active_span_union() {
        let request = test_request(None);
        let mut first = test_turn("turn-1", 0, DialogTurnKind::UserDialog);
        first.start_time = 1_000;
        first.end_time = Some(1_300);
        first.duration_ms = Some(300);
        first.model_rounds[0].start_time = 1_010;
        first.model_rounds[0].end_time = Some(1_110);
        first.model_rounds[0].duration_ms = Some(100);

        let mut second = test_turn("turn-2", 1, DialogTurnKind::ManualCompaction);
        second.start_time = 1_200;
        second.end_time = Some(1_500);
        second.duration_ms = Some(300);
        second.model_rounds[0].start_time = 1_220;
        second.model_rounds[0].end_time = Some(1_340);
        second.model_rounds[0].duration_ms = Some(120);

        let report = build_session_usage_report_from_turns(
            request,
            &[first, second],
            &[],
            1_778_347_200_000,
        );

        assert_eq!(report.time.accounting, UsageTimeAccounting::Exact);
        assert_eq!(
            report.time.denominator,
            UsageTimeDenominator::ActiveTurnTime
        );
        assert_eq!(report.time.wall_time_ms, Some(500));
        assert_eq!(report.time.active_turn_ms, Some(500));
        assert_eq!(report.time.model_ms, Some(220));
        assert_eq!(report.time.idle_gap_ms, Some(0));
        assert_eq!(report.compression.manual_compaction_count, 1);
    }

    #[test]
    fn report_excludes_local_command_turns_from_usage_metrics() {
        let request = test_request(None);
        let mut user_turn = test_turn("turn-1", 0, DialogTurnKind::UserDialog);
        user_turn.start_time = 1_000;
        user_turn.end_time = Some(1_300);
        user_turn.duration_ms = Some(300);
        user_turn.model_rounds[0].duration_ms = Some(200);

        let mut local_usage_turn = test_turn("local-usage-1", 1, DialogTurnKind::LocalCommand);
        local_usage_turn.start_time = 50_000;
        local_usage_turn.end_time = Some(50_000);
        local_usage_turn.duration_ms = Some(0);
        local_usage_turn.model_rounds[0].duration_ms = Some(9_000);

        let report = build_session_usage_report_from_turns(
            request,
            &[user_turn, local_usage_turn],
            &[],
            1_778_347_200_000,
        );

        assert_eq!(report.scope.turn_count, 1);
        assert_eq!(report.scope.from_turn_id.as_deref(), Some("turn-1"));
        assert_eq!(report.scope.to_turn_id.as_deref(), Some("turn-1"));
        assert_eq!(report.time.wall_time_ms, Some(300));
        assert_eq!(report.time.active_turn_ms, Some(300));
        assert_eq!(report.time.model_ms, Some(200));
        assert_eq!(report.models[0].duration_ms, Some(200));
        assert_eq!(report.tools[0].call_count, 1);
        assert_eq!(report.files.files[0].operation_count, 1);
    }

    #[test]
    fn report_uses_persisted_model_span_facts_without_token_records() {
        let request = test_request(None);
        let mut turn = test_turn("turn-1", 0, DialogTurnKind::UserDialog);
        turn.model_rounds = vec![
            test_model_round("round-a", "turn-1", 0, "model-a", 90),
            test_model_round("round-b", "turn-1", 1, "model-b", 140),
        ];

        let report =
            build_session_usage_report_from_turns(request, &[turn], &[], 1_778_347_200_000);

        assert!(report
            .coverage
            .available
            .contains(&UsageCoverageKey::ModelRoundTiming));
        assert!(!report
            .coverage
            .missing
            .contains(&UsageCoverageKey::ModelRoundTiming));
        assert_eq!(
            report
                .models
                .iter()
                .map(|model| (
                    model.model_id.as_str(),
                    model.call_count,
                    model.duration_ms,
                    model.total_tokens
                ))
                .collect::<Vec<_>>(),
            vec![
                ("model-a", 1, Some(90), None),
                ("model-b", 1, Some(140), None),
            ]
        );
        assert!(report.slowest.iter().any(|span| {
            span.kind == UsageSlowSpanKind::Model
                && span.label == "model-b"
                && span.duration_ms == 140
        }));
    }

    #[test]
    fn report_uses_clear_label_when_model_identity_is_missing() {
        let request = test_request(None);
        let mut turn = test_turn("turn-1", 0, DialogTurnKind::UserDialog);
        turn.model_rounds[0].model_id = None;
        turn.model_rounds[0].model_alias = None;
        turn.model_rounds[0].duration_ms = Some(180);

        let report =
            build_session_usage_report_from_turns(request, &[turn], &[], 1_778_347_200_000);

        assert_eq!(report.models[0].model_id, "unknown_model");
        assert!(report.slowest.iter().any(|span| {
            span.kind == UsageSlowSpanKind::Model
                && span.label == "unknown_model"
                && span.duration_ms == 180
        }));
    }

    #[test]
    fn report_adds_turn_anchors_to_slowest_spans() {
        let request = test_request(None);
        let mut turn = test_turn_with_tools(
            "turn-7",
            7,
            DialogTurnKind::UserDialog,
            vec![test_tool_item(
                "tool-7",
                "write_file",
                Some(true),
                500,
                "D:/workspace/bitfun/src/main.rs",
            )],
        );
        turn.duration_ms = Some(900);
        turn.model_rounds[0].duration_ms = Some(700);

        let report =
            build_session_usage_report_from_turns(request, &[turn], &[], 1_778_347_200_000);

        for kind in [
            UsageSlowSpanKind::Turn,
            UsageSlowSpanKind::Model,
            UsageSlowSpanKind::Tool,
        ] {
            let span = report
                .slowest
                .iter()
                .find(|span| span.kind == kind)
                .expect("anchored slow span");
            assert_eq!(span.turn_id.as_deref(), Some("turn-7"));
            assert_eq!(span.turn_index, Some(7));
        }
    }

    #[test]
    fn report_adds_representative_anchors_to_model_tool_and_error_rows() {
        let request = test_request(None);
        let mut failed_turn = test_turn_with_tools(
            "turn-2",
            2,
            DialogTurnKind::UserDialog,
            vec![test_tool_item(
                "tool-failed",
                "write_file",
                Some(false),
                120,
                "D:/workspace/bitfun/src/main.rs",
            )],
        );
        failed_turn.model_rounds[0].model_id = Some("model-a".to_string());
        failed_turn.model_rounds[0].model_alias = Some("model-a".to_string());
        failed_turn.model_rounds[0].duration_ms = Some(220);
        let mut model_error_turn =
            test_turn_with_tools("turn-4", 4, DialogTurnKind::UserDialog, vec![]);
        model_error_turn.status = TurnStatus::Error;

        let report = build_session_usage_report_from_turns(
            request,
            &[failed_turn, model_error_turn],
            &[],
            1_778_347_200_000,
        );

        let model = report
            .models
            .iter()
            .find(|model| model.model_id == "model-a")
            .expect("model row");
        assert_eq!(model.sample_turn_id.as_deref(), Some("turn-2"));
        assert_eq!(model.sample_turn_index, Some(2));

        let tool = report
            .tools
            .iter()
            .find(|tool| tool.tool_name == "write_file")
            .expect("tool row");
        assert_eq!(tool.sample_turn_id.as_deref(), Some("turn-2"));
        assert_eq!(tool.sample_turn_index, Some(2));
        assert_eq!(tool.sample_item_id.as_deref(), Some("tool-failed"));

        let tool_error = report
            .errors
            .examples
            .iter()
            .find(|example| example.label == "write_file")
            .expect("tool error example");
        assert_eq!(tool_error.sample_turn_id.as_deref(), Some("turn-2"));
        assert_eq!(tool_error.sample_turn_index, Some(2));
        assert_eq!(tool_error.sample_item_id.as_deref(), Some("tool-failed"));

        let model_error = report
            .errors
            .examples
            .iter()
            .find(|example| example.label == "Model/runtime turn errors")
            .expect("model error example");
        assert_eq!(model_error.sample_turn_id.as_deref(), Some("turn-4"));
        assert_eq!(model_error.sample_turn_index, Some(4));
        assert_eq!(model_error.sample_item_id, None);
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
    fn report_sums_tool_phase_timings_and_marks_phase_coverage_available() {
        let request = test_request(None);
        let mut first = test_tool_item(
            "tool-1",
            "write_file",
            Some(true),
            100,
            "D:/workspace/bitfun/src/a.rs",
        );
        first.queue_wait_ms = Some(7);
        first.preflight_ms = Some(11);
        first.confirmation_wait_ms = Some(13);
        first.execution_ms = Some(69);

        let mut second = test_tool_item(
            "tool-2",
            "write_file",
            Some(true),
            80,
            "D:/workspace/bitfun/src/b.rs",
        );
        second.queue_wait_ms = Some(3);
        second.preflight_ms = Some(5);
        second.confirmation_wait_ms = Some(0);
        second.execution_ms = Some(72);

        let turn =
            test_turn_with_tools("turn-1", 0, DialogTurnKind::UserDialog, vec![first, second]);

        let report =
            build_session_usage_report_from_turns(request, &[turn], &[], 1_778_347_200_000);

        let write = report
            .tools
            .iter()
            .find(|tool| tool.tool_name == "write_file")
            .expect("write tool row");
        assert_eq!(write.duration_ms, Some(180));
        assert_eq!(write.queue_wait_ms, Some(10));
        assert_eq!(write.preflight_ms, Some(16));
        assert_eq!(write.confirmation_wait_ms, Some(13));
        assert_eq!(write.execution_ms, Some(141));
        assert!(report
            .coverage
            .available
            .contains(&UsageCoverageKey::ToolPhaseTiming));
        assert!(!report
            .coverage
            .missing
            .contains(&UsageCoverageKey::ToolPhaseTiming));
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
    fn remote_workspace_uses_wrapped_tool_inputs_for_file_rows() {
        let request = test_request(Some("ssh-1"));
        let turn = test_turn_with_tools(
            "turn-1",
            0,
            DialogTurnKind::UserDialog,
            vec![
                test_tool_item_with_input(
                    "tool-1",
                    "Write",
                    Some(true),
                    100,
                    serde_json::json!({ "file_path": "D:/workspace/bitfun/src/main.rs" }),
                ),
                test_tool_item_with_input(
                    "tool-2",
                    "Edit",
                    Some(true),
                    80,
                    serde_json::json!({ "target_file": "D:/workspace/bitfun/src/lib.rs" }),
                ),
            ],
        );

        let report = build_session_usage_report_from_sources(
            request,
            &[turn],
            &[],
            &UsageSnapshotFacts::default(),
            1_778_347_200_000,
        );

        assert_eq!(report.workspace.kind, UsageWorkspaceKind::RemoteSsh);
        assert_eq!(report.files.scope, UsageFileScope::ToolInputsOnly);
        assert_eq!(report.files.changed_files, Some(2));
        assert_eq!(
            report
                .files
                .files
                .iter()
                .map(|row| row.path_label.as_str())
                .collect::<Vec<_>>(),
            vec!["src/lib.rs", "src/main.rs"]
        );
    }

    #[test]
    fn report_includes_error_examples_for_failed_turns_and_tools() {
        let request = test_request(None);
        let mut failed_turn = test_turn_with_tools(
            "turn-1",
            0,
            DialogTurnKind::UserDialog,
            vec![
                test_tool_item(
                    "tool-1",
                    "Write",
                    Some(false),
                    100,
                    "D:/workspace/bitfun/src/main.rs",
                ),
                test_tool_item("tool-2", "Bash", Some(false), 120, "D:/workspace/bitfun"),
            ],
        );
        failed_turn.status = TurnStatus::Error;

        let report =
            build_session_usage_report_from_turns(request, &[failed_turn], &[], 1_778_347_200_000);

        assert_eq!(report.errors.total_errors, 3);
        assert_eq!(report.errors.tool_errors, 2);
        assert_eq!(report.errors.model_errors, 1);
        assert_eq!(
            report
                .errors
                .examples
                .iter()
                .map(|example| (example.label.as_str(), example.count))
                .collect::<Vec<_>>(),
            vec![("Model/runtime turn errors", 1), ("Bash", 1), ("Write", 1),]
        );
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
                duration_ms: Some(200),
                provider_id: None,
                model_id: Some("model-a".to_string()),
                model_alias: Some("model-a".to_string()),
                first_chunk_ms: None,
                first_visible_output_ms: None,
                stream_duration_ms: None,
                attempt_count: None,
                failure_category: None,
                token_details: None,
                status: "completed".to_string(),
            }],
            start_time: 1_000 + turn_index as u64,
            end_time: Some(1_300 + turn_index as u64),
            duration_ms: Some(300),
            status: TurnStatus::Completed,
        }
    }

    fn test_model_round(
        id: &str,
        turn_id: &str,
        round_index: usize,
        model_id: &str,
        duration_ms: u64,
    ) -> ModelRoundData {
        ModelRoundData {
            id: id.to_string(),
            turn_id: turn_id.to_string(),
            round_index,
            timestamp: 1_000 + round_index as u64,
            text_items: vec![],
            tool_items: vec![],
            thinking_items: vec![],
            start_time: 1_000 + round_index as u64,
            end_time: Some(1_000 + round_index as u64 + duration_ms),
            duration_ms: Some(duration_ms),
            provider_id: Some("test-provider".to_string()),
            model_id: Some(model_id.to_string()),
            model_alias: Some(model_id.to_string()),
            first_chunk_ms: Some(5),
            first_visible_output_ms: Some(8),
            stream_duration_ms: Some(duration_ms.saturating_sub(10)),
            attempt_count: Some(1),
            failure_category: None,
            token_details: None,
            status: "completed".to_string(),
        }
    }

    fn test_tool_item(
        id: &str,
        tool_name: &str,
        success: Option<bool>,
        duration_ms: u64,
        file_path: &str,
    ) -> ToolItemData {
        test_tool_item_with_input(
            id,
            tool_name,
            success,
            duration_ms,
            serde_json::json!({
                "file_path": file_path
            }),
        )
    }

    fn test_tool_item_with_input(
        id: &str,
        tool_name: &str,
        success: Option<bool>,
        duration_ms: u64,
        input: serde_json::Value,
    ) -> ToolItemData {
        ToolItemData {
            id: id.to_string(),
            tool_name: tool_name.to_string(),
            tool_call: ToolCallData {
                input,
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
            subagent_model_id: None,
            subagent_model_alias: None,
            status: Some(
                match success {
                    Some(true) => "completed",
                    Some(false) => "failed",
                    None => "cancelled",
                }
                .to_string(),
            ),
            interruption_reason: success.is_none().then(|| "cancelled".to_string()),
            queue_wait_ms: None,
            preflight_ms: None,
            confirmation_wait_ms: None,
            execution_ms: None,
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
