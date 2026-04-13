/**
 * ContentCanvas main container component.
 * Core component for the right panel, aggregating submodules.
 */

import React, { useCallback, useEffect, useMemo, useRef } from 'react';
import { EditorArea } from './editor-area';
import { AnchorZone } from './anchor-zone';
import { MissionControl } from './mission-control';
import { EmptyState } from './empty-state';
import { useCanvasStore } from './stores';
import { useTabLifecycle, useKeyboardShortcuts, usePanelTabCoordinator } from './hooks';
import type { AnchorPosition } from './types';
import { openMainSession, selectActiveBtwSessionTab } from '@/flow_chat/services/openBtwSession';
import './ContentCanvas.scss';
export interface ContentCanvasProps {
  /** Workspace path */
  workspacePath?: string;
  /** App mode */
  mode?: 'agent' | 'project';
  /** Whether the containing scene is currently visible */
  isSceneActive?: boolean;
  /** Interaction callback */
  onInteraction?: (itemId: string, userInput: string) => Promise<void>;
  /** Before-close callback */
  onBeforeClose?: (content: any) => Promise<boolean>;
  /** Disable pop-out and panel-close controls (used in panel-view scene) */
  disablePopOut?: boolean;
}

export const ContentCanvas: React.FC<ContentCanvasProps> = ({
  workspacePath,
  mode = 'agent',
  isSceneActive = true,
  onInteraction,
  disablePopOut = false,
}) => {
  // Store state
  const {
    primaryGroup,
    layout,
    isMissionControlOpen,
    setAnchorPosition,
    setAnchorSize,
    closeMissionControl,
    openMissionControl,
  } = useCanvasStore();
  const activeBtwSessionTab = useCanvasStore(state => selectActiveBtwSessionTab(state as any));
  const activeBtwSessionData = activeBtwSessionTab?.content.data as
    | { childSessionId: string; parentSessionId: string; workspacePath?: string }
    | undefined;
  const lastSyncedBtwTabIdRef = useRef<string | null>(null);
  // Initialize hooks
  const { handleCloseWithDirtyCheck, handleCloseAllWithDirtyCheck } = useTabLifecycle({ mode });
  useKeyboardShortcuts({ enabled: true, handleCloseWithDirtyCheck });
  // Panel/tab state coordinator (auto manage expand/collapse)
  const { collapsePanel } = usePanelTabCoordinator({
    autoCollapseOnEmpty: true,
    autoExpandOnTabOpen: true,
  });

  useEffect(() => {
    if (mode !== 'agent' || !activeBtwSessionTab?.id || !activeBtwSessionData?.parentSessionId) {
      lastSyncedBtwTabIdRef.current = null;
      return;
    }

    if (lastSyncedBtwTabIdRef.current === activeBtwSessionTab.id) {
      return;
    }

    lastSyncedBtwTabIdRef.current = activeBtwSessionTab.id;
    void openMainSession(activeBtwSessionData.parentSessionId);
  }, [activeBtwSessionData?.parentSessionId, activeBtwSessionTab?.id, mode]);

  // Check if primary group has visible tabs
  const hasPrimaryVisibleTabs = useMemo(() => {
    const primaryVisible = primaryGroup.tabs.filter(t => !t.isHidden).length;
    return primaryVisible > 0;
  }, [primaryGroup.tabs]);

  // Handle anchor close
  const handleAnchorClose = useCallback(() => {
    setAnchorPosition('hidden');
  }, [setAnchorPosition]);

  // Handle anchor position change
  const handleAnchorPositionChange = useCallback((position: AnchorPosition) => {
    setAnchorPosition(position);
  }, [setAnchorPosition]);

  // Handle anchor size change
  const handleAnchorSizeChange = useCallback((size: number) => {
    setAnchorSize(size);
  }, [setAnchorSize]);

  // Handle mission control open
  const handleOpenMissionControl = useCallback(() => {
    openMissionControl();
  }, [openMissionControl]);

  // Handle mission control close
  const handleCloseMissionControl = useCallback(() => {
    closeMissionControl();
  }, [closeMissionControl]);

  // Render content
  const renderContent = () => {
    // Show empty state when primary group has no visible tabs
    if (!hasPrimaryVisibleTabs) {
      return <EmptyState onClose={disablePopOut ? undefined : collapsePanel} />;
    }

    return (
      <div className="canvas-content-canvas__main">
        {/* Editor area */}
        <div className="canvas-content-canvas__editor">
          <EditorArea
            workspacePath={workspacePath}
            isSceneActive={isSceneActive}
            onOpenMissionControl={handleOpenMissionControl}
            onInteraction={onInteraction}
            onTabCloseWithDirtyCheck={handleCloseWithDirtyCheck}
            onTabCloseAllWithDirtyCheck={handleCloseAllWithDirtyCheck}
            disablePopOut={disablePopOut}
          />
        </div>

        {/* Anchor area */}
        {layout.anchorPosition !== 'hidden' && (
          <AnchorZone
            position={layout.anchorPosition}
            size={layout.anchorSize}
            onSizeChange={handleAnchorSizeChange}
            onPositionChange={handleAnchorPositionChange}
            onClose={handleAnchorClose}
          >
            {/* Anchor content (e.g., terminal) renders here */}
            <div className="canvas-content-canvas__anchor-content">
            </div>
          </AnchorZone>
        )}
      </div>
    );
  };

  return (
    <div
      className={`canvas-content-canvas ${layout.isMaximized ? 'is-maximized' : ''}`}
      data-shortcut-scope="canvas"
    >
      {/* Main content */}
      {renderContent()}

      {/* Mission control overlay */}
      <MissionControl
        isOpen={isMissionControlOpen}
        onClose={handleCloseMissionControl}
        handleCloseWithDirtyCheck={handleCloseWithDirtyCheck}
      />
    </div>
  );
};
ContentCanvas.displayName = 'ContentCanvas';

export default ContentCanvas;
