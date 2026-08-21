//! Concrete tool-pack owner crate.
//!
//! This crate owns compile-time availability facts and the stable product
//! provider plan. Concrete implementations, registry ordering, permissions,
//! and runtime materialization remain in Core.

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

pub fn tool_feature_group(tool_name: &str) -> Option<ToolPackFeatureGroup> {
    match tool_name {
        "LS"
        | "Read"
        | "Glob"
        | "Grep"
        | "Write"
        | "Edit"
        | "Delete"
        | "ExecCommand"
        | "WriteStdin"
        | "ExecControl"
        | "GetTime"
        | "ListModels"
        | "WorkspaceScan"
        | "KnowledgeBaseSearch" => Some(ToolPackFeatureGroup::Basic),
        "Git" | "Worktree" | "ReviewPlatform" | "GetFileDiff" => Some(ToolPackFeatureGroup::Git),
        "ListMCPResources" | "ReadMCPResource" | "ListMCPPrompts" | "GetMCPPrompt" => {
            Some(ToolPackFeatureGroup::Mcp)
        }
        "WebSearch" | "WebFetch" | "ControlHub" => Some(ToolPackFeatureGroup::BrowserWeb),
        "ComputerUse" => Some(ToolPackFeatureGroup::ComputerUse),
        "view_image" | "analyze_image" => Some(ToolPackFeatureGroup::ImageAnalysis),
        "GenerativeUI" | "InitMiniApp" | "FinalizeMiniApp" | "PublishMiniApp"
        | "PublishAppearance" | "PageDeploy" | "PagePublish" | "Playbook" => {
            Some(ToolPackFeatureGroup::MiniApp)
        }
        "CreateCanvas" | "ReadCanvas" | "UpdateCanvas" | "PatchCanvas" => {
            Some(ToolPackFeatureGroup::Canvas)
        }
        "Task"
        | "AgentWait"
        | "LaunchReviewAgent"
        | "Skill"
        | "AskUserQuestion"
        | "TodoWrite"
        | "get_goal"
        | "create_goal"
        | "update_goal"
        | "CreatePlan"
        | "PlanList"
        | "PlanRead"
        | "PlanUpdate"
        | "LegionControl"
        | "acp_control"
        | "acp_message"
        | "acp_history"
        | "submit_code_review"
        | "GetToolSpec"
        | "CallDeferredTool"
        | "SessionControl"
        | "SessionMessage"
        | "SessionHistory"
        | "Cron"
        | "create_group_chat"
        | "invite_group_member"
        | "remove_group_member"
        | "send_group_message"
        | "get_group_history"
        | "list_group_chats"
        | "fork_group_chat"
        | "group_member_status"
        | "delete_group_chat"
        | "update_group_member_tools"
        | "update_group_wiring" => Some(ToolPackFeatureGroup::AgentControl),
        _ => None,
    }
}

