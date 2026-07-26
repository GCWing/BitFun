/**
 * Floating mini chat — circular button in bottom-right that expands to a panel
 * hosting the main window session surface. Used in non-agent scenes only; the
 * agent scene already shows that surface as its own scene.
 *
 * The panel renders ChatPane verbatim — same conversation view, same full
 * composer as the session scene — so it never lags behind the main chat UI and
 * carries no second conversation/composer implementation of its own. Only the
 * bubble chrome (trigger, open/close animation, session header) lives here.
 */

import React, { useState, useCallback, useMemo, useEffect, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import { MessageSquare, X } from 'lucide-react';
import { flowChatStore } from '../../flow_chat/store/FlowChatStore';
import { syncSessionToModernStore } from '../../flow_chat/services/storeSync';
import ChatPane from '@/app/scenes/session/ChatPane';
import { Tooltip } from '@/component-library';
import { useCurrentWorkspace } from '@/infrastructure/contexts/WorkspaceContext';
import { SessionMenu, useFlowChatSessions } from '../../flow_chat/components/session-menu';
import { useSceneStore } from '@/app/stores/sceneStore';
import { useMiniAppStore } from '@/app/scenes/miniapps/miniAppStore';
import './FloatingMiniChat.scss';

/**
 * Panel lifecycle. `opening` covers the scale-up transition, during which the
 * panel's hit area is still smaller than its final rect — see the backdrop
 * below for why that distinction matters.
 */
type PanelPhase = 'closed' | 'opening' | 'open';

/** Fallback for a transitionend that never arrives (reduced motion, interrupted
 *  transition). Must stay >= $fmc-open-duration in FloatingMiniChat.scss. */
const PANEL_OPEN_SETTLE_MS = 600;

export const FloatingMiniChat: React.FC = () => {
  const { t } = useTranslation('flow-chat');
  const activeTabId = useSceneStore((state) => state.activeTabId);
  const customizingAppIds = useMiniAppStore((state) => state.customizingAppIds);
  const { workspacePath } = useCurrentWorkspace();

  const [phase, setPhase] = useState<PanelPhase>('closed');
  const [surfaceMounted, setSurfaceMounted] = useState(false);
  const isOpen = phase !== 'closed';
  const panelRef = useRef<HTMLDivElement>(null);

  const { activeSession, sessionTitle } = useFlowChatSessions();

  const isStreaming = useMemo(() => {
    if (!activeSession || !activeSession.dialogTurns || activeSession.dialogTurns.length === 0) {
      return false;
    }
    const lastTurn = activeSession.dialogTurns[activeSession.dialogTurns.length - 1];
    return (
      lastTurn.status === 'processing' ||
      lastTurn.status === 'finishing' ||
      lastTurn.status === 'image_analyzing'
    );
  }, [activeSession]);

  const activeMiniAppId = useMemo(
    () => (typeof activeTabId === 'string' && activeTabId.startsWith('miniapp:')
      ? activeTabId.slice('miniapp:'.length)
      : null),
    [activeTabId]
  );
  const shouldAvoidMiniAppCustomizer = Boolean(
    activeMiniAppId && customizingAppIds.includes(activeMiniAppId)
  );

  // Idempotent so it is safe to fire from several input paths at once, and so a
  // repeat press during the open animation can never toggle the panel back.
  //
  // Keeping this commit cheap matters too: flipping the panel open is all it
  // does, so the open transition is committed on the very next frame. Store
  // sync and the session surface mount are deferred below — doing them here
  // blocks the main thread before the browser ever starts the transition.
  const handleOpen = useCallback(() => {
    setPhase((prev) => (prev === 'closed' ? 'opening' : prev));
  }, []);

  const handleClose = useCallback(() => {
    setPhase('closed');
  }, []);

  // Open on pointer press, not on click. A click only fires when pointerdown
  // and pointerup land on the same element, so anything that moves or replaces
  // the trigger between the two (a re-render from a streaming session, the
  // panel animation, a stray pointer move on a trackpad) silently swallows the
  // press — which is what made the button need several tries. onClick is kept
  // for keyboard and assistive activation, where no pointer events fire.
  const handleTriggerPointerDown = useCallback((e: React.PointerEvent) => {
    if (e.button !== 0) return;
    handleOpen();
  }, [handleOpen]);

  // Mount the session surface only once the open transition has been handed to
  // the compositor, so ChatPane's mount cost can no longer stall the animation.
  useEffect(() => {
    if (!isOpen) {
      setSurfaceMounted(false);
      return;
    }

    let innerFrame = 0;
    const outerFrame = requestAnimationFrame(() => {
      innerFrame = requestAnimationFrame(() => {
        // Sync the active session into modernFlowChatStore so the panel shows
        // up-to-date content (it may have streamed while the panel was closed).
        const { activeSessionId } = flowChatStore.getState();
        if (activeSessionId) {
          syncSessionToModernStore(activeSessionId);
        }
        setSurfaceMounted(true);
      });
    });

    return () => {
      cancelAnimationFrame(outerFrame);
      cancelAnimationFrame(innerFrame);
    };
  }, [isOpen]);

  // Settle `opening` → `open` once the panel reaches full size.
  useEffect(() => {
    if (phase !== 'opening') return;
    const timer = window.setTimeout(() => setPhase('open'), PANEL_OPEN_SETTLE_MS);
    return () => window.clearTimeout(timer);
  }, [phase]);

  const handlePanelTransitionEnd = useCallback((e: React.TransitionEvent) => {
    if (e.target !== panelRef.current || e.propertyName !== 'transform') return;
    setPhase((prev) => (prev === 'opening' ? 'open' : prev));
  }, []);

  const handlePanelKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (e.key !== 'Escape') return;
      // Inside the reused session surface Escape belongs to the chat scope
      // (dismiss slash/mention popups, stop generation) exactly as it does in
      // the main window, so only bubble chrome handles it here. SessionMenu
      // stops propagation while its dropdown is open.
      const target = e.target as HTMLElement | null;
      if (target?.closest?.('[data-shortcut-scope="chat"]')) return;
      e.preventDefault();
      handleClose();
    },
    [handleClose]
  );

  const panelClassName = [
    'bitfun-fmc__panel',
    isOpen && 'bitfun-fmc__panel--open',
    isStreaming && 'bitfun-fmc__panel--processing'
  ]
    .filter(Boolean)
    .join(' ');

  return (
    <div className={[
      'bitfun-fmc',
      isOpen && 'bitfun-fmc--open',
      shouldAvoidMiniAppCustomizer && 'bitfun-fmc--miniapp-customizing',
    ].filter(Boolean).join(' ')}>
      {/* Fullscreen backdrop to catch outside clicks. It stays inert until the
          panel has finished scaling up: until then the panel's hit area is
          still smaller than its final rect, so a click aimed at the panel would
          land here and close what the user just opened. Rendering it (without
          the close handler) throughout keeps those clicks from reaching the
          scene underneath. */}
      {isOpen && (
        <div
          className="bitfun-fmc__backdrop"
          onMouseDown={phase === 'open' ? handleClose : undefined}
        />
      )}

      {/* Circular trigger — sits above the backdrop and the collapsed panel via
          an explicit z-index, and is taken out of hit testing with
          `visibility` (not just pointer-events) while the panel is open. */}
      <button
        type="button"
        className="bitfun-fmc__button"
        onPointerDown={handleTriggerPointerDown}
        onClick={handleOpen}
        aria-expanded={isOpen}
        aria-label={t('toolCards.toolbar.startNewChat')}
      >
        <MessageSquare size={20} />
      </button>

      {/* Expanded panel */}
      <div
        ref={panelRef}
        className={panelClassName}
        onKeyDown={handlePanelKeyDown}
        onTransitionEnd={handlePanelTransitionEnd}
      >
        {/* Header — same shape as floating window mode: the "+" opens the
            shared session menu (new code / new cowork / switch), and the title
            is a plain display rather than a second, differently-behaved
            switcher. */}
        <div className="bitfun-fmc__header">
          <SessionMenu />

          <div className="bitfun-fmc__title-wrapper">
            <div className="bitfun-fmc__title-display" title={sessionTitle}>
              <span className="bitfun-fmc__title-text">{sessionTitle}</span>
            </div>
          </div>

          {/* Tool confirmation and stop controls come from the reused session
              surface (permission panel above ChatInput, ChatInput stop button),
              so the header only owns bubble chrome. */}
          <Tooltip content={t('planner.cancel')}>
            <button type="button" className="bitfun-fmc__header-btn bitfun-fmc__header-btn--close" onClick={handleClose}>
              <X size={14} />
            </button>
          </Tooltip>
        </div>

        {/* Main window session surface, reused as-is. Only mounted while the
            panel is open to avoid running a second VirtualMessageList and store
            sync in the background while the agent streams in another scene. */}
        <div className="bitfun-fmc__body">
          {surfaceMounted && (
            <ChatPane
              width={0}
              isFullscreen={false}
              isSceneActive
              workspacePath={workspacePath}
              showChatInput
            />
          )}
        </div>
      </div>
    </div>
  );
};

export default FloatingMiniChat;
