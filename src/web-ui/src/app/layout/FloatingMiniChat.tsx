/**
 * Hello — the bottom-right entry that expands to the shared session surface.
 * Text chat is always the default; realtime voice is an explicit mode switch
 * inside the expanded panel.
 *
 * The panel renders ChatPane verbatim — same conversation view, same full
 * composer as the session scene — so it never lags behind the main chat UI and
 * carries no second conversation/composer implementation of its own. Only the
 * bubble chrome (trigger, open/close animation, session header) lives here.
 */

import React, { useState, useCallback, useMemo, useEffect, useRef } from 'react';
import { useTranslation } from 'react-i18next';

import { flowChatStore } from '../../flow_chat/store/FlowChatStore';
import { syncSessionToModernStore } from '../../flow_chat/services/storeSync';
import ChatPane from '@/app/scenes/session/ChatPane';
import type {
  ChatInputRegistration,
  ChatInputSubmission,
} from '@/flow_chat/components/chatInputRegistration';
import { Icon, IconButton, LauncherButton, Tooltip } from '@openbitfun/ui';

import { useCurrentWorkspace } from '@/infrastructure/contexts/WorkspaceContext';
import { SessionMenu, useFlowChatSessions } from '../../flow_chat/components/session-menu';
import { useSceneStore } from '@/app/stores/sceneStore';
import {
  useMiniAppStore,
  MINIAPP_COMPOSER_DRAFT_EVENT,
  type MiniAppComposerClaim,
  type MiniAppDraftEventDetail,
  type MiniAppComposerMessageDetail,
} from '@/app/scenes/miniapps/miniAppStore';
import { postMiniAppComposerMessage } from '@/app/scenes/miniapps/miniAppComposerMessages';
import { pickLocalizedString } from '@/app/scenes/miniapps/utils/pickLocalizedString';
import { renderMiniAppIcon } from '@/app/scenes/miniapps/utils/miniAppIcons';
import MiniAppBubbleWelcome from './MiniAppBubbleWelcome';
import { isFloatingMiniChatSessionExecuting } from './floatingMiniChatActivity';
import {
  canRenderFloatingMiniChatSession,
  isFloatingMiniChatIsolated,
} from './floatingMiniChatIsolation';
import {
  ConversationModeSurface,
  ConversationVoiceModeIcon,
} from '@/flow_chat/components/voice/ConversationModeSurface';
import { useRealtimeVoiceCall } from '@/flow_chat/components/voice/RealtimeVoiceCallContext';
import type { VoiceMiniAppCallTarget } from '@/flow_chat/components/voice/voiceClientContext';
import './FloatingMiniChat.scss';

/**
 * Panel lifecycle. `opening` covers the scale-up transition, during which the
 * panel's hit area is still smaller than its final rect — see the backdrop
 * below for why that distinction matters.
 */
type PanelPhase = 'closed' | 'opening' | 'open';

/** Fallback for a transitionend that never arrives (reduced motion, interrupted
 *  transition). Must stay >= $fmc-open-duration in FloatingMiniChat.scss. */
const PANEL_OPEN_SETTLE_MS = 280;

