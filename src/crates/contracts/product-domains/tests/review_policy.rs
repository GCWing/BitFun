use bitfun_product_domains::review::{
    decide_review_quality, ReviewExecutionMode, ReviewIntent, ReviewLevel,
    ReviewQualityDecisionReason, ReviewQualityDecisionRequest, ReviewStrategyLevel,
    ReviewTargetFacts, ReviewTargetResolution,
};

fn request(intent: ReviewIntent) -> ReviewQualityDecisionRequest {
    ReviewQualityDecisionRequest {
        intent,
        target: ReviewTargetFacts {
            resolution: ReviewTargetResolution::Resolved,
            file_count: 1,
            total_lines_changed: Some(10),
            security_sensitive_file_count: 0,
            workspace_area_count: 1,
            contract_surface_changed: false,
        },
    }
}

#[test]
fn explicit_review_uses_the_least_costly_sufficient_level() {
    let quick = decide_review_quality(request(ReviewIntent::Review));
    assert_eq!(quick.level, ReviewLevel::L1);
    assert_eq!(quick.execution_mode, ReviewExecutionMode::Standard);
    assert_eq!(quick.strategy_level, ReviewStrategyLevel::Quick);
    assert!(!quick.requires_consent);

    let mut medium = request(ReviewIntent::Review);
    medium.target.file_count = 6;
    let medium = decide_review_quality(medium);
    assert_eq!(medium.level, ReviewLevel::L2);
    assert_eq!(medium.execution_mode, ReviewExecutionMode::Strict);
    assert_eq!(medium.strategy_level, ReviewStrategyLevel::Normal);
    assert!(medium.requires_consent);

    let mut broad = request(ReviewIntent::Review);
    broad.target.file_count = 24;
    let broad = decide_review_quality(broad);
    assert_eq!(broad.level, ReviewLevel::L3);
    assert_eq!(broad.execution_mode, ReviewExecutionMode::Strict);
    assert_eq!(broad.strategy_level, ReviewStrategyLevel::Deep);
    assert!(broad.requires_consent);
}

#[test]
fn strict_intent_is_explicit_and_auditable() {
    let decision = decide_review_quality(request(ReviewIntent::Strict));

    assert_eq!(decision.level, ReviewLevel::L3);
    assert_eq!(decision.strategy_level, ReviewStrategyLevel::Deep);
    assert_eq!(decision.reason, ReviewQualityDecisionReason::ExplicitStrict);
    assert!(decision.requires_consent);
}

#[test]
fn unresolved_targets_do_not_silently_fan_out() {
    let mut input = request(ReviewIntent::Review);
    input.target.resolution = ReviewTargetResolution::Unknown;
    input.target.file_count = 50;

    let decision = decide_review_quality(input);

    assert_eq!(decision.level, ReviewLevel::L1);
    assert_eq!(decision.execution_mode, ReviewExecutionMode::Standard);
    assert_eq!(
        decision.reason,
        ReviewQualityDecisionReason::UnresolvedTarget
    );
    assert!(!decision.requires_consent);
}

#[test]
fn resolved_targets_with_unknown_change_size_use_directional_review() {
    let decision = decide_review_quality(ReviewQualityDecisionRequest {
        intent: ReviewIntent::Review,
        target: ReviewTargetFacts {
            resolution: ReviewTargetResolution::Resolved,
            file_count: 1,
            total_lines_changed: None,
            security_sensitive_file_count: 0,
            workspace_area_count: 1,
            contract_surface_changed: false,
        },
    });

    assert_eq!(decision.level, ReviewLevel::L2);
    assert!(decision.requires_consent);
}

#[test]
fn sensitive_or_cross_boundary_changes_receive_directional_coverage() {
    let mut security = request(ReviewIntent::Review);
    security.target.security_sensitive_file_count = 1;
    assert_eq!(decide_review_quality(security).level, ReviewLevel::L2);

    let mut contract = request(ReviewIntent::Review);
    contract.target.contract_surface_changed = true;
    assert_eq!(decide_review_quality(contract).level, ReviewLevel::L2);

    let mut cross_area = request(ReviewIntent::Review);
    cross_area.target.workspace_area_count = 2;
    assert_eq!(decide_review_quality(cross_area).level, ReviewLevel::L2);
}

#[test]
fn serialized_contract_uses_surface_friendly_names() {
    let value = serde_json::to_value(decide_review_quality(request(ReviewIntent::Review)))
        .expect("decision should serialize");

    assert_eq!(value["level"], "l1");
    assert_eq!(value["executionMode"], "standard");
    assert_eq!(value["strategyLevel"], "quick");
    assert_eq!(value["requiresConsent"], false);
}
