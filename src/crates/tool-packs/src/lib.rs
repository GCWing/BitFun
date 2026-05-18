//! Concrete tool-pack owner crate.
//!
//! The feature scaffold is intentionally behavior-neutral until the core
//! `ToolUseContext` and registry boundaries are split into portable ports.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ToolPackFeatureGroup {
    Basic,
    Git,
    Mcp,
    BrowserWeb,
    ComputerUse,
    ImageAnalysis,
    MiniApp,
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
        (
            cfg!(feature = "agent-control"),
            ToolPackFeatureGroup::AgentControl,
        ),
    ]
    .into_iter()
    .filter_map(|(enabled, group)| enabled.then_some(group))
    .collect()
}

#[cfg(test)]
mod tests {
    use super::{all_feature_groups, enabled_feature_groups, ToolPackFeatureGroup};

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
        assert_eq!(ToolPackFeatureGroup::AgentControl.id(), "agent-control");
    }
}
