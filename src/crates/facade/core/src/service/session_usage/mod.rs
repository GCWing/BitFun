pub mod service;

pub use bitfun_services_core::session_usage::{classifier, redaction, render, types};
pub use bitfun_services_core::session_usage::{
    classify_tool_usage, display_workspace_relative_path, redact_usage_label,
    render_usage_report_markdown, render_usage_report_terminal, RedactedLabel, UsageToolCategory,
};
pub use bitfun_services_core::session_usage::{
    SessionUsageReport, UsageCacheCoverage, UsageCompressionBreakdown, UsageCoverage,
    UsageCoverageKey, UsageCoverageLevel, UsageErrorBreakdown, UsageErrorExample,
    UsageFileBreakdown, UsageFileRow, UsageFileScope, UsageModelBreakdown, UsagePrivacy,
    UsageScope, UsageScopeKind, UsageSlowSpan, UsageSlowSpanKind, UsageSnapshotFacts,
    UsageSnapshotOperationSummary, UsageTimeAccounting, UsageTimeBreakdown, UsageTimeDenominator,
    UsageTokenBreakdown, UsageTokenSource, UsageToolBreakdown, UsageWorkspace, UsageWorkspaceKind,
    SESSION_USAGE_REPORT_SCHEMA_VERSION,
};
pub use service::{
    build_session_usage_report_from_sources, build_session_usage_report_from_turns,
    generate_session_usage_report, SessionUsageReportRequest,
};
