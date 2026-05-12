use super::support::merge_dynamic_mcp_tools;
use super::AgentRegistry;
use crate::agentic::agents::registry::builtin::default_model_id_for_builtin_agent;
use crate::agentic::agents::registry::types::{
    AgentCategory, AgentEntry, CustomSubagentConfig, SubAgentSource, SubagentListScope,
    SubagentQueryContext,
};
use crate::agentic::agents::registry::visibility::{
    BuiltinSubagentExposure, SubagentVisibilityPolicy,
};
use crate::agentic::agents::Agent;
use async_trait::async_trait;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

struct TestAgent {
    id: String,
}

#[async_trait]
impl Agent for TestAgent {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn id(&self) -> &str {
        &self.id
    }

    fn name(&self) -> &str {
        &self.id
    }

    fn description(&self) -> &str {
        "Test subagent"
    }

    fn prompt_template_name(&self, _model_name: Option<&str>) -> &str {
        "test_agent"
    }

    fn default_tools(&self) -> Vec<String> {
        vec!["Read".to_string()]
    }
}

fn test_project_entry(id: &str, enabled: bool) -> AgentEntry {
    AgentEntry {
        category: AgentCategory::SubAgent,
        subagent_source: Some(SubAgentSource::Project),
        agent: Arc::new(TestAgent { id: id.to_string() }),
        visibility_policy: SubagentVisibilityPolicy::public(),
        custom_config: Some(CustomSubagentConfig {
            enabled,
            model: "fast".to_string(),
        }),
    }
}

fn insert_project_subagent(registry: &AgentRegistry, workspace: &Path, id: &str, enabled: bool) {
    let mut entries = HashMap::new();
    entries.insert(id.to_string(), test_project_entry(id, enabled));
    registry
        .write_project_subagents()
        .insert(workspace.to_path_buf(), entries);
}

#[test]
fn top_level_modes_default_to_auto() {
    for agent_type in [
        "agentic",
        "Cowork",
        "Plan",
        "debug",
        "Claw",
        "DeepResearch",
        "Team",
    ] {
        assert_eq!(default_model_id_for_builtin_agent(agent_type), "auto");
    }
}

#[tokio::test]
async fn computer_use_is_builtin_subagent_not_mode() {
    let registry = AgentRegistry::new();
    let modes = registry.get_modes_info().await;
    assert!(
        !modes.iter().any(|agent| agent.id == "ComputerUse"),
        "ComputerUse should be delegated through Task as a built-in sub-agent, not exposed as a top-level mode"
    );

    let subagents = registry.get_subagents_info(None).await;
    let computer_use = subagents
        .iter()
        .find(|agent| agent.id == "ComputerUse")
        .expect("ComputerUse should be registered as a built-in sub-agent");
    assert!(computer_use
        .default_tools
        .contains(&"ControlHub".to_string()));
    assert!(computer_use
        .default_tools
        .contains(&"ComputerUse".to_string()));
    assert_eq!(
        computer_use.visibility.as_ref().map(|value| value.exposure),
        Some(BuiltinSubagentExposure::Restricted)
    );
}

#[test]
fn non_deep_review_builtin_subagents_default_to_primary() {
    for agent_type in ["Explore", "FileFinder", "CodeReview", "GenerateDoc", "Init"] {
        assert_eq!(
            default_model_id_for_builtin_agent(agent_type),
            "primary",
            "{agent_type} should default to the primary model slot"
        );
    }
}

#[test]
fn deep_review_family_defaults_to_fast() {
    for agent_type in [
        "DeepReview",
        "ReviewBusinessLogic",
        "ReviewPerformance",
        "ReviewSecurity",
        "ReviewArchitecture",
        "ReviewFrontend",
        "ReviewJudge",
        "ReviewFixer",
    ] {
        assert_eq!(
            default_model_id_for_builtin_agent(agent_type),
            "fast",
            "{agent_type} should stay on the fast model slot"
        );
    }
}

#[tokio::test]
async fn frontend_reviewer_is_registered_as_review_subagent() {
    let registry = AgentRegistry::new();
    let subagents = registry.get_subagents_info(None).await;
    let frontend = subagents
        .iter()
        .find(|agent| agent.id == "ReviewFrontend")
        .expect("ReviewFrontend should be registered as a subagent");

    assert!(frontend.is_review);
    assert!(frontend.is_readonly);
}

