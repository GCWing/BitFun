use bitfun_services_core::token_usage::{
    ModelTokenStats, SessionTokenStats, TimeRange, TokenUsageQuery, TokenUsageRecord,
};
use chrono::Utc;

#[test]
fn token_usage_record_preserves_model_identity_and_cached_availability_default() {
    let record: TokenUsageRecord = serde_json::from_value(serde_json::json!({
        "model_config_id": "model-config-a",
        "effective_model_name": "model-a",
        "session_id": "session-1",
        "turn_id": "turn-1",
        "timestamp": Utc::now(),
        "input_tokens": 10,
        "output_tokens": 5,
        "cached_tokens": 0,
        "total_tokens": 15
    }))
    .expect("token usage record should deserialize");

    assert_eq!(record.model_config_id, "model-config-a");
    assert_eq!(record.effective_model_name, "model-a");
    assert!(!record.cached_tokens_available);
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

#[test]
fn model_cache_hit_ratio_requires_reported_cache_telemetry() {
    let no_telemetry = ModelTokenStats {
        model_id: "model-a".to_string(),
        total_input: 100,
        total_cached: 0,
        ..Default::default()
    };
    assert_eq!(no_telemetry.cache_hit_ratio(), None);

    let reported = ModelTokenStats {
        model_id: "model-a".to_string(),
        total_input: 300,
        total_cached: 50,
        cache_reported_input_tokens: 100,
        ..Default::default()
    };
    assert_eq!(reported.cache_hit_ratio(), Some(0.5));
}

#[test]
fn session_cache_hit_ratio_uses_reported_input_denominator() {
    let stats = SessionTokenStats {
        session_id: "session-1".to_string(),
        model_id: "model-a".to_string(),
        total_input: 300,
        total_output: 20,
        total_cached: 50,
        total_cache_write: 0,
        cache_reported_input_tokens: 100,
        total_tokens: 320,
        request_count: 2,
        created_at: Utc::now(),
        last_updated: Utc::now(),
    };

    assert_eq!(stats.cache_hit_ratio(), Some(0.5));
}
