import { DEFAULT_SETTINGS_PAGE_ID, isSettingsPageId } from './settingsRegistry';
import type { SettingsDestination } from './settingsTypes';

const LEGACY_ECOSYSTEM_COMPATIBILITY_IDS = new Set([
  'external-sources',
  'tools.integrations',
]);

/** Old Settings links now redirect to the owning product surface. */
export function isLegacyEcosystemCompatibilityDestination(value: unknown): boolean {
  if (typeof value === 'string') {
    return LEGACY_ECOSYSTEM_COMPATIBILITY_IDS.has(value);
  }
  if (!value || typeof value !== 'object' || !('pageId' in value)) return false;
  return typeof value.pageId === 'string'
    && LEGACY_ECOSYSTEM_COMPATIBILITY_IDS.has(value.pageId);
}

/**
 * Upgrade boundary for links emitted by older installs, extensions, and peers.
 * Product code must use canonical SettingsDestination values directly; legacy
 * identifiers are intentionally contained in this one reader-only table.
 */
const LEGACY_DESTINATION_MIGRATIONS: Readonly<Record<string, SettingsDestination>> = {
  basics: { pageId: 'application.general' },
  appearance: { pageId: 'application.appearance' },
  font: { pageId: 'application.appearance' },
  fonts: { pageId: 'application.appearance' },
  'session-personalization': { pageId: 'application.pet' },
  'application.input': { pageId: 'application.voice' },
  keyboard: { pageId: 'application.shortcuts' },
  shortcuts: { pageId: 'application.shortcuts' },
  keybindings: { pageId: 'application.shortcuts' },
  hotkeys: { pageId: 'application.shortcuts' },
  'voice-input': { pageId: 'application.voice' },
  'application.development': { pageId: 'application.terminal' },
  editor: { pageId: 'application.editor' },
  terminal: { pageId: 'application.terminal' },
  models: { pageId: 'ai.models' },
  memories: { pageId: 'ai.memory' },
  'session-config': { pageId: 'workspace.session' },
  'session-permissions': { pageId: 'tools.execution' },
  'tools.device-control': { pageId: 'tools.desktop-control' },
  'tools.browser-control': { pageId: 'tools.desktop-control' },
  review: { pageId: 'tools.execution' },
  'deep-review': { pageId: 'tools.execution' },
  'code-review': { pageId: 'tools.execution' },
  'review-team': { pageId: 'tools.execution' },
  worktrees: { pageId: 'workspace.worktrees' },
  'quick-actions': { pageId: 'tools.automation', viewId: 'quick-actions' },
  hooks: { pageId: 'tools.automation', viewId: 'hooks' },
  'mcp-tools': { pageId: 'tools.mcp' },
  'acp-agents': { pageId: 'tools.acp' },
  'usage-statistics': { pageId: 'data.usage' },
  'archived-sessions': { pageId: 'data.archived' },
  'data.history': { pageId: 'data.usage' },
  logging: { pageId: 'data.diagnostics' },
};

export function resolveSettingsDestination(value: string): SettingsDestination {
  if (isSettingsPageId(value)) return { pageId: value };
  return LEGACY_DESTINATION_MIGRATIONS[value] ?? { pageId: DEFAULT_SETTINGS_PAGE_ID };
}
