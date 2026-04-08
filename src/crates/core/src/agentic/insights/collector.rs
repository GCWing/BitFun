use crate::agentic::core::{Message, MessageContent, MessageRole, ToolCall, ToolResult};
use crate::agentic::insights::session_paths::collect_effective_session_storage_roots;
use crate::agentic::insights::types::*;
use crate::agentic::persistence::PersistenceManager;
use crate::infrastructure::get_path_manager_arc;
use crate::service::session::{DialogTurnData, TurnStatus};
use crate::util::errors::BitFunResult;
use chrono::{DateTime, Utc};
use log::{debug, warn};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const MAX_TRANSCRIPT_CHARS: usize = 16000;
const MAX_TEXT_PER_MESSAGE: usize = 800;
const TAIL_RESERVE_CHARS: usize = 4000;
/// Gaps longer than this between messages are treated as "user away" and excluded
/// from both active duration and response time calculations.
const ACTIVITY_GAP_THRESHOLD_SECS: u64 = 30 * 60;

pub struct InsightsCollector;

impl InsightsCollector {
    /// Stage 1: Collect session data from PersistenceManager across all workspaces
    pub async fn collect(days: u32) -> BitFunResult<(BaseStats, Vec<SessionTranscript>)> {
        let path_manager = get_path_manager_arc();
        let pm = PersistenceManager::new(path_manager)?;
        let cutoff = SystemTime::now() - Duration::from_secs(days as u64 * 86400);

        let workspace_paths = collect_effective_session_storage_roots().await;

        let mut transcripts = Vec::new();
        let mut base_stats = BaseStats::default();
        let mut seen_session_ids = HashSet::new();

        for ws_path in &workspace_paths {
            let sessions = match pm.list_sessions(ws_path).await {
                Ok(s) => s,
                Err(e) => {
                    warn!("Skipping workspace {}: {}", ws_path.display(), e);
                    continue;
                }
            };

            for summary in &sessions {
                if summary.last_activity_at < cutoff {
                    continue;
                }

                if !seen_session_ids.insert(summary.session_id.clone()) {
                    continue;
                }

                let session = match pm.load_session(ws_path, &summary.session_id).await {
                    Ok(s) => s,
                    Err(e) => {
                        warn!(
                            "Skipping session {}: load failed: {}",
                            summary.session_id, e
                        );
                        continue;
                    }
                };

                let turns = pm
                    .load_session_turns(ws_path, &summary.session_id)
                    .await
                    .unwrap_or_default();

                let messages =
                    match Self::load_session_messages_with_turns(
                        &pm, ws_path, &summary.session_id, &turns,
                    ).await {
                        Ok(m) if !m.is_empty() => m,
                        Ok(_) => {
                            debug!(
                                "Skipping session {}: no messages found",
                                summary.session_id
                            );
                            continue;
                        }
                        Err(e) => {
                            warn!(
                                "Skipping session {}: load messages failed: {}",
                                summary.session_id, e
                            );
                            continue;
                        }
                    };

                let mut transcript =
                    Self::build_transcript(&summary.session_id, &session, &messages);
                transcript.workspace_path = Some(ws_path.to_string_lossy().to_string());
                transcript.last_activity_unix_secs = summary
                    .last_activity_at
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                Self::accumulate_stats(&mut base_stats, &session, &messages);
                accumulate_code_stats_from_turns(&mut base_stats, &turns);
                transcripts.push(transcript);
            }
        }

        base_stats.total_sessions = transcripts.len() as u32;

        if let Some(earliest) = transcripts.iter().min_by_key(|t| &t.created_at) {
            base_stats.first_session_at = Some(earliest.created_at.clone());
        }
        if let Some(latest) = transcripts.iter().max_by_key(|t| &t.created_at) {
            base_stats.last_session_at = Some(latest.created_at.clone());
        }

        // Compute response time buckets from raw intervals
        if !base_stats.response_times_raw.is_empty() {
            base_stats.response_time_buckets =
                bucket_response_times(&base_stats.response_times_raw);
            let (median, avg) =
                compute_response_time_stats(&base_stats.response_times_raw);
            base_stats.median_response_time_secs = Some(median);
            base_stats.avg_response_time_secs = Some(avg);
        }

        debug!(
            "Collected {} sessions with {} total messages",
            transcripts.len(),
            base_stats.total_messages
        );

        Ok((base_stats, transcripts))
    }

