/**
 * Workspace layout component with a three-column floating layout.
 *
 * Key features:
 * - Three-column layout: left panel + flexible center + right panel
 * - Fixed left/right widths with a flexible center (flex: 1)
 * - Resizing left does not affect right, and vice versa
 * - Center panel absorbs width changes as a buffer
 *
 * Enhancements:
 * - Panel display modes: collapsed / compact / comfortable / expanded
 * - Snap behavior at key thresholds
 * - Width memory: restore size after expand
 * - Shortcuts: Ctrl+\\ toggle left, Ctrl+] toggle right
 * - Double-click resizer toggles compact/comfortable
 * - Full ARIA accessibility support
 * - Touch-friendly behavior
 * - Smoother animations and visual feedback
 * - Min widths: left 200px (compact), right 300px (compact)
 * - Note: drag-to-collapse is removed; use buttons or shortcuts
 */

import React, { useRef, useState, useCallback, useEffect, useMemo } from 'react';
import { Trans, useTranslation } from 'react-i18next';
import { useApp } from '../hooks/useApp';
import { useCurrentWorkspace } from '../../infrastructure/contexts/WorkspaceContext';
import { createLogger } from '@/shared/utils/logger';

const log = createLogger('WorkspaceLayout');

// Panel components
import { LeftPanel, CenterPanel, RightPanel, type RightPanelRef } from '../components/panels';

// Panel config
import {
  LEFT_PANEL_CONFIG,
  RIGHT_PANEL_CONFIG,
  PANEL_COMMON_CONFIG,
  PANEL_SHORTCUTS,
  STORAGE_KEYS,
  PanelDisplayMode,
  getPanelDisplayMode,
  getModeWidth,
  getSnappedWidth,
  getNextMode,
  savePanelWidth,
  loadPanelWidth,
} from './panelConfig';

import './WorkspaceLayout.scss';

interface WorkspaceLayoutProps {
  className?: string;
  /** Whether entry animation is active */
  isEntering?: boolean;
}

