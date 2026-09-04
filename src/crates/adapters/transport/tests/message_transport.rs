use openbitfun_transport::{encode_json_with_limit, JsonCodecError};

#[test]
fn bounded_json_encoding_stops_before_exceeding_the_budget() {
    let error = encode_json_with_limit(&serde_json::json!({ "payload": "x".repeat(64) }), 32)
        .expect_err("oversized JSON must be rejected");

    assert!(matches!(
        error,
        JsonCodecError::PayloadTooLarge {
            minimum_size: 33,
            max_bytes: 32
        }
    ));
}
