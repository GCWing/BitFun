use crate::service::config::global::GlobalConfigManager;
use crate::service::config::types::{AgentProfileConfig, GlobalConfig};
use crate::util::errors::BitFunResult;
use bitfun_runtime_ports::{
    resolve_child_permission_policy, resolve_permission_policy, ChildPermissionPolicyLayers,
    PermissionConstraintLayer, PermissionEffect, PermissionPolicyLayers, PermissionRule,
    PermissionRuntimeCeiling, ResolvedPermissionPolicy,
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

pub(crate) async fn load_parent_permission_runtime_ceiling(
    agent_type: Option<&str>,
) -> BitFunResult<PermissionRuntimeCeiling> {
    let service = GlobalConfigManager::get_service().await?;
    let global: GlobalConfig = service.get_config(None).await?;
    let profile = agent_type.and_then(|agent_type| {
        let profile_id = crate::agentic::agents::resolve_mode_config_profile_id(agent_type);
        global.ai.agent_profiles.get(profile_id.as_ref())
    });
    Ok(derive_parent_permission_runtime_ceiling(profile))
}

pub(crate) fn resolve_effective_permission_policy(
    global: &GlobalConfig,
    project_rules: &[PermissionRule],
    agent_profile: Option<&AgentProfileConfig>,
    agent_definition_constraints: Option<&PermissionConstraintLayer>,
    parent_runtime_ceiling: Option<&PermissionRuntimeCeiling>,
    enforced: &[PermissionRule],
) -> ResolvedPermissionPolicy {
    let agent_rules = agent_profile
        .map(|profile| profile.tool_permission_rules.as_slice())
        .unwrap_or(&[]);

    let resolved = match parent_runtime_ceiling {
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
    };
    let Some(agent_definition_constraints) =
        agent_definition_constraints.filter(|constraints| !constraints.is_empty())
    else {
        return resolved;
    };
    let (rules, mut constraint_layers) = resolved.into_parts();
    constraint_layers.insert(0, agent_definition_constraints.clone());
    ResolvedPermissionPolicy::new(rules, constraint_layers)
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
            resolve_effective_permission_policy(&global, &project, Some(&profile), None, None, &[]);

        assert_eq!(
            PermissionEvaluator::case_sensitive().evaluate_policy_resource(
                "edit",
                "generated/review.md",
                &resolved,
            ),
            PermissionEffect::Allow
        );
        assert_eq!(
            PermissionEvaluator::case_sensitive().evaluate_policy_resource(
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

        let resolved = resolve_effective_permission_policy(
            &global,
            &[],
            Some(&child_profile),
            None,
            Some(&ceiling),
            &[],
        );
        let evaluator = PermissionEvaluator::case_sensitive();

        assert_eq!(
            evaluator.evaluate_policy_resource("bash", "rm -rf target", &resolved),
            PermissionEffect::Deny
        );
        assert_eq!(
            evaluator.evaluate_policy_resource("external_directory", "C:/outside", &resolved),
            PermissionEffect::Ask
        );
    }
}
