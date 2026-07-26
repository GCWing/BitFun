use crate::agentic::agents::{
    get_agent_registry, PromptBuilder, SubagentListScope, SubagentQueryContext,
    ToolListingSections, UserContextPolicy,
};
use crate::agentic::tools::implementations::skills::{get_skill_registry, SkillInfo};
use crate::agentic::tools::manifest_resolver::{resolve_tool_manifest, ResolvedToolManifest};
use crate::agentic::tools::product_runtime::GetToolSpecTool;
use crate::agentic::tools::tool_context_runtime;
use crate::agentic::tools::ToolRuntimeRestrictions;
use crate::agentic::workspace::WorkspaceServices;
use crate::agentic::WorkspaceBinding;
pub use bitfun_agent_runtime::skill_agent_snapshot::{
    build_skill_agent_tool_listing_sections_from_snapshot, diff_skill_agent_snapshot,
    render_full_agent_listing_body, render_full_skill_listing_body, AgentSnapshotEntry,
    SkillAgentDiff, SkillSnapshotEntry, TurnSkillAgentSnapshot, TurnSkillAgentSnapshotStore,
};

#[derive(Debug, Clone)]
pub struct SkillAgentSnapshotResolution {
    pub snapshot: TurnSkillAgentSnapshot,
    pub tool_listing_sections: ToolListingSections,
}

pub async fn resolve_skill_agent_snapshot(
    agent_type: &str,
    workspace: Option<&WorkspaceBinding>,
    workspace_services: Option<&WorkspaceServices>,
    enable_tools: bool,
    context_vars: &std::collections::HashMap<String, String>,
    runtime_tool_restrictions: &ToolRuntimeRestrictions,
) -> SkillAgentSnapshotResolution {
    if !enable_tools {
        return SkillAgentSnapshotResolution {
            snapshot: TurnSkillAgentSnapshot::default(),
            tool_listing_sections: ToolListingSections::default(),
        };
    }

    let agent_registry = get_agent_registry();
    agent_registry
        .load_custom_agents(
            workspace
                .filter(|binding| !binding.is_remote())
                .map(|binding| binding.root_path()),
        )
        .await;

    let tool_policy = agent_registry
        .get_agent_tool_policy(agent_type, workspace.map(|binding| binding.root_path()))
        .await;

    let tool_description_context = tool_context_runtime::build_tool_description_context(
        agent_type,
        workspace,
        workspace_services,
        None,
        context_vars,
        runtime_tool_restrictions,
    );
    let manifest = resolve_tool_manifest(
        &tool_policy.allowed_tools,
        &tool_policy.exposure_overrides,
        &tool_description_context,
    )
    .await;

    let snapshot = build_skill_agent_snapshot(
        workspace,
        workspace_services,
        agent_type,
        &manifest,
        runtime_tool_restrictions,
    )
    .await;
    let tool_listing_sections = build_tool_listing_sections(&manifest, &snapshot);

    SkillAgentSnapshotResolution {
        snapshot,
        tool_listing_sections,
    }
}

async fn build_skill_agent_snapshot(
    workspace: Option<&WorkspaceBinding>,
    workspace_services: Option<&WorkspaceServices>,
    agent_type: &str,
    manifest: &ResolvedToolManifest,
    runtime_tool_restrictions: &ToolRuntimeRestrictions,
) -> TurnSkillAgentSnapshot {
    let has_tool = |tool_name: &str| {
        manifest
            .tool_definitions
            .iter()
            .any(|definition| definition.name == tool_name)
    };

    let mut snapshot = TurnSkillAgentSnapshot::default();

    if has_tool("Skill") {
        snapshot.skills = load_skill_entries(workspace, workspace_services, Some(agent_type)).await;
    }

    if has_tool("Task") {
        snapshot.subagents =
            load_subagent_entries(workspace, Some(agent_type), runtime_tool_restrictions).await;
    }

    snapshot
}

