/**
 * WorkspaceBody — main workspace container.
 *
 * Left-right layout:
 *   .nav-area   (240px, flex-column)
 *     NavBar        (32px — back/forward + drag + WindowControls)
 *     NavPanel      (flex:1 — navigation sidebar)
 *   .scene-area (flex:1, flex-column)
 *     SceneBar      (32px — scene tab strip)
 *     SceneViewport (flex:1 — active scene content)
 */

import React, { useCallback, useRef } from 'react';
import { useCurrentWorkspace } from '../../infrastructure/contexts/WorkspaceContext';
import { NavBar } from '../components/NavBar';
import NavPanel from '../components/NavPanel/NavPanel';
import { SceneBar } from '../components/SceneBar';
import { SceneViewport } from '../scenes';
import { useApp } from '../hooks/useApp';
import './WorkspaceBody.scss';

interface WorkspaceBodyProps {
  className?: string;
  isEntering?: boolean;
  isExiting?: boolean;
  onMinimize?: () => void;
  onMaximize?: () => void;
  onClose?: () => void;
  isMaximized?: boolean;
}

const WorkspaceBody: React.FC<WorkspaceBodyProps> = ({
  className = '',
  isEntering = false,
  isExiting = false,
  onMinimize,
  onMaximize,
  onClose,
  isMaximized = false,
}) => {
  const { workspace: currentWorkspace } = useCurrentWorkspace();
  const { state, toggleLeftPanel } = useApp();
  const collapseDragRef = useRef<{ startX: number; hasCollapsed: boolean } | null>(null);
  const isNavCollapsed = state.layout.leftPanelCollapsed;

  const handleNavCollapseDragStart = useCallback((event: React.MouseEvent<HTMLDivElement>) => {
    if (event.button !== 0 || isNavCollapsed) return;
    event.preventDefault();

    const COLLAPSE_THRESHOLD = 64;
    collapseDragRef.current = { startX: event.clientX, hasCollapsed: false };
    document.body.classList.add('bitfun-is-dragging-nav-collapse');

    const cleanup = () => {
      collapseDragRef.current = null;
      document.body.classList.remove('bitfun-is-dragging-nav-collapse');
      window.removeEventListener('mousemove', handleMouseMove);
      window.removeEventListener('mouseup', handleMouseUp);
    };

    const handleMouseMove = (moveEvent: MouseEvent) => {
      const dragState = collapseDragRef.current;
      if (!dragState || dragState.hasCollapsed) return;
      const deltaX = moveEvent.clientX - dragState.startX;
      if (deltaX <= -COLLAPSE_THRESHOLD) {
        dragState.hasCollapsed = true;
        toggleLeftPanel();
        cleanup();
      }
    };

    const handleMouseUp = () => cleanup();

    window.addEventListener('mousemove', handleMouseMove);
    window.addEventListener('mouseup', handleMouseUp);
  }, [isNavCollapsed, toggleLeftPanel]);

  return (
    <div className={`bitfun-workspace-body${isEntering ? ' is-entering' : ''}${isExiting ? ' is-exiting' : ''} ${className}`}>
      {isNavCollapsed && (
        <div className="bitfun-workspace-body__collapsed-nav">
          <NavBar isCollapsed onExpandNav={toggleLeftPanel} onMaximize={onMaximize} />
        </div>
      )}

      {/* Left: nav history bar + navigation sidebar — always rendered for slide animation */}
      <div className={`bitfun-workspace-body__nav-area${isNavCollapsed ? ' is-collapsed' : ''}`}>
        <NavBar onMaximize={onMaximize} />
        <NavPanel className="bitfun-workspace-body__nav-panel" />
        {!isNavCollapsed && (
          <div
            className="bitfun-workspace-body__nav-divider"
            onMouseDown={handleNavCollapseDragStart}
            role="separator"
            aria-hidden="true"
          />
        )}
      </div>

      {/* Right: scene tab bar + scene content */}
      <div className="bitfun-workspace-body__scene-area">
        <SceneBar
          onMinimize={onMinimize}
          onMaximize={onMaximize}
          onClose={onClose}
          isMaximized={isMaximized}
        />
        <SceneViewport
          workspacePath={currentWorkspace?.rootPath}
          isEntering={isEntering}
        />
      </div>
    </div>
  );
};

export default WorkspaceBody;
