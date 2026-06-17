use super::catalog::{builtin_skill_spec, BuiltinSkillGroup, BuiltinSkillSpec};
use crate::agents::{resolve_mode_config_profile_id, SHARED_CODING_MODE_CONFIG_PROFILE_ID};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SkillModeId {
    CodingShared,
    Agentic,
    Multitask,
    Cowork,
    Plan,
    Debug,
    Team,
    Claw,
    ComputerUse,
    DeepResearch,
    Other,
}

impl SkillModeId {
    fn parse(mode_id: &str) -> Self {
        match mode_id.trim() {
            SHARED_CODING_MODE_CONFIG_PROFILE_ID => Self::CodingShared,
            "agentic" => Self::Agentic,
            "Multitask" => Self::Multitask,
            "Cowork" => Self::Cowork,
            "Plan" => Self::Plan,
            "debug" => Self::Debug,
            "Team" => Self::Team,
            "Claw" => Self::Claw,
            "ComputerUse" => Self::ComputerUse,
            "DeepResearch" => Self::DeepResearch,
            _ => Self::Other,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PolicyEffect {
    Enable,
    Disable,
}

impl PolicyEffect {
    fn is_enabled(self) -> bool {
        matches!(self, Self::Enable)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SkillSelector {
    Group(BuiltinSkillGroup),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SkillPolicyRule {
    selector: SkillSelector,
    effect: PolicyEffect,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ModeSkillPolicy {
    builtin_default: PolicyEffect,
    rules: &'static [SkillPolicyRule],
}

const DISABLE_OFFICE: SkillPolicyRule = SkillPolicyRule {
    selector: SkillSelector::Group(BuiltinSkillGroup::Office),
    effect: PolicyEffect::Disable,
};

const DISABLE_GSTACK: SkillPolicyRule = SkillPolicyRule {
    selector: SkillSelector::Group(BuiltinSkillGroup::Gstack),
    effect: PolicyEffect::Disable,
};

const ENABLE_OFFICE: SkillPolicyRule = SkillPolicyRule {
    selector: SkillSelector::Group(BuiltinSkillGroup::Office),
    effect: PolicyEffect::Enable,
};

const ENABLE_META: SkillPolicyRule = SkillPolicyRule {
    selector: SkillSelector::Group(BuiltinSkillGroup::Meta),
    effect: PolicyEffect::Enable,
};

const OPEN_META_ONLY_POLICY: ModeSkillPolicy = ModeSkillPolicy {
    builtin_default: PolicyEffect::Disable,
    rules: &[ENABLE_META],
};

const PLAN_POLICY: ModeSkillPolicy = ModeSkillPolicy {
    builtin_default: PolicyEffect::Disable,
    rules: &[],
};

const DEBUG_POLICY: ModeSkillPolicy = PLAN_POLICY;

const AGENTIC_POLICY: ModeSkillPolicy = ModeSkillPolicy {
    builtin_default: PolicyEffect::Enable,
    rules: &[DISABLE_OFFICE, DISABLE_GSTACK],
};

const COWORK_POLICY: ModeSkillPolicy = ModeSkillPolicy {
    builtin_default: PolicyEffect::Disable,
    rules: &[ENABLE_OFFICE, ENABLE_META],
};

const TEAM_POLICY: ModeSkillPolicy = ModeSkillPolicy {
    builtin_default: PolicyEffect::Enable,
    rules: &[DISABLE_OFFICE],
};

fn policy_for_mode(mode_id: &str) -> ModeSkillPolicy {
    let policy_scope = resolve_mode_config_profile_id(mode_id);
    match SkillModeId::parse(policy_scope.as_ref()) {
        SkillModeId::CodingShared => AGENTIC_POLICY,
        SkillModeId::Plan => PLAN_POLICY,
        SkillModeId::Debug => DEBUG_POLICY,
        SkillModeId::Agentic | SkillModeId::Multitask | SkillModeId::Claw => AGENTIC_POLICY,
        SkillModeId::Cowork => COWORK_POLICY,
        SkillModeId::Team => TEAM_POLICY,
        SkillModeId::ComputerUse | SkillModeId::DeepResearch | SkillModeId::Other => {
            OPEN_META_ONLY_POLICY
        }
    }
}

fn selector_matches(selector: SkillSelector, spec: &BuiltinSkillSpec) -> bool {
    match selector {
        SkillSelector::Group(group) => spec.group == group,
    }
}

fn resolve_builtin_default_effect(spec: &BuiltinSkillSpec, mode_id: &str) -> PolicyEffect {
    let policy = policy_for_mode(mode_id);
    let mut current = policy.builtin_default;

    for rule in policy.rules {
        if selector_matches(rule.selector, spec) {
            current = rule.effect;
        }
    }

    current
}

pub fn resolve_builtin_default_enabled(dir_name: &str, mode_id: &str) -> Option<bool> {
    builtin_skill_spec(dir_name)
        .map(|spec| resolve_builtin_default_effect(spec, mode_id).is_enabled())
}
