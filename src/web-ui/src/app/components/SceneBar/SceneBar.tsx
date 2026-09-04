/**
 * SceneBar — horizontally scrollable scene-level tab bar.
 *
 * Delegates state to useSceneManager.
 * The Session tab uses the current session title as its single visible label.
 */

import React, { useCallback } from 'react';

import { Icon, TabGroup, type TabGroupItem } from '@openbitfun/ui';
import { useSceneTabNavigation } from './useSceneTabNavigation';
import { useSceneManager } from '../../hooks/useSceneManager';
import { useCurrentSessionTitle } from '../../hooks/useCurrentSessionTitle';
import { isSceneTabClosable } from '../../scenes/registry';
import { useI18n } from '@/infrastructure/i18n/hooks/useI18n';
import type { SceneTabId } from './types';
import './SceneBar.scss';

function getSceneIdFromTabTarget(target: EventTarget | null): SceneTabId | undefined {
  if (!(target instanceof HTMLElement)) return undefined;
  const item = target.closest<HTMLElement>('[data-openbitfun-part="item"]');
  const tab = item?.querySelector<HTMLElement>('[role="tab"][data-openbitfun-value]');
  return tab?.dataset.openbitfunValue as SceneTabId | undefined;
}

interface SceneBarProps {
  className?: string;
}

const SceneBar: React.FC<SceneBarProps> = ({
  className = '',
}) => {
  const {
    openTabs,
    activeTabId,
    navigationMotion,
    tabDefs,
    activateScene,
    closeScene,
  } = useSceneManager();
  const sessionTitle = useCurrentSessionTitle();
  const { t } = useI18n('common');
  const sceneBarClassName = `openbitfun-scene-bar ${className}`.trim();
  const {
    tabRegionRef,
    tabsRef,
    scrollState: tabScrollState,
    handleScroll: handleTabsScroll,
    handleWheel: handleTabsWheel,
    scrollByPage: scrollTabsByPage,
  } = useSceneTabNavigation({
    activeTabId,
    navigationMotion,
    openTabIds: openTabs.map(tab => tab.id),
  });

  const handleTabValueChange = useCallback((value: string) => {
    activateScene(value as SceneTabId);
  }, [activateScene]);

  const handleTabsMouseDown = useCallback((e: React.MouseEvent<HTMLDivElement>) => {
    if (e.button !== 1) return;
    if ((e.target as HTMLElement | null)?.closest('[data-scene-bar-part="closeTab"]')) return;
    const sceneId = getSceneIdFromTabTarget(e.target);
    if (!sceneId || !isSceneTabClosable(tabDefs.find(def => def.id === sceneId))) return;
    e.preventDefault();
  }, [tabDefs]);

  const handleTabsAuxClick = useCallback((e: React.MouseEvent<HTMLDivElement>) => {
    if (e.button !== 1) return;
    if ((e.target as HTMLElement | null)?.closest('[data-scene-bar-part="closeTab"]')) return;
    const sceneId = getSceneIdFromTabTarget(e.target);
    if (!sceneId || !isSceneTabClosable(tabDefs.find(def => def.id === sceneId))) return;
    e.preventDefault();
    e.stopPropagation();
    closeScene(sceneId);
  }, [closeScene, tabDefs]);

  const handleTabsKeyDown = useCallback((e: React.KeyboardEvent<HTMLDivElement>) => {
    if (e.key !== 'Delete') return;
    const sceneId = getSceneIdFromTabTarget(e.target);
    if (!sceneId || !isSceneTabClosable(tabDefs.find(def => def.id === sceneId))) return;
    e.preventDefault();
    e.stopPropagation();
    closeScene(sceneId);
  }, [closeScene, tabDefs]);

  const tabItems = openTabs.reduce<TabGroupItem[]>((items, tab) => {
    const def = tabDefs.find(candidate => candidate.id === tab.id);
    if (!def) return items;

    const translatedLabel = def.labelKey ? t(def.labelKey) : def.label;
    const displayLabel = tab.id === 'session' && sessionTitle
      ? sessionTitle
      : translatedLabel;
    const closeLabel = t('sceneBar.closeTab', { label: displayLabel });
    const closable = isSceneTabClosable(def);

    items.push({
      value: tab.id,
      label: <span className="openbitfun-scene-bar__tab-title">{displayLabel}</span>,
      // Keep the close hit target stationary between pointer down and up;
      // shrinking it can retarget the click at the button edge (issue #2210).
      endAction: closable ? (
        <button
          type="button"
          aria-label={closeLabel}
          title={closeLabel}
          data-motion="none"
          data-scene-bar-part="closeTab"
          data-scene-id={tab.id}
          onClick={(event) => {
            event.stopPropagation();
            closeScene(tab.id);
          }}
          tabIndex={-1}
        >
          <Icon name="xmark" size="xs" aria-hidden="true" />
        </button>
      ) : undefined,
    });
    return items;
  }, []);

  return (
    <div data-openbitfun-component="scene-bar" data-openbitfun-part="root"
      className={sceneBarClassName}
    >
      <div
        ref={tabRegionRef}
        className="openbitfun-scene-bar__tab-region"
        data-overflow={tabScrollState.hasOverflow ? 'true' : 'false'}
        data-openbitfun-component="scene-bar"
        data-openbitfun-part="tabs"
      >
        {tabScrollState.hasOverflow && (
          <button
            type="button"
            className="openbitfun-scene-bar__scroll-button"
            aria-label={t('sceneBar.scrollPrevious')}
            title={t('sceneBar.scrollPrevious')}
            disabled={!tabScrollState.canScrollBackward}
            onClick={() => scrollTabsByPage(-1)}
            data-openbitfun-component="scene-bar"
            data-openbitfun-part="scrollPrevious"
          >
            <Icon name="chevron-left" size="sm" aria-hidden="true" />
          </button>
        )}

        <TabGroup
          ref={tabsRef}
          className="openbitfun-scene-bar__tabs"
          aria-label={t('sceneBar.tabsLabel')}
          items={tabItems}
          size="sm"
          value={activeTabId ?? undefined}
          onValueChange={handleTabValueChange}
          onScroll={handleTabsScroll}
          onWheel={handleTabsWheel}
          onKeyDown={handleTabsKeyDown}
          onMouseDown={handleTabsMouseDown}
          onAuxClick={handleTabsAuxClick}
          data-scene-bar-part="tabs"
        />

        {tabScrollState.hasOverflow && (
          <button
            type="button"
            className="openbitfun-scene-bar__scroll-button"
            aria-label={t('sceneBar.scrollNext')}
            title={t('sceneBar.scrollNext')}
            disabled={!tabScrollState.canScrollForward}
            onClick={() => scrollTabsByPage(1)}
            data-openbitfun-component="scene-bar"
            data-openbitfun-part="scrollNext"
          >
            <Icon name="chevron-right" size="sm" aria-hidden="true" />
          </button>
        )}
      </div>

    </div>
  );
};

export default SceneBar;
