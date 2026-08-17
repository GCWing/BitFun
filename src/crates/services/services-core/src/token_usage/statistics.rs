//! Aggregation of raw token usage records into dashboard statistics.
//!
//! The pure functions in this module own the shape of the usage statistics
//! surfaced by the settings "usage statistics" page: totals, distribution
//! breakdowns (model / provider group / endpoint) with per-entry cache hit
//! rates, and a token trend series. Callers supply per-record attribution
//! (provider group, endpoint) so the module stays free of configuration and
//! catalog dependencies.

use super::types::TokenUsageRecord;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Bucketing granularity for the token usage trend.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UsageGranularity {
    Hour,
    Day,
}

/// Per-record attribution resolved by the caller.
#[derive(Debug, Clone)]
pub struct UsageAttribution {
    /// Provider group label (e.g. "OpenAI").
    pub group: String,
    /// Endpoint label (e.g. "api.openai.com/v1/chat/completions").
    pub endpoint: String,
}

/// One row of a distribution breakdown (model / group / endpoint).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageStatisticsEntry {
    pub name: String,
    pub requests: u32,
    pub tokens: u64,
    /// Cache hit ratio (0.0..=1.0) for this entry when any request reported
    /// cache telemetry, otherwise `None`.
    pub cache_hit_rate: Option<f64>,
    /// Cache HIT tokens served for this entry.
    #[serde(skip)]
    pub cached_tokens: u64,
    /// Prompt input tokens from requests that reported cache telemetry.
    #[serde(skip)]
    pub reported_input_tokens: u64,
}

impl UsageStatisticsEntry {
    fn new(name: String) -> Self {
        Self {
            name,
            requests: 0,
            tokens: 0,
            cache_hit_rate: None,
            cached_tokens: 0,
            reported_input_tokens: 0,
        }
    }
}

/// One bucket of the token usage trend.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageTrendPoint {
    /// Bucket start (UTC). Hourly buckets are aligned to the hour, daily buckets
    /// to midnight UTC.
    pub bucket: DateTime<Utc>,
    pub input_tokens: u64,
    pub output_tokens: u64,
    /// Tokens served from prefix cache (cache HIT).
    pub cache_read_tokens: u64,
    /// Tokens written into the cache (Anthropic cache WRITE).
    pub cache_write_tokens: u64,
    /// Cache hit ratio (0.0..=1.0) when the bucket contains cache telemetry,
    /// otherwise `None`.
    pub cache_hit_rate: Option<f64>,
}

/// Complete statistics payload for the usage dashboard.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageStatistics {
    pub total_requests: u32,
    pub total_tokens: u64,
    pub total_input_tokens: u64,
    pub total_output_tokens: u64,
    pub total_cached_tokens: u64,
    pub total_cache_write_tokens: u64,
    /// Prompt input tokens from requests that reported cache telemetry. Together
    /// with `total_cached_tokens` it yields the overall cache hit rate.
    pub total_cache_reported_input_tokens: u64,
    pub by_model: Vec<UsageStatisticsEntry>,
    pub by_group: Vec<UsageStatisticsEntry>,
    pub by_endpoint: Vec<UsageStatisticsEntry>,
    pub trend: Vec<UsageTrendPoint>,
    /// Granularity actually used for the trend (may be coarser than requested
    /// when the requested bucketing would produce too many points).
    pub granularity: UsageGranularity,
}

/// Upper bound on trend points. Very long time ranges (e.g. "all time" with
/// hourly buckets) are coarsened or truncated to the most recent buckets so the
/// payload stays small.
const MAX_TREND_POINTS: usize = 800;

/// Aggregate raw records into dashboard statistics.
///
/// `attribute` maps every record to its provider group and endpoint label; the
/// aggregation itself is pure.
pub fn aggregate_statistics<F>(
    records: &[TokenUsageRecord],
    granularity: UsageGranularity,
    attribute: F,
) -> UsageStatistics
where
    F: Fn(&TokenUsageRecord) -> UsageAttribution,
{
    let mut total_requests = 0u32;
    let mut total_input = 0u64;
    let mut total_output = 0u64;
    let mut total_cached = 0u64;
    let mut total_cache_write = 0u64;
    let mut total_cache_reported_input = 0u64;

    let mut by_model: HashMap<String, UsageStatisticsEntry> = HashMap::new();
    let mut by_group: HashMap<String, UsageStatisticsEntry> = HashMap::new();
    let mut by_endpoint: HashMap<String, UsageStatisticsEntry> = HashMap::new();

    for record in records {
        total_requests += 1;
        total_input += record.input_tokens as u64;
        total_output += record.output_tokens as u64;
        total_cached += record.cached_tokens as u64;
        total_cache_write += record.cache_write_tokens as u64;
        if record.cached_tokens_available {
            total_cache_reported_input += record.input_tokens as u64;
        }

        let attribution = attribute(record);
        accumulate_entry(&mut by_model, record.effective_model_name.clone(), record);
        accumulate_entry(&mut by_group, attribution.group, record);
        accumulate_entry(&mut by_endpoint, attribution.endpoint, record);
    }

    let (trend, effective_granularity) = build_trend(records, granularity);

    UsageStatistics {
        total_requests,
        total_tokens: total_input + total_output,
        total_input_tokens: total_input,
        total_output_tokens: total_output,
        total_cached_tokens: total_cached,
        total_cache_write_tokens: total_cache_write,
        total_cache_reported_input_tokens: total_cache_reported_input,
        by_model: finalize_entries(by_model),
        by_group: finalize_entries(by_group),
        by_endpoint: finalize_entries(by_endpoint),
        trend,
        granularity: effective_granularity,
    }
}

