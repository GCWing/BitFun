/**
 * AnchorZone component.
 * Anchor container for fixed content like terminal/output.
 */

import React, { useRef, useState, useCallback, useEffect } from 'react';
import { Maximize2, Minimize2 } from 'lucide-react';
import { useTranslation } from 'react-i18next';

import type { AnchorPosition } from '../types';
import { LAYOUT_CONFIG, clampAnchorSize } from '../types';
import './AnchorZone.scss';
import { Icon, Tooltip } from '@bitfun/ui';

export interface AnchorZoneProps {
  /** Position */
  position: AnchorPosition;
  /** Size */
  size: number;
  /** Whether maximized */
  isMaximized?: boolean;
  /** Size change callback */
  onSizeChange: (size: number) => void;
  /** Position change callback */
  onPositionChange: (position: AnchorPosition) => void;
  /** Close callback */
  onClose: () => void;
  /** Toggle maximize callback */
  onToggleMaximize?: () => void;
  /** Children */
  children: React.ReactNode;
}

export const AnchorZone: React.FC<AnchorZoneProps> = ({
  position,
  size,
  isMaximized = false,
  onSizeChange,
  onClose,
  onToggleMaximize,
  children,
}) => {
  const { t } = useTranslation('components');
  const containerRef = useRef<HTMLDivElement>(null);
  const [isResizing, setIsResizing] = useState(false);
  const [isCollapsed, setIsCollapsed] = useState(false);
  const startPosRef = useRef(0);
  const startSizeRef = useRef(size);

  const isBottom = position === 'bottom';

  // Handle resize start
  const handleResizeStart = useCallback((e: React.MouseEvent) => {
    e.preventDefault();
    setIsResizing(true);
    startPosRef.current = isBottom ? e.clientY : e.clientX;
    startSizeRef.current = size;
  }, [isBottom, size]);

  // Handle resizing
  useEffect(() => {
    if (!isResizing) return;

    const handleMouseMove = (e: MouseEvent) => {
      const currentPos = isBottom ? e.clientY : e.clientX;
      const delta = startPosRef.current - currentPos;
      const newSize = clampAnchorSize(startSizeRef.current + delta);
      onSizeChange(newSize);
    };

    const handleMouseUp = () => {
      setIsResizing(false);
    };

    document.addEventListener('mousemove', handleMouseMove);
    document.addEventListener('mouseup', handleMouseUp);

    return () => {
      document.removeEventListener('mousemove', handleMouseMove);
      document.removeEventListener('mouseup', handleMouseUp);
    };
  }, [isResizing, isBottom, onSizeChange]);

  // Double click to reset size
  const handleDoubleClick = useCallback(() => {
    onSizeChange(LAYOUT_CONFIG.DEFAULT_ANCHOR_SIZE);
  }, [onSizeChange]);

  // Collapse/expand
  const toggleCollapse = useCallback(() => {
    setIsCollapsed(!isCollapsed);
  }, [isCollapsed]);

  // Toggle position
  if (position === 'hidden') {
    return null;
  }

  return (
    <div data-bf-component="content-canvas" data-bf-part="anchor" data-bf-position={position}
      data-bf-state={[isResizing && 'resizing', isCollapsed && 'collapsed', isMaximized && 'maximized'].filter(Boolean).join(' ')}
      ref={containerRef}
      className={`canvas-anchor-zone canvas-anchor-zone--${position} ${
        isResizing ? 'is-resizing' : ''
      } ${isCollapsed ? 'is-collapsed' : ''} ${isMaximized ? 'is-maximized' : ''}`}
      style={isCollapsed ? undefined : {
        [isBottom ? 'height' : 'width']: isMaximized ? '100%' : `${size}px`,
      }}
    >
      {/* Resize handle */}
      <Tooltip content={t('canvas.dragToResize')}>
        <div
          className="canvas-anchor-zone__resizer"
          data-bf-component="content-canvas"
          data-bf-part="anchorResizer"
          onMouseDown={handleResizeStart}
          onDoubleClick={handleDoubleClick}
        >
          <div className="canvas-anchor-zone__resizer-line" />
        </div>
      </Tooltip>

      {/* Header */}
      <div className="canvas-anchor-zone__header" data-bf-component="content-canvas" data-bf-part="anchorHeader">
        <div className="canvas-anchor-zone__title" data-bf-component="content-canvas" data-bf-part="anchorTitle">
          <Icon name="terminal" size="sm" />
          <span>{t('canvas.terminal')}</span>
        </div>

        <div className="canvas-anchor-zone__actions" data-bf-component="content-canvas" data-bf-part="anchorActions">
          {/* Collapse/expand */}
          <Tooltip content={isCollapsed ? t('tooltip.expand') : t('tooltip.collapse')}>
            <button
              className="canvas-anchor-zone__action-btn"
              onClick={toggleCollapse}
            >
              {isCollapsed ? (
                isBottom ? <Icon name="chevron-up" size="sm" /> : <Icon name="chevron-up" size="sm" />
              ) : (
                isBottom ? <Icon name="chevron-down" size="sm" /> : <Icon name="chevron-down" size="sm" />
              )}
            </button>
          </Tooltip>

          {/* Maximize */}
          {onToggleMaximize && (
            <Tooltip content={isMaximized ? t('windowControls.restore') : t('windowControls.maximize')}>
              <button
                className="canvas-anchor-zone__action-btn"
                onClick={onToggleMaximize}
              >
                {isMaximized ? <Minimize2 size={14} /> : <Maximize2 size={14} />}
              </button>
            </Tooltip>
          )}

          {/* Close */}
          <Tooltip content={t('tooltip.close')}>
            <button
              className="canvas-anchor-zone__action-btn canvas-anchor-zone__close-btn"
              onClick={onClose}
            >
              <Icon name="xmark" size="sm" />
            </button>
          </Tooltip>
        </div>
      </div>

      {/* Content */}
      {!isCollapsed && (
        <div className="canvas-anchor-zone__content" data-bf-component="content-canvas" data-bf-part="anchorContent">
          {children}
        </div>
      )}
    </div>
  );
};

AnchorZone.displayName = 'AnchorZone';

export default AnchorZone;
