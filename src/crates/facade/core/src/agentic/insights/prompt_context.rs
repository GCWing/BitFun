//! Slim aggregate JSON and bounded text blocks for LLM prompts (no duplicate long lists).

use crate::agentic::insights::types::InsightsAggregate;
use serde::Serialize;
use std::collections::HashMap;

/// Max lines aligned with Claude Code insights reference.
pub const MAX_PROMPT_SESSION_SUMMARIES: usize = 50;
pub const MAX_PROMPT_FRICTION_DETAILS: usize = 20;
pub const MAX_PROMPT_USER_INSTRUCTIONS: usize = 15;

#[derive(Serialize)]
pub struct AggregatePromptStats<'a> {
    pub sessions: u32,
    pub analyzed: u32,
    pub date_range: &'a crate::agentic::insights::types::DateRange,
    pub messages: u32,
    pub hours: f32,
    pub top_tools: &'a [(String, u32)],
    pub top_goals: &'a [(String, u32)],
    pub outcomes: &'a HashMap<String, u32>,
    pub satisfaction: &'a HashMap<String, u32>,
    pub friction: &'a HashMap<String, u32>,
    pub success: &'a HashMap<String, u32>,
    pub languages: &'a HashMap<String, u32>,
    pub session_types: &'a HashMap<String, u32>,
    pub tool_errors: &'a HashMap<String, u32>,
    pub hour_counts: &'a HashMap<u32, u32>,
    pub agent_types: &'a HashMap<String, u32>,
    pub msgs_per_day: f32,
    pub response_time_buckets: &'a HashMap<String, u32>,
    pub median_response_time_secs: Option<f64>,
    pub avg_response_time_secs: Option<f64>,
    pub total_lines_added: usize,
    pub total_lines_removed: usize,
    pub total_files_modified: usize,
}

impl<'a> From<&'a InsightsAggregate> for AggregatePromptStats<'a> {
    fn from(a: &'a InsightsAggregate) -> Self {
        Self {
            sessions: a.sessions,
            analyzed: a.analyzed,
            date_range: &a.date_range,
            messages: a.messages,
            hours: a.hours,
            top_tools: &a.top_tools,
            top_goals: &a.top_goals,
            outcomes: &a.outcomes,
            satisfaction: &a.satisfaction,
            friction: &a.friction,
            success: &a.success,
            languages: &a.languages,
            session_types: &a.session_types,
            tool_errors: &a.tool_errors,
            hour_counts: &a.hour_counts,
            agent_types: &a.agent_types,
            msgs_per_day: a.msgs_per_day,
            response_time_buckets: &a.response_time_buckets,
            median_response_time_secs: a.median_response_time_secs,
            avg_response_time_secs: a.avg_response_time_secs,
            total_lines_added: a.total_lines_added,
            total_lines_removed: a.total_lines_removed,
            total_files_modified: a.total_files_modified,
        }
    }
}

pub fn aggregate_stats_json_for_prompt(aggregate: &InsightsAggregate) -> String {
    let stats = AggregatePromptStats::from(aggregate);
    serde_json::to_string_pretty(&stats).unwrap_or_else(|_| "{}".to_string())
}

/// Bullet list for templates that embed `{summaries}` after a label.
pub fn summaries_block(aggregate: &InsightsAggregate) -> String {
    let lines: Vec<&str> = aggregate
        .session_summaries
        .iter()
        .take(MAX_PROMPT_SESSION_SUMMARIES)
        .map(|s| s.as_str())
        .collect();
    if lines.is_empty() {
        return String::new();
    }
    format!("- {}", lines.join("\n- "))
}

pub fn friction_block(aggregate: &InsightsAggregate) -> String {
    let lines: Vec<&str> = aggregate
        .friction_details
        .iter()
        .filter(|s| !s.trim().is_empty())
        .take(MAX_PROMPT_FRICTION_DETAILS)
        .map(|s| s.as_str())
        .collect();
    if lines.is_empty() {
        return String::new();
    }
    format!("- {}", lines.join("\n- "))
}

pub fn user_instructions_block(aggregate: &InsightsAggregate) -> String {
    let mut seen = std::collections::HashSet::<&str>::new();
    let mut lines: Vec<&str> = Vec::new();
    for s in &aggregate.user_instructions {
        if s.trim().is_empty() {
            continue;
        }
        if seen.insert(s.as_str()) && lines.len() < MAX_PROMPT_USER_INSTRUCTIONS {
            lines.push(s.as_str());
        }
    }
    if lines.is_empty() {
        "None captured".to_string()
    } else {
        format!("- {}", lines.join("\n- "))
    }
}
