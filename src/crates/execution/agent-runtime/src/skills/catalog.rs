#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum BuiltinSkillGroup {
    Office,
    Meta,
    MiniApp,
    ComputerUse,
    Canvas,
    Gstack,
}

impl BuiltinSkillGroup {
    fn as_str(self) -> &'static str {
        match self {
            Self::Office => "office",
            Self::Meta => "meta",
            Self::MiniApp => "miniapp",
            Self::ComputerUse => "computer-use",
            Self::Canvas => "canvas",
            Self::Gstack => "gstack",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct BuiltinSkillSpec {
    pub(super) dir_name: &'static str,
    pub(super) group: BuiltinSkillGroup,
}

const BUILTIN_SKILL_SPECS: &[BuiltinSkillSpec] = &[
    BuiltinSkillSpec {
        dir_name: "agent-browser",
        group: BuiltinSkillGroup::ComputerUse,
    },
    BuiltinSkillSpec {
        dir_name: "docs-canvas",
        group: BuiltinSkillGroup::Canvas,
    },
    BuiltinSkillSpec {
        dir_name: "bitfun-canvas",
        group: BuiltinSkillGroup::Canvas,
    },
    BuiltinSkillSpec {
        dir_name: "docx",
        group: BuiltinSkillGroup::Office,
    },
    BuiltinSkillSpec {
        dir_name: "find-skills",
        group: BuiltinSkillGroup::Meta,
    },
    BuiltinSkillSpec {
        dir_name: "miniapp-dev",
        group: BuiltinSkillGroup::MiniApp,
    },
    BuiltinSkillSpec {
        dir_name: "gstack-autoplan",
        group: BuiltinSkillGroup::Gstack,
    },
    BuiltinSkillSpec {
        dir_name: "gstack-cso",
        group: BuiltinSkillGroup::Gstack,
    },
    BuiltinSkillSpec {
        dir_name: "gstack-design-consultation",
        group: BuiltinSkillGroup::Gstack,
    },
    BuiltinSkillSpec {
        dir_name: "gstack-design-review",
        group: BuiltinSkillGroup::Gstack,
    },
    BuiltinSkillSpec {
        dir_name: "gstack-document-release",
        group: BuiltinSkillGroup::Gstack,
    },
    BuiltinSkillSpec {
        dir_name: "gstack-investigate",
        group: BuiltinSkillGroup::Gstack,
    },
    BuiltinSkillSpec {
        dir_name: "gstack-office-hours",
        group: BuiltinSkillGroup::Gstack,
    },
    BuiltinSkillSpec {
        dir_name: "gstack-plan-ceo-review",
        group: BuiltinSkillGroup::Gstack,
    },
    BuiltinSkillSpec {
        dir_name: "gstack-plan-design-review",
        group: BuiltinSkillGroup::Gstack,
    },
    BuiltinSkillSpec {
        dir_name: "gstack-plan-eng-review",
        group: BuiltinSkillGroup::Gstack,
    },
    BuiltinSkillSpec {
        dir_name: "gstack-qa",
        group: BuiltinSkillGroup::Gstack,
    },
    BuiltinSkillSpec {
        dir_name: "gstack-qa-only",
        group: BuiltinSkillGroup::Gstack,
    },
    BuiltinSkillSpec {
        dir_name: "gstack-retro",
        group: BuiltinSkillGroup::Gstack,
    },
    BuiltinSkillSpec {
        dir_name: "gstack-review",
        group: BuiltinSkillGroup::Gstack,
    },
    BuiltinSkillSpec {
        dir_name: "gstack-ship",
        group: BuiltinSkillGroup::Gstack,
    },
    BuiltinSkillSpec {
        dir_name: "pdf",
        group: BuiltinSkillGroup::Office,
    },
    BuiltinSkillSpec {
        dir_name: "ppt-design",
        group: BuiltinSkillGroup::Office,
    },
    BuiltinSkillSpec {
        dir_name: "pptx",
        group: BuiltinSkillGroup::Office,
    },
    BuiltinSkillSpec {
        dir_name: "pr-review-canvas",
        group: BuiltinSkillGroup::Canvas,
    },
    BuiltinSkillSpec {
        dir_name: "writing-skills",
        group: BuiltinSkillGroup::Meta,
    },
    BuiltinSkillSpec {
        dir_name: "xlsx",
        group: BuiltinSkillGroup::Office,
    },
];

pub(super) fn builtin_skill_spec(dir_name: &str) -> Option<&'static BuiltinSkillSpec> {
    BUILTIN_SKILL_SPECS
        .iter()
        .find(|spec| spec.dir_name == dir_name)
}

fn builtin_skill_group(dir_name: &str) -> Option<BuiltinSkillGroup> {
    builtin_skill_spec(dir_name).map(|spec| spec.group)
}

pub fn builtin_skill_group_key(dir_name: &str) -> Option<&'static str> {
    builtin_skill_group(dir_name).map(BuiltinSkillGroup::as_str)
}