fn accumulate_entry(
    map: &mut HashMap<String, UsageStatisticsEntry>,
    name: String,
    record: &TokenUsageRecord,
) {
    let entry = map
        .entry(name.clone())
        .or_insert_with(|| UsageStatisticsEntry::new(name));
    entry.requests += 1;
    entry.tokens += record.total_tokens as u64;
    entry.cached_tokens += record.cached_tokens as u64;
    if record.cached_tokens_available {
        entry.reported_input_tokens += record.input_tokens as u64;
    }
}

fn finalize_entries(map: HashMap<String, UsageStatisticsEntry>) -> Vec<UsageStatisticsEntry> {
    let mut entries = map.into_values().collect::<Vec<_>>();
    for entry in &mut entries {
        entry.cache_hit_rate = if entry.reported_input_tokens == 0 {
            None
        } else if entry.cached_tokens == entry.reported_input_tokens {
            // Exact full cache hit: represent as exactly 1.0 so the UI never
            // shows 99.99...% for a mathematically complete hit.
            Some(1.0)
        } else {
            Some(entry.cached_tokens as f64 / entry.reported_input_tokens as f64)
        };
    }
    entries.sort_by(|left, right| {
        right
            .tokens
            .cmp(&left.tokens)
            .then_with(|| right.requests.cmp(&left.requests))
            .then_with(|| left.name.cmp(&right.name))
    });
    entries
}

/// Build the trend series, filling empty buckets so the chart stays contiguous.
///
/// Returns the granularity actually applied: when the requested granularity
/// would exceed [`MAX_TREND_POINTS`] buckets, bucketing is coarsened to days,
/// and when even that is too long only the most recent buckets are retained.
fn build_trend(
    records: &[TokenUsageRecord],
    granularity: UsageGranularity,
) -> (Vec<UsageTrendPoint>, UsageGranularity) {
    if records.is_empty() {
        return (Vec::new(), granularity);
    }

    let mut granularity = granularity;
    let mut buckets = bucket_records(records, granularity);

    let mut first = *buckets.keys().min().unwrap_or(&0);
    let mut last = *buckets.keys().max().unwrap_or(&0);
    let mut step_seconds = step_seconds_for(granularity);

    let mut bucket_count = (last - first) / step_seconds + 1;
    if granularity == UsageGranularity::Hour && bucket_count > MAX_TREND_POINTS as i64 {
        granularity = UsageGranularity::Day;
        step_seconds = step_seconds_for(granularity);
        // Re-key already collected buckets on the coarser grid.
        let mut rebucketed: HashMap<i64, TrendBucket> = HashMap::new();
        for (key, mut bucket) in buckets {
            bucket.key = key - key.rem_euclid(step_seconds);
            rebucketed
                .entry(bucket.key)
                .and_modify(|existing: &mut TrendBucket| existing.merge(&bucket))
                .or_insert(bucket);
        }
        buckets = rebucketed;
        first = *buckets.keys().min().unwrap_or(&0);
        last = *buckets.keys().max().unwrap_or(&0);
        bucket_count = (last - first) / step_seconds + 1;
    }

    let start = if bucket_count > MAX_TREND_POINTS as i64 {
        // Extremely long ranges keep only the most recent buckets.
        last - (MAX_TREND_POINTS as i64 - 1) * step_seconds
    } else {
        first
    };

    let mut trend = Vec::with_capacity((last - start) as usize / step_seconds as usize + 1);
    let mut cursor = start;
    while cursor <= last {
        let bucket = buckets.remove(&cursor);
        trend.push(
            TrendBucket {
                key: cursor,
                ..bucket.unwrap_or_default()
            }
            .into_point(),
        );
        cursor += step_seconds;
    }

    (trend, granularity)
}

