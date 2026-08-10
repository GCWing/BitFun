import React, { useRef, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { EditorGroup } from './EditorGroup';
import { SplitHandle } from './SplitHandle';
import type { ExternalChatSessionPayload } from './DropZone';
import { useCanvasStore } from '../stores';
import { buildBtwSessionPanelContent } from '@/flow_chat/services/btwSessionPane';
import type { 
  EditorGroupId, 
  TabDragPayload, 
  DropPosition,
  PanelContent,
} from '../types';
import { EDITOR_GROUP_IDS, EDITOR_GROUP_ROW, EDITOR_GROUP_COL, GRID_MAX_DIM } from '../types/layout';
import './EditorArea.scss';
export interface EditorAreaProps {
  workspacePath?: string;
  isSceneActive?: boolean;
  onOpenMissionControl?: () => void;
  onInteraction?: (itemId: string, userInput: string) => Promise<void>;
  onTabCloseWithDirtyCheck?: (tabId: string, groupId: EditorGroupId) => Promise<boolean>;
  onTabCloseAllWithDirtyCheck?: (groupId: EditorGroupId) => Promise<boolean>;
  disablePopOut?: boolean;
  terminalResizeSuspended?: boolean;
}

export const EditorArea: React.FC<EditorAreaProps> = ({
  workspacePath,
  isSceneActive = true,
  onOpenMissionControl,
  onInteraction,
  onTabCloseWithDirtyCheck,
  onTabCloseAllWithDirtyCheck,
  disablePopOut = false,
  terminalResizeSuspended = false,
}) => {
  const containerRef = useRef<HTMLDivElement>(null);
  const topRowRef = useRef<HTMLDivElement>(null);
  const grid9Ref = useRef<HTMLDivElement>(null);
  const { t } = useTranslation('flow-chat');

  // Fine-grained selectors: subscribe to each slice/action individually so
  // unrelated store changes do not re-render the editor area.
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
  const draggingTabId = useCanvasStore(state => state.draggingTabId);
  const draggingFromGroupId = useCanvasStore(state => state.draggingFromGroupId);
  const switchToTab = useCanvasStore(state => state.switchToTab);
  const closeTab = useCanvasStore(state => state.closeTab);
  const closeAllTabs = useCanvasStore(state => state.closeAllTabs);
  const promoteTab = useCanvasStore(state => state.promoteTab);
  const togglePinTab = useCanvasStore(state => state.togglePinTab);
  const startDrag = useCanvasStore(state => state.startDrag);
  const endDrag = useCanvasStore(state => state.endDrag);
  const reorderTab = useCanvasStore(state => state.reorderTab);
  const handleDrop = useCanvasStore(state => state.handleDrop);
  const setSplitRatio = useCanvasStore(state => state.setSplitRatio);
  const setSplitRatio2 = useCanvasStore(state => state.setSplitRatio2);
  const setGrid9ColRatio = useCanvasStore(state => state.setGrid9ColRatio);
  const setGrid9RowRatio = useCanvasStore(state => state.setGrid9RowRatio);
  const setActiveGroup = useCanvasStore(state => state.setActiveGroup);
  const updateTabContent = useCanvasStore(state => state.updateTabContent);
  const setTabDirty = useCanvasStore(state => state.setTabDirty);
  const setTabFileDeletedFromDisk = useCanvasStore(state => state.setTabFileDeletedFromDisk);
  const addTab = useCanvasStore(state => state.addTab);
  const setSplitMode = useCanvasStore(state => state.setSplitMode);
  const applyGrid9Template = useCanvasStore(state => state.applyGrid9Template);
  const mergeGrid9Cells = useCanvasStore(state => state.mergeGrid9Cells);
  const removeGrid9Cell = useCanvasStore(state => state.removeGrid9Cell);

  /** All 16 groups keyed by slot id, in EDITOR_GROUP_IDS order. */
  const groupsById = {
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
  } as const;

  const handleTabClick = useCallback((groupId: EditorGroupId) => (tabId: string) => {
    switchToTab(tabId, groupId);
  }, [switchToTab]);

  const handleTabDoubleClick = useCallback((groupId: EditorGroupId) => (tabId: string) => {
    promoteTab(tabId, groupId);
  }, [promoteTab]);

  const handleTabClose = useCallback((groupId: EditorGroupId) => async (tabId: string) => {
    if (onTabCloseWithDirtyCheck) {
      await onTabCloseWithDirtyCheck(tabId, groupId);
      return;
    }
    closeTab(tabId, groupId);
  }, [closeTab, onTabCloseWithDirtyCheck]);

  const handleCloseAllTabs = useCallback((groupId: EditorGroupId) => async () => {
    if (onTabCloseAllWithDirtyCheck) {
      await onTabCloseAllWithDirtyCheck(groupId);
      return;
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

  // External chat session dropped into a group: add it as a btw-session tab
  // (rendered by BtwSessionPanel, same mechanism as subagent side-threads).
  // The tab lands in the target group; the 1-9 dynamic split chain is entered
  // via the grid toggle / progressive drags, not by a single-column jump.
  const handleExternalChatDrop = useCallback((groupId: EditorGroupId) => (payload: ExternalChatSessionPayload) => {
    const content = buildBtwSessionPanelContent(
      payload.sessionId,
      payload.sessionId,
      undefined,
      undefined,
      payload.title,
    );
    addTab(content, 'active', groupId);
    window.dispatchEvent(new CustomEvent('expand-right-panel'));
  }, [addTab]);

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
      isSceneActive={isSceneActive}
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
      onExternalChatDrop={handleExternalChatDrop(groupId)}
      onGroupFocus={handleGroupFocus(groupId)}
      onContentChange={handleContentChange(groupId)}
      onDirtyStateChange={handleDirtyStateChange(groupId)}
      onTabFileDeletedFromDiskChange={handleTabFileDeletedFromDiskChange(groupId)}
      onOpenMissionControl={groupId === 'primary' ? onOpenMissionControl : undefined}
      onCloseAllTabs={handleCloseAllTabs(groupId)}
      onInteraction={onInteraction}
      disablePopOut={disablePopOut}
      terminalResizeSuspended={terminalResizeSuspended}
      grid9Slot={groupId === 'primary' ? {
        active: layout.splitMode === 'grid9',
        onToggle: () => setSplitMode(layout.splitMode === 'grid9' ? 'none' : 'grid9'),
        label: t('layout.gridTemplate.label'),
        templates: [
          { cols: 2, rows: 2, label: t('layout.gridTemplate.four') },
          { cols: 3, rows: 2, label: t('layout.gridTemplate.six') },
          { cols: 3, rows: 3, label: t('layout.gridTemplate.nine') },
          { cols: 4, rows: 4, label: t('layout.gridTemplate.sixteen') },
        ],
        onApplyTemplate: (cols, rows) => applyGrid9Template(cols, rows),
      } : undefined}
      onMergeCell={(() => {
        // Merge this grid9 cell into a neighbour: prefer the left cell in the
        // same row (col > 0), otherwise the cell above (row > 0). "Merge two
        // small windows into one big window" — the free split/merge primitive.
        if (layout.splitMode !== 'grid9' || groupId === 'primary') return undefined;
        const row = EDITOR_GROUP_ROW[groupId];
        const col = EDITOR_GROUP_COL[groupId];
        let target: EditorGroupId | null = null;
        if (col > 0) {
          target = EDITOR_GROUP_IDS[row * GRID_MAX_DIM + (col - 1)];
        } else if (row > 0) {
          target = EDITOR_GROUP_IDS[(row - 1) * GRID_MAX_DIM + col];
        }
        if (!target) return undefined;
        return () => mergeGrid9Cells(groupId, target);
      })()}
      canMergeCell={
        layout.splitMode === 'grid9' && groupId !== 'primary' &&
        group.tabs.length > 0 && (EDITOR_GROUP_COL[groupId] > 0 || EDITOR_GROUP_ROW[groupId] > 0)
      }
      onRemoveCell={
        layout.splitMode === 'grid9' && group.tabs.length === 0
          ? () => removeGrid9Cell(groupId)
          : undefined
      }
      canRemoveCell={
        layout.splitMode === 'grid9' && group.tabs.length === 0 &&
        (layout.grid9ColsCount > 1 || layout.grid9RowsCount > 1)
      }
    />
  );

  const { splitMode, splitRatio, splitRatio2, grid9Cols, grid9Rows, grid9ColsCount, grid9RowsCount } = layout;

  if (splitMode === 'grid9') {
    // Dynamic cols×rows grid (1..GRID_MAX_DIM each) that fully tiles the right
    // panel: four-cell = 2×2, six-cell = 2×3 / 3×2, nine-cell = 3×3,
    // sixteen-cell = 4×4. Only the active rows/columns are rendered (no
    // invisible 4×4 frame), so the template truly fills the panel edge to edge.
    const rowGap = 2;  // px visual gap (explicit gap tracks between cells)
    const colGap = 2;
    const cols = grid9ColsCount;  // 1..GRID_MAX_DIM
    const rows = grid9RowsCount;  // 1..GRID_MAX_DIM
    // Build the CSS grid template as explicit alternating tracks:
    // [col0, gap, col1, gap, col2, ...] — (2*cols-1) columns and (2*rows-1) rows.
    // Gaps are real tracks so the SplitHandles can sit on them. Cells land on
    // track 2c+1 / 2r+1, column handles on 2c+2, row handles on 2r+2.
    // Ratios are stored as 1/GRID_MAX_DIM shares (grid9Cols/Rows), so for
    // cols<GRID_MAX_DIM the raw values must be NORMALIZED to sum to 1 —
    // otherwise 2 columns of 0.25fr only fill 2/4 of the panel and leave a
    // blank band (cells don't fill).
    const rawColRatios = Array.from({ length: cols }, (_, i) => grid9Cols[i] ?? 1 / GRID_MAX_DIM);
    const rawRowRatios = Array.from({ length: rows }, (_, i) => grid9Rows[i] ?? 1 / GRID_MAX_DIM);
    const colSum = rawColRatios.reduce((a, b) => a + b, 0) || 1;
    const rowSum = rawRowRatios.reduce((a, b) => a + b, 0) || 1;
    const colRatios = rawColRatios.map((r) => r / colSum);
    const rowRatios = rawRowRatios.map((r) => r / rowSum);
    const gridTemplateColumns = colRatios
      .map((r) => `${r}fr`)
      .join(` ${colGap}px `);
    const gridTemplateRows = rowRatios
      .map((r) => `${r}fr`)
      .join(` ${rowGap}px `);
    // Render cell at (row, col) with a column handle after it (except last col)
    // and a row handle after each row (except last row).
    const renderGrid9 = () => {
      const nodes: React.ReactNode[] = [];
      for (let r = 0; r < rows; r++) {
        for (let c = 0; c < cols; c++) {
          const gid = EDITOR_GROUP_IDS[r * GRID_MAX_DIM + c];
          nodes.push(
            <div
              key={gid}
              data-bf-component="canvas-editor-area"
              data-bf-part="grid9Cell"
              data-bf-group={gid}
              data-bf-state="active"
              className="canvas-editor-area__grid9-cell"
              style={{ gridColumn: 2 * c + 1, gridRow: 2 * r + 1 }}
            >
              {renderEditorGroup(gid, groupsById[gid])}
            </div>
          );
          if (c < cols - 1) {
            nodes.push(
              <SplitHandle
                key={`${gid}-colh`}
                direction="horizontal"
                ratio={colRatios[c]}
                onRatioChange={(nr) => setGrid9ColRatio(c, nr)}
                containerRef={grid9Ref}
                style={{ gridColumn: 2 * c + 2, gridRow: 2 * r + 1 }}
              />
            );
          }
        }
        if (r < rows - 1) {
          nodes.push(
            <SplitHandle
              key={`row-${r}`}
              direction="vertical"
              ratio={rowRatios[r]}
              onRatioChange={(nr) => setGrid9RowRatio(r, nr)}
              containerRef={grid9Ref}
              style={{ gridColumn: `1 / -1`, gridRow: 2 * r + 2 }}
            />
          );
        }
      }
      return nodes;
    };

    return (
      <div
        data-bf-component="canvas-editor-area"
        data-bf-part="root"
        data-bf-layout="grid9"
        ref={grid9Ref}
        className="canvas-editor-area is-grid9"
      >
        <div
          className="canvas-editor-area__grid9-canvas"
          style={{ gridTemplateColumns, gridTemplateRows }}
        >
          {renderGrid9()}
        </div>
      </div>
    );
  }

  if (splitMode === 'none') {
    return (
      <div data-bf-component="canvas-editor-area" data-bf-part="root" data-bf-layout="none" ref={containerRef} className="canvas-editor-area">
        <div data-bf-component="canvas-editor-area" data-bf-part="primary" className="canvas-editor-area__primary">
          {renderEditorGroup('primary', primaryGroup)}
        </div>
      </div>
    );
  }

  if (splitMode === 'horizontal') {
    return (
      <div data-bf-component="canvas-editor-area" data-bf-part="root" data-bf-layout="horizontal" ref={containerRef} className="canvas-editor-area is-split is-horizontal">
        <div data-bf-component="canvas-editor-area" data-bf-part="primary" className="canvas-editor-area__primary" style={{ width: `${splitRatio * 100}%` }}>
          {renderEditorGroup('primary', primaryGroup)}
        </div>
        <SplitHandle
          direction="horizontal"
          ratio={splitRatio}
          onRatioChange={setSplitRatio}
          containerRef={containerRef}
        />
        <div data-bf-component="canvas-editor-area" data-bf-part="secondary" className="canvas-editor-area__secondary" style={{ width: `${(1 - splitRatio) * 100}%` }}>
          {renderEditorGroup('secondary', secondaryGroup)}
        </div>
      </div>
    );
  }

  if (splitMode === 'vertical') {
    return (
      <div data-bf-component="canvas-editor-area" data-bf-part="root" data-bf-layout="vertical" ref={containerRef} className="canvas-editor-area is-split is-vertical">
        <div data-bf-component="canvas-editor-area" data-bf-part="primary" className="canvas-editor-area__primary" style={{ height: `${splitRatio * 100}%` }}>
          {renderEditorGroup('primary', primaryGroup)}
        </div>
        <SplitHandle
          direction="vertical"
          ratio={splitRatio}
          onRatioChange={setSplitRatio}
          containerRef={containerRef}
        />
        <div data-bf-component="canvas-editor-area" data-bf-part="secondary" className="canvas-editor-area__secondary" style={{ height: `${(1 - splitRatio) * 100}%` }}>
          {renderEditorGroup('secondary', secondaryGroup)}
        </div>
      </div>
    );
  }

  if (splitMode === 'grid') {
    return (
      <div data-bf-component="canvas-editor-area" data-bf-part="root" data-bf-layout="grid" ref={containerRef} className="canvas-editor-area is-grid">
        <div data-bf-component="canvas-editor-area" data-bf-part="topRow" ref={topRowRef} className="canvas-editor-area__top-row" style={{ flex: `0 0 calc(${splitRatio * 100}% - 2px)` }}>
          <div data-bf-component="canvas-editor-area" data-bf-part="primary" className="canvas-editor-area__primary" style={{ flex: `0 0 calc(${splitRatio2 * 100}% - 2px)` }}>
            {renderEditorGroup('primary', primaryGroup)}
          </div>
          <SplitHandle
            direction="horizontal"
            ratio={splitRatio2}
            onRatioChange={setSplitRatio2}
            containerRef={topRowRef}
          />
          <div data-bf-component="canvas-editor-area" data-bf-part="secondary" className="canvas-editor-area__secondary" style={{ flex: 1, minWidth: 0 }}>
            {renderEditorGroup('secondary', secondaryGroup)}
          </div>
        </div>
        <SplitHandle
          direction="vertical"
          ratio={splitRatio}
          onRatioChange={setSplitRatio}
          containerRef={containerRef}
        />
        <div data-bf-component="canvas-editor-area" data-bf-part="tertiary" className="canvas-editor-area__tertiary" style={{ flex: 1, minHeight: 0 }}>
          {renderEditorGroup('tertiary', tertiaryGroup)}
        </div>
      </div>
    );
  }

  return null;
};

EditorArea.displayName = 'EditorArea';

export default EditorArea;
