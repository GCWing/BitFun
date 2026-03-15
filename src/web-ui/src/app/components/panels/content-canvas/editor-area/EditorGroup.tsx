/**
 * EditorGroup component.
 * A single editor group with tab bar and content area.
 */

import React, { useCallback, useMemo, useRef, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { TabBar } from '../tab-bar';
import { DropZone } from './DropZone';
import FlexiblePanel from '../../base/FlexiblePanel';
import type { 
  EditorGroupId, 
  EditorGroupState, 
  TabDragPayload,
  DropPosition,
  PanelContent,
  SplitMode,
} from '../types';
import './EditorGroup.scss';

export interface EditorGroupProps {
  groupId: EditorGroupId;
  group: EditorGroupState;
  isActive: boolean;
  draggingTabId: string | null;
  draggingFromGroupId: EditorGroupId | null;
  splitMode: SplitMode;
  workspacePath?: string;
  onTabClick: (tabId: string) => void;
  onTabDoubleClick: (tabId: string) => void;
  onTabClose: (tabId: string) => Promise<void> | void;
  onTabPin: (tabId: string) => void;
  onDragStart: (payload: TabDragPayload) => void;
  onDragEnd: () => void;
  onReorderTab: (tabId: string, newIndex: number) => void;
  onDrop: (position: DropPosition) => void;
  onGroupFocus: () => void;
  onContentChange: (tabId: string, content: PanelContent) => void;
  onDirtyStateChange: (tabId: string, isDirty: boolean) => void;
  onOpenMissionControl?: () => void;
  onCloseAllTabs?: () => Promise<void> | void;
  onInteraction?: (itemId: string, userInput: string) => Promise<void>;
}

export const EditorGroup: React.FC<EditorGroupProps> = ({
  groupId,
  group,
  isActive,
  draggingTabId,
  draggingFromGroupId,
  splitMode,
  workspacePath,
  onTabClick,
  onTabDoubleClick,
  onTabClose,
  onTabPin,
  onDragStart,
  onDragEnd,
  onReorderTab,
  onDrop,
  onGroupFocus,
  onContentChange,
  onDirtyStateChange,
  onOpenMissionControl,
  onCloseAllTabs,
  onInteraction,
}) => {
  const { t } = useTranslation('components');
  const visibleTabs = useMemo(() => group.tabs.filter(t => !t.isHidden), [group.tabs]);
  
  // Cache recently visited tabs (max 5) for instant switching
  const cachedTabsRef = useRef<Set<string>>(new Set());
  
  // Update cache: keep active tab and 4 most recent tabs
  useEffect(() => {
    // Remove closed tabs
    const validTabIds = new Set(group.tabs.filter(t => !t.isHidden).map(t => t.id));
    cachedTabsRef.current = new Set(
      Array.from(cachedTabsRef.current).filter(id => validTabIds.has(id))
    );
    
    // Add active tab
    if (group.activeTabId && validTabIds.has(group.activeTabId)) {
      cachedTabsRef.current.add(group.activeTabId);
      
      // If cache exceeds 5, keep active and 4 most recent
      if (cachedTabsRef.current.size > 5) {
        const sortedTabs = [...group.tabs]
          .filter(t => !t.isHidden && t.id !== group.activeTabId)
          .sort((a, b) => (b.lastAccessedAt || 0) - (a.lastAccessedAt || 0))
          .slice(0, 4)
          .map(t => t.id);
        
        cachedTabsRef.current = new Set([group.activeTabId, ...sortedTabs]);
      }
    }
  }, [group.activeTabId, group.tabs]);
  
  // Tabs to render (active + cached)
  const tabsToRender = useMemo(() => {
    const result = group.tabs.filter(t => 
      !t.isHidden && 
      (t.id === group.activeTabId || cachedTabsRef.current.has(t.id))
    );
    return result;
  }, [group.tabs, group.activeTabId]);

  const handleContentChange = useCallback((content: PanelContent | null) => {
    if (content && group.activeTabId) {
      onContentChange(group.activeTabId, content);
    }
  }, [group.activeTabId, onContentChange]);

  const handleDirtyStateChange = useCallback((isDirty: boolean) => {
    if (group.activeTabId) {
      onDirtyStateChange(group.activeTabId, isDirty);
    }
  }, [group.activeTabId, onDirtyStateChange]);

  const isDragging = draggingTabId !== null;

  return (
    <div
      className={`canvas-editor-group ${isActive ? 'is-active' : ''}`}
      onClick={onGroupFocus}
    >
      {/* Tab bar */}
      <TabBar
        tabs={group.tabs}
        groupId={groupId}
        activeTabId={group.activeTabId}
        isActiveGroup={isActive}
        onTabClick={onTabClick}
        onTabDoubleClick={onTabDoubleClick}
        onTabClose={onTabClose}
        onTabPin={onTabPin}
        onDragStart={onDragStart}
        onDragEnd={onDragEnd}
        draggingTabId={draggingTabId}
        onReorderTab={onReorderTab}
        onOpenMissionControl={onOpenMissionControl}
        onCloseAllTabs={onCloseAllTabs}
      />

      <DropZone
        groupId={groupId}
        isDragging={isDragging}
        draggingFromGroupId={draggingFromGroupId}
        splitMode={splitMode}
        onDrop={onDrop}
      >
        <div className="canvas-editor-group__content">
          {/* Render cached tabs (active shown, others hidden) for instant switching */}
          {tabsToRender.length > 0 ? (
            tabsToRender.map((tab) => (
              <div
                key={tab.id}
                className="canvas-editor-group__tab-content"
                style={{ display: group.activeTabId === tab.id ? 'flex' : 'none' }}
              >
                <FlexiblePanel
                  content={tab.content as any}
                  isActive={group.activeTabId === tab.id}
                  onContentChange={group.activeTabId === tab.id ? handleContentChange : undefined}
                  onDirtyStateChange={group.activeTabId === tab.id ? handleDirtyStateChange : undefined}
                  onInteraction={onInteraction}
                  workspacePath={workspacePath}
                />
              </div>
            ))
          ) : visibleTabs.length === 0 ? (
            <div className="canvas-editor-group__empty">
              <div className="canvas-editor-group__empty-content">
                <span>{t('canvas.dragTabHere')}</span>
              </div>
            </div>
          ) : null}
        </div>
      </DropZone>
    </div>
  );
};

EditorGroup.displayName = 'EditorGroup';

export default EditorGroup;