export const FloatingMiniChat: React.FC = () => {
  const { t, i18n } = useTranslation('flow-chat');
  const { t: tVoice } = useTranslation('settings/voice-input');
  const {
    phase: voicePhase,
    end: endVoiceCall,
  } = useRealtimeVoiceCall();
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
    claimToken: string;
    sessionId?: string;
  } | null>(null);
  const composerPrefillIdRef = useRef(0);
  /** True while a press that may legitimately close the panel is in flight. */
  const backdropArmedRef = useRef(false);
  const isOpen = phase !== 'closed';
  const isVoiceMode = voicePhase !== 'idle';
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
  const isMiniAppBubbleIsolated = isFloatingMiniChatIsolated({
    activeMiniAppId,
    permissions: activeMiniApp?.permissions,
    hasComposerClaim: Boolean(activeComposerClaim),
  });
  const {
    activeSession,
    trackedSession: activeMiniAppSession,
    sessionTitle,
  } = useFlowChatSessions(activeComposerSessionId);
  const isMiniAppSessionReady = canRenderFloatingMiniChatSession({
    isolated: isMiniAppBubbleIsolated,
    claimedSessionId: activeComposerSessionId,
    activeSessionId: activeSession?.sessionId,
  });
  const displayedSession = isMiniAppBubbleIsolated
    ? activeMiniAppSession
    : activeSession;
  const miniAppVoiceTarget = useMemo<VoiceMiniAppCallTarget | undefined>(() => {
    if (
      !isMiniAppBubbleIsolated
      || !activeMiniAppId
      || !activeComposerToken
      || !activeComposerSessionId
      || !isMiniAppSessionReady
    ) return undefined;
    return {
      kind: 'miniapp',
      appId: activeMiniAppId,
      appName: activeMiniAppName,
      claimToken: activeComposerToken,
      sessionId: activeComposerSessionId,
      workspacePath: displayedSession?.workspacePath,
    };
  }, [
    activeComposerSessionId,
    activeComposerToken,
    activeMiniAppId,
    activeMiniAppName,
    displayedSession?.workspacePath,
    isMiniAppBubbleIsolated,
    isMiniAppSessionReady,
  ]);
  const displayedTitle = isMiniAppBubbleIsolated
    ? (bubbleCustomization?.title || activeMiniAppName)
    : sessionTitle;
  const popupTitle = isVoiceMode ? tVoice('voiceCall.call.title') : displayedTitle;

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
    if (isVoiceMode) endVoiceCall();
    setPhase('closed');
  }, [endVoiceCall, isVoiceMode]);

  const setMiniAppComposerDraft = useCallback((
    text: string,
    claimToken: string,
    sessionId?: string,
  ) => {
    composerPrefillIdRef.current += 1;
    setComposerPrefill({
      id: composerPrefillIdRef.current,
      text,
      claimToken,
      sessionId,
    });
  }, []);

  const handleMiniAppSuggestion = useCallback((prompt: string) => {
    if (!activeComposerToken) return;
    setMiniAppComposerDraft(prompt, activeComposerToken, activeComposerSessionId);
  }, [activeComposerSessionId, activeComposerToken, setMiniAppComposerDraft]);

  const handleMiniAppDraftConsumed = useCallback((id: number) => {
    setComposerPrefill((current) => (current?.id === id ? null : current));
  }, []);

  const handleMiniAppSubmit = useCallback((submission: ChatInputSubmission) => {
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
    postMiniAppComposerMessage(detail);
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
      remoteConnectionId:
        displayedSession?.remoteConnectionId
        || displayedSession?.config?.remoteConnectionId,
      draft: composerPrefill
        ? { id: composerPrefill.id, text: composerPrefill.text }
        : undefined,
      onDraftConsumed: handleMiniAppDraftConsumed,
      onSubmit: handleMiniAppSubmit,
    };
  }, [
    activeComposerClaim,
    activeMiniAppName,
    bubbleCustomization?.composer?.placeholder,
    composerPrefill,
    displayedSession?.config?.remoteConnectionId,
    displayedSession?.remoteConnectionId,
    displayedSession?.workspacePath,
    handleMiniAppDraftConsumed,
    handleMiniAppSubmit,
    t,
  ]);

  useEffect(() => {
    setComposerPrefill((current) => {
      if (!current) return current;
      if (current.claimToken !== activeComposerToken) return null;
      // focusSession and setComposerDraft are two consecutive bridge calls.
      // React may observe their host-store updates in either order, so keep a
      // draft that already identifies the newly focused session instead of
      // clearing it during that transition.
      if (current.sessionId && current.sessionId !== activeComposerSessionId) {
        return null;
      }
      return current;
    });
  }, [activeComposerSessionId, activeComposerToken]);

  const activateMiniAppSession = useCallback((claim: MiniAppComposerClaim | undefined) => {
    const sessionId = claim?.sessionId;
    if (!sessionId) return false;
    const state = flowChatStore.getState();
    if (!state.sessions.has(sessionId)) return false;
    if (state.activeSessionId !== sessionId) {
      flowChatStore.switchSession(sessionId);
    }
    syncSessionToModernStore(sessionId);
    return true;
  }, []);

  const rememberPreviousHostSession = useCallback((claimToken: string) => {
    if (previousHostSessionRef.current?.claimToken === claimToken) return;
    const state = flowChatStore.getState();
    const currentSession = state.activeSessionId
      ? state.sessions.get(state.activeSessionId)
      : undefined;
    previousHostSessionRef.current = {
      claimToken,
      sessionId:
        currentSession && currentSession.sessionKind !== 'miniapp'
          ? currentSession.sessionId
          : null,
    };
  }, []);

  const restorePreviousHostSession = useCallback((claimToken?: string) => {
    const previous = previousHostSessionRef.current;
    if (
      !previous
      || (claimToken !== undefined && previous.claimToken !== claimToken)
    ) return;
    previousHostSessionRef.current = null;
    if (!previous.sessionId) return;

    const latest = flowChatStore.getState();
    if (!latest.sessions.has(previous.sessionId)) return;
    const currentSession = latest.activeSessionId
      ? latest.sessions.get(latest.activeSessionId)
      : undefined;
    // Do not overwrite an explicit navigation that already selected another
    // ordinary session while the bubble was closing/unmounting.
    if (
      currentSession
      && currentSession.sessionKind !== 'miniapp'
      && currentSession.sessionId !== previous.sessionId
    ) {
      return;
    }
    if (latest.activeSessionId !== previous.sessionId) {
      flowChatStore.switchSession(previous.sessionId);
    }
    syncSessionToModernStore(previous.sessionId);
  }, []);

  // ChatPane is intentionally the shared session surface, so displaying a
  // MiniApp session temporarily switches the shared store. Capture the user's
  // normal session for the lifetime of the open claimed bubble and restore it
  // when the panel closes or the active MiniApp changes.
  useEffect(() => {
    if (!isOpen || !activeComposerToken) return;

    rememberPreviousHostSession(activeComposerToken);

    return () => {
      restorePreviousHostSession(activeComposerToken);
    };
  }, [
    activeComposerToken,
    isOpen,
    rememberPreviousHostSession,
    restorePreviousHostSession,
  ]);

  // setComposerDraft activates the MiniApp session synchronously before the
  // open-state effect commits. Keep an unconditional unmount fallback so a
  // simultaneous scene change cannot strand that hidden session globally.
  useEffect(() => () => {
    restorePreviousHostSession();
  }, [restorePreviousHostSession]);

  // The claim carries the topic's dedicated session. Activate it only while
  // the bubble is open; if the Agent session has not reached FlowChat yet,
  // wait for that exact id instead of falling back to the latest normal chat.
  useEffect(() => {
    if (isVoiceMode || !isOpen || !activeComposerToken || !activeComposerSessionId) return;

    if (activateMiniAppSession(activeComposerClaim)) return;
    const unsubscribe = flowChatStore.subscribe((state) => {
      if (!state.sessions.has(activeComposerSessionId)) return;
      unsubscribe();
      activateMiniAppSession(activeComposerClaim);
    });
    return unsubscribe;
  }, [
    activateMiniAppSession,
    activeComposerClaim,
    activeComposerSessionId,
    activeComposerToken,
    isOpen,
    isVoiceMode,
  ]);

  // A MiniApp holding the composer can hand it a prepared prompt (PPT Live's
  // welcome examples). Open the bubble so the user sees what landed there and
  // can edit before sending — this never submits on its own.
  useEffect(() => {
    const claimToken = activeComposerClaim?.token;
    if (!claimToken || !activeMiniAppId) return;
    const handler = (e: Event) => {
      const detail = (e as CustomEvent<MiniAppDraftEventDetail>).detail;
      if (!detail || detail.token !== claimToken || typeof detail.text !== 'string') return;
      // focusSession and setComposerDraft can arrive before React commits the
      // updated claim. Read the live claim, switch synchronously, then open the
      // panel so its first rendered frame already owns the correct session.
      const liveClaim = useMiniAppStore.getState().composerClaims[activeMiniAppId];
      if (liveClaim?.token !== detail.token) return;
      // The live claim is authoritative if a newer focusSession overtook an
      // older queued draft event. The event snapshot is only a fallback for
      // the same host transition.
      const sessionId = liveClaim.sessionId || detail.sessionId;
      const draftClaim = sessionId && liveClaim.sessionId !== sessionId
        ? { ...liveClaim, sessionId }
        : liveClaim;
      // The draft path activates the topic before React opens the panel, so
      // capture the ordinary chat first; closing the bubble must restore it.
      rememberPreviousHostSession(detail.token);
      activateMiniAppSession(draftClaim);
      setMiniAppComposerDraft(detail.text, detail.token, sessionId);
      handleOpen();
    };
    window.addEventListener(MINIAPP_COMPOSER_DRAFT_EVENT, handler);
    return () => {
      window.removeEventListener(MINIAPP_COMPOSER_DRAFT_EVENT, handler);
    };
  }, [
    activateMiniAppSession,
    activeComposerClaim?.token,
    activeMiniAppId,
    handleOpen,
    rememberPreviousHostSession,
    setMiniAppComposerDraft,
  ]);

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
    if (!isOpen || isVoiceMode) {
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
  }, [isOpen, isVoiceMode]);

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
    'openbitfun-fmc__panel',
    isOpen && 'openbitfun-fmc__panel--open',
    isStreaming && 'openbitfun-fmc__panel--processing',
  ]
    .filter(Boolean)
    .join(' ');

  return (
    <div
      data-openbitfun-component="floating-mini-chat"
      data-openbitfun-part="root"
      data-openbitfun-mode={activeComposerClaim ? 'miniapp' : 'chat'}
      data-openbitfun-communication-mode={isVoiceMode ? 'voice' : 'chat'}
      data-openbitfun-state={[
        isOpen && 'open',
        isStreaming && 'processing',
        isVoiceMode && 'voice',
        shouldAvoidMiniAppCustomizer && 'customizing',
      ].filter(Boolean).join(' ') || undefined}
      className={[
        'openbitfun-fmc',
        isOpen && 'openbitfun-fmc--open',
        shouldAvoidMiniAppCustomizer && 'openbitfun-fmc--miniapp-customizing',
      ].filter(Boolean).join(' ')}
    >
      {/* Fullscreen backdrop to catch outside clicks. It stays inert until the
          panel has finished scaling up: until then the panel's hit area is
          still smaller than its final rect, so a click aimed at the panel would
          land here and close what the user just opened. Rendering it (without
          the close handler) throughout keeps those clicks from reaching the
          scene underneath.

          Closing happens on click, NOT on mousedown: closing on mousedown
          unmounts the backdrop in the middle of the press gesture, and pulling
          the hit target of an in-flight gesture out from over a cross-document
          iframe (a MiniApp) leaves the webview routing subsequent mousemoves
          against the stale layer — the iframe loses hover and eats an extra
          click until the next mousedown re-hit-tests. Arming on mousedown
          preserves the opening-phase protection above: a press that began
          while the panel was still scaling up never closes it. */}
      {isOpen && (
        <div
          data-openbitfun-component="floating-mini-chat"
          data-openbitfun-part="backdrop"
          className="openbitfun-fmc__backdrop"
          onMouseDown={() => { backdropArmedRef.current = phase === 'open'; }}
          onClick={() => {
            if (backdropArmedRef.current) handleClose();
            backdropArmedRef.current = false;
          }}
        />
      )}

      {/* Hello trigger — sits above the backdrop and the collapsed panel via
          an explicit z-index, and is taken out of hit testing with
          `visibility` (not just pointer-events) while the panel is open. */}
      {isMiniAppBubbleIsolated ? (
        <button
          data-openbitfun-component="floating-mini-chat"
          data-openbitfun-part="trigger"
          type="button"
          className={[
            'openbitfun-fmc__button',
            'openbitfun-fmc__button--miniapp',
            isMiniAppSessionExecuting && 'openbitfun-fmc__button--processing',
          ].filter(Boolean).join(' ')}
          onPointerDown={handleTriggerPointerDown}
          onClick={handleOpen}
          aria-expanded={isOpen}
          aria-busy={isMiniAppSessionExecuting || undefined}
          aria-label={displayedTitle}
        >
          {isMiniAppSessionExecuting && (
            <span
              className="openbitfun-fmc__button-activity"
              data-openbitfun-component="floating-mini-chat"
              data-openbitfun-part="triggerActivity"
              aria-hidden="true"
            />
          )}
          <span
            className="openbitfun-fmc__miniapp-trigger-icon"
            data-openbitfun-component="floating-mini-chat"
            data-openbitfun-part="triggerIcon"
            aria-hidden="true"
          >
            {renderMiniAppIcon(activeMiniAppIcon, 20)}
          </span>
        </button>
      ) : (
        <LauncherButton
          aria-expanded={isOpen}
          aria-label={t('toolCards.toolbar.startNewChat')}
          className="openbitfun-fmc__button openbitfun-fmc__button--hello"
          onClick={handleOpen}
          onPointerDown={handleTriggerPointerDown}
        >
          <span
            aria-hidden="true"
            className="openbitfun-fmc__button-label openbitfun-fmc__button-label--compact"
          >
            {tVoice('voiceCall.call.launcherCompactLabel')}
          </span>
          <span
            aria-hidden="true"
            className="openbitfun-fmc__button-label openbitfun-fmc__button-label--expanded"
          >
            {tVoice('voiceCall.call.launcherLabel')}
          </span>
        </LauncherButton>
      )}

      {/* Expanded panel */}
      <div
        ref={panelRef}
        className={panelClassName}
        onKeyDown={handlePanelKeyDown}
        onTransitionEnd={handlePanelTransitionEnd}
        data-openbitfun-component="floating-mini-chat"
        data-openbitfun-part="panel"
      >
        {/* Header — normal chat keeps the shared SessionMenu. An isolated
            Agentic MiniApp replaces that switcher with app identity, including
            during claim/session bootstrap, so normal chats are never exposed. */}
        <div className="openbitfun-fmc__header" data-openbitfun-component="floating-mini-chat" data-openbitfun-part="header">
          {isVoiceMode ? (
            <ConversationVoiceModeIcon />
          ) : isMiniAppBubbleIsolated ? (
            <div
              className="openbitfun-fmc__miniapp-session-icon"
              data-openbitfun-component="floating-mini-chat"
              data-openbitfun-part="sessionIcon"
              aria-hidden="true"
            >
              {renderMiniAppIcon(activeMiniAppIcon, 14)}
            </div>
          ) : (
            <SessionMenu />
          )}

          <div className="openbitfun-fmc__title-wrapper" data-openbitfun-component="floating-mini-chat" data-openbitfun-part="title">
            <div className="openbitfun-fmc__title-display" title={popupTitle}>
              <span className="openbitfun-fmc__title-text">{popupTitle}</span>
            </div>
          </div>

          {/* Tool confirmation and stop controls come from the reused session
              surface (permission panel above ChatInput, ChatInput stop button),
              so the header only owns bubble chrome. */}
          <Tooltip content={t('session.close')}>
            <IconButton
              data-openbitfun-component="floating-mini-chat"
              data-openbitfun-part="headerAction"
              className="openbitfun-fmc__close"
              icon={<Icon name="xmark" size="lg" />}
              onClick={handleClose}
              size="sm"
              aria-label={t('session.close')}
            />
          </Tooltip>
        </div>

        {/* Main window session surface, reused as-is. Only mounted while the
            panel is open to avoid running a second VirtualMessageList and store
            sync in the background while the agent streams in another scene. */}
        <div
          className="openbitfun-fmc__body"
          data-openbitfun-component="floating-mini-chat"
          data-openbitfun-part="body"
        >
          <ConversationModeSurface
            voiceTarget={miniAppVoiceTarget}
            voiceStartDisabled={isMiniAppBubbleIsolated && !miniAppVoiceTarget}
            switchTestId="hello-realtime-voice-mode-switch"
          >
            {surfaceMounted && isMiniAppSessionReady && (
              <ChatPane
                width={0}
                isFullscreen={false}
                isSceneActive
                workspacePath={
                  isMiniAppBubbleIsolated
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
            {surfaceMounted && isMiniAppBubbleIsolated && !isMiniAppSessionReady && (
              <div className="openbitfun-fmc__miniapp-session-pending" data-openbitfun-component="floating-mini-chat" data-openbitfun-part="pending">
                <div
                  className="openbitfun-fmc__miniapp-session-pending-icon"
                  data-openbitfun-component="floating-mini-chat"
                  data-openbitfun-part="pendingIcon"
                  aria-hidden="true"
                >
                  {renderMiniAppIcon(activeMiniAppIcon, 22)}
                </div>
                <span>
                  {t('miniAppComposer.hint', { app: activeMiniAppName })}
                </span>
              </div>
            )}
          </ConversationModeSurface>
        </div>
      </div>
    </div>
  );
};

export default FloatingMiniChat;
