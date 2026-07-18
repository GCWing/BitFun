use bitfun_core_types::{
    SessionUsageReport, UsageCoverageLevel, UsageToolCategory, SESSION_USAGE_REPORT_SCHEMA_VERSION,
};

#[test]
fn session_usage_report_is_a_shared_stable_contract() {
    let mut report = SessionUsageReport::partial_unavailable("session-1", 1_778_347_200_000);
    report.report_id = "usage-session-1-1778347200000".to_string();

    let json = serde_json::to_value(&report).expect("serialize usage report");
    let restored: SessionUsageReport =
        serde_json::from_value(json.clone()).expect("deserialize usage report");

    assert_eq!(restored, report);
    assert_eq!(
        json["schemaVersion"],
        serde_json::json!(SESSION_USAGE_REPORT_SCHEMA_VERSION)
    );
    assert_eq!(json["sessionId"], "session-1");
    assert_eq!(report.coverage.level, UsageCoverageLevel::Partial);
}

#[test]
fn usage_tool_category_keeps_the_persisted_wire_values() {
    assert_eq!(
        serde_json::to_value(UsageToolCategory::Git).expect("serialize category"),
        "git"
    );
}
