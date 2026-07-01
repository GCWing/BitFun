//! Concrete tool-pack owner crate.
//!
//! The feature scaffold is intentionally behavior-neutral until the core
//! `ToolUseContext` and registry boundaries are split into portable ports.

use std::collections::HashSet;
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ToolPackFeatureGroup {
    Basic,
    Git,
    Mcp,
    BrowserWeb,
    ComputerUse,
    ImageAnalysis,
    MiniApp,
    Canvas,
    AgentControl,
}

impl ToolPackFeatureGroup {
    pub const fn id(self) -> &'static str {
        match self {
            Self::Basic => "basic",
            Self::Git => "git",
            Self::Mcp => "mcp",
            Self::BrowserWeb => "browser-web",
            Self::ComputerUse => "computer-use",
            Self::ImageAnalysis => "image-analysis",
            Self::MiniApp => "miniapp",
            Self::Canvas => "canvas",
            Self::AgentControl => "agent-control",
        }
    }
}

pub const ALL_FEATURE_GROUPS: &[ToolPackFeatureGroup] = &[
    ToolPackFeatureGroup::Basic,
    ToolPackFeatureGroup::Git,
    ToolPackFeatureGroup::Mcp,
    ToolPackFeatureGroup::BrowserWeb,
    ToolPackFeatureGroup::ComputerUse,
    ToolPackFeatureGroup::ImageAnalysis,
    ToolPackFeatureGroup::MiniApp,
    ToolPackFeatureGroup::Canvas,
    ToolPackFeatureGroup::AgentControl,
];

pub fn all_feature_groups() -> &'static [ToolPackFeatureGroup] {
    ALL_FEATURE_GROUPS
}

pub fn enabled_feature_groups() -> Vec<ToolPackFeatureGroup> {
    [
        (cfg!(feature = "basic"), ToolPackFeatureGroup::Basic),
        (cfg!(feature = "git"), ToolPackFeatureGroup::Git),
        (cfg!(feature = "mcp"), ToolPackFeatureGroup::Mcp),
        (
            cfg!(feature = "browser-web"),
            ToolPackFeatureGroup::BrowserWeb,
        ),
        (
            cfg!(feature = "computer-use"),
            ToolPackFeatureGroup::ComputerUse,
        ),
        (
            cfg!(feature = "image-analysis"),
            ToolPackFeatureGroup::ImageAnalysis,
        ),
        (cfg!(feature = "miniapp"), ToolPackFeatureGroup::MiniApp),
        (cfg!(feature = "canvas"), ToolPackFeatureGroup::Canvas),
        (
            cfg!(feature = "agent-control"),
            ToolPackFeatureGroup::AgentControl,
        ),
    ]
    .into_iter()
    .filter_map(|(enabled, group)| enabled.then_some(group))
    .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ToolProviderGroupPlan {
    provider_id: &'static str,
    feature_groups: &'static [ToolPackFeatureGroup],
    tool_names: &'static [&'static str],
}

impl ToolProviderGroupPlan {
    pub const fn provider_id(self) -> &'static str {
        self.provider_id
    }

    pub const fn feature_groups(self) -> &'static [ToolPackFeatureGroup] {
        self.feature_groups
    }

    pub const fn tool_names(self) -> &'static [&'static str] {
        self.tool_names
    }
}

const CORE_BASIC_FEATURE_GROUPS: &[ToolPackFeatureGroup] = &[ToolPackFeatureGroup::Basic];
const CORE_AGENT_FEATURE_GROUPS: &[ToolPackFeatureGroup] = &[ToolPackFeatureGroup::AgentControl];
const CORE_CANVAS_FEATURE_GROUPS: &[ToolPackFeatureGroup] = &[ToolPackFeatureGroup::Canvas];
const CORE_SESSION_FEATURE_GROUPS: &[ToolPackFeatureGroup] = &[ToolPackFeatureGroup::AgentControl];
const CORE_INTEGRATION_FEATURE_GROUPS: &[ToolPackFeatureGroup] = &[
    ToolPackFeatureGroup::BrowserWeb,
    ToolPackFeatureGroup::Mcp,
    ToolPackFeatureGroup::Git,
    ToolPackFeatureGroup::MiniApp,
    ToolPackFeatureGroup::ComputerUse,
    ToolPackFeatureGroup::ImageAnalysis,
    ToolPackFeatureGroup::AgentControl,
];

