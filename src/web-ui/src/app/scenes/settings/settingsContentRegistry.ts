import { lazy } from 'react';
import type { ConfigTab } from './settingsConfig';

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
  'session-personalization': loadSessionConfig,
  'session-permissions': loadSessionConfig,
  'quick-actions': loadQuickActionsConfig,
  'voice-input': loadVoiceInputConfig,
  review: loadReviewConfig,
  memories: loadMemoriesConfig,
  'mcp-tools': loadMcpToolsConfig,
  'external-sources': loadExternalSourcesConfig,
  'acp-agents': loadAcpAgentsConfig,
  editor: loadEditorConfig,
  keyboard: loadKeyboardShortcutsTab,
};

export async function preloadSettingsTabContent(tab: ConfigTab): Promise<void> {
  await SETTINGS_CONTENT_LOADERS[tab]?.();
}
