use crate::service::session_usage::types::*;

pub fn render_usage_report_terminal(report: &SessionUsageReport) -> String {
    let mut out = Vec::new();
    out.push("Session usage".to_string());
    out.push(format!("Report: {}", report.report_id));
    out.push(format!("Session: {}", report.session_id));
    out.push(format!(
        "Workspace: {}",
        report
            .workspace
            .path_label
            .as_deref()
            .unwrap_or("unavailable")
    ));
    out.push(format!(
        "Scope: {} turns{}",
        report.scope.turn_count,
        if report.scope.includes_subagents {
            " including subagents"
        } else {
            ""
        }
    ));
    out.push(format!(
        "Coverage: {}",
        coverage_level_label(&report.coverage.level)
    ));
    out.push(format!(
        "Time accounting: {} ({})",
        time_accounting_label(&report.time.accounting),
        time_denominator_label(&report.time.denominator)
    ));
    out.push(format!(
        "Session span: {}",
        format_optional_duration(report.time.wall_time_ms)
    ));
    out.push(format!(
        "Recorded turn time: {}",
        format_optional_duration(report.time.active_turn_ms)
    ));
    out.push(format!(
        "Tool call time: {}",
        format_optional_duration(report.time.tool_ms)
    ));
    out.push(format!(
        "Tokens: input {}, output {}, total {}",
        format_optional_number(report.tokens.input_tokens),
        format_optional_number(report.tokens.output_tokens),
        format_optional_number(report.tokens.total_tokens)
    ));
    out.push(format!(
        "Cached tokens: {}",
        match report.tokens.cache_coverage {
            UsageCacheCoverage::Available | UsageCacheCoverage::Partial => {
                format_optional_number(report.tokens.cached_tokens)
            }
            UsageCacheCoverage::Unavailable => "not reported".to_string(),
        }
    ));
    out.push(format!(
        "Files changed: {}",
        format_optional_number(report.files.changed_files)
    ));
    out.push(format!("Errors: {}", report.errors.total_errors));

    if !report.coverage.missing.is_empty() {
        out.push(format!(
            "Unavailable: {}",
            report
                .coverage
                .missing
                .iter()
                .map(coverage_key_label)
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }

    if !report.slowest.is_empty() {
        out.push("Slowest spans:".to_string());
        for span in &report.slowest {
            out.push(format!(
                "- {} [{}]: {}",
                if span.redacted {
                    "redacted"
                } else {
                    &span.label
                },
                slow_span_kind_label(&span.kind),
                format_duration(span.duration_ms)
            ));
        }
    }

    out.join("\n")
}

pub fn render_usage_report_markdown(report: &SessionUsageReport) -> String {
    let mut out = String::new();
    out.push_str("# Session Usage Report\n\n");
    out.push_str(&format!(
        "- Report: `{}`\n",
        escape_markdown(&report.report_id)
    ));
    out.push_str(&format!(
        "- Session: `{}`\n",
        escape_markdown(&report.session_id)
    ));
    out.push_str(&format!(
        "- Workspace: {}\n",
        escape_markdown(
            report
                .workspace
                .path_label
                .as_deref()
                .unwrap_or("unavailable")
        )
    ));
    out.push_str(&format!(
        "- Scope: {} turns{}\n",
        report.scope.turn_count,
        if report.scope.includes_subagents {
            ", including subagents"
        } else {
            ""
        }
    ));
    out.push_str(&format!(
        "- Coverage: {}\n\n",
        coverage_level_label(&report.coverage.level)
    ));

    if report.coverage.level != UsageCoverageLevel::Complete {
        out.push_str("> Partial coverage: some metrics depend on provider or tool metadata that was not recorded for this session. Those fields are marked not reported instead of zero.\n\n");
    }

    out.push_str("## Time\n\n");
    out.push_str("| Metric | Value |\n| --- | --- |\n");
    out.push_str(&format!(
        "| Accounting | {} |\n",
        time_accounting_label(&report.time.accounting)
    ));
    out.push_str(&format!(
        "| Denominator | {} |\n",
        time_denominator_label(&report.time.denominator)
    ));
    out.push_str(&format!(
        "| Session span | {} |\n",
        format_optional_duration(report.time.wall_time_ms)
    ));
    out.push_str(&format!(
        "| Recorded turn time | {} |\n",
        format_optional_duration(report.time.active_turn_ms)
    ));
    out.push_str(&format!(
        "| Model round time | {} |\n",
        format_optional_duration(report.time.model_ms)
    ));
    out.push_str(&format!(
        "| Tool call time | {} |\n\n",
        format_optional_duration(report.time.tool_ms)
    ));

    out.push_str("## Tokens\n\n");
    out.push_str("| Metric | Value |\n| --- | --- |\n");
    out.push_str(&format!(
        "| Source | {} |\n",
        token_source_label(&report.tokens.source)
    ));
    out.push_str(&format!(
        "| Input | {} |\n",
        format_optional_number(report.tokens.input_tokens)
    ));
    out.push_str(&format!(
        "| Output | {} |\n",
        format_optional_number(report.tokens.output_tokens)
    ));
    out.push_str(&format!(
        "| Total | {} |\n",
        format_optional_number(report.tokens.total_tokens)
    ));
    out.push_str(&format!(
        "| Cached | {} |\n\n",
        match report.tokens.cache_coverage {
            UsageCacheCoverage::Available | UsageCacheCoverage::Partial => {
                format_optional_number(report.tokens.cached_tokens)
            }
            UsageCacheCoverage::Unavailable => "not reported".to_string(),
        }
    ));

    if !report.models.is_empty() {
        let include_duration = report
            .models
            .iter()
            .any(|model| model.duration_ms.is_some());
        out.push_str("## Models\n\n");
        if include_duration {
            out.push_str("| Model | Calls | Recorded time | Input | Output | Total |\n| --- | ---: | --- | ---: | ---: | ---: |\n");
        } else {
            out.push_str(
                "| Model | Calls | Input | Output | Total |\n| --- | ---: | ---: | ---: | ---: |\n",
            );
        }
        for model in &report.models {
            if include_duration {
                out.push_str(&format!(
                    "| {} | {} | {} | {} | {} | {} |\n",
                    escape_markdown(&model.model_id),
                    model.call_count,
                    format_optional_duration(model.duration_ms),
                    format_optional_number(model.input_tokens),
                    format_optional_number(model.output_tokens),
                    format_optional_number(model.total_tokens)
                ));
            } else {
                out.push_str(&format!(
                    "| {} | {} | {} | {} | {} |\n",
                    escape_markdown(&model.model_id),
                    model.call_count,
                    format_optional_number(model.input_tokens),
                    format_optional_number(model.output_tokens),
                    format_optional_number(model.total_tokens)
                ));
            }
        }
        out.push('\n');
    }

    if !report.tools.is_empty() {
        out.push_str("## Tools\n\n");
        out.push_str("| Tool | Category | Calls | Success | Errors | Recorded time | P95 |\n| --- | --- | ---: | ---: | ---: | --- | --- |\n");
        for tool in &report.tools {
            out.push_str(&format!(
                "| {} | {} | {} | {} | {} | {} | {} |\n",
                if tool.redacted {
                    "redacted".to_string()
                } else {
                    escape_markdown(&tool.tool_name)
                },
                tool_category_label(&tool.category),
                tool.call_count,
                tool.success_count,
                tool.error_count,
                format_optional_duration(tool.duration_ms),
                format_optional_duration(tool.p95_duration_ms)
            ));
        }
        out.push('\n');
    }

    out.push_str("## Files\n\n");
    out.push_str(&format!(
        "- Changed files: {}\n",
        format_optional_number(report.files.changed_files)
    ));
    out.push_str(&format!(
        "- Added lines: {}\n",
        format_optional_number(report.files.added_lines)
    ));
    out.push_str(&format!(
        "- Deleted lines: {}\n\n",
        format_optional_number(report.files.deleted_lines)
    ));
    if !report.files.files.is_empty() {
        out.push_str("| File | Operations | Added | Deleted |\n| --- | ---: | ---: | ---: |\n");
        for file in &report.files.files {
            out.push_str(&format!(
                "| {} | {} | {} | {} |\n",
                if file.redacted {
                    "redacted".to_string()
                } else {
                    escape_markdown(&file.path_label)
                },
                file.operation_count,
                format_optional_number(file.added_lines),
                format_optional_number(file.deleted_lines)
            ));
        }
        out.push('\n');
    }

    if !report.slowest.is_empty() {
        out.push_str("## Slowest Spans\n\n");
        out.push_str("| Label | Kind | Duration |\n| --- | --- | --- |\n");
        for span in &report.slowest {
            out.push_str(&format!(
                "| {} | {} | {} |\n",
                if span.redacted {
                    "redacted".to_string()
                } else {
                    escape_markdown(&span.label)
                },
                slow_span_kind_label(&span.kind),
                format_duration(span.duration_ms)
            ));
        }
        out.push('\n');
    }

    if !report.coverage.missing.is_empty() {
        out.push_str("## Coverage Gaps\n\n");
        for key in &report.coverage.missing {
            out.push_str(&format!("- `{}`\n", coverage_key_label(key)));
        }
        out.push('\n');
    }

    out.push_str("## Privacy\n\n");
    out.push_str("- Prompt content included: no\n");
    out.push_str("- Tool inputs included: no\n");
    out.push_str("- Command outputs included: no\n");
    out.push_str("- File contents included: no\n");

    out
}

fn format_optional_number(value: Option<u64>) -> String {
    value
        .map(|v| v.to_string())
        .unwrap_or_else(|| "unavailable".to_string())
}

fn format_optional_duration(value: Option<u64>) -> String {
    value
        .map(format_duration)
        .unwrap_or_else(|| "unavailable".to_string())
}

fn format_duration(ms: u64) -> String {
    if ms < 1_000 {
        return format!("{}ms", ms);
    }

    let seconds = ms / 1_000;
    if seconds < 60 {
        return format!("{}s", seconds);
    }

    let minutes = seconds / 60;
    let remaining_seconds = seconds % 60;
    if minutes < 60 {
        if remaining_seconds == 0 {
            return format!("{}m", minutes);
        }
        return format!("{}m {}s", minutes, remaining_seconds);
    }

    let hours = minutes / 60;
    let remaining_minutes = minutes % 60;
    if remaining_minutes == 0 {
        format!("{}h", hours)
    } else {
        format!("{}h {}m", hours, remaining_minutes)
    }
}

fn coverage_level_label(level: &UsageCoverageLevel) -> &'static str {
    match level {
        UsageCoverageLevel::Complete => "complete",
        UsageCoverageLevel::Partial => "partial",
        UsageCoverageLevel::Minimal => "minimal",
    }
}

fn time_accounting_label(accounting: &UsageTimeAccounting) -> &'static str {
    match accounting {
        UsageTimeAccounting::Approximate => "approximate",
        UsageTimeAccounting::Exact => "exact",
        UsageTimeAccounting::Unavailable => "unavailable",
    }
}

