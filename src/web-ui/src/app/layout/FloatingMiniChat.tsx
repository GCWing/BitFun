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
import type {
  ChatInputRegistration,
  ChatInputSubmission,
} from '@/flow_chat/components/chatInputRegistration';
import { Tooltip } from '@/component-library';
import { useCurrentWorkspace } from '@/infrastructure/contexts/WorkspaceContext';
import { SessionMenu, useFlowChatSessions } from '../../flow_chat/components/session-menu';
import { useSceneStore } from '@/app/stores/sceneStore';
import {
  useMiniAppStore,
  MINIAPP_COMPOSER_MESSAGE_EVENT,
  MINIAPP_COMPOSER_DRAFT_EVENT,
  type MiniAppComposerMessageDetail,
} from '@/app/scenes/miniapps/miniAppStore';
import { pickLocalizedString } from '@/app/scenes/miniapps/utils/pickLocalizedString';
import { renderMiniAppIcon } from '@/app/scenes/miniapps/utils/miniAppIcons';
import MiniAppBubbleWelcome from './MiniAppBubbleWelcome';
import { isFloatingMiniChatSessionExecuting } from './floatingMiniChatActivity';
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
  const { t, i18n } = useTranslation('flow-chat');
  const activeTabId = useSceneStore((state) => state.activeTabId);
  const customizingAppIds = useMiniAppStore((state) => state.customizingAppIds);
  const composerClaims = useMiniAppStore((state) => state.composerClaims);
  const apps = useMiniAppStore((state) => state.apps);
  const { workspacePath } = useCurrentWorkspace();

  const [phase, setPhase] = useState<PanelPhase>('closed');
  const [surfaceMounted, setSurfaceMounted] = useState(false);
  const [composerPrefill, setComposerPrefill] = useState<{
    id: number;
    text: string;
  } | null>(null);
  const composerPrefillIdRef = useRef(0);
  const isOpen = phase !== 'closed';
  const panelRef = useRef<HTMLDivElement>(null);
  const previousHostSessionRef = useRef<{
    claimToken: string;
    sessionId: string | null;
  } | null>(null);

  const activeMiniAppId = useMemo(
    () => (typeof activeTabId === 'string' && activeTabId.startsWith('miniapp:')
      ? activeTabId.slice('miniapp:'.length)
      : null),
    [activeTabId]
  );
  const shouldAvoidMiniAppCustomizer = Boolean(
    activeMiniAppId && customizingAppIds.includes(activeMiniAppId)
  );

  // A claim registers bounded content and submission routing into the same
  // shared bubble surface. MiniApps never replace ChatPane or ChatInput.
  const activeComposerClaim = activeMiniAppId ? composerClaims[activeMiniAppId] : undefined;
  const activeMiniApp = useMemo(
    () => apps.find((app) => app.id === activeMiniAppId),
    [activeMiniAppId, apps]
  );
  const activeMiniAppName = useMemo(() => {
    if (!activeMiniAppId) return '';
    if (!activeMiniApp) return activeMiniAppId;
    return pickLocalizedString(activeMiniApp, i18n.language, 'name') || activeMiniAppId;
  }, [activeMiniApp, activeMiniAppId, i18n.language]);
  const activeMiniAppDescription = useMemo(() => {
    if (!activeMiniApp) return '';
    return pickLocalizedString(activeMiniApp, i18n.language, 'description')
      || activeMiniApp.description;
  }, [activeMiniApp, i18n.language]);
  const activeMiniAppIcon = activeMiniApp?.icon || 'Box';
  const bubbleCustomization = activeComposerClaim?.customization;
  const activeComposerToken = activeComposerClaim?.token;
  const activeComposerSessionId = activeComposerClaim?.sessionId;
  const {
    activeSession,
    trackedSession: activeMiniAppSession,
    sessionTitle,
  } = useFlowChatSessions(activeComposerSessionId);
  const isMiniAppSessionReady = Boolean(
    activeComposerSessionId && activeSession?.sessionId === activeComposerSessionId
  );
  const displayedSession = activeComposerClaim
    ? activeMiniAppSession
    : activeSession;
  const displayedTitle = activeComposerClaim
    ? (bubbleCustomization?.title || activeMiniAppName)
    : sessionTitle;

  const isStreaming = useMemo(() => {
    const lastTurn = displayedSession?.dialogTurns.at(-1);
    return (
      lastTurn?.status === 'processing'
      || lastTurn?.status === 'finishing'
      || lastTurn?.status === 'image_analyzing'
    );
  }, [displayedSession]);
  const isMiniAppSessionExecuting = Boolean(
    activeComposerClaim
    && isFloatingMiniChatSessionExecuting(activeMiniAppSession),
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

  const setMiniAppComposerDraft = useCallback((text: string) => {
    composerPrefillIdRef.current += 1;
    setComposerPrefill({
      id: composerPrefillIdRef.current,
      text,
    });
  }, []);

  const handleMiniAppSuggestion = useCallback((prompt: string) => {
    setMiniAppComposerDraft(prompt);
  }, [setMiniAppComposerDraft]);

  const handleMiniAppDraftConsumed = useCallback((id: number) => {
    setComposerPrefill((current) => (current?.id === id ? null : current));
  }, []);

  const handleMiniAppSubmit = useCallback(async (submission: ChatInputSubmission) => {
    if (!activeComposerToken) {
      throw new Error('The active MiniApp no longer owns the floating chat registration.');
    }
    const detail: MiniAppComposerMessageDetail = {
      token: activeComposerToken,
      text: submission.text,
      displayText: submission.displayText,
      contexts: submission.contexts,
      composerPresentation: submission.composerPresentation,
      sessionId: submission.sessionId,
      workspacePath: submission.workspacePath,
    };
    window.dispatchEvent(new CustomEvent(MINIAPP_COMPOSER_MESSAGE_EVENT, { detail }));
  }, [activeComposerToken]);

  const chatInputRegistration = useMemo<ChatInputRegistration | undefined>(() => {
    if (!activeComposerClaim) {
      return undefined;
    }
    return {
      registrationId: activeComposerClaim.token,
      placeholder:
        bubbleCustomization?.composer?.placeholder
        || activeComposerClaim.placeholder
        || t('miniAppComposer.placeholder', { app: activeMiniAppName }),
      // Registration presence makes this an isolated workspace even when the
      // hidden session has no path, so the normal project never leaks in.
      workspacePath: displayedSession?.workspacePath || '',
      draft: composerPrefill || undefined,
      onDraftConsumed: handleMiniAppDraftConsumed,
      onSubmit: handleMiniAppSubmit,
    };
  }, [
    activeComposerClaim,
    activeMiniAppName,
    bubbleCustomization?.composer?.placeholder,
    composerPrefill,
    displayedSession?.workspacePath,
    handleMiniAppDraftConsumed,
    handleMiniAppSubmit,
    t,
  ]);

  useEffect(() => {
    setComposerPrefill(null);
  }, [activeComposerSessionId, activeComposerToken]);

  // ChatPane is intentionally the shared session surface, so displaying a
  // MiniApp session temporarily switches the shared store. Capture the user's
  // normal session for the lifetime of the open claimed bubble and restore it
  // when the panel closes or the active MiniApp changes.
  useEffect(() => {
    if (!isOpen || !activeComposerToken) return;

    const state = flowChatStore.getState();
    const currentSession = state.activeSessionId
      ? state.sessions.get(state.activeSessionId)
      : undefined;
    previousHostSessionRef.current = {
      claimToken: activeComposerToken,
      sessionId:
        currentSession && currentSession.sessionKind !== 'miniapp'
          ? currentSession.sessionId
          : null,
    };

    return () => {
      const previous = previousHostSessionRef.current;
      if (!previous || previous.claimToken !== activeComposerToken) return;
      previousHostSessionRef.current = null;
      if (!previous.sessionId) return;
      const latest = flowChatStore.getState();
      if (!latest.sessions.has(previous.sessionId)) return;
      if (latest.activeSessionId !== previous.sessionId) {
        flowChatStore.switchSession(previous.sessionId);
      }
      syncSessionToModernStore(previous.sessionId);
    };
  }, [activeComposerToken, isOpen]);

  // The claim carries the topic's dedicated session. Activate it only while
  // the bubble is open; if the Agent session has not reached FlowChat yet,
  // wait for that exact id instead of falling back to the latest normal chat.
  useEffect(() => {
    if (!isOpen || !activeComposerToken || !activeComposerSessionId) return;

    const activateDedicatedSession = () => {
      const state = flowChatStore.getState();
      if (!state.sessions.has(activeComposerSessionId)) return false;
      if (state.activeSessionId !== activeComposerSessionId) {
        flowChatStore.switchSession(activeComposerSessionId);
      }
      syncSessionToModernStore(activeComposerSessionId);
      return true;
    };

    if (activateDedicatedSession()) return;
    const unsubscribe = flowChatStore.subscribe((state) => {
      if (!state.sessions.has(activeComposerSessionId)) return;
      unsubscribe();
      activateDedicatedSession();
    });
    return unsubscribe;
  }, [activeComposerSessionId, activeComposerToken, isOpen]);

  // A MiniApp holding the composer can hand it a prepared prompt (PPT Live's
  // welcome examples). Open the bubble so the user sees what landed there and
  // can edit before sending — this never submits on its own.
  useEffect(() => {
    const claimToken = activeComposerClaim?.token;
    if (!claimToken) return;
    const handler = (e: Event) => {
      const detail = (e as CustomEvent<{ token?: string; text?: string }>).detail;
      if (!detail || detail.token !== claimToken || typeof detail.text !== 'string') return;
      setMiniAppComposerDraft(detail.text);
      handleOpen();
    };
    window.addEventListener(MINIAPP_COMPOSER_DRAFT_EVENT, handler);
    return () => {
      window.removeEventListener(MINIAPP_COMPOSER_DRAFT_EVENT, handler);
    };
  }, [activeComposerClaim?.token, handleOpen, setMiniAppComposerDraft]);

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
    isStreaming && 'bitfun-fmc__panel--processing',
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
        className={[
          'bitfun-fmc__button',
          activeComposerClaim && 'bitfun-fmc__button--miniapp',
          isMiniAppSessionExecuting && 'bitfun-fmc__button--processing',
        ].filter(Boolean).join(' ')}
        onPointerDown={handleTriggerPointerDown}
        onClick={handleOpen}
        aria-expanded={isOpen}
        aria-busy={isMiniAppSessionExecuting || undefined}
        aria-label={activeComposerClaim ? displayedTitle : t('toolCards.toolbar.startNewChat')}
      >
        {isMiniAppSessionExecuting && (
          <span
            className="bitfun-fmc__button-activity"
            aria-hidden="true"
          />
        )}
        {activeComposerClaim ? (
          <span
            className="bitfun-fmc__miniapp-trigger-icon"
            aria-hidden="true"
          >
            {renderMiniAppIcon(activeMiniAppIcon, 20)}
          </span>
        ) : (
          <MessageSquare size={20} />
        )}
      </button>

      {/* Expanded panel */}
      <div
        ref={panelRef}
        className={panelClassName}
        onKeyDown={handlePanelKeyDown}
        onTransitionEnd={handlePanelTransitionEnd}
      >
        {/* Header — normal chat keeps the shared SessionMenu. A claimed
            MiniApp bubble replaces that switcher with app identity so the user
            cannot navigate the dedicated composer back into a normal chat. */}
        <div className="bitfun-fmc__header">
          {activeComposerClaim ? (
            <div
              className="bitfun-fmc__miniapp-session-icon"
              aria-hidden="true"
            >
              {renderMiniAppIcon(activeMiniAppIcon, 14)}
            </div>
          ) : (
            <SessionMenu />
          )}

          <div className="bitfun-fmc__title-wrapper">
            <div className="bitfun-fmc__title-display" title={displayedTitle}>
              <span className="bitfun-fmc__title-text">{displayedTitle}</span>
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
          {surfaceMounted && (!activeComposerClaim || isMiniAppSessionReady) && (
            <ChatPane
              width={0}
              isFullscreen={false}
              isSceneActive
              workspacePath={
                activeComposerClaim
                  ? displayedSession?.workspacePath
                  : workspacePath
              }
              showChatInput
              chatInputRegistration={chatInputRegistration}
              emptyState={activeComposerClaim ? (
                <MiniAppBubbleWelcome
                  appName={activeMiniAppName}
                  appDescription={activeMiniAppDescription}
                  appIcon={activeMiniAppIcon}
                  customization={bubbleCustomization}
                  workspacePath={displayedSession?.workspacePath}
                  onSuggestion={handleMiniAppSuggestion}
                />
              ) : undefined}
            />
          )}
          {surfaceMounted && activeComposerClaim && !isMiniAppSessionReady && (
            <div className="bitfun-fmc__miniapp-session-pending">
              <div
                className="bitfun-fmc__miniapp-session-pending-icon"
                aria-hidden="true"
              >
                {renderMiniAppIcon(activeMiniAppIcon, 22)}
              </div>
              <span>
                {t('miniAppComposer.hint', { app: activeMiniAppName })}
              </span>
            </div>
          )}
        </div>
      </div>
    </div>
  );
};

export default FloatingMiniChat;