fn step_seconds_for(granularity: UsageGranularity) -> i64 {
    match granularity {
        UsageGranularity::Hour => 3600,
        UsageGranularity::Day => 86_400,
    }
}

fn bucket_records(
    records: &[TokenUsageRecord],
    granularity: UsageGranularity,
) -> HashMap<i64, TrendBucket> {
    let mut buckets: HashMap<i64, TrendBucket> = HashMap::new();
    for record in records {
        let key = bucket_key(record.timestamp, granularity);
        let bucket = buckets.entry(key).or_insert_with(|| TrendBucket {
            key,
            ..TrendBucket::default()
        });
        bucket.input_tokens += record.input_tokens as u64;
        bucket.output_tokens += record.output_tokens as u64;
        bucket.cached_tokens += record.cached_tokens as u64;
        bucket.cache_write_tokens += record.cache_write_tokens as u64;
        if record.cached_tokens_available {
            bucket.reported_input_tokens += record.input_tokens as u64;
        }
    }
    buckets
}

fn bucket_key(timestamp: DateTime<Utc>, granularity: UsageGranularity) -> i64 {
    let seconds = timestamp.timestamp();
    match granularity {
        UsageGranularity::Hour => seconds - seconds.rem_euclid(3600),
        UsageGranularity::Day => {
            let date = timestamp.date_naive();
            let midnight = date.and_hms_opt(0, 0, 0).expect("midnight is valid");
            midnight.and_utc().timestamp()
        }
    }
}

#[derive(Debug, Default, Clone, Copy)]
struct TrendBucket {
    key: i64,
    input_tokens: u64,
    output_tokens: u64,
    cached_tokens: u64,
    cache_write_tokens: u64,
    reported_input_tokens: u64,
}

impl TrendBucket {
    fn merge(&mut self, other: &TrendBucket) {
        self.input_tokens += other.input_tokens;
        self.output_tokens += other.output_tokens;
        self.cached_tokens += other.cached_tokens;
        self.cache_write_tokens += other.cache_write_tokens;
        self.reported_input_tokens += other.reported_input_tokens;
    }

