use bitfun_services_core::token_usage::{TimeRange, TokenUsageQuery, TokenUsageRecord};
use chrono::Utc;

#[test]
fn token_usage_record_preserves_cached_availability_default() {
    let record: TokenUsageRecord = serde_json::from_value(serde_json::json!({
        "model_id": "model-a",
        "session_id": "session-1",
        "turn_id": "turn-1",
        "timestamp": Utc::now(),
        "input_tokens": 10,
        "output_tokens": 5,
        "cached_tokens": 0,
        "total_tokens": 15
    }))
    .expect("legacy token usage record should deserialize");

    assert!(!record.cached_tokens_available);
    assert_eq!(record.llm_latency_ms, None);
}

#[test]
fn token_usage_record_serializes_llm_latency() {
    let record: TokenUsageRecord = serde_json::from_value(serde_json::json!({
        "model_id": "model-a",
        "session_id": "session-1",
        "turn_id": "turn-1",
        "timestamp": Utc::now(),
        "input_tokens": 10,
        "output_tokens": 5,
        "cached_tokens": 0,
        "cached_tokens_available": false,
        "total_tokens": 15,
        "llm_latency_ms": 2500
    }))
    .expect("token usage record should deserialize");

    let json = serde_json::to_value(record).expect("record should serialize");
    assert_eq!(json["llm_latency_ms"], 2500);
}

#[test]
fn token_usage_query_preserves_include_subagent_default() {
    let query = TokenUsageQuery {
        model_id: None,
        session_id: None,
        time_range: TimeRange::All,
        limit: None,
        offset: None,
        include_subagent: false,
    };

    let restored: TokenUsageQuery =
        serde_json::from_value(serde_json::to_value(query).expect("query should serialize"))
            .expect("query should deserialize");

    assert!(!restored.include_subagent);
}
