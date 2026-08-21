/**
 * TabBar component.
 * Tab bar container that manages visibility and overflow.
 */

import React, { useState, useRef, useEffect, useCallback, useMemo, useLayoutEffect } from 'react';
import { Table2, X, Combine, Trash2 } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { Tooltip } from '@/component-library';
import { Tab } from './Tab';
import { TabOverflowMenu } from './TabOverflowMenu';
import type { CanvasTab, EditorGroupId, TabDragPayload } from '../types';
import { createLogger } from '@/shared/utils/logger';
import './TabBar.scss';

const log = createLogger('TabBar');
const TAB_REORDER_DURATION_MS = 160;
const TAB_REORDER_EASING = 'cubic-bezier(0.22, 1, 0.36, 1)';

export interface TabBarProps {
  /** Tab list */
  tabs: CanvasTab[];
  /** Editor group ID */
  groupId: EditorGroupId;
  /** Active tab ID */
  activeTabId: string | null;
  /** Whether this group is active */
  isActiveGroup: boolean;
  /** Click tab */
  onTabClick: (tabId: string) => void;
  /** Double-click tab */
  onTabDoubleClick: (tabId: string) => void;
  /** Close tab */
  onTabClose: (tabId: string) => Promise<void> | void;
  /** Pin tab */
  onTabPin: (tabId: string) => void;
  /** Drag start */
  onDragStart: (payload: TabDragPayload) => void;
  /** Drag end */
  onDragEnd: () => void;
  /** Dragging tab ID */
  draggingTabId: string | null;
  /** Reorder tab */
  onReorderTab: (tabId: string, newIndex: number) => void;
  /** Open mission control */
  onOpenMissionControl?: () => void;
  /** Close all tabs */
  onCloseAllTabs?: () => Promise<void> | void;
  /** Pop out tab as independent scene */
  onTabPopOut?: (tabId: string) => void;
  /** Optional grid template toggle rendered in the actions area (primary
   *  group): four-cell (2x2), six-cell (2x3) and nine-cell (3x3) presets. */
  grid9Slot?: {
    active: boolean;
    onToggle: () => void;
    label: string;
    /** Preset templates shown in the dropdown: [cols, rows, i18n key]. */
    templates?: Array<{ cols: number; rows: number; label: string }>;
    onApplyTemplate?: (cols: number, rows: number) => void;
  };
  /** Merge this grid9 cell's tabs into a neighbour (free split/merge). Only
   *  shown in grid9 mode on non-primary cells with content. */
  onMergeCell?: () => void;
  /** Whether merge affordance is available (grid9 + non-primary + has tabs). */
  canMergeCell?: boolean;
  /** Remove this blank grid9 cell (shrink + re-tile remaining cells). */
  onRemoveCell?: () => void;
  /** Whether the remove affordance is available (blank cell, grid large enough). */
  canRemoveCell?: boolean;
}

/**
 * Estimate tab width based on title length.
 * - Base padding: 6px * 2 = 12px (left/right)
 * - Gap: 4px
 * - Close button: 16px
 * - Char width: ~7px/char (12px font)
 * - CJK chars: ~12px/char
 */
const estimateTabWidth = (title: string): number => {
  const PADDING = 16; // 8px * 2
  const GAP = 4;
  const CLOSE_BTN = 16;
  const MIN_WIDTH = 80;
  const MAX_WIDTH = 180;
  
  // Estimate title width: CJK ~12px, others ~7px
  let titleWidth = 0;
  for (const char of title) {
    // Simple check: CJK unicode range
    if (char.charCodeAt(0) > 255) {
      titleWidth += 12;
    } else {
      titleWidth += 7;
    }
  }
  
  const estimated = PADDING + titleWidth + GAP + CLOSE_BTN;
  return Math.max(MIN_WIDTH, Math.min(MAX_WIDTH, estimated));
};

const tabTitleForWidthEstimate = (tab: CanvasTab, deletedLabel: string): string =>
  tab.fileDeletedFromDisk ? `${tab.title} - ${deletedLabel}` : tab.title;

