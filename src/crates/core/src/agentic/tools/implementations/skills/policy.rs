//! Mode-aware built-in skill policy.
//!
//! The policy layer answers a narrow question: given a built-in skill and a
//! mode identifier, should that skill be enabled by default before any user or
//! project override is applied?

use super::catalog::{builtin_skill_spec, BuiltinSkillGroup, BuiltinSkillId, BuiltinSkillSpec};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkillModeId {
    Agentic,
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
    pub fn parse(mode_id: &str) -> Self {
        match mode_id.trim() {
            "agentic" => Self::Agentic,
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
pub enum PolicyEffect {
    Enable,
    Disable,
}

impl PolicyEffect {
    pub fn is_enabled(self) -> bool {
        matches!(self, Self::Enable)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkillSelector {
    Builtin(BuiltinSkillId),
    Group(BuiltinSkillGroup),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SkillPolicyRule {
    pub selector: SkillSelector,
    pub effect: PolicyEffect,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModeSkillPolicy {
    pub builtin_default: PolicyEffect,
    pub rules: &'static [SkillPolicyRule],
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

// Open-ended modes should only surface the lightweight metadata helpers by
// default. The rest of the built-ins remain opt-in.
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

// Team mode keeps the broad built-in toolkit except for office helpers.
// Office skills remain exclusive to Cowork's default profile so document
// handling does not show up by default in other modes.
const TEAM_POLICY: ModeSkillPolicy = ModeSkillPolicy {
    builtin_default: PolicyEffect::Enable,
    rules: &[DISABLE_OFFICE],
};

pub fn policy_for_mode(mode_id: &str) -> ModeSkillPolicy {
    match SkillModeId::parse(mode_id) {
        SkillModeId::Plan => PLAN_POLICY,
        SkillModeId::Debug => DEBUG_POLICY,
        SkillModeId::Agentic | SkillModeId::Claw => AGENTIC_POLICY,
        SkillModeId::Cowork => COWORK_POLICY,
        SkillModeId::Team => TEAM_POLICY,
        SkillModeId::ComputerUse | SkillModeId::DeepResearch | SkillModeId::Other => {
            OPEN_META_ONLY_POLICY
        }
    }
}

fn selector_matches(selector: SkillSelector, spec: &BuiltinSkillSpec) -> bool {
    match selector {
        SkillSelector::Builtin(skill_id) => spec.id == skill_id,
        SkillSelector::Group(group) => spec.group == group,
    }
}

pub fn resolve_builtin_default_effect(spec: &BuiltinSkillSpec, mode_id: &str) -> PolicyEffect {
    let policy = policy_for_mode(mode_id);
    let mut current = policy.builtin_default;

    // Rules are applied in declaration order. Later rules can intentionally
    // override broader earlier rules, which keeps profile definitions explicit
    // without introducing another priority system.
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

#[cfg(test)]
mod tests {
    use super::{resolve_builtin_default_enabled, PolicyEffect, SkillModeId};

    #[test]
    fn builtin_defaults_follow_mode_policies() {
        assert_eq!(SkillModeId::parse("agentic"), SkillModeId::Agentic);
        assert_eq!(SkillModeId::parse("debug"), SkillModeId::Debug);
        assert_eq!(SkillModeId::parse("something-else"), SkillModeId::Other);

        assert_eq!(
            resolve_builtin_default_enabled("pdf", "agentic"),
            Some(false)
        );
        assert_eq!(
            resolve_builtin_default_enabled("agent-browser", "agentic"),
            Some(true)
        );
        assert_eq!(resolve_builtin_default_enabled("pdf", "Cowork"), Some(true));
        assert_eq!(
            resolve_builtin_default_enabled("agent-browser", "Cowork"),
            Some(false)
        );
        assert_eq!(
            resolve_builtin_default_enabled("gstack-review", "Team"),
            Some(true)
        );
        assert_eq!(resolve_builtin_default_enabled("pdf", "Team"), Some(false));
        assert_eq!(
            resolve_builtin_default_enabled("find-skills", "DeepResearch"),
            Some(true)
        );
        assert_eq!(
            resolve_builtin_default_enabled("pdf", "DeepResearch"),
            Some(false)
        );
        assert_eq!(
            resolve_builtin_default_enabled("agent-browser", "Claw"),
            Some(true)
        );
        assert_eq!(resolve_builtin_default_enabled("pdf", "Claw"), Some(false));
        assert_eq!(resolve_builtin_default_enabled("pdf", "Other"), Some(false));
    }

    #[test]
    fn unknown_builtins_return_none() {
        assert_eq!(resolve_builtin_default_enabled("not-real", "agentic"), None);
        assert!(PolicyEffect::Enable.is_enabled());
        assert!(!PolicyEffect::Disable.is_enabled());
    }
}
