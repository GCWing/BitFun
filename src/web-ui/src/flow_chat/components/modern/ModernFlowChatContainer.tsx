/**
 * Modern FlowChat container.
 * Uses virtual scrolling with Zustand and syncs legacy store state.
 */

import React, { useMemo, useCallback, useRef, useEffect, useLayoutEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useShortcut } from '@/infrastructure/hooks/useShortcut';
import { FlowChatManager } from '@/flow_chat/services/FlowChatManager';
import { useSessionModeStore } from '@/app/stores/sessionModeStore';
import { VirtualMessageList, VirtualMessageListRef } from './VirtualMessageList';
import { FlowChatHeader, type FlowChatHeaderTurnSummary } from './FlowChatHeader';
import { WelcomePanel } from '../WelcomePanel';
import { HistorySessionPlaceholder } from './HistorySessionPlaceholder';
import { FlowChatContext, FlowChatContextValue } from './FlowChatContext';
import { useExploreGroupState } from './useExploreGroupState';
import { useFlowChatFileActions } from './useFlowChatFileActions';
import { useFlowChatNavigation } from './useFlowChatNavigation';
import { useFlowChatCopyDialog } from './useFlowChatCopyDialog';
import { useFlowChatSync } from './useFlowChatSync';
import { useFlowChatToolActions } from './useFlowChatToolActions';
import { useFlowChatSearch } from './useFlowChatSearch';
import { useVirtualItems, useActiveSession, useVisibleTurnInfo, type VisibleTurnInfo } from '../../store/modernFlowChatStore';
import type { FlowChatConfig, FlowToolItem, Session, DialogTurn } from '../../types/flow-chat';
import type { LineRange } from '@/component-library';
import { isChatPopupActive, subscribeChatPopupChange } from '../chatPopupState';
import { useWorkspaceContext } from '@/infrastructure/contexts/WorkspaceContext';
import { parsePullRequestUrl } from '@/shared/utils/pullRequestLinks';
import { createReviewPlatformPullRequestDetailTab } from '@/shared/utils/tabUtils';
import { isAcpFlowSession } from '../../utils/acpSession';
import { flowChatStore } from '../../store/FlowChatStore';
import { openBtwSessionInAuxPane } from '../../services/openBtwSession';
import { resolveThreadGoalHeaderTitle } from '../../utils/threadGoalDisplay';
import {
  findDialogTurn,
  shouldUseStickyLatestPin,
} from '../../utils/flowChatTurnScrollPolicy';
import { startupTrace } from '@/shared/utils/startupTrace';
import { scheduleAfterStartupPaint } from '@/shared/utils/startupTaskScheduling';
import './ModernFlowChatContainer.scss';

interface ModernFlowChatContainerProps {
  className?: string;
  config?: Partial<FlowChatConfig>;

  // Callbacks compatible with the legacy version.
  onFileViewRequest?: (filePath: string, fileName: string, lineRange?: LineRange) => void;
  onTabOpen?: (tabInfo: any, sessionId?: string, panelType?: string) => void;
  onOpenVisualization?: (type: string, data: any) => void;
  onSwitchToChatPanel?: () => void;
}

type BackgroundSubagentSummary = {
  sessionId: string;
  title: string;
  agentType?: string;
  status: 'processing' | 'finishing';
  workspacePath?: string;
  remoteConnectionId?: string;
  remoteSshHost?: string;
  parentToolCallId?: string;
  subagentType?: string;
};

const LATEST_TURN_AUTO_PIN_MAX_ATTEMPTS = 8;
const HISTORY_INITIAL_CONTENT_PAINT_MAX_ATTEMPTS = 120;

function isBackgroundTaskTool(item: FlowToolItem): boolean {
  const input = item.toolCall?.input;
  if (!input || typeof input !== 'object') {
    return false;
  }

  return (input as Record<string, unknown>).run_in_background === true;
}

function readSubagentExecutionStatus(session: Session): 'processing' | 'finishing' | null {
  const latestTurn = session.dialogTurns[session.dialogTurns.length - 1];
  if (!latestTurn) {
    return null;
  }

  if (
    latestTurn.status === 'pending' ||
    latestTurn.status === 'image_analyzing' ||
    latestTurn.status === 'processing'
  ) {
    return 'processing';
  }

  if (latestTurn.status === 'finishing' || latestTurn.status === 'cancelling') {
    return 'finishing';
  }

  return null;
}

