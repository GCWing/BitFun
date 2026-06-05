use crate::stream::types::unified::UnifiedResponse;
use chrono::{DateTime, Local};
use log::debug;
use std::collections::BTreeMap;
use std::time::Instant;

#[derive(Debug)]
pub(super) struct StreamStats {
    provider: &'static str,
    started_at: Instant,
    started_at_wall: DateTime<Local>,
    first_event_at: Option<Instant>,
    first_event_at_wall: Option<DateTime<Local>>,
    last_event_at: Option<Instant>,
    last_event_at_wall: Option<DateTime<Local>>,
    total_sse_events: usize,
    total_unified_responses: usize,
    counters: BTreeMap<String, usize>,
}

impl StreamStats {
    pub(super) fn new(provider: &'static str) -> Self {
        Self {
            provider,
            started_at: Instant::now(),
            started_at_wall: Local::now(),
            first_event_at: None,
            first_event_at_wall: None,
            last_event_at: None,
            last_event_at_wall: None,
            total_sse_events: 0,
            total_unified_responses: 0,
            counters: BTreeMap::new(),
        }
    }

    pub(super) fn record_sse_event(&mut self, event_kind: impl AsRef<str>) {
        let now = Instant::now();
        let now_wall = Local::now();
        if self.first_event_at.is_none() {
            self.first_event_at = Some(now);
            self.first_event_at_wall = Some(now_wall);
        }
        self.last_event_at = Some(now);
        self.last_event_at_wall = Some(now_wall);
        self.total_sse_events += 1;
        self.increment(format!("sse:{}", event_kind.as_ref()));
    }

    pub(super) fn increment(&mut self, label: impl Into<String>) {
        *self.counters.entry(label.into()).or_insert(0) += 1;
    }

    pub(super) fn record_unified_response(&mut self, response: &UnifiedResponse) {
        self.total_unified_responses += 1;

        let mut classified = false;

        if response.text.is_some() {
            self.increment("out:text");
            classified = true;
        }
        if response.reasoning_content.is_some() {
            self.increment("out:reasoning");
            classified = true;
        }
        if response.tool_call.is_some() {
            self.increment("out:tool_call");
            classified = true;
        }
        if response.usage.is_some() {
            self.increment("out:usage");
            classified = true;
        }
        if response.finish_reason.is_some() {
            self.increment("out:finish_reason");
            classified = true;
        }
        if response.thinking_signature.is_some() {
            self.increment("out:thinking_signature");
            classified = true;
        }
        if response.provider_metadata.is_some() {
            self.increment("out:provider_metadata");
            classified = true;
        }

        if !classified {
            self.increment("out:other");
        }
    }

    pub(super) fn log_summary(&self, reason: &str) {
        let ended_at_wall = Local::now();
        let wall_elapsed = self.started_at.elapsed();
        let wall_elapsed_ms = wall_elapsed.as_millis();
        let first_event_latency_ms = self
            .first_event_at
            .map(|instant| instant.duration_since(self.started_at).as_millis())
            .unwrap_or(0);
        let receive_elapsed_secs = match (self.first_event_at, self.last_event_at) {
            (Some(first), Some(last)) => last.duration_since(first).as_secs_f64(),
            _ => 0.0,
        };
        let receive_elapsed_ms = (receive_elapsed_secs * 1000.0).round() as u128;
        let unified_response_rate_per_sec = if receive_elapsed_secs > 0.0 {
            self.total_unified_responses as f64 / receive_elapsed_secs
        } else {
            0.0
        };
        let started_at = self.started_at_wall.format("%Y-%m-%d %H:%M:%S%.3f");
        let first_event_at = self
            .first_event_at_wall
            .map(|value| value.format("%Y-%m-%d %H:%M:%S%.3f").to_string())
            .unwrap_or_else(|| "none".to_string());
        let last_event_at = self
            .last_event_at_wall
            .map(|value| value.format("%Y-%m-%d %H:%M:%S%.3f").to_string())
            .unwrap_or_else(|| "none".to_string());
        let ended_at = ended_at_wall.format("%Y-%m-%d %H:%M:%S%.3f");
        let counter_lines = if self.counters.is_empty() {
            "counter.none=0".to_string()
        } else {
            self.counters
                .iter()
                .map(|(label, count)| format!("counter.{}={}", label, count))
                .collect::<Vec<_>>()
                .join("\n")
        };

        debug!(
            target: "ai::stream_stats",
            "{} stream stats\nreason={}\nstarted_at={}\nfirst_event_at={}\nlast_event_at={}\nended_at={}\ntotal_sse_events={}\ntotal_unified_responses={}\nfirst_event_latency_ms={}\nreceive_elapsed_ms={}\nwall_elapsed_ms={}\nunified_response_rate_per_sec={:.2}\n{}",
            self.provider,
            reason,
            started_at,
            first_event_at,
            last_event_at,
            ended_at,
            self.total_sse_events,
            self.total_unified_responses,
            first_event_latency_ms,
            receive_elapsed_ms,
            wall_elapsed_ms,
            unified_response_rate_per_sec,
            counter_lines
        );
    }
}
