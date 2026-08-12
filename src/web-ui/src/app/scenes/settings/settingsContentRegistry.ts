import { lazy } from 'react';
import type { ConfigTab } from './settingsConfig';
import { preloadSettingsTabI18n } from './settingsTabI18n';

const loadAIModelConfig = () => import('../../../infrastructure/config/components/AIModelConfig');
const loadMcpToolsConfig = () => import('../../../infrastructure/config/components/McpToolsConfig');
const loadAcpAgentsConfig = () => import('../../../infrastructure/config/components/AcpAgentsConfig');
const loadExternalSourcesConfig = () => import('../../../infrastructure/config/components/ExternalSourcesConfig');
const loadEditorConfig = () => import('../../../infrastructure/config/components/EditorConfig');
const loadBasicsConfig = () => import('../../../infrastructure/config/components/BasicsConfig');
const loadAppearanceConfig = () => import('../../../infrastructure/config/components/AppearanceConfig');
const loadReviewConfig = () => import('../../../infrastructure/config/components/ReviewConfig');
const loadMemoriesConfig = () => import('../../../infrastructure/config/components/MemoriesConfig');
const loadQuickActionsConfig = () => import('../../../infrastructure/config/components/QuickActionsConfig');
const loadVoiceInputConfig = () => import('../../../infrastructure/config/components/VoiceInputConfig');
const loadArchivedSessionsConfig = () => import('./components/ArchivedSessionsConfig');
const loadWorktreesConfig = () => import('../../../infrastructure/config/components/WorktreesConfig');
const loadKeyboardShortcutsTab = () => import('./components/KeyboardShortcutsTab');
const loadSessionConfig = () => import('../../../infrastructure/config/components/SessionConfig');

export const AIModelConfig = lazy(loadAIModelConfig);
export const McpToolsConfig = lazy(loadMcpToolsConfig);
export const AcpAgentsConfig = lazy(loadAcpAgentsConfig);
export const ExternalSourcesConfig = lazy(loadExternalSourcesConfig);
export const EditorConfig = lazy(loadEditorConfig);
export const BasicsConfig = lazy(loadBasicsConfig);
export const AppearanceConfig = lazy(loadAppearanceConfig);
export const ReviewConfig = lazy(loadReviewConfig);
export const MemoriesConfig = lazy(loadMemoriesConfig);
export const QuickActionsConfig = lazy(loadQuickActionsConfig);
export const VoiceInputConfig = lazy(loadVoiceInputConfig);
export const ArchivedSessionsConfig = lazy(loadArchivedSessionsConfig);
export const WorktreesConfig = lazy(loadWorktreesConfig);
export const KeyboardShortcutsTab = lazy(loadKeyboardShortcutsTab);
export const SessionPersonalizationConfig = lazy(() =>
  loadSessionConfig().then((module) => ({
    default: module.SessionPersonalizationConfig,
  }))
);
export const SessionPermissionsConfig = lazy(() =>
  loadSessionConfig().then((module) => ({
    default: module.SessionPermissionsConfig,
  }))
);

const SETTINGS_CONTENT_LOADERS: Partial<Record<ConfigTab, () => Promise<unknown>>> = {
  basics: loadBasicsConfig,
  appearance: loadAppearanceConfig,
  models: loadAIModelConfig,
  'archived-sessions': loadArchivedSessionsConfig,
  worktrees: loadWorktreesConfig,
  'session-personalization': loadSessionConfig,
  'session-permissions': loadSessionConfig,
  'quick-actions': loadQuickActionsConfig,
  'voice-input': loadVoiceInputConfig,
  review: loadReviewConfig,
  memories: loadMemoriesConfig,
  'mcp-tools': loadMcpToolsConfig,
  'external-sources': loadExternalSourcesConfig,
  // Hooks no longer have a standalone GUI page; a stale deep link resolves to
  // the external AI applications surface where Hooks live as a capability.
  hooks: loadExternalSourcesConfig,
  'acp-agents': loadAcpAgentsConfig,
  editor: loadEditorConfig,
  keyboard: loadKeyboardShortcutsTab,
};

/**
 * Tabs whose chunk *and* i18n namespaces are already in memory. Rendering such a
 * tab is a plain synchronous mount: no Suspense fallback, no frame of raw i18n keys.
 */
const readyTabs = new Set<ConfigTab>();

export function isSettingsTabContentReady(tab: ConfigTab): boolean {
  return readyTabs.has(tab);
}

/**
 * Warm everything a tab needs to paint in a single frame: its lazy panel chunk
 * (with the CSS Vite ships alongside it) and its i18n namespaces. Callers await
 * this before switching tabs so the panel appears fully rendered instead of
 * stepping through skeleton → untranslated keys → content.
 */
export async function preloadSettingsTabContent(tab: ConfigTab): Promise<void> {
  if (readyTabs.has(tab)) return;

  await Promise.all([
    SETTINGS_CONTENT_LOADERS[tab]?.(),
    preloadSettingsTabI18n(tab),
  ]);

  readyTabs.add(tab);
}