function collectRunningBackgroundSubagents(parentSessionId: string | undefined): BackgroundSubagentSummary[] {
  if (!parentSessionId) {
    return [];
  }

  const { sessions } = flowChatStore.getState();
  const parentSession = sessions.get(parentSessionId);
  if (!parentSession) {
    return [];
  }

  const backgroundTaskBySessionId = new Map<string, FlowToolItem>();
  for (const turn of parentSession.dialogTurns) {
    for (const round of turn.modelRounds) {
      for (const item of round.items) {
        if (
          item.type === 'tool' &&
          item.toolName?.toLowerCase() === 'task' &&
          item.subagentSessionId &&
          isBackgroundTaskTool(item as FlowToolItem)
        ) {
          backgroundTaskBySessionId.set(item.subagentSessionId, item as FlowToolItem);
        }
      }
    }
  }

  const results: BackgroundSubagentSummary[] = [];
  for (const session of sessions.values()) {
    if (session.sessionKind !== 'subagent' || session.parentSessionId !== parentSessionId) {
      continue;
    }

    const parentTask = backgroundTaskBySessionId.get(session.sessionId);
    if (!parentTask) {
      continue;
    }

    const status = readSubagentExecutionStatus(session);
    if (!status) {
      continue;
    }

    results.push({
      sessionId: session.sessionId,
      title: session.title?.trim() || parentTask.toolCall?.input?.description || 'Background subagent',
      agentType: session.subagentType || parentTask.toolCall?.input?.subagent_type || parentTask.toolCall?.input?.subagentType,
      status,
      workspacePath: session.workspacePath,
      remoteConnectionId: session.remoteConnectionId,
      remoteSshHost: session.remoteSshHost,
      parentToolCallId: session.parentToolCallId || parentTask.toolCall?.id || parentTask.id,
      subagentType: session.subagentType || parentTask.toolCall?.input?.subagent_type || parentTask.toolCall?.input?.subagentType,
    });
  }

  return results.sort((a, b) => {
    const aSession = sessions.get(a.sessionId);
    const bSession = sessions.get(b.sessionId);
    const createdAtDiff = (aSession?.createdAt ?? 0) - (bSession?.createdAt ?? 0);
    if (createdAtDiff !== 0) {
      return createdAtDiff;
    }

    return a.sessionId.localeCompare(b.sessionId);
  });
}