#[test]
fn built_in_deep_review_reviewers_are_marked_as_review_agents() {
    let registry = AgentRegistry::new();

    for agent_type in [
        "ReviewBusinessLogic",
        "ReviewPerformance",
        "ReviewSecurity",
        "ReviewArchitecture",
        "ReviewFrontend",
        "ReviewJudge",
    ] {
        assert_eq!(
            registry.get_subagent_is_review(agent_type),
            Some(true),
            "{agent_type} must pass DeepReview Task policy validation"
        );
    }
}

#[tokio::test]
async fn task_visible_subagents_are_filtered_by_parent_agent() {
    let registry = AgentRegistry::new();

    let agentic_visible = registry
        .get_subagents_for_query(&SubagentQueryContext {
            parent_agent_type: Some("agentic"),
            workspace_root: None,
            list_scope: SubagentListScope::TaskVisible,
            include_disabled: false,
        })
        .await;
    assert!(agentic_visible.iter().any(|agent| agent.id == "Explore"));
    assert!(!agentic_visible.iter().any(|agent| agent.id == "ReviewSecurity"));
    assert!(!agentic_visible
        .iter()
        .any(|agent| agent.id == "ResearchSpecialist"));

    let deep_review_visible = registry
        .get_subagents_for_query(&SubagentQueryContext {
            parent_agent_type: Some("DeepReview"),
            workspace_root: None,
            list_scope: SubagentListScope::TaskVisible,
            include_disabled: false,
        })
        .await;
    assert!(deep_review_visible
        .iter()
        .any(|agent| agent.id == "ReviewSecurity"));
    assert!(!deep_review_visible
        .iter()
        .any(|agent| agent.id == "ResearchSpecialist"));

    let deep_research_visible = registry
        .get_subagents_for_query(&SubagentQueryContext {
            parent_agent_type: Some("DeepResearch"),
            workspace_root: None,
            list_scope: SubagentListScope::TaskVisible,
            include_disabled: false,
        })
        .await;
    assert!(deep_research_visible
        .iter()
        .any(|agent| agent.id == "ResearchSpecialist"));
    assert!(!deep_research_visible
        .iter()
        .any(|agent| agent.id == "ReviewSecurity"));
}

#[test]
fn merge_dynamic_mcp_tools_appends_registered_mcp_tools_once() {
    let configured_tools = vec!["Read".to_string(), "Bash".to_string()];
    let registered_tool_names = vec![
        "Read".to_string(),
        "mcp__notion__notion-search".to_string(),
        "mcp__github__list_issues".to_string(),
        "mcp__notion__notion-search".to_string(),
    ];

    let merged = merge_dynamic_mcp_tools(configured_tools, &registered_tool_names);

    assert_eq!(
        merged,
        vec![
            "Read".to_string(),
            "Bash".to_string(),
            "mcp__notion__notion-search".to_string(),
            "mcp__github__list_issues".to_string(),
        ]
    );
}

#[test]
fn project_subagent_config_lookup_is_workspace_scoped() {
    let registry = AgentRegistry::new();
    let workspace_a = PathBuf::from("D:/workspace/project-a");
    let workspace_b = PathBuf::from("D:/workspace/project-b");
    insert_project_subagent(&registry, &workspace_a, "SharedReviewer", false);
    insert_project_subagent(&registry, &workspace_b, "SharedReviewer", true);

    assert_eq!(
        registry
            .get_custom_subagent_config("SharedReviewer", Some(&workspace_a))
            .expect("workspace A config")
            .enabled,
        false
    );
    assert_eq!(
        registry
            .get_custom_subagent_config("SharedReviewer", Some(&workspace_b))
            .expect("workspace B config")
            .enabled,
        true
    );
    assert!(
        registry
            .get_custom_subagent_config("SharedReviewer", None)
            .is_none(),
        "unscoped lookup must not pick an arbitrary project subagent"
    );
    assert!(registry.has_project_custom_subagent("SharedReviewer"));
}
