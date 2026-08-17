use super::statistics::{UsageGranularity, UsageStatisticsFilter, UsageStatisticsFilterKind};
use super::types::{TimeRange, TokenUsageQuery};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};

/// Surface-neutral request for the settings usage dashboard.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenUsageStatisticsRequest {
    /// One of "last24Hours" | "today" | "thisWeek" | "thisMonth" | "all" | "custom".
    pub time_range: String,
    /// One of "hour" | "day".
    pub granularity: String,
    #[serde(default)]
    pub start: Option<DateTime<Utc>>,
    #[serde(default)]
    pub end: Option<DateTime<Utc>>,
    /// IANA time zone used for local-calendar ranges and trend buckets.
    #[serde(default)]
    pub time_zone: Option<String>,
    #[serde(default)]
    pub include_subagent: bool,
    #[serde(default)]
    pub filter_kind: UsageStatisticsFilterKind,
    #[serde(default)]
    pub filter_query: Option<String>,
}

pub struct ResolvedTokenUsageStatisticsRequest {
    pub query: TokenUsageQuery,
    pub granularity: UsageGranularity,
    pub filter: Option<UsageStatisticsFilter>,
}

impl TokenUsageStatisticsRequest {
    pub fn resolve(self) -> Result<ResolvedTokenUsageStatisticsRequest, String> {
        let time_range = match self.time_range.as_str() {
            "today" => TimeRange::Today,
            "thisWeek" => TimeRange::ThisWeek,
            "thisMonth" => TimeRange::ThisMonth,
            "all" => TimeRange::All,
            "custom" => {
                let start = self
                    .start
                    .ok_or_else(|| "custom time range requires a start timestamp".to_string())?;
                let end = self.end.unwrap_or_else(Utc::now);
                if end <= start {
                    return Err("custom time range end must be after start".to_string());
                }
                TimeRange::Custom { start, end }
            }
            _ => {
                let end = Utc::now();
                TimeRange::Custom {
                    start: end - Duration::hours(24),
                    end,
                }
            }
        };
        let granularity = match self.granularity.as_str() {
            "day" => UsageGranularity::Day,
            _ => UsageGranularity::Hour,
        };
        let filter = self
            .filter_query
            .as_deref()
            .map(str::trim)
            .filter(|query| !query.is_empty())
            .map(|query| UsageStatisticsFilter {
                kind: self.filter_kind,
                query: query.to_string(),
            });

        Ok(ResolvedTokenUsageStatisticsRequest {
            query: TokenUsageQuery {
                model_id: None,
                session_id: None,
                time_range,
                time_zone: self.time_zone,
                limit: None,
                offset: None,
                include_subagent: self.include_subagent,
            },
            granularity,
            filter,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_request_without_filter_fields_defaults_to_unfiltered() {
        let request: TokenUsageStatisticsRequest = serde_json::from_value(serde_json::json!({
            "timeRange": "today",
            "granularity": "hour"
        }))
        .expect("request");

        assert_eq!(request.filter_kind, UsageStatisticsFilterKind::All);
        assert_eq!(request.resolve().expect("resolved request").filter, None);
    }

    #[test]
    fn filter_fields_deserialize_and_trim_query() {
        let request: TokenUsageStatisticsRequest = serde_json::from_value(serde_json::json!({
            "timeRange": "today",
            "granularity": "hour",
            "filterKind": "provider",
            "filterQuery": "  DeepSeek  "
        }))
        .expect("request");

        assert_eq!(
            request.resolve().expect("resolved request").filter,
            Some(UsageStatisticsFilter {
                kind: UsageStatisticsFilterKind::Provider,
                query: "DeepSeek".to_string(),
            })
        );
    }

    #[test]
    fn custom_range_rejects_non_increasing_bounds() {
        let request: TokenUsageStatisticsRequest = serde_json::from_value(serde_json::json!({
            "timeRange": "custom",
            "granularity": "day",
            "start": "2026-08-17T12:00:00Z",
            "end": "2026-08-17T12:00:00Z"
        }))
        .expect("request");

        assert_eq!(
            request.resolve().err().expect("invalid range"),
            "custom time range end must be after start"
        );
    }
}
