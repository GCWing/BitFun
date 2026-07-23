use bitfun_product_domains::tool_permissions::{
    merge_permission_rule_layers, resolve_child_permission_policy, resolve_permission_policy,
    wildcard_matches, ChildPermissionPolicyLayers, PermissionDelegationContext, PermissionEffect,
    PermissionEvaluator, PermissionPolicyConfig, PermissionPolicyLayers, PermissionPolicyPreset,
    PermissionReply, PermissionReplySource, PermissionRequest, PermissionRequestEvent,
    PermissionRequestSource, PermissionRequestSourceKind, PermissionResourceCaseSensitivity,
    PermissionRule, PermissionRuntimeCeiling, ToolPermissionConfig,
};
use serde_json::json;
use serde_json::Map;

fn rule(action: &str, resource: &str, effect: PermissionEffect) -> PermissionRule {
    PermissionRule::new(action, resource, effect)
}

fn policy(preset: PermissionPolicyPreset, rules: Vec<PermissionRule>) -> PermissionPolicyConfig {
    PermissionPolicyConfig { preset, rules }
}

#[test]
fn tool_permission_config_defaults_to_ask_with_auto_approve_disabled() {
    let config = ToolPermissionConfig::default();

    assert_eq!(config.policy.preset, PermissionPolicyPreset::Ask);
    assert!(config.policy.rules.is_empty());
    assert!(!config.interaction.auto_approve_ask);
    assert_eq!(
        serde_json::to_value(config).expect("serialize tool permission config"),
        json!({
            "policy": {
                "preset": "ask",
                "rules": [],
            },
            "interaction": {
                "auto_approve_ask": false,
            },
        })
    );
}

#[test]
fn policy_presets_expand_into_ordinary_baseline_rules() {
    let ask = policy(PermissionPolicyPreset::Ask, Vec::new());
    let full_access = policy(PermissionPolicyPreset::FullAccess, Vec::new());
    let evaluator = PermissionEvaluator::case_sensitive();

    let ask_rules = resolve_permission_policy(PermissionPolicyLayers {
        product_defaults: &[],
        global: &ask,
        project: &[],
        agent: &[],
        enforced: &[],
    });
    let full_access_rules = resolve_permission_policy(PermissionPolicyLayers {
        product_defaults: &[],
        global: &full_access,
        project: &[],
        agent: &[],
        enforced: &[],
    });

    assert_eq!(
        ask_rules,
        vec![
            rule("*", "*", PermissionEffect::Ask),
            rule("read", "*", PermissionEffect::Allow),
            rule("read", "*/.env", PermissionEffect::Ask),
            rule("read", "*/.env.*", PermissionEffect::Ask),
            rule("read", "*/.env.example", PermissionEffect::Allow),
            rule("websearch", "*", PermissionEffect::Allow),
            rule("webfetch", "*", PermissionEffect::Allow),
            rule("task", "*", PermissionEffect::Allow),
            rule("skill", "*", PermissionEffect::Allow),
            rule("git", "git status *", PermissionEffect::Allow),
            rule("git", "git diff *", PermissionEffect::Allow),
            rule("git", "git log *", PermissionEffect::Allow),
            rule("git", "git show *", PermissionEffect::Allow),
            rule("git", "git blame *", PermissionEffect::Allow),
            rule("git", "git rev-parse *", PermissionEffect::Allow),
            rule("git", "git describe *", PermissionEffect::Allow),
            rule("git", "git shortlog *", PermissionEffect::Allow),
            rule("git", "git branch", PermissionEffect::Allow),
        ]
    );
    assert_eq!(
        full_access_rules,
        vec![rule("*", "*", PermissionEffect::Allow)]
    );
    assert_eq!(
        evaluator.evaluate_resource("edit", "src/main.rs", &ask_rules),
        PermissionEffect::Ask
    );
    assert_eq!(
        evaluator.evaluate_resource("edit", "src/main.rs", &full_access_rules),
        PermissionEffect::Allow
    );
}

#[test]
fn ask_preset_allows_low_risk_actions_and_keeps_mutations_guarded() {
    let rules = resolve_permission_policy(PermissionPolicyLayers {
        product_defaults: &[],
        global: &policy(PermissionPolicyPreset::Ask, Vec::new()),
        project: &[],
        agent: &[],
        enforced: &[],
    });
    let evaluator = PermissionEvaluator::case_sensitive();

    for (action, resource) in [
        ("read", "C:/repo/README.md"),
        ("read", "C:/repo/.env.example"),
        ("websearch", "BitFun permission model"),
        ("webfetch", "https://example.com/docs"),
        ("task", "general"),
        ("task", "send_input:session-1"),
        ("skill", "pdf"),
        ("git", "git status"),
        ("git", "git diff --staged"),
        ("git", "git log --oneline -10"),
        ("git", "git show HEAD"),
        ("git", "git blame src/main.rs"),
        ("git", "git rev-parse HEAD"),
        ("git", "git describe --tags"),
        ("git", "git shortlog -sn"),
        ("git", "git branch"),
    ] {
        assert_eq!(
            evaluator.evaluate_resource(action, resource, &rules),
            PermissionEffect::Allow,
            "{action} {resource}"
        );
    }

    for (action, resource) in [
        ("read", "C:/repo/.env"),
        ("read", "C:/repo/.env.local"),
        ("external_directory", "C:/outside"),
        ("edit", "C:/repo/src/main.rs"),
        ("bash", "cargo test"),
        ("git", "git branch feature/new"),
        ("git", "git add src/main.rs"),
        ("git", "git commit -m change"),
        ("git", "git push origin main"),
        ("mcp", "server/tool"),
        ("future_action", "resource"),
    ] {
        assert_eq!(
            evaluator.evaluate_resource(action, resource, &rules),
            PermissionEffect::Ask,
            "{action} {resource}"
        );
    }
}