fn build_tool_listing_sections(
    manifest: &ResolvedToolManifest,
    snapshot: &TurnSkillAgentSnapshot,
) -> ToolListingSections {
    let has_tool = |tool_name: &str| {
        manifest
            .tool_definitions
            .iter()
            .any(|definition| definition.name == tool_name)
    };

    ToolListingSections {
        skill_listing: has_tool("Skill")
            .then(|| render_full_skill_listing_body(&snapshot.skills))
            .filter(|body| !body.is_empty()),
        agent_listing: has_tool("Task")
            .then(|| render_full_agent_listing_body(&snapshot.subagents))
            .filter(|body| !body.is_empty()),
        deferred_tool_listing: if has_tool("GetToolSpec") {
            GetToolSpecTool::build_deferred_tools_context_section(&manifest.deferred_tool_summaries)
        } else {
            None
        },
    }
}

async fn load_skill_entries(
    workspace: Option<&WorkspaceBinding>,
    workspace_services: Option<&WorkspaceServices>,
    agent_type: Option<&str>,
) -> Vec<SkillSnapshotEntry> {
    let registry = get_skill_registry();
    let skills = match workspace {
        Some(workspace) if workspace.is_remote() => {
            if let Some(services) = workspace_services {
                registry
                    .get_resolved_skills_for_remote_workspace(
                        services.fs.as_ref(),
                        &workspace.root_path_string(),
                        agent_type,
                    )
                    .await
            } else {
                Vec::new()
            }
        }
        Some(workspace) => {
            registry
                .get_resolved_skills_for_workspace(Some(workspace.root_path()), agent_type)
                .await
        }
        None => {
            registry
                .get_resolved_skills_for_workspace(None, agent_type)
                .await
        }
    };

    skills
        .into_iter()
        .map(skill_snapshot_entry_from_skill_info)
        .collect()
}

fn skill_snapshot_entry_from_skill_info(skill: SkillInfo) -> SkillSnapshotEntry {
    SkillSnapshotEntry {
        name: skill.name,
        description: skill.description,
        location: skill.path,
    }
}

async fn load_subagent_entries(
    workspace: Option<&WorkspaceBinding>,
    agent_type: Option<&str>,
    runtime_tool_restrictions: &ToolRuntimeRestrictions,
) -> Vec<AgentSnapshotEntry> {
    let registry = get_agent_registry();
    let workspace_root = workspace
        .filter(|workspace| !workspace.is_remote())
        .map(|workspace| workspace.root_path());
    let agents = registry
        .get_subagents_for_query(&SubagentQueryContext {
            parent_agent_type: agent_type,
            workspace_root,
            list_scope: SubagentListScope::TaskVisible,
            include_disabled: false,
            external_sources_supported: false,
        })
        .await;

    agents
        .into_iter()
        .map(|agent| {
            let default_tools = agent
                .default_tools
                .into_iter()
                .filter(|tool_name| runtime_tool_restrictions.is_tool_allowed(tool_name))
                .collect();
            AgentSnapshotEntry {
                id: agent.id,
                description: agent.description,
                default_tools,
            }
        })
        .collect()
}

pub async fn build_embedded_user_context_reminder(
    workspace: Option<&WorkspaceBinding>,
    workspace_id: Option<&str>,
    session_id: &str,
    user_context_policy: &UserContextPolicy,
) -> Option<String> {
    let workspace = workspace?;
    let context = crate::agentic::agents::build_prompt_context_for_workspace(
        workspace,
        workspace_id,
        session_id,
        None,
        None,
        ToolListingSections::default(),
        Default::default(),
    )
    .await?;
    PromptBuilder::new(context)
        .build_user_context_reminder(user_context_policy)
        .await
}

#[cfg(test)]
mod tests {
    use super::load_subagent_entries;
    use crate::agentic::tools::ToolRuntimeRestrictions;

    #[tokio::test]
    async fn subagent_projection_hides_runtime_denied_tools() {
        let mut restrictions = ToolRuntimeRestrictions::default();
        restrictions
            .denied_tool_names
            .insert("ControlHub".to_string());

        let agents = load_subagent_entries(None, Some("Claw"), &restrictions).await;
        let computer_use = agents
            .iter()
            .find(|agent| agent.id == "ComputerUse")
            .expect("Claw should advertise the ComputerUse subagent");

        assert!(!computer_use
            .default_tools
            .iter()
            .any(|tool_name| !restrictions.is_tool_allowed(tool_name)));
    }
}