const WorkspaceLayout: React.FC<WorkspaceLayoutProps> = ({
  className = '',
  isEntering = false
}) => {
  const { t } = useTranslation('flow-chat');
  const { state, switchLeftPanelTab, updateLeftPanelWidth, updateRightPanelWidth, toggleRightPanel, toggleLeftPanel } = useApp();
  const { workspace: currentWorkspace } = useCurrentWorkspace();
  const rightPanelRef = useRef<RightPanelRef>(null);
  
  // Dragging state
  const [isDraggingLeft, setIsDraggingLeft] = useState(false);
  const [isDraggingRight, setIsDraggingRight] = useState(false);
  const [isDraggingEditor, setIsDraggingEditor] = useState(false);
  const [isHoveringLeft, setIsHoveringLeft] = useState(false);
  const [isHoveringRight, setIsHoveringRight] = useState(false);
  const [isHoveringEditor, setIsHoveringEditor] = useState(false);
  
  
  // Width memory for restoring after expand
  const [lastLeftWidth, setLastLeftWidth] = useState<number>(() => 
    loadPanelWidth(STORAGE_KEYS.LEFT_PANEL_LAST_WIDTH, LEFT_PANEL_CONFIG.COMFORTABLE_DEFAULT)
  );
  const [lastRightWidth, setLastRightWidth] = useState<number>(() => 
    loadPanelWidth(STORAGE_KEYS.RIGHT_PANEL_LAST_WIDTH, RIGHT_PANEL_CONFIG.COMFORTABLE_DEFAULT)
  );
  
  // Container refs for boundary calculations
  const containerRef = useRef<HTMLDivElement>(null);
  const leftPanelElementRef = useRef<HTMLDivElement | null>(null);
  const leftResizerRef = useRef<HTMLDivElement>(null);
  const rightResizerRef = useRef<HTMLDivElement>(null);
  const editorResizerRef = useRef<HTMLDivElement>(null);
  const animationFrameRef = useRef<number | null>(null);

  // Current rendered widths
  const currentLeftWidth = state.layout.leftPanelWidth || LEFT_PANEL_CONFIG.COMFORTABLE_DEFAULT;
  const currentRightWidth = state.layout.rightPanelWidth || RIGHT_PANEL_CONFIG.COMFORTABLE_DEFAULT;

  // Compute current panel display modes
  const leftPanelMode: PanelDisplayMode = useMemo(() => {
    if (state.layout.leftPanelCollapsed) return 'collapsed';
    return getPanelDisplayMode(currentLeftWidth, LEFT_PANEL_CONFIG);
  }, [state.layout.leftPanelCollapsed, currentLeftWidth]);

  const rightPanelMode: PanelDisplayMode = useMemo(() => {
    if (state.layout.rightPanelCollapsed) return 'collapsed';
    return getPanelDisplayMode(currentRightWidth, RIGHT_PANEL_CONFIG);
  }, [state.layout.rightPanelCollapsed, currentRightWidth]);

  // Keep right panel visible when chat is hidden.
  useEffect(() => {
    if (state.layout.chatCollapsed && state.layout.rightPanelCollapsed) {
      toggleRightPanel();
    }
  }, [state.layout.chatCollapsed, state.layout.rightPanelCollapsed, toggleRightPanel]);

  /**
   * Calculate valid left panel width.
   */
  const calculateValidLeftWidth = useCallback((newWidth: number): number => {
    if (!containerRef.current) return newWidth;
    
    const containerWidth = containerRef.current.offsetWidth;
    const rightSpace = state.layout.rightPanelCollapsed ? 0 : currentRightWidth + PANEL_COMMON_CONFIG.RESIZER_WIDTH;
    const requiredSpace = state.layout.chatCollapsed
      ? rightSpace
      : PANEL_COMMON_CONFIG.MIN_CENTER_WIDTH + rightSpace + PANEL_COMMON_CONFIG.RESIZER_WIDTH;
    const dynamicMaxWidth = containerWidth - requiredSpace;
    const maxWidth = Math.min(LEFT_PANEL_CONFIG.MAX_WIDTH, dynamicMaxWidth);
    
    // Use compact width as minimum (not collapse threshold).
    const minWidth = LEFT_PANEL_CONFIG.COMPACT_WIDTH;
    
    return Math.min(maxWidth, Math.max(minWidth, newWidth));
  }, [state.layout.chatCollapsed, currentRightWidth, state.layout.rightPanelCollapsed]);

  /**
   * Calculate valid right panel width.
   */
  const calculateValidRightWidth = useCallback((newWidth: number): number => {
    if (!containerRef.current) return newWidth;
    
    const containerWidth = containerRef.current.offsetWidth;
    const leftSpace = state.layout.leftPanelCollapsed ? 0 : currentLeftWidth + PANEL_COMMON_CONFIG.RESIZER_WIDTH;
    const requiredSpace = PANEL_COMMON_CONFIG.MIN_CENTER_WIDTH + leftSpace + PANEL_COMMON_CONFIG.RESIZER_WIDTH;
    const dynamicMaxWidth = containerWidth - requiredSpace;
    const maxWidth = Math.min(RIGHT_PANEL_CONFIG.MAX_WIDTH, dynamicMaxWidth);
    
    const minWidth = RIGHT_PANEL_CONFIG.COMPACT_WIDTH;
    
    return Math.min(maxWidth, Math.max(minWidth, newWidth));
  }, [currentLeftWidth, state.layout.leftPanelCollapsed]);

  /**
   * Persist width and update memory.
   */
  const saveAndUpdateLeftWidth = useCallback((width: number) => {
    updateLeftPanelWidth(width);
    setLastLeftWidth(width);
    savePanelWidth(STORAGE_KEYS.LEFT_PANEL_LAST_WIDTH, width);
  }, [updateLeftPanelWidth]);

  const saveAndUpdateRightWidth = useCallback((width: number) => {
    updateRightPanelWidth(width);
    setLastRightWidth(width);
    savePanelWidth(STORAGE_KEYS.RIGHT_PANEL_LAST_WIDTH, width);
  }, [updateRightPanelWidth]);

  /**
   * Double-click to toggle left panel mode.
   */
  const handleDoubleClickLeft = useCallback(() => {
    const nextMode = getNextMode(leftPanelMode);
    const targetWidth = getModeWidth(nextMode, LEFT_PANEL_CONFIG);
    const validWidth = calculateValidLeftWidth(targetWidth);
    saveAndUpdateLeftWidth(validWidth);
  }, [leftPanelMode, calculateValidLeftWidth, saveAndUpdateLeftWidth]);

  /**
   * Double-click to toggle right panel mode.
   */
  const handleDoubleClickRight = useCallback(() => {
    const nextMode = getNextMode(rightPanelMode);
    const targetWidth = getModeWidth(nextMode, RIGHT_PANEL_CONFIG);
    const validWidth = calculateValidRightWidth(targetWidth);
    saveAndUpdateRightWidth(validWidth);
  }, [rightPanelMode, calculateValidRightWidth, saveAndUpdateRightWidth]);

  /**
   * Double-click the resizer in editor mode.
   */
  const handleDoubleClickEditor = useCallback(() => {
    const nextMode = getNextMode(leftPanelMode);
    const targetWidth = getModeWidth(nextMode, LEFT_PANEL_CONFIG);
    const validWidth = calculateValidLeftWidth(targetWidth);
    saveAndUpdateLeftWidth(validWidth);
  }, [leftPanelMode, calculateValidLeftWidth, saveAndUpdateLeftWidth]);

  /**
   * Toggle left panel (restore remembered width).
   */
  const handleToggleLeftPanel = useCallback(() => {
    if (state.layout.leftPanelCollapsed) {
      // Restore remembered width on expand
      const restoredWidth = calculateValidLeftWidth(lastLeftWidth);
      updateLeftPanelWidth(restoredWidth);
    }
    toggleLeftPanel();
  }, [state.layout.leftPanelCollapsed, lastLeftWidth, calculateValidLeftWidth, updateLeftPanelWidth, toggleLeftPanel]);

  /**
   * Toggle right panel (restore remembered width).
   */
  const handleToggleRightPanel = useCallback(() => {
    if (state.layout.rightPanelCollapsed) {
      // Restore remembered width on expand
      const restoredWidth = calculateValidRightWidth(lastRightWidth);
      updateRightPanelWidth(restoredWidth);
    }
    toggleRightPanel();
  }, [state.layout.rightPanelCollapsed, lastRightWidth, calculateValidRightWidth, updateRightPanelWidth, toggleRightPanel]);

  /**
   * Handle left resizer mouse drag.
   */
  const handleMouseDownLeft = useCallback((e: React.MouseEvent) => {
    e.preventDefault();
    
    if (!containerRef.current) return;

    const startX = e.clientX;
    const startWidth = currentLeftWidth;
    let lastValidWidth = startWidth;
    
    setIsDraggingLeft(true);
    document.body.style.cursor = 'col-resize';
    document.body.style.userSelect = 'none';

    const handleMouseMove = (moveEvent: MouseEvent) => {
      if (animationFrameRef.current !== null) {
        cancelAnimationFrame(animationFrameRef.current);
      }

      animationFrameRef.current = requestAnimationFrame(() => {
        const deltaX = moveEvent.clientX - startX;
        const newWidth = startWidth + deltaX;
        const validWidth = calculateValidLeftWidth(newWidth);
        
        lastValidWidth = validWidth;
        
        // Perf: update DOM directly during drag to avoid CenterPanel/FlowChat re-render.
        if (leftPanelElementRef.current) {
          leftPanelElementRef.current.style.width = `${validWidth}px`;
        } else {
          updateLeftPanelWidth(validWidth);
        }
        
        animationFrameRef.current = null;
      });
    };

    const handleMouseUp = () => {
      if (animationFrameRef.current !== null) {
        cancelAnimationFrame(animationFrameRef.current);
      }
      
      document.removeEventListener('mousemove', handleMouseMove);
      document.removeEventListener('mouseup', handleMouseUp);
      document.body.style.cursor = '';
      document.body.style.userSelect = '';
      
      const snappedWidth = getSnappedWidth(lastValidWidth, LEFT_PANEL_CONFIG, false);
      
      if (snappedWidth !== lastValidWidth) {
        saveAndUpdateLeftWidth(snappedWidth);
      } else {
        updateLeftPanelWidth(lastValidWidth);
        setLastLeftWidth(lastValidWidth);
        savePanelWidth(STORAGE_KEYS.LEFT_PANEL_LAST_WIDTH, lastValidWidth);
      }
      
      // Clear dragging on the next frame to avoid flicker with layout/transition updates.
      requestAnimationFrame(() => {
        requestAnimationFrame(() => setIsDraggingLeft(false));
      });
    };

    document.addEventListener('mousemove', handleMouseMove);
    document.addEventListener('mouseup', handleMouseUp);
  }, [currentLeftWidth, calculateValidLeftWidth, updateLeftPanelWidth, saveAndUpdateLeftWidth]);

  /**
   * Handle right resizer mouse drag.
   */
  const handleMouseDownRight = useCallback((e: React.MouseEvent) => {
    e.preventDefault();
    
    if (!containerRef.current) return;

    const startX = e.clientX;
    const startWidth = currentRightWidth;
    let lastValidWidth = startWidth;
    
    setIsDraggingRight(true);
    document.body.style.cursor = 'col-resize';
    document.body.style.userSelect = 'none';

    const handleMouseMove = (moveEvent: MouseEvent) => {
      if (animationFrameRef.current !== null) {
        cancelAnimationFrame(animationFrameRef.current);
      }

      animationFrameRef.current = requestAnimationFrame(() => {
        const deltaX = startX - moveEvent.clientX;
        const newWidth = startWidth + deltaX;
        const validWidth = calculateValidRightWidth(newWidth);
        
        lastValidWidth = validWidth;
        
        // Perf: update DOM directly during drag to avoid CenterPanel/FlowChat re-render.
        if (rightPanelElementRef.current && !state.layout.chatCollapsed) {
          rightPanelElementRef.current.style.width = `${validWidth}px`;
        } else {
          updateRightPanelWidth(validWidth);
        }
        
        animationFrameRef.current = null;
      });
    };

    const handleMouseUp = () => {
      if (animationFrameRef.current !== null) {
        cancelAnimationFrame(animationFrameRef.current);
      }
      
      document.removeEventListener('mousemove', handleMouseMove);
      document.removeEventListener('mouseup', handleMouseUp);
      document.body.style.cursor = '';
      document.body.style.userSelect = '';
      
      const snappedWidth = getSnappedWidth(lastValidWidth, RIGHT_PANEL_CONFIG, false);
      if (snappedWidth !== lastValidWidth) {
        saveAndUpdateRightWidth(snappedWidth);
      } else {
        updateRightPanelWidth(lastValidWidth);
        setLastRightWidth(lastValidWidth);
        savePanelWidth(STORAGE_KEYS.RIGHT_PANEL_LAST_WIDTH, lastValidWidth);
      }
      
      requestAnimationFrame(() => {
        requestAnimationFrame(() => setIsDraggingRight(false));
      });
    };

    document.addEventListener('mousemove', handleMouseMove);
    document.addEventListener('mouseup', handleMouseUp);
  }, [currentRightWidth, calculateValidRightWidth, updateRightPanelWidth, saveAndUpdateRightWidth, state.layout.chatCollapsed]);

  /**
   * Handle resizer mouse drag in editor mode.
   */
  const handleMouseDownEditor = useCallback((e: React.MouseEvent) => {
    e.preventDefault();
    
    if (!containerRef.current) return;

    const startX = e.clientX;
    const startWidth = currentLeftWidth;
    let lastValidWidth = startWidth;
    
    setIsDraggingEditor(true);
    document.body.style.cursor = 'col-resize';
    document.body.style.userSelect = 'none';

    const handleMouseMove = (moveEvent: MouseEvent) => {
      if (animationFrameRef.current !== null) {
        cancelAnimationFrame(animationFrameRef.current);
      }

      animationFrameRef.current = requestAnimationFrame(() => {
        const deltaX = moveEvent.clientX - startX;
        const newWidth = startWidth + deltaX;
        const validWidth = calculateValidLeftWidth(newWidth);
        
        lastValidWidth = validWidth;
        
        if (leftPanelElementRef.current) {
          leftPanelElementRef.current.style.width = `${validWidth}px`;
        } else {
          updateLeftPanelWidth(validWidth);
        }
        
        animationFrameRef.current = null;
      });
    };

    const handleMouseUp = () => {
      if (animationFrameRef.current !== null) {
        cancelAnimationFrame(animationFrameRef.current);
      }
      
      document.removeEventListener('mousemove', handleMouseMove);
      document.removeEventListener('mouseup', handleMouseUp);
      document.body.style.cursor = '';
      document.body.style.userSelect = '';
      
      const snappedWidth = getSnappedWidth(lastValidWidth, LEFT_PANEL_CONFIG, false);
      if (snappedWidth !== lastValidWidth) {
        saveAndUpdateLeftWidth(snappedWidth);
      } else {
        updateLeftPanelWidth(lastValidWidth);
        setLastLeftWidth(lastValidWidth);
        savePanelWidth(STORAGE_KEYS.LEFT_PANEL_LAST_WIDTH, lastValidWidth);
      }
      
      requestAnimationFrame(() => {
        requestAnimationFrame(() => setIsDraggingEditor(false));
      });
    };

    document.addEventListener('mousemove', handleMouseMove);
    document.addEventListener('mouseup', handleMouseUp);
  }, [currentLeftWidth, calculateValidLeftWidth, updateLeftPanelWidth, saveAndUpdateLeftWidth]);

  /**
   * Keyboard shortcuts.
   */
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      const isMac = navigator.platform.toUpperCase().indexOf('MAC') >= 0;
      const ctrlOrMeta = isMac ? e.metaKey : e.ctrlKey;
      
      // Ctrl/Cmd + \ toggles left panel
      if (ctrlOrMeta && e.key === PANEL_SHORTCUTS.TOGGLE_LEFT.key) {
        e.preventDefault();
        handleToggleLeftPanel();
        return;
      }
      
      // Ctrl/Cmd + ] toggles right panel
      if (ctrlOrMeta && e.key === PANEL_SHORTCUTS.TOGGLE_RIGHT.key && !e.shiftKey) {
        e.preventDefault();
        handleToggleRightPanel();
        return;
      }
      
      // Ctrl/Cmd + 0 toggles all panels
      if (ctrlOrMeta && e.key === PANEL_SHORTCUTS.TOGGLE_BOTH.key) {
        e.preventDefault();
        handleToggleLeftPanel();
        handleToggleRightPanel();
        return;
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [handleToggleLeftPanel, handleToggleRightPanel]);

  // Immediate right panel expansion (disable animation)
  const [isRightPanelExpandingImmediate, setIsRightPanelExpandingImmediate] = useState(false);
  const rightPanelElementRef = useRef<HTMLDivElement>(null);

  /**
   * Listen for immediate expansion events (no animation).
   */
  useEffect(() => {
    const handleExpandImmediate = (event: CustomEvent) => {
      if (event.detail?.noAnimation && state.layout.rightPanelCollapsed) {
        setIsRightPanelExpandingImmediate(true);
        // Remove class after expansion completes
        setTimeout(() => {
          setIsRightPanelExpandingImmediate(false);
        }, 0);
      }
    };

    window.addEventListener('expand-right-panel-immediate', handleExpandImmediate as EventListener);

    return () => {
      window.removeEventListener('expand-right-panel-immediate', handleExpandImmediate as EventListener);
    };
  }, [state.layout.rightPanelCollapsed]);

  /**
   * Responsive window resizing.
   */
  useEffect(() => {
    const handleWindowResize = () => {
      const validLeftWidth = calculateValidLeftWidth(currentLeftWidth);
      if (validLeftWidth !== currentLeftWidth) {
        updateLeftPanelWidth(validLeftWidth);
      }
      
      const validRightWidth = calculateValidRightWidth(currentRightWidth);
      if (validRightWidth !== currentRightWidth) {
        updateRightPanelWidth(validRightWidth);
      }
    };

    window.addEventListener('resize', handleWindowResize);
    return () => window.removeEventListener('resize', handleWindowResize);
  }, [currentLeftWidth, currentRightWidth, calculateValidLeftWidth, calculateValidRightWidth, updateLeftPanelWidth, updateRightPanelWidth]);

  /**
   * Cleanup animation frames.
   */
  useEffect(() => {
    return () => {
      if (animationFrameRef.current !== null) {
        cancelAnimationFrame(animationFrameRef.current);
      }
    };
  }, []);

  // Build container class name
  const containerClassName = [
    'bitfun-workspace-layout',
    'bitfun-workspace-layout--floating',
    className,
    (isDraggingLeft || isDraggingRight || isDraggingEditor) && 'bitfun-workspace-layout--dragging',
    isEntering && 'layout-entering'
  ].filter(Boolean).join(' ');

  // Panel state
  const isRightAsMain = state.layout.chatCollapsed;
  const isCenterHidden = state.layout.centerPanelCollapsed || isRightAsMain;
  const allCollapsed = state.layout.leftPanelCollapsed && isCenterHidden && state.layout.rightPanelCollapsed;

  // Panel mode data attributes
  const leftPanelDataMode = leftPanelMode;
  const rightPanelDataMode = rightPanelMode;

  const panelModeLabels = useMemo(() => ({
    collapsed: t('layout.panelMode.collapsed'),
    compact: t('layout.panelMode.compact'),
    comfortable: t('layout.panelMode.comfortable'),
    expanded: t('layout.panelMode.expanded')
  }), [t]);

  const panelCollapseHintStyles = useMemo(() => {
    const toCssContentValue = (value: string) => `"${value.replace(/"/g, '\\"')}"`;
    return {
      ['--panel-collapse-hint-left' as any]: toCssContentValue(t('layout.panelCollapseHintLeft')),
      ['--panel-collapse-hint-right' as any]: toCssContentValue(t('layout.panelCollapseHintRight'))
    } as React.CSSProperties;
  }, [t]);

  return (
    <div 
      ref={containerRef}
      className={containerClassName}
      style={panelCollapseHintStyles}
    >
      {/* Left auxiliary panel */}
      {!state.layout.leftPanelCollapsed && (
        <div 
          ref={leftPanelElementRef}
          className={`bitfun-left-panel ${(isDraggingLeft || isDraggingEditor) ? 'bitfun-left-panel--dragging' : ''}`}
          style={{ width: `${currentLeftWidth}px` }}
          data-mode={leftPanelDataMode}
        >
          <LeftPanel
            activeTab={state.layout.leftPanelActiveTab}
            width={0}
            isFullscreen={false}
            isDragging={false}
            onSwitchTab={switchLeftPanelTab}
            workspacePath={currentWorkspace?.rootPath}
          />
        </div>
      )}

      {/* Left resizer */}
      {!state.layout.leftPanelCollapsed && !state.layout.centerPanelCollapsed && !isRightAsMain && (
        <div 
          ref={leftResizerRef}
          className={`bitfun-panel-resizer ${isDraggingLeft ? 'bitfun-panel-resizer--dragging' : ''} ${isHoveringLeft ? 'bitfun-panel-resizer--hovering' : ''}`}
          onMouseDown={handleMouseDownLeft}
          onDoubleClick={handleDoubleClickLeft}
          onMouseEnter={() => setIsHoveringLeft(true)}
          onMouseLeave={() => setIsHoveringLeft(false)}
          tabIndex={0}
          role="separator"
          aria-orientation="vertical"
          aria-label={t('layout.resizer.leftAriaLabel')}
          aria-valuenow={currentLeftWidth}
          aria-valuemin={LEFT_PANEL_CONFIG.COMPACT_WIDTH}
          aria-valuemax={LEFT_PANEL_CONFIG.MAX_WIDTH}
          title={t('layout.resizer.title', { mode: panelModeLabels[leftPanelMode] })}
        >
          <div className="bitfun-panel-resizer__line" />
          <div className="bitfun-panel-resizer__handle">
            <svg 
              width="16" 
              height="16" 
              viewBox="0 0 16 16" 
              fill="none"
              className="bitfun-panel-resizer__icon"
            >
              <circle cx="6" cy="4" r="1" fill="currentColor" />
              <circle cx="6" cy="8" r="1" fill="currentColor" />
              <circle cx="6" cy="12" r="1" fill="currentColor" />
              <circle cx="10" cy="4" r="1" fill="currentColor" />
              <circle cx="10" cy="8" r="1" fill="currentColor" />
              <circle cx="10" cy="12" r="1" fill="currentColor" />
            </svg>
          </div>
        </div>
      )}

      {/* Draggable resizer in editor mode */}
      {isRightAsMain && !state.layout.leftPanelCollapsed && !state.layout.rightPanelCollapsed && (
        <div 
          ref={editorResizerRef}
          className={`bitfun-panel-resizer ${isDraggingEditor ? 'bitfun-panel-resizer--dragging' : ''} ${isHoveringEditor ? 'bitfun-panel-resizer--hovering' : ''}`}
          onMouseDown={handleMouseDownEditor}
          onDoubleClick={handleDoubleClickEditor}
          onMouseEnter={() => setIsHoveringEditor(true)}
          onMouseLeave={() => setIsHoveringEditor(false)}
          tabIndex={0}
          role="separator"
          aria-orientation="vertical"
          aria-label={t('layout.resizer.centerAriaLabel')}
          title={t('layout.resizer.title', { mode: panelModeLabels[leftPanelMode] })}
        >
          <div className="bitfun-panel-resizer__line" />
          <div className="bitfun-panel-resizer__handle">
            <svg 
              width="16" 
              height="16" 
              viewBox="0 0 16 16" 
              fill="none"
              className="bitfun-panel-resizer__icon"
            >
              <circle cx="6" cy="4" r="1" fill="currentColor" />
              <circle cx="6" cy="8" r="1" fill="currentColor" />
              <circle cx="6" cy="12" r="1" fill="currentColor" />
              <circle cx="10" cy="4" r="1" fill="currentColor" />
              <circle cx="10" cy="8" r="1" fill="currentColor" />
              <circle cx="10" cy="12" r="1" fill="currentColor" />
            </svg>
          </div>
        </div>
      )}

      {/* Center FlowChat panel */}
      {!state.layout.centerPanelCollapsed && !isRightAsMain && (
        <div 
          className={`bitfun-center-panel ${(isDraggingLeft || isDraggingRight) ? 'bitfun-center-panel--dragging' : ''}`}
        >
          <CenterPanel
            width={0}
            isFullscreen={false}
            isDragging={false}
            workspacePath={currentWorkspace?.rootPath}
          />
        </div>
      )}

      {/* Right resizer */}
      {!state.layout.centerPanelCollapsed && !state.layout.rightPanelCollapsed && !isRightAsMain && (
        <div 
          ref={rightResizerRef}
          className={`bitfun-panel-resizer ${isDraggingRight ? 'bitfun-panel-resizer--dragging' : ''} ${isHoveringRight ? 'bitfun-panel-resizer--hovering' : ''}`}
          onMouseDown={handleMouseDownRight}
          onDoubleClick={handleDoubleClickRight}
          onMouseEnter={() => setIsHoveringRight(true)}
          onMouseLeave={() => setIsHoveringRight(false)}
          tabIndex={0}
          role="separator"
          aria-orientation="vertical"
          aria-label={t('layout.resizer.rightAriaLabel')}
          aria-valuenow={currentRightWidth}
          aria-valuemin={RIGHT_PANEL_CONFIG.COMPACT_WIDTH}
          aria-valuemax={RIGHT_PANEL_CONFIG.MAX_WIDTH}
          title={t('layout.resizer.title', { mode: panelModeLabels[rightPanelMode] })}
        >
          <div className="bitfun-panel-resizer__line" />
          <div className="bitfun-panel-resizer__handle">
            <svg 
              width="16" 
              height="16" 
              viewBox="0 0 16 16" 
              fill="none"
              className="bitfun-panel-resizer__icon"
            >
              <circle cx="6" cy="4" r="1" fill="currentColor" />
              <circle cx="6" cy="8" r="1" fill="currentColor" />
              <circle cx="6" cy="12" r="1" fill="currentColor" />
              <circle cx="10" cy="4" r="1" fill="currentColor" />
              <circle cx="10" cy="8" r="1" fill="currentColor" />
              <circle cx="10" cy="12" r="1" fill="currentColor" />
            </svg>
          </div>
        </div>
      )}

      {/* Right panel */}
      <div 
        ref={rightPanelElementRef}
        className={`bitfun-right-panel ${state.layout.rightPanelCollapsed ? 'bitfun-right-panel--collapsed' : ''} ${isDraggingRight ? 'bitfun-right-panel--dragging' : ''} ${isRightAsMain ? 'bitfun-right-panel--editor-mode' : ''} ${isRightPanelExpandingImmediate ? 'bitfun-right-panel--no-animation' : ''}`}
        style={{
          display: state.layout.rightPanelCollapsed ? 'none' : 'flex',
          // In editor mode, let the right panel fill remaining space.
          width: state.layout.rightPanelCollapsed ? undefined : (isRightAsMain ? undefined : `${currentRightWidth}px`)
        }}
        data-mode={rightPanelDataMode}
      >
        <RightPanel
          ref={rightPanelRef}
          workspacePath={currentWorkspace?.rootPath}
        />
      </div>

      {/* Placeholder when all panels are collapsed */}
      {allCollapsed && (
        <div className="bitfun-no-panels-placeholder">
          <div className="bitfun-no-panels-placeholder__empty-state">
            <div className="bitfun-no-panels-placeholder__empty-icon">
              <svg width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5">
                <rect x="3" y="3" width="18" height="18" rx="2"/>
                <path d="M9 9h6v6H9z"/>
              </svg>
            </div>
            <h3 className="bitfun-no-panels-placeholder__title">{t('layout.noPanels')}</h3>
            <p className="bitfun-no-panels-placeholder__description">
              <Trans
                i18nKey="layout.noPanelsHint"
                t={t}
                components={{ kbd: <kbd /> }}
              />
            </p>
          </div>
        </div>
      )}
    </div>
  );
};

export default WorkspaceLayout;
