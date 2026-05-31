/**
 * Single namespace list consumed by i18next and the i18n audit.
 *
 * Keep this aligned with every locale folder under `src/web-ui/src/locales`.
 * Adding a namespace should require this file plus one JSON file per locale.
 */
export const ALL_NAMESPACES = [
  'common',
  'components',
  'errors',
  'flow-chat',
  'flow-chat/processing-hints',
  'notifications',
  'panels/files',
  'panels/git',
  'panels/terminal',
  'scenes/agents',
  'scenes/capabilities',
  'scenes/miniapp',
  'scenes/profile',
  'scenes/skills',
  'settings',
  'settings/acp-agents',
  'settings/agentic-tools',
  'settings/ai-features',
  'settings/ai-model',
  'settings/appearance',
  'settings/basics',
  'settings/debug',
  'settings/default-model',
  'settings/editor',
  'settings/lsp',
  'settings/mcp',
  'settings/mcp-tools',
  'settings/quick-actions',
  'settings/review',
  'settings/session-config',
  'settings/skills',
  'shared',
  'tools',
] as const;

export const WEB_UI_BOOTSTRAP_NAMESPACES = [
  'common',
  'components',
  'errors',
  'flow-chat',
  'panels/files',
  'panels/git',
  'settings/ai-model',
  'settings/lsp',
  'shared',
  'tools',
] as const satisfies readonly (typeof ALL_NAMESPACES)[number][];
