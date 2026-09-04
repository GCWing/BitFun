/**
 * MissionControl component.
 * Mission control overlay showing thumbnails of all open files.
 */

import React, { useState, useEffect, useMemo, useCallback, useRef } from 'react';
import { Button, Icon, IconButton, KeyHint, ScrollArea, useDismissibleLayer } from '@openbitfun/ui';
import { Merge } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { ThumbnailCard } from './ThumbnailCard';
import { SearchFilter } from './SearchFilter';
import { useCanvasStore } from '../stores';
import type { EditorGroupId } from '../types';
import './MissionControl.scss';

export interface MissionControlProps {
  /** Whether open */
  isOpen: boolean;
  /** Close callback */
  onClose: () => void;
  /** Dirty-check callback before closing tab */
  handleCloseWithDirtyCheck?: (tabId: string, groupId: EditorGroupId) => Promise<boolean>;
}

export const MissionControl: React.FC<MissionControlProps> = ({
  isOpen,
  onClose,
  handleCloseWithDirtyCheck,
}) => {
  const { t } = useTranslation('components');
  const rootRef = useRef<HTMLDivElement>(null);
  const [searchQuery, setSearchQuery] = useState('');
  const [selectedGroups, setSelectedGroups] = useState<Set<EditorGroupId>>(new Set(['primary', 'secondary', 'tertiary']));
  const [, setDraggingTabId] = useState<string | null>(null);
  // Fine-grained selectors so unrelated store changes do not re-render.
  const primaryGroup = useCanvasStore(state => state.primaryGroup);
  const secondaryGroup = useCanvasStore(state => state.secondaryGroup);
  const tertiaryGroup = useCanvasStore(state => state.tertiaryGroup);
  const activeGroupId = useCanvasStore(state => state.activeGroupId);
  const layout = useCanvasStore(state => state.layout);
  const switchToTab = useCanvasStore(state => state.switchToTab);
  const closeTab = useCanvasStore(state => state.closeTab);
  const togglePinTab = useCanvasStore(state => state.togglePinTab);
  const setSplitMode = useCanvasStore(state => state.setSplitMode);
  useDismissibleLayer({
    dismissOnPointerOutside: false,
    enabled: isOpen,
    layerRef: rootRef,
    scope: 'canvas',
    onDismiss: onClose,
  });

  // Organize tabs by group
  const organizedTabs = useMemo(() => {
    const primary = primaryGroup.tabs
      .filter(t => !t.isHidden)
      .map(t => ({ tab: t, groupId: 'primary' as EditorGroupId }));
    const secondary = secondaryGroup.tabs
      .filter(t => !t.isHidden)
      .map(t => ({ tab: t, groupId: 'secondary' as EditorGroupId }));
    const tertiary = tertiaryGroup.tabs
      .filter(t => !t.isHidden)
      .map(t => ({ tab: t, groupId: 'tertiary' as EditorGroupId }));

    return {
      primary,
      secondary,
      tertiary,
      all: [...primary, ...secondary, ...tertiary],
    };
  }, [primaryGroup.tabs, secondaryGroup.tabs, tertiaryGroup.tabs]);

  // Aggregate all tabs (for search and stats)
  const allTabs = organizedTabs.all;

  // Filter matching tabs (search + group filter)
  const filteredTabs = useMemo(() => {
    let result = allTabs;
    
    // Filter by group first
    if (selectedGroups.size < 3) {
      result = result.filter(({ groupId }) => selectedGroups.has(groupId));
    }
    
    // Then filter by search query
    if (searchQuery.trim()) {
      const query = searchQuery.toLowerCase();
      result = result.filter(({ tab }) => {
        return (
          tab.title.toLowerCase().includes(query) ||
          tab.content.data?.filePath?.toLowerCase().includes(query) ||
          tab.content.type.toLowerCase().includes(query)
        );
      });
    }
    
    return result;
  }, [allTabs, searchQuery, selectedGroups]);

  // Active tab ID
  const activeTabId = useMemo(() => {
    const group = activeGroupId === 'primary' 
      ? primaryGroup 
      : activeGroupId === 'secondary' 
        ? secondaryGroup 
        : tertiaryGroup;
    return group.activeTabId;
  }, [activeGroupId, primaryGroup, secondaryGroup, tertiaryGroup]);

  useEffect(() => {
    if (!isOpen) return;
    rootRef.current?.focus({ preventScroll: true });
  }, [isOpen]);

  // Close on backdrop click
  const handleBackdropClick = useCallback((e: React.MouseEvent) => {
    if (e.target === e.currentTarget) {
      onClose();
    }
  }, [onClose]);

  // Handle tab click
  const handleTabClick = useCallback((tabId: string, groupId: EditorGroupId) => {
    switchToTab(tabId, groupId);
    onClose();
  }, [switchToTab, onClose]);

  // Handle tab close
  const handleTabClose = useCallback(async (tabId: string, groupId: EditorGroupId) => {
    if (handleCloseWithDirtyCheck) {
      await handleCloseWithDirtyCheck(tabId, groupId);
      return;
    }
    closeTab(tabId, groupId);
  }, [closeTab, handleCloseWithDirtyCheck]);

  // Handle pin
  const handleTabPin = useCallback((tabId: string, groupId: EditorGroupId) => {
    togglePinTab(tabId, groupId);
  }, [togglePinTab]);

  // Drag start
  const handleDragStart = useCallback((tabId: string) => (_e: React.DragEvent) => {
    setDraggingTabId(tabId);
  }, []);

  // Drag end
  const handleDragEnd = useCallback(() => {
    setDraggingTabId(null);
  }, []);

  // Reset search and filters
  useEffect(() => {
    if (!isOpen) {
      setSearchQuery('');
      setSelectedGroups(new Set(['primary', 'secondary', 'tertiary']));
    }
  }, [isOpen]);

  // Toggle group filter
  const toggleGroupFilter = useCallback((groupId: EditorGroupId) => {
    setSelectedGroups(prev => {
      const next = new Set(prev);
      if (next.has(groupId)) {
        next.delete(groupId);
      } else {
        next.add(groupId);
      }
      return next;
    });
  }, []);

  // Check for multiple groups
  const hasMultipleGroups = useMemo(() => {
    return layout.splitMode !== 'none';
  }, [layout.splitMode]);

  // Merge all groups into primary
  const handleMergeAll = useCallback(() => {
    setSplitMode('none');
    onClose();
  }, [setSplitMode, onClose]);

  if (!isOpen) {
    return null;
  }

  return (
    <div data-openbitfun-component="mission-control" data-openbitfun-part="root" data-openbitfun-state="open"
      ref={rootRef}
      className="canvas-mission-control"
      data-shortcut-scope="canvas"
      tabIndex={-1}
      onClick={handleBackdropClick}
    >
      <div className="canvas-mission-control__content" data-openbitfun-component="mission-control" data-openbitfun-part="content">
        {/* Header */}
        <div data-openbitfun-component="mission-control" data-openbitfun-part="header" className="canvas-mission-control__header">
          <h2 className="canvas-mission-control__title">{t('tabs.missionControl')}</h2>
          <div data-openbitfun-component="mission-control" data-openbitfun-part="headerActions" className="canvas-mission-control__header-actions">
            {hasMultipleGroups && (
              <Button
                variant="outline"
                size="sm"
                leadingIcon={<Merge />}
                onClick={handleMergeAll}
                title={t('canvas.mergeAllGroups')}
              >
                {t('canvas.mergeAll')}
              </Button>
            )}
            <IconButton
              size="sm"
              aria-label={t('tooltip.close')}
              icon={<Icon name="xmark" size="lg" />}
              onClick={onClose}
              title={t('tooltip.close')}
            />
          </div>
        </div>

        {/* Search and filter area */}
        <div data-openbitfun-component="mission-control" data-openbitfun-part="filters" className="canvas-mission-control__filters">
          <div className="canvas-mission-control__filters-row">
            <div data-openbitfun-component="mission-control" data-openbitfun-part="search" className="canvas-mission-control__search-wrapper">
              <SearchFilter
                value={searchQuery}
                onChange={setSearchQuery}
                matchCount={filteredTabs.length}
                totalCount={allTabs.length}
              />
            </div>
            
            {/* Group filters - compact icon buttons */}
            {hasMultipleGroups && (
              <div data-openbitfun-component="mission-control" data-openbitfun-part="groupFilters" className="canvas-mission-control__group-filters">
                {[
                  { id: 'primary' as EditorGroupId, labelKey: 'canvas.groupPrimaryFull', shortLabelKey: 'canvas.groupPrimary' },
                  { id: 'secondary' as EditorGroupId, labelKey: 'canvas.groupSecondaryFull', shortLabelKey: 'canvas.groupSecondary' },
                  { id: 'tertiary' as EditorGroupId, labelKey: 'canvas.groupTertiaryFull', shortLabelKey: 'canvas.groupTertiary' },
                ].map(({ id, labelKey, shortLabelKey }) => {
                  const hasTabs = organizedTabs[id as keyof typeof organizedTabs].length > 0;
                  if (!hasTabs) return null;
                  
                  return (
                    <button data-openbitfun-component="mission-control" data-openbitfun-part="filter" data-openbitfun-group={id} data-openbitfun-state={selectedGroups.has(id) ? 'active' : ''}
                      key={id}
                      className={`canvas-mission-control__group-filter canvas-mission-control__group-filter--${id} ${selectedGroups.has(id) ? 'is-active' : ''}`}
                      onClick={() => toggleGroupFilter(id)}
                      title={t(labelKey)}
                    >
                      <span className="canvas-mission-control__group-filter-indicator" />
                      <span className="canvas-mission-control__group-filter-text">{t(shortLabelKey)}</span>
                    </button>
                  );
                })}
              </div>
            )}
          </div>
        </div>

        {/* Thumbnail grid - unified display */}
        <ScrollArea data-openbitfun-component="mission-control" data-openbitfun-part="grid" className="canvas-mission-control__grid">
          {filteredTabs.length > 0 ? (
            filteredTabs.map(({ tab, groupId }) => (
              <ThumbnailCard
                key={tab.id}
                tab={tab}
                groupId={groupId}
                isActive={tab.id === activeTabId && groupId === activeGroupId}
                onClick={() => handleTabClick(tab.id, groupId)}
                onClose={() => handleTabClose(tab.id, groupId)}
                onPin={() => handleTabPin(tab.id, groupId)}
                onDragStart={handleDragStart(tab.id)}
                onDragEnd={handleDragEnd}
              />
            ))
          ) : (
            <div data-openbitfun-component="mission-control" data-openbitfun-part="empty" className="canvas-mission-control__empty">
              {searchQuery || selectedGroups.size < 3 ? (
                <span>{t('canvas.noMatchingFiles')}</span>
              ) : (
                <span>{t('canvas.noOpenFiles')}</span>
              )}
            </div>
          )}
        </ScrollArea>

        {/* Footer hint */}
        <div data-openbitfun-component="mission-control" data-openbitfun-part="footer" className="canvas-mission-control__footer">
          <span>{t('canvas.clickToSwitch')}</span>
          <div data-openbitfun-component="mission-control" data-openbitfun-part="separator" className="canvas-mission-control__separator" />
          <span><KeyHint>Esc</KeyHint> {t('canvas.exit')}</span>
        </div>
      </div>
    </div>
  );
};

MissionControl.displayName = 'MissionControl';

export default MissionControl;