const PRODUCT_TOOL_PROVIDER_GROUP_PLAN: &[ToolProviderGroupPlan] = &[
    ToolProviderGroupPlan {
        provider_id: "core.basic",
        feature_groups: CORE_BASIC_FEATURE_GROUPS,
        tool_names: &[
            "LS",
            "Read",
            "view_image",
            "analyze_image",
            "Glob",
            "Grep",
            "Write",
            "Edit",
            "Delete",
            "ExecCommand",
            "WriteStdin",
            "ExecControl",
            "GetTime",
        ],
    },
    ToolProviderGroupPlan {
        provider_id: "core.agent",
        feature_groups: CORE_AGENT_FEATURE_GROUPS,
        tool_names: &[
            "Task",
            "LaunchReviewAgent",
            "Skill",
            "AskUserQuestion",
            "TodoWrite",
            "get_goal",
            "create_goal",
            "update_goal",
            "CreatePlan",
            "submit_code_review",
            "GetToolSpec",
            "GetFileDiff",
        ],
    },
    ToolProviderGroupPlan {
        provider_id: "core.canvas",
        feature_groups: CORE_CANVAS_FEATURE_GROUPS,
        tool_names: &["CreateCanvas", "ReadCanvas", "UpdateCanvas", "PatchCanvas"],
    },
    ToolProviderGroupPlan {
        provider_id: "core.session",
        feature_groups: CORE_SESSION_FEATURE_GROUPS,
        tool_names: &["SessionControl", "SessionMessage", "SessionHistory", "Cron"],
    },
    ToolProviderGroupPlan {
        provider_id: "core.integration",
        feature_groups: CORE_INTEGRATION_FEATURE_GROUPS,
        tool_names: &[
            "WebSearch",
            "WebFetch",
            "ListMCPResources",
            "ReadMCPResource",
            "ListMCPPrompts",
            "GetMCPPrompt",
            "GenerativeUI",
            "Git",
            "ReviewPlatform",
            "InitMiniApp",
            "ControlHub",
            "ComputerUse",
            "Playbook",
        ],
    },
];

pub fn product_tool_provider_group_plan() -> &'static [ToolProviderGroupPlan] {
    PRODUCT_TOOL_PROVIDER_GROUP_PLAN
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolProviderGroupPlanSelectionError {
    UnknownToolProviderGroup { provider_id: &'static str },
}

impl fmt::Display for ToolProviderGroupPlanSelectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownToolProviderGroup { provider_id } => {
                write!(formatter, "unknown tool provider group {provider_id}")
            }
        }
    }
}

impl std::error::Error for ToolProviderGroupPlanSelectionError {}

