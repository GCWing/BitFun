/**
 * SessionScene — Session scene layout.
 *
 * Layout (left to right):
 *   ChatPane (flex:1, FlowChat conversation)
 *   PaneResizer (draggable divider)
 *   AuxPane (variable width, ContentCanvas tabs)
 *
 * Resizer logic moved here from WorkspaceShell.
 */

import React, { useRef, useState, useCallback, useEffect, useLayoutEffect, useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { useApp } from '../../hooks/useApp';
import ChatPane from './ChatPane';
import AuxPane, { type AuxPaneRef } from './AuxPane';
import BottomTerminalPane from './BottomTerminalPane';
import {
  getCachedTerminalPanelPosition,
  onTerminalPanelPositionChange,
  refreshTerminalPanelPosition,
} from '@/tools/terminal/services/terminalPanelPreferenceService';
import type { TerminalPanelPosition } from '@/infrastructure/config/types';

import {
  BOTTOM_TERMINAL_PANEL_CONFIG,
  RIGHT_PANEL_CONFIG,
  PANEL_COMMON_CONFIG,
  STORAGE_KEYS,
  PanelDisplayMode,
  getPanelDisplayMode,
  getModeWidth,
  getSnappedWidth,
  getNextMode,
  savePanelWidth,
  loadPanelWidth,
} from '../../layout/panelConfig';

import './SessionScene.scss';

const TERMINAL_PANEL_RESIZE_SUSPEND_FALLBACK_MS = 360;

interface SessionSceneProps {
  workspacePath?: string;
  isEntering?: boolean;
  isActive?: boolean;
}

const SessionScene: React.FC<SessionSceneProps> = ({
  workspacePath,
  isEntering = false,
  isActive = true,
}) => {
  const { t } = useTranslation('flow-chat');
  const {
    state,
    updateRightPanelWidth,
    toggleRightPanel,
    updateBottomTerminalPanelHeight,
    toggleBottomTerminalPanel,
  } = useApp();
  const auxPaneRef = useRef<AuxPaneRef>(null);

  const [isDraggingRight, setIsDraggingRight] = useState(false);
  const [isDraggingBottom, setIsDraggingBottom] = useState(false);
  const [isHovering, setIsHovering] = useState(false);
  const [isHoveringBottom, setIsHoveringBottom] = useState(false);
  const [isRightPanelTransitioning, setIsRightPanelTransitioning] = useState(false);
  const [isBottomTerminalPanelTransitioning, setIsBottomTerminalPanelTransitioning] = useState(false);
  const [terminalPanelPosition, setTerminalPanelPosition] = useState<TerminalPanelPosition>(() =>
    getCachedTerminalPanelPosition(),
  );

  const [, setLastRightWidth] = useState<number>(() =>
    loadPanelWidth(STORAGE_KEYS.RIGHT_PANEL_LAST_WIDTH, RIGHT_PANEL_CONFIG.COMFORTABLE_DEFAULT)
  );

  const containerRef = useRef<HTMLDivElement>(null);
  const resizerRef = useRef<HTMLDivElement>(null);
  const auxPaneElementRef = useRef<HTMLDivElement>(null);
  const bottomTerminalPaneElementRef = useRef<HTMLDivElement>(null);
  const animationFrameRef = useRef<number | null>(null);
  const bottomExpandPendingRef = useRef(false);
  const bottomCollapsePendingRef = useRef(false);
  const rightPanelTransitionTimerRef = useRef<number | null>(null);
  const bottomPanelTransitionTimerRef = useRef<number | null>(null);
  const previousRightTransitionKeyRef = useRef<string | null>(null);
  const previousBottomTransitionKeyRef = useRef<string | null>(null);

  const currentRightWidth = state.layout.rightPanelWidth || RIGHT_PANEL_CONFIG.COMFORTABLE_DEFAULT;
  const currentBottomHeight = state.layout.bottomTerminalPanelHeight || BOTTOM_TERMINAL_PANEL_CONFIG.COMFORTABLE_DEFAULT;
  const isTerminalDockedBottom = terminalPanelPosition === 'bottom';
  const isDragging = isDraggingRight || isDraggingBottom;

  const stopRightPanelTransition = useCallback(() => {
    if (rightPanelTransitionTimerRef.current !== null) {
      window.clearTimeout(rightPanelTransitionTimerRef.current);
      rightPanelTransitionTimerRef.current = null;
    }
    setIsRightPanelTransitioning(false);
  }, []);

  const startRightPanelTransition = useCallback(() => {
    if (rightPanelTransitionTimerRef.current !== null) {
      window.clearTimeout(rightPanelTransitionTimerRef.current);
    }
    setIsRightPanelTransitioning(true);
    rightPanelTransitionTimerRef.current = window.setTimeout(
      stopRightPanelTransition,
      TERMINAL_PANEL_RESIZE_SUSPEND_FALLBACK_MS,
    );
  }, [stopRightPanelTransition]);

  const stopBottomPanelTransition = useCallback(() => {
    if (bottomPanelTransitionTimerRef.current !== null) {
      window.clearTimeout(bottomPanelTransitionTimerRef.current);
      bottomPanelTransitionTimerRef.current = null;
    }
    setIsBottomTerminalPanelTransitioning(false);
  }, []);

  const startBottomPanelTransition = useCallback(() => {
    if (bottomPanelTransitionTimerRef.current !== null) {
      window.clearTimeout(bottomPanelTransitionTimerRef.current);
    }
    setIsBottomTerminalPanelTransitioning(true);
    bottomPanelTransitionTimerRef.current = window.setTimeout(
      stopBottomPanelTransition,
      TERMINAL_PANEL_RESIZE_SUSPEND_FALLBACK_MS,
    );
  }, [stopBottomPanelTransition]);

  const rightPanelMode: PanelDisplayMode = useMemo(() => {
    if (state.layout.rightPanelCollapsed) return 'collapsed';
    return getPanelDisplayMode(currentRightWidth, RIGHT_PANEL_CONFIG);
  }, [state.layout.rightPanelCollapsed, currentRightWidth]);

  const bottomTerminalPanelMode: PanelDisplayMode = useMemo(() => {
    if (state.layout.bottomTerminalPanelCollapsed) return 'collapsed';
    return getPanelDisplayMode(currentBottomHeight, BOTTOM_TERMINAL_PANEL_CONFIG);
  }, [state.layout.bottomTerminalPanelCollapsed, currentBottomHeight]);

  useEffect(() => {
    const unsubscribe = onTerminalPanelPositionChange(setTerminalPanelPosition);
    void refreshTerminalPanelPosition();
    return unsubscribe;
  }, []);

  // Keep right panel visible when chat is hidden
  useEffect(() => {
    if (state.layout.chatCollapsed && state.layout.rightPanelCollapsed) {
      toggleRightPanel();
    }
  }, [state.layout.chatCollapsed, state.layout.rightPanelCollapsed, toggleRightPanel]);

  const calculateValidRightWidth = useCallback((newWidth: number): number => {
    if (!containerRef.current) return newWidth;
    const containerWidth = containerRef.current.offsetWidth;
    // When the container hasn't been laid out yet (e.g. window just restored from
    // minimize), offsetWidth may be 0. Bail early to avoid clamping to a tiny value.
    if (containerWidth <= 0) return newWidth;
    // NavPanel (240px) is outside SessionScene — only account for resizer + min chat width
    const reserved = PANEL_COMMON_CONFIG.RESIZER_WIDTH + PANEL_COMMON_CONFIG.MIN_CENTER_WIDTH;
    const dynamicMax = containerWidth - reserved;
    const maxWidth = Math.min(RIGHT_PANEL_CONFIG.MAX_WIDTH, dynamicMax);
    return Math.min(maxWidth, Math.max(RIGHT_PANEL_CONFIG.COMPACT_WIDTH, newWidth));
  }, []);

  const calculateValidBottomHeight = useCallback((newHeight: number): number => {
    if (!containerRef.current) return newHeight;
    const containerHeight = containerRef.current.offsetHeight;
    if (containerHeight <= 0) return newHeight;
    const reserved = PANEL_COMMON_CONFIG.RESIZER_WIDTH + 220;
    const dynamicMax = containerHeight - reserved;
    const maxHeight = Math.min(BOTTOM_TERMINAL_PANEL_CONFIG.MAX_WIDTH, dynamicMax);
    return Math.min(maxHeight, Math.max(BOTTOM_TERMINAL_PANEL_CONFIG.COMPACT_WIDTH, newHeight));
  }, []);

  const saveAndUpdateRightWidth = useCallback((width: number) => {
    updateRightPanelWidth(width);
    setLastRightWidth(width);
    savePanelWidth(STORAGE_KEYS.RIGHT_PANEL_LAST_WIDTH, width);
  }, [updateRightPanelWidth]);

  const saveAndUpdateBottomHeight = useCallback((height: number) => {
    updateBottomTerminalPanelHeight(height);
    savePanelWidth(STORAGE_KEYS.BOTTOM_TERMINAL_PANEL_LAST_HEIGHT, height);
  }, [updateBottomTerminalPanelHeight]);

  const handleDoubleClick = useCallback(() => {
    const nextMode = getNextMode(rightPanelMode);
    const targetWidth = getModeWidth(nextMode, RIGHT_PANEL_CONFIG);
    saveAndUpdateRightWidth(calculateValidRightWidth(targetWidth));
  }, [rightPanelMode, calculateValidRightWidth, saveAndUpdateRightWidth]);

  const handleBottomDoubleClick = useCallback(() => {
    const nextMode = getNextMode(bottomTerminalPanelMode);
    const targetHeight = getModeWidth(nextMode, BOTTOM_TERMINAL_PANEL_CONFIG);
    saveAndUpdateBottomHeight(calculateValidBottomHeight(targetHeight));
  }, [bottomTerminalPanelMode, calculateValidBottomHeight, saveAndUpdateBottomHeight]);

  const handleMouseDownResizer = useCallback((e: React.MouseEvent) => {
    e.preventDefault();
    if (!containerRef.current) return;

    const startX = e.clientX;
    const startWidth = currentRightWidth;
    let lastValidWidth = startWidth;

    setIsDraggingRight(true);
    document.body.style.cursor = 'col-resize';
    document.body.style.userSelect = 'none';

    const onMove = (ev: MouseEvent) => {
      if (animationFrameRef.current !== null) cancelAnimationFrame(animationFrameRef.current);
      animationFrameRef.current = requestAnimationFrame(() => {
        const valid = calculateValidRightWidth(startWidth + (startX - ev.clientX));
        lastValidWidth = valid;
        if (auxPaneElementRef.current && !state.layout.chatCollapsed) {
          auxPaneElementRef.current.style.width = `${valid}px`;
        } else {
          updateRightPanelWidth(valid);
        }
        animationFrameRef.current = null;
      });
    };

    const onUp = () => {
      if (animationFrameRef.current !== null) cancelAnimationFrame(animationFrameRef.current);
      document.removeEventListener('mousemove', onMove);
      document.removeEventListener('mouseup', onUp);
      document.body.style.cursor = '';
      document.body.style.userSelect = '';

      const snapped = getSnappedWidth(lastValidWidth, RIGHT_PANEL_CONFIG, false);
      if (snapped !== lastValidWidth) {
        saveAndUpdateRightWidth(snapped);
      } else {
        updateRightPanelWidth(lastValidWidth);
        setLastRightWidth(lastValidWidth);
        savePanelWidth(STORAGE_KEYS.RIGHT_PANEL_LAST_WIDTH, lastValidWidth);
      }
      requestAnimationFrame(() => requestAnimationFrame(() => setIsDraggingRight(false)));
    };

    document.addEventListener('mousemove', onMove);
    document.addEventListener('mouseup', onUp);
  }, [currentRightWidth, calculateValidRightWidth, updateRightPanelWidth, saveAndUpdateRightWidth, state.layout.chatCollapsed]);

  const handleMouseDownBottomResizer = useCallback((e: React.MouseEvent) => {
    e.preventDefault();
    if (!containerRef.current) return;

    const startY = e.clientY;
    const startHeight = currentBottomHeight;
    let lastValidHeight = startHeight;

    setIsDraggingBottom(true);
    document.body.style.cursor = 'row-resize';
    document.body.style.userSelect = 'none';

    const onMove = (ev: MouseEvent) => {
      if (animationFrameRef.current !== null) cancelAnimationFrame(animationFrameRef.current);
      animationFrameRef.current = requestAnimationFrame(() => {
        const valid = calculateValidBottomHeight(startHeight + (startY - ev.clientY));
        lastValidHeight = valid;
        if (bottomTerminalPaneElementRef.current) {
          bottomTerminalPaneElementRef.current.style.height = `${valid}px`;
        } else {
          updateBottomTerminalPanelHeight(valid);
        }
        animationFrameRef.current = null;
      });
    };

    const onUp = () => {
      if (animationFrameRef.current !== null) cancelAnimationFrame(animationFrameRef.current);
      document.removeEventListener('mousemove', onMove);
      document.removeEventListener('mouseup', onUp);
      document.body.style.cursor = '';
      document.body.style.userSelect = '';

      const snapped = getSnappedWidth(lastValidHeight, BOTTOM_TERMINAL_PANEL_CONFIG, false);
      if (snapped !== lastValidHeight) {
        saveAndUpdateBottomHeight(snapped);
      } else {
        updateBottomTerminalPanelHeight(lastValidHeight);
        savePanelWidth(STORAGE_KEYS.BOTTOM_TERMINAL_PANEL_LAST_HEIGHT, lastValidHeight);
      }
      requestAnimationFrame(() => requestAnimationFrame(() => setIsDraggingBottom(false)));
    };

    document.addEventListener('mousemove', onMove);
    document.addEventListener('mouseup', onUp);
  }, [calculateValidBottomHeight, currentBottomHeight, saveAndUpdateBottomHeight, updateBottomTerminalPanelHeight]);

  // No-animation expansion
  const [isAuxPaneExpandingImmediate, setIsAuxPaneExpandingImmediate] = useState(false);

  useEffect(() => {
    const handler = (event: CustomEvent) => {
      if (event.detail?.noAnimation && state.layout.rightPanelCollapsed) {
        setIsAuxPaneExpandingImmediate(true);
        setTimeout(() => setIsAuxPaneExpandingImmediate(false), 0);
      }
    };
    window.addEventListener('expand-right-panel-immediate', handler as EventListener);
    return () => window.removeEventListener('expand-right-panel-immediate', handler as EventListener);
  }, [state.layout.rightPanelCollapsed]);

  const expandBottomTerminalPanel = useCallback(() => {
    const saved = loadPanelWidth(
      STORAGE_KEYS.BOTTOM_TERMINAL_PANEL_LAST_HEIGHT,
      BOTTOM_TERMINAL_PANEL_CONFIG.COMFORTABLE_DEFAULT,
    );
    updateBottomTerminalPanelHeight(calculateValidBottomHeight(saved));

    if (state.layout.bottomTerminalPanelCollapsed && !bottomExpandPendingRef.current) {
      bottomExpandPendingRef.current = true;
      requestAnimationFrame(() => {
        toggleBottomTerminalPanel();
        bottomExpandPendingRef.current = false;
      });
    }
  }, [
    calculateValidBottomHeight,
    state.layout.bottomTerminalPanelCollapsed,
    toggleBottomTerminalPanel,
    updateBottomTerminalPanelHeight,
  ]);

  const collapseBottomTerminalPanel = useCallback(() => {
    if (!state.layout.bottomTerminalPanelCollapsed && !bottomCollapsePendingRef.current) {
      bottomCollapsePendingRef.current = true;
      requestAnimationFrame(() => {
        toggleBottomTerminalPanel();
        bottomCollapsePendingRef.current = false;
      });
    }
  }, [state.layout.bottomTerminalPanelCollapsed, toggleBottomTerminalPanel]);

  // Responsive resize — also validate on mount to clamp widths restored from
  // localStorage that may exceed the current (non-maximized) window size.
  useEffect(() => {
    const validate = () => {
      const valid = calculateValidRightWidth(currentRightWidth);
      if (valid !== currentRightWidth) updateRightPanelWidth(valid);
      if (isTerminalDockedBottom) {
        const validBottom = calculateValidBottomHeight(currentBottomHeight);
        if (validBottom !== currentBottomHeight) updateBottomTerminalPanelHeight(validBottom);
      }
    };
    const rafId = requestAnimationFrame(validate);
    window.addEventListener('resize', validate);
    return () => {
      cancelAnimationFrame(rafId);
      window.removeEventListener('resize', validate);
    };
  }, [
    currentRightWidth,
    currentBottomHeight,
    calculateValidRightWidth,
    calculateValidBottomHeight,
    isTerminalDockedBottom,
    updateRightPanelWidth,
    updateBottomTerminalPanelHeight,
  ]);

  // Restore right panel width when window regains visibility (e.g. after minimize → restore).
  // This acts as a safety net in case any layout recalculation during the restore
  // cycle lost the user's manual width adjustment.
  const prevVisibleRef = useRef(true);
  useEffect(() => {
    const handleVisibility = () => {
      const nowVisible = document.visibilityState === 'visible';
      if (nowVisible && !prevVisibleRef.current) {
        const saved = loadPanelWidth(STORAGE_KEYS.RIGHT_PANEL_LAST_WIDTH, currentRightWidth);
        if (saved !== currentRightWidth && !state.layout.rightPanelCollapsed) {
          updateRightPanelWidth(saved);
        }
      }
      prevVisibleRef.current = nowVisible;
    };
    document.addEventListener('visibilitychange', handleVisibility);
    return () => document.removeEventListener('visibilitychange', handleVisibility);
  }, [currentRightWidth, updateRightPanelWidth, state.layout.rightPanelCollapsed]);

  // Cleanup animation frames
  useEffect(() => () => {
    if (animationFrameRef.current !== null) cancelAnimationFrame(animationFrameRef.current);
    if (rightPanelTransitionTimerRef.current !== null) {
      window.clearTimeout(rightPanelTransitionTimerRef.current);
    }
    if (bottomPanelTransitionTimerRef.current !== null) {
      window.clearTimeout(bottomPanelTransitionTimerRef.current);
    }
  }, []);

  const isRightAsMain = state.layout.chatCollapsed;
  const isChatHidden = state.layout.centerPanelCollapsed || isRightAsMain;

  // Terminal resize is suspended for the whole CSS transition. xterm.js reflows
  // its buffer on every resize, so fitting through intermediate panel widths can
  // permanently damage scrollback before the panel reaches its final size.
  useLayoutEffect(() => {
    const transitionKey = [
      state.layout.rightPanelCollapsed ? 'collapsed' : 'open',
      currentRightWidth,
      isRightAsMain ? 'main' : 'side',
    ].join(':');

    if (previousRightTransitionKeyRef.current === null) {
      previousRightTransitionKeyRef.current = transitionKey;
      return;
    }

    if (previousRightTransitionKeyRef.current === transitionKey) {
      return;
    }

    previousRightTransitionKeyRef.current = transitionKey;

    if (isDraggingRight || isAuxPaneExpandingImmediate) {
      return;
    }

    startRightPanelTransition();
  }, [
    currentRightWidth,
    isAuxPaneExpandingImmediate,
    isDraggingRight,
    isRightAsMain,
    startRightPanelTransition,
    state.layout.rightPanelCollapsed,
  ]);

  // Bottom docking uses the same suspension as the right panel. This keeps the
  // compact final height usable while avoiding transient animation heights.
  useLayoutEffect(() => {
    const transitionKey = [
      state.layout.bottomTerminalPanelCollapsed ? 'collapsed' : 'open',
      currentBottomHeight,
      isTerminalDockedBottom ? 'bottom' : 'right',
    ].join(':');

    if (previousBottomTransitionKeyRef.current === null) {
      previousBottomTransitionKeyRef.current = transitionKey;
      return;
    }

    if (previousBottomTransitionKeyRef.current === transitionKey) {
      return;
    }

    previousBottomTransitionKeyRef.current = transitionKey;

    if (isDraggingBottom || !isTerminalDockedBottom) {
      return;
    }

    startBottomPanelTransition();
  }, [
    currentBottomHeight,
    isDraggingBottom,
    isTerminalDockedBottom,
    startBottomPanelTransition,
    state.layout.bottomTerminalPanelCollapsed,
  ]);

  const handleRightPanelTransitionEnd = useCallback((event: React.TransitionEvent<HTMLDivElement>) => {
    if (event.currentTarget !== event.target) return;
    if (event.propertyName !== 'width') return;
    stopRightPanelTransition();
  }, [stopRightPanelTransition]);

  const handleBottomPanelTransitionEnd = useCallback((event: React.TransitionEvent<HTMLDivElement>) => {
    if (event.currentTarget !== event.target) return;
    if (event.propertyName !== 'height') return;
    stopBottomPanelTransition();
  }, [stopBottomPanelTransition]);

  const panelModeLabels = useMemo(() => ({
    collapsed:    t('layout.panelMode.collapsed'),
    compact:      t('layout.panelMode.compact'),
    comfortable:  t('layout.panelMode.comfortable'),
    expanded:     t('layout.panelMode.expanded'),
  }), [t]);

  const panelCollapseHintStyles = useMemo(() => {
    const q = (v: string) => `"${v.replace(/"/g, '\\"')}"`;
    return {
      '--panel-collapse-hint-right': q(t('layout.panelCollapseHintRight')),
    } as React.CSSProperties & Record<'--panel-collapse-hint-right', string>;
  }, [t]);

  return (
    <div
      ref={containerRef}
      className={[
        'bitfun-session-scene',
        isDragging && 'bitfun-session-scene--dragging',
        isDraggingBottom && 'bitfun-session-scene--dragging-bottom',
        isTerminalDockedBottom && 'bitfun-session-scene--terminal-bottom',
        isEntering && 'layout-entering',
      ].filter(Boolean).join(' ')}
      style={panelCollapseHintStyles}
      data-testid="session-scene"
    >
      <div className="bitfun-session-scene__main-row">
        {/* ChatPane — FlowChat conversation */}
        {!isChatHidden && (
          <div
            className={`bitfun-session-scene__chat-pane ${isDragging ? 'bitfun-session-scene__chat-pane--dragging' : ''}`}
            data-testid="session-chat-pane"
          >
            <ChatPane
              width={0}
              isFullscreen={false}
              isSceneActive={isActive}
              isDragging={false}
              workspacePath={workspacePath}
              showChatInput
            />
          </div>
        )}

        {/* Resizer — always rendered (when chat visible) for slide animation */}
        {!isChatHidden && (
          <div
            ref={resizerRef}
            className={[
              'bitfun-pane-resizer',
              state.layout.rightPanelCollapsed && 'bitfun-pane-resizer--collapsed',
              isDraggingRight && 'bitfun-pane-resizer--dragging',
              isHovering && 'bitfun-pane-resizer--hovering',
            ].filter(Boolean).join(' ')}
            onMouseDown={handleMouseDownResizer}
            onDoubleClick={handleDoubleClick}
            onMouseEnter={() => setIsHovering(true)}
            onMouseLeave={() => setIsHovering(false)}
            tabIndex={state.layout.rightPanelCollapsed ? -1 : 0}
            role="separator"
            aria-orientation="vertical"
            aria-label={t('layout.resizer.rightAriaLabel')}
            aria-valuenow={currentRightWidth}
            aria-valuemin={RIGHT_PANEL_CONFIG.COMPACT_WIDTH}
            aria-valuemax={RIGHT_PANEL_CONFIG.MAX_WIDTH}
            title={t('layout.resizer.title', { mode: panelModeLabels[rightPanelMode] })}
            data-testid="session-right-pane-resizer"
          >
            <div className="bitfun-pane-resizer__line" />
            <div className="bitfun-pane-resizer__handle">
              <svg width="16" height="16" viewBox="0 0 16 16" fill="none" className="bitfun-pane-resizer__icon">
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

        {/* AuxPane — ContentCanvas */}
        <div
          ref={auxPaneElementRef}
          className={[
            'bitfun-session-scene__aux-pane',
            state.layout.rightPanelCollapsed         && 'bitfun-session-scene__aux-pane--collapsed',
            isDraggingRight                          && 'bitfun-session-scene__aux-pane--dragging',
            isRightAsMain                            && 'bitfun-session-scene__aux-pane--editor-mode',
            isAuxPaneExpandingImmediate              && 'bitfun-session-scene__aux-pane--no-animation',
          ].filter(Boolean).join(' ')}
          style={{
            width: state.layout.rightPanelCollapsed
              ? undefined
              : isRightAsMain ? undefined : `${currentRightWidth}px`,
          }}
          data-mode={rightPanelMode}
          data-testid="session-aux-pane"
          onTransitionEnd={handleRightPanelTransitionEnd}
        >
          <AuxPane
            ref={auxPaneRef}
            workspacePath={workspacePath}
            isSceneActive={isActive}
            terminalResizeSuspended={isRightPanelTransitioning || isDraggingRight}
          />
        </div>
      </div>

      {isTerminalDockedBottom ? (
        <>
          <div
            className={[
              'bitfun-bottom-pane-resizer',
              state.layout.bottomTerminalPanelCollapsed && 'bitfun-bottom-pane-resizer--collapsed',
              isDraggingBottom && 'bitfun-bottom-pane-resizer--dragging',
              isHoveringBottom && 'bitfun-bottom-pane-resizer--hovering',
            ].filter(Boolean).join(' ')}
            onMouseDown={handleMouseDownBottomResizer}
            onDoubleClick={handleBottomDoubleClick}
            onMouseEnter={() => setIsHoveringBottom(true)}
            onMouseLeave={() => setIsHoveringBottom(false)}
            tabIndex={state.layout.bottomTerminalPanelCollapsed ? -1 : 0}
            role="separator"
            aria-orientation="horizontal"
            aria-label={t('layout.resizer.terminalBottomAriaLabel')}
            aria-valuenow={currentBottomHeight}
            aria-valuemin={BOTTOM_TERMINAL_PANEL_CONFIG.COMPACT_WIDTH}
            aria-valuemax={BOTTOM_TERMINAL_PANEL_CONFIG.MAX_WIDTH}
            title={t('layout.resizer.title', { mode: panelModeLabels[bottomTerminalPanelMode] })}
          >
            <div className="bitfun-bottom-pane-resizer__line" />
            <div className="bitfun-bottom-pane-resizer__handle">
              <svg width="16" height="16" viewBox="0 0 16 16" fill="none" className="bitfun-bottom-pane-resizer__icon">
                <circle cx="4" cy="6" r="1" fill="currentColor" />
                <circle cx="8" cy="6" r="1" fill="currentColor" />
                <circle cx="12" cy="6" r="1" fill="currentColor" />
                <circle cx="4" cy="10" r="1" fill="currentColor" />
                <circle cx="8" cy="10" r="1" fill="currentColor" />
                <circle cx="12" cy="10" r="1" fill="currentColor" />
              </svg>
            </div>
          </div>

          <div
            ref={bottomTerminalPaneElementRef}
            className={[
              'bitfun-session-scene__bottom-terminal-pane',
              state.layout.bottomTerminalPanelCollapsed && 'bitfun-session-scene__bottom-terminal-pane--collapsed',
              isDraggingBottom && 'bitfun-session-scene__bottom-terminal-pane--dragging',
            ].filter(Boolean).join(' ')}
            style={{
              height: state.layout.bottomTerminalPanelCollapsed ? undefined : `${currentBottomHeight}px`,
            }}
            data-mode={bottomTerminalPanelMode}
            onTransitionEnd={handleBottomPanelTransitionEnd}
          >
            <BottomTerminalPane
              workspacePath={workspacePath}
              isSceneActive={isActive}
              isCollapsed={state.layout.bottomTerminalPanelCollapsed}
              onExpand={expandBottomTerminalPanel}
              onCollapse={collapseBottomTerminalPanel}
              terminalResizeSuspended={isBottomTerminalPanelTransitioning || isDraggingBottom}
            />
          </div>
        </>
      ) : null}
    </div>
  );
};

export default SessionScene;
