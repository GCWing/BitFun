/**
 * MissionControl component.
 * Mission control overlay showing thumbnails of all open files.
 */

import React, { useState, useEffect, useMemo, useCallback, useRef } from 'react';
import { X, Merge } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { useDismissibleLayer } from '@/infrastructure/hooks/useDismissibleLayer';
import { ThumbnailCard } from './ThumbnailCard';
import { SearchFilter } from './SearchFilter';
import { useCanvasStore } from '../stores';
import { EDITOR_GROUP_IDS } from '../types';
import type { CanvasTab, EditorGroupId } from '../types';
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
  const [selectedGroups, setSelectedGroups] = useState<Set<EditorGroupId>>(new Set(EDITOR_GROUP_IDS));
  const [, setDraggingTabId] = useState<string | null>(null);
  // Fine-grained selectors so unrelated store changes do not re-render.
  const primaryGroup = useCanvasStore(state => state.primaryGroup);
  const secondaryGroup = useCanvasStore(state => state.secondaryGroup);
  const tertiaryGroup = useCanvasStore(state => state.tertiaryGroup);
  const slot4Group = useCanvasStore(state => state.slot4Group);
  const slot5Group = useCanvasStore(state => state.slot5Group);
  const slot6Group = useCanvasStore(state => state.slot6Group);
  const slot7Group = useCanvasStore(state => state.slot7Group);
  const slot8Group = useCanvasStore(state => state.slot8Group);
  const slot9Group = useCanvasStore(state => state.slot9Group);
  const slot10Group = useCanvasStore(state => state.slot10Group);
  const slot11Group = useCanvasStore(state => state.slot11Group);
  const slot12Group = useCanvasStore(state => state.slot12Group);
  const slot13Group = useCanvasStore(state => state.slot13Group);
  const slot14Group = useCanvasStore(state => state.slot14Group);
  const slot15Group = useCanvasStore(state => state.slot15Group);
  const slot16Group = useCanvasStore(state => state.slot16Group);
  const activeGroupId = useCanvasStore(state => state.activeGroupId);
  const layout = useCanvasStore(state => state.layout);
  const switchToTab = useCanvasStore(state => state.switchToTab);
  const closeTab = useCanvasStore(state => state.closeTab);
  const togglePinTab = useCanvasStore(state => state.togglePinTab);
  const setSplitMode = useCanvasStore(state => state.setSplitMode);
  useDismissibleLayer({
    enabled: isOpen,
    scope: 'canvas',
    onDismiss: onClose,
    id: 'canvas-mission-control',
  });

  const groupsById = useMemo(() => ({
    primary: primaryGroup,
    secondary: secondaryGroup,
    tertiary: tertiaryGroup,
    slot4: slot4Group,
    slot5: slot5Group,
    slot6: slot6Group,
    slot7: slot7Group,
    slot8: slot8Group,
    slot9: slot9Group,
    slot10: slot10Group,
    slot11: slot11Group,
    slot12: slot12Group,
    slot13: slot13Group,
    slot14: slot14Group,
    slot15: slot15Group,
    slot16: slot16Group,
  } as const), [
    primaryGroup,
    secondaryGroup,
    tertiaryGroup,
    slot4Group,
    slot5Group,
    slot6Group,
    slot7Group,
    slot8Group,
    slot9Group,
    slot10Group,
    slot11Group,
    slot12Group,
    slot13Group,
    slot14Group,
    slot15Group,
    slot16Group,
  ]);

  // Organize tabs by group
  const organizedTabs = useMemo(() => {
    const entries = EDITOR_GROUP_IDS.map((id) => ({
      groupId: id,
      tabs: groupsById[id].tabs.filter(t => !t.isHidden).map(tab => ({ tab, groupId: id as EditorGroupId })),
    }));
    const all = entries.flatMap(e => e.tabs);
    const byId = Object.fromEntries(entries.map(e => [e.groupId, e.tabs])) as Record<EditorGroupId, { tab: CanvasTab; groupId: EditorGroupId }[]>;
    return { ...byId, all } as Record<EditorGroupId, { tab: CanvasTab; groupId: EditorGroupId }[]> & { all: { tab: CanvasTab; groupId: EditorGroupId }[] };
  }, [groupsById]);

  // Aggregate all tabs (for search and stats)
  const allTabs = organizedTabs.all;

  // Filter matching tabs (search + group filter)
  const filteredTabs = useMemo(() => {
    let result = allTabs;
    
    // Filter by group first
    if (selectedGroups.size < EDITOR_GROUP_IDS.length) {
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
    return groupsById[activeGroupId].activeTabId;
  }, [activeGroupId, groupsById]);

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
      setSelectedGroups(new Set(EDITOR_GROUP_IDS));
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

  /** Short slot label: 1..9 in row-major order. */
  const slotLabel = useCallback((id: EditorGroupId): string => {
    const idx = EDITOR_GROUP_IDS.indexOf(id);
    return String(idx + 1);
  }, []);

  // Merge all groups into primary
  const handleMergeAll = useCallback(() => {
    setSplitMode('none');
    onClose();
  }, [setSplitMode, onClose]);

  if (!isOpen) {
    return null;
  }

  return (
    <div data-bf-component="mission-control" data-bf-part="root" data-bf-state="open"
      ref={rootRef}
      className="canvas-mission-control"
      data-shortcut-scope="canvas"
      tabIndex={-1}
      onClick={handleBackdropClick}
    >
      <div className="canvas-mission-control__content" data-bf-component="mission-control" data-bf-part="content">
        {/* Header */}
        <div data-bf-component="mission-control" data-bf-part="header" className="canvas-mission-control__header">
          <h2 className="canvas-mission-control__title">{t('tabs.missionControl')}</h2>
          <div data-bf-component="mission-control" data-bf-part="headerActions" className="canvas-mission-control__header-actions">
            {hasMultipleGroups && (
              <button
                className="canvas-mission-control__merge-btn"
                onClick={handleMergeAll}
                title={t('canvas.mergeAllGroups')}
              >
                <Merge size={14} />
                <span>{t('canvas.mergeAll')}</span>
              </button>
            )}
            <button
              className="canvas-mission-control__close-btn"
              onClick={onClose}
            >
              <X size={14} />
            </button>
          </div>
        </div>

        {/* Search and filter area */}
        <div data-bf-component="mission-control" data-bf-part="filters" className="canvas-mission-control__filters">
          <div className="canvas-mission-control__filters-row">
            <div data-bf-component="mission-control" data-bf-part="search" className="canvas-mission-control__search-wrapper">
              <SearchFilter
                value={searchQuery}
                onChange={setSearchQuery}
                matchCount={filteredTabs.length}
                totalCount={allTabs.length}
              />
            </div>
            
            {/* Group filters - compact icon buttons */}
            {hasMultipleGroups && (
              <div data-bf-component="mission-control" data-bf-part="groupFilters" className="canvas-mission-control__group-filters">
                {EDITOR_GROUP_IDS.map((id) => {
                  const group = groupsById[id];
                  const hasTabs = group.tabs.filter(t => !t.isHidden).length > 0;
                  if (!hasTabs) return null;
                  
                  return (
                    <button data-bf-component="mission-control" data-bf-part="filter" data-bf-group={id} data-bf-state={selectedGroups.has(id) ? 'active' : ''}
                      key={id}
                      className={`canvas-mission-control__group-filter canvas-mission-control__group-filter--${id} ${selectedGroups.has(id) ? 'is-active' : ''}`}
                      onClick={() => toggleGroupFilter(id)}
                      title={t('canvas.groupSlot', { slot: slotLabel(id) })}
                    >
                      <span className="canvas-mission-control__group-filter-indicator" />
                      <span className="canvas-mission-control__group-filter-text">{slotLabel(id)}</span>
                    </button>
                  );
                })}
              </div>
            )}
          </div>
        </div>

        {/* Thumbnail grid - unified display */}
        <div data-bf-component="mission-control" data-bf-part="grid" className="canvas-mission-control__grid">
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
            <div data-bf-component="mission-control" data-bf-part="empty" className="canvas-mission-control__empty">
              {searchQuery || selectedGroups.size < EDITOR_GROUP_IDS.length ? (
                <span>{t('canvas.noMatchingFiles')}</span>
              ) : (
                <span>{t('canvas.noOpenFiles')}</span>
              )}
            </div>
          )}
        </div>

        {/* Footer hint */}
        <div data-bf-component="mission-control" data-bf-part="footer" className="canvas-mission-control__footer">
          <span>{t('canvas.clickToSwitch')}</span>
          <div data-bf-component="mission-control" data-bf-part="separator" className="canvas-mission-control__separator" />
          <span><kbd>Esc</kbd> {t('canvas.exit')}</span>
        </div>
      </div>
    </div>
  );
};

MissionControl.displayName = 'MissionControl';

export default MissionControl;