pub fn try_product_tool_provider_group_plan_for_ids(
    provider_ids: &[&'static str],
) -> Result<Vec<ToolProviderGroupPlan>, ToolProviderGroupPlanSelectionError> {
    let requested_provider_ids = provider_ids.iter().copied().collect::<HashSet<_>>();
    let mut found_provider_ids = HashSet::new();
    let mut plan = Vec::new();

    for group_plan in product_tool_provider_group_plan() {
        if requested_provider_ids.contains(group_plan.provider_id()) {
            found_provider_ids.insert(group_plan.provider_id());
            plan.push(*group_plan);
        }
    }

    for provider_id in provider_ids {
        if !found_provider_ids.contains(provider_id) {
            return Err(
                ToolProviderGroupPlanSelectionError::UnknownToolProviderGroup { provider_id },
            );
        }
    }

    Ok(plan)
}

#[cfg(test)]
mod tests {
    use super::{
        all_feature_groups, enabled_feature_groups, product_tool_provider_group_plan,
        try_product_tool_provider_group_plan_for_ids, ToolPackFeatureGroup,
        ToolProviderGroupPlanSelectionError,
    };

    #[test]
    fn all_feature_groups_cover_planned_tool_pack_scaffold() {
        let feature_ids = all_feature_groups()
            .iter()
            .map(|group| group.id())
            .collect::<Vec<_>>();

        assert_eq!(
            feature_ids,
            vec![
                "basic",
                "git",
                "mcp",
                "browser-web",
                "computer-use",
                "image-analysis",
                "miniapp",
                "canvas",
                "agent-control"
            ]
        );
    }

    #[test]
    fn enabled_feature_groups_reflect_compile_time_features() {
        let groups = enabled_feature_groups();

        assert_eq!(
            groups.contains(&ToolPackFeatureGroup::Basic),
            cfg!(feature = "basic")
        );
        assert_eq!(
            groups.contains(&ToolPackFeatureGroup::Git),
            cfg!(feature = "git")
        );
        assert_eq!(
            groups.contains(&ToolPackFeatureGroup::Mcp),
            cfg!(feature = "mcp")
        );
        assert_eq!(
            groups.contains(&ToolPackFeatureGroup::BrowserWeb),
            cfg!(feature = "browser-web")
        );
        assert_eq!(
            groups.contains(&ToolPackFeatureGroup::ComputerUse),
            cfg!(feature = "computer-use")
        );
        assert_eq!(
            groups.contains(&ToolPackFeatureGroup::ImageAnalysis),
            cfg!(feature = "image-analysis")
        );
        assert_eq!(
            groups.contains(&ToolPackFeatureGroup::MiniApp),
            cfg!(feature = "miniapp")
        );
        assert_eq!(
            groups.contains(&ToolPackFeatureGroup::Canvas),
            cfg!(feature = "canvas")
        );
        assert_eq!(
            groups.contains(&ToolPackFeatureGroup::AgentControl),
            cfg!(feature = "agent-control")
        );
    }

    #[test]
    fn feature_group_ids_match_cargo_feature_names() {
        assert_eq!(ToolPackFeatureGroup::Basic.id(), "basic");
        assert_eq!(ToolPackFeatureGroup::Git.id(), "git");
        assert_eq!(ToolPackFeatureGroup::Mcp.id(), "mcp");
        assert_eq!(ToolPackFeatureGroup::BrowserWeb.id(), "browser-web");
        assert_eq!(ToolPackFeatureGroup::ComputerUse.id(), "computer-use");
        assert_eq!(ToolPackFeatureGroup::ImageAnalysis.id(), "image-analysis");
        assert_eq!(ToolPackFeatureGroup::MiniApp.id(), "miniapp");
        assert_eq!(ToolPackFeatureGroup::Canvas.id(), "canvas");
        assert_eq!(ToolPackFeatureGroup::AgentControl.id(), "agent-control");
    }

    #[test]
    fn product_provider_group_plan_preserves_core_runtime_order() {
        let provider_ids = product_tool_provider_group_plan()
            .iter()
            .map(|group| group.provider_id())
            .collect::<Vec<_>>();

        assert_eq!(
            provider_ids,
            vec![
                "core.basic",
                "core.agent",
                "core.canvas",
                "core.session",
                "core.integration"
            ]
        );
    }

    #[test]
    fn product_provider_group_plan_preserves_builtin_tool_order() {
        let tool_names = product_tool_provider_group_plan()
            .iter()
            .flat_map(|group| group.tool_names().iter().copied())
            .collect::<Vec<_>>();

        assert_eq!(
            tool_names,
            vec![
                "LS",
                "Read",
                "view_image",
                "analyze_image",
                "Glob",
                "Grep",
                "Write",
                "Edit",
                "Delete",
                "ExecCommand",
                "WriteStdin",
                "ExecControl",
                "GetTime",
                "Task",
                "LaunchReviewAgent",
                "Skill",
                "AskUserQuestion",
                "TodoWrite",
                "get_goal",
                "create_goal",
                "update_goal",
                "CreatePlan",
                "submit_code_review",
                "GetToolSpec",
                "GetFileDiff",
                "CreateCanvas",
                "ReadCanvas",
                "UpdateCanvas",
                "PatchCanvas",
                "SessionControl",
                "SessionMessage",
                "SessionHistory",
                "Cron",
                "WebSearch",
                "WebFetch",
                "ListMCPResources",
                "ReadMCPResource",
                "ListMCPPrompts",
                "GetMCPPrompt",
                "GenerativeUI",
                "Git",
                "ReviewPlatform",
                "InitMiniApp",
                "ControlHub",
                "ComputerUse",
                "Playbook",
            ]
        );
    }

    #[test]
    fn product_provider_group_plan_preserves_feature_group_mapping() {
        let feature_groups = product_tool_provider_group_plan()
            .iter()
            .map(|group| {
                (
                    group.provider_id(),
                    group
                        .feature_groups()
                        .iter()
                        .map(|feature_group| feature_group.id())
                        .collect::<Vec<_>>(),
                )
            })
            .collect::<Vec<_>>();

        assert_eq!(
            feature_groups,
            vec![
                ("core.basic", vec!["basic"]),
                ("core.agent", vec!["agent-control"]),
                ("core.canvas", vec!["canvas"]),
                ("core.session", vec!["agent-control"]),
                (
                    "core.integration",
                    vec![
                        "browser-web",
                        "mcp",
                        "git",
                        "miniapp",
                        "computer-use",
                        "image-analysis",
                        "agent-control",
                    ]
                ),
            ]
        );
    }

    #[test]
    fn product_provider_group_plan_selector_preserves_product_plan_order_for_requested_ids() {
        let plan =
            try_product_tool_provider_group_plan_for_ids(&["core.integration", "core.basic"])
                .expect("known provider groups should select");

        let provider_ids = plan
            .iter()
            .map(|group| group.provider_id())
            .collect::<Vec<_>>();

        assert_eq!(provider_ids, vec!["core.basic", "core.integration"]);
    }

    #[test]
    fn product_provider_group_plan_selector_rejects_unknown_provider_ids() {
        let error = try_product_tool_provider_group_plan_for_ids(&["core.basic", "core.missing"])
            .expect_err("unknown provider ids must not be silently ignored");

        assert_eq!(
            error,
            ToolProviderGroupPlanSelectionError::UnknownToolProviderGroup {
                provider_id: "core.missing"
            }
        );
    }
}
