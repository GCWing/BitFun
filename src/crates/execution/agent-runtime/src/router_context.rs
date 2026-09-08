//! Router-only, incremental context. Never contains or mutates agent compression state.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{HashSet, VecDeque};

pub trait RouterTokenCounter: Send + Sync {
    fn count(&self, text: &str) -> usize;
    fn name(&self) -> &'static str;
}

/// Conservative fallback for the router's byte-level BPE when no tokenizer is installed.
pub struct Utf8ByteBudget;

impl RouterTokenCounter for Utf8ByteBudget {
    fn count(&self, text: &str) -> usize {
        text.len()
    }

    fn name(&self) -> &'static str {
        "utf8_byte_upper_bound"
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RouterEntryKind {
    Round,
    UserUpdate,
    Feedback,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouterEntry {
    pub sequence: u64,
    pub source_id: String,
    pub kind: RouterEntryKind,
    #[serde(default)]
    pub is_error: bool,
    pub content: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct RouterContextState {
    pub version: u32,
    pub session_id: String,
    pub dialog_turn_id: String,
    pub task: String,
    pub summary: String,
    pub summarized_through: u64,
    pub next_sequence: u64,
    pub observed_rounds: u64,
    pub omitted_through: u64,
    pub entries: VecDeque<RouterEntry>,
    pub seen_message_ids: HashSet<String>,
    pub latest_user_updates: VecDeque<String>,
}

impl Default for RouterContextState {
    fn default() -> Self {
        Self {
            version: 1,
            session_id: String::new(),
            dialog_turn_id: String::new(),
            task: String::new(),
            summary: String::new(),
            summarized_through: 0,
            next_sequence: 1,
            observed_rounds: 0,
            omitted_through: 0,
            entries: VecDeque::new(),
            seen_message_ids: HashSet::new(),
            latest_user_updates: VecDeque::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct RouterSummaryWork {
    pub base_through: u64,
    pub through: u64,
    pub prompt: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct PreparedRouterContext {
    /// Trace writers emit the actual input separately from these budget/cursor facts.
    #[serde(skip_serializing)]
    pub user_prompt: String,
    pub preparation_ms: u64,
    pub input_tokens: usize,
    pub budget_tokens: usize,
    pub counter: &'static str,
    pub summarized_through: u64,
    pub observed_through: u64,
    pub omitted_through: u64,
}

impl RouterContextState {
    pub fn claim_message(&mut self, id: &str) -> bool {
        self.seen_message_ids.insert(id.to_string())
    }

    pub fn append(
        &mut self,
        source_id: String,
        kind: RouterEntryKind,
        content: Value,
        counter: &dyn RouterTokenCounter,
    ) {
        if kind == RouterEntryKind::Round {
            self.observed_rounds += 1;
        }
        if kind == RouterEntryKind::UserUpdate {
            if content.as_str() == Some(self.task.as_str()) {
                // Initial history can include previous tasks. The current task resets
                // their steering priority without erasing historical observations.
                self.latest_user_updates.clear();
            }
            if let Some(text) = content.as_str().filter(|text| *text != self.task) {
                if self.latest_user_updates.back().map(String::as_str) != Some(text) {
                    self.latest_user_updates
                        .push_back(truncate(text, 1_024, counter));
                    while self.latest_user_updates.len() > 4 {
                        self.latest_user_updates.pop_front();
                    }
                }
            }
        }
        let flagged = |value: &Value| value.get("is_error").and_then(Value::as_bool) == Some(true);
        let is_error = flagged(&content)
            || content
                .get("tool_results")
                .and_then(Value::as_array)
                .is_some_and(|values| values.iter().any(flagged))
            || content
                .pointer("/assistant/tool_calls")
                .and_then(Value::as_array)
                .is_some_and(|values| values.iter().any(flagged));
        self.entries.push_back(RouterEntry {
            sequence: self.next_sequence,
            source_id,
            kind,
            is_error,
            content: compact_json(&content, 4_096, counter),
        });
        self.next_sequence += 1;
        // Backpressure for failed/slow summary providers, never an agent-loop limit.
        while self.entries.len() > 128 {
            if let Some(entry) = self.entries.pop_front() {
                self.omitted_through = entry.sequence;
            }
        }
    }

    fn recent_start(&self, recent_rounds: usize) -> usize {
        self.entries
            .iter()
            .enumerate()
            .rev()
            .filter(|(_, entry)| entry.kind == RouterEntryKind::Round)
            .nth(recent_rounds.max(1) - 1)
            .map(|(index, _)| index)
            .unwrap_or(0)
    }

    pub fn summary_work(
        &self,
        recent_rounds: usize,
        min_rounds: usize,
        counter: &dyn RouterTokenCounter,
    ) -> Option<RouterSummaryWork> {
        let prefix: Vec<_> = self
            .entries
            .iter()
            .take(self.recent_start(recent_rounds))
            .collect();
        if prefix
            .iter()
            .filter(|entry| entry.kind == RouterEntryKind::Round)
            .count()
            < min_rounds
        {
            return None;
        }
        let through = prefix.last()?.sequence;
        let entries: Vec<_> = prefix.iter().map(|entry| json!(entry)).collect();
        Some(RouterSummaryWork {
            base_through: self.summarized_through,
            through,
            prompt: format!(
                "Task:\n{}\n\nPrevious router summary:\n{}\n\nObserved entries through sequence {through}:\n{}\n\nPreviously omitted through sequence: {}",
                truncate(&self.task, 800, counter),
                truncate(&self.summary, 900, counter),
                compact_json(&json!(entries), 6_000, counter),
                self.omitted_through,
            ),
        })
    }

    pub fn apply_summary(
        &mut self,
        work: &RouterSummaryWork,
        summary: &str,
        counter: &dyn RouterTokenCounter,
    ) -> bool {
        if work.base_through != self.summarized_through
            || work.through <= self.summarized_through
            || work.through >= self.next_sequence
            || summary.trim().is_empty()
        {
            return false;
        }
        self.summary = truncate(summary.trim(), 900, counter);
        self.summarized_through = work.through;
        self.entries.retain(|entry| entry.sequence > work.through);
        true
    }

    pub fn prepare(
        &self,
        recent_rounds: usize,
        max_tokens: usize,
        counter: &dyn RouterTokenCounter,
    ) -> PreparedRouterContext {
        let max_tokens = max_tokens.max(512);
        let task_budget = (max_tokens / 5).min(800);
        // Latest steering first so shrinking never preferentially drops the active request.
        let updates = self
            .latest_user_updates
            .iter()
            .rev()
            .cloned()
            .collect::<Vec<_>>()
            .join("\nPrevious user update:\n");
        let task = if updates.is_empty() {
            truncate(&self.task, task_budget, counter)
        } else {
            format!(
                "{}\nUser update:\n{}",
                truncate(&self.task, task_budget / 2, counter),
                truncate(&updates, task_budget / 2, counter)
            )
        };
        let start = self.recent_start(recent_rounds);
        let pending: Vec<_> = self
            .entries
            .iter()
            .take(start)
            .map(|entry| json!(entry))
            .collect();
        let history_budget = max_tokens / 4;
        let history = format!(
            "{}\nUnsummarized earlier observations: {}\nOmitted through sequence: {}",
            truncate(&self.summary, history_budget / 2, counter),
            compact_json(&json!(pending), history_budget / 2, counter),
            self.omitted_through,
        );
        let recent: Vec<_> = self.entries.iter().skip(start).collect();
        let newest_round = recent
            .iter()
            .rposition(|entry| entry.kind == RouterEntryKind::Round)
            .unwrap_or(recent.len().saturating_sub(1));
        let mut recent_budget = max_tokens
            .saturating_sub(counter.count(&task) + counter.count(&history) + 100)
            .max(64);
        let user_prompt = loop {
            // The newest completed round gets half the detail; feedback retains its own share.
            let mut rendered = Vec::new();
            for (index, entry) in recent.iter().enumerate() {
                let budget = if recent.len() == 1 {
                    recent_budget
                } else if index == newest_round {
                    recent_budget / 2
                } else {
                    recent_budget / (2 * (recent.len() - 1))
                };
                // Sequence/ids are bookkeeping, not useful prediction input. Keep the original
                // trajectory shape for rounds and explicit wrappers only for feedback.
                let value = if entry.kind == RouterEntryKind::Round {
                    let mut value = entry.content.clone();
                    if let Some(object) = value.as_object_mut() {
                        object.insert("is_error".into(), json!(entry.is_error));
                    }
                    value
                } else {
                    json!({"kind": entry.kind, "content": entry.content})
                };
                rendered.push(compact_json(&value, budget.max(16), counter));
            }
            let trajectory = compact_json(&json!(rendered), recent_budget, counter);
            let prompt = format!("## Task\n{task}\n\n## Earlier history summary\n{history}\n\n## Recent trajectory\n{trajectory}");
            if counter.count(&prompt) <= max_tokens {
                break prompt;
            }
            if recent_budget <= 16 {
                // Preserve the three-section protocol even for exceptionally small budgets.
                break format!("## Task\n{}\n\n## Earlier history summary\n(context budget exhausted)\n\n## Recent trajectory\n[]", truncate(&task, max_tokens.saturating_sub(160), counter));
            }
            recent_budget /= 2;
        };
        PreparedRouterContext {
            preparation_ms: 0,
            input_tokens: counter.count(&user_prompt),
            budget_tokens: max_tokens,
            counter: counter.name(),
            user_prompt,
            summarized_through: self.summarized_through,
            observed_through: self.next_sequence.saturating_sub(1),
            omitted_through: self.omitted_through,
        }
    }
}

/// UTF-8 safe, explicitly marked head/tail truncation, measured with the supplied tokenizer.
pub fn truncate(text: &str, budget: usize, counter: &dyn RouterTokenCounter) -> String {
    // Avoid tokenizing megabytes of logs/code on a routing boundary. This separate
    // source-size guard is an explicitly marked lossy projection, not token counting.
    if text.len() > 65_536 {
        let mut head = 16_384;
        while !text.is_char_boundary(head) {
            head -= 1;
        }
        let mut tail = text.len() - 16_384;
        while !text.is_char_boundary(tail) {
            tail += 1;
        }
        return truncate(
            &format!(
                "{}\n[source middle omitted]\n{}",
                &text[..head],
                &text[tail..]
            ),
            budget,
            counter,
        );
    }
    if counter.count(text) <= budget {
        return text.to_string();
    }
    let offsets: Vec<_> = text
        .char_indices()
        .map(|(offset, _)| offset)
        .chain(std::iter::once(text.len()))
        .collect();
    let mut low = 0;
    let mut high = offsets.len().saturating_sub(1);
    let mut best = String::new();
    while low <= high {
        let keep = low + (high - low) / 2;
        let head = keep / 2;
        let tail = keep - head;
        let candidate = format!(
            "{}\n[omitted]\n{}",
            &text[..offsets[head]],
            &text[offsets[offsets.len() - 1 - tail]..]
        );
        if counter.count(&candidate) <= budget {
            best = candidate;
            low = keep + 1;
        } else if keep == 0 {
            break;
        } else {
            high = keep - 1;
        }
    }
    best
}

pub fn compact_json(value: &Value, budget: usize, counter: &dyn RouterTokenCounter) -> Value {
    fn shrink(
        value: &Value,
        field_budget: usize,
        items: usize,
        counter: &dyn RouterTokenCounter,
    ) -> Value {
        match value {
            Value::String(text) => Value::String(truncate(text, field_budget, counter)),
            Value::Array(values) => {
                let mut retained: Vec<usize> = (0..values.len()).collect();
                // Prefer errors and recent results while preserving the retained chronology.
                retained.sort_by_key(|index| {
                    (
                        values[*index].get("is_error").and_then(Value::as_bool) != Some(true),
                        std::cmp::Reverse(*index),
                    )
                });
                retained.truncate(items);
                retained.sort_unstable();
                let mut output: Vec<_> = retained
                    .iter()
                    .map(|index| shrink(&values[*index], field_budget, items, counter))
                    .collect();
                if values.len() > output.len() {
                    output.insert(0, json!({"omitted_items": values.len() - output.len()}));
                }
                Value::Array(output)
            }
            Value::Object(object) => {
                let mut output: serde_json::Map<String, Value> = object
                    .iter()
                    .take(items.max(8))
                    .map(|(key, value)| (key.clone(), shrink(value, field_budget, items, counter)))
                    .collect();
                if object.len() > output.len() {
                    output.insert("omitted_fields".into(), json!(object.len() - output.len()));
                }
                Value::Object(output)
            }
            _ => value.clone(),
        }
    }
    let rendered = value.to_string();
    if rendered.len() <= 65_536 && counter.count(&rendered) <= budget {
        return value.clone();
    }
    let mut fields = 1_024;
    let mut items = 32;
    loop {
        let compact = shrink(value, fields, items, counter);
        if counter.count(&compact.to_string()) <= budget {
            return compact;
        }
        if fields > 32 {
            fields /= 2;
        } else if items > 1 {
            items /= 2;
        } else {
            let preview = truncate(&rendered, budget.saturating_sub(48), counter);
            let result = json!({"truncated": true, "preview": preview});
            if counter.count(&result.to_string()) <= budget {
                return result;
            }
            return Value::Null;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn append_round(state: &mut RouterContextState, index: usize) {
        state.append(format!("m{index}"), RouterEntryKind::Round, json!({"round_id": index, "assistant": {"text": format!("step-{index}"), "tool_calls": []}, "tool_results": []}), &Utf8ByteBudget);
    }

    #[test]
    fn delayed_summary_covers_only_its_snapshot_and_keeps_new_evidence() {
        let mut state = RouterContextState::default();
        for index in 0..8 {
            append_round(&mut state, index);
        }
        let work = state.summary_work(3, 4, &Utf8ByteBudget).unwrap();
        append_round(&mut state, 8);
        assert!(state.apply_summary(&work, "Earlier factual summary", &Utf8ByteBudget));
        assert_eq!(state.entries.front().unwrap().sequence, work.through + 1);
        let prompt = state.prepare(3, 4096, &Utf8ByteBudget).user_prompt;
        assert!(prompt.contains("Earlier factual summary"));
        assert!(prompt.contains("step-8"));
        assert!(!state.apply_summary(&work, "stale", &Utf8ByteBudget));
    }

    #[test]
    fn huge_multilingual_context_is_bounded_and_latest_update_survives() {
        let mut state = RouterContextState {
            task: "任务 code 🦀 ".repeat(10_000),
            ..Default::default()
        };
        for index in 0..8 {
            append_round(&mut state, index);
        }
        state.append(
            "steer".into(),
            RouterEntryKind::UserUpdate,
            json!("Do not edit; inspect the failing test first"),
            &Utf8ByteBudget,
        );
        for budget in [512, 2048, 4096] {
            let prepared = state.prepare(3, budget, &Utf8ByteBudget);
            assert!(
                prepared.input_tokens <= budget,
                "{} > {budget}",
                prepared.input_tokens
            );
            assert!(prepared.user_prompt.contains("## Recent trajectory"));
            let trajectory = prepared
                .user_prompt
                .split("## Recent trajectory\n")
                .nth(1)
                .unwrap();
            serde_json::from_str::<Value>(trajectory).unwrap();
        }
        assert!(state
            .prepare(3, 4096, &Utf8ByteBudget)
            .user_prompt
            .contains("Do not edit"));
    }

    #[test]
    fn missing_summary_keeps_earlier_observations_and_serialization_defaults() {
        let mut state: RouterContextState = serde_json::from_str(r#"{"task":"fix bug"}"#).unwrap();
        for index in 0..6 {
            append_round(&mut state, index);
        }
        let prompt = state.prepare(3, 4096, &Utf8ByteBudget).user_prompt;
        assert!(prompt.contains("step-0"));
        assert!(prompt.contains("step-5"));
        assert!(state.claim_message("message-1"));
        let mut restored: RouterContextState =
            serde_json::from_str(&serde_json::to_string(&state).unwrap()).unwrap();
        assert!(!restored.claim_message("message-1"));
        assert_eq!(restored.version, 1);
    }

    #[test]
    fn failed_summary_backlog_is_bounded_with_explicit_omission() {
        let mut state = RouterContextState::default();
        for index in 0..200 {
            append_round(&mut state, index);
        }
        assert_eq!(state.entries.len(), 128);
        assert_eq!(state.omitted_through, 72);
        assert_eq!(state.observed_rounds, 200);
        let work = state.summary_work(3, 4, &Utf8ByteBudget).unwrap();
        assert_eq!(work.through, 197);
        let prepared = state.prepare(3, 4096, &Utf8ByteBudget);
        assert!(prepared.user_prompt.contains("step-199"));
        assert_eq!(prepared.omitted_through, 72);
        assert!(prepared.input_tokens <= 4096);
    }

    #[test]
    fn large_output_is_projected_before_expensive_tokenization_and_keeps_errors() {
        struct CheckedCounter;
        impl RouterTokenCounter for CheckedCounter {
            fn count(&self, text: &str) -> usize {
                assert!(
                    text.len() <= 65_536,
                    "raw logs must not reach the tokenizer"
                );
                text.len()
            }
            fn name(&self) -> &'static str {
                "checked_byte_count"
            }
        }
        let tools: Vec<_> = (0..40).map(|index| json!({
            "tool_id": index, "is_error": index == 0,
            "result_for_assistant": if index == 0 { "CRITICAL_ASSERTION_FAILURE".to_string() } else { "large log 世界 ".repeat(10_000) }
        })).collect();
        let mut state = RouterContextState::default();
        state.append(
            "error-round".into(),
            RouterEntryKind::Round,
            json!({"assistant": {"text": "test"}, "tool_results": tools}),
            &CheckedCounter,
        );
        assert!(state.entries[0].is_error);
        let prepared = state.prepare(3, 4096, &CheckedCounter);
        assert!(prepared.user_prompt.contains("CRITICAL_ASSERTION_FAILURE"));
        assert!(prepared.user_prompt.contains("omitted"));
        assert!(prepared.input_tokens <= 4096);
    }
}
