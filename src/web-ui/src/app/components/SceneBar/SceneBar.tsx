/**
 * SceneBar — horizontally scrollable scene-level tab bar.
 *
 * Delegates state to useSceneManager.
 * AI Agent tab shows the current session title as a subtitle.
 */

import React, { useCallback, useRef } from 'react';
import { ChevronLeft, ChevronRight, X } from 'lucide-react';
import { TabGroup, type TabGroupItem } from '@bitfun/ui';
import { useSceneTabNavigation } from './useSceneTabNavigation';
import { WindowControls } from '@/component-library';
import { useSceneManager } from '../../hooks/useSceneManager';
import { useCurrentSessionTitle } from '../../hooks/useCurrentSessionTitle';
import { useCurrentSettingsPageTitle } from '../../hooks/useCurrentSettingsPageTitle';
import { useI18n } from '@/infrastructure/i18n/hooks/useI18n';
import { createLogger } from '@/shared/utils/logger';
import { supportsNativeWindowDragging } from '@/infrastructure/runtime';
import type { SceneTabId } from './types';
import './SceneBar.scss';

const log = createLogger('SceneBar');

const INTERACTIVE_SELECTOR =
  'button, input, textarea, select, a, [role="button"], [contenteditable="true"], .window-controls';

function blocksWindowChromeInteraction(target: HTMLElement): boolean {
  const interactive = target.closest<HTMLElement>(INTERACTIVE_SELECTOR);
  return interactive !== null && interactive.getAttribute('role') !== 'tab';
}

function getSceneIdFromTabTarget(target: EventTarget | null): SceneTabId | undefined {
  if (!(target instanceof HTMLElement)) return undefined;
  const item = target.closest<HTMLElement>('[data-bf-part="item"]');
  const tab = item?.querySelector<HTMLElement>('[role="tab"][data-bf-value]');
  return tab?.dataset.bfValue as SceneTabId | undefined;
}

interface SceneBarProps {
  className?: string;
  onMinimize?: () => void;
  onMaximize?: () => void;
  onClose?: () => void;
  isMaximized?: boolean;
}

const SceneBar: React.FC<SceneBarProps> = ({
  className = '',
  onMinimize,
  onMaximize,
  onClose,
  isMaximized = false,
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
  const hasWindowControls = !!(onMinimize && onMaximize && onClose);
  const sceneBarClassName = `bitfun-scene-bar ${!hasWindowControls ? 'bitfun-scene-bar--no-controls' : ''} ${className}`.trim();
  const isSingleTab = openTabs.length <= 1;
  const canDragWindow = supportsNativeWindowDragging();
  const lastMouseDownTimeRef = useRef<number>(0);
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
    if (!sceneId || tabDefs.find(def => def.id === sceneId)?.pinned) return;
    e.preventDefault();
  }, [tabDefs]);

  const handleTabsAuxClick = useCallback((e: React.MouseEvent<HTMLDivElement>) => {
    if (e.button !== 1) return;
    if ((e.target as HTMLElement | null)?.closest('[data-scene-bar-part="closeTab"]')) return;
    const sceneId = getSceneIdFromTabTarget(e.target);
    if (!sceneId || tabDefs.find(def => def.id === sceneId)?.pinned) return;
    e.preventDefault();
    e.stopPropagation();
    closeScene(sceneId);
  }, [closeScene, tabDefs]);

  const handleBarMouseDown = useCallback((e: React.MouseEvent) => {
    if (!canDragWindow) return;
    if (!isSingleTab) return;

    const now = Date.now();
    const timeSinceLastMouseDown = now - lastMouseDownTimeRef.current;
    lastMouseDownTimeRef.current = now;

    if (e.button !== 0) return;
    const target = e.target as HTMLElement | null;
    if (!target) return;
    if (blocksWindowChromeInteraction(target)) return;
    if (timeSinceLastMouseDown < 500 && timeSinceLastMouseDown > 50) return;

    void (async () => {
      try {
        const { getCurrentWindow } = await import('@tauri-apps/api/window');
        await getCurrentWindow().startDragging();
      } catch (error) {
        log.debug('startDragging failed', error);
      }
    })();
  }, [canDragWindow, isSingleTab]);

  const handleBarDoubleClick = useCallback((e: React.MouseEvent) => {
    if (!isSingleTab) return;
    const target = e.target as HTMLElement | null;
    if (!target) return;
    if (blocksWindowChromeInteraction(target)) return;
    onMaximize?.();
  }, [isSingleTab, onMaximize]);

  const tabItems = openTabs.reduce<TabGroupItem[]>((items, tab) => {
    const def = tabDefs.find(candidate => candidate.id === tab.id);
    if (!def) return items;

    const translatedLabel = def.labelKey ? t(def.labelKey) : def.label;
    const subtitle =
      (tab.id === 'session' && sessionTitle ? sessionTitle : undefined)
      ?? (tab.id === 'settings' && settingsPageTitle ? settingsPageTitle : undefined);
    const closeLabel = t('sceneBar.closeTab', { label: translatedLabel });

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
      endAction: def.pinned ? undefined : (
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
          <X size={12} aria-hidden="true" />
        </button>
      ),
    });
    return items;
  }, []);

  return (
    <div data-bf-component="scene-bar" data-bf-part="root"
      className={sceneBarClassName}
      onMouseDown={handleBarMouseDown}
      onDoubleClick={handleBarDoubleClick}
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
            <ChevronLeft size={14} aria-hidden="true" />
          </button>
        )}

        <TabGroup
          ref={tabsRef}
          className="bitfun-scene-bar__tabs"
          aria-label={t('sceneBar.tabsLabel')}
          items={tabItems}
          value={activeTabId}
          onValueChange={handleTabValueChange}
          onScroll={handleTabsScroll}
          onWheel={handleTabsWheel}
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
            <ChevronRight size={14} aria-hidden="true" />
          </button>
        )}
      </div>

      {hasWindowControls && (
        <div className="bitfun-scene-bar__controls" data-bf-component="scene-bar" data-bf-part="controls">
          <WindowControls
            onMinimize={onMinimize!}
            onMaximize={onMaximize!}
            onClose={onClose!}
            isMaximized={isMaximized}
          />
        </div>
      )}
    </div>
  );
};

export default SceneBar;
