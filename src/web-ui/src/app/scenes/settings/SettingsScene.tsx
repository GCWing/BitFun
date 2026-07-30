/**
 * SettingsScene — content-only renderer for the Settings scene.
 *
 * The left-side navigation lives in SettingsNav (rendered by NavPanel via
 * nav-registry). This component only renders the active config content panel
 * driven by settingsStore.activeTab.
 */

import React, {
  Suspense,
  useEffect,
  useState,
} from 'react';
import { useSettingsStore } from './settingsStore';
import type { ConfigTab } from './settingsConfig';
import {
  AcpAgentsConfig,
  AIModelConfig,
  AppearanceConfig,
  ArchivedSessionsConfig,
  BasicsConfig,
  EditorConfig,
  ExternalSourcesConfig,
  HooksConfig,
  KeyboardShortcutsTab,
  McpToolsConfig,
  MemoriesConfig,
  QuickActionsConfig,
  ReviewConfig,
  SessionPermissionsConfig,
  SessionPersonalizationConfig,
  VoiceInputConfig,
  WorktreesConfig,
  isSettingsTabContentReady,
  preloadSettingsTabContent,
} from './settingsContentRegistry';
import './SettingsScene.scss';

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

function resolveSettingsContent(tab: ConfigTab): React.ComponentType | null {
  switch (tab) {
    case 'basics':                  return BasicsConfig;
    case 'appearance':              return AppearanceConfig;
    case 'models':                  return AIModelConfig;
    case 'archived-sessions':       return ArchivedSessionsConfig;
    case 'worktrees':               return WorktreesConfig;
    case 'session-personalization': return SessionPersonalizationConfig;
    case 'session-permissions':     return SessionPermissionsConfig;
    case 'quick-actions':           return QuickActionsConfig;
    case 'voice-input':             return VoiceInputConfig;
    case 'review':                  return ReviewConfig;
    case 'memories':                return MemoriesConfig;
    case 'mcp-tools':               return McpToolsConfig;
    case 'external-sources':        return ExternalSourcesConfig;
    case 'hooks':                   return HooksConfig;
    case 'acp-agents':              return AcpAgentsConfig;
    case 'editor':                  return EditorConfig;
    case 'keyboard':                return KeyboardShortcutsTab;
    default:                        return null;
  }
}

const SettingsScene: React.FC = () => {
  const activeTab = useSettingsStore(s => s.activeTab);
  const setActiveTab = useSettingsStore(s => s.setActiveTab);

  const resolvedTab: ConfigTab =
    (activeTab as string) === 'session-config' ? 'session-personalization' : activeTab;

  useEffect(() => {
    /** Legacy merged session settings tab removed in favor of two panels. */
    if ((activeTab as string) === 'session-config') {
      setActiveTab('session-personalization');
    }
  }, [activeTab, setActiveTab]);

  /**
   * Cold entries into the scene (first open after launch, deep links) mount a
   * panel whose chunk and i18n namespaces are still in flight, which paints the
   * skeleton and then a frame of raw i18n keys. Hold the first paint until those
   * resources land — an empty content area for a few ms reads as instant, a
   * three-stage flash does not. SettingsNav preloads before it flips the active
   * tab, so tab switches are never gated here.
   */
  const [firstPaintReady, setFirstPaintReady] = useState(() =>
    isSettingsTabContentReady(resolvedTab)
  );

  useEffect(() => {
    if (firstPaintReady) return;

    let cancelled = false;
    const commit = () => {
      if (!cancelled) setFirstPaintReady(true);
    };
    void preloadSettingsTabContent(resolvedTab).then(commit, commit);

    return () => {
      cancelled = true;
    };
  }, [firstPaintReady, resolvedTab]);

  const Content = firstPaintReady ? resolveSettingsContent(resolvedTab) : null;

  return (
    <div className="bitfun-settings-scene" data-testid="settings-scene" data-settings-tab={resolvedTab}>
      <div className="bitfun-settings-scene__content-stack">
        {Content ? (
          <div
            key={resolvedTab}
            className="bitfun-settings-scene__content-wrapper"
            data-testid="settings-scene-content"
            data-settings-panel={resolvedTab}
            data-settings-panel-active="true"
          >
            <Suspense fallback={<SettingsSceneLoading />}>
              <Content />
            </Suspense>
          </div>
        ) : null}
      </div>
    </div>
  );
};

export default SettingsScene;
