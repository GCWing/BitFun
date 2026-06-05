//! Built-in skill catalog.
//!
//! This module is the single source of truth for built-in skill identity and
//! grouping metadata. Runtime policy code should depend on this catalog instead
//! of scattering string matches across multiple files.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BuiltinSkillId {
    AgentBrowser,
    Docx,
    FindSkills,
    GstackAutoplan,
    GstackCso,
    GstackDesignConsultation,
    GstackDesignReview,
    GstackDocumentRelease,
    GstackInvestigate,
    GstackOfficeHours,
    GstackPlanCeoReview,
    GstackPlanDesignReview,
    GstackPlanEngReview,
    GstackQa,
    GstackQaOnly,
    GstackRetro,
    GstackReview,
    GstackShip,
    Pdf,
    Pptx,
    WritingSkills,
    Xlsx,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BuiltinSkillGroup {
    Office,
    Meta,
    ComputerUse,
    Gstack,
}

impl BuiltinSkillGroup {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Office => "office",
            Self::Meta => "meta",
            Self::ComputerUse => "computer-use",
            Self::Gstack => "gstack",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuiltinSkillSpec {
    pub id: BuiltinSkillId,
    pub dir_name: &'static str,
    pub group: BuiltinSkillGroup,
}

const BUILTIN_SKILL_SPECS: &[BuiltinSkillSpec] = &[
    BuiltinSkillSpec {
        id: BuiltinSkillId::AgentBrowser,
        dir_name: "agent-browser",
        group: BuiltinSkillGroup::ComputerUse,
    },
    BuiltinSkillSpec {
        id: BuiltinSkillId::Docx,
        dir_name: "docx",
        group: BuiltinSkillGroup::Office,
    },
    BuiltinSkillSpec {
        id: BuiltinSkillId::FindSkills,
        dir_name: "find-skills",
        group: BuiltinSkillGroup::Meta,
    },
    BuiltinSkillSpec {
        id: BuiltinSkillId::GstackAutoplan,
        dir_name: "gstack-autoplan",
        group: BuiltinSkillGroup::Gstack,
    },
    BuiltinSkillSpec {
        id: BuiltinSkillId::GstackCso,
        dir_name: "gstack-cso",
        group: BuiltinSkillGroup::Gstack,
    },
    BuiltinSkillSpec {
        id: BuiltinSkillId::GstackDesignConsultation,
        dir_name: "gstack-design-consultation",
        group: BuiltinSkillGroup::Gstack,
    },
    BuiltinSkillSpec {
        id: BuiltinSkillId::GstackDesignReview,
        dir_name: "gstack-design-review",
        group: BuiltinSkillGroup::Gstack,
    },
    BuiltinSkillSpec {
        id: BuiltinSkillId::GstackDocumentRelease,
        dir_name: "gstack-document-release",
        group: BuiltinSkillGroup::Gstack,
    },
    BuiltinSkillSpec {
        id: BuiltinSkillId::GstackInvestigate,
        dir_name: "gstack-investigate",
        group: BuiltinSkillGroup::Gstack,
    },
    BuiltinSkillSpec {
        id: BuiltinSkillId::GstackOfficeHours,
        dir_name: "gstack-office-hours",
        group: BuiltinSkillGroup::Gstack,
    },
    BuiltinSkillSpec {
        id: BuiltinSkillId::GstackPlanCeoReview,
        dir_name: "gstack-plan-ceo-review",
        group: BuiltinSkillGroup::Gstack,
    },
    BuiltinSkillSpec {
        id: BuiltinSkillId::GstackPlanDesignReview,
        dir_name: "gstack-plan-design-review",
        group: BuiltinSkillGroup::Gstack,
    },
    BuiltinSkillSpec {
        id: BuiltinSkillId::GstackPlanEngReview,
        dir_name: "gstack-plan-eng-review",
        group: BuiltinSkillGroup::Gstack,
    },
    BuiltinSkillSpec {
        id: BuiltinSkillId::GstackQa,
        dir_name: "gstack-qa",
        group: BuiltinSkillGroup::Gstack,
    },
    BuiltinSkillSpec {
        id: BuiltinSkillId::GstackQaOnly,
        dir_name: "gstack-qa-only",
        group: BuiltinSkillGroup::Gstack,
    },
    BuiltinSkillSpec {
        id: BuiltinSkillId::GstackRetro,
        dir_name: "gstack-retro",
        group: BuiltinSkillGroup::Gstack,
    },
    BuiltinSkillSpec {
        id: BuiltinSkillId::GstackReview,
        dir_name: "gstack-review",
        group: BuiltinSkillGroup::Gstack,
    },
    BuiltinSkillSpec {
        id: BuiltinSkillId::GstackShip,
        dir_name: "gstack-ship",
        group: BuiltinSkillGroup::Gstack,
    },
    BuiltinSkillSpec {
        id: BuiltinSkillId::Pdf,
        dir_name: "pdf",
        group: BuiltinSkillGroup::Office,
    },
    BuiltinSkillSpec {
        id: BuiltinSkillId::Pptx,
        dir_name: "pptx",
        group: BuiltinSkillGroup::Office,
    },
    BuiltinSkillSpec {
        id: BuiltinSkillId::WritingSkills,
        dir_name: "writing-skills",
        group: BuiltinSkillGroup::Meta,
    },
    BuiltinSkillSpec {
        id: BuiltinSkillId::Xlsx,
        dir_name: "xlsx",
        group: BuiltinSkillGroup::Office,
    },
];

pub fn builtin_skill_spec(dir_name: &str) -> Option<&'static BuiltinSkillSpec> {
    BUILTIN_SKILL_SPECS
        .iter()
        .find(|spec| spec.dir_name == dir_name)
}

pub fn builtin_skill_group(dir_name: &str) -> Option<BuiltinSkillGroup> {
    builtin_skill_spec(dir_name).map(|spec| spec.group)
}

pub fn builtin_skill_group_key(dir_name: &str) -> Option<&'static str> {
    builtin_skill_group(dir_name).map(BuiltinSkillGroup::as_str)
}

#[cfg(test)]
mod tests {
    use super::{builtin_skill_group, builtin_skill_group_key, BUILTIN_SKILL_SPECS};
    use crate::agentic::tools::implementations::skills::builtin::builtin_skill_dir_names;
    use std::collections::HashSet;

    #[test]
    fn builtin_skill_groups_match_expected_sets() {
        assert_eq!(builtin_skill_group_key("docx"), Some("office"));
        assert_eq!(builtin_skill_group_key("pdf"), Some("office"));
        assert_eq!(builtin_skill_group_key("pptx"), Some("office"));
        assert_eq!(builtin_skill_group_key("xlsx"), Some("office"));
        assert_eq!(builtin_skill_group_key("find-skills"), Some("meta"));
        assert_eq!(builtin_skill_group_key("writing-skills"), Some("meta"));
        assert_eq!(
            builtin_skill_group_key("agent-browser"),
            Some("computer-use")
        );
        assert_eq!(builtin_skill_group_key("gstack-review"), Some("gstack"));
        assert_eq!(builtin_skill_group("unknown-skill"), None);
    }

    #[test]
    fn catalog_covers_all_embedded_builtin_skills() {
        let known: HashSet<&'static str> = BUILTIN_SKILL_SPECS
            .iter()
            .map(|spec| spec.dir_name)
            .collect();

        for dir_name in builtin_skill_dir_names() {
            assert!(
                known.contains(dir_name.as_str()),
                "Missing built-in skill catalog entry for '{}'",
                dir_name
            );
        }
    }
}
