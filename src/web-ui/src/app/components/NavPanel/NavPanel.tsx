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
 *   All other scenes (settings, …):
 *     Simple crossfade — MainNav hidden instantly, SceneNav fades in.
 *
 * MainNav is always mounted so its state is preserved across transitions.
 */

import React, { Suspense, useState, useEffect, useRef, useCallback } from 'react';
import { useI18n } from '@/infrastructure/i18n';
import { useNavSceneStore } from '../../stores/navSceneStore';
import { getSceneNav } from '../../scenes/nav-registry';
import type { SceneTabId } from '../SceneBar/types';
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

  const [mountedSceneId, setMountedSceneId] = useState<SceneTabId | null>(navSceneId);
  useEffect(() => {
    if (navSceneId) setMountedSceneId(navSceneId);
  }, [navSceneId]);

  const SceneNavComponent = mountedSceneId ? getSceneNav(mountedSceneId) : null;

  const useSplitOpen = !!(showSceneNav && mountedSceneId && SPLIT_OPEN_SCENES.has(mountedSceneId));

  const contentRef = useRef<HTMLDivElement>(null);

  const updateClipOrigin = useCallback(() => {
    const container = contentRef.current;
    if (!container) return;
    const anchor = container.querySelector<HTMLElement>('.bitfun-nav-panel__item-slot.is-departing-anchor');
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
    'bitfun-nav-panel__content',
    showSceneNav && 'is-scene',
    useSplitOpen && 'is-split-open',
  ].filter(Boolean).join(' ');

  const sceneCls = [
    'bitfun-nav-panel__layer bitfun-nav-panel__layer--scene',
    showSceneNav && 'is-active',
  ].filter(Boolean).join(' ');

  return (
    <nav className={`bitfun-nav-panel ${className}`} aria-label={t('nav.aria.mainNav')}>
      <div ref={contentRef} className={contentCls}>

        <div className="bitfun-nav-panel__layer bitfun-nav-panel__layer--main">
          <MainNav
            isDeparting={useSplitOpen}
            anchorNavSceneId={useSplitOpen ? mountedSceneId : null}
          />
        </div>

        {SceneNavComponent && (
          <div className={sceneCls}>
            <Suspense fallback={null}>
              <div key={mountedSceneId} className="bitfun-nav-panel__scene-inner">
                <SceneNavComponent />
              </div>
            </Suspense>
          </div>
        )}

      </div>
      <PersistentFooterActions />
    </nav>
  );
};

export default NavPanel;