    /// Load messages for a session, trying sources in priority order:
    /// 1. Latest context snapshot (most complete, includes compression)
    /// 2. Rebuild from pre-loaded turn data
    async fn load_session_messages_with_turns(
        pm: &PersistenceManager,
        workspace_path: &Path,
        session_id: &str,
        turns: &[DialogTurnData],
    ) -> BitFunResult<Vec<Message>> {
        if let Ok(Some((_turn_index, messages))) =
            pm.load_latest_turn_context_snapshot(workspace_path, session_id)
                .await
        {
            if !messages.is_empty() {
                return Ok(messages);
            }
        }

        if !turns.is_empty() {
            return Ok(rebuild_messages_from_turns(turns));
        }

        Ok(vec![])
    }

    fn build_transcript(
        session_id: &str,
        session: &crate::agentic::core::Session,
        messages: &[Message],
    ) -> SessionTranscript {
        let mut all_parts: Vec<String> = Vec::new();
        let mut tool_names: Vec<String> = Vec::new();
        let mut has_errors = false;

        for msg in messages {
            match &msg.content {
                MessageContent::Text(text) => {
                    let role_tag = match msg.role {
                        MessageRole::User => "[User]",
                        MessageRole::Assistant => "[Assistant]",
                        MessageRole::System => continue,
                        MessageRole::Tool => continue,
                    };
                    let truncated = truncate_text(text, MAX_TEXT_PER_MESSAGE);
                    all_parts.push(format!("{}: {}", role_tag, truncated));
                }
                MessageContent::Mixed {
                    text, tool_calls, ..
                } => {
                    if !text.is_empty() {
                        let truncated = truncate_text(text, MAX_TEXT_PER_MESSAGE);
                        all_parts.push(format!("[Assistant]: {}", truncated));
                    }
                    for tc in tool_calls {
                        if !tool_names.contains(&tc.tool_name) {
                            tool_names.push(tc.tool_name.clone());
                        }
                        all_parts.push(format!("[Tool: {}]", tc.tool_name));
                    }
                }
                MessageContent::ToolResult {
                    tool_name,
                    is_error,
                    ..
                } => {
                    if *is_error {
                        has_errors = true;
                        all_parts.push(format!("[Tool Error: {}]", tool_name));
                    }
                }
                MessageContent::Multimodal { text, .. } => {
                    if !text.is_empty() {
                        let truncated = truncate_text(text, MAX_TEXT_PER_MESSAGE);
                        all_parts.push(format!("[User]: {} [+images]", truncated));
                    }
                }
            }
        }

        let transcript = smart_truncate_parts(&all_parts, MAX_TRANSCRIPT_CHARS, TAIL_RESERVE_CHARS);

        let duration_minutes = Self::compute_active_duration(messages) / 60;

        let created_at = system_time_to_iso(session.created_at);

        SessionTranscript {
            session_id: session_id.to_string(),
            agent_type: session.agent_type.clone(),
            session_name: session.session_name.clone(),
            workspace_path: None,
            last_activity_unix_secs: 0,
            duration_minutes,
            message_count: messages.len() as u32,
            turn_count: session.dialog_turn_ids.len() as u32,
            created_at,
            transcript,
            tool_names,
            has_errors,
        }
    }

