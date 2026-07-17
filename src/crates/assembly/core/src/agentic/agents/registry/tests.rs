use super::support::merge_dynamic_mcp_tools;
use super::{AgentRegistry, ExternalSubagentRegistration, ExternalSubagentRoute};
use crate::agentic::agents::definitions::custom::{CustomMode, CustomSubagent, CustomSubagentKind};
use crate::agentic::agents::registry::builtin::default_model_id_for_builtin_agent;
use crate::agentic::agents::registry::types::{
    subagent_source_from_custom_kind, AgentCategory, AgentEntry, AgentSource, CustomSubagentConfig,
    SubAgentSource, SubagentListScope, SubagentOverrideState, SubagentQueryContext,
};
use crate::agentic::agents::registry::visibility::{
    BuiltinSubagentExposure, SubagentVisibilityPolicy,
};
use crate::agentic::agents::{resolve_mode_config_profile_id, Agent, UserContextPolicy};
use crate::service::config::types::AgentSubagentOverrideState;
use async_trait::async_trait;
use bitfun_agent_runtime::custom_agent::{
    custom_agent_save_markdown_file, CustomAgentDefinition, CustomAgentDiscoveryRoots,
    CustomAgentKind, CustomAgentLevel,
};
use bitfun_agent_runtime::sdk::{RuntimeAgentRegistry, RuntimeAgentRegistryQuery};
use std::collections::{BTreeMap, HashMap};
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

    fn user_context_policy(&self) -> UserContextPolicy {
        UserContextPolicy::empty()
    }

    fn default_tools(&self) -> Vec<String> {
        vec!["Read".to_string()]
    }
}

fn test_project_entry(id: &str, model: &str) -> AgentEntry {
    AgentEntry {
        category: AgentCategory::SubAgent,
        source: AgentSource::Project,
        subagent_source: Some(SubAgentSource::Project),
        agent: Arc::new(TestAgent { id: id.to_string() }),
        visibility_policy: SubagentVisibilityPolicy::public(),
        custom_config: Some(CustomSubagentConfig {
            model: model.to_string(),
            model_is_explicit: true,
        }),
    }
}

fn test_project_custom_entry(id: &str, review: bool) -> AgentEntry {
    let mut agent = CustomSubagent::new(
        id.to_string(),
        "Project custom subagent".to_string(),
        vec!["Read".to_string()],
        "prompt".to_string(),
        review,
        format!("{id}.md"),
        CustomSubagentKind::Project,
    );
    agent.data.review = review;

    AgentEntry {
        category: AgentCategory::SubAgent,
        source: AgentSource::Project,
        subagent_source: Some(SubAgentSource::Project),
        agent: Arc::new(agent),
        visibility_policy: SubagentVisibilityPolicy::public(),
        custom_config: Some(CustomSubagentConfig {
            model: "fast".to_string(),
            model_is_explicit: true,
        }),
    }
}

fn insert_project_subagent(registry: &AgentRegistry, workspace: &Path, id: &str, model: &str) {
    let mut entries = HashMap::new();
    entries.insert(id.to_string(), test_project_entry(id, model));
    registry
        .write_project_subagents()
        .insert(workspace.to_path_buf(), entries);
}

#[tokio::test]
async fn review_lookup_is_scoped_to_the_requested_workspace() {
    let registry = AgentRegistry::new();
    let review_workspace = PathBuf::from("review-workspace");
    let ordinary_workspace = PathBuf::from("ordinary-workspace");
    let agent_id = "SharedProjectAgent";

    registry.write_project_subagents().insert(
        review_workspace.clone(),
        HashMap::from([(
            agent_id.to_string(),
            test_project_custom_entry(agent_id, true),
        )]),
    );
    registry.write_project_subagents().insert(
        ordinary_workspace.clone(),
        HashMap::from([(
            agent_id.to_string(),
            test_project_custom_entry(agent_id, false),
        )]),
    );

    assert_eq!(
        registry
            .get_subagent_is_review_for_workspace(agent_id, Some(&review_workspace))
            .await,
        Some(true)
    );
    assert_eq!(
        registry
            .get_subagent_is_review_for_workspace(agent_id, Some(&ordinary_workspace))
            .await,
        Some(false)
    );
    assert_eq!(
        registry
            .get_subagent_is_review_for_workspace(agent_id, None)
            .await,
        None,
        "a project agent must not leak into an unrelated workspace lookup"
    );
}

