use super::catalog::{builtin_skill_spec, BuiltinSkillGroup, BuiltinSkillSpec};
use crate::agents::{resolve_mode_config_profile_id, SHARED_CODING_MODE_CONFIG_PROFILE_ID};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SkillModeId {
    CodingShared,
    Cowork,
    Claw,
    Creative,
    ComputerUse,
    DeepResearch,
    Other,
}

impl SkillModeId {
    fn parse(mode_id: &str) -> Self {
        match mode_id.trim() {
            SHARED_CODING_MODE_CONFIG_PROFILE_ID => Self::CodingShared,
            "Cowork" => Self::Cowork,
            "Claw" => Self::Claw,
            "Creative" => Self::Creative,
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

const DISABLE_MINIAPP: SkillPolicyRule = SkillPolicyRule {
    selector: SkillSelector::Group(BuiltinSkillGroup::MiniApp),
    effect: PolicyEffect::Disable,
};

const DISABLE_CREATION: SkillPolicyRule = SkillPolicyRule {
    selector: SkillSelector::Group(BuiltinSkillGroup::Creation),
    effect: PolicyEffect::Disable,
};

const DISABLE_DEBUGGING: SkillPolicyRule = SkillPolicyRule {
    selector: SkillSelector::Group(BuiltinSkillGroup::Debugging),
    effect: PolicyEffect::Disable,
};

// ControlHub's browser domain is the single default browser-automation path.
// The computer-use skill group (agent-browser) stays opt-in in every mode so the
// model never sees two parallel browser stacks; users can still enable it via
// mode skill overrides or invoke it explicitly.
const DISABLE_COMPUTER_USE: SkillPolicyRule = SkillPolicyRule {
    selector: SkillSelector::Group(BuiltinSkillGroup::ComputerUse),
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

const ENABLE_COORDINATION: SkillPolicyRule = SkillPolicyRule {
    selector: SkillSelector::Group(BuiltinSkillGroup::Coordination),
    effect: PolicyEffect::Enable,
};

const ENABLE_PLANNING: SkillPolicyRule = SkillPolicyRule {
    selector: SkillSelector::Group(BuiltinSkillGroup::Planning),
    effect: PolicyEffect::Enable,
};

const OPEN_META_ONLY_POLICY: ModeSkillPolicy = ModeSkillPolicy {
    builtin_default: PolicyEffect::Disable,
    rules: &[ENABLE_META],
};

const AGENTIC_POLICY: ModeSkillPolicy = ModeSkillPolicy {
    builtin_default: PolicyEffect::Enable,
    rules: &[
        DISABLE_OFFICE,
        DISABLE_GSTACK,
        DISABLE_COMPUTER_USE,
        DISABLE_MINIAPP,
        DISABLE_CREATION,
    ],
};

const CLAW_POLICY: ModeSkillPolicy = ModeSkillPolicy {
    builtin_default: PolicyEffect::Enable,
    rules: &[
        DISABLE_OFFICE,
        DISABLE_GSTACK,
        DISABLE_COMPUTER_USE,
        DISABLE_MINIAPP,
        DISABLE_CREATION,
        DISABLE_DEBUGGING,
    ],
};

const CREATIVE_POLICY: ModeSkillPolicy = ModeSkillPolicy {
    builtin_default: PolicyEffect::Enable,
    rules: &[
        DISABLE_OFFICE,
        DISABLE_GSTACK,
        DISABLE_COMPUTER_USE,
        DISABLE_DEBUGGING,
    ],
};

const COWORK_POLICY: ModeSkillPolicy = ModeSkillPolicy {
    builtin_default: PolicyEffect::Disable,
    rules: &[
        ENABLE_OFFICE,
        ENABLE_META,
        ENABLE_COORDINATION,
        ENABLE_PLANNING,
    ],
};

const DEEP_RESEARCH_POLICY: ModeSkillPolicy = ModeSkillPolicy {
    builtin_default: PolicyEffect::Disable,
    rules: &[ENABLE_META, ENABLE_COORDINATION],
};

fn policy_for_mode(mode_id: &str) -> ModeSkillPolicy {
    let policy_scope = resolve_mode_config_profile_id(mode_id);
    match SkillModeId::parse(policy_scope.as_ref()) {
        SkillModeId::CodingShared => AGENTIC_POLICY,
        SkillModeId::Claw => CLAW_POLICY,
        SkillModeId::Creative => CREATIVE_POLICY,
        SkillModeId::Cowork => COWORK_POLICY,
        SkillModeId::DeepResearch => DEEP_RESEARCH_POLICY,
        SkillModeId::ComputerUse | SkillModeId::Other => OPEN_META_ONLY_POLICY,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::{SHARED_CODING_MODE_CONFIG_PROFILE_ID, SHARED_CODING_MODE_IDS};
    use crate::skills::catalog::BUILTIN_SKILL_SPECS;

    #[test]
    fn agent_browser_defaults_off_in_every_mode() {
        for mode_id in [
            "agentic",
            "coding_shared",
            "Claw",
            "Creative",
            "Cowork",
            "ComputerUse",
            "DeepResearch",
            "SomeUnknownMode",
        ] {
            assert_eq!(
                resolve_builtin_default_enabled("agent-browser", mode_id),
                Some(false),
                "agent-browser must stay opt-in for mode {mode_id}: ControlHub's browser domain is the default browser path"
            );
        }
    }

    #[test]
    fn shared_coding_modes_use_their_profile_builtin_skill_defaults() {
        for spec in BUILTIN_SKILL_SPECS {
            let expected = resolve_builtin_default_enabled(
                spec.dir_name,
                SHARED_CODING_MODE_CONFIG_PROFILE_ID,
            );
            for mode_id in SHARED_CODING_MODE_IDS {
                assert_eq!(
                    resolve_builtin_default_enabled(spec.dir_name, mode_id),
                    expected,
                    "builtin skill {} differs for shared coding mode {}",
                    spec.dir_name,
                    mode_id
                );
            }
        }
    }

    #[test]
    fn debug_skill_defaults_on_only_in_agentic() {
        for mode_id in ["agentic", "coding_shared"] {
            assert_eq!(
                resolve_builtin_default_enabled("debug", mode_id),
                Some(true),
                "debug should default on for agentic profile identity {mode_id}"
            );
        }

        for mode_id in [
            "Claw",
            "Creative",
            "Cowork",
            "ComputerUse",
            "DeepResearch",
            "Team",
            "SomeUnknownMode",
        ] {
            assert_eq!(
                resolve_builtin_default_enabled("debug", mode_id),
                Some(false),
                "debug should default off outside agentic: {mode_id}"
            );
        }
    }

    #[test]
    fn multitask_skill_replaces_multitask_mode_in_coding_workflows() {
        for mode_id in [
            "agentic",
            "coding_shared",
            "Claw",
            "Cowork",
            "Creative",
            "DeepResearch",
        ] {
            assert_eq!(
                resolve_builtin_default_enabled("multitask", mode_id),
                Some(true),
                "multitask should be available in {mode_id}"
            );
        }

        for mode_id in ["ComputerUse", "SomeUnknownMode"] {
            assert_eq!(
                resolve_builtin_default_enabled("multitask", mode_id),
                Some(false),
                "multitask should remain opt-in in {mode_id}"
            );
        }
    }

    #[test]
    fn plan_skill_replaces_plan_mode_in_writable_skill_workflows() {
        for mode_id in ["agentic", "coding_shared", "Claw", "Cowork", "Creative"] {
            assert_eq!(
                resolve_builtin_default_enabled("plan", mode_id),
                Some(true),
                "plan should be available in {mode_id}"
            );
        }

        for mode_id in ["ComputerUse", "DeepResearch", "SomeUnknownMode"] {
            assert_eq!(
                resolve_builtin_default_enabled("plan", mode_id),
                Some(false),
                "plan should remain opt-in or unavailable in {mode_id}"
            );
        }
    }

    #[test]
    fn product_creation_skills_default_only_in_creative_mode() {
        for skill in ["miniapp-dev", "bitfun-frontend-dev"] {
            for mode_id in [
                "agentic",
                "coding_shared",
                "Claw",
                "Creative",
                "Cowork",
                "ComputerUse",
                "DeepResearch",
                "SomeUnknownMode",
            ] {
                assert_eq!(
                    resolve_builtin_default_enabled(skill, mode_id),
                    Some(mode_id == "Creative"),
                    "creation skill {skill} has unexpected default exposure in {mode_id}"
                );
            }
        }
    }
}
