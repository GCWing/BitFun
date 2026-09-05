import React, { useEffect, useLayoutEffect, useRef, useState, useCallback, useMemo } from 'react';
import { MobileIconButton } from '@openbitfun/ui/mobile';
import { useI18n } from '../i18n';
import { useControlTargetEpoch } from '../hooks/useControlTargetEpoch';
import {
  isRemoteControlTargetChangedError,
  RemoteControlTargetChangedError,
  RemoteSessionManager,
  SessionPoller,
  type PollResponse,
  type ChatMessage,
  type RemoteModelCatalog,
} from '../services/RemoteSessionManager';
import { useMobileStore } from '../services/store';
import { createRemoteCacheScope, remoteCache } from '../services/RemoteCache';
import ChatHeader from '../components/ChatHeader';
import ChatComposerBar from '../components/ChatComposerBar';
import {
  loadLastSelectedModelId,
  ModelSelectorPill,
  normalizeSelectedModelId,
  persistLastSelectedModelId,
  ReasoningPresetPill,
  resolvePreferredModelSelection,
} from '../components/ChatModelControls';
import ChatMessageActions from '../components/ChatMessageActions';
import ChatFeedback from '../components/ChatFeedback';
import { copyToClipboard } from '../components/ChatMarkdown';
import ChatTranscript from '../components/ChatTranscript';

function reportRemoteSessionError(
  error: unknown,
  setError: (message: string) => void,
): void {
  if (isRemoteControlTargetChangedError(error)) return;
  setError(error instanceof Error ? error.message : String(error));
}

interface ChatPageProps {
  sessionMgr: RemoteSessionManager;
  sessionId: string;
  sessionName?: string;
  agentType?: string;
  onBack: () => void;
  autoFocus?: boolean;
  wideLayout?: boolean;
}

// ─── Markdown ───────────────────────────────────────────────────────────────