fn time_denominator_label(denominator: &UsageTimeDenominator) -> &'static str {
    match denominator {
        UsageTimeDenominator::SessionWallTime => "session wall time",
        UsageTimeDenominator::ActiveTurnTime => "active turn time",
        UsageTimeDenominator::Unavailable => "unavailable",
    }
}

fn token_source_label(source: &UsageTokenSource) -> &'static str {
    match source {
        UsageTokenSource::TokenUsageRecords => "token usage records",
        UsageTokenSource::Unavailable => "unavailable",
    }
}

fn slow_span_kind_label(kind: &UsageSlowSpanKind) -> &'static str {
    match kind {
        UsageSlowSpanKind::Model => "model",
        UsageSlowSpanKind::Tool => "tool",
        UsageSlowSpanKind::Turn => "turn",
    }
}

fn coverage_key_label(key: &UsageCoverageKey) -> &'static str {
    match key {
        UsageCoverageKey::ModelRoundTiming => "model_round_timing",
        UsageCoverageKey::ToolPhaseTiming => "tool_phase_timing",
        UsageCoverageKey::CachedTokens => "cached_tokens",
        UsageCoverageKey::TokenDetailBreakdown => "token_detail_breakdown",
        UsageCoverageKey::SubagentScope => "subagent_scope",
        UsageCoverageKey::RemoteSnapshotStats => "remote_snapshot_stats",
        UsageCoverageKey::FileLineStats => "file_line_stats",
        UsageCoverageKey::WorkspaceIdentity => "workspace_identity",
    }
}

