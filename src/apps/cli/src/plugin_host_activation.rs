use std::path::PathBuf;

use bitfun_core::BitFunResult;
use bitfun_runtime_ports::AgentSessionWorkspaceBinding;

#[derive(Debug, Clone, PartialEq, Eq)]
struct PluginWorkspaceActivationTarget {
    directory: PathBuf,
    worktree: PathBuf,
    project_id: Option<String>,
}

fn activation_target(
    binding: &AgentSessionWorkspaceBinding,
) -> Option<PluginWorkspaceActivationTarget> {
    if binding.remote_connection_id.is_some() || binding.remote_ssh_host.is_some() {
        return None;
    }

    let workspace = PathBuf::from(&binding.workspace_path);
    Some(PluginWorkspaceActivationTarget {
        directory: workspace.clone(),
        worktree: workspace,
        project_id: binding.workspace_id.clone(),
    })
}

pub(crate) async fn ensure_configured_plugin_execution_supported() -> BitFunResult<bool> {
    // Plugin activation is optional. Isolated Runtime clients (including unit
    // tests) may create sessions before the process-level config service is
    // initialized; that must not make ordinary session creation fail.
    if !bitfun_core::service::config::GlobalConfigManager::is_initialized() {
        return Ok(false);
    }

    let config_service = bitfun_core::service::config::get_global_config_service().await?;
    let config: bitfun_core::service::config::GlobalConfig =
        config_service.get_config(None).await?;
    let has_configured_plugins = config.has_configured_plugins();
    if has_configured_plugins
        && crate::PLUGIN_HOST_LAUNCH_POLICY
            == bitfun_core::plugin_host::PluginHostLaunchPolicy::Disabled
    {
        return Err(bitfun_core::BitFunError::NotImplemented(
            "Configured Plugin Host execution is not enabled; plugin definitions were not imported or executed"
            .to_string(),
        ));
    }
    Ok(has_configured_plugins)
}

pub(crate) async fn ensure_plugin_workspace_ready(
    binding: &AgentSessionWorkspaceBinding,
) -> BitFunResult<()> {
    if !ensure_configured_plugin_execution_supported().await? {
        return Ok(());
    }

    let Some(target) = activation_target(binding) else {
        return Err(bitfun_core::BitFunError::NotImplemented(
            "Configured Plugin Host is unsupported for Remote CLI workspaces; no controller-local fallback was attempted"
                .to_string(),
        ));
    };

    bitfun_core::plugin_host::ensure_configured_plugin_instance(
        crate::PLUGIN_HOST_LAUNCH_POLICY,
        target.directory,
        target.worktree,
        target.project_id,
        serde_json::Map::new(),
    )
    .await
    .map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::{activation_target, PluginWorkspaceActivationTarget};
    use bitfun_runtime_ports::{AgentSessionWorkspaceBinding, SessionExecutionTarget};
    use std::path::PathBuf;

    fn binding() -> AgentSessionWorkspaceBinding {
        AgentSessionWorkspaceBinding {
            workspace_id: Some("workspace-1".to_string()),
            workspace_path: "C:/workspace/project".to_string(),
            project_workspace_path: Some("C:/workspace/project".to_string()),
            execution_target: Some(SessionExecutionTarget::local("C:/workspace/project")),
            remote_connection_id: None,
            remote_ssh_host: None,
        }
    }

    #[test]
    fn local_binding_maps_to_plugin_workspace_target() {
        assert_eq!(
            activation_target(&binding()),
            Some(PluginWorkspaceActivationTarget {
                directory: PathBuf::from("C:/workspace/project"),
                worktree: PathBuf::from("C:/workspace/project"),
                project_id: Some("workspace-1".to_string()),
            })
        );
    }

    #[test]
    fn remote_binding_skips_local_plugin_host() {
        let mut binding = binding();
        binding.remote_connection_id = Some("remote-1".to_string());

        assert_eq!(activation_target(&binding), None);
    }

    #[test]
    fn cli_does_not_enable_unowned_plugin_execution() {
        assert_eq!(
            crate::PLUGIN_HOST_LAUNCH_POLICY,
            bitfun_core::plugin_host::PluginHostLaunchPolicy::Disabled
        );
    }
}
