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

import React, { useCallback, useState } from 'react';
import { useCurrentWorkspace } from '../../infrastructure/contexts/WorkspaceContext';
import { NavBar } from '../components/NavBar';
import NavPanel from '../components/NavPanel/NavPanel';
import PersistentFooterActions from '../components/NavPanel/components/PersistentFooterActions';
import { SceneBar } from '../components/SceneBar';
import { SceneViewport } from '../scenes';
import { useApp } from '../hooks/useApp';
import './WorkspaceBody.scss';

const NAV_DEFAULT_WIDTH = 240;
const NAV_MIN_WIDTH = 240;
const NAV_MAX_WIDTH = 480;
const COLLAPSE_THRESHOLD = 64;

interface WorkspaceBodyProps {
  className?: string;
  isEntering?: boolean;
  isExiting?: boolean;
  onMinimize?: () => void;
  onMaximize?: () => void;
  onClose?: () => void;
  isMaximized?: boolean;
  sceneOverlay?: React.ReactNode;
}

const WorkspaceBody: React.FC<WorkspaceBodyProps> = ({
  className = '',
  isEntering = false,
  isExiting = false,
  onMinimize,
  onMaximize,
  onClose,
  isMaximized = false,
  sceneOverlay,
}) => {
  const { workspace: currentWorkspace } = useCurrentWorkspace();
  const { state, toggleLeftPanel } = useApp();
  const isNavCollapsed = state.layout.leftPanelCollapsed;
  const [navWidth, setNavWidth] = useState(NAV_DEFAULT_WIDTH);
  const [isDividerHovered, setIsDividerHovered] = useState(false);

  const handleDividerMouseEnter = useCallback(() => {
    setIsDividerHovered(true);
    document.body.classList.add('bitfun-divider-hovered');
  }, []);

  const handleDividerMouseLeave = useCallback(() => {
    setIsDividerHovered(false);
    document.body.classList.remove('bitfun-divider-hovered');
  }, []);

  const handleNavCollapseDragStart = useCallback((event: React.MouseEvent<HTMLDivElement>) => {
    if (event.button !== 0 || isNavCollapsed) return;
    event.preventDefault();

    const startX = event.clientX;
    const startWidth = navWidth;
    let hasCollapsed = false;

    document.body.classList.add('bitfun-is-dragging-nav-collapse');
    document.body.classList.add('bitfun-is-resizing-nav');

    const handleMouseMove = (moveEvent: MouseEvent) => {
      if (hasCollapsed) return;
      const deltaX = moveEvent.clientX - startX;
      const rawWidth = startWidth + deltaX;

      // Collapse only after the width hits minimum AND continues left by COLLAPSE_THRESHOLD
      if (rawWidth <= NAV_MIN_WIDTH - COLLAPSE_THRESHOLD) {
        hasCollapsed = true;
        toggleLeftPanel();
        cleanup();
        return;
      }
      const newWidth = Math.min(NAV_MAX_WIDTH, Math.max(NAV_MIN_WIDTH, rawWidth));
      setNavWidth(newWidth);
    };

    const handleMouseUp = () => cleanup();

    function cleanup() {
      document.body.classList.remove('bitfun-is-dragging-nav-collapse');
      document.body.classList.remove('bitfun-is-resizing-nav');
      document.body.classList.remove('bitfun-divider-hovered');
      window.removeEventListener('mousemove', handleMouseMove);
      window.removeEventListener('mouseup', handleMouseUp);
    }

    window.addEventListener('mousemove', handleMouseMove);
    window.addEventListener('mouseup', handleMouseUp);
  }, [isNavCollapsed, navWidth, toggleLeftPanel]);

  return (
    <div className={`bitfun-workspace-body${isEntering ? ' is-entering' : ''}${isExiting ? ' is-exiting' : ''}${isDividerHovered ? ' is-divider-hovered' : ''} ${className}`}>
      {isNavCollapsed && (
        <div className="bitfun-workspace-body__collapsed-nav">
          <NavBar isCollapsed onExpandNav={toggleLeftPanel} onMaximize={onMaximize} />
        </div>
      )}

      {/* Left: nav history bar + navigation sidebar — always rendered for slide animation */}
      <div
        className={`bitfun-workspace-body__nav-area${isNavCollapsed ? ' is-collapsed' : ''}`}
        style={isNavCollapsed ? undefined : { '--nav-width': `${navWidth}px` } as React.CSSProperties}
      >
        <NavBar onExpandNav={toggleLeftPanel} onMaximize={onMaximize} />
        <NavPanel className="bitfun-workspace-body__nav-panel" />
      </div>

      {/* Resize divider — placed at workspace-body level to avoid overflow:hidden clipping */}
      {!isNavCollapsed && (
        <div
          className="bitfun-workspace-body__nav-divider"
          style={{ '--nav-width': `${navWidth}px` } as React.CSSProperties}
          onMouseDown={handleNavCollapseDragStart}
          onMouseEnter={handleDividerMouseEnter}
          onMouseLeave={handleDividerMouseLeave}
          role="separator"
          aria-hidden="true"
        />
      )}

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
        {sceneOverlay}
      </div>

      {/* Independent footer bar — always visible regardless of nav collapse state */}
      <div className="bitfun-workspace-body__nav-footer">
        <PersistentFooterActions />
      </div>
    </div>
  );
};

export default WorkspaceBody;
