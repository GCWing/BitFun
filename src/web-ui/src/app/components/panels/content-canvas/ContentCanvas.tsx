/**
 * ContentCanvas main container component.
 * Core component for the right panel, aggregating submodules.
 */

import React, { useEffect, useCallback, useMemo } from 'react';
import { EditorArea } from './editor-area';
import { AnchorZone } from './anchor-zone';
import { MissionControl } from './mission-control';
import { EmptyState } from './empty-state';
import { useCanvasStore } from './stores';
import { useTabLifecycle, useKeyboardShortcuts, usePanelTabCoordinator } from './hooks';
import type { AnchorPosition } from './types';
import { createLogger } from '@/shared/utils/logger';
import './ContentCanvas.scss';

const log = createLogger('ContentCanvas');

export interface ContentCanvasProps {
  /** Workspace path */
  workspacePath?: string;
  /** App mode */
  mode?: 'agent' | 'project' | 'git';
  /** Interaction callback */
  onInteraction?: (itemId: string, userInput: string) => Promise<void>;
  /** Before-close callback */
  onBeforeClose?: (content: any) => Promise<boolean>;
}

export const ContentCanvas: React.FC<ContentCanvasProps> = ({
  workspacePath,
  mode = 'agent',
  onInteraction,
  onBeforeClose,
}) => {
  // Store state
  const {
    primaryGroup,
    secondaryGroup,
    layout,
    isMissionControlOpen,
    setAnchorPosition,
    setAnchorSize,
    closeMissionControl,
    openMissionControl,
  } = useCanvasStore();

  // Initialize hooks
  const { handleCloseWithDirtyCheck, handleCloseAllWithDirtyCheck } = useTabLifecycle({ mode });
  useKeyboardShortcuts({ enabled: true, handleCloseWithDirtyCheck });
  // Panel/tab state coordinator (auto manage expand/collapse)
  usePanelTabCoordinator({
    autoCollapseOnEmpty: true,
    autoExpandOnTabOpen: true,
  });

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
      return <EmptyState />;
    }

    return (
      <div className="canvas-content-canvas__main">
        {/* Editor area */}
        <div className="canvas-content-canvas__editor">
          <EditorArea
            workspacePath={workspacePath}
            onOpenMissionControl={handleOpenMissionControl}
            onInteraction={onInteraction}
            onTabCloseWithDirtyCheck={handleCloseWithDirtyCheck}
            onTabCloseAllWithDirtyCheck={handleCloseAllWithDirtyCheck}
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
    <div className={`canvas-content-canvas ${layout.isMaximized ? 'is-maximized' : ''}`}>
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
