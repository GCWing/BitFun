// Ratchet for legacy backend references that still live in the CLI TUI.
// Counts may decrease without updating this file. Adding a marker to a new
// file, moving debt between files, or increasing a count requires migration
// through the CLI-local TuiBackend/App Server boundary instead.

export const tuiLegacyBackendMarkers = [
  'bitfun_core::',
  'bitfun_agent_runtime::',
  'bitfun_services_',
  'bitfun_runtime_services',
  'bitfun_agent_runtime_ipc',
  'CliAgentRuntimeClient',
  'CliContextReloadClient',
  'CoreAgentRuntimeCompatibility',
  'crate::account::',
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
    'bitfun_core::': 12,
    'bitfun_agent_runtime::': 3,
    CoreAgentRuntimeCompatibility: 3,
  },
  'src/apps/cli/src/modes/chat/commands.rs': { 'bitfun_services_': 1 },
  'src/apps/cli/src/modes/chat/external_editor.rs': {
    'bitfun_services_': 2,
    'std::fs': 3,
    'std::process::': 1,
  },
  'src/apps/cli/src/modes/chat/account.rs': {
    'crate::account::': 11,
    'crate::account_sync::': 5,
  },
  'src/apps/cli/src/modes/chat/external_hooks.rs': { 'bitfun_core::': 8 },
  'src/apps/cli/src/modes/chat/external_review.rs': { 'bitfun_core::': 5 },
  'src/apps/cli/src/modes/chat/mcp.rs': {
    'bitfun_core::': 38,
    get_mcp_service: 4,
  },
  'src/apps/cli/src/modes/chat/provider_models.rs': {
    'bitfun_core::': 4,
    'crate::account_sync::': 2,
  },
  'src/apps/cli/src/modes/chat/run.rs': {
    'bitfun_core::': 2,
    'bitfun_agent_runtime::': 3,
    'bitfun_services_': 1,
    'crate::account_sync::': 1,
  },
  'src/apps/cli/src/modes/chat/session_lineage.rs': { 'bitfun_agent_runtime::': 1 },
  'src/apps/cli/src/modes/chat/selection.rs': {
    'bitfun_core::': 7,
    'crate::account::': 2,
  },
  'src/apps/cli/src/modes/chat/tests.rs': {
    'bitfun_core::': 8,
    'bitfun_agent_runtime::': 3,
  },
  'src/apps/cli/src/modes/chat/worktree.rs': { 'bitfun_core::': 2 },
  'src/apps/cli/src/ui/chat/popups.rs': {
    'bitfun_core::': 1,
    'bitfun_agent_runtime::': 2,
    'crate::account::': 3,
    'crate::account_sync::': 2,
  },
  'src/apps/cli/src/ui/chat/input.rs': { 'bitfun_agent_runtime::': 4 },
  'src/apps/cli/src/ui/chat/state.rs': { 'bitfun_agent_runtime::': 1 },
  'src/apps/cli/src/ui/composer.rs': { 'bitfun_agent_runtime::': 2 },
  'src/apps/cli/src/ui/image_paste.rs': { 'std::fs': 5 },
  'src/apps/cli/src/ui/login_form.rs': {
    'crate::account::': 1,
    'crate::account_sync::': 1,
  },
  'src/apps/cli/src/ui/prompt_command_shell_review.rs': { 'bitfun_core::': 2 },
  'src/apps/cli/src/ui/permission.rs': { 'bitfun_agent_runtime::': 2 },
  'src/apps/cli/src/ui/session_lineage_selector.rs': { 'bitfun_agent_runtime::': 1 },
  'src/apps/cli/src/ui/startup.rs': {
    'bitfun_core::': 14,
    CoreAgentRuntimeCompatibility: 3,
    'crate::account::': 13,
    'crate::account_sync::': 8,
  },
  'src/apps/cli/src/ui/workspace_diff.rs': { 'bitfun_agent_runtime::': 2 },
  'src/apps/cli/src/ui/workspace_reference.rs': { 'bitfun_agent_runtime::': 1 },
};
