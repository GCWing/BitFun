// Ratchet for legacy backend references that still live in the CLI TUI.
// Counts may decrease without updating this file. Adding a marker to a new
// file, moving debt between files, or increasing a count requires migration
// through CliAgentRuntimeClient or the existing owner/service API. TUI
// controllers may call those owners directly; no surface service wrapper is
// allowed.

export const tuiLegacyBackendMarkers = [
  'openbitfun_agent_runtime::',
  'openbitfun_services_',
  'openbitfun_runtime_services',
  'openbitfun_agent_runtime_ipc',
  'CliContextReloadClient',
  'CoreAgentRuntimeCompatibility',
  'crate::account_sync::',
  'get_mcp_service',
  'std::fs',
  'tokio::fs',
  'std::process::',
  'tokio::process',
  'reqwest::',
];

export const tuiLegacyBackendBudgets = {
  'src/apps/cli/src/modes/chat.rs': {
    'openbitfun_agent_runtime::': 3,
    CoreAgentRuntimeCompatibility: 3,
  },
  'src/apps/cli/src/modes/chat/commands.rs': { 'openbitfun_services_': 1 },
  'src/apps/cli/src/modes/chat/external_editor.rs': {
    'openbitfun_services_': 2,
    'std::fs': 3,
    'std::process::': 1,
  },
  'src/apps/cli/src/modes/chat/account.rs': {
    'crate::account_sync::': 0,
  },
  'src/apps/cli/src/modes/chat/provider_models.rs': {
    'crate::account_sync::': 0,
  },
  'src/apps/cli/src/modes/chat/run.rs': {
    'openbitfun_agent_runtime::': 3,
    'openbitfun_services_': 1,
    'crate::account_sync::': 1,
  },
  'src/apps/cli/src/modes/chat/session_lineage.rs': { 'openbitfun_agent_runtime::': 1 },
  'src/apps/cli/src/modes/chat/selection.rs': {
  },
  'src/apps/cli/src/modes/chat/tests.rs': {
    'openbitfun_agent_runtime::': 3,
  },
  'src/apps/cli/src/ui/chat/popups.rs': {
    'openbitfun_agent_runtime::': 2,
    'crate::account_sync::': 2,
  },
  'src/apps/cli/src/ui/chat/input.rs': { 'openbitfun_agent_runtime::': 4 },
  'src/apps/cli/src/ui/chat/state.rs': { 'openbitfun_agent_runtime::': 1 },
  'src/apps/cli/src/ui/composer.rs': { 'openbitfun_agent_runtime::': 2 },
  'src/apps/cli/src/ui/image_paste.rs': { 'std::fs': 5 },
  'src/apps/cli/src/ui/login_form.rs': {
    'crate::account_sync::': 1,
  },
  'src/apps/cli/src/ui/prompt_command_shell_review.rs': {},
  'src/apps/cli/src/ui/permission.rs': { 'openbitfun_agent_runtime::': 2 },
  'src/apps/cli/src/ui/session_lineage_selector.rs': { 'openbitfun_agent_runtime::': 1 },
  'src/apps/cli/src/ui/startup.rs': {
    CoreAgentRuntimeCompatibility: 0,
    'crate::account_sync::': 0,
  },
  'src/apps/cli/src/ui/workspace_diff.rs': { 'openbitfun_agent_runtime::': 2 },
  'src/apps/cli/src/ui/workspace_reference.rs': { 'openbitfun_agent_runtime::': 1 },
};

export const tuiForbiddenSurfaceServiceMarkers = [
  'surface_services',
  'Arc<dyn ModelService>',
  'Arc<dyn SkillService>',
  'Arc<dyn SubagentService>',
  'Arc<dyn McpService>',
  'Arc<dyn ExternalSourceService>',
  'Arc<dyn HookService>',
  'Arc<dyn AccountService>',
  'Arc<dyn WorktreeService>',
];