#[test]
fn resolved_policy_preserves_layer_order_and_enforced_limits() {
    let product_defaults = vec![rule("read", "*", PermissionEffect::Allow)];
    let global = policy(
        PermissionPolicyPreset::FullAccess,
        vec![rule("bash", "rm *", PermissionEffect::Ask)],
    );
    let project = vec![rule("edit", "generated/*", PermissionEffect::Deny)];
    let agent = vec![rule("edit", "generated/review.md", PermissionEffect::Allow)];
    let enforced = vec![rule("edit", "generated/*", PermissionEffect::Deny)];

    let resolved = resolve_permission_policy(PermissionPolicyLayers {
        product_defaults: &product_defaults,
        global: &global,
        project: &project,
        agent: &agent,
        enforced: &enforced,
    });

    assert_eq!(
        resolved,
        [
            product_defaults,
            PermissionPolicyPreset::FullAccess.baseline_rules(),
            global.rules,
            project,
            agent,
            enforced,
        ]
        .concat()
    );

    let evaluator = PermissionEvaluator::case_sensitive();
    assert_eq!(
        evaluator.evaluate_resource("bash", "rm -rf target", &resolved),
        PermissionEffect::Ask
    );
    assert_eq!(
        evaluator.evaluate_resource("edit", "generated/review.md", &resolved),
        PermissionEffect::Deny
    );
    assert_eq!(
        evaluator.evaluate_resource("webfetch", "https://example.com", &resolved),
        PermissionEffect::Allow
    );
}

#[test]
fn runtime_ceiling_accepts_empty_ask_and_deny_rules() {
    assert!(PermissionRuntimeCeiling::try_new(Vec::new())
        .expect("empty ceiling should be valid")
        .is_empty());

    let rules = vec![
        rule("read", "secrets/*", PermissionEffect::Ask),
        rule("bash", "rm *", PermissionEffect::Deny),
    ];
    let ceiling = PermissionRuntimeCeiling::try_new(rules.clone())
        .expect("ask and deny rules should be valid ceiling restrictions");
    assert_eq!(ceiling.rules(), rules);
}

#[test]
fn runtime_ceiling_rejects_allow_rules_with_typed_context() {
    let error = PermissionRuntimeCeiling::try_new(vec![
        rule("read", "secrets/*", PermissionEffect::Ask),
        rule("bash", "cargo test", PermissionEffect::Allow),
    ])
    .expect_err("allow must not enter a runtime ceiling");

    assert_eq!(error.rule_index, 1);
    assert_eq!(error.action, "bash");
    assert_eq!(error.resource, "cargo test");
}

#[test]
fn child_policy_preserves_exact_layer_order_and_security_precedence() {
    let product_defaults = vec![rule("read", "*", PermissionEffect::Allow)];
    let global = policy(
        PermissionPolicyPreset::Ask,
        vec![rule("edit", "generated/*", PermissionEffect::Ask)],
    );
    let project = vec![rule("edit", "generated/*", PermissionEffect::Deny)];
    let child_agent = vec![rule("edit", "generated/review.md", PermissionEffect::Allow)];
    let ceiling_rules = vec![rule("edit", "generated/review.md", PermissionEffect::Ask)];
    let ceiling = PermissionRuntimeCeiling::try_new(ceiling_rules.clone())
        .expect("ask ceiling should be valid");
    let enforced = vec![rule("edit", "generated/review.md", PermissionEffect::Deny)];

    let resolved = resolve_child_permission_policy(ChildPermissionPolicyLayers {
        product_defaults: &product_defaults,
        global: &global,
        project: &project,
        child_agent: &child_agent,
        parent_runtime_ceiling: &ceiling,
        enforced: &enforced,
    });

    assert_eq!(
        resolved,
        [
            product_defaults,
            PermissionPolicyPreset::Ask.baseline_rules(),
            global.rules,
            project,
            child_agent,
            ceiling_rules,
            enforced,
        ]
        .concat()
    );

    let evaluator = PermissionEvaluator::case_sensitive();
    assert_eq!(
        evaluator.evaluate_resource("edit", "generated/review.md", &resolved),
        PermissionEffect::Deny,
        "enforced rules must remain later than the parent ceiling"
    );
}