pub fn unavailable_feature_groups(requested: &[ToolPackFeatureGroup]) -> Vec<ToolPackFeatureGroup> {
    let enabled = enabled_feature_groups().into_iter().collect::<HashSet<_>>();
    let mut seen = HashSet::new();
    requested
        .iter()
        .copied()
        .filter(|group| !enabled.contains(group))
        .filter(|group| seen.insert(*group))
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

const CORE_BASIC_FEATURE_GROUPS: &[ToolPackFeatureGroup] = &[
    ToolPackFeatureGroup::Basic,
    ToolPackFeatureGroup::ImageAnalysis,
];
const CORE_AGENT_FEATURE_GROUPS: &[ToolPackFeatureGroup] = &[
    ToolPackFeatureGroup::AgentControl,
    ToolPackFeatureGroup::Git,
];
const CORE_CANVAS_FEATURE_GROUPS: &[ToolPackFeatureGroup] = &[ToolPackFeatureGroup::Canvas];
const CORE_SESSION_FEATURE_GROUPS: &[ToolPackFeatureGroup] = &[ToolPackFeatureGroup::AgentControl];
const CORE_INTEGRATION_FEATURE_GROUPS: &[ToolPackFeatureGroup] = &[
    ToolPackFeatureGroup::BrowserWeb,
    ToolPackFeatureGroup::Mcp,
    ToolPackFeatureGroup::Git,
    ToolPackFeatureGroup::MiniApp,
    ToolPackFeatureGroup::ComputerUse,
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
            "WorkspaceScan",
            "KnowledgeBaseSearch",
            "Write",
            "Edit",
            "Delete",
            "ExecCommand",
            "WriteStdin",
            "ExecControl",
            "GetTime",
            "ListModels",
        ],
    },
    ToolProviderGroupPlan {
        provider_id: "core.agent",
        feature_groups: CORE_AGENT_FEATURE_GROUPS,
        tool_names: &[
            "Task",
            "AgentWait",
            "LaunchReviewAgent",
            "Skill",
            "AskUserQuestion",
            "TodoWrite",
            "get_goal",
            "create_goal",
            "update_goal",
            "CreatePlan",
            "PlanList",
            "PlanRead",
            "PlanUpdate",
            "submit_code_review",
            "GetToolSpec",
            "CallDeferredTool",
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
        tool_names: &[
            "SessionControl",
            "LegionControl",
            "SessionMessage",
            "SessionHistory",
            "acp_control",
            "acp_message",
            "acp_history",
            "Cron",
            "create_group_chat",
            "invite_group_member",
            "remove_group_member",
            "send_group_message",
            "get_group_history",
            "list_group_chats",
            "fork_group_chat",
            "group_member_status",
            "delete_group_chat",
            "update_group_member_tools",
            "update_group_wiring",
        ],
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
            "Worktree",
            "ReviewPlatform",
            "InitMiniApp",
            "FinalizeMiniApp",
            "PublishMiniApp",
            "PublishAppearance",
            "PageDeploy",
            "PagePublish",
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
    use std::collections::HashSet;

    use super::{
        all_feature_groups, enabled_feature_groups, product_tool_provider_group_plan,
        tool_feature_group, try_product_tool_provider_group_plan_for_ids,
        unavailable_feature_groups, ToolPackFeatureGroup, ToolProviderGroupPlanSelectionError,
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
    fn provider_plan_reports_every_requested_group_missing_from_the_binary() {
        let unavailable = unavailable_feature_groups(all_feature_groups());
        for group in all_feature_groups() {
            assert_eq!(
                unavailable.contains(group),
                !enabled_feature_groups().contains(group),
                "{} availability must reflect the compiled tool-pack feature",
                group.id(),
            );
        }
    }

    #[test]
    fn every_builtin_tool_has_one_compile_time_owner_group() {
        for tool_name in product_tool_provider_group_plan()
            .iter()
            .flat_map(|group| group.tool_names())
        {
            assert!(
                tool_feature_group(tool_name).is_some(),
                "{tool_name} must have a compile-time feature owner"
            );
        }
    }

    #[test]
    fn every_provider_declares_exactly_its_tool_owner_groups() {
        for provider in product_tool_provider_group_plan() {
            let declared = provider
                .feature_groups()
                .iter()
                .copied()
                .collect::<HashSet<_>>();
            let actual = provider
                .tool_names()
                .iter()
                .map(|tool_name| {
                    tool_feature_group(tool_name).unwrap_or_else(|| {
                        panic!("{tool_name} must have a compile-time feature owner")
                    })
                })
                .collect::<HashSet<_>>();

            assert_eq!(
                declared,
                actual,
                "{} feature groups must match its tool owners",
                provider.provider_id()
            );
            assert_eq!(
                provider.feature_groups().len(),
                declared.len(),
                "{} must not declare duplicate feature groups",
                provider.provider_id()
            );
        }
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
                "WorkspaceScan",
                "KnowledgeBaseSearch",
                "Write",
                "Edit",
                "Delete",
                "ExecCommand",
                "WriteStdin",
                "ExecControl",
                "GetTime",
                "ListModels",
                "Task",
                "AgentWait",
                "LaunchReviewAgent",
                "Skill",
                "AskUserQuestion",
                "TodoWrite",
                "get_goal",
                "create_goal",
                "update_goal",
                "CreatePlan",
                "PlanList",
                "PlanRead",
                "PlanUpdate",
                "submit_code_review",
                "GetToolSpec",
                "CallDeferredTool",
                "GetFileDiff",
                "CreateCanvas",
                "ReadCanvas",
                "UpdateCanvas",
                "PatchCanvas",
                "SessionControl",
                "LegionControl",
                "SessionMessage",
                "SessionHistory",
                "acp_control",
                "acp_message",
                "acp_history",
                "Cron",
                "create_group_chat",
                "invite_group_member",
                "remove_group_member",
                "send_group_message",
                "get_group_history",
                "list_group_chats",
                "fork_group_chat",
                "group_member_status",
                "delete_group_chat",
                "update_group_member_tools",
                "update_group_wiring",
                "WebSearch",
                "WebFetch",
                "ListMCPResources",
                "ReadMCPResource",
                "ListMCPPrompts",
                "GetMCPPrompt",
                "GenerativeUI",
                "Git",
                "Worktree",
                "ReviewPlatform",
                "InitMiniApp",
                "FinalizeMiniApp",
                "PublishMiniApp",
                "PublishAppearance",
                "PageDeploy",
                "PagePublish",
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
                ("core.basic", vec!["basic", "image-analysis"]),
                ("core.agent", vec!["agent-control", "git"]),
                ("core.canvas", vec!["canvas"]),
                ("core.session", vec!["agent-control"]),
                (
                    "core.integration",
                    vec!["browser-web", "mcp", "git", "miniapp", "computer-use",]
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
