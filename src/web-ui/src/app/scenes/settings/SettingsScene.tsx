/**
 * SettingsScene — self-contained settings page with internal left-right layout.
 *
 * Previous design: SettingsNav was injected into the outer NavPanel via nav-registry.
 * New design: SettingsNav is embedded directly inside this scene, forming a
 * standalone left-right layout that does not depend on the outer navigation shell.
 *
 * Layout:
 *   ┌──────────────────────────────────────────────────┐
 *   │ SettingsNav (220px) │ SettingsContent (flex:1)    │
 *   │   search            │   BasicsConfig /            │
 *   │   category list     │   AIModelConfig / …         │
 *   └──────────────────────────────────────────────────┘
 */

import React, { lazy, Suspense } from 'react';
import { useSettingsStore } from './settingsStore';
import SettingsNav from './SettingsNav';
import './SettingsScene.scss';
import AIModelConfig from '../../../infrastructure/config/components/AIModelConfig';
import SessionConfig from '../../../infrastructure/config/components/SessionConfig';
import EditorConfig from '../../../infrastructure/config/components/EditorConfig';
import BasicsConfig from '../../../infrastructure/config/components/BasicsConfig';

const KeyboardShortcutsTab = lazy(() => import('./components/KeyboardShortcutsTab'));

const SettingsScene: React.FC = () => {
  const activeTab = useSettingsStore(s => s.activeTab);

  let Content: React.ComponentType | null = null;

  if (activeTab === 'keyboard') {
    Content = () => (
      <Suspense fallback={null}>
        <KeyboardShortcutsTab />
      </Suspense>
    );
  } else {
    switch (activeTab) {
      case 'basics':           Content = BasicsConfig;     break;
      case 'models':           Content = AIModelConfig;    break;
      case 'session-config':   Content = SessionConfig;    break;
      case 'editor':           Content = EditorConfig;     break;
    }
  }

  return (
    <div className="bitfun-settings-scene">
      {/* Left: settings navigation (embedded, not injected via nav-registry) */}
      <div className="bitfun-settings-scene__nav">
        <SettingsNav />
      </div>

      {/* Right: active settings content */}
      <div className="bitfun-settings-scene__content">
        {Content && (
          <div key={activeTab} className="bitfun-settings-scene__content-wrapper">
            <Content />
          </div>
        )}
      </div>
    </div>
  );
};

export default SettingsScene;