#[test]
fn parent_ceiling_overrides_child_agent_allow() {
    let global = policy(PermissionPolicyPreset::FullAccess, Vec::new());
    let child_agent = vec![rule("read", "secrets/*", PermissionEffect::Allow)];
    let ceiling =
        PermissionRuntimeCeiling::try_new(vec![rule("read", "secrets/*", PermissionEffect::Deny)])
            .expect("deny ceiling should be valid");

    let resolved = resolve_child_permission_policy(ChildPermissionPolicyLayers {
        product_defaults: &[],
        global: &global,
        project: &[],
        child_agent: &child_agent,
        parent_runtime_ceiling: &ceiling,
        enforced: &[],
    });

    assert_eq!(
        PermissionEvaluator::case_sensitive().evaluate_resource(
            "read",
            "secrets/token.txt",
            &resolved,
        ),
        PermissionEffect::Deny
    );
}

#[test]
fn task_and_skill_default_allow_do_not_authorize_child_tools() {
    let global = policy(PermissionPolicyPreset::Ask, Vec::new());
    let ceiling = PermissionRuntimeCeiling::default();
    let resolved = resolve_child_permission_policy(ChildPermissionPolicyLayers {
        product_defaults: &[],
        global: &global,
        project: &[],
        child_agent: &[],
        parent_runtime_ceiling: &ceiling,
        enforced: &[],
    });
    let evaluator = PermissionEvaluator::case_sensitive();

    assert_eq!(
        evaluator.evaluate_resource("task", "Explore", &resolved),
        PermissionEffect::Allow
    );
    assert_eq!(
        evaluator.evaluate_resource("skill", "pdf", &resolved),
        PermissionEffect::Allow
    );
    assert_eq!(
        evaluator.evaluate_resource("edit", "src/main.rs", &resolved),
        PermissionEffect::Ask
    );
}

#[test]
fn legacy_skip_confirmation_field_does_not_enable_access_or_auto_approve() {
    let config: ToolPermissionConfig = serde_json::from_value(json!({
        "skip_tool_confirmation": true,
    }))
    .expect("deserialize legacy-shaped permission config");

    assert_eq!(config, ToolPermissionConfig::default());
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
fn permission_request_correlation_fields_use_stable_wire_shape() {
    let request = PermissionRequest {
        request_id: "request-1".to_string(),
        round_id: "round-1".to_string(),
        order: 2,
        tool_call_id: Some("call-1".to_string()),
        project_path: Some("/workspace/project".to_string()),
        project_id: "project-1".to_string(),
        session_id: "session-1".to_string(),
        agent_id: "agentic".to_string(),
        action: "read".to_string(),
        resources: vec!["README.md".to_string()],
        save_resources: Vec::new(),
        source: PermissionRequestSource {
            kind: PermissionRequestSourceKind::ToolCall,
            identity: "Read".to_string(),
        },
        delegation: Some(PermissionDelegationContext {
            parent_session_id: "parent-session-1".to_string(),
            parent_dialog_turn_id: Some("parent-turn-1".to_string()),
            parent_tool_call_id: "parent-task-call-1".to_string(),
            subagent_type: "Explore".to_string(),
        }),
        display_metadata: Map::new(),
    };
    let value = serde_json::to_value(&request).expect("serialize permission request");
    assert_eq!(value["roundId"], "round-1");
    assert_eq!(value["order"], 2);
    assert_eq!(value["toolCallId"], "call-1");
    assert_eq!(value["projectPath"], "/workspace/project");
    assert_eq!(
        value["delegation"],
        json!({
            "parentSessionId": "parent-session-1",
            "parentDialogTurnId": "parent-turn-1",
            "parentToolCallId": "parent-task-call-1",
            "subagentType": "Explore",
        })
    );

    let top_level = PermissionRequest {
        delegation: None,
        ..request.clone()
    };
    let top_level_value =
        serde_json::to_value(top_level).expect("serialize top-level permission request");
    assert!(top_level_value.get("delegation").is_none());

    let partial_delegation = PermissionRequest {
        delegation: Some(PermissionDelegationContext {
            parent_dialog_turn_id: None,
            ..request.delegation.expect("delegation should exist")
        }),
        ..request
    };
    let partial_value =
        serde_json::to_value(partial_delegation).expect("serialize partial permission delegation");
    assert_eq!(
        partial_value["delegation"]["parentSessionId"],
        "parent-session-1"
    );
    assert!(partial_value["delegation"]
        .get("parentDialogTurnId")
        .is_none());
}

#[test]
fn permission_request_events_use_camel_case_fields() {
    assert_eq!(
        serde_json::to_value(PermissionRequestEvent::Replied {
            request_id: "request-1".to_string(),
            reply: PermissionReply::Once,
            source: PermissionReplySource::AutoApprove,
        })
        .expect("serialize replied permission event"),
        json!({
            "event": "replied",
            "requestId": "request-1",
            "reply": { "reply": "once" },
            "source": "auto_approve",
        })
    );
    assert_eq!(
        serde_json::to_value(PermissionRequestEvent::Cancelled {
            request_id: "request-2".to_string(),
            reason: "session closed".to_string(),
        })
        .expect("serialize cancelled permission event"),
        json!({
            "event": "cancelled",
            "requestId": "request-2",
            "reason": "session closed",
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
