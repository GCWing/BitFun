/**
 * SceneBar — horizontally scrollable scene-level tab bar.
 *
 * Delegates state to useSceneManager.
 * AI Agent tab shows the current session title as a subtitle.
 */

import React, { useCallback } from 'react';

import { Icon, TabGroup, type TabGroupItem } from '@bitfun/ui';
import { useSceneTabNavigation } from './useSceneTabNavigation';
import { useSceneManager } from '../../hooks/useSceneManager';
import { useCurrentSessionTitle } from '../../hooks/useCurrentSessionTitle';
import { useCurrentSettingsPageTitle } from '../../hooks/useCurrentSettingsPageTitle';
import { isSceneTabClosable } from '../../scenes/registry';
import { useI18n } from '@/infrastructure/i18n/hooks/useI18n';
import type { SceneTabId } from './types';
import './SceneBar.scss';

function getSceneIdFromTabTarget(target: EventTarget | null): SceneTabId | undefined {
  if (!(target instanceof HTMLElement)) return undefined;
  const item = target.closest<HTMLElement>('[data-bf-part="item"]');
  const tab = item?.querySelector<HTMLElement>('[role="tab"][data-bf-value]');
  return tab?.dataset.bfValue as SceneTabId | undefined;
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
  const settingsPageTitle = useCurrentSettingsPageTitle();
  const { t } = useI18n('common');
  const sceneBarClassName = `bitfun-scene-bar ${className}`.trim();
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
    const subtitle =
      (tab.id === 'session' && sessionTitle ? sessionTitle : undefined)
      ?? (tab.id === 'settings' && settingsPageTitle ? settingsPageTitle : undefined);
    const closeLabel = t('sceneBar.closeTab', { label: translatedLabel });
    const closable = isSceneTabClosable(def);

    items.push({
      value: tab.id,
      icon: def.Icon ? <def.Icon aria-hidden="true" /> : undefined,
      label: (
        <span className="bitfun-scene-bar__tab-label">
          <span className="bitfun-scene-bar__tab-title">{translatedLabel}</span>
          {subtitle && (
            <>
              <span className="bitfun-scene-bar__tab-separator" aria-hidden="true">/</span>
              <span className="bitfun-scene-bar__tab-subtitle">{subtitle}</span>
            </>
          )}
        </span>
      ),
      endAction: closable ? (
        <button
          type="button"
          aria-label={closeLabel}
          title={closeLabel}
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
    <div data-bf-component="scene-bar" data-bf-part="root"
      className={sceneBarClassName}
    >
      <div
        ref={tabRegionRef}
        className="bitfun-scene-bar__tab-region"
        data-overflow={tabScrollState.hasOverflow ? 'true' : 'false'}
        data-bf-component="scene-bar"
        data-bf-part="tabs"
      >
        {tabScrollState.hasOverflow && (
          <button
            type="button"
            className="bitfun-scene-bar__scroll-button"
            aria-label={t('sceneBar.scrollPrevious')}
            title={t('sceneBar.scrollPrevious')}
            disabled={!tabScrollState.canScrollBackward}
            onClick={() => scrollTabsByPage(-1)}
            data-bf-component="scene-bar"
            data-bf-part="scrollPrevious"
          >
            <Icon name="chevron-left" size="sm" aria-hidden="true" />
          </button>
        )}

        <TabGroup
          ref={tabsRef}
          className="bitfun-scene-bar__tabs"
          aria-label={t('sceneBar.tabsLabel')}
          items={tabItems}
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
            className="bitfun-scene-bar__scroll-button"
            aria-label={t('sceneBar.scrollNext')}
            title={t('sceneBar.scrollNext')}
            disabled={!tabScrollState.canScrollForward}
            onClick={() => scrollTabsByPage(1)}
            data-bf-component="scene-bar"
            data-bf-part="scrollNext"
          >
            <Icon name="chevron-right" size="sm" aria-hidden="true" />
          </button>
        )}
      </div>

    </div>
  );
};

export default SceneBar;
