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
  useLayoutEffect,
  useRef,
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
  KeyboardShortcutsTab,
  McpToolsConfig,
  MemoriesConfig,
  QuickActionsConfig,
  ReviewConfig,
  SessionPermissionsConfig,
  SessionPersonalizationConfig,
  VoiceInputConfig,
} from './settingsContentRegistry';
import './SettingsScene.scss';

// Keep in sync with settings-content-exit in SettingsScene.scss.
const SETTINGS_CONTENT_EXIT_DURATION_MS = 180;

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
    case 'session-personalization': return SessionPersonalizationConfig;
    case 'session-permissions':     return SessionPermissionsConfig;
    case 'quick-actions':           return QuickActionsConfig;
    case 'voice-input':             return VoiceInputConfig;
    case 'review':                  return ReviewConfig;
    case 'memories':                return MemoriesConfig;
    case 'mcp-tools':               return McpToolsConfig;
    case 'external-sources':        return ExternalSourcesConfig;
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
  const [outgoingTab, setOutgoingTab] = useState<ConfigTab | null>(null);
  const previousTabRef = useRef<ConfigTab>(resolvedTab);

  useEffect(() => {
    /** Legacy merged session settings tab removed in favor of two panels. */
    if ((activeTab as string) === 'session-config') {
      setActiveTab('session-personalization');
    }
  }, [activeTab, setActiveTab]);

  // Derive the previous tab during render so React keeps its keyed subtree
  // mounted in the same commit that introduces the incoming page.
  const renderedOutgoingTab = previousTabRef.current !== resolvedTab
    ? previousTabRef.current
    : outgoingTab;

  useLayoutEffect(() => {
    const previousTab = previousTabRef.current;
    previousTabRef.current = resolvedTab;
    if (previousTab === resolvedTab) return;

    setOutgoingTab(previousTab);
    const exitTimer = window.setTimeout(() => {
      setOutgoingTab(current => current === previousTab ? null : current);
    }, SETTINGS_CONTENT_EXIT_DURATION_MS);

    return () => window.clearTimeout(exitTimer);
  }, [resolvedTab]);

  const renderedTabs: ConfigTab[] = [resolvedTab];
  if (renderedOutgoingTab && renderedOutgoingTab !== resolvedTab) {
    renderedTabs.push(renderedOutgoingTab);
  }

  return (
    <div className="bitfun-settings-scene" data-testid="settings-scene" data-settings-tab={resolvedTab}>
      <div className="bitfun-settings-scene__content-stack">
        {renderedTabs.map(tab => {
          const Content = resolveSettingsContent(tab);
          if (!Content) return null;

          const isActive = tab === resolvedTab;
          const isOutgoing = !isActive && tab === renderedOutgoingTab;
          return (
            <div
              key={tab}
              className={[
                'bitfun-settings-scene__content-wrapper',
                isActive && 'bitfun-settings-scene__content-wrapper--active',
                isActive && renderedOutgoingTab && 'bitfun-settings-scene__content-wrapper--entering',
                isOutgoing && 'bitfun-settings-scene__content-wrapper--outgoing',
              ].filter(Boolean).join(' ')}
              aria-hidden={!isActive}
              data-testid="settings-scene-content"
              data-settings-panel={tab}
              data-settings-panel-active={isActive ? 'true' : 'false'}
            >
              <Suspense fallback={isActive ? <SettingsSceneLoading /> : null}>
                <Content />
              </Suspense>
            </div>
          );
        })}
      </div>
    </div>
  );
};

export default SettingsScene;