#[tokio::test]
async fn review_lookup_cold_loads_the_requested_project_registry() {
    let env = CustomAgentTestEnv::new("bitfun-project-review-lookup");
    let registry = AgentRegistry::new();
    let agent_id = "ProjectReviewer";
    write_project_custom_review_subagent(
        &env.workspace_agents_dir.join("project-reviewer.md"),
        agent_id,
    );

    assert_eq!(
        registry
            .get_subagent_is_review_for_workspace(agent_id, Some(&env.workspace_root))
            .await,
        Some(true)
    );
}

#[test]
fn top_level_modes_default_to_auto() {
    for agent_type in [
        "agentic",
        "Multitask",
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

#[test]
fn custom_subagent_kind_maps_to_registry_source() {
    assert_eq!(
        subagent_source_from_custom_kind(CustomSubagentKind::Project),
        SubAgentSource::Project
    );
    assert_eq!(
        subagent_source_from_custom_kind(CustomSubagentKind::User),
        SubAgentSource::User
    );
}

#[test]
fn registry_exposes_sdk_agent_ids_without_leaking_core_agent_details() {
    let registry = AgentRegistry::new();
    let workspace = PathBuf::from("D:/workspace/project");
    let other_workspace = PathBuf::from("D:/workspace/other");
    insert_project_subagent(&registry, &workspace, "ProjectReviewer", "fast");
    insert_project_subagent(&registry, &other_workspace, "OtherProjectReviewer", "fast");

    let global_agent_ids =
        RuntimeAgentRegistry::agent_ids(&registry, RuntimeAgentRegistryQuery::default());

    assert!(global_agent_ids.contains(&"agentic".to_string()));
    assert!(global_agent_ids.contains(&"Explore".to_string()));
    assert!(!global_agent_ids.contains(&"ProjectReviewer".to_string()));
    assert!(!global_agent_ids.contains(&"OtherProjectReviewer".to_string()));

    let agent_ids = RuntimeAgentRegistry::agent_ids(
        &registry,
        RuntimeAgentRegistryQuery {
            workspace_root: Some(&workspace),
        },
    );

    assert!(agent_ids.contains(&"ProjectReviewer".to_string()));
    assert!(!agent_ids.contains(&"OtherProjectReviewer".to_string()));
    assert_eq!(
        agent_ids,
        {
            let mut sorted = agent_ids.clone();
            sorted.sort();
            sorted.dedup();
            sorted
        },
        "SDK agent registry projection must be stable and deduplicated"
    );
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
    for agent_type in [
        "Explore",
        "FileFinder",
        "CodeReview",
        "GeneralPurpose",
        "MemoryPhase2",
    ] {
        assert_eq!(
            default_model_id_for_builtin_agent(agent_type),
            "primary",
            "{agent_type} should default to the primary model slot"
        );
    }
}

#[test]
fn memory_phase2_hidden_agent_is_registered() {
    let registry = AgentRegistry::new();
    let agent = registry
        .get_agent("MemoryPhase2", None)
        .expect("MemoryPhase2 should be registered as a hidden built-in agent");

    assert_eq!(agent.id(), "MemoryPhase2");
    assert_eq!(agent.name(), "Memory Phase 2");
}

#[test]
fn generate_doc_hidden_agent_defaults_to_fast() {
    assert_eq!(default_model_id_for_builtin_agent("GenerateDoc"), "fast");
}

#[test]
fn deep_review_family_defaults_to_fast() {
    for agent_type in [
        "DeepReview",
        "ReviewGeneral",
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
fn built_in_readonly_reviewers_are_marked_as_review_agents() {
    let registry = AgentRegistry::new();

    for agent_type in [
        "ReviewGeneral",
        "ReviewBusinessLogic",
        "ReviewPerformance",
        "ReviewSecurity",
        "ReviewArchitecture",
        "ReviewFrontend",
        "ReviewJudge",
        "CodeReview",
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
            external_sources_supported: false,
        })
        .await;
    assert!(agentic_visible.iter().any(|agent| agent.id == "Explore"));
    assert!(agentic_visible
        .iter()
        .any(|agent| agent.id == "GeneralPurpose"));
    let code_review = agentic_visible
        .iter()
        .find(|agent| agent.id == "CodeReview")
        .expect("CodeReview should be available as an isolated review task");
    assert!(code_review.is_review);
    assert!(code_review.is_readonly);
    assert!(!agentic_visible
        .iter()
        .any(|agent| agent.id == "ReviewSecurity"));
    assert!(!agentic_visible
        .iter()
        .any(|agent| agent.id == "ResearchSpecialist"));

    let deep_review_visible = registry
        .get_subagents_for_query(&SubagentQueryContext {
            parent_agent_type: Some("DeepReview"),
            workspace_root: None,
            list_scope: SubagentListScope::TaskVisible,
            include_disabled: false,
            external_sources_supported: false,
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
            external_sources_supported: false,
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
    let configured_tools = vec!["Read".to_string(), "ExecCommand".to_string()];
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
            "ExecCommand".to_string(),
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
    insert_project_subagent(&registry, &workspace_a, "SharedReviewer", "fast");
    insert_project_subagent(&registry, &workspace_b, "SharedReviewer", "primary");

    assert_eq!(
        registry
            .get_custom_subagent_config("SharedReviewer", Some(&workspace_a))
            .expect("workspace A config")
            .model,
        "fast"
    );
    assert_eq!(
        registry
            .get_custom_subagent_config("SharedReviewer", Some(&workspace_b))
            .expect("workspace B config")
            .model,
        "primary"
    );
    assert!(
        registry
            .get_custom_subagent_config("SharedReviewer", None)
            .is_none(),
        "unscoped lookup must not pick an arbitrary project subagent"
    );
    assert!(registry.has_project_custom_subagent("SharedReviewer"));
}

#[tokio::test]
async fn prompt_stability_task_visible_subagents_are_sorted_deterministically() {
    let registry = AgentRegistry::new();
    let workspace = PathBuf::from("D:/workspace/project-c");

    registry.register_agent(
        Arc::new(TestAgent {
            id: "zBuiltin".to_string(),
        }),
        AgentCategory::SubAgent,
        AgentSource::Builtin,
        Some(SubAgentSource::Builtin),
        None,
    );
    registry.register_agent(
        Arc::new(TestAgent {
            id: "ABuiltin".to_string(),
        }),
        AgentCategory::SubAgent,
        AgentSource::Builtin,
        Some(SubAgentSource::Builtin),
        None,
    );

    let mut project_entries = HashMap::new();
    project_entries.insert(
        "zProject".to_string(),
        test_project_entry("zProject", "fast"),
    );
    project_entries.insert(
        "AProject".to_string(),
        test_project_entry("AProject", "fast"),
    );
    registry
        .write_project_subagents()
        .insert(workspace.clone(), project_entries);

    registry.register_agent(
        Arc::new(TestAgent {
            id: "zUser".to_string(),
        }),
        AgentCategory::SubAgent,
        AgentSource::User,
        Some(SubAgentSource::User),
        Some(CustomSubagentConfig {
            model: "fast".to_string(),
            model_is_explicit: true,
        }),
    );
    registry.register_agent(
        Arc::new(TestAgent {
            id: "AUser".to_string(),
        }),
        AgentCategory::SubAgent,
        AgentSource::User,
        Some(SubAgentSource::User),
        Some(CustomSubagentConfig {
            model: "fast".to_string(),
            model_is_explicit: true,
        }),
    );
    registry.set_user_custom_agents_loaded(true);

    let visible = registry
        .get_subagents_for_query(&SubagentQueryContext {
            parent_agent_type: None,
            workspace_root: Some(&workspace),
            list_scope: SubagentListScope::RegistryManagement,
            include_disabled: false,
            external_sources_supported: false,
        })
        .await;

    let ids: Vec<&str> = visible.iter().map(|agent| agent.id.as_str()).collect();
    let expected = vec![
        "ABuiltin",
        "Explore",
        "FileFinder",
        "GeneralPurpose",
        "zBuiltin",
        "AProject",
        "zProject",
        "AUser",
        "zUser",
    ];

    assert_eq!(ids, expected);
}

#[tokio::test]
async fn parent_subagent_overrides_follow_source_scopes() {
    let registry = AgentRegistry::new();
    let workspace = PathBuf::from("__test_workspace__/project-d");

    registry.register_agent(
        Arc::new(CustomSubagent::new(
            "UserScout".to_string(),
            "User scout".to_string(),
            vec!["Read".to_string()],
            "prompt".to_string(),
            true,
            "user-scout.md".to_string(),
            CustomSubagentKind::User,
        )),
        AgentCategory::SubAgent,
        AgentSource::User,
        Some(SubAgentSource::User),
        Some(CustomSubagentConfig {
            model: "fast".to_string(),
            model_is_explicit: true,
        }),
    );
    registry.set_user_custom_agents_loaded(true);

    let mut project_entries = HashMap::new();
    project_entries.insert(
        "ProjectScout".to_string(),
        AgentEntry {
            category: AgentCategory::SubAgent,
            source: AgentSource::Project,
            subagent_source: Some(SubAgentSource::Project),
            agent: Arc::new(CustomSubagent::new(
                "ProjectScout".to_string(),
                "Project scout".to_string(),
                vec!["Read".to_string()],
                "prompt".to_string(),
                true,
                "project-scout.md".to_string(),
                CustomSubagentKind::Project,
            )),
            visibility_policy: SubagentVisibilityPolicy::public(),
            custom_config: Some(CustomSubagentConfig {
                model: "fast".to_string(),
                model_is_explicit: true,
            }),
        },
    );
    registry
        .write_project_subagents()
        .insert(workspace.clone(), project_entries);

    let builtin_query = SubagentQueryContext {
        parent_agent_type: Some("agentic"),
        workspace_root: Some(&workspace),
        list_scope: SubagentListScope::RegistryManagement,
        include_disabled: true,
        external_sources_supported: false,
    };

    let project_override_key = "project::bitfun::ProjectScout".to_string();
    let user_override_key = "user::bitfun::UserScout".to_string();
    let builtin_override_key = "builtin::builtin::Explore".to_string();

    let mut project_parent_map = HashMap::new();
    project_parent_map.insert(
        project_override_key.clone(),
        AgentSubagentOverrideState::Disabled,
    );
    project_parent_map.insert(
        user_override_key.clone(),
        AgentSubagentOverrideState::Disabled,
    );
    project_parent_map.insert(
        builtin_override_key.clone(),
        AgentSubagentOverrideState::Disabled,
    );
    let mut project_overrides = HashMap::new();
    project_overrides.insert(
        resolve_mode_config_profile_id("agentic").into_owned(),
        project_parent_map,
    );

    let mut user_parent_map = HashMap::new();
    user_parent_map.insert(
        project_override_key.clone(),
        AgentSubagentOverrideState::Enabled,
    );
    user_parent_map.insert(user_override_key, AgentSubagentOverrideState::Disabled);
    user_parent_map.insert(builtin_override_key, AgentSubagentOverrideState::Disabled);
    let mut user_overrides = HashMap::new();
    user_overrides.insert(
        resolve_mode_config_profile_id("agentic").into_owned(),
        user_parent_map,
    );

    let visible = {
        use crate::agentic::agents::registry::availability::resolve_availability;

        let explore = registry
            .find_agent_entry("Explore", Some(&workspace))
            .expect("builtin entry");
        let user = registry
            .find_agent_entry("UserScout", Some(&workspace))
            .expect("user entry");
        let project = registry
            .find_agent_entry("ProjectScout", Some(&workspace))
            .expect("project entry");

        (
            resolve_availability(
                &explore,
                builtin_query.parent_agent_type,
                Some(&project_overrides),
                &user_overrides,
            ),
            resolve_availability(
                &user,
                builtin_query.parent_agent_type,
                Some(&project_overrides),
                &user_overrides,
            ),
            resolve_availability(
                &project,
                builtin_query.parent_agent_type,
                Some(&project_overrides),
                &user_overrides,
            ),
        )
    };

    assert_eq!(
        visible.0.override_state,
        Some(SubagentOverrideState::Disabled)
    );
    assert_eq!(
        visible.1.override_state,
        Some(SubagentOverrideState::Disabled)
    );
    assert_eq!(
        visible.2.override_state,
        Some(SubagentOverrideState::Disabled)
    );
}

#[tokio::test]
async fn explicit_custom_mode_load_exposes_user_mode_metadata_in_modes_info() {
    let env = CustomAgentTestEnv::new("bitfun-custom-mode-registry-load");
    let registry = AgentRegistry::new();
    let mode_path = env.user_agents_dir.join("planner-plus.md");
    write_user_custom_mode(
        &mode_path,
        "PlannerPlus",
        "Planner Plus",
        vec!["Read".to_string(), "Grep".to_string()],
        UserContextPolicy::empty().with_workspace_instructions(),
        "primary",
        true,
    );

    registry
        .load_custom_agents_from_test_roots(None, &env.discovery_roots(None))
        .await;

    let mode = registry
        .get_modes_info()
        .await
        .into_iter()
        .find(|agent| agent.id == "PlannerPlus")
        .expect("custom mode should be present in modes info");

    assert_eq!(mode.source, AgentSource::User);
    assert_eq!(mode.path, Some(mode_path.to_string_lossy().to_string()));
    assert_eq!(mode.model, Some("primary".to_string()));
    assert_eq!(
        mode.default_tools,
        vec!["Read".to_string(), "Grep".to_string()]
    );
    assert!(mode.is_readonly);
}

#[tokio::test]
async fn custom_mode_does_not_appear_in_subagent_list() {
    let env = CustomAgentTestEnv::new("bitfun-custom-mode-registry-separation");
    let registry = AgentRegistry::new();
    write_user_custom_mode(
        &env.user_agents_dir.join("planner-plus.md"),
        "PlannerPlus",
        "Planner Plus",
        vec!["Read".to_string()],
        UserContextPolicy::empty().with_workspace_instructions(),
        "auto",
        false,
    );
    write_user_custom_subagent(&env.user_agents_dir.join("helper.md"), "Helper");

    registry
        .load_custom_agents_from_test_roots(None, &env.discovery_roots(None))
        .await;

    let subagents = registry.get_subagents_info(None).await;
    assert!(!subagents.iter().any(|agent| agent.id == "PlannerPlus"));
    assert!(subagents.iter().any(|agent| agent.id == "Helper"));
}

#[tokio::test]
async fn project_scoped_custom_mode_is_skipped_while_project_subagent_loads() {
    let env = CustomAgentTestEnv::new("bitfun-custom-mode-registry-project");
    let registry = AgentRegistry::new();
    let workspace_root = env.workspace_root.clone();

    write_project_custom_mode(
        &env.workspace_agents_dir.join("project-mode.md"),
        "ProjectPlanner",
    );
    write_project_custom_subagent(
        &env.workspace_agents_dir.join("project-helper.md"),
        "ProjectHelper",
    );

    registry
        .load_custom_agents_from_test_roots(
            Some(&workspace_root),
            &env.discovery_roots(Some(workspace_root.clone())),
        )
        .await;

    let modes = registry.get_modes_info().await;
    let subagents = registry.get_subagents_info(Some(&workspace_root)).await;

    assert!(!modes.iter().any(|agent| agent.id == "ProjectPlanner"));
    assert!(subagents.iter().any(|agent| agent.id == "ProjectHelper"));
}

#[tokio::test]
async fn custom_mode_detail_reports_kind_level_model_path_and_policy() {
    let env = CustomAgentTestEnv::new("bitfun-custom-mode-registry-detail");
    let registry = AgentRegistry::new();
    let mode_path = env.user_agents_dir.join("planner-plus.md");
    write_user_custom_mode(
        &mode_path,
        "PlannerPlus",
        "Planner Plus",
        vec!["Read".to_string(), "Grep".to_string()],
        UserContextPolicy::empty().with_workspace_instructions(),
        "primary",
        true,
    );

    registry
        .load_custom_agents_from_test_roots(None, &env.discovery_roots(None))
        .await;

    let detail = registry
        .get_custom_agent_detail("PlannerPlus", None)
        .await
        .expect("custom mode detail should load");

    assert_eq!(detail.kind, "mode");
    assert_eq!(detail.level, "user");
    assert_eq!(detail.model, "primary");
    assert_eq!(detail.path, mode_path.to_string_lossy().to_string());
    assert_eq!(
        detail.user_context_policy,
        vec!["workspace_instructions".to_string()]
    );
    assert_eq!(detail.tools, vec!["Read".to_string(), "Grep".to_string()]);
    assert!(detail.readonly);
    assert!(!detail.review);
}

#[tokio::test]
async fn updating_custom_mode_model_persists_and_keeps_mode_category() {
    let env = CustomAgentTestEnv::new("bitfun-custom-mode-registry-update-model");
    let registry = AgentRegistry::new();
    let mode_path = env.user_agents_dir.join("planner-plus.md");
    write_user_custom_mode(
        &mode_path,
        "PlannerPlus",
        "Planner Plus",
        vec!["Read".to_string()],
        UserContextPolicy::empty().with_workspace_instructions(),
        "auto",
        false,
    );

    registry
        .load_custom_agents_from_test_roots(None, &env.discovery_roots(None))
        .await;
    registry
        .update_and_save_custom_agent_config(
            "PlannerPlus",
            Some("primary".to_string()),
            false,
            None,
        )
        .expect("mode model update should save");

    let mode = registry
        .get_modes_info()
        .await
        .into_iter()
        .find(|agent| agent.id == "PlannerPlus")
        .expect("updated mode should still be present");
    let saved = std::fs::read_to_string(&mode_path).expect("updated mode file should be readable");

    assert_eq!(mode.model, Some("primary".to_string()));
    assert_eq!(mode.source, AgentSource::User);
    assert!(registry.get_mode_agent("PlannerPlus").is_some());
    assert!(!registry
        .get_subagents_info(None)
        .await
        .iter()
        .any(|agent| agent.id == "PlannerPlus"));
    assert!(saved.contains("kind: mode"));
    assert!(saved.contains("model: primary"));
}

#[tokio::test]
async fn updating_custom_mode_definition_rewrites_file_and_preserves_mode_kind() {
    let env = CustomAgentTestEnv::new("bitfun-custom-mode-registry-update-definition");
    let registry = AgentRegistry::new();
    let mode_path = env.user_agents_dir.join("planner-plus.md");
    write_user_custom_mode(
        &mode_path,
        "PlannerPlus",
        "Planner Plus",
        vec!["Read".to_string()],
        UserContextPolicy::empty().with_workspace_instructions(),
        "auto",
        false,
    );

    registry
        .load_custom_agents_from_test_roots(None, &env.discovery_roots(None))
        .await;
    registry
        .update_custom_agent_definition(
            "PlannerPlus",
            None,
            "Planner Pro".to_string(),
            "Updated planning mode".to_string(),
            "Always explain your plan first.".to_string(),
            Some(vec!["Read".to_string(), "Grep".to_string()]),
            Some(true),
            None,
            Some(UserContextPolicy::empty().with_workspace_context()),
            Some("primary".to_string()),
        )
        .await
        .expect("mode definition update should save");

    let detail = registry
        .get_custom_agent_detail("PlannerPlus", None)
        .await
        .expect("updated mode detail should load");
    let saved = std::fs::read_to_string(&mode_path).expect("updated mode file should be readable");

    assert_eq!(detail.kind, "mode");
    assert_eq!(detail.name, "Planner Pro");
    assert_eq!(detail.description, "Updated planning mode");
    assert_eq!(detail.prompt, "Always explain your plan first.");
    assert_eq!(detail.model, "primary");
    assert_eq!(detail.tools, vec!["Read".to_string(), "Grep".to_string()]);
    assert_eq!(
        detail.user_context_policy,
        vec!["workspace_context".to_string()]
    );
    assert!(saved.contains("kind: mode"));
    assert!(saved.contains("name: Planner Pro"));
    assert!(saved.contains("model: primary"));
    assert!(saved.contains("- workspace_context"));
}

struct CustomAgentTestEnv {
    root: PathBuf,
    workspace_root: PathBuf,
    workspace_agents_dir: PathBuf,
    user_agents_dir: PathBuf,
}

impl CustomAgentTestEnv {
    fn new(prefix: &str) -> Self {
        let root = std::env::temp_dir().join(format!("{prefix}-{}", unique_suffix()));
        let workspace_root = root.join("workspace");
        let workspace_agents_dir = workspace_root.join(".bitfun").join("agents");
        let user_agents_dir = root.join("user-root").join("agents");
        std::fs::create_dir_all(&workspace_agents_dir)
            .expect("workspace agents dir should be created");
        std::fs::create_dir_all(&user_agents_dir).expect("user agents dir should be created");

        Self {
            root,
            workspace_root,
            workspace_agents_dir,
            user_agents_dir,
        }
    }

    fn discovery_roots(&self, workspace_root: Option<PathBuf>) -> CustomAgentDiscoveryRoots {
        CustomAgentDiscoveryRoots {
            workspace_root,
            bitfun_user_agents_dir: Some(self.user_agents_dir.clone()),
            home_dir: None,
        }
    }
}

impl Drop for CustomAgentTestEnv {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn write_user_custom_mode(
    path: &Path,
    id: &str,
    name: &str,
    tools: Vec<String>,
    user_context_policy: UserContextPolicy,
    model: &str,
    readonly: bool,
) {
    let mode = CustomMode::new(
        id.to_string(),
        name.to_string(),
        "User-defined custom mode".to_string(),
        tools,
        "Act as a focused project specialist.".to_string(),
        readonly,
        path.to_string_lossy().to_string(),
        model.to_string(),
        user_context_policy,
    );
    mode.save_to_file(None)
        .expect("custom mode markdown should save");
}

fn write_project_custom_mode(path: &Path, id: &str) {
    let definition = CustomAgentDefinition::from_front_matter_fields(
        Some(id),
        Some(id),
        Some("Project custom mode"),
        Some(CustomAgentKind::Mode),
        None,
        None,
        None,
        None,
        None,
        "Project-scoped modes should be rejected.".to_string(),
        CustomAgentLevel::Project,
    )
    .expect("project mode definition should be valid")
    .definition;
    custom_agent_save_markdown_file(path, &definition).expect("project mode markdown should save");
}

fn write_user_custom_subagent(path: &Path, id: &str) {
    let subagent = CustomSubagent::new_with_id(
        id.to_string(),
        id.to_string(),
        "User helper subagent".to_string(),
        vec!["Read".to_string()],
        "Investigate the relevant files.".to_string(),
        true,
        path.to_string_lossy().to_string(),
        CustomSubagentKind::User,
        "fast".to_string(),
        UserContextPolicy::empty().with_workspace_instructions(),
    );
    subagent
        .save_to_file(None)
        .expect("custom subagent markdown should save");
}

fn write_project_custom_subagent(path: &Path, id: &str) {
    let subagent = CustomSubagent::new_with_id(
        id.to_string(),
        id.to_string(),
        "Project helper subagent".to_string(),
        vec!["Read".to_string()],
        "Investigate the relevant files.".to_string(),
        true,
        path.to_string_lossy().to_string(),
        CustomSubagentKind::Project,
        "fast".to_string(),
        UserContextPolicy::empty().with_workspace_instructions(),
    );
    subagent
        .save_to_file(None)
        .expect("project subagent markdown should save");
}

fn write_project_custom_review_subagent(path: &Path, id: &str) {
    let mut subagent = CustomSubagent::new_with_id(
        id.to_string(),
        id.to_string(),
        "Project review subagent".to_string(),
        vec!["Read".to_string()],
        "Review the relevant files.".to_string(),
        true,
        path.to_string_lossy().to_string(),
        CustomSubagentKind::Project,
        "fast".to_string(),
        UserContextPolicy::empty().with_workspace_instructions(),
    );
    subagent.data.review = true;
    subagent
        .save_to_file(None)
        .expect("project review subagent markdown should save");
}

fn unique_suffix() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};

    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after UNIX epoch")
        .as_nanos()
        .to_string()
}

#[tokio::test]
async fn external_routes_are_workspace_scoped_fail_closed_and_generation_leased() {
    let registry = AgentRegistry::new();
    let workspace = PathBuf::from("C:/workspace/external-agent-registry");
    let runtime_v1 = "external::candidate::behavior-v1";
    let agent_v1: Arc<dyn Agent> = Arc::new(TestAgent {
        id: runtime_v1.to_string(),
    });
    registry.install_external_subagent_routes(
        &workspace,
        vec![ExternalSubagentRegistration {
            runtime_key: runtime_v1.to_string(),
            logical_id: "Explore".to_string(),
            provider_label: "OpenCode".to_string(),
            model_binding: super::ExternalSubagentModelBinding {
                model_id: "inherit".to_string(),
                configuration_fingerprint: "model-config-v1".to_string(),
            },
            hidden: false,
            agent: agent_v1,
        }],
        [(
            "explore".to_string(),
            ExternalSubagentRoute::External(runtime_v1.to_string()),
        )]
        .into_iter()
        .collect(),
    );

    let local_only = registry
        .get_subagents_for_query(&SubagentQueryContext {
            parent_agent_type: Some("agentic"),
            workspace_root: Some(&workspace),
            list_scope: SubagentListScope::TaskVisible,
            include_disabled: false,
            external_sources_supported: false,
        })
        .await;
    assert!(local_only
        .iter()
        .any(|agent| { agent.id == "Explore" && agent.source == AgentSource::Builtin }));

    let external = registry
        .get_subagents_for_query(&SubagentQueryContext {
            parent_agent_type: Some("agentic"),
            workspace_root: Some(&workspace),
            list_scope: SubagentListScope::TaskVisible,
            include_disabled: false,
            external_sources_supported: true,
        })
        .await;
    let projected = external
        .iter()
        .find(|agent| agent.id == "Explore")
        .expect("external route replaces the same-name local projection");
    assert_eq!(projected.source, AgentSource::External);
    assert_eq!(
        projected.external_provider_label.as_deref(),
        Some("OpenCode")
    );
    assert_eq!(projected.model.as_deref(), Some("inherit"));
    assert_eq!(projected.model_is_explicit, Some(true));
    assert!(!projected.supports_follow_up);
    assert!(registry.is_external_subagent_route("Explore", Some(&workspace)));
    assert!(registry.is_external_subagent_route("EXPLORE", Some(&workspace)));
    assert!(!registry.is_external_subagent_route("Explore", None));
    assert!(!registry.is_external_subagent_route("Explore", Some(Path::new("C:/workspace/other"))));

    let binding = registry
        .resolve_subagent_for_fresh_invocation("Explore", Some(&workspace), true)
        .expect("external invocation binding");
    assert_eq!(binding.runtime_agent_key, runtime_v1);
    assert!(!binding.supports_follow_up);
    let leased_model = binding
        .lease
        .as_ref()
        .expect("external binding keeps a generation lease")
        .model_binding();
    assert_eq!(leased_model.model_id, "inherit");
    assert_eq!(leased_model.configuration_fingerprint, "model-config-v1");

    let runtime_v2 = "external::candidate::behavior-v2";
    let agent_v2: Arc<dyn Agent> = Arc::new(TestAgent {
        id: runtime_v2.to_string(),
    });
    registry.install_external_subagent_routes(
        &workspace,
        vec![ExternalSubagentRegistration {
            runtime_key: runtime_v2.to_string(),
            logical_id: "Explore".to_string(),
            provider_label: "OpenCode".to_string(),
            model_binding: super::ExternalSubagentModelBinding {
                model_id: "inherit".to_string(),
                configuration_fingerprint: "model-config-v2".to_string(),
            },
            hidden: false,
            agent: agent_v2,
        }],
        [(
            "explore".to_string(),
            ExternalSubagentRoute::External(runtime_v2.to_string()),
        )]
        .into_iter()
        .collect(),
    );
    assert!(registry.get_agent(runtime_v1, Some(&workspace)).is_some());
    assert_eq!(
        binding
            .lease
            .as_ref()
            .expect("old generation remains leased")
            .model_binding()
            .configuration_fingerprint,
        "model-config-v1"
    );

    registry.install_external_subagent_routes(
        &workspace,
        Vec::new(),
        [("Explore".to_string(), ExternalSubagentRoute::Unavailable)]
            .into_iter()
            .collect(),
    );
    assert!(registry.get_agent(runtime_v1, Some(&workspace)).is_some());
    assert!(registry.get_agent(runtime_v2, Some(&workspace)).is_none());
    assert!(registry
        .resolve_subagent_for_fresh_invocation("Explore", Some(&workspace), true)
        .is_none());
    assert!(registry.is_external_subagent_route("Explore", Some(&workspace)));
    registry.install_external_subagent_routes(&workspace, Vec::new(), BTreeMap::new());
    assert!(registry
        .resolve_subagent_for_fresh_invocation("Explore", Some(&workspace), true)
        .is_none());
    assert!(registry.is_external_subagent_route("Explore", Some(&workspace)));
    drop(binding);
    assert!(registry.get_agent(runtime_v1, Some(&workspace)).is_none());
}