function sanitizeMessageText(content: string): string {
  return content
    .replace(/#img:\S+\s*/g, '')
    .replace(/\[Image:.*?\]\n(?:Path:.*?\n|Image ID:.*?\n)?/g, '')
    .trim();
}


// ─── Thinking (ModelThinkingDisplay-style) ───────────────────────────────────



// ─── ChatPage ───────────────────────────────────────────────────────────────

const ChatPage: React.FC<ChatPageProps> = ({
  sessionMgr,
  sessionId,
  sessionName,
  agentType: sessionAgentType = 'agentic',
  onBack,
  autoFocus,
  wideLayout = false,
}) => {
  const { t } = useI18n();
  const {
    getMessages,
    setMessages,
    appendNewMessages,
    activeTurn,
    setActiveTurn,
    error,
    setError,
    currentWorkspace,
    authenticatedUserId,
    controlTarget,
    updateSessionName,
  } = useMobileStore();

  const messages = getMessages(sessionId);
  const [input, setInput] = useState('');
  const [liveTitle, setLiveTitle] = useState(sessionName);
  const [modelCatalog, setModelCatalog] = useState<RemoteModelCatalog | null>(null);
  const [selectedModelId, setSelectedModelId] = useState<string>('auto');
  const [modelUpdating, setModelUpdating] = useState(false);
  const [pendingImages, setPendingImages] = useState<{ name: string; dataUrl: string }[]>([]);
  const [imageAnalyzing, setImageAnalyzing] = useState(false);
  const [isCancelling, setIsCancelling] = useState(false);
  const [optimisticMsg, setOptimisticMsg] = useState<{
    id: string; text: string; images: { name: string; data_url: string }[];
  } | null>(null);
  const [inputExpanded, setInputExpanded] = useState(!!autoFocus);
  const inputRef = useRef<HTMLTextAreaElement>(null);
  const fileInputRef = useRef<HTMLInputElement>(null);
  const inputBarRef = useRef<HTMLDivElement>(null);
  const pollerRef = useRef<SessionPoller | null>(null);
  const messagesRequestSeqRef = useRef(0);
  const [isLoadingMore, setIsLoadingMore] = useState(false);
  const [hasMore, setHasMore] = useState(true);
  const isLoadingMoreRef = useRef(false);
  const hasMoreRef = useRef(true);
  const controlTargetEpoch = useControlTargetEpoch(sessionMgr);
  const cacheScope = useMemo(() => createRemoteCacheScope(
    authenticatedUserId,
    controlTarget?.deviceId ?? sessionMgr.controlTargetDeviceId,
  ), [authenticatedUserId, controlTarget?.deviceId, sessionMgr, controlTargetEpoch]);
  const chatTargetOwnerRef = useRef({
    sessionMgr,
    sessionId,
    epoch: controlTargetEpoch,
    active: true,
  });

  if (
    chatTargetOwnerRef.current.sessionMgr !== sessionMgr
    || chatTargetOwnerRef.current.sessionId !== sessionId
    || chatTargetOwnerRef.current.epoch !== controlTargetEpoch
  ) {
    chatTargetOwnerRef.current = {
      sessionMgr,
      sessionId,
      epoch: controlTargetEpoch,
      active: true,
    };
  }

  const captureChatTargetEpoch = useCallback((): number | null => {
    const owner = chatTargetOwnerRef.current;
    if (
      !owner.active
      || owner.sessionMgr !== sessionMgr
      || owner.sessionId !== sessionId
      || owner.epoch !== sessionMgr.controlTargetEpoch
    ) {
      return null;
    }
    return owner.epoch;
  }, [controlTargetEpoch, sessionId, sessionMgr]);

  const isChatTargetCurrent = useCallback((epoch: number | null): boolean => {
    const owner = chatTargetOwnerRef.current;
    return epoch !== null
      && owner.active
      && owner.sessionMgr === sessionMgr
      && owner.sessionId === sessionId
      && owner.epoch === epoch
      && sessionMgr.controlTargetEpoch === epoch;
  }, [controlTargetEpoch, sessionId, sessionMgr]);

  const modelSelectionInitializedRef = useRef(false);
  const modelCatalogRequestSeqRef = useRef(0);
  const messagesEndRef = useRef<HTMLDivElement>(null);
  const messagesContainerRef = useRef<HTMLDivElement>(null);
  const [expandedMsgIds, setExpandedMsgIds] = useState<Set<string>>(new Set());
  const [infoToast, setInfoToast] = useState<string | null>(null);
  const [showScrollToBottom, setShowScrollToBottom] = useState(false);
  const [menuMessage, setMenuMessage] = useState<ChatMessage | null>(null);
  const [actionToast, setActionToast] = useState<string | null>(null);
  const [deletingMsg, setDeletingMsg] = useState(false);
  const msgLongPressTimerRef = useRef<ReturnType<typeof setTimeout>>();
  const msgLongPressPosRef = useRef({ x: 0, y: 0 });
  const msgToastTimerRef = useRef<ReturnType<typeof setTimeout>>();
  const committedChatTargetRef = useRef({ sessionMgr, sessionId, epoch: controlTargetEpoch });

  useLayoutEffect(() => {
    const previous = committedChatTargetRef.current;
    const targetChanged = previous.sessionMgr !== sessionMgr
      || previous.sessionId !== sessionId
      || previous.epoch !== controlTargetEpoch;
    const owner = chatTargetOwnerRef.current;
    owner.active = owner.sessionMgr === sessionMgr
      && owner.sessionId === sessionId
      && owner.epoch === controlTargetEpoch
      && sessionMgr.controlTargetEpoch === controlTargetEpoch;
    if (targetChanged) {
      messagesRequestSeqRef.current += 1;
      modelCatalogRequestSeqRef.current += 1;
      isLoadingMoreRef.current = false;
      hasMoreRef.current = true;
      setIsLoadingMore(false);
      setHasMore(true);
      setModelUpdating(false);
      setImageAnalyzing(false);
      setIsCancelling(false);
      setOptimisticMsg(null);
      modelSelectionInitializedRef.current = false;
      setModelCatalog(null);
      setSelectedModelId('auto');
      setMessages(sessionId, []);
      setMenuMessage(null);
      setDeletingMsg(false);
      setActionToast(null);
      setInfoToast(null);
      setExpandedMsgIds(new Set());
      setShowScrollToBottom(false);
      setActiveTurn(null);
      if (msgLongPressTimerRef.current) {
        clearTimeout(msgLongPressTimerRef.current);
        msgLongPressTimerRef.current = undefined;
      }
      if (msgToastTimerRef.current) {
        clearTimeout(msgToastTimerRef.current);
        msgToastTimerRef.current = undefined;
      }
      pollerRef.current?.stop();
      pollerRef.current = null;
    }
    committedChatTargetRef.current = { sessionMgr, sessionId, epoch: controlTargetEpoch };
    return () => {
      owner.active = false;
      messagesRequestSeqRef.current += 1;
      modelCatalogRequestSeqRef.current += 1;
      pollerRef.current?.stop();
    };
  }, [controlTargetEpoch, sessionId, sessionMgr, setActiveTurn, setMessages]);

  const isStreaming = activeTurn != null && activeTurn.status === 'active';

  useEffect(() => {
    if (!isStreaming) setIsCancelling(false);
  }, [isStreaming]);

  const [now, setNow] = useState(() => Date.now());
  const handleAnswerQuestion = useCallback(async (toolId: string, answers: any) => {
    const targetEpoch = captureChatTargetEpoch();
    if (targetEpoch === null) throw new RemoteControlTargetChangedError();
    try {
      await sessionMgr.answerQuestion(toolId, answers);
      if (!isChatTargetCurrent(targetEpoch)) throw new RemoteControlTargetChangedError();
    } catch (err) {
      reportRemoteSessionError(err, setError);
      throw err;
    }
  }, [captureChatTargetEpoch, isChatTargetCurrent, sessionMgr, setError]);

  const handleApproveTool = useCallback(async (toolId: string) => {
    const targetEpoch = captureChatTargetEpoch();
    if (targetEpoch === null) throw new RemoteControlTargetChangedError();
    try {
      await sessionMgr.confirmTool(toolId);
      if (!isChatTargetCurrent(targetEpoch)) throw new RemoteControlTargetChangedError();
      pollerRef.current?.nudge();
    } catch (err) {
      reportRemoteSessionError(err, setError);
      throw err;
    }
  }, [captureChatTargetEpoch, isChatTargetCurrent, sessionMgr, setError]);

  const handleRejectTool = useCallback(async (toolId: string) => {
    const targetEpoch = captureChatTargetEpoch();
    if (targetEpoch === null) throw new RemoteControlTargetChangedError();
    try {
      await sessionMgr.rejectTool(toolId, t('chat.rejectedByUser'));
      if (!isChatTargetCurrent(targetEpoch)) throw new RemoteControlTargetChangedError();
      pollerRef.current?.nudge();
    } catch (err) {
      reportRemoteSessionError(err, setError);
      throw err;
    }
  }, [captureChatTargetEpoch, isChatTargetCurrent, sessionMgr, setError, t]);

  const handleCancelTool = useCallback((toolId: string, reason = t('common.cancel')) => {
    if (captureChatTargetEpoch() === null) return;
    sessionMgr.cancelTool(toolId, reason).catch((error) => {
      reportRemoteSessionError(error, setError);
    });
  }, [captureChatTargetEpoch, sessionMgr, setError, t]);

  /** Fetch metadata for a workspace file before the user confirms the download. */
  const handleGetFileInfo = useCallback(
    async (filePath: string) => {
      const targetEpoch = captureChatTargetEpoch();
      if (targetEpoch === null) throw new RemoteControlTargetChangedError();
      const info = await sessionMgr.getFileInfo(filePath, sessionId);
      if (!isChatTargetCurrent(targetEpoch)) throw new RemoteControlTargetChangedError();
      return info;
    },
    [captureChatTargetEpoch, isChatTargetCurrent, sessionId, sessionMgr],
  );

  /** Download a workspace file referenced by a `computer://` link. */
  const handleFileDownload = useCallback(async (
    filePath: string,
    onProgress?: (downloaded: number, total: number) => void,
  ) => {
    const targetEpoch = captureChatTargetEpoch();
    if (targetEpoch === null) return;
    try {
      const { name, contentBase64, mimeType } = await sessionMgr.readFile(
        filePath,
        sessionId,
        (downloaded, total) => {
          if (isChatTargetCurrent(targetEpoch)) onProgress?.(downloaded, total);
        },
      );
      if (!isChatTargetCurrent(targetEpoch)) return;
      const byteCharacters = atob(contentBase64);
      const byteNumbers = new Uint8Array(byteCharacters.length);
      for (let i = 0; i < byteCharacters.length; i++) {
        byteNumbers[i] = byteCharacters.charCodeAt(i);
      }
      const blob = new Blob([byteNumbers], { type: mimeType });
      const url = URL.createObjectURL(blob);
      const anchor = document.createElement('a');
      anchor.href = url;
      anchor.download = name;
      document.body.appendChild(anchor);
      anchor.click();
      document.body.removeChild(anchor);
      URL.revokeObjectURL(url);
    } catch (err) {
      // Use the backend's message directly; it's already user-readable.
      reportRemoteSessionError(err, setError);
      throw err;
    }
  }, [captureChatTargetEpoch, isChatTargetCurrent, sessionId, sessionMgr, setError]);

  const loadModelCatalog = useCallback(async () => {
    const targetEpoch = captureChatTargetEpoch();
    if (targetEpoch === null) return null;
    const requestSeq = ++modelCatalogRequestSeqRef.current;
    try {
      const catalog = await sessionMgr.getModelCatalog(sessionId);
      if (
        requestSeq !== modelCatalogRequestSeqRef.current
        || !isChatTargetCurrent(targetEpoch)
      ) return null;
      setModelCatalog(catalog);
      if (!modelSelectionInitializedRef.current) {
        const preferredSelection = resolvePreferredModelSelection(loadLastSelectedModelId(), catalog);
        const sessionModelId = normalizeSelectedModelId(catalog.session_model_id || 'auto', catalog);
        const nextModelId = preferredSelection.modelId || sessionModelId;

        if (preferredSelection.modelId && preferredSelection.modelId !== sessionModelId) {
          const selection = catalog.reasoning_preset_selection_supported === true
            ? await sessionMgr.setSessionModelSelection(sessionId, preferredSelection.modelId, null)
            : {
                model_id: await sessionMgr.setSessionModel(sessionId, preferredSelection.modelId),
                reasoning_preset: null,
              };
          if (
            requestSeq !== modelCatalogRequestSeqRef.current
            || !isChatTargetCurrent(targetEpoch)
          ) return null;
          const normalizedModelId = selection.model_id;
          setSelectedModelId(normalizedModelId || 'auto');
          setModelCatalog(current => current ? {
            ...current,
            session_model_id: normalizedModelId,
            session_reasoning_preset: selection.reasoning_preset,
          } : current);
          if (preferredSelection.fellBackToAuto && (!normalizedModelId || normalizedModelId === 'auto')) {
            persistLastSelectedModelId('auto');
          }
        } else {
          setSelectedModelId(nextModelId || 'auto');
          if (preferredSelection.fellBackToAuto && nextModelId === 'auto') {
            persistLastSelectedModelId('auto');
          }
        }
        modelSelectionInitializedRef.current = true;
      }
      return catalog;
    } catch (err) {
      if (
        requestSeq === modelCatalogRequestSeqRef.current
        && isChatTargetCurrent(targetEpoch)
      ) reportRemoteSessionError(err, setError);
      return null;
    }
  }, [captureChatTargetEpoch, isChatTargetCurrent, sessionId, sessionMgr, setError]);

  const handleSelectModel = useCallback(async (modelId: string) => {
    if (modelUpdating || isStreaming || imageAnalyzing) return;
    const targetEpoch = captureChatTargetEpoch();
    if (targetEpoch === null) return;
    setModelUpdating(true);
    try {
      const selection = modelCatalog?.reasoning_preset_selection_supported === true
        ? await sessionMgr.setSessionModelSelection(sessionId, modelId, null)
        : {
            model_id: await sessionMgr.setSessionModel(sessionId, modelId),
            reasoning_preset: null,
          };
      if (!isChatTargetCurrent(targetEpoch)) return;
      const normalizedModelId = selection.model_id;
      setSelectedModelId(normalizedModelId || 'auto');
      setModelCatalog(current => current ? {
        ...current,
        session_model_id: normalizedModelId,
        session_reasoning_preset: selection.reasoning_preset,
      } : current);
      persistLastSelectedModelId(normalizedModelId || 'auto');
    } catch (err) {
      reportRemoteSessionError(err, setError);
    } finally {
      if (isChatTargetCurrent(targetEpoch)) setModelUpdating(false);
    }
  }, [captureChatTargetEpoch, imageAnalyzing, isChatTargetCurrent, isStreaming, modelCatalog?.reasoning_preset_selection_supported, modelUpdating, sessionId, sessionMgr, setError]);

  const handleSelectReasoningPreset = useCallback(async (reasoningPreset: string | null) => {
    if (
      modelUpdating
      || isStreaming
      || imageAnalyzing
      || modelCatalog?.reasoning_preset_selection_supported !== true
    ) return;
    const targetEpoch = captureChatTargetEpoch();
    if (targetEpoch === null) return;
    const previousReasoningPreset = modelCatalog.session_reasoning_preset ?? null;
    setModelUpdating(true);
    setModelCatalog(current => current ? {
      ...current,
      session_reasoning_preset: reasoningPreset,
    } : current);
    try {
      const selection = await sessionMgr.setSessionModelSelection(
        sessionId,
        selectedModelId,
        reasoningPreset,
      );
      if (!isChatTargetCurrent(targetEpoch)) return;
      setSelectedModelId(selection.model_id || 'auto');
      setModelCatalog(current => current ? {
        ...current,
        session_model_id: selection.model_id,
        session_reasoning_preset: selection.reasoning_preset,
      } : current);
    } catch (err) {
      if (isChatTargetCurrent(targetEpoch)) {
        setModelCatalog(current => current ? {
          ...current,
          session_reasoning_preset: previousReasoningPreset,
        } : current);
        reportRemoteSessionError(err, setError);
      }
    } finally {
      if (isChatTargetCurrent(targetEpoch)) setModelUpdating(false);
    }
  }, [captureChatTargetEpoch, imageAnalyzing, isChatTargetCurrent, isStreaming, modelCatalog?.reasoning_preset_selection_supported, modelUpdating, selectedModelId, sessionId, sessionMgr, setError]);

  useEffect(() => {
    if (!isStreaming) return;
    const timer = setInterval(() => setNow(Date.now()), 500);
    return () => clearInterval(timer);
  }, [isStreaming]);

  useEffect(() => {
    if (!error) return;
    const t = setTimeout(() => setError(null), 5000);
    return () => clearTimeout(t);
  }, [error, setError]);

  useEffect(() => {
    if (!infoToast) return;
    const timer = setTimeout(() => setInfoToast(null), 3200);
    return () => clearTimeout(timer);
  }, [infoToast]);

  const loadMessages = useCallback(async (beforeId?: string) => {
    if (beforeId && (isLoadingMoreRef.current || !hasMoreRef.current)) return;
    const targetEpoch = captureChatTargetEpoch();
    if (targetEpoch === null) return;
    const requestSeq = ++messagesRequestSeqRef.current;
    try {
      isLoadingMoreRef.current = true;
      setIsLoadingMore(true);
      const resp = await sessionMgr.getSessionMessages(sessionId, 50, beforeId);
      if (
        requestSeq !== messagesRequestSeqRef.current
        || !isChatTargetCurrent(targetEpoch)
      ) return;
      if (beforeId) {
        const currentMsgs = getMessages(sessionId);
        const nextMessages = [...resp.messages, ...currentMsgs];
        setMessages(sessionId, nextMessages);
        remoteCache.saveTranscript(cacheScope, sessionId, nextMessages, resp.has_more);
      } else {
        setMessages(sessionId, resp.messages);
        remoteCache.saveTranscript(cacheScope, sessionId, resp.messages, resp.has_more);
      }
      setHasMore(resp.has_more);
      hasMoreRef.current = resp.has_more;
    } catch (e: any) {
      if (
        requestSeq === messagesRequestSeqRef.current
        && isChatTargetCurrent(targetEpoch)
      ) reportRemoteSessionError(e, setError);
    } finally {
      if (
        requestSeq === messagesRequestSeqRef.current
        && isChatTargetCurrent(targetEpoch)
      ) {
        isLoadingMoreRef.current = false;
        setIsLoadingMore(false);
      }
    }
  }, [cacheScope, captureChatTargetEpoch, getMessages, isChatTargetCurrent, sessionId, sessionMgr, setError, setMessages]);

  // ── Message long-press context menu ──────────────────────────────
  const clearMsgLongPressTimer = () => {
    if (msgLongPressTimerRef.current) {
      clearTimeout(msgLongPressTimerRef.current);
      msgLongPressTimerRef.current = undefined;
    }
  };

  const handleMsgTouchStart = useCallback((m: ChatMessage, e: React.TouchEvent) => {
    if (deletingMsg) return;
    clearMsgLongPressTimer();
    msgLongPressPosRef.current = { x: e.touches[0].clientX, y: e.touches[0].clientY };
    msgLongPressTimerRef.current = setTimeout(() => {
      setMenuMessage(m);
      msgLongPressTimerRef.current = undefined;
    }, 500);
  }, [deletingMsg]);

  const handleMsgTouchMove = useCallback((e: React.TouchEvent) => {
    const dx = Math.abs(e.touches[0].clientX - msgLongPressPosRef.current.x);
    const dy = Math.abs(e.touches[0].clientY - msgLongPressPosRef.current.y);
    if (dx > 10 || dy > 10) clearMsgLongPressTimer();
  }, []);

  const handleMsgTouchEnd = useCallback(() => {
    clearMsgLongPressTimer();
  }, []);

  const showMsgToast = useCallback((msg: string) => {
    if (msgToastTimerRef.current) clearTimeout(msgToastTimerRef.current);
    setActionToast(msg);
    msgToastTimerRef.current = setTimeout(() => setActionToast(null), 2000);
  }, []);

  const handleCopyMessage = useCallback(async () => {
    if (!menuMessage) return;
    const text = sanitizeMessageText(menuMessage.content);
    try {
      await copyToClipboard(text);
      showMsgToast(t('chat.messageCopied'));
    } catch {
      showMsgToast(t('chat.copyFailed'));
    }
    setMenuMessage(null);
  }, [menuMessage, showMsgToast, t]);

  const handleResendMessage = useCallback(async () => {
    if (!menuMessage || menuMessage.role !== 'user') return;
    const targetEpoch = captureChatTargetEpoch();
    if (targetEpoch === null) return;
    const text = sanitizeMessageText(menuMessage.content);
    if (!text) return;
    setMenuMessage(null);
    const imageContexts = menuMessage.images?.length
      ? menuMessage.images.map((img, idx) => {
          const mimeType = img.data_url.split(';')[0]?.replace('data:', '') || 'image/png';
          return {
            id: `mobile_resend_${Date.now()}_${idx}`,
            data_url: img.data_url,
            mime_type: mimeType,
            metadata: { name: img.name, source: 'remote' },
          };
        })
      : undefined;
    try {
      await sessionMgr.sendMessage(sessionId, text, sessionAgentType, imageContexts);
      if (!isChatTargetCurrent(targetEpoch)) return;
      pollerRef.current?.nudge();
    } catch (e: any) {
      reportRemoteSessionError(e, setError);
    }
  }, [captureChatTargetEpoch, isChatTargetCurrent, menuMessage, sessionAgentType, sessionId, sessionMgr, setError]);

  const handleDeleteMessage = useCallback(async () => {
    if (!menuMessage) return;
    setDeletingMsg(true);
    try {
      useMobileStore.getState().deleteMessage(sessionId, menuMessage.id);
      remoteCache.saveTranscript(
        cacheScope,
        sessionId,
        useMobileStore.getState().getMessages(sessionId),
        hasMoreRef.current,
      );
      showMsgToast(t('chat.messageDeleted'));
    } finally {
      setDeletingMsg(false);
      setMenuMessage(null);
    }
  }, [cacheScope, menuMessage, sessionId, showMsgToast, t]);

  // Cleanup timers on unmount
  useEffect(() => {
    return () => {
      clearMsgLongPressTimer();
      if (msgToastTimerRef.current) clearTimeout(msgToastTimerRef.current);
    };
  }, []);

  const isNearBottomRef = useRef(true);
  const programmaticScrollRef = useRef(false);
  const lastShowScrollToBottomRef = useRef(false);
  const BOTTOM_THRESHOLD = 80;

  const handleScroll = useCallback(() => {
    const container = messagesContainerRef.current;
    if (!container) return;

    const gap = container.scrollHeight - container.scrollTop - container.clientHeight;
    const nearBottom = gap < BOTTOM_THRESHOLD;
    isNearBottomRef.current = nearBottom;
    if (nearBottom) {
      programmaticScrollRef.current = false;
    }
    if (!programmaticScrollRef.current) {
      const show = !nearBottom;
      if (show !== lastShowScrollToBottomRef.current) {
        lastShowScrollToBottomRef.current = show;
        setShowScrollToBottom(show);
      }
    }

    if (container.scrollTop < 100 && hasMore && !isLoadingMore) {
      const msgs = getMessages(sessionId);
      if (msgs.length > 0) loadMessages(msgs[0].id);
    }
  }, [hasMore, isLoadingMore, getMessages, sessionId, loadMessages]);

  const scrollToBottom = useCallback(() => {
    programmaticScrollRef.current = true;
    isNearBottomRef.current = true;
    setShowScrollToBottom(false);
    lastShowScrollToBottomRef.current = false;
    messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, []);

  // Initial load + start poller
  const initialScrollDone = useRef(false);
  const pendingInitialScroll = useRef(false);
  const chatInitSeqRef = useRef(0);
  useEffect(() => {
    modelSelectionInitializedRef.current = false;
    hasMoreRef.current = true;
    isLoadingMoreRef.current = false;
    setHasMore(true);
    setIsLoadingMore(false);
    setModelCatalog(null);
    setSelectedModelId('auto');
  }, [sessionId]);

  useEffect(() => {
    initialScrollDone.current = false;
    pendingInitialScroll.current = false;
    const initSeq = ++chatInitSeqRef.current;
    let cancelled = false;
    const targetEpoch = captureChatTargetEpoch();
    if (targetEpoch === null) return;
    const isInitCurrent = () => (
      !cancelled
      && chatInitSeqRef.current === initSeq
      && isChatTargetCurrent(targetEpoch)
    );
    const initialize = async () => {
      const catalogPromise = loadModelCatalog();
      const cached = await remoteCache.loadTranscript(cacheScope, sessionId);
      if (!isInitCurrent()) return;
      if (cached) {
        setMessages(sessionId, cached.messages);
        setHasMore(cached.hasMore);
        hasMoreRef.current = cached.hasMore;
        pendingInitialScroll.current = true;
      }

      // Always reconcile with the authoritative host. The cached transcript is
      // only an immediate paint and remains isolated to this account/device.
      await loadMessages();
      const initialCatalog = await catalogPromise;
      if (!isInitCurrent()) return;
      const initialMsgCount = useMobileStore.getState().getMessages(sessionId).length;
      pendingInitialScroll.current = true;

      const poller = new SessionPoller(sessionMgr, sessionId, (resp: PollResponse) => {
        if (!isInitCurrent()) return;
        if (resp.message_snapshot) {
          // Completion can grow the content of an already-counted assistant
          // message. Replace from the host's durable transcript; message count
          // alone cannot detect that repair.
          setMessages(sessionId, resp.message_snapshot);
          remoteCache.saveTranscript(
            cacheScope,
            sessionId,
            resp.message_snapshot,
            hasMoreRef.current,
          );
        } else if (resp.new_messages && resp.new_messages.length > 0) {
          appendNewMessages(sessionId, resp.new_messages);
          remoteCache.saveTranscript(
            cacheScope,
            sessionId,
            useMobileStore.getState().getMessages(sessionId),
            hasMoreRef.current,
          );
        }

        // Detect count mismatch (messages inserted in the middle due to
        // persistence race).  When the local count doesn't match the server
        // total, do a full reload to pick up all messages.
        if (resp.total_msg_count != null) {
          const localCount = useMobileStore.getState().getMessages(sessionId).length;
          if (localCount !== resp.total_msg_count) {
            sessionMgr.getSessionMessages(sessionId, 200).then(fresh => {
              if (!isInitCurrent()) return;
              useMobileStore.getState().setMessages(sessionId, fresh.messages);
              remoteCache.saveTranscript(cacheScope, sessionId, fresh.messages, fresh.has_more);
            }).catch(() => {});
          }
        }

        if (resp.title) {
          setLiveTitle(resp.title);
          updateSessionName(sessionId, resp.title);
          remoteCache.renameSession(cacheScope, sessionId, resp.title);
        }
        if (resp.model_catalog) {
          setModelCatalog(resp.model_catalog);
          setSelectedModelId(normalizeSelectedModelId(
            resp.model_catalog.session_model_id || 'auto',
            resp.model_catalog,
          ));
        }
        setActiveTurn(resp.active_turn ?? null);
      }, initialCatalog?.version || 0);

      poller.start(initialMsgCount);
      pollerRef.current = poller;
    };
    void initialize();

    return () => {
      cancelled = true;
      if (chatInitSeqRef.current === initSeq) chatInitSeqRef.current += 1;
      pollerRef.current?.stop();
      pollerRef.current = null;
      setActiveTurn(null);
    };
  }, [
    appendNewMessages,
    cacheScope,
    captureChatTargetEpoch,
    isChatTargetCurrent,
    loadMessages,
    loadModelCatalog,
    sessionId,
    sessionMgr,
    setActiveTurn,
    setMessages,
    updateSessionName,
  ]);

  const prevMsgCountRef = useRef(0);

  // Scroll to bottom BEFORE paint on initial message load,
  // so the user never sees the list at scroll-top then flash to bottom.
  useLayoutEffect(() => {
    if (!pendingInitialScroll.current || messages.length === 0) return;
    pendingInitialScroll.current = false;
    const container = messagesContainerRef.current;
    if (container) {
      container.scrollTop = container.scrollHeight;
    }
    initialScrollDone.current = true;
    prevMsgCountRef.current = messages.length;
  }, [messages]);

  useEffect(() => {
    if (!initialScrollDone.current) return;
    if (messages.length !== prevMsgCountRef.current) {
      const isNewAppend = messages.length > prevMsgCountRef.current;
      prevMsgCountRef.current = messages.length;
      if (isNewAppend && !isLoadingMore && isNearBottomRef.current) {
        programmaticScrollRef.current = true;
        messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' });
      }
    }
  }, [messages.length, isLoadingMore]);

  useEffect(() => {
    if (!initialScrollDone.current || !isStreaming) return;
    if (!isNearBottomRef.current) return;
    programmaticScrollRef.current = true;
    messagesEndRef.current?.scrollIntoView({ behavior: 'auto' });
  }, [activeTurn, isStreaming]);

  useEffect(() => {
    if (optimisticMsg) {
      programmaticScrollRef.current = true;
      isNearBottomRef.current = true;
      messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' });
    }
  }, [optimisticMsg]);

  useEffect(() => {
    if (!initialScrollDone.current || !isStreaming) return;
    const container = messagesContainerRef.current;
    if (!container) return;
    const tid = setInterval(() => {
      if (!isNearBottomRef.current) return;
      const gap = container.scrollHeight - container.scrollTop - container.clientHeight;
      if (gap > 10 && gap < 400) {
        programmaticScrollRef.current = true;
        container.scrollTo({ top: container.scrollHeight, behavior: 'smooth' });
      }
    }, 300);
    return () => clearInterval(tid);
  }, [isStreaming]);

  const handleSend = useCallback(async () => {
    const text = input.trim();
    const imgs = pendingImages;
    if ((!text && imgs.length === 0) || imageAnalyzing) return;
    const targetEpoch = captureChatTargetEpoch();
    if (targetEpoch === null) return;
    const wasStreaming = isStreaming;
    setInput('');
    setPendingImages([]);
    if (!wasStreaming) {
      setInputExpanded(false);
    }

    const hasImages = imgs.length > 0;
    const imageContexts = hasImages
      ? imgs.map((img, idx) => {
          const mimeType = img.dataUrl.split(';')[0]?.replace('data:', '') || 'image/png';
          return {
            id: `mobile_img_${Date.now()}_${idx}`,
            data_url: img.dataUrl,
            mime_type: mimeType,
            metadata: { name: img.name, source: 'remote' },
          };
        })
      : undefined;

    if (hasImages) {
      setOptimisticMsg({
        id: `opt_${Date.now()}`,
        text: text || '',
        images: imgs.map(i => ({ name: i.name, data_url: i.dataUrl })),
      });
      setImageAnalyzing(true);
    }

    try {
      await sessionMgr.sendMessage(
        sessionId,
        text || t('chat.imageAttachmentFallback'),
        sessionAgentType,
        imageContexts,
      );
      if (!isChatTargetCurrent(targetEpoch)) return;
      pollerRef.current?.nudge();
      if (wasStreaming) {
        setInfoToast(t('chat.messageQueued'));
      }
    } catch (e: any) {
      reportRemoteSessionError(e, setError);
    } finally {
      if (isChatTargetCurrent(targetEpoch)) {
        setImageAnalyzing(false);
        setOptimisticMsg(null);
      }
    }
  }, [captureChatTargetEpoch, imageAnalyzing, input, isChatTargetCurrent, isStreaming, pendingImages, sessionAgentType, sessionId, sessionMgr, setError, t]);

  const handleImageSelect = useCallback(() => {
    fileInputRef.current?.click();
  }, []);

  const handleFileChange = useCallback(async (e: React.ChangeEvent<HTMLInputElement>) => {
    const files = e.target.files;
    if (!files) return;
    const maxImages = 5;
    const remaining = maxImages - pendingImages.length;
    const toProcess = Array.from(files).slice(0, remaining);

    const { compressImageFile } = await import('../services/imageCompressor');
    for (const file of toProcess) {
      try {
        const compressed = await compressImageFile(file);
        setPendingImages((prev) => {
          if (prev.length >= maxImages) return prev;
          return [...prev, { name: compressed.name, dataUrl: compressed.dataUrl }];
        });
      } catch {
        const reader = new FileReader();
        reader.onload = () => {
          const dataUrl = reader.result as string;
          setPendingImages((prev) => {
            if (prev.length >= maxImages) return prev;
            return [...prev, { name: file.name, dataUrl }];
          });
        };
        reader.readAsDataURL(file);
      }
    }
    e.target.value = '';
  }, [pendingImages.length]);

  const removeImage = useCallback((idx: number) => {
    setPendingImages((prev) => prev.filter((_, i) => i !== idx));
  }, []);

  const expandInput = useCallback(() => {
    setInputExpanded(true);
    requestAnimationFrame(() => inputRef.current?.focus());
  }, []);

  useEffect(() => {
    if (autoFocus) {
      requestAnimationFrame(() => inputRef.current?.focus());
    }
  }, [autoFocus]);

  useEffect(() => {
    if (!inputExpanded) return;
    const handleClickOutside = (e: MouseEvent) => {
      if (inputBarRef.current && !inputBarRef.current.contains(e.target as Node)) {
        if (!input.trim() && pendingImages.length === 0) {
          setInputExpanded(false);
        }
      }
    };
    document.addEventListener('mousedown', handleClickOutside);
    return () => document.removeEventListener('mousedown', handleClickOutside);
  }, [inputExpanded, input, pendingImages.length]);

  const isComposingRef = useRef(false);

  const handleCompositionStart = useCallback(() => {
    isComposingRef.current = true;
  }, []);

  const handleCompositionEnd = useCallback(() => {
    // Delay clearing to handle Safari's event ordering where
    // compositionend fires before the final keydown(Enter)
    setTimeout(() => {
      isComposingRef.current = false;
    }, 0);
  }, []);

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter' && !e.shiftKey) {
      if ((e.nativeEvent as KeyboardEvent).isComposing || isComposingRef.current) {
        return;
      }
      e.preventDefault();
      handleSend();
    }
  };

  const handleCancel = async () => {
    const targetEpoch = captureChatTargetEpoch();
    if (targetEpoch === null || isCancelling) return;
    setIsCancelling(true);
    try {
      await sessionMgr.cancelTask(sessionId, activeTurn?.turn_id);
    } catch {
      // best effort
      if (isChatTargetCurrent(targetEpoch)) setIsCancelling(false);
    }
  };

  const workspaceName = currentWorkspace?.project_name || currentWorkspace?.path?.split('/').pop() || '';
  const gitBranch = currentWorkspace?.git_branch;
  const displayName = liveTitle || sessionName || t('chat.session');

  return (
    <div className={`chat-page${wideLayout ? ' chat-page--wide' : ''}`}>
      <ChatHeader
        deviceName={controlTarget && !controlTarget.isHome ? controlTarget.deviceName || undefined : undefined}
        displayName={displayName}
        gitBranch={gitBranch}
        isStreaming={isStreaming}
        onBack={onBack}
        onCancel={handleCancel}
        sessionId={sessionId}
        wideLayout={wideLayout}
        workspaceName={workspaceName}
      />

      {/* Messages */}
      <div className="chat-page__messages" ref={messagesContainerRef} onScroll={handleScroll}>
        {isLoadingMore && (
          <div className="chat-page__load-more-indicator">{t('chat.loadingOlderMessages')}</div>
        )}

        <ChatTranscript
          activeTurn={activeTurn}
          expandedMessageIds={expandedMsgIds}
          imageAnalyzing={imageAnalyzing}
          menuMessageId={menuMessage?.id}
          messages={messages}
          now={now}
          optimisticMessage={optimisticMsg}
          onAnswerQuestion={handleAnswerQuestion}
          onApproveTool={handleApproveTool}
          onCancelActiveTool={(toolId) => handleCancelTool(toolId, 'User cancelled')}
          onCancelLegacyTool={handleCancelTool}
          onRejectTool={handleRejectTool}
          onFileDownload={handleFileDownload}
          onGetFileInfo={handleGetFileInfo}
          onMessageContextMenu={(message, event) => {
            event.preventDefault();
            setMenuMessage(message);
          }}
          onMessageTouchEnd={handleMsgTouchEnd}
          onMessageTouchMove={handleMsgTouchMove}
          onMessageTouchStart={handleMsgTouchStart}
          onToggleMessage={(messageId, expanded) => {
            setExpandedMsgIds((previous) => {
              const next = new Set(previous);
              if (expanded) next.add(messageId);
              else next.delete(messageId);
              return next;
            });
          }}
        />

        <div ref={messagesEndRef} />

      </div>

      {showScrollToBottom && (
        <MobileIconButton
          appearance="floating"
          className="chat-page__scroll-to-bottom"
          onClick={scrollToBottom}
          aria-label={t('chat.scrollToBottom')}
          icon={<svg aria-hidden="true" focusable="false" width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
            <polyline points="6 9 12 15 18 9" />
          </svg>}
        />
      )}

      <ChatMessageActions
        deleting={deletingMsg}
        message={menuMessage}
        onClose={() => setMenuMessage(null)}
        onCopy={() => void handleCopyMessage()}
        onDelete={() => void handleDeleteMessage()}
        onResend={() => void handleResendMessage()}
      />

      {/* Floating Input Bar — two-stage (matches desktop ChatInput) */}
      <input
        ref={fileInputRef}
        type="file"
        accept="image/png,image/jpeg,image/jpg,image/gif,image/webp"
        multiple
        style={{ display: 'none' }}
        onChange={handleFileChange}
      />
      <ChatComposerBar
        cancelling={isCancelling}
        containerRef={inputBarRef}
        expanded={inputExpanded}
        imageAnalyzing={imageAnalyzing}
        input={input}
        inputRef={inputRef}
        modelControls={(
          <>
            <ModelSelectorPill catalog={modelCatalog} selectedModelId={selectedModelId} disabled={imageAnalyzing || isStreaming || modelUpdating} onSelect={handleSelectModel} />
            <ReasoningPresetPill catalog={modelCatalog} selectedModelId={selectedModelId} disabled={imageAnalyzing || isStreaming || modelUpdating} onSelect={handleSelectReasoningPreset} />
          </>
        )}
        onActivate={expandInput}
        onAttach={handleImageSelect}
        onCancel={() => void handleCancel()}
        onChange={setInput}
        onCompositionEnd={handleCompositionEnd}
        onCompositionStart={handleCompositionStart}
        onKeyDown={handleKeyDown}
        onRemoveImage={removeImage}
        onSend={handleSend}
        pendingImages={pendingImages}
        streaming={isStreaming}
      />

      <ChatFeedback
        actionMessage={actionToast}
        errorMessage={error}
        infoMessage={infoToast}
        onDismissError={() => setError(null)}
        onDismissInfo={() => setInfoToast(null)}
      />
    </div>
  );
};

export default ChatPage;