fn tool_category_label(
    category: &crate::service::session_usage::classifier::UsageToolCategory,
) -> &'static str {
    match category {
        crate::service::session_usage::classifier::UsageToolCategory::Git => "git",
        crate::service::session_usage::classifier::UsageToolCategory::Shell => "shell",
        crate::service::session_usage::classifier::UsageToolCategory::File => "file",
        crate::service::session_usage::classifier::UsageToolCategory::Other => "other",
    }
}

fn escape_markdown(value: &str) -> String {
    value.replace('|', "\\|")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_usage_report_terminal_marks_cache_not_reported() {
        let report = test_report();

        let rendered = render_usage_report_terminal(&report);

        assert!(rendered.contains("Cached tokens: not reported"));
        assert!(!rendered.contains("Cached tokens: 0"));
    }

    #[test]
    fn render_usage_report_markdown_includes_partial_coverage_note() {
        let report = test_report();

        let rendered = render_usage_report_markdown(&report);

        assert!(rendered.contains("Partial coverage"));
        assert!(rendered.contains("cached_tokens"));
    }

    #[test]
    fn render_usage_report_markdown_redacts_slowest_labels() {
        let mut report = test_report();
        report.slowest.push(UsageSlowSpan {
            label: "D:/workspace/private/secret.txt".to_string(),
            kind: UsageSlowSpanKind::Tool,
            duration_ms: 1200,
            redacted: true,
            turn_id: None,
            turn_index: None,
        });

        let rendered = render_usage_report_markdown(&report);

        assert!(rendered.contains("redacted"));
        assert!(!rendered.contains("secret.txt"));
    }

    #[test]
    fn render_usage_report_stays_token_only_without_billing_language() {
        let report = test_report();

        let terminal = render_usage_report_terminal(&report);
        let markdown = render_usage_report_markdown(&report);
        let combined = format!("{}\n{}", terminal, markdown);

        assert!(combined.contains("Tokens: input"));
        assert!(!combined.contains("Estimated cost"));
        assert!(!combined.contains("USD"));
        assert!(!combined.contains("Price source"));
        assert!(!combined.contains("billing"));
        assert!(!combined.contains("plan"));
    }
}
