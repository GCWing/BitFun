pub use bitfun_core_types::{
    SessionUsageReport, UsageCacheCoverage, UsageCompressionBreakdown, UsageCoverage,
    UsageCoverageKey, UsageCoverageLevel, UsageErrorBreakdown, UsageErrorExample,
    UsageFileBreakdown, UsageFileRow, UsageFileScope, UsageModelBreakdown, UsagePrivacy,
    UsageScope, UsageScopeKind, UsageSlowSpan, UsageSlowSpanKind, UsageSnapshotFacts,
    UsageSnapshotOperationSummary, UsageTimeAccounting, UsageTimeBreakdown, UsageTimeDenominator,
    UsageTokenBreakdown, UsageTokenSource, UsageToolBreakdown, UsageWorkspace, UsageWorkspaceKind,
    SESSION_USAGE_REPORT_SCHEMA_VERSION,
};

#[cfg(test)]
pub(crate) fn test_report() -> SessionUsageReport {
    let mut report = SessionUsageReport::partial_unavailable("session-1", 1_778_347_200_000);
    report.report_id = "usage-session-1-1778347200000".to_string();
    report.workspace = UsageWorkspace {
        kind: UsageWorkspaceKind::Local,
        path_label: Some("D:/workspace/bitfun".to_string()),
        workspace_id: Some("workspace-1".to_string()),
        remote_connection_id: None,
        remote_ssh_host: None,
    };
    report.scope = UsageScope {
        kind: UsageScopeKind::EntireSession,
        turn_count: 3,
        from_turn_id: Some("turn-1".to_string()),
        to_turn_id: Some("turn-3".to_string()),
        includes_subagents: true,
    };
    report.coverage.available = vec![UsageCoverageKey::WorkspaceIdentity];
    report.coverage.missing = vec![
        UsageCoverageKey::CachedTokens,
        UsageCoverageKey::TokenDetailBreakdown,
    ];
    report.time = UsageTimeBreakdown {
        accounting: UsageTimeAccounting::Approximate,
        denominator: UsageTimeDenominator::SessionWallTime,
        wall_time_ms: Some(62_000),
        active_turn_ms: Some(51_000),
        model_ms: None,
        tool_ms: Some(12_000),
        idle_gap_ms: Some(11_000),
    };
    report.tokens = UsageTokenBreakdown {
        source: UsageTokenSource::TokenUsageRecords,
        input_tokens: Some(1200),
        output_tokens: Some(340),
        total_tokens: Some(1540),
        cached_tokens: None,
        cache_coverage: UsageCacheCoverage::Unavailable,
        cache_hit_rate: None,
    };
    report.models = vec![UsageModelBreakdown {
        model_id: "test-model".to_string(),
        call_count: 2,
        input_tokens: Some(1200),
        output_tokens: Some(340),
        total_tokens: Some(1540),
        cached_tokens: None,
        cache_hit_rate: None,
        duration_ms: None,
        sample_turn_id: Some("turn-1".to_string()),
        sample_turn_index: Some(0),
    }];
    report.tools = vec![UsageToolBreakdown {
        tool_name: "read_file".to_string(),
        category: bitfun_core_types::UsageToolCategory::File,
        call_count: 1,
        success_count: 1,
        error_count: 0,
        duration_ms: Some(1200),
        p95_duration_ms: None,
        queue_wait_ms: None,
        preflight_ms: None,
        confirmation_wait_ms: None,
        execution_ms: None,
        sample_turn_id: Some("turn-1".to_string()),
        sample_turn_index: Some(0),
        sample_item_id: Some("tool-1".to_string()),
        redacted: false,
    }];
    report.files = UsageFileBreakdown {
        scope: UsageFileScope::ToolInputsOnly,
        changed_files: Some(1),
        added_lines: None,
        deleted_lines: None,
        files: vec![UsageFileRow {
            path_label: "src/main.rs".to_string(),
            operation_count: 1,
            added_lines: None,
            deleted_lines: None,
            session_id: None,
            turn_indexes: vec![],
            operation_ids: vec![],
            redacted: false,
        }],
    };
    report
}
