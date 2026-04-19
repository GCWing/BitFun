/**
 * WorkspaceBody — main workspace container.
 *
 * New full-width layout (top to bottom):
 *   UnifiedTopBar  (40px, always visible, spans full width)
 *   main-content   (flex:1, full width — SessionScene / OverlayScene)
 *
 * Floating layers rendered on top of main-content:
 *   SessionCapsule  — vertical pill for session navigation
 *   NotificationButton — bottom-right bell (opens NotificationCenter)
 *   FloatingFileTree — currently disabled/commented out
 */

import React from 'react';
import AgenticOSWorkspace from '../overlay/AgenticOSWorkspace';
import UnifiedTopBar from '../components/UnifiedTopBar/UnifiedTopBar';
import NotificationButton from '../components/TitleBar/NotificationButton';
import SessionCapsule from '../components/SessionCapsule/SessionCapsule';
import WorkspaceFooterActions from '../components/WorkspaceFooterActions/WorkspaceFooterActions';
import { useOverlayStore } from '../stores/overlayStore';
import { useLiveAppCatalogSync } from '../scenes/apps/live-app/hooks/useLiveAppCatalogSync';
import './WorkspaceBody.scss';

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
  const activeOverlay = useOverlayStore((s) => s.activeOverlay);
  useLiveAppCatalogSync();

  return (
    <div
      className={`bitfun-workspace-body${isEntering ? ' is-entering' : ''}${isExiting ? ' is-exiting' : ''} ${className}`}
    >
      {/* Full-width unified top bar */}
      <UnifiedTopBar
        activeOverlay={activeOverlay}
        onMinimize={onMinimize}
        onMaximize={onMaximize}
        onClose={onClose}
        isMaximized={isMaximized}
      />

      {/* Full-width content area */}
      <div className="bitfun-workspace-body__content">
        <AgenticOSWorkspace
          isEntering={isEntering}
          onMinimize={onMinimize}
          onMaximize={onMaximize}
          onClose={onClose}
          isMaximized={isMaximized}
        />
        {sceneOverlay}
      </div>

      {/* Floating session capsule */}
      <SessionCapsule />

      {/* Bottom-left floating: More menu (Dispatcher, Shell, …) */}
      <div className="bitfun-workspace-body__nav-footer">
        <WorkspaceFooterActions />
      </div>

      {/* Bottom-right: notification center trigger */}
      <div className="bitfun-workspace-body__notification-fab">
        <NotificationButton
          className="bitfun-workspace-body__notification-btn"
          tooltipPlacement="left"
        />
      </div>

      {/*
       * FloatingFileTree — CURRENTLY DISABLED
       * Uncomment and import when ready to enable.
       *
       * import FloatingFileTree from '../components/FloatingFileTree/FloatingFileTree';
       * <FloatingFileTree />
       */}
    </div>
  );
};

export default WorkspaceBody;
