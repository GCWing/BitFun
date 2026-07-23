/**
 * SettingsScene — content-only renderer for the Settings scene.
 *
 * The left-side navigation lives in SettingsNav (rendered by NavPanel via
 * nav-registry). This component only renders the active config content panel
 * driven by settingsStore.activeTab.
 */

import React, { lazy, Suspense, useEffect } from 'react';
import { useSettingsStore } from './settingsStore';
import './SettingsScene.scss';

const AIModelConfig = lazy(() => import('../../../infrastructure/config/components/AIModelConfig'));
const McpToolsConfig = lazy(() => import('../../../infrastructure/config/components/McpToolsConfig'));
const AcpAgentsConfig = lazy(() => import('../../../infrastructure/config/components/AcpAgentsConfig'));
const ExternalSourcesConfig = lazy(() => import('../../../infrastructure/config/components/ExternalSourcesConfig'));
const EditorConfig = lazy(() => import('../../../infrastructure/config/components/EditorConfig'));
const BasicsConfig = lazy(() => import('../../../infrastructure/config/components/BasicsConfig'));
const AppearanceConfig = lazy(() => import('../../../infrastructure/config/components/AppearanceConfig'));
const ReviewConfig = lazy(() => import('../../../infrastructure/config/components/ReviewConfig'));
const MemoriesConfig = lazy(() => import('../../../infrastructure/config/components/MemoriesConfig'));
const QuickActionsConfig = lazy(() => import('../../../infrastructure/config/components/QuickActionsConfig'));
const ArchivedSessionsConfig = lazy(() => import('./components/ArchivedSessionsConfig'));
const KeyboardShortcutsTab = lazy(() => import('./components/KeyboardShortcutsTab'));
const SessionPersonalizationConfig = lazy(() =>
  import('../../../infrastructure/config/components/SessionConfig').then((module) => ({
    default: module.SessionPersonalizationConfig,
  }))
);
const SessionPermissionsConfig = lazy(() =>
  import('../../../infrastructure/config/components/SessionConfig').then((module) => ({
    default: module.SessionPermissionsConfig,
  }))
);

function SettingsSceneLoading() {
  return (
    <div className="bitfun-settings-scene__loading" aria-busy="true" aria-hidden="true">
      <div className="bitfun-settings-scene__loading-line bitfun-settings-scene__loading-line--title" />
      <div className="bitfun-settings-scene__loading-line" />
      <div className="bitfun-settings-scene__loading-line" />
      <div className="bitfun-settings-scene__loading-block" />
    </div>
  );
}

const SettingsScene: React.FC = () => {
  const activeTab = useSettingsStore(s => s.activeTab);
  const setActiveTab = useSettingsStore(s => s.setActiveTab);

  const resolvedTab: typeof activeTab =
    (activeTab as string) === 'session-config' ? 'session-personalization' : activeTab;

  useEffect(() => {
    /** Legacy merged session settings tab removed in favor of two panels. */
    if ((activeTab as string) === 'session-config') {
      setActiveTab('session-personalization');
    }
  }, [activeTab, setActiveTab]);

  let Content: React.ComponentType | null = null;

  switch (resolvedTab) {
    case 'basics':           Content = BasicsConfig;         break;
    case 'appearance':       Content = AppearanceConfig;     break;
    case 'models':           Content = AIModelConfig;        break;
    case 'archived-sessions': Content = ArchivedSessionsConfig; break;
    case 'session-personalization': Content = SessionPersonalizationConfig; break;
    case 'session-permissions':     Content = SessionPermissionsConfig;     break;
    case 'quick-actions':    Content = QuickActionsConfig;   break;
    case 'review':           Content = ReviewConfig;         break;
    case 'memories':         Content = MemoriesConfig;       break;
    case 'mcp-tools':        Content = McpToolsConfig;      break;
    case 'external-sources': Content = ExternalSourcesConfig; break;
    case 'acp-agents':       Content = AcpAgentsConfig;     break;
    case 'editor':           Content = EditorConfig;         break;
    case 'keyboard':         Content = KeyboardShortcutsTab; break;
  }

  return (
    <div className="bitfun-settings-scene" data-testid="settings-scene" data-settings-tab={resolvedTab}>
      {Content && (
        <div
          key={resolvedTab}
          className="bitfun-settings-scene__content-wrapper"
          data-testid="settings-scene-content"
        >
          <Suspense fallback={<SettingsSceneLoading />}>
            <Content />
          </Suspense>
        </div>
      )}
    </div>
  );
};

export default SettingsScene;
