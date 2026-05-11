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
  'settings/agents',
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
  'tools',
] as const;