    fn into_point(self) -> UsageTrendPoint {
        let cache_hit_rate = (self.reported_input_tokens > 0)
            .then(|| self.cached_tokens as f64 / self.reported_input_tokens as f64);
        UsageTrendPoint {
            bucket: DateTime::from_timestamp(self.key, 0).unwrap_or(Utc::now()),
            input_tokens: self.input_tokens,
            output_tokens: self.output_tokens,
            cache_read_tokens: self.cached_tokens,
            cache_write_tokens: self.cache_write_tokens,
            cache_hit_rate,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration, TimeZone, Timelike, Utc};

    fn record(
        model: &str,
        timestamp: DateTime<Utc>,
        input: u32,
        output: u32,
        cached: u32,
        cache_write: u32,
        cached_available: bool,
    ) -> TokenUsageRecord {
        TokenUsageRecord {
            model_config_id: "config-a".to_string(),
            effective_model_name: model.to_string(),
            session_id: "session-a".to_string(),
            turn_id: "turn-a".to_string(),
            timestamp,
            input_tokens: input,
            output_tokens: output,
            cached_tokens: cached,
            cached_tokens_available: cached_available,
            cache_write_tokens: cache_write,
            total_tokens: input + output,
            token_details: None,
            is_subagent: false,
        }
    }

    fn attribution() -> impl Fn(&TokenUsageRecord) -> UsageAttribution {
        |record| UsageAttribution {
            group: format!("provider-of-{}", record.effective_model_name),
            endpoint: format!("/endpoint-of-{}", record.effective_model_name),
        }
    }

    #[test]
    fn aggregates_totals_and_breakdowns() {
        let t0 = Utc.with_ymd_and_hms(2026, 8, 16, 10, 0, 0).unwrap();
        let records = vec![
            record("model-a", t0, 1000, 200, 500, 0, true),
            record(
                "model-a",
                t0 + Duration::minutes(30),
                2000,
                300,
                0,
                100,
                false,
            ),
            record("model-b", t0 + Duration::hours(2), 500, 50, 0, 0, false),
        ];

        let stats = aggregate_statistics(&records, UsageGranularity::Hour, attribution());

        assert_eq!(stats.total_requests, 3);
        assert_eq!(stats.total_input_tokens, 3500);
        assert_eq!(stats.total_output_tokens, 550);
        assert_eq!(stats.total_cached_tokens, 500);
        assert_eq!(stats.total_cache_write_tokens, 100);
        assert_eq!(stats.total_tokens, 4050);
        assert_eq!(stats.total_cache_reported_input_tokens, 1000);

        assert_eq!(stats.by_model.len(), 2);
        let model_a = stats
            .by_model
            .iter()
            .find(|entry| entry.name == "model-a")
            .unwrap();
        assert_eq!(model_a.requests, 2);
        assert_eq!(model_a.tokens, 3500);
        assert_eq!(model_a.cached_tokens, 500);
        assert_eq!(model_a.reported_input_tokens, 1000);
        assert!((model_a.cache_hit_rate.unwrap() - 0.5).abs() < 1e-9);

        let model_b = stats
            .by_model
            .iter()
            .find(|entry| entry.name == "model-b")
            .unwrap();
        assert!(model_b.cache_hit_rate.is_none());

        assert_eq!(stats.by_group.len(), 2);
        assert_eq!(stats.by_endpoint.len(), 2);
    }

    #[test]
    fn entry_without_cache_telemetry_has_no_hit_rate() {
        let t0 = Utc.with_ymd_and_hms(2026, 8, 16, 10, 0, 0).unwrap();
        let records = vec![record("model-a", t0, 100, 100, 0, 0, false)];
        let stats = aggregate_statistics(&records, UsageGranularity::Hour, attribution());
        assert_eq!(stats.by_model[0].cache_hit_rate, None);
        assert_eq!(stats.total_cache_reported_input_tokens, 0);
    }

    #[test]
    fn exact_full_cache_hit_is_reported_as_exactly_one() {
        let t0 = Utc.with_ymd_and_hms(2026, 8, 16, 10, 0, 0).unwrap();
        // cached == input on a single request: mathematically a 100% hit.
        let records = vec![record("model-a", t0, 1000, 0, 1000, 0, true)];
        let stats = aggregate_statistics(&records, UsageGranularity::Hour, attribution());
        assert_eq!(stats.by_model[0].cache_hit_rate, Some(1.0));
        assert_eq!(
            stats.total_cached_tokens,
            stats.total_cache_reported_input_tokens
        );
    }

    #[test]
    fn trend_buckets_by_hour_and_fills_gaps() {
        let t0 = Utc.with_ymd_and_hms(2026, 8, 16, 10, 0, 0).unwrap();
        let records = vec![
            record("model-a", t0, 100, 0, 0, 0, false),
            record("model-a", t0 + Duration::hours(3), 200, 0, 0, 0, false),
        ];
        let stats = aggregate_statistics(&records, UsageGranularity::Hour, attribution());
        assert_eq!(stats.granularity, UsageGranularity::Hour);
        assert_eq!(stats.trend.len(), 4);
        assert_eq!(stats.trend[0].input_tokens, 100);
        assert_eq!(stats.trend[1].input_tokens, 0);
        assert_eq!(stats.trend[3].input_tokens, 200);
        assert_eq!(
            stats.trend[0].bucket,
            t0.with_minute(0).unwrap().with_second(0).unwrap()
        );
    }

    #[test]
    fn trend_cache_hit_rate_uses_reported_input() {
        let t0 = Utc.with_ymd_and_hms(2026, 8, 16, 10, 0, 0).unwrap();
        let records = vec![
            record("model-a", t0, 1000, 0, 250, 0, true),
            record("model-b", t0 + Duration::minutes(10), 100, 0, 0, 0, false),
        ];
        let stats = aggregate_statistics(&records, UsageGranularity::Hour, attribution());
        let point = &stats.trend[0];
        assert_eq!(point.input_tokens, 1100);
        assert_eq!(point.cache_read_tokens, 250);
        // Only the record with explicit cache telemetry feeds the ratio.
        assert!((point.cache_hit_rate.unwrap() - 0.25).abs() < 1e-9);
    }

    #[test]
    fn trend_coarsens_to_days_when_hourly_span_is_too_long() {
        let t0 = Utc.with_ymd_and_hms(2026, 8, 16, 0, 0, 0).unwrap();
        let records = vec![
            record("model-a", t0, 100, 0, 0, 0, false),
            record("model-a", t0 + Duration::days(60), 200, 0, 0, 0, false),
        ];
        let stats = aggregate_statistics(&records, UsageGranularity::Hour, attribution());
        assert_eq!(stats.granularity, UsageGranularity::Day);
        assert_eq!(stats.trend.len(), 61);
    }

    #[test]
    fn empty_records_produce_empty_statistics() {
        let stats = aggregate_statistics(&[], UsageGranularity::Hour, attribution());
        assert_eq!(stats.total_requests, 0);
        assert_eq!(stats.trend.len(), 0);
        assert!(stats.by_model.is_empty());
    }
}
