import React, { Suspense, useEffect, useLayoutEffect, useRef, useState } from 'react';
import { NavigationTransitionBoundary } from '@/app/navigation/NavigationTransitionBoundary';
import {
  getSettingsPageManifest,
  isSettingsPageReady,
  preloadSettingsPage,
} from './settingsRegistry';
import { useSettingsStore } from './settingsStore';
import type { SettingsPageId } from './settingsTypes';
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

const SettingsScene: React.FC = () => {
  const activePageId = useSettingsStore((state) => state.activePageId);
  const activeViewId = useSettingsStore((state) => state.activeViewId);
  const navigationRequestId = useSettingsStore((state) => state.navigationRequestId);
  const pageTransitionTarget = useSettingsStore((state) => state.pageTransitionTarget);
  const pageTransitionMotion = useSettingsStore((state) => state.pageTransitionMotion);
  const pageTransitionSequence = useSettingsStore((state) => state.pageTransitionSequence);
  const appliedTransitionSequenceRef = useRef(pageTransitionSequence);
  const [preparedPageId, setPreparedPageId] = useState<SettingsPageId | null>(() => (
    isSettingsPageReady(activePageId) ? activePageId : null
  ));

  useEffect(() => {
    if (isSettingsPageReady(activePageId)) {
      setPreparedPageId(activePageId);
      return;
    }
    let cancelled = false;
    const commit = () => {
      if (!cancelled) setPreparedPageId(activePageId);
    };
    void preloadSettingsPage(activePageId).then(commit, commit);
    return () => {
      cancelled = true;
    };
  }, [activePageId]);

  const shouldAnimatePageTransition = (
    appliedTransitionSequenceRef.current !== pageTransitionSequence
    && pageTransitionTarget === activePageId
    && pageTransitionMotion === 'pointer'
  );

  useLayoutEffect(() => {
    appliedTransitionSequenceRef.current = pageTransitionSequence;
  }, [pageTransitionSequence]);

  const manifest = getSettingsPageManifest(activePageId);
  const Content = preparedPageId === activePageId ? manifest.component : null;

  return (
    <div
      className="bitfun-settings-scene"
      data-testid="settings-scene"
      data-settings-page={activePageId}
      data-bf-scene="settings"
      data-bf-part="root"
      data-bf-page={activePageId}
    >
      {Content ? (
        <NavigationTransitionBoundary
          transitionKey={activePageId}
          motion={shouldAnimatePageTransition ? 'pointer' : 'none'}
          className="bitfun-settings-scene__content-transition"
          layerClassName="bitfun-settings-scene__content-wrapper"
        >
          <div
            data-testid="settings-scene-content"
            data-bf-scene="settings"
            data-bf-part="content"
            data-bf-page={activePageId}
          >
            <Suspense fallback={<SettingsSceneLoading />}>
              <Content
                viewId={activeViewId ?? undefined}
                navigationRequestId={navigationRequestId}
              />
            </Suspense>
          </div>
        </NavigationTransitionBoundary>
      ) : <SettingsSceneLoading />}
    </div>
  );
};

export default SettingsScene;