export const TabBar: React.FC<TabBarProps> = ({
  tabs,
  groupId,
  activeTabId,
  isActiveGroup,
  onTabClick,
  onTabDoubleClick,
  onTabClose,
  onTabPin,
  onDragStart,
  onDragEnd,
  draggingTabId,
  onReorderTab,
  onOpenMissionControl,
  onCloseAllTabs,
  onTabPopOut,
  grid9Slot,
  onMergeCell,
  canMergeCell = false,
  onRemoveCell,
  canRemoveCell = false,
}) => {
  const { t } = useTranslation('components');
  const [visibleTabsCount, setVisibleTabsCount] = useState(tabs.length);
  const [dragOverIndex, setDragOverIndex] = useState<number | null>(null);
  // Track initial layout measurement completion
  const [layoutReady, setLayoutReady] = useState(false);
  // Grid template dropdown open state (four/six/nine-cell presets)
  const [grid9MenuOpen, setGrid9MenuOpen] = useState(false);
  
  const containerRef = useRef<HTMLDivElement>(null);
  const tabsListRef = useRef<HTMLDivElement>(null);
  const actionsRef = useRef<HTMLDivElement>(null);
  const tabWrapperRefs = useRef<Map<string, HTMLDivElement>>(new Map());
  const pendingReorderRectsRef = useRef<Map<string, DOMRect> | null>(null);
  const reorderAnimationsRef = useRef<Map<string, Animation>>(new Map());
  // Cache actual tab widths (keyed by tab.id + title since title affects width)
  const tabWidthCacheRef = useRef<Map<string, number>>(new Map());

  // Filter out hidden tabs
  const visibleTabs = useMemo(() => tabs.filter(t => !t.isHidden), [tabs]);
  
  // Build cache key (id + title because title changes affect width)
  const getTabCacheKey = useCallback(
    (tab: CanvasTab) => `${tab.id}:${tab.title}:${tab.fileDeletedFromDisk ? '1' : '0'}`,
    []
  );

  // Get tab width: use cache if available, otherwise estimate
  const getTabWidth = useCallback((tab: CanvasTab): number => {
    const cacheKey = getTabCacheKey(tab);
    const cached = tabWidthCacheRef.current.get(cacheKey);
    if (cached !== undefined) {
      return cached;
    }
    // Estimated width
    return estimateTabWidth(tabTitleForWidthEstimate(tab, t('tabs.fileDeleted')));
  }, [getTabCacheKey, t]);

  // Compute visible tab count based on DOM measurements
  const calculateVisibleTabs = useCallback(() => {
    if (!containerRef.current || visibleTabs.length === 0) {
      setVisibleTabsCount(visibleTabs.length);
      setLayoutReady(true);
      return;
    }

    const containerWidth = containerRef.current.clientWidth;
    
    // Measure rendered tabs and update cache
    if (tabsListRef.current) {
      const tabElements = tabsListRef.current.querySelectorAll('.canvas-tab-bar__tab-wrapper');
      tabElements.forEach((el, index) => {
        if (index < visibleTabs.length) {
          const width = (el as HTMLElement).offsetWidth;
          if (width > 0) {
            const cacheKey = getTabCacheKey(visibleTabs[index]);
            tabWidthCacheRef.current.set(cacheKey, width);
          }
        }
      });
    }
    
    // Total width of all tabs
    const allTabWidths = visibleTabs.map(tab => getTabWidth(tab));
    const totalTabsWidth = allTabWidths.reduce((sum, w) => sum + w, 0);
    
    // Base actions width (excluding overflow button)
    // Close-all button: 28px + gap
    const baseActionsWidth = (onCloseAllTabs ? 28 : 0) + 4;
    // Overflow button width (~50px with badge, 28px with only mission control)
    const overflowBtnWidth = onOpenMissionControl ? 50 : 28;
    // Gap before actions area
    const actionsGap = 8;
    
    // Phase 1: check if all tabs fit without overflow
    // Overflow can be hidden only when mission control entry is not needed
    const availableWithoutOverflow = containerWidth - baseActionsWidth - actionsGap;
    const canFitAll = !onOpenMissionControl && totalTabsWidth <= availableWithoutOverflow;
    
    // Compute actual available width
    const actionsWidth = canFitAll ? baseActionsWidth : (baseActionsWidth + overflowBtnWidth);
    const availableWidth = containerWidth - actionsWidth - actionsGap;
    
    // Phase 2: iterate tabs to determine how many fit
    let totalWidth = 0;
    let count = 0;
    
    for (let i = 0; i < visibleTabs.length; i++) {
      const tabWidth = allTabWidths[i];
      
      if (totalWidth + tabWidth <= availableWidth) {
        totalWidth += tabWidth;
        count++;
      } else {
        break;
      }
    }

    // Always show at least one tab
    const finalCount = Math.max(1, Math.min(count, visibleTabs.length));
    setVisibleTabsCount(finalCount);
    setLayoutReady(true);
  }, [visibleTabs, getTabWidth, getTabCacheKey, onCloseAllTabs, onOpenMissionControl]);

  // Reset to render all tabs when list changes (re-measure)
  useEffect(() => {
    // Reset to show all, then let calculateVisibleTabs recompute
    setVisibleTabsCount(visibleTabs.length);
    setLayoutReady(false);
  }, [visibleTabs.length]);

  // Use useLayoutEffect to measure right after DOM update
  useLayoutEffect(() => {
    // Wait a frame to ensure tabs are rendered
    const frameId = requestAnimationFrame(() => {
      calculateVisibleTabs();
    });
    
    return () => cancelAnimationFrame(frameId);
  }, [visibleTabs, calculateVisibleTabs]);

  // Observe container size changes
  useEffect(() => {
    const resizeObserver = new ResizeObserver(() => {
      // Use requestAnimationFrame to avoid frequent recalculations
      requestAnimationFrame(() => {
        calculateVisibleTabs();
      });
    });

    if (containerRef.current) {
      resizeObserver.observe(containerRef.current);
    }

    return () => {
      resizeObserver.disconnect();
    };
  }, [calculateVisibleTabs]);

  // Split visible and overflow tabs
  const displayedTabs = visibleTabs.slice(0, visibleTabsCount);
  const overflowTabs = visibleTabs.slice(visibleTabsCount);
  const displayedTabSignature = displayedTabs.map(tab => tab.id).join(':');

  useEffect(() => () => {
    reorderAnimationsRef.current.forEach(animation => animation.cancel());
    reorderAnimationsRef.current.clear();
  }, []);

  useLayoutEffect(() => {
    const previousRects = pendingReorderRectsRef.current;
    if (!previousRects) return;
    pendingReorderRectsRef.current = null;

    const reduceMotion = window.matchMedia?.('(prefers-reduced-motion: reduce)').matches ?? false;
    if (reduceMotion) return;

    for (const [tabId, element] of tabWrapperRefs.current) {
      const previousRect = previousRects.get(tabId);
      if (!previousRect) continue;
      const deltaX = previousRect.left - element.getBoundingClientRect().left;
      if (Math.abs(deltaX) < 0.5) continue;

      reorderAnimationsRef.current.get(tabId)?.cancel();
      const animation = element.animate(
        [
          { transform: `translateX(${deltaX}px)` },
          { transform: 'translateX(0)' },
        ],
        {
          duration: TAB_REORDER_DURATION_MS,
          easing: TAB_REORDER_EASING,
        },
      );
      reorderAnimationsRef.current.set(tabId, animation);
      animation.addEventListener('finish', () => {
        if (reorderAnimationsRef.current.get(tabId) === animation) {
          reorderAnimationsRef.current.delete(tabId);
        }
      }, { once: true });
    }
  }, [displayedTabSignature]);

  // Handle tab drag start
  const handleTabDragStart = useCallback((tab: CanvasTab) => (_e: React.DragEvent) => {
    onDragStart({
      tabId: tab.id,
      sourceGroupId: groupId,
      tab,
    });
  }, [groupId, onDragStart]);

  // Handle drag over
  const handleDragOver = useCallback((e: React.DragEvent, index: number) => {
    e.preventDefault();
    e.stopPropagation();
    e.dataTransfer.dropEffect = 'move';
    setDragOverIndex(index);
  }, []);

  // Handle drag leave
  const handleDragLeave = useCallback((e: React.DragEvent) => {
    if (!e.currentTarget.contains(e.relatedTarget as Node)) {
      setDragOverIndex(null);
    }
  }, []);

  // Handle drop
  const handleDrop = useCallback((e: React.DragEvent, targetIndex: number) => {
    e.preventDefault();
    e.stopPropagation();
    setDragOverIndex(null);

    if (!draggingTabId) return;

    try {
      const data = JSON.parse(e.dataTransfer.getData('application/json'));
      
      // Only reorder within the same group
      if (data.sourceGroupId === groupId) {
        const currentIndex = visibleTabs.findIndex(t => t.id === data.tabId);
        if (currentIndex !== -1 && currentIndex !== targetIndex) {
          pendingReorderRectsRef.current = new Map(
            Array.from(tabWrapperRefs.current, ([tabId, element]) => (
              [tabId, element.getBoundingClientRect()] as const
            )),
          );
          onReorderTab(data.tabId, targetIndex);
        }
      }
    } catch (err) {
      log.error('Failed to parse drag data', err);
    }
  }, [draggingTabId, groupId, visibleTabs, onReorderTab]);

  const draggingDisplayedIndex = draggingTabId
    ? displayedTabs.findIndex(tab => tab.id === draggingTabId)
    : -1;
  const draggedTabWidth = draggingDisplayedIndex >= 0
    ? getTabWidth(displayedTabs[draggingDisplayedIndex])
    : 0;

  const getDragShift = (index: number): number => {
    if (dragOverIndex === null || draggingDisplayedIndex < 0 || index === draggingDisplayedIndex) {
      return 0;
    }
    if (draggingDisplayedIndex < dragOverIndex && index > draggingDisplayedIndex && index <= dragOverIndex) {
      return -draggedTabWidth;
    }
    if (draggingDisplayedIndex > dragOverIndex && index >= dragOverIndex && index < draggingDisplayedIndex) {
      return draggedTabWidth;
    }
    return 0;
  };

  // Clear indicator when drag ends
  useEffect(() => {
    if (!draggingTabId) {
      setDragOverIndex(null);
    }
  }, [draggingTabId]);

  const handleCloseOtherTabs = useCallback((targetTabId: string) => async () => {
    for (const tab of visibleTabs) {
      if (tab.id !== targetTabId && tab.state !== 'pinned') {
        await onTabClose(tab.id);
      }
    }
  }, [onTabClose, visibleTabs]);

  return (
    <div data-bf-component="canvas-tab-bar" data-bf-part="root" data-bf-group={groupId} data-bf-state={isActiveGroup ? 'active' : ''}
      ref={containerRef}
      className={`canvas-tab-bar ${isActiveGroup ? 'is-active-group' : ''}`}
    >
      {/* Tab list */}
      <div ref={tabsListRef} className="canvas-tab-bar__tabs" data-bf-component="canvas-tab-bar" data-bf-part="list" data-bf-group={groupId}>
        {displayedTabs.map((tab, index) => (
          <div
            data-bf-component="canvas-tab-bar"
            data-bf-part="tabWrapper"
            data-tab-id={tab.id}
            key={tab.id}
            className="canvas-tab-bar__tab-wrapper"
            ref={(element) => {
              if (element) tabWrapperRefs.current.set(tab.id, element);
              else tabWrapperRefs.current.delete(tab.id);
            }}
            style={{
              transform: getDragShift(index) === 0
                ? undefined
                : `translateX(${getDragShift(index)}px)`,
            }}
            onDragOver={(e) => handleDragOver(e, index)}
            onDragLeave={handleDragLeave}
            onDrop={(e) => handleDrop(e, index)}
          >
            {/* Drop indicator */}
            {dragOverIndex === index && draggingTabId && (
              <div data-bf-component="canvas-tab-bar" data-bf-part="dropIndicator" className="canvas-tab-drop-indicator" />
            )}
            
            <Tab
              tab={tab}
              groupId={groupId}
              isActive={activeTabId === tab.id}
              onClick={() => onTabClick(tab.id)}
              onDoubleClick={() => onTabDoubleClick(tab.id)}
              onClose={() => onTabClose(tab.id)}
              onPin={() => onTabPin(tab.id)}
              onDragStart={handleTabDragStart(tab)}
              onDragEnd={onDragEnd}
              isDragging={draggingTabId === tab.id}
              onPopOut={onTabPopOut ? () => onTabPopOut(tab.id) : undefined}
              onCloseOthers={visibleTabs.length > 1 ? handleCloseOtherTabs(tab.id) : undefined}
              onCloseAll={onCloseAllTabs}
            />
          </div>
        ))}
      </div>

      {/* Actions area */}
      <div ref={actionsRef} className="canvas-tab-bar__actions" data-bf-component="canvas-tab-bar" data-bf-part="actions" data-bf-group={groupId}>
        {/* Grid template toggle (right panel top-right): clicking opens the
            four/six/nine-cell presets; the button itself toggles the last
            applied grid on/off. */}
        {grid9Slot && (
          <div className="canvas-tab-bar__grid9-wrap" data-bf-component="canvas-tab-bar" data-bf-part="gridTemplate">
            <Tooltip content={grid9Slot.label} placement="bottom">
              <button
                type="button"
                data-bf-component="canvas-tab-bar"
                data-bf-part="action"
                data-bf-state={grid9Slot.active ? 'active' : ''}
                className={`canvas-tab-bar__action-btn canvas-tab-bar__grid9-btn${grid9Slot.active ? ' is-active' : ''}`}
                onClick={(e) => {
                  e.stopPropagation();
                  // Grid active → clicking the button exits the grid (M1:
                  // previously templates always existed so the click only
                  // opened the menu and the grid could never be turned off
                  // from this button).
                  if (grid9Slot.active) {
                    setGrid9MenuOpen(false);
                    grid9Slot.onToggle();
                    return;
                  }
                  // Inactive → open the template dropdown (four/six/nine-cell).
                  if (!grid9Slot.templates?.length) {
                    grid9Slot.onToggle();
                    return;
                  }
                  setGrid9MenuOpen(!grid9MenuOpen);
                }}
                aria-label={grid9Slot.label}
                aria-pressed={grid9Slot.active}
                aria-expanded={grid9MenuOpen}
              >
                <Table2 size={14} />
              </button>
            </Tooltip>
            {grid9MenuOpen && grid9Slot.templates && (
              <div
                className="canvas-tab-bar__grid9-menu"
                data-bf-component="canvas-tab-bar"
                data-bf-part="gridTemplateMenu"
                onClick={(e) => e.stopPropagation()}
              >
                {grid9Slot.templates.map((tpl) => (
                  <button
                    key={`${tpl.cols}x${tpl.rows}`}
                    type="button"
                    className="canvas-tab-bar__grid9-menu-item"
                    data-bf-component="canvas-tab-bar"
                    data-bf-part="gridTemplateItem"
                    onClick={(e) => {
                      e.stopPropagation();
                      setGrid9MenuOpen(false);
                      grid9Slot.onApplyTemplate?.(tpl.cols, tpl.rows);
                    }}
                  >
                    {tpl.label}
                  </button>
                ))}
                {grid9Slot.active && (
                  <button
                    type="button"
                    className="canvas-tab-bar__grid9-menu-item canvas-tab-bar__grid9-menu-item--exit"
                    data-bf-component="canvas-tab-bar"
                    data-bf-part="gridTemplateExit"
                    onClick={(e) => {
                      e.stopPropagation();
                      setGrid9MenuOpen(false);
                      grid9Slot.onToggle();
                    }}
                  >
                    {t('tabs.exitGrid')}
                  </button>
                )}
              </div>
            )}
          </div>
        )}

        {/* Merge cell (grid9 free split/merge): merge this cell's tabs into a
            neighbour so two small windows become one big window. */}
        {onMergeCell && canMergeCell && (
          <Tooltip content={t('tabs.mergeCell')} placement="bottom">
            <button
              type="button"
              data-bf-component="canvas-tab-bar"
              data-bf-part="action"
              className="canvas-tab-bar__action-btn canvas-tab-bar__merge-btn"
              onClick={(e) => {
                e.stopPropagation();
                onMergeCell();
              }}
              aria-label={t('tabs.mergeCell')}
            >
              <Combine size={14} />
            </button>
          </Tooltip>
        )}

        {/* Remove blank grid9 cell: shrink the grid and re-tile the rest so the
            remaining conversations fill the panel. */}
        {onRemoveCell && canRemoveCell && (
          <Tooltip content={t('tabs.removeCell')} placement="bottom">
            <button
              type="button"
              data-bf-component="canvas-tab-bar"
              data-bf-part="action"
              className="canvas-tab-bar__action-btn canvas-tab-bar__remove-cell-btn"
              onClick={(e) => {
                e.stopPropagation();
                onRemoveCell();
              }}
              aria-label={t('tabs.removeCell')}
            >
              <Trash2 size={14} />
            </button>
          </Tooltip>
        )}

        {/* Overflow menu (all groups; mission control only in primary) */}
        {visibleTabs.length > 0 && layoutReady && (
          <TabOverflowMenu
            overflowTabs={overflowTabs}
            activeTabId={activeTabId}
            onTabClick={onTabClick}
            onTabClose={onTabClose}
            onReorderTab={onReorderTab}
            onOpenMissionControl={onOpenMissionControl}
          />
        )}

        {/* Close all tabs button */}
        {onCloseAllTabs && visibleTabs.length > 0 && (
          <Tooltip content={t('tabs.closeAll')} placement="bottom">
            <button
              data-bf-component="canvas-tab-bar"
              data-bf-part="action"
              className="canvas-tab-bar__action-btn canvas-tab-bar__action-btn--close-all"
              onClick={async (e) => {
                e.stopPropagation();
                await onCloseAllTabs();
              }}
            >
              <X size={14} />
            </button>
          </Tooltip>
        )}
      </div>
    </div>
  );
};

TabBar.displayName = 'TabBar';

export default TabBar;
