use bitfun_product_domains::review::{
    decide_review_quality, fallback_review_plan, validate_review_plan, ReviewExecutionMode,
    ReviewIntent, ReviewLevel, ReviewMode, ReviewPlan, ReviewPlanValidationError,
    ReviewQualityDecisionReason, ReviewQualityDecisionRequest, ReviewStrategyLevel,
    ReviewSubjectCandidate, ReviewSubjectRole, ReviewTargetFacts, ReviewTargetResolution,
};

fn issue_candidate(id: &str) -> ReviewSubjectCandidate {
    ReviewSubjectCandidate::Issue {
        id: id.into(),
        web_url: format!("https://example.com/issues/{id}"),
        host: "example.com".into(),
        project_path: "owner/project".into(),
        issue_id: id.into(),
    }
}

fn workspace_candidate(id: &str) -> ReviewSubjectCandidate {
    ReviewSubjectCandidate::Workspace {
        id: id.into(),
        workspace_path: "C:/workspace/project".into(),
    }
}

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

#[test]
fn review_plan_only_composes_subjects_without_prescribing_review_content() {
    let plan: ReviewPlan = serde_json::from_value(serde_json::json!({
        "mode": "single_subject",
        "subjects": [{"candidate_id": "candidate-1", "role": "primary"}]
    }))
    .expect("composition should not require model-owned questions or output");

    let serialized = serde_json::to_value(plan).expect("plan should serialize");
    assert!(serialized.get("questions").is_none());
    assert!(serialized.get("output").is_none());
}

#[test]
fn fallback_composes_common_review_relationships_without_a_model() {
    let single = vec![workspace_candidate("candidate-1")];
    assert_eq!(
        fallback_review_plan(&single, "review local changes")
            .unwrap()
            .mode,
        ReviewMode::SingleSubject
    );

    let trace = vec![
        issue_candidate("candidate-1"),
        workspace_candidate("candidate-2"),
    ];
    let trace_plan = fallback_review_plan(&trace, "review against the issue").unwrap();
    assert_eq!(trace_plan.mode, ReviewMode::RequirementTrace);
    assert_eq!(validate_review_plan(&trace, &trace_plan), Ok(()));

    let ambiguous = fallback_review_plan(&trace, "review these subjects").unwrap();
    assert_eq!(ambiguous.mode, ReviewMode::MultiSubject);

    let ambiguous_chinese = fallback_review_plan(&trace, "审核 Issue 和 PR，要求重点看性能")
        .expect("ambiguous focus should remain model-owned");
    assert_eq!(ambiguous_chinese.mode, ReviewMode::MultiSubject);

    let ambiguous_english = fallback_review_plan(
        &trace,
        "Review the Issue and PR; requirement: focus on performance",
    )
    .expect("a bare requirement label should remain model-owned");
    assert_eq!(ambiguous_english.mode, ReviewMode::MultiSubject);
}

#[test]
fn explicit_comparison_intent_wins_over_candidate_kind_assumptions() {
    let candidates = vec![
        issue_candidate("candidate-1"),
        workspace_candidate("candidate-2"),
    ];

    let plan = fallback_review_plan(&candidates, "compare the issue with local changes").unwrap();

    assert_eq!(plan.mode, ReviewMode::Comparative);
}

#[test]
fn fallback_compares_two_change_sets_and_keeps_issue_as_context() {
    let candidates = vec![
        issue_candidate("candidate-1"),
        ReviewSubjectCandidate::PullRequest {
            id: "candidate-2".into(),
            web_url: "https://example.com/pulls/2".into(),
            host: "example.com".into(),
            project_path: "owner/project".into(),
            pull_request_id: "2".into(),
        },
        workspace_candidate("candidate-3"),
    ];

    let plan = fallback_review_plan(&candidates, "compare the PR with local changes").unwrap();

    assert_eq!(plan.mode, ReviewMode::Comparative);
    assert_eq!(
        plan.subjects[0].role,
        ReviewSubjectRole::SupportingReference
    );
    assert_eq!(plan.subjects[1].role, ReviewSubjectRole::Baseline);
    assert_eq!(
        plan.subjects[2].role,
        ReviewSubjectRole::CandidateImplementation
    );
    assert_eq!(validate_review_plan(&candidates, &plan), Ok(()));
}

#[test]
fn composition_rejects_empty_duplicate_or_excessive_candidate_catalogs() {
    assert_eq!(
        fallback_review_plan(&[], "review"),
        Err(ReviewPlanValidationError::EmptyCandidateCatalog)
    );

    let duplicates = vec![
        workspace_candidate("candidate-1"),
        issue_candidate("candidate-1"),
    ];
    assert!(matches!(
        fallback_review_plan(&duplicates, "review"),
        Err(ReviewPlanValidationError::DuplicateCandidateCatalogId { .. })
    ));

    let blank_id = vec![workspace_candidate(" ")];
    assert!(matches!(
        fallback_review_plan(&blank_id, "review"),
        Err(ReviewPlanValidationError::InvalidCandidateId)
    ));

    let excessive = (0..17)
        .map(|index| workspace_candidate(&format!("candidate-{index}")))
        .collect::<Vec<_>>();
    assert!(matches!(
        fallback_review_plan(&excessive, "review"),
        Err(ReviewPlanValidationError::TooManyCandidates { .. })
    ));
}

#[test]
fn composition_bounds_plan_subjects_ids_and_focus_before_scanning() {
    let candidates = vec![
        workspace_candidate("candidate-1"),
        workspace_candidate("candidate-2"),
    ];
    let oversized_plan: ReviewPlan = serde_json::from_value(serde_json::json!({
        "mode": "multi_subject",
        "subjects": (0..17).map(|index| serde_json::json!({
            "candidate_id": format!("candidate-{index}"),
            "role": "primary"
        })).collect::<Vec<_>>()
    }))
    .unwrap();
    assert!(matches!(
        validate_review_plan(&candidates, &oversized_plan),
        Err(ReviewPlanValidationError::TooManyPlanSubjects { .. })
    ));

    let oversized_id_plan: ReviewPlan = serde_json::from_value(serde_json::json!({
        "mode": "multi_subject",
        "subjects": [
            {"candidate_id": "x".repeat(257), "role": "primary"},
            {"candidate_id": "candidate-2", "role": "primary"}
        ]
    }))
    .unwrap();
    assert_eq!(
        validate_review_plan(&candidates, &oversized_id_plan),
        Err(ReviewPlanValidationError::InvalidCandidateId)
    );

    let focus = format!("{} compare", "x".repeat(8_000));
    assert_eq!(
        fallback_review_plan(&candidates, &focus).unwrap().mode,
        ReviewMode::MultiSubject
    );
}
