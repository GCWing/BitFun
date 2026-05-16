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
import AIModelConfig from '../../../infrastructure/config/components/AIModelConfig';
import {
  SessionPersonalizationConfig,
  SessionPermissionsConfig,
} from '../../../infrastructure/config/components/SessionConfig';
import McpToolsConfig from '../../../infrastructure/config/components/McpToolsConfig';
import AcpAgentsConfig from '../../../infrastructure/config/components/AcpAgentsConfig';
import EditorConfig from '../../../infrastructure/config/components/EditorConfig';
import BasicsConfig from '../../../infrastructure/config/components/BasicsConfig';
import AppearanceConfig from '../../../infrastructure/config/components/AppearanceConfig';
import ReviewConfig from '../../../infrastructure/config/components/ReviewConfig';
import QuickActionsConfig from '../../../infrastructure/config/components/QuickActionsConfig';

const KeyboardShortcutsTab = lazy(() => import('./components/KeyboardShortcutsTab'));

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

  if (resolvedTab === 'keyboard') {
    return (
      <div className="bitfun-settings-scene">
        <div key="keyboard" className="bitfun-settings-scene__content-wrapper">
          <Suspense fallback={null}>
            <KeyboardShortcutsTab />
          </Suspense>
        </div>
      </div>
    );
  }

  let Content: React.ComponentType | null = null;

  switch (resolvedTab) {
    case 'basics':           Content = BasicsConfig;         break;
    case 'appearance':       Content = AppearanceConfig;     break;
    case 'models':           Content = AIModelConfig;        break;
    case 'session-personalization': Content = SessionPersonalizationConfig; break;
    case 'session-permissions':     Content = SessionPermissionsConfig;     break;
    case 'quick-actions':    Content = QuickActionsConfig;   break;
    case 'review':           Content = ReviewConfig;         break;
    case 'mcp-tools':        Content = McpToolsConfig;      break;
    case 'acp-agents':       Content = AcpAgentsConfig;     break;
    case 'editor':           Content = EditorConfig;         break;
  }

  return (
    <div className="bitfun-settings-scene">
      {Content && (
        <div key={resolvedTab} className="bitfun-settings-scene__content-wrapper">
          <Content />
        </div>
      )}
    </div>
  );
};

export default SettingsScene;
