use crate::service::config::types::{AgentProfileConfig, GlobalConfig};
use bitfun_runtime_ports::{
    resolve_child_permission_policy, resolve_permission_policy, ChildPermissionPolicyLayers,
    PermissionEffect, PermissionPolicyLayers, PermissionRule, PermissionRuntimeCeiling,
};

pub(crate) fn derive_parent_permission_runtime_ceiling(
    agent_profile: Option<&AgentProfileConfig>,
) -> PermissionRuntimeCeiling {
    let rules = agent_profile
        .into_iter()
        .flat_map(|profile| profile.tool_permission_rules.iter())
        .filter(|rule| {
            rule.effect == PermissionEffect::Deny
                || (rule.action == "external_directory" && rule.effect == PermissionEffect::Ask)
        })
        .cloned()
        .collect();

    PermissionRuntimeCeiling::try_new(rules)
        .expect("parent permission ceiling extraction must exclude allow rules")
}

pub(crate) fn resolve_effective_permission_rules(
    global: &GlobalConfig,
    project_rules: &[PermissionRule],
    agent_profile: Option<&AgentProfileConfig>,
    parent_runtime_ceiling: Option<&PermissionRuntimeCeiling>,
    enforced: &[PermissionRule],
) -> Vec<PermissionRule> {
    let agent_rules = agent_profile
        .map(|profile| profile.tool_permission_rules.as_slice())
        .unwrap_or(&[]);

    match parent_runtime_ceiling {
        Some(parent_runtime_ceiling) => {
            resolve_child_permission_policy(ChildPermissionPolicyLayers {
                product_defaults: &[],
                global: &global.tool_permissions.policy,
                project: project_rules,
                child_agent: agent_rules,
                parent_runtime_ceiling,
                enforced,
            })
        }
        None => resolve_permission_policy(PermissionPolicyLayers {
            product_defaults: &[],
            global: &global.tool_permissions.policy,
            project: project_rules,
            agent: agent_rules,
            enforced,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bitfun_runtime_ports::{PermissionEvaluator, PermissionPolicyPreset};

    fn rule(action: &str, resource: &str, effect: PermissionEffect) -> PermissionRule {
        PermissionRule::new(action, resource, effect)
    }

    #[test]
    fn parent_ceiling_keeps_only_profile_denies_and_external_directory_asks() {
        let profile = AgentProfileConfig {
            tool_permission_rules: vec![
                rule("read", "*", PermissionEffect::Allow),
                rule("bash", "rm *", PermissionEffect::Deny),
                rule("edit", "src/*", PermissionEffect::Ask),
                rule("external_directory", "*", PermissionEffect::Ask),
                rule("external_directory", "C:/blocked", PermissionEffect::Deny),
                rule("external_directory", "C:/trusted", PermissionEffect::Allow),
            ],
            ..AgentProfileConfig::default()
        };

        let ceiling = derive_parent_permission_runtime_ceiling(Some(&profile));

        assert_eq!(
            ceiling.rules(),
            [
                rule("bash", "rm *", PermissionEffect::Deny),
                rule("external_directory", "*", PermissionEffect::Ask),
                rule("external_directory", "C:/blocked", PermissionEffect::Deny),
            ]
        );
        assert!(ceiling
            .rules()
            .iter()
            .all(|rule| rule.effect != PermissionEffect::Allow));
    }

    #[test]
    fn top_level_resolution_keeps_existing_global_project_and_agent_order() {
        let mut global = GlobalConfig::default();
        global.tool_permissions.policy.preset = PermissionPolicyPreset::FullAccess;
        global.tool_permissions.policy.rules = vec![rule("bash", "rm *", PermissionEffect::Ask)];
        let project = vec![rule("edit", "generated/*", PermissionEffect::Deny)];
        let profile = AgentProfileConfig {
            tool_permission_rules: vec![rule(
                "edit",
                "generated/review.md",
                PermissionEffect::Allow,
            )],
            ..AgentProfileConfig::default()
        };

        let resolved =
            resolve_effective_permission_rules(&global, &project, Some(&profile), None, &[]);

        assert_eq!(
            PermissionEvaluator::case_sensitive().evaluate_resource(
                "edit",
                "generated/review.md",
                &resolved,
            ),
            PermissionEffect::Allow
        );
        assert_eq!(
            PermissionEvaluator::case_sensitive().evaluate_resource(
                "edit",
                "generated/api.rs",
                &resolved,
            ),
            PermissionEffect::Deny
        );
    }

    #[test]
    fn child_profile_allow_cannot_loosen_parent_deny_or_external_directory_ask() {
        let mut global = GlobalConfig::default();
        global.tool_permissions.policy.preset = PermissionPolicyPreset::FullAccess;
        let child_profile = AgentProfileConfig {
            tool_permission_rules: vec![
                rule("bash", "rm *", PermissionEffect::Allow),
                rule("external_directory", "*", PermissionEffect::Allow),
            ],
            ..AgentProfileConfig::default()
        };
        let ceiling = PermissionRuntimeCeiling::try_new(vec![
            rule("bash", "rm *", PermissionEffect::Deny),
            rule("external_directory", "*", PermissionEffect::Ask),
        ])
        .expect("test ceiling should be valid");

        let resolved = resolve_effective_permission_rules(
            &global,
            &[],
            Some(&child_profile),
            Some(&ceiling),
            &[],
        );
        let evaluator = PermissionEvaluator::case_sensitive();

        assert_eq!(
            evaluator.evaluate_resource("bash", "rm -rf target", &resolved),
            PermissionEffect::Deny
        );
        assert_eq!(
            evaluator.evaluate_resource("external_directory", "C:/outside", &resolved),
            PermissionEffect::Ask
        );
    }
}
