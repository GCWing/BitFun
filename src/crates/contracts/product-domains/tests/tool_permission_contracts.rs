use bitfun_product_domains::tool_permissions::{
    merge_permission_rule_layers, wildcard_matches, PermissionEffect, PermissionEvaluator,
    PermissionReply, PermissionResourceCaseSensitivity, PermissionRule,
};
use serde_json::json;

fn rule(action: &str, resource: &str, effect: PermissionEffect) -> PermissionRule {
    PermissionRule::new(action, resource, effect)
}

#[test]
fn permission_rule_uses_stable_wire_values() {
    let value = serde_json::to_value(rule("read", "src/*", PermissionEffect::Ask))
        .expect("serialize permission rule");

    assert_eq!(
        value,
        json!({
            "action": "read",
            "resource": "src/*",
            "effect": "ask",
        })
    );
    assert_eq!(
        serde_json::from_value::<PermissionRule>(value).expect("deserialize permission rule"),
        rule("read", "src/*", PermissionEffect::Ask)
    );
}

#[test]
fn permission_reply_uses_stable_tagged_wire_values() {
    assert_eq!(
        serde_json::to_value(PermissionReply::Once).expect("serialize once reply"),
        json!({ "reply": "once" })
    );
    assert_eq!(
        serde_json::to_value(PermissionReply::Always).expect("serialize always reply"),
        json!({ "reply": "always" })
    );
    assert_eq!(
        serde_json::to_value(PermissionReply::Reject {
            feedback: Some("Use a read-only path".to_string()),
        })
        .expect("serialize reject reply"),
        json!({
            "reply": "reject",
            "feedback": "Use a read-only path",
        })
    );
}

#[test]
fn wildcard_matching_supports_star_question_and_normalized_separators() {
    let sensitive = PermissionResourceCaseSensitivity::Sensitive;

    assert!(wildcard_matches("src/main.rs", "src/*.rs", sensitive));
    assert!(wildcard_matches("src/main.rs", "src/mai?.rs", sensitive));
    assert!(wildcard_matches(
        r"src\nested\main.rs",
        "src/*/main.rs",
        sensitive
    ));
    assert!(wildcard_matches("git", "git *", sensitive));
    assert!(wildcard_matches("git status", "git *", sensitive));
    assert!(!wildcard_matches("src/main.ts", "src/*.rs", sensitive));
    assert!(!wildcard_matches(
        "src/deep/main.rs",
        "src/????.rs",
        sensitive
    ));
}

#[test]
fn windows_compatible_matching_is_case_insensitive_for_resources() {
    let evaluator = PermissionEvaluator::windows_compatible();
    let rules = vec![rule(
        "read",
        r"C:\Users\Developer\Project\*",
        PermissionEffect::Allow,
    )];

    assert_eq!(
        evaluator.evaluate_resource("read", r"c:\users\developer\project\SRC\main.rs", &rules,),
        PermissionEffect::Allow
    );
    assert_eq!(
        PermissionEvaluator::case_sensitive().evaluate_resource(
            "read",
            r"c:\users\developer\project\SRC\main.rs",
            &rules,
        ),
        PermissionEffect::Ask
    );
}

#[test]
fn last_matching_action_and_resource_rule_wins() {
    let evaluator = PermissionEvaluator::case_sensitive();
    let rules = vec![
        rule("*", "*", PermissionEffect::Ask),
        rule("read", "src/*", PermissionEffect::Allow),
        rule("read", "src/private/*", PermissionEffect::Deny),
        rule("read", "src/private/public.txt", PermissionEffect::Allow),
    ];

    assert_eq!(
        evaluator.evaluate_resource("read", "src/lib.rs", &rules),
        PermissionEffect::Allow
    );
    assert_eq!(
        evaluator.evaluate_resource("read", "src/private/key.txt", &rules),
        PermissionEffect::Deny
    );
    assert_eq!(
        evaluator.evaluate_resource("read", "src/private/public.txt", &rules),
        PermissionEffect::Allow
    );
    assert_eq!(
        evaluator.evaluate_resource("edit", "src/lib.rs", &rules),
        PermissionEffect::Ask
    );
}

#[test]
fn merged_layers_preserve_global_project_agent_override_order() {
    let global = vec![rule("*", "*", PermissionEffect::Ask)];
    let project = vec![rule("read", "*", PermissionEffect::Allow)];
    let agent = vec![rule("read", "secrets/*", PermissionEffect::Deny)];
    let merged = merge_permission_rule_layers(&[&global, &project, &agent]);
    let evaluator = PermissionEvaluator::case_sensitive();

    assert_eq!(merged, [global, project, agent].concat());
    assert_eq!(
        evaluator.evaluate_resource("read", "README.md", &merged),
        PermissionEffect::Allow
    );
    assert_eq!(
        evaluator.evaluate_resource("read", "secrets/token.txt", &merged),
        PermissionEffect::Deny
    );
}

#[test]
fn unmatched_and_empty_resource_requests_default_to_ask() {
    let evaluator = PermissionEvaluator::case_sensitive();
    let rules = vec![rule("read", "src/*", PermissionEffect::Allow)];

    assert_eq!(
        evaluator.evaluate_resource("edit", "src/lib.rs", &rules),
        PermissionEffect::Ask
    );
    assert_eq!(
        evaluator.evaluate_resources("read", &[], &rules),
        PermissionEffect::Ask
    );
}

#[test]
fn multi_resource_decision_is_atomic_with_deny_then_ask_precedence() {
    let evaluator = PermissionEvaluator::case_sensitive();
    let rules = vec![
        rule("edit", "src/*", PermissionEffect::Allow),
        rule("edit", "src/generated/*", PermissionEffect::Ask),
        rule("edit", "src/secrets/*", PermissionEffect::Deny),
    ];

    assert_eq!(
        evaluator.evaluate_resources("edit", &["src/lib.rs".into(), "src/main.rs".into()], &rules,),
        PermissionEffect::Allow
    );
    assert_eq!(
        evaluator.evaluate_resources(
            "edit",
            &["src/lib.rs".into(), "src/generated/api.rs".into()],
            &rules,
        ),
        PermissionEffect::Ask
    );
    assert_eq!(
        evaluator.evaluate_resources(
            "edit",
            &["src/generated/api.rs".into(), "src/secrets/key.rs".into(),],
            &rules,
        ),
        PermissionEffect::Deny
    );
}