export const ModernFlowChatContainer: React.FC<ModernFlowChatContainerProps> = ({
  className = '',
  config,
  onFileViewRequest,
  onTabOpen,
  onOpenVisualization,
  onSwitchToChatPanel,
}) => {
  const { t } = useTranslation('flow-chat');
  const virtualItems = useVirtualItems();
  const activeSession = useActiveSession();
  const visibleTurnInfo = useVisibleTurnInfo();
  const [pendingHeaderTurnId, setPendingHeaderTurnId] = useState<string | null>(null);
  const [searchOpenRequest, setSearchOpenRequest] = useState(0);
  // Track whether a slash-command or @-mention popup is open in ChatInput.
  // When a popup is active, the global Escape shortcut is disabled so the
  // popup can be closed with Escape instead of cancelling the current task.
  const [chatPopupActive, setChatPopupActive] = useState(() => isChatPopupActive());

  useEffect(() => {
    return subscribeChatPopupChange(() => {
      setChatPopupActive(isChatPopupActive());
    });
  }, []);
  const [backgroundSubagents, setBackgroundSubagents] = useState<BackgroundSubagentSummary[]>([]);
  const autoPinnedTurnKeyRef = useRef<string | null>(null);
  const releasedHistoryCompletionKeyRef = useRef<string | null>(null);
  const virtualListRef = useRef<VirtualMessageListRef>(null);
  const chatScopeRef = useRef<HTMLDivElement>(null);
  const [historyInitialContentReadyKey, setHistoryInitialContentReadyKey] = useState<string | null>(null);
  const { workspacePath } = useWorkspaceContext();
  const allowUserMessageRollback = !isAcpFlowSession(activeSession);
  const historyState = activeSession?.historyState;
  const hasRestoredTurnsPendingVirtualItems =
    historyState === 'ready' &&
    (activeSession?.dialogTurns.length ?? 0) > 0 &&
    virtualItems.length === 0;
  const showHistoryPlaceholder = virtualItems.length === 0 && (
    historyState === 'metadata-only' ||
    historyState === 'hydrating' ||
    historyState === 'failed' ||
    hasRestoredTurnsPendingVirtualItems
  );
  const {
    exploreGroupStates,
    onExploreGroupToggle: handleExploreGroupToggle,
    onExpandGroup: handleExpandGroup,
    onExpandAllInTurn: handleExpandAllInTurn,
    onCollapseGroup: handleCollapseGroup,
  } = useExploreGroupState(virtualItems);
  const { handleToolConfirm, handleToolReject } = useFlowChatToolActions();

  const { handleFileViewRequest } = useFlowChatFileActions({
    workspacePath,
    onFileViewRequest,
  });
  const handleHttpLinkClick = useCallback((url: string, _event: React.MouseEvent<HTMLAnchorElement>) => {
    const pullRequestTarget = parsePullRequestUrl(url);
    if (!pullRequestTarget) {
      return false;
    }

    createReviewPlatformPullRequestDetailTab({
      workspacePath: activeSession?.workspacePath || workspacePath,
      pullRequestId: pullRequestTarget.pullRequestId,
      pullRequestUrl: pullRequestTarget.webUrl,
      title: `PR #${pullRequestTarget.pullRequestId}`,
    });
    return true;
  }, [activeSession?.workspacePath, workspacePath]);
  const {
    searchQuery,
    onSearchChange,
    matches: searchMatches,
    matchIndices: searchMatchIndices,
    currentMatchIndex: searchCurrentMatchIndex,
    currentMatchVirtualIndex: searchCurrentMatchVirtualIndex,
    goToNext: handleSearchNext,
    goToPrev: handleSearchPrev,
    clearSearch,
  } = useFlowChatSearch(virtualItems);

  useFlowChatSync();
  useFlowChatCopyDialog();

  useFlowChatNavigation({
    activeSessionId: activeSession?.sessionId,
    virtualItems,
    virtualListRef,
    onExpandExploreGroup: handleExpandGroup,
  });

  const contextValue: FlowChatContextValue = useMemo(() => ({
    onFileViewRequest: handleFileViewRequest,
    onTabOpen,
    onHttpLinkClick: handleHttpLinkClick,
    onOpenVisualization,
    onSwitchToChatPanel,
    onToolConfirm: handleToolConfirm,
    onToolReject: handleToolReject,
    sessionId: activeSession?.sessionId,
    activeSessionOverride: activeSession,
    allowUserMessageRollback,
    config: {
      enableMarkdown: true,
      autoScroll: true,
      showTimestamps: false,
      maxHistoryRounds: 50,
      enableVirtualScroll: true,
      theme: 'dark',
      ...config,
    },
    exploreGroupStates,
    onExploreGroupToggle: handleExploreGroupToggle,
    onExpandGroup: handleExpandGroup,
    onExpandAllInTurn: handleExpandAllInTurn,
    onCollapseGroup: handleCollapseGroup,
    searchQuery,
    searchMatchIndices,
    searchCurrentMatchVirtualIndex,
  }), [
    handleFileViewRequest,
    onTabOpen,
    handleHttpLinkClick,
    onOpenVisualization,
    onSwitchToChatPanel,
    handleToolConfirm,
    handleToolReject,
    activeSession,
    allowUserMessageRollback,
    config,
    exploreGroupStates,
    handleExploreGroupToggle,
    handleExpandGroup,
    handleExpandAllInTurn,
    handleCollapseGroup,
    searchQuery,
    searchMatchIndices,
    searchCurrentMatchVirtualIndex,
  ]);

  const resolveLocalCommandHeaderTitle = useCallback((metadata: DialogTurn['userMessage']['metadata']) => {
    if (metadata?.localCommandKind === 'usage_report') {
      return t('usage.title');
    }
    const threadGoalTitle = resolveThreadGoalHeaderTitle(
      metadata as Record<string, unknown> | undefined
    );
    if (threadGoalTitle) {
      return threadGoalTitle;
    }
    return null;
  }, [t]);

  const turnSummaries = useMemo<FlowChatHeaderTurnSummary[]>(() => {
    return (activeSession?.dialogTurns ?? [])
      .filter(turn => !!turn.userMessage)
      .map((turn, index) => ({
        turnId: turn.id,
        turnIndex: index + 1,
        backendTurnIndex: turn.backendTurnIndex,
        title: resolveLocalCommandHeaderTitle(turn.userMessage?.metadata)
          ?? turn.userMessage?.content ?? '',
      }));
  }, [activeSession?.dialogTurns, resolveLocalCommandHeaderTitle]);
  const headerTotalTurns = activeSession?.isPartial === true
    ? Math.max(activeSession.totalTurnCount ?? turnSummaries.length, turnSummaries.length)
    : turnSummaries.length;
  const headerTurnIndexOffset = activeSession?.isPartial === true
    ? Math.max(0, headerTotalTurns - turnSummaries.length)
    : 0;
  const headerTurnSummaries = useMemo<FlowChatHeaderTurnSummary[]>(() => {
    if (headerTurnIndexOffset === 0 && activeSession?.isPartial !== true) {
      return turnSummaries;
    }
    return turnSummaries.map(turn => ({
      ...turn,
      turnIndex: typeof turn.backendTurnIndex === 'number'
        ? turn.backendTurnIndex + 1
        : turn.turnIndex + headerTurnIndexOffset,
    }));
  }, [activeSession?.isPartial, headerTurnIndexOffset, turnSummaries]);
  const headerTurnSummaryById = useMemo(() => {
    return new Map(headerTurnSummaries.map(turn => [turn.turnId, turn]));
  }, [headerTurnSummaries]);
  const latestTurnId = turnSummaries[turnSummaries.length - 1]?.turnId;
  const hasPendingHistoryCompletion = activeSession?.sessionId
    ? flowChatStore.hasPendingSessionHistoryCompletion(activeSession.sessionId)
    : false;
  const historyInitialContentKey =
    activeSession?.sessionId &&
    latestTurnId &&
    activeSession.historyState === 'ready' &&
    virtualItems.length > 0 &&
    (
      activeSession.contextRestoreState === 'pending' ||
      hasPendingHistoryCompletion
    )
      ? `${activeSession.sessionId}:${latestTurnId}:${turnSummaries.length}`
      : null;
  const showHistoryInitialContentOverlay =
    historyInitialContentKey !== null &&
    historyInitialContentReadyKey !== historyInitialContentKey;
  const blockHistoryOverlayActivation = useCallback((event: React.SyntheticEvent<HTMLElement>) => {
    if (!showHistoryInitialContentOverlay) {
      return;
    }

    event.preventDefault();
    event.stopPropagation();
  }, [showHistoryInitialContentOverlay]);
  const latestTurn = useMemo(
    () => findDialogTurn(activeSession?.dialogTurns, latestTurnId),
    [activeSession?.dialogTurns, latestTurnId],
  );
  const latestTurnUsesStickyPin = shouldUseStickyLatestPin(latestTurn);

  const navigationVisibleTurnInfo = useMemo<VisibleTurnInfo | null>(() => {
    if (!pendingHeaderTurnId) {
      if (!visibleTurnInfo) {
        return null;
      }

      const localTurn = turnSummaries.find(turn => turn.turnId === visibleTurnInfo.turnId);
      if (!localTurn) {
        return visibleTurnInfo;
      }

      return {
        ...visibleTurnInfo,
        turnIndex: localTurn.turnIndex,
        totalTurns: turnSummaries.length,
      };
    }

    const targetTurn = turnSummaries.find(turn => turn.turnId === pendingHeaderTurnId);
    if (!targetTurn) {
      return visibleTurnInfo;
    }

    return {
      turnId: targetTurn.turnId,
      turnIndex: targetTurn.turnIndex,
      totalTurns: turnSummaries.length,
      userMessage: targetTurn.title,
    };
  }, [pendingHeaderTurnId, turnSummaries, visibleTurnInfo]);
  const effectiveVisibleTurnInfo = useMemo<VisibleTurnInfo | null>(() => {
    if (!navigationVisibleTurnInfo) {
      return null;
    }

    return {
      ...navigationVisibleTurnInfo,
      turnIndex: headerTurnSummaryById.get(navigationVisibleTurnInfo.turnId)?.turnIndex
        ?? navigationVisibleTurnInfo.turnIndex + headerTurnIndexOffset,
      totalTurns: headerTotalTurns,
    };
  }, [headerTotalTurns, headerTurnIndexOffset, headerTurnSummaryById, navigationVisibleTurnInfo]);
  const canJumpToPreviousTurn = (navigationVisibleTurnInfo?.turnIndex ?? 0) > 1;
  const canJumpToNextTurn = !!navigationVisibleTurnInfo &&
    navigationVisibleTurnInfo.turnIndex > 0 &&
    navigationVisibleTurnInfo.turnIndex < turnSummaries.length;

  const currentHeaderMessage = useMemo(() => {
    const turnId = effectiveVisibleTurnInfo?.turnId;
    if (!turnId) {
      return effectiveVisibleTurnInfo?.userMessage ?? '';
    }
    const turn = activeSession?.dialogTurns.find(item => item.id === turnId);
    const localCommandTitle = resolveLocalCommandHeaderTitle(turn?.userMessage?.metadata);
    if (localCommandTitle) {
      return localCommandTitle;
    }
    return effectiveVisibleTurnInfo?.userMessage ?? '';
  }, [activeSession?.dialogTurns, effectiveVisibleTurnInfo?.turnId, effectiveVisibleTurnInfo?.userMessage, resolveLocalCommandHeaderTitle]);

  useEffect(() => {
    if (!pendingHeaderTurnId) return;

    if (visibleTurnInfo?.turnId === pendingHeaderTurnId) {
      setPendingHeaderTurnId(null);
      return;
    }

    const targetStillExists = turnSummaries.some(turn => turn.turnId === pendingHeaderTurnId);
    if (!targetStillExists) {
      setPendingHeaderTurnId(null);
    }
  }, [pendingHeaderTurnId, turnSummaries, visibleTurnInfo?.turnId]);

  useEffect(() => {
    autoPinnedTurnKeyRef.current = null;
    releasedHistoryCompletionKeyRef.current = null;
    setHistoryInitialContentReadyKey(null);
    setPendingHeaderTurnId(null);
  }, [activeSession?.sessionId]);

  useLayoutEffect(() => {
    const sessionId = activeSession?.sessionId;
    const latestTurnKey = sessionId && latestTurnId
      ? `${sessionId}:${latestTurnId}:${turnSummaries.length}`
      : null;
    if (!sessionId || !latestTurnId || autoPinnedTurnKeyRef.current === latestTurnKey) {
      return;
    }

    const resolvedLatestTurnId = latestTurnId;
    const resolvedLatestTurnKey = latestTurnKey;
    const pinMode = latestTurnUsesStickyPin
      ? 'sticky-latest'
      : null;
    const previousAnchoredLatestTurnKeyPrefix = `${sessionId}:${latestTurnId}:`;
    const hasPreviouslyAnchoredSameLatestTurn =
      autoPinnedTurnKeyRef.current?.startsWith(previousAnchoredLatestTurnKeyPrefix) === true;
    const latestTurnRenderedInViewport = virtualListRef.current?.isTurnRenderedInViewport(latestTurnId) === true;
    const shouldForceLatestAnchorAfterTurnCountChange =
      hasPreviouslyAnchoredSameLatestTurn &&
      autoPinnedTurnKeyRef.current !== resolvedLatestTurnKey;
    if (
      !shouldForceLatestAnchorAfterTurnCountChange &&
      hasPreviouslyAnchoredSameLatestTurn &&
      visibleTurnInfo?.turnId === latestTurnId &&
      latestTurnRenderedInViewport
    ) {
      autoPinnedTurnKeyRef.current = resolvedLatestTurnKey;
      startupTrace.markPhase('historical_session_latest_anchor_skipped', {
        reason: 'latest_turn_already_visible',
        mode: pinMode ?? 'bottom',
      });
      return;
    }
    if (
      hasPreviouslyAnchoredSameLatestTurn &&
      visibleTurnInfo?.turnId === latestTurnId &&
      !latestTurnRenderedInViewport
    ) {
      startupTrace.markPhase('historical_session_latest_anchor_stale_visible_info', {
        mode: pinMode ?? 'bottom',
      });
    }

    setPendingHeaderTurnId(resolvedLatestTurnId);

    let frameId: number | null = null;
    let cancelled = false;
    let attempts = 0;

    const attemptLatestViewportAnchor = () => {
      if (cancelled) {
        return;
      }

      attempts += 1;
      let accepted = false;
      const list = virtualListRef.current;

      if (pinMode) {
        accepted = list?.pinTurnToTop(resolvedLatestTurnId, {
          behavior: 'auto',
          pinMode,
        }) ?? false;
      } else if (list) {
        accepted = list.scrollToTurnEndAndClearPin(resolvedLatestTurnId);
      }

      startupTrace.markPhase('historical_session_latest_anchor_attempt', {
        accepted,
        attempt: attempts,
        mode: pinMode ?? 'bottom',
      });

      if (accepted) {
        autoPinnedTurnKeyRef.current = resolvedLatestTurnKey;
        return;
      }

      if (attempts >= LATEST_TURN_AUTO_PIN_MAX_ATTEMPTS) {
        setPendingHeaderTurnId(null);
        startupTrace.markPhase('historical_session_latest_anchor_failed', {
          attempts,
          mode: pinMode ?? 'bottom',
        });
        return;
      }

      frameId = requestAnimationFrame(attemptLatestViewportAnchor);
    };

    if (shouldForceLatestAnchorAfterTurnCountChange) {
      attemptLatestViewportAnchor();
    } else {
      frameId = requestAnimationFrame(attemptLatestViewportAnchor);
    }

    return () => {
      cancelled = true;
      if (frameId !== null) {
        cancelAnimationFrame(frameId);
      }
    };
  }, [
    activeSession?.sessionId,
    latestTurnId,
    latestTurnUsesStickyPin,
    turnSummaries.length,
    visibleTurnInfo?.turnId,
  ]);

  useEffect(() => {
    const sessionId = activeSession?.sessionId;
    if (
      !sessionId ||
      activeSession.historyState !== 'ready' ||
      (
        activeSession.contextRestoreState !== 'pending' &&
        !hasPendingHistoryCompletion
      ) ||
      !latestTurnId
    ) {
      return;
    }

    const releaseKey = `${sessionId}:${latestTurnId}:${turnSummaries.length}`;
    if (releasedHistoryCompletionKeyRef.current === releaseKey) {
      return;
    }

    let cancelled = false;
    let frameId: number | null = null;
    let cancelAfterPaint: (() => void) | null = null;
    let attempts = 0;

    const releaseAfterPaint = () => {
      if (cancelled) {
        return;
      }
      releasedHistoryCompletionKeyRef.current = releaseKey;
      const released = flowChatStore.releaseSessionHistoryCompletionAfterInitialPaint(sessionId);
      if (released) {
        startupTrace.markPhase('historical_session_initial_content_painted', {
          sessionId,
          latestTurnId,
          turnCount: turnSummaries.length,
        });
      }
    };

    const checkLatestTextVisibility = () => {
      if (cancelled) {
        return;
      }

      attempts += 1;
      if (virtualListRef.current?.isTurnTextRenderedInViewport(latestTurnId) === true) {
        setHistoryInitialContentReadyKey(releaseKey);
        cancelAfterPaint = scheduleAfterStartupPaint(releaseAfterPaint, { frameCount: 2 });
        return;
      }

      if (attempts >= HISTORY_INITIAL_CONTENT_PAINT_MAX_ATTEMPTS) {
        setHistoryInitialContentReadyKey(releaseKey);
        releasedHistoryCompletionKeyRef.current = releaseKey;
        const released = flowChatStore.releaseSessionHistoryCompletionAfterInitialPaint(sessionId);
        startupTrace.markPhase('historical_session_initial_content_paint_signal_missed', {
          sessionId,
          latestTurnId,
          attempts,
          released,
        });
        return;
      }

      frameId = requestAnimationFrame(checkLatestTextVisibility);
    };

    frameId = requestAnimationFrame(checkLatestTextVisibility);

    return () => {
      cancelled = true;
      if (frameId !== null) {
        cancelAnimationFrame(frameId);
      }
      cancelAfterPaint?.();
    };
  }, [
    activeSession?.historyState,
    activeSession?.contextRestoreState,
    activeSession?.sessionId,
    hasPendingHistoryCompletion,
    latestTurnId,
    turnSummaries.length,
  ]);

  useEffect(() => {
    if (searchCurrentMatchVirtualIndex < 0) return;
    const frameId = requestAnimationFrame(() => {
      virtualListRef.current?.scrollToIndex(searchCurrentMatchVirtualIndex);
    });
    return () => {
      cancelAnimationFrame(frameId);
    };
  }, [searchCurrentMatchVirtualIndex]);

  useEffect(() => {
    const sessionId = activeSession?.sessionId;
    const trimmedSearchQuery = searchQuery.trim();
    if (
      !sessionId ||
      activeSession.historyState !== 'ready' ||
      !hasPendingHistoryCompletion ||
      trimmedSearchQuery.length === 0
    ) {
      return;
    }

    if (latestTurnId) {
      releasedHistoryCompletionKeyRef.current = `${sessionId}:${latestTurnId}:${turnSummaries.length}`;
    }

    const released = flowChatStore.releaseSessionHistoryCompletionAfterInitialPaint(sessionId);
    if (released) {
      startupTrace.markPhase('historical_session_full_hydrate_released_for_search', {
        sessionId,
        queryLength: trimmedSearchQuery.length,
        turnCount: turnSummaries.length,
      });
    }
  }, [
    activeSession?.historyState,
    activeSession?.sessionId,
    hasPendingHistoryCompletion,
    latestTurnId,
    searchQuery,
    turnSummaries.length,
  ]);

  const handleJumpToTurn = useCallback((turnId: string) => {
    if (!turnId) return;

    const isLatestTurn = turnSummaries[turnSummaries.length - 1]?.turnId === turnId;
    const targetTurn = findDialogTurn(activeSession?.dialogTurns, turnId);
    const pinMode = isLatestTurn && shouldUseStickyLatestPin(targetTurn)
      ? 'sticky-latest'
      : 'transient';

    const accepted = virtualListRef.current?.pinTurnToTop(turnId, {
      behavior: 'smooth',
      pinMode,
    }) ?? false;

    setPendingHeaderTurnId(accepted ? turnId : null);
  }, [activeSession?.dialogTurns, turnSummaries]);

  const handleJumpToPreviousTurn = useCallback(() => {
    if (!navigationVisibleTurnInfo || navigationVisibleTurnInfo.turnIndex <= 1) return;
    const previousTurn = turnSummaries[navigationVisibleTurnInfo.turnIndex - 2];
    if (!previousTurn) return;
    handleJumpToTurn(previousTurn.turnId);
  }, [handleJumpToTurn, navigationVisibleTurnInfo, turnSummaries]);

  const handleJumpToNextTurn = useCallback(() => {
    if (!navigationVisibleTurnInfo || navigationVisibleTurnInfo.turnIndex >= turnSummaries.length) return;
    const nextTurn = turnSummaries[navigationVisibleTurnInfo.turnIndex];
    if (!nextTurn) return;
    handleJumpToTurn(nextTurn.turnId);
  }, [handleJumpToTurn, navigationVisibleTurnInfo, turnSummaries]);

  const handleRetryHistoryLoad = useCallback(() => {
    const sessionId = activeSession?.sessionId;
    if (!sessionId) return;
    void FlowChatManager.getInstance().switchChatSession(sessionId);
  }, [activeSession?.sessionId]);

  useEffect(() => {
    const syncBackgroundSubagents = () => {
      setBackgroundSubagents(collectRunningBackgroundSubagents(activeSession?.sessionId));
    };

    syncBackgroundSubagents();
    return flowChatStore.subscribe(syncBackgroundSubagents);
  }, [activeSession?.sessionId]);

  const handleOpenBackgroundSubagent = useCallback((childSessionId: string) => {
    const subagent = backgroundSubagents.find(item => item.sessionId === childSessionId);
    if (!subagent || !activeSession?.sessionId) {
      return;
    }

    openBtwSessionInAuxPane({
      childSessionId,
      parentSessionId: activeSession.sessionId,
      workspacePath: subagent.workspacePath || activeSession.workspacePath,
      sessionKind: 'subagent',
      sessionTitle: subagent.title,
      agentType: subagent.agentType,
      parentToolCallId: subagent.parentToolCallId,
      subagentType: subagent.subagentType,
      remoteConnectionId: subagent.remoteConnectionId || activeSession.remoteConnectionId,
      remoteSshHost: subagent.remoteSshHost || activeSession.remoteSshHost,
      includeInternal: true,
    });
  }, [activeSession, backgroundSubagents]);

  useShortcut(
    'chat.stopGeneration',
    { key: 'Escape', scope: 'chat', allowInInput: true },
    () => {
      void FlowChatManager.getInstance().cancelCurrentTask();
    },
    { priority: 20, enabled: !chatPopupActive, description: 'keyboard.shortcuts.chat.stopGeneration' }
  );

  useShortcut(
    'chat.newSession',
    { key: 'N', ctrl: true, scope: 'chat' },
    () => {
      void (async () => {
        try {
          useSessionModeStore.getState().setMode('code');
          await FlowChatManager.getInstance().createChatSession({}, 'agentic');
        } catch {
          /* ignore */
        }
      })();
    },
    { priority: 10, description: 'keyboard.shortcuts.chat.newSession' }
  );

  useShortcut(
    'btw-fill',
    { key: 'B', ctrl: true, alt: true, scope: 'chat', allowInInput: true },
    () => {
      const selected = (window.getSelection?.()?.toString() ?? '').trim();
      const message = selected ? `/btw Explain this:\n\n${selected}` : '/btw ';
      window.dispatchEvent(new CustomEvent('fill-chat-input', { detail: { message } }));
    },
    { priority: 20, description: 'keyboard.shortcuts.chat.btwFill' }
  );

  useShortcut(
    'chat.search',
    { key: 'F', ctrl: true, scope: 'chat', allowInInput: false },
    () => {
      setSearchOpenRequest(prev => prev + 1);
    },
    { priority: 15, description: 'keyboard.shortcuts.chat.search' }
  );

  return (
    <FlowChatContext.Provider value={contextValue}>
      <div
        ref={chatScopeRef}
        className={`modern-flowchat-container flow-chat-typography ${className}`}
        data-shortcut-scope="chat"
      >
        <FlowChatHeader
          currentTurn={effectiveVisibleTurnInfo?.turnIndex ?? 0}
          totalTurns={effectiveVisibleTurnInfo?.totalTurns ?? 0}
          currentUserMessage={currentHeaderMessage}
          visible={virtualItems.length > 0}
          sessionId={activeSession?.sessionId}
          turns={headerTurnSummaries}
          onJumpToTurn={handleJumpToTurn}
          onJumpToCurrentTurn={() => {
            const turnId = effectiveVisibleTurnInfo?.turnId;
            if (turnId) handleJumpToTurn(turnId);
          }}
          onJumpToPreviousTurn={handleJumpToPreviousTurn}
          onJumpToNextTurn={handleJumpToNextTurn}
          canJumpToPreviousTurn={canJumpToPreviousTurn}
          canJumpToNextTurn={canJumpToNextTurn}
          searchQuery={searchQuery}
          onSearchChange={onSearchChange}
          searchMatchCount={searchMatches.length}
          searchCurrentMatch={searchMatches.length > 0 ? searchCurrentMatchIndex + 1 : 0}
          onSearchNext={handleSearchNext}
          onSearchPrev={handleSearchPrev}
          onSearchClose={clearSearch}
          searchOpenRequest={searchOpenRequest}
          backgroundSubagents={backgroundSubagents}
          onOpenBackgroundSubagent={handleOpenBackgroundSubagent}
        />

        <div
          className="modern-flowchat-container__messages"
          onClickCapture={blockHistoryOverlayActivation}
          onMouseDownCapture={blockHistoryOverlayActivation}
          onPointerDownCapture={blockHistoryOverlayActivation}
        >
          {showHistoryPlaceholder ? (
            <HistorySessionPlaceholder
              state={
                historyState === 'failed'
                  ? 'failed'
                  : historyState === 'metadata-only'
                    ? 'metadata-only'
                    : 'hydrating'
              }
              onRetry={handleRetryHistoryLoad}
            />
          ) : virtualItems.length === 0 ? (
            <WelcomePanel
              key={activeSession?.sessionId ?? 'welcome'}
              sessionMode={activeSession?.mode}
              workspacePath={activeSession?.workspacePath}
              onQuickAction={(command) => {
                window.dispatchEvent(new CustomEvent('fill-chat-input', {
                  detail: { message: command }
                }));
              }}
            />
          ) : (
            <>
              <VirtualMessageList
                // Remount per session so Virtuoso does not reuse the previous
                // viewport before the new session's auto-pin settles.
                key={activeSession?.sessionId ?? 'virtual-message-list'}
                ref={virtualListRef}
              />
              {showHistoryInitialContentOverlay && (
                <div className="modern-flowchat-container__history-overlay">
                  <HistorySessionPlaceholder state="hydrating" />
                </div>
              )}
            </>
          )}
        </div>
      </div>
    </FlowChatContext.Provider>
  );
};

ModernFlowChatContainer.displayName = 'ModernFlowChatContainer';
