/**
 * NavPanel — navigation sidebar container.
 *
 * Two transition modes depending on the target scene:
 *
 *   file-viewer:
 *     Split-open accordion — MainNav items depart up/down from the anchor
 *     item while SceneNav is revealed via clip-path expanding from the
 *     anchor's Y position. Both layers coexist in the DOM (overlay).
 *
 *   All other pointer-opened scenes (settings, …):
 *     MainNav and SceneNav use a short paired crossfade/translation. Keyboard
 *     and programmatic navigation stay immediate.
 *
 * MainNav is always mounted so its state is preserved across transitions.
 */

import React, {
  Suspense,
  startTransition,
  useState,
  useEffect,
  useRef,
  useCallback,
} from 'react';
import { useI18n } from '@/infrastructure/i18n';
import { useNavSceneStore } from '../../stores/navSceneStore';
import { getSceneNav, preloadSceneNav } from '../../scenes/nav-registry';
import type { SceneTabId } from '../SceneBar/types';
import { NavigationTransitionBoundary } from '@/app/navigation/NavigationTransitionBoundary';
import type { InteractionMotion } from '@/shared/utils/motionPreference';
import MainNav from './MainNav';
import PersistentFooterActions from './components/PersistentFooterActions';
import './NavPanel.scss';

/** Scenes that use the split-open accordion transition. */
const SPLIT_OPEN_SCENES: ReadonlySet<SceneTabId> = new Set(['file-viewer']);

interface NavPanelProps {
  // Persist the last known sceneId so SceneNav content remains visible
  // during the closing accordion animation (navSceneId may clear before
  // the transition ends).
  className?: string;
}

const NavPanel: React.FC<NavPanelProps> = ({ className = '' }) => {
  const { t } = useI18n('common');
  const showSceneNav = useNavSceneStore(s => s.showSceneNav);
  const navSceneId = useNavSceneStore(s => s.navSceneId);
  const navigationMotion = useNavSceneStore(s => s.navigationMotion);

  const [mountedSceneId, setMountedSceneId] = useState<SceneTabId | null>(navSceneId);
  const [mountedSceneMotion, setMountedSceneMotion] = useState<InteractionMotion>(navigationMotion);
  const sceneRequestRef = useRef(0);
  useEffect(() => {
    const requestId = ++sceneRequestRef.current;
    if (!navSceneId) return;

    const commit = () => {
      if (sceneRequestRef.current !== requestId) return;
      // React keeps the currently painted navigation visible if the cached
      // lazy component still suspends for a final promise microtask.
      startTransition(() => {
        setMountedSceneId(navSceneId);
        setMountedSceneMotion(navigationMotion);
      });
    };
    void preloadSceneNav(navSceneId).then(commit, commit);
  }, [navSceneId, navigationMotion]);

  const SceneNavComponent = mountedSceneId ? getSceneNav(mountedSceneId) : null;

  const hasMountedSceneNav = showSceneNav && mountedSceneId !== null;
  const useSplitOpen = !!(
    hasMountedSceneNav
    && mountedSceneId
    && SPLIT_OPEN_SCENES.has(mountedSceneId)
  );

  const contentRef = useRef<HTMLDivElement>(null);

  const updateClipOrigin = useCallback(() => {
    const container = contentRef.current;
    if (!container) return;
    const anchor = container.querySelector<HTMLElement>('.openbitfun-nav-panel__item-slot.is-departing-anchor');
    if (anchor) {
      const containerRect = container.getBoundingClientRect();
      const anchorRect = anchor.getBoundingClientRect();
      const anchorCenterY = anchorRect.top + anchorRect.height / 2 - containerRect.top;
      const pct = (anchorCenterY / containerRect.height) * 100;
      container.style.setProperty('--clip-origin-top', `${pct}%`);
      container.style.setProperty('--clip-origin-bottom', `${100 - pct}%`);
    }
  }, []);

  useEffect(() => {
    if (useSplitOpen) {
      requestAnimationFrame(updateClipOrigin);
    }
  }, [useSplitOpen, updateClipOrigin]);

  const contentCls = [
    'openbitfun-nav-panel__content',
    hasMountedSceneNav && 'is-scene',
    useSplitOpen && 'is-split-open',
    (showSceneNav ? mountedSceneMotion : navigationMotion) === 'pointer' && 'has-pointer-motion',
  ].filter(Boolean).join(' ');

  const sceneCls = [
    'openbitfun-nav-panel__layer openbitfun-nav-panel__layer--scene',
    hasMountedSceneNav && 'is-active',
  ].filter(Boolean).join(' ');
  const appearanceState = [
    showSceneNav && 'scene',
    useSplitOpen && 'split',
  ].filter(Boolean).join(' ');

  return (
    <div
      data-openbitfun-component="nav-panel"
      data-openbitfun-part="root"
      data-openbitfun-state={appearanceState}
      data-openbitfun-theme-scope="chrome"
      className={`openbitfun-nav-panel ${className}`}
      aria-label={t('nav.aria.mainNav')}
      data-testid="nav-panel"
    >
      <div ref={contentRef} className={contentCls} data-openbitfun-component="nav-panel" data-openbitfun-part="content">

        <div className="openbitfun-nav-panel__layer openbitfun-nav-panel__layer--main" data-openbitfun-component="nav-panel" data-openbitfun-part="mainLayer" data-openbitfun-layer="main">
          <MainNav
            isDeparting={useSplitOpen}
            anchorNavSceneId={useSplitOpen ? mountedSceneId : null}
          />
        </div>

        {SceneNavComponent && (
          <div className={sceneCls} data-openbitfun-component="nav-panel" data-openbitfun-part="sceneLayer" data-openbitfun-layer="scene" data-openbitfun-state={showSceneNav ? 'active' : ''}>
            <Suspense fallback={null}>
              <NavigationTransitionBoundary
                transitionKey={mountedSceneId ?? 'main'}
                motion={showSceneNav && mountedSceneMotion === 'pointer' ? 'pointer' : 'none'}
                className="openbitfun-nav-panel__scene-transition"
                layerClassName="openbitfun-nav-panel__scene-inner"
                data-openbitfun-component="nav-panel"
                data-openbitfun-part="sceneContent"
              >
                <SceneNavComponent />
              </NavigationTransitionBoundary>
            </Suspense>
          </div>
        )}

      </div>
      <PersistentFooterActions />
    </div>
  );
};

export default NavPanel;