    fn accumulate_stats(
        base_stats: &mut BaseStats,
        session: &crate::agentic::core::Session,
        messages: &[Message],
    ) {
        base_stats.total_messages += messages.len() as u32;
        base_stats.total_turns += session.dialog_turn_ids.len() as u32;

        let active_secs = Self::compute_active_duration(messages);
        base_stats.total_duration_minutes += active_secs / 60;

        *base_stats
            .agent_types
            .entry(session.agent_type.clone())
            .or_insert(0) += 1;

        let mut last_assistant_time: Option<SystemTime> = None;
        for msg in messages {
            if msg.role == MessageRole::User {
                if let Ok(dur) = msg.timestamp.duration_since(UNIX_EPOCH) {
                    let dt = DateTime::<Utc>::from(UNIX_EPOCH + dur);
                    let hour = dt.format("%H").to_string().parse::<u32>().unwrap_or(0);
                    *base_stats.hour_counts.entry(hour).or_insert(0) += 1;
                }
            }

            match &msg.content {
                MessageContent::Mixed { tool_calls, .. } => {
                    for tc in tool_calls {
                        *base_stats
                            .tool_usage
                            .entry(tc.tool_name.clone())
                            .or_insert(0) += 1;
                    }
                }
                MessageContent::ToolResult {
                    tool_name,
                    is_error,
                    ..
                } => {
                    if *is_error {
                        *base_stats
                            .tool_errors
                            .entry(tool_name.clone())
                            .or_insert(0) += 1;
                    }
                }
                _ => {}
            }

            match msg.role {
                MessageRole::Assistant => {
                    last_assistant_time = Some(msg.timestamp);
                }
                MessageRole::User => {
                    if let Some(prev) = last_assistant_time {
                        if let Ok(duration) = msg.timestamp.duration_since(prev) {
                            let secs = duration.as_secs();
                            if (2..=ACTIVITY_GAP_THRESHOLD_SECS).contains(&secs) {
                                base_stats.response_times_raw.push(secs as f64);
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }

    /// Compute active usage duration by summing adjacent message gaps,
    /// capping each gap at `ACTIVITY_GAP_THRESHOLD_SECS`.
    fn compute_active_duration(messages: &[Message]) -> u64 {
        if messages.len() < 2 {
            return 0;
        }
        let mut total_secs: u64 = 0;
        for pair in messages.windows(2) {
            if let Ok(gap) = pair[1].timestamp.duration_since(pair[0].timestamp) {
                let gap_secs = gap.as_secs();
                if gap_secs <= ACTIVITY_GAP_THRESHOLD_SECS {
                    total_secs += gap_secs;
                }
            }
        }
        total_secs
    }

    /// Stage 3: Aggregate facets into InsightsAggregate
    pub fn aggregate(base_stats: &BaseStats, facets: &[SessionFacet]) -> InsightsAggregate {
        let mut goals: HashMap<String, u32> = HashMap::new();
        let mut outcomes: HashMap<String, u32> = HashMap::new();
        let mut satisfaction: HashMap<String, u32> = HashMap::new();
        let mut friction: HashMap<String, u32> = HashMap::new();
        let mut success: HashMap<String, u32> = HashMap::new();
        let mut session_types: HashMap<String, u32> = HashMap::new();
        let mut session_summaries = Vec::new();
        let mut friction_details = Vec::new();
        let mut user_instructions = Vec::new();

        for facet in facets {
            for (k, v) in &facet.goal_categories {
                *goals.entry(k.clone()).or_insert(0) += v;
            }
            *outcomes.entry(facet.outcome.clone()).or_insert(0) += 1;
            for (k, v) in &facet.user_satisfaction_counts {
                *satisfaction.entry(k.clone()).or_insert(0) += v;
            }
            for (k, v) in &facet.friction_counts {
                *friction.entry(k.clone()).or_insert(0) += v;
            }
            if !facet.primary_success.is_empty() && facet.primary_success != "none" {
                *success
                    .entry(facet.primary_success.clone())
                    .or_insert(0) += 1;
            }
            *session_types
                .entry(facet.session_type.clone())
                .or_insert(0) += 1;

            if !facet.brief_summary.is_empty() {
                session_summaries.push(facet.brief_summary.clone());
            }
            if !facet.friction_detail.is_empty() {
                friction_details.push(facet.friction_detail.clone());
            }
            for instr in &facet.user_instructions {
                if !user_instructions.contains(instr) {
                    user_instructions.push(instr.clone());
                }
            }
        }

        let mut top_tools: Vec<(String, u32)> = base_stats.tool_usage.iter().map(|(k, v)| (k.clone(), *v)).collect();
        top_tools.sort_by(|a, b| b.1.cmp(&a.1));
        top_tools.truncate(15);

        let mut top_goals: Vec<(String, u32)> = goals.iter().map(|(k, v)| (k.clone(), *v)).collect();
        top_goals.sort_by(|a, b| b.1.cmp(&a.1));
        top_goals.truncate(10);

        let hours = base_stats.total_duration_minutes as f32 / 60.0;
        let date_range = DateRange {
            start: base_stats
                .first_session_at
                .clone()
                .unwrap_or_default(),
            end: base_stats
                .last_session_at
                .clone()
                .unwrap_or_default(),
        };

        let days_covered = compute_days_covered(&date_range);
        let msgs_per_day = if days_covered > 0 {
            base_stats.total_messages as f32 / days_covered as f32
        } else {
            base_stats.total_messages as f32
        };

        let languages = base_stats.languages_by_files.clone();

        InsightsAggregate {
            sessions: base_stats.total_sessions,
            analyzed: facets.len() as u32,
            date_range,
            messages: base_stats.total_messages,
            hours,
            top_tools,
            top_goals,
            outcomes,
            satisfaction,
            friction,
            success,
            languages,
            session_summaries,
            friction_details,
            user_instructions,
            session_types,
            tool_errors: base_stats.tool_errors.clone(),
            hour_counts: base_stats.hour_counts.clone(),
            agent_types: base_stats.agent_types.clone(),
            msgs_per_day,
            response_time_buckets: base_stats.response_time_buckets.clone(),
            median_response_time_secs: base_stats.median_response_time_secs,
            avg_response_time_secs: base_stats.avg_response_time_secs,
            total_lines_added: base_stats.total_lines_added,
            total_lines_removed: base_stats.total_lines_removed,
            total_files_modified: base_stats.total_files_modified,
        }
    }
}

/// Rebuild `Vec<Message>` from turn data, including tool call and tool result information
/// needed by `build_transcript` and `accumulate_stats`.
/// Preserves timestamps from turn data and marks cancelled turns with `[Cancelled]`.
fn rebuild_messages_from_turns(turns: &[DialogTurnData]) -> Vec<Message> {
    let mut messages = Vec::new();

    for turn in turns {
        if !turn.kind.is_model_visible() {
            continue;
        }

        let user_ts = UNIX_EPOCH + Duration::from_millis(turn.start_time);
        let mut user_msg = Message::user(turn.user_message.content.clone());
        user_msg.timestamp = user_ts;
        messages.push(user_msg);

        for (round_idx, round) in turn.model_rounds.iter().enumerate() {
            let assistant_text = round
                .text_items
                .iter()
                .map(|item| item.content.clone())
                .filter(|c| !c.trim().is_empty())
                .collect::<Vec<_>>()
                .join("\n\n");

            let tool_calls: Vec<ToolCall> = round
                .tool_items
                .iter()
                .map(|ti| ToolCall {
                    tool_id: ti.tool_call.id.clone(),
                    tool_name: ti.tool_name.clone(),
                    arguments: ti.tool_call.input.clone(),
                    is_error: false,
                })
                .collect();

            let round_ts = if let Some(end_time) = turn.end_time {
                let start = turn.start_time;
                let total_rounds = turn.model_rounds.len().max(1) as u64;
                let step = (end_time.saturating_sub(start)) / (total_rounds + 1);
                UNIX_EPOCH + Duration::from_millis(start + step * (round_idx as u64 + 1))
            } else {
                UNIX_EPOCH + Duration::from_millis(turn.start_time + (round_idx as u64 + 1) * 1000)
            };

            if !tool_calls.is_empty() {
                let mut msg =
                    Message::assistant_with_tools(assistant_text.clone(), tool_calls);
                msg.timestamp = round_ts;
                messages.push(msg);
            } else if !assistant_text.trim().is_empty() {
                let mut msg = Message::assistant(assistant_text);
                msg.timestamp = round_ts;
                messages.push(msg);
            }

            for ti in &round.tool_items {
                if let Some(result_data) = &ti.tool_result {
                    let mut msg = Message::tool_result(ToolResult {
                        tool_id: ti.tool_call.id.clone(),
                        tool_name: ti.tool_name.clone(),
                        result: result_data.result.clone(),
                        result_for_assistant: None,
                        is_error: !result_data.success,
                        duration_ms: result_data.duration_ms,
                        image_attachments: None,
                    });
                    msg.timestamp = round_ts;
                    messages.push(msg);
                }
            }
        }

        if turn.status == TurnStatus::Cancelled {
            let cancel_ts = turn
                .end_time
                .map(|t| UNIX_EPOCH + Duration::from_millis(t))
                .unwrap_or(user_ts);
            let mut cancel_msg = Message::assistant("[Cancelled by user]".to_string());
            cancel_msg.timestamp = cancel_ts;
            messages.push(cancel_msg);
        }
    }

    messages
}

/// Keep head + tail of transcript parts, inserting an omission marker in the middle
/// when total length exceeds `max_chars`. This preserves the beginning (context/goals)
/// and end (final outcome) of a session.
fn smart_truncate_parts(parts: &[String], max_chars: usize, tail_reserve: usize) -> String {
    let total: usize = parts.iter().map(|p| p.len() + 1).sum();
    if total <= max_chars {
        return parts.join("\n");
    }

    let head_budget = max_chars.saturating_sub(tail_reserve);
    let mut head_parts = Vec::new();
    let mut head_used = 0;
    let mut head_end_idx = 0;

    for (i, part) in parts.iter().enumerate() {
        let cost = part.len() + 1;
        if head_used + cost > head_budget {
            break;
        }
        head_parts.push(part.as_str());
        head_used += cost;
        head_end_idx = i + 1;
    }

    let mut tail_parts = Vec::new();
    let mut tail_used = 0;
    let mut tail_start_idx = parts.len();

    for (i, part) in parts.iter().enumerate().rev() {
        if i < head_end_idx {
            break;
        }
        let cost = part.len() + 1;
        if tail_used + cost > tail_reserve {
            break;
        }
        tail_parts.push(part.as_str());
        tail_used += cost;
        tail_start_idx = i;
    }
    tail_parts.reverse();

    let omitted = tail_start_idx.saturating_sub(head_end_idx);

    let mut result = head_parts.join("\n");
    if omitted > 0 {
        result.push_str(&format!("\n\n[... {} messages omitted ...]\n\n", omitted));
    }
    result.push_str(&tail_parts.join("\n"));
    result
}

fn truncate_text(text: &str, max_len: usize) -> String {
    let trimmed = text.trim();
    if trimmed.len() <= max_len {
        trimmed.to_string()
    } else {
        let mut end = max_len.min(trimmed.len());
        while end > 0 && !trimmed.is_char_boundary(end) {
            end -= 1;
        }
        format!("{}...", &trimmed[..end])
    }
}

fn system_time_to_iso(t: SystemTime) -> String {
    match t.duration_since(UNIX_EPOCH) {
        Ok(dur) => {
            let dt = DateTime::<Utc>::from(UNIX_EPOCH + dur);
            dt.to_rfc3339()
        }
        Err(_) => "unknown".to_string(),
    }
}

fn bucket_response_times(raw: &[f64]) -> HashMap<String, u32> {
    let buckets: &[(&str, f64, f64)] = &[
        ("2-10s", 2.0, 10.0),
        ("10-30s", 10.0, 30.0),
        ("30s-1m", 30.0, 60.0),
        ("1-2m", 60.0, 120.0),
        ("2-5m", 120.0, 300.0),
        ("5-15m", 300.0, 900.0),
        (">15m", 900.0, f64::MAX),
    ];

    let mut result: HashMap<String, u32> = HashMap::new();
    for &val in raw {
        for &(label, lo, hi) in buckets {
            if val >= lo && val < hi {
                *result.entry(label.to_string()).or_insert(0) += 1;
                break;
            }
        }
    }
    result
}

fn compute_response_time_stats(raw: &[f64]) -> (f64, f64) {
    if raw.is_empty() {
        return (0.0, 0.0);
    }

    let avg = raw.iter().sum::<f64>() / raw.len() as f64;

    let mut sorted = raw.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let median = if sorted.len().is_multiple_of(2) {
        let mid = sorted.len() / 2;
        (sorted[mid - 1] + sorted[mid]) / 2.0
    } else {
        sorted[sorted.len() / 2]
    };

    (median, avg)
}

fn compute_days_covered(range: &DateRange) -> u32 {
    let parse = |s: &str| -> Option<DateTime<Utc>> {
        DateTime::parse_from_rfc3339(s).ok().map(|d| d.with_timezone(&Utc))
    };

    match (parse(&range.start), parse(&range.end)) {
        (Some(start), Some(end)) => {
            let diff = end.signed_duration_since(start);
            let days = diff.num_days().unsigned_abs() as u32;
            days.max(1)
        }
        _ => 1,
    }
}

/// Extract code change statistics from persistent turn data.
///
/// For Edit tool results: uses `old_end_line - start_line + 1` as lines removed
/// and `new_end_line - start_line + 1` as lines added, falling back to counting
/// newlines in `old_string`/`new_string`.
///
/// For Write tool: counts newlines in the written content as lines added.
///
/// Per session, each distinct file path touched by Edit/Write contributes once to `languages_by_files`
/// according to [`language_name_for_path`].
fn accumulate_code_stats_from_turns(base_stats: &mut BaseStats, turns: &[DialogTurnData]) {
    let mut modified_files: HashSet<String> = HashSet::new();

    for turn in turns {
        for round in &turn.model_rounds {
            for ti in &round.tool_items {
                let Some(ref result_data) = ti.tool_result else {
                    continue;
                };
                if !result_data.success {
                    continue;
                }

                match ti.tool_name.as_str() {
                    "Edit" => {
                        let result = &result_data.result;

                        if let Some(fp) = result.get("file_path").and_then(|v| v.as_str()) {
                            modified_files.insert(fp.to_string());
                        }

                        let (lines_removed, lines_added) =
                            if let (Some(start), Some(old_end), Some(new_end)) = (
                                result.get("start_line").and_then(|v| v.as_u64()),
                                result.get("old_end_line").and_then(|v| v.as_u64()),
                                result.get("new_end_line").and_then(|v| v.as_u64()),
                            ) {
                                let removed = old_end.saturating_sub(start) + 1;
                                let added = new_end.saturating_sub(start) + 1;
                                (removed as usize, added as usize)
                            } else {
                                let old_lines = result
                                    .get("old_string")
                                    .and_then(|v| v.as_str())
                                    .map(|s| s.lines().count().max(1))
                                    .unwrap_or(0);
                                let new_lines = result
                                    .get("new_string")
                                    .and_then(|v| v.as_str())
                                    .map(|s| s.lines().count().max(1))
                                    .unwrap_or(0);
                                (old_lines, new_lines)
                            };

                        base_stats.total_lines_removed += lines_removed;
                        base_stats.total_lines_added += lines_added;
                    }
                    "Write" => {
                        let result = &result_data.result;

                        if let Some(fp) = result.get("file_path").and_then(|v| v.as_str()) {
                            modified_files.insert(fp.to_string());
                        }

                        let input = &ti.tool_call.input;
                        if let Some(content) = input.get("content").and_then(|v| v.as_str()) {
                            base_stats.total_lines_added += content.lines().count().max(1);
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    for path in &modified_files {
        if let Some(lang) = language_name_for_path(path) {
            *base_stats
                .languages_by_files
                .entry(lang.to_string())
                .or_insert(0) += 1;
        }
    }

    base_stats.total_files_modified += modified_files.len();
}

/// Infer a language label from a file path (extension or well-known filename).
fn language_name_for_path(path: &str) -> Option<&'static str> {
    let p = Path::new(path);
    if let Some(name) = p.file_name().and_then(|n| n.to_str()) {
        match name.to_ascii_lowercase().as_str() {
            "dockerfile" | "containerfile" => return Some("Dockerfile"),
            "makefile" | "gnumakefile" => return Some("Makefile"),
            "cargo.toml" | "cargo.lock" => return Some("Rust"),
            _ => {}
        }
    }
    let ext = p.extension()?.to_str()?.to_ascii_lowercase();
    Some(match ext.as_str() {
        "ts" | "tsx" => "TypeScript",
        "js" | "jsx" | "mjs" | "cjs" => "JavaScript",
        "py" | "pyi" | "pyw" => "Python",
        "rs" => "Rust",
        "go" => "Go",
        "java" => "Java",
        "kt" | "kts" => "Kotlin",
        "swift" => "Swift",
        "cs" => "C#",
        "cpp" | "cc" | "cxx" | "hpp" => "C/C++",
        "c" | "h" => "C/C++",
        "rb" => "Ruby",
        "php" => "PHP",
        "vue" => "Vue",
        "svelte" => "Svelte",
        "md" | "mdx" => "Markdown",
        "json" | "jsonc" => "JSON",
        "yaml" | "yml" => "YAML",
        "toml" => "TOML",
        "xml" => "XML",
        "html" | "htm" => "HTML",
        "css" | "scss" | "sass" | "less" => "CSS",
        "sh" | "bash" | "zsh" | "fish" => "Shell",
        "ps1" => "PowerShell",
        "sql" => "SQL",
        "gradle" => "Gradle",
        "properties" => "Properties",
        _ => return None,
    })
}
