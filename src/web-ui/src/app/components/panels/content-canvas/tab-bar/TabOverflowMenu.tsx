/**
 * TabOverflowMenu component.
 * Combines mission control entry and overflow tabs menu.
 * - Mission control without overflow: click to open mission control
 * - Overflow: show +N badge and dropdown; first item is mission control (if available)
 * - Overflow without mission control: show overflow menu only
 */

import React, { useState, useRef, useEffect, useCallback } from 'react';
import { createPortal } from 'react-dom';
import { LayoutGrid, ChevronDown, X } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { Tooltip } from '@/component-library';
import type { CanvasTab } from '../types';
import './TabOverflowMenu.scss';
export interface TabOverflowMenuProps {
  /** Overflow tabs */
  overflowTabs: CanvasTab[];
  /** Active tab ID */
  activeTabId: string | null;
  /** Tab click callback */
  onTabClick: (tabId: string) => void;
  /** Close tab callback */
  onTabClose: (tabId: string) => Promise<void> | void;
  /** Reorder tab callback (move to index) */
  onReorderTab: (tabId: string, newIndex: number) => void;
  /** Open mission control (optional, only for primary group) */
  onOpenMissionControl?: () => void;
}

export const TabOverflowMenu: React.FC<TabOverflowMenuProps> = ({
  overflowTabs,
  activeTabId,
  onTabClick,
  onTabClose,
  onReorderTab,
  onOpenMissionControl,
}) => {
  const { t } = useTranslation('components');
  const [isOpen, setIsOpen] = useState(false);
  const [menuPosition, setMenuPosition] = useState({ top: 0, left: 0 });
  const wrapperRef = useRef<HTMLDivElement>(null);
  const menuRef = useRef<HTMLDivElement>(null);

  const hasOverflow = overflowTabs.length > 0;
  const hasMissionControl = !!onOpenMissionControl;

  // Update menu position
  const updateMenuPosition = useCallback(() => {
    if (wrapperRef.current) {
      const rect = wrapperRef.current.getBoundingClientRect();
      const menuWidth = 240;
      
      // Compute left to keep menu within right boundary
      let left = rect.left;
      if (left + menuWidth > window.innerWidth) {
        left = rect.right - menuWidth;
      }
      
      setMenuPosition({
        top: rect.bottom + 4,
        left: Math.max(8, left),
      });
    }
  }, []);

  // Button click
  const handleButtonClick = useCallback(() => {
    if (hasOverflow) {
      if (!isOpen) {
        updateMenuPosition();
      }
      setIsOpen(prev => !prev);
    } else if (hasMissionControl) {
      onOpenMissionControl?.();
    }
  }, [hasOverflow, hasMissionControl, isOpen, updateMenuPosition, onOpenMissionControl]);

  // Close menu on outside click
  useEffect(() => {
    if (!isOpen) return;

    const handleClickOutside = (event: MouseEvent) => {
      const target = event.target as Node;
      if (
        menuRef.current &&
        !menuRef.current.contains(target) &&
        wrapperRef.current &&
        !wrapperRef.current.contains(target)
      ) {
        setIsOpen(false);
      }
    };

    // Delay listener to avoid triggering the current click
    const timer = setTimeout(() => {
      document.addEventListener('mousedown', handleClickOutside);
    }, 0);

    return () => {
      clearTimeout(timer);
      document.removeEventListener('mousedown', handleClickOutside);
    };
  }, [isOpen]);

  // Handle mission control click
  const handleMissionControlClick = useCallback(() => {
    onOpenMissionControl?.();
    setIsOpen(false);
  }, [onOpenMissionControl]);

  // Handle tab click
  const handleTabClick = useCallback((tabId: string) => {
    // Move tab to front (index 0) so it becomes visible
    onReorderTab(tabId, 0);
    // Then switch to the tab
    onTabClick(tabId);
    setIsOpen(false);
  }, [onTabClick, onReorderTab]);

  // Handle close click
  const handleCloseClick = useCallback(async (e: React.MouseEvent, tabId: string) => {
    e.stopPropagation();
    await onTabClose(tabId);
  }, [onTabClose]);

  // Decide whether to show button: overflow tabs or mission control
  const shouldShowButton = hasOverflow || hasMissionControl;
  
  // Hide button when no overflow and no mission control
  if (!shouldShowButton) {
    return null;
  }

  const tooltipContent = hasOverflow 
    ? hasMissionControl
      ? `${t('tabs.missionControl')} · ${t('tabs.hiddenTabsCount', { count: overflowTabs.length })}`
      : t('tabs.hiddenTabsCount', { count: overflowTabs.length })
    : hasMissionControl
      ? t('tabs.missionControl')
      : '';

  return (
    <div ref={wrapperRef} className="canvas-tab-panorama-wrapper">
      <Tooltip content={tooltipContent} placement="bottom">
        <button
          className={`canvas-tab-panorama-btn ${hasOverflow ? 'has-overflow' : ''} ${isOpen ? 'is-open' : ''} ${!hasMissionControl ? 'overflow-only' : ''}`}
          onClick={handleButtonClick}
        >
          {hasMissionControl ? (
            <LayoutGrid size={14} />
          ) : (
            <ChevronDown size={14} />
          )}
          {hasOverflow && (
            <span className="canvas-tab-panorama-btn__badge">
              +{overflowTabs.length}
            </span>
          )}
        </button>
      </Tooltip>

      {isOpen && hasOverflow && createPortal(
        <div
          ref={menuRef}
          className="canvas-tab-overflow-menu"
          style={{
            position: 'fixed',
            top: `${menuPosition.top}px`,
            left: `${menuPosition.left}px`,
          }}
        >
          {/* Mission control entry - shown only when available */}
          {hasMissionControl && (
            <>
              <div
                className="canvas-tab-overflow-menu__mission-control"
                onClick={handleMissionControlClick}
              >
                <LayoutGrid size={14} />
                <span>{t('tabs.missionControl')}</span>
                <kbd>⌘.</kbd>
              </div>

              {/* Divider */}
              <div className="canvas-tab-overflow-menu__divider" />
            </>
          )}

          {/* Overflow tab list */}
          <div className="canvas-tab-overflow-menu__list">
            {overflowTabs.map((tab) => {
              const deletedSuffix = tab.fileDeletedFromDisk ? ` - ${t('tabs.fileDeleted')}` : '';
              const titleWithDeleted = `${tab.title}${deletedSuffix}`;
              return (
              <div
                key={tab.id}
                className={`canvas-tab-overflow-menu__item ${
                  activeTabId === tab.id ? 'is-active' : ''
                } ${tab.isDirty ? 'is-dirty' : ''} ${tab.fileDeletedFromDisk ? 'is-file-deleted' : ''}`}
                onClick={() => handleTabClick(tab.id)}
              >
                <span className="canvas-tab-overflow-menu__item-title">
                  {tab.state === 'preview' && <em>{titleWithDeleted}</em>}
                  {tab.state !== 'preview' && titleWithDeleted}
                </span>
                
                {tab.isDirty && (
                  <span className="canvas-tab-overflow-menu__item-dirty">●</span>
                )}
                
                <button
                  className="canvas-tab-overflow-menu__item-close"
                  onClick={(e) => handleCloseClick(e, tab.id)}
                >
                  <X size={12} />
                </button>
              </div>
            );
            })}
          </div>
        </div>,
        document.body
      )}
    </div>
  );
};

TabOverflowMenu.displayName = 'TabOverflowMenu';

export default TabOverflowMenu;
