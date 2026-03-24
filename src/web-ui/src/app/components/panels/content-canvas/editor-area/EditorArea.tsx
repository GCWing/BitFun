import React, { useRef, useCallback } from 'react';
import { EditorGroup } from './EditorGroup';
import { SplitHandle } from './SplitHandle';
import { useCanvasStore } from '../stores';
import type { 
  EditorGroupId, 
  TabDragPayload, 
  DropPosition,
  PanelContent,
} from '../types';
import './EditorArea.scss';
export interface EditorAreaProps {
  workspacePath?: string;
  onOpenMissionControl?: () => void;
  onInteraction?: (itemId: string, userInput: string) => Promise<void>;
  onTabCloseWithDirtyCheck?: (tabId: string, groupId: EditorGroupId) => Promise<boolean>;
  onTabCloseAllWithDirtyCheck?: (groupId: EditorGroupId) => Promise<boolean>;
}

export const EditorArea: React.FC<EditorAreaProps> = ({
  workspacePath,
  onOpenMissionControl,
  onInteraction,
  onTabCloseWithDirtyCheck,
  onTabCloseAllWithDirtyCheck,
}) => {
  const containerRef = useRef<HTMLDivElement>(null);
  const topRowRef = useRef<HTMLDivElement>(null);

  const {
    primaryGroup,
    secondaryGroup,
    tertiaryGroup,
    activeGroupId,
    layout,
    draggingTabId,
    draggingFromGroupId,
    switchToTab,
    closeTab,
    closeAllTabs,
    promoteTab,
    togglePinTab,
    startDrag,
    endDrag,
    reorderTab,
    handleDrop,
    setSplitRatio,
    setSplitRatio2,
    setActiveGroup,
    updateTabContent,
    setTabDirty,
    setTabFileDeletedFromDisk,
  } = useCanvasStore();

  const handleTabClick = useCallback((groupId: EditorGroupId) => (tabId: string) => {
    switchToTab(tabId, groupId);
  }, [switchToTab]);

  const handleTabDoubleClick = useCallback((groupId: EditorGroupId) => (tabId: string) => {
    promoteTab(tabId, groupId);
  }, [promoteTab]);

  const handleTabClose = useCallback((groupId: EditorGroupId) => async (tabId: string) => {
    if (onTabCloseWithDirtyCheck) {
      const shouldClose = await onTabCloseWithDirtyCheck(tabId, groupId);
      if (!shouldClose) return;
    }
    closeTab(tabId, groupId);
  }, [closeTab, onTabCloseWithDirtyCheck]);

  const handleCloseAllTabs = useCallback((groupId: EditorGroupId) => async () => {
    if (onTabCloseAllWithDirtyCheck) {
      const shouldClose = await onTabCloseAllWithDirtyCheck(groupId);
      if (!shouldClose) return;
    }
    closeAllTabs(groupId);
  }, [closeAllTabs, onTabCloseAllWithDirtyCheck]);

  const handleTabPin = useCallback((groupId: EditorGroupId) => (tabId: string) => {
    togglePinTab(tabId, groupId);
  }, [togglePinTab]);

  const handleDragStart = useCallback((payload: TabDragPayload) => {
    startDrag(payload.tabId, payload.sourceGroupId);
  }, [startDrag]);

  const handleDragEnd = useCallback(() => {
    endDrag();
  }, [endDrag]);

  const handleReorderTab = useCallback((groupId: EditorGroupId) => (tabId: string, newIndex: number) => {
    reorderTab(tabId, groupId, newIndex);
  }, [reorderTab]);

  const handleDropOnGroup = useCallback((groupId: EditorGroupId) => (position: DropPosition) => {
    if (draggingTabId && draggingFromGroupId) {
      handleDrop(draggingTabId, draggingFromGroupId, groupId, position);
      endDrag();
    }
  }, [draggingTabId, draggingFromGroupId, handleDrop, endDrag]);

  const handleGroupFocus = useCallback((groupId: EditorGroupId) => () => {
    setActiveGroup(groupId);
  }, [setActiveGroup]);

  const handleContentChange = useCallback((groupId: EditorGroupId) => (tabId: string, content: PanelContent) => {
    updateTabContent(tabId, groupId, content);
  }, [updateTabContent]);

  const handleDirtyStateChange = useCallback((groupId: EditorGroupId) => (tabId: string, isDirty: boolean) => {
    setTabDirty(tabId, groupId, isDirty);
  }, [setTabDirty]);

  const handleTabFileDeletedFromDiskChange = useCallback(
    (groupId: EditorGroupId) => (tabId: string, missing: boolean) => {
      setTabFileDeletedFromDisk(tabId, groupId, missing);
    },
    [setTabFileDeletedFromDisk]
  );

  const renderEditorGroup = (groupId: EditorGroupId, group: typeof primaryGroup) => (
    <EditorGroup
      groupId={groupId}
      group={group}
      isActive={activeGroupId === groupId}
      draggingTabId={draggingTabId}
      draggingFromGroupId={draggingFromGroupId}
      splitMode={layout.splitMode}
      workspacePath={workspacePath}
      onTabClick={handleTabClick(groupId)}
      onTabDoubleClick={handleTabDoubleClick(groupId)}
      onTabClose={handleTabClose(groupId)}
      onTabPin={handleTabPin(groupId)}
      onDragStart={handleDragStart}
      onDragEnd={handleDragEnd}
      onReorderTab={handleReorderTab(groupId)}
      onDrop={handleDropOnGroup(groupId)}
      onGroupFocus={handleGroupFocus(groupId)}
      onContentChange={handleContentChange(groupId)}
      onDirtyStateChange={handleDirtyStateChange(groupId)}
      onTabFileDeletedFromDiskChange={handleTabFileDeletedFromDiskChange(groupId)}
      onOpenMissionControl={groupId === 'primary' ? onOpenMissionControl : undefined}
      onCloseAllTabs={handleCloseAllTabs(groupId)}
      onInteraction={onInteraction}
    />
  );

  const { splitMode, splitRatio, splitRatio2 } = layout;

  if (splitMode === 'none') {
    return (
      <div ref={containerRef} className="canvas-editor-area">
        <div className="canvas-editor-area__primary">
          {renderEditorGroup('primary', primaryGroup)}
        </div>
      </div>
    );
  }

  if (splitMode === 'horizontal') {
    return (
      <div ref={containerRef} className="canvas-editor-area is-split is-horizontal">
        <div className="canvas-editor-area__primary" style={{ width: `${splitRatio * 100}%` }}>
          {renderEditorGroup('primary', primaryGroup)}
        </div>
        <SplitHandle
          direction="horizontal"
          ratio={splitRatio}
          onRatioChange={setSplitRatio}
          containerRef={containerRef}
        />
        <div className="canvas-editor-area__secondary" style={{ width: `${(1 - splitRatio) * 100}%` }}>
          {renderEditorGroup('secondary', secondaryGroup)}
        </div>
      </div>
    );
  }

  if (splitMode === 'vertical') {
    return (
      <div ref={containerRef} className="canvas-editor-area is-split is-vertical">
        <div className="canvas-editor-area__primary" style={{ height: `${splitRatio * 100}%` }}>
          {renderEditorGroup('primary', primaryGroup)}
        </div>
        <SplitHandle
          direction="vertical"
          ratio={splitRatio}
          onRatioChange={setSplitRatio}
          containerRef={containerRef}
        />
        <div className="canvas-editor-area__secondary" style={{ height: `${(1 - splitRatio) * 100}%` }}>
          {renderEditorGroup('secondary', secondaryGroup)}
        </div>
      </div>
    );
  }

  if (splitMode === 'grid') {
    return (
      <div ref={containerRef} className="canvas-editor-area is-grid">
        <div ref={topRowRef} className="canvas-editor-area__top-row" style={{ flex: `0 0 calc(${splitRatio * 100}% - 2px)` }}>
          <div className="canvas-editor-area__primary" style={{ flex: `0 0 calc(${splitRatio2 * 100}% - 2px)` }}>
            {renderEditorGroup('primary', primaryGroup)}
          </div>
          <SplitHandle
            direction="horizontal"
            ratio={splitRatio2}
            onRatioChange={setSplitRatio2}
            containerRef={topRowRef}
          />
          <div className="canvas-editor-area__secondary" style={{ flex: 1, minWidth: 0 }}>
            {renderEditorGroup('secondary', secondaryGroup)}
          </div>
        </div>
        <SplitHandle
          direction="vertical"
          ratio={splitRatio}
          onRatioChange={setSplitRatio}
          containerRef={containerRef}
        />
        <div className="canvas-editor-area__tertiary" style={{ flex: 1, minHeight: 0 }}>
          {renderEditorGroup('tertiary', tertiaryGroup)}
        </div>
      </div>
    );
  }

  return null;
};

EditorArea.displayName = 'EditorArea';

export default EditorArea;
