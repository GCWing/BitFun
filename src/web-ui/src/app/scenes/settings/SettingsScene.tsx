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
import { ViewTransitionBoundary } from '@/component-library';
import { useSettingsStore } from './settingsStore';
import { useExternalAppAwareness } from '@/infrastructure/config/components/external-sources';
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
  ThresholdsConfig,
  UsageStatisticsConfig,
  VoiceInputConfig,
  WorktreesConfig,
  isSettingsTabContentReady,
  preloadSettingsTabContent,
} from './settingsContentRegistry';
import './SettingsScene.scss';

function SettingsSceneLoading() {
  return (
    <div
      className="bitfun-settings-scene__loading"
      aria-busy="true"
      aria-hidden="true"
      data-bf-scene="settings"
      data-bf-part="loading"
    >
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
    case 'usage-statistics':        return UsageStatisticsConfig;
    case 'archived-sessions':       return ArchivedSessionsConfig;
    case 'worktrees':               return WorktreesConfig;
    case 'session-personalization': return SessionPersonalizationConfig;
    case 'session-permissions':     return SessionPermissionsConfig;
    case 'quick-actions':           return QuickActionsConfig;
    case 'voice-input':             return VoiceInputConfig;
    case 'review':                  return ReviewConfig;
    case 'memories':                return MemoriesConfig;
    case 'ai-thresholds':           return ThresholdsConfig;
    case 'mcp-tools':               return McpToolsConfig;
    case 'external-sources':        return ExternalSourcesConfig;
    // Hooks are part of the external AI applications surface.
    case 'hooks':                   return ExternalSourcesConfig;
    case 'acp-agents':              return AcpAgentsConfig;
    case 'editor':                  return EditorConfig;
    case 'keyboard':                return KeyboardShortcutsTab;
    default:                        return null;
  }
}

const SettingsScene: React.FC = () => {
  useExternalAppAwareness();
  const activeTab = useSettingsStore(s => s.activeTab);
  const contentFocus = useSettingsStore(s => s.contentFocus);
  const contentFocusRequestId = useSettingsStore(s => s.contentFocusRequestId);
  const tabTransitionTarget = useSettingsStore(s => s.tabTransitionTarget);
  const tabTransitionMotion = useSettingsStore(s => s.tabTransitionMotion);
  const tabTransitionSequence = useSettingsStore(s => s.tabTransitionSequence);
  const appliedTransitionSequenceRef = useRef(tabTransitionSequence);

  const resolvedTab: ConfigTab = activeTab;

  const shouldAnimateTabTransition = (
    appliedTransitionSequenceRef.current !== tabTransitionSequence
    && tabTransitionTarget === resolvedTab
    && tabTransitionMotion === 'pointer'
  );

  useLayoutEffect(() => {
    appliedTransitionSequenceRef.current = tabTransitionSequence;
  }, [tabTransitionSequence]);

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
    <div
      className="bitfun-settings-scene"
      data-testid="settings-scene"
      data-settings-tab={resolvedTab}
      data-bf-scene="settings"
      data-bf-part="root"
      data-bf-tab={resolvedTab}
    >
      {Content && (
        <ViewTransitionBoundary
          viewKey={resolvedTab}
          animate={shouldAnimateTabTransition}
          className="bitfun-settings-scene__content-transition"
          viewClassName="bitfun-settings-scene__content-wrapper"
        >
          <div
            data-testid="settings-scene-content"
            data-bf-scene="settings"
            data-bf-part="content"
            data-bf-tab={resolvedTab}
          >
            <Suspense fallback={<SettingsSceneLoading />}>
              {resolvedTab === 'external-sources' || resolvedTab === 'hooks' ? (
                <ExternalSourcesConfig
                  initialFocus={contentFocus === 'hooks' ? 'hooks' : undefined}
                  focusRequestId={contentFocus === 'hooks' ? contentFocusRequestId : undefined}
                />
              ) : (
                <Content />
              )}
            </Suspense>
          </div>
        </ViewTransitionBoundary>
      )}
    </div>
  );
};

export default SettingsScene;
