/**
 * Modern FlowChat container.
 * Uses virtual scrolling with Zustand and syncs legacy store state.
 */

import React, { useMemo, useCallback, useRef, useEffect, useLayoutEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useShortcut } from '@/infrastructure/hooks/useShortcut';
import { FlowChatManager } from '@/flow_chat/services/FlowChatManager';
import { useSessionModeStore } from '@/app/stores/sessionModeStore';
import {
  VirtualMessageList,
  type FlowChatTurnPinRequestStatus,
  type VirtualMessageListRef,
} from './VirtualMessageList';
import {
  FlowChatHeader,
  type FlowChatHeaderCommandSummary,
  type FlowChatHeaderSubagentSummary,
  type FlowChatHeaderTurnSummary,
} from './FlowChatHeader';
import { BackgroundCommandInputDialog } from '../background-command/BackgroundCommandInputDialog';
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
import type { FlowChatConfig, DialogTurn } from '../../types/flow-chat';
import {
  useBackgroundCommandActivityStore,
  visibleBackgroundCommandActivitiesForSession,
  type BackgroundCommandActivity,
} from '../../store/backgroundCommandActivityStore';
import {
  useBackgroundSubagentActivityStore,
  visibleBackgroundSubagentActivitiesForSession,
} from '../../store/backgroundSubagentActivityStore';
import type { LineRange } from '@/component-library';
import { isChatPopupActive, subscribeChatPopupChange } from '../chatPopupState';
import { useWorkspaceContext } from '@/infrastructure/contexts/WorkspaceContext';
import { parsePullRequestUrl } from '@/shared/utils/pullRequestLinks';
import { createBackgroundCommandOutputTab, createReviewPlatformPullRequestDetailTab } from '@/shared/utils/tabUtils';
import { isAcpFlowSession } from '../../utils/acpSession';
import { flowChatStore } from '../../store/FlowChatStore';
import { openBtwSessionInAuxPane } from '../../services/btwSessionPane';
import { resolveThreadGoalHeaderTitle } from '../../utils/threadGoalDisplay';
import {
  findDialogTurn,
  shouldUseStickyLatestPin,
  shouldUseLatestTurnFollowOutput,
} from '../../utils/flowChatTurnScrollPolicy';
import { isRemoteTraceContext, startupTrace } from '@/shared/utils/startupTrace';
import { scheduleAfterStartupPaint } from '@/shared/utils/startupTaskScheduling';
import { agentAPI } from '@/infrastructure/api/service-api/AgentAPI';
import { notificationService } from '@/shared/notification-system';
import {
  clearHistorySessionOpenTransition,
  getHistorySessionOpenTransitionSnapshot,
  hasRenderableSessionContent,
  HISTORY_SESSION_OPEN_INTENT_EVENT,
  type HistorySessionOpenIntentDetail,
} from '../../services/sessionOpenIntent';
import {
  recordHistorySessionDiagnosticEvent,
  warnHistorySessionLoadingLayerStalled,
} from '../../services/historySessionDiagnostics';
import {
  type BackgroundSubagentActivityItem,
} from '../../utils/backgroundSubagentActivity';
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

type BackgroundSubagentSummary = BackgroundSubagentActivityItem;

type BackgroundCommandSummary = {
  execSessionKey: string;
  execSessionId: number;
  title: string;
  command: string;
  status: 'running' | 'exited' | 'interrupted' | 'killed' | 'pruned' | 'failed';
  remote?: boolean;
  tty?: boolean;
  exitCode?: number;
  startedAt?: number;
  elapsedMs?: number;
  isStopping?: boolean;
};

const LATEST_TURN_AUTO_PIN_MAX_ATTEMPTS = 8;
const HISTORY_INITIAL_CONTENT_PAINT_MAX_ATTEMPTS = 30;
const HISTORY_LOADING_LAYER_STALL_WARN_MS = 800;
const HEADER_TURN_PIN_RETRY_MAX_ATTEMPTS = 120;
const MOCK_BACKGROUND_ACTIVITIES_STORAGE_KEY = 'bitfun.flowChat.mockBackgroundActivities';

const MOCK_BACKGROUND_SUBAGENTS: BackgroundSubagentSummary[] = [
  {
    sessionId: 'mock-background-subagent-review',
    parentSessionId: 'mock-parent-session',
    title: 'Reviewing auth boundary changes',
    agentType: 'ReviewSecurity',
    status: 'processing',
    createdAt: Date.now() - 36_000,
    updatedAt: Date.now() - 4_000,
  },
  {
    sessionId: 'mock-background-subagent-docs',
    parentSessionId: 'mock-parent-session',
    title: 'Preparing migration notes for command lifecycle events',
    agentType: 'GeneralPurpose',
    status: 'finishing',
    createdAt: Date.now() - 58_000,
    updatedAt: Date.now() - 6_000,
  },
];

const MOCK_BACKGROUND_COMMANDS: BackgroundCommandSummary[] = [
  {
    execSessionKey: 'mock:interactive-input',
    execSessionId: 4216,
    title: 'node interactive-test.js',
    command: 'node interactive-test.js',
    status: 'running',
    remote: false,
    tty: true,
    startedAt: Date.now() - 24_000,
    elapsedMs: 24_000,
  },
  {
    execSessionKey: 'mock:test',
    execSessionId: 4217,
    title: 'cargo test -p terminal-core lifecycle_reports_running_and_natural_exit',
    command: 'cargo test -p terminal-core lifecycle_reports_running_and_natural_exit',
    status: 'running',
    remote: false,
    tty: true,
    startedAt: Date.now() - 42_000,
    elapsedMs: 42_000,
  },
  {
    execSessionKey: 'mock:build',
    execSessionId: 4218,
    title: 'pnpm run desktop:dev -- --profile heavy-ui-check',
    command: 'pnpm run desktop:dev -- --profile heavy-ui-check',
    status: 'running',
    remote: true,
    tty: true,
    startedAt: Date.now() - 96_000,
    elapsedMs: 96_000,
  },
  {
    execSessionKey: 'mock:finished',
    execSessionId: 4219,
    title: 'node scripts/i18n-audit.mjs',
    command: 'node scripts/i18n-audit.mjs',
    status: 'exited',
    remote: false,
    tty: false,
    exitCode: 0,
    startedAt: Date.now() - 14_000,
    elapsedMs: 13_400,
  },
];

function shouldShowMockBackgroundActivities(): boolean {
  if (!import.meta.env.DEV || typeof window === 'undefined') {
    return false;
  }

  const params = new URLSearchParams(window.location.search);
  return (
    params.get('mockBackgroundActivities') === '1' ||
    window.localStorage?.getItem(MOCK_BACKGROUND_ACTIVITIES_STORAGE_KEY) === '1'
  );
}

function commandTitle(command: string): string {
  const trimmed = command.trim();
  if (!trimmed) {
    return '';
  }
  return trimmed.length > 96 ? `${trimmed.slice(0, 96)}...` : trimmed;
}

function backgroundCommandSummaryFromActivity(activity: BackgroundCommandActivity): BackgroundCommandSummary {
  const endedAt = activity.endedAtMs;
  return {
    execSessionKey: activity.execSessionKey,
    execSessionId: activity.execSessionId,
    title: commandTitle(activity.command),
    command: activity.command,
    status: activity.status,
    remote: activity.remote,
    tty: activity.tty,
    exitCode: activity.exitCode,
    startedAt: activity.startedAtMs,
    elapsedMs: (activity.status === 'running' ? Date.now() : endedAt ?? Date.now()) - activity.startedAtMs,
  };
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
  const [queuedHeaderTurnPinId, setQueuedHeaderTurnPinId] = useState<string | null>(null);
  const [pendingHistoryOpenSession, setPendingHistoryOpenSession] = useState<HistorySessionOpenIntentDetail | null>(null);
  const [searchOpenRequest, setSearchOpenRequest] = useState(0);
  // Track whether a slash-command or @-mention popup is open in ChatInput.
  // When a popup is active, the global Escape shortcut is disabled so the
  // popup can be closed with Escape instead of cancelling the current task.
  const [chatPopupActive, setChatPopupActive] = useState(() => isChatPopupActive());
  const backgroundCommandActivities = useBackgroundCommandActivityStore(state => state.activities);
  const backgroundSubagentActivities = useBackgroundSubagentActivityStore(state => state.activities);

  useEffect(() => {
    return subscribeChatPopupChange(() => {
      setChatPopupActive(isChatPopupActive());
    });
  }, []);
  const [stoppingBackgroundSubagentIds, setStoppingBackgroundSubagentIds] = useState<Set<string>>(() => new Set());
  const [stoppingBackgroundCommandIds, setStoppingBackgroundCommandIds] = useState<Set<string>>(() => new Set());
  const [backgroundCommandInputTarget, setBackgroundCommandInputTarget] = useState<FlowChatHeaderCommandSummary | null>(null);
  const [isSendingBackgroundCommandInput, setIsSendingBackgroundCommandInput] = useState(false);
  const autoPinnedTurnKeyRef = useRef<string | null>(null);
  const releasedHistoryCompletionKeyRef = useRef<string | null>(null);
  const visibleTurnInfoRef = useRef<VisibleTurnInfo | null>(visibleTurnInfo);
  const turnSummariesRef = useRef<FlowChatHeaderTurnSummary[]>([]);
  const requestHeaderTurnPinRef = useRef<((turnId: string, behavior?: ScrollBehavior) => FlowChatTurnPinRequestStatus) | null>(null);
  const virtualListRef = useRef<VirtualMessageListRef>(null);
  const chatScopeRef = useRef<HTMLDivElement>(null);
  const [historyInitialContentReadyKey, setHistoryInitialContentReadyKey] = useState<string | null>(null);
  const [historyInitialContentPostPaintKey, setHistoryInitialContentPostPaintKey] = useState<string | null>(null);
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
  const isPendingHistoryOpenActiveSession =
    pendingHistoryOpenSession !== null &&
    activeSession?.sessionId === pendingHistoryOpenSession.sessionId;
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

  useEffect(() => {
    const handleHistorySessionOpenIntent = (event: Event) => {
      const detail = (event as CustomEvent<HistorySessionOpenIntentDetail>).detail;
      if (!detail?.sessionId) {
        return;
      }

      setPendingHistoryOpenSession({
        sessionId: detail.sessionId,
        sessionTitle: detail.sessionTitle,
      });
      startupTrace.markPhase('historical_session_open_intent_overlay', {
        sessionId: detail.sessionId,
      });
    };

    window.addEventListener(HISTORY_SESSION_OPEN_INTENT_EVENT, handleHistorySessionOpenIntent);
    return () => {
      window.removeEventListener(HISTORY_SESSION_OPEN_INTENT_EVENT, handleHistorySessionOpenIntent);
    };
  }, []);

  useEffect(() => {
    if (!pendingHistoryOpenSession) {
      return;
    }

    const timeoutId = window.setTimeout(() => {
      setPendingHistoryOpenSession(current => {
        if (current?.sessionId === pendingHistoryOpenSession.sessionId) {
          clearHistorySessionOpenTransition(pendingHistoryOpenSession.sessionId);
          return null;
        }
        return current;
      });
    }, 4000);

    return () => {
      window.clearTimeout(timeoutId);
    };
  }, [pendingHistoryOpenSession]);

  useEffect(() => {
    if (!isPendingHistoryOpenActiveSession) {
      return;
    }

    if (showHistoryPlaceholder && historyState !== 'failed') {
      return;
    }

    if (historyState === 'failed') {
      clearHistorySessionOpenTransition(pendingHistoryOpenSession.sessionId);
    }
    setPendingHistoryOpenSession(null);
  }, [
    historyState,
    isPendingHistoryOpenActiveSession,
    pendingHistoryOpenSession?.sessionId,
    showHistoryPlaceholder,
  ]);

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

  const turnSummaryCacheRef = useRef<Map<string, FlowChatHeaderTurnSummary>>(new Map());

  // Clear cache on session change
  useEffect(() => {
    turnSummaryCacheRef.current.clear();
  }, [activeSession?.sessionId]);

  const turnSummaries = useMemo<FlowChatHeaderTurnSummary[]>(() => {
    const cache = turnSummaryCacheRef.current;
    const turns = activeSession?.dialogTurns ?? [];
    const result: FlowChatHeaderTurnSummary[] = [];
    for (let i = 0; i < turns.length; i++) {
      const turn = turns[i];
      if (!turn.userMessage) continue;
      const cached = cache.get(turn.id);
      if (cached) {
        result.push({ ...cached, turnIndex: result.length + 1 });
        continue;
      }
      const summary: FlowChatHeaderTurnSummary = {
        turnId: turn.id,
        turnIndex: result.length + 1,
        backendTurnIndex: turn.backendTurnIndex,
        title: resolveLocalCommandHeaderTitle(turn.userMessage?.metadata)
          ?? turn.userMessage?.content ?? '',
      };
      cache.set(turn.id, summary);
      result.push(summary);
    }
    return result;
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
  const hasDeferredHistoryProjection = activeSession?.sessionId
    ? flowChatStore.hasDeferredSessionHistoryProjection(activeSession.sessionId)
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
      ? `${activeSession.sessionId}:${latestTurnId}`
      : null;
  const shouldBlockHistoryInitialContentInteraction =
    historyInitialContentKey !== null &&
    historyInitialContentReadyKey !== historyInitialContentKey;
  const shouldDeferBackgroundCommandSnapshot =
    activeSession?.historyState === 'metadata-only' ||
    activeSession?.historyState === 'hydrating' ||
    (
      historyInitialContentKey !== null &&
      historyInitialContentPostPaintKey !== historyInitialContentKey
    );
  const shouldScheduleBackgroundCommandSnapshotAfterPaint =
    historyInitialContentKey !== null &&
    historyInitialContentPostPaintKey === historyInitialContentKey;
  const showFailedHistoryPlaceholder =
    showHistoryPlaceholder && historyState === 'failed';
  const showHistoryOpenIntentOverlay =
    pendingHistoryOpenSession !== null &&
    (
      activeSession?.sessionId !== pendingHistoryOpenSession.sessionId ||
      (isPendingHistoryOpenActiveSession && showHistoryPlaceholder && !showFailedHistoryPlaceholder)
    );
  const shouldBlockHistoryTransitionInteraction =
    shouldBlockHistoryInitialContentInteraction ||
    showHistoryOpenIntentOverlay;
  const showHistoryLoadingLayer =
    !showHistoryOpenIntentOverlay && !showFailedHistoryPlaceholder && showHistoryPlaceholder;
  useEffect(() => {
    if (!showHistoryLoadingLayer || !activeSession?.sessionId) {
      return;
    }

    const sessionId = activeSession.sessionId;
    recordHistorySessionDiagnosticEvent(sessionId, 'loading_layer_entered', {
      historyState,
      isHistorical: activeSession.isHistorical === true,
      isRemote: isRemoteTraceContext(activeSession.remoteConnectionId, activeSession.remoteSshHost),
      hasRenderableContent: hasRenderableSessionContent(activeSession),
      dialogTurnCount: activeSession.dialogTurns.length,
    });

    const timeoutId = window.setTimeout(() => {
      const latestState = flowChatStore.getState();
      const latestSession = latestState.sessions.get(sessionId) ?? activeSession;
      const activeSessionIdMatches = latestState.activeSessionId
        ? latestState.activeSessionId === sessionId
        : activeSession.sessionId === sessionId;

      warnHistorySessionLoadingLayerStalled(sessionId, {
        durationMs: HISTORY_LOADING_LAYER_STALL_WARN_MS,
        historyState: latestSession.historyState,
        isHistorical: latestSession.isHistorical === true,
        isRemote: isRemoteTraceContext(latestSession.remoteConnectionId, latestSession.remoteSshHost),
        activeSessionIdMatches,
        hasRenderableContent: hasRenderableSessionContent(latestSession),
        dialogTurnCount: latestSession.dialogTurns.length,
        hasPendingHistoryCompletion,
        hasDeferredHistoryProjection,
      });
    }, HISTORY_LOADING_LAYER_STALL_WARN_MS);

    return () => {
      window.clearTimeout(timeoutId);
      recordHistorySessionDiagnosticEvent(sessionId, 'loading_layer_exited', {
        historyState,
      });
    };
  }, [
    activeSession,
    hasDeferredHistoryProjection,
    hasPendingHistoryCompletion,
    historyState,
    showHistoryLoadingLayer,
  ]);
  const blockHistoryOverlayActivation = useCallback((event: React.SyntheticEvent<HTMLElement>) => {
    if (!showHistoryLoadingLayer && !shouldBlockHistoryTransitionInteraction) {
      return;
    }

    event.preventDefault();
    event.stopPropagation();
  }, [shouldBlockHistoryTransitionInteraction, showHistoryLoadingLayer]);
  const latestTurn = useMemo(
    () => findDialogTurn(activeSession?.dialogTurns, latestTurnId),
    [activeSession?.dialogTurns, latestTurnId],
  );
  const latestTurnUsesFollowOutput = shouldUseLatestTurnFollowOutput(latestTurn);
  const latestTurnUsesStickyPin = shouldUseStickyLatestPin(latestTurn);

  const navigationVisibleTurnInfo = useMemo<VisibleTurnInfo | null>(() => {
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
  }, [turnSummaries, visibleTurnInfo]);
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

  useEffect(() => {
    visibleTurnInfoRef.current = visibleTurnInfo;
  }, [visibleTurnInfo]);

  useEffect(() => {
    turnSummariesRef.current = turnSummaries;
  }, [turnSummaries]);

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

  const requestHeaderTurnPin = useCallback((turnId: string, behavior: ScrollBehavior = 'smooth'): FlowChatTurnPinRequestStatus => {
    const isLatestTurn = turnSummaries[turnSummaries.length - 1]?.turnId === turnId;
    const targetTurn = findDialogTurn(activeSession?.dialogTurns, turnId);
    const pinMode = isLatestTurn && shouldUseStickyLatestPin(targetTurn)
      ? 'sticky-latest'
      : 'transient';

    return virtualListRef.current?.pinTurnToTopWithStatus(turnId, {
      behavior,
      pinMode,
    }) ?? 'rejected';
  }, [activeSession?.dialogTurns, turnSummaries]);
  useEffect(() => {
    requestHeaderTurnPinRef.current = requestHeaderTurnPin;
  }, [requestHeaderTurnPin]);
  const handleVirtualListUserScrollIntent = useCallback(() => {
    setQueuedHeaderTurnPinId(null);
    setPendingHeaderTurnId(null);
  }, []);

  useEffect(() => {
    if (!queuedHeaderTurnPinId) return;

    let cancelled = false;
    let frameId: number | null = null;
    let attempts = 0;

    const retry = () => {
      if (cancelled) return;

      if (visibleTurnInfoRef.current?.turnId === queuedHeaderTurnPinId) {
        setQueuedHeaderTurnPinId(null);
        setPendingHeaderTurnId(null);
        return;
      }

      const targetStillExists = turnSummariesRef.current.some(turn => turn.turnId === queuedHeaderTurnPinId);
      if (!targetStillExists) {
        setQueuedHeaderTurnPinId(null);
        setPendingHeaderTurnId(null);
        return;
      }

      const pinStatus = requestHeaderTurnPinRef.current?.(queuedHeaderTurnPinId, 'auto') ?? 'rejected';
      if (pinStatus === 'settled' || pinStatus === 'pending') {
        setQueuedHeaderTurnPinId(null);
        setPendingHeaderTurnId(null);
        return;
      }

      attempts += 1;
      if (attempts >= HEADER_TURN_PIN_RETRY_MAX_ATTEMPTS) {
        setQueuedHeaderTurnPinId(null);
        setPendingHeaderTurnId(null);
        return;
      }

      frameId = requestAnimationFrame(retry);
    };

    frameId = requestAnimationFrame(retry);

    return () => {
      cancelled = true;
      if (frameId !== null) {
        cancelAnimationFrame(frameId);
      }
    };
  }, [
    queuedHeaderTurnPinId,
  ]);

  useLayoutEffect(() => {
    autoPinnedTurnKeyRef.current = null;
    releasedHistoryCompletionKeyRef.current = null;
  }, [activeSession?.sessionId]);

  useEffect(() => {
    setHistoryInitialContentReadyKey(null);
    setHistoryInitialContentPostPaintKey(null);
    setPendingHeaderTurnId(null);
    setQueuedHeaderTurnPinId(null);
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
    if (latestTurnUsesFollowOutput) {
      autoPinnedTurnKeyRef.current = resolvedLatestTurnKey;
      setPendingHeaderTurnId(null);
      startupTrace.markPhase('historical_session_latest_anchor_skipped', {
        sessionId,
        latestTurnId,
        reason: 'streaming_follow_output',
        mode: pinMode ?? 'follow-output',
        turnCount: turnSummaries.length,
      });
      return;
    }
    const previousAnchoredLatestTurnKeyPrefix = `${sessionId}:${latestTurnId}:`;
    const hasPreviouslyAnchoredSameLatestTurn =
      autoPinnedTurnKeyRef.current?.startsWith(previousAnchoredLatestTurnKeyPrefix) === true;
    const latestTurnRenderedInViewport = virtualListRef.current?.isTurnRenderedInViewport(latestTurnId) === true;
    const sameLatestTurnCountChanged =
      hasPreviouslyAnchoredSameLatestTurn &&
      autoPinnedTurnKeyRef.current !== resolvedLatestTurnKey;
    const shouldSkipLocalFullHistoryReanchor =
      sameLatestTurnCountChanged &&
      !isRemoteTraceContext(activeSession.remoteConnectionId, activeSession.remoteSshHost);
    const shouldForceLatestAnchorAfterTurnCountChange =
      sameLatestTurnCountChanged &&
      !shouldSkipLocalFullHistoryReanchor;
    if (shouldSkipLocalFullHistoryReanchor) {
      autoPinnedTurnKeyRef.current = resolvedLatestTurnKey;
      startupTrace.markPhase('historical_session_latest_anchor_skipped', {
        sessionId,
        latestTurnId,
        reason: 'local_full_history_projection',
        mode: pinMode ?? 'bottom',
        turnCount: turnSummaries.length,
      });
      return;
    }
    if (
      !shouldForceLatestAnchorAfterTurnCountChange &&
      hasPreviouslyAnchoredSameLatestTurn &&
      visibleTurnInfo?.turnId === latestTurnId &&
      latestTurnRenderedInViewport
    ) {
      autoPinnedTurnKeyRef.current = resolvedLatestTurnKey;
      startupTrace.markPhase('historical_session_latest_anchor_skipped', {
        sessionId,
        latestTurnId,
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
        sessionId,
        latestTurnId,
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
        sessionId,
        latestTurnId: resolvedLatestTurnId,
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
          sessionId,
          latestTurnId: resolvedLatestTurnId,
          attempts,
          mode: pinMode ?? 'bottom',
        });
        return;
      }

      frameId = requestAnimationFrame(attemptLatestViewportAnchor);
    };

    const shouldAttemptLatestAnchorImmediately =
      shouldForceLatestAnchorAfterTurnCountChange ||
      activeSession?.isHistorical === true ||
      activeSession?.contextRestoreState === 'pending' ||
      hasPendingHistoryCompletion;

    if (shouldAttemptLatestAnchorImmediately) {
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
    activeSession?.isHistorical,
    activeSession?.contextRestoreState,
    activeSession?.remoteConnectionId,
    activeSession?.remoteSshHost,
    hasPendingHistoryCompletion,
    latestTurnId,
    latestTurnUsesFollowOutput,
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

    const releaseKey = `${sessionId}:${latestTurnId}`;
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
      clearHistorySessionOpenTransition(sessionId);
      startupTrace.markPhase('historical_session_initial_content_painted', {
        sessionId,
        latestTurnId,
        released,
        turnCount: turnSummaries.length,
      });
      setHistoryInitialContentPostPaintKey(releaseKey);
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
        setHistoryInitialContentPostPaintKey(releaseKey);
        releasedHistoryCompletionKeyRef.current = releaseKey;
        clearHistorySessionOpenTransition(sessionId);
        startupTrace.markPhase('historical_session_initial_content_paint_signal_missed', {
          sessionId,
          latestTurnId,
          attempts,
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

  const handleJumpToTurn = useCallback((turnId: string) => {
    if (!turnId) return false;

    const targetStillExists = turnSummaries.some(turn => turn.turnId === turnId);
    if (!targetStillExists) {
      setQueuedHeaderTurnPinId(null);
      setPendingHeaderTurnId(null);
      return false;
    }

    const pinStatus = requestHeaderTurnPin(turnId);
    if (pinStatus === 'settled') {
      setQueuedHeaderTurnPinId(null);
      setPendingHeaderTurnId(null);
      return true;
    }

    if (pinStatus === 'pending') {
      setQueuedHeaderTurnPinId(null);
      setPendingHeaderTurnId(null);
      return false;
    }

    setQueuedHeaderTurnPinId(turnId);
    setPendingHeaderTurnId(null);
    return false;
  }, [requestHeaderTurnPin, turnSummaries]);

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

  const activeSessionIdRef = useRef<string | null>(null);

  useEffect(() => {
    activeSessionIdRef.current = activeSession?.sessionId ?? null;
  }, [activeSession?.sessionId]);

  useEffect(() => {
    if (!activeSession?.sessionId) {
      return;
    }

    useBackgroundSubagentActivityStore
      .getState()
      .reconcileParent(flowChatStore.getState(), activeSession.sessionId);
  }, [activeSession?.dialogTurns.length, activeSession?.historyState, activeSession?.sessionId]);

  useEffect(() => {
    const agentSessionId = activeSession?.sessionId;
    if (!agentSessionId || shouldDeferBackgroundCommandSnapshot) {
      return;
    }

    let cancelled = false;
    let cancelScheduledSnapshot: (() => void) | null = null;
    const recoverSnapshot = () => {
      const pendingHistoryTransition = getHistorySessionOpenTransitionSnapshot();
      if (
        cancelled ||
        activeSessionIdRef.current !== agentSessionId ||
        (pendingHistoryTransition && pendingHistoryTransition.sessionId !== agentSessionId)
      ) {
        return;
      }

      void agentAPI.listBackgroundCommandActivities({ agentSessionId })
        .then((response) => {
          const currentHistoryTransition = getHistorySessionOpenTransitionSnapshot();
          if (
            !cancelled &&
            activeSessionIdRef.current === agentSessionId &&
            (!currentHistoryTransition || currentHistoryTransition.sessionId === agentSessionId)
          ) {
            useBackgroundCommandActivityStore
              .getState()
              .hydrateActivities(agentSessionId, response.activities);
          }
        })
        .catch(() => {
          /* Snapshot recovery is best-effort; live events remain authoritative. */
        });
    };

    if (shouldScheduleBackgroundCommandSnapshotAfterPaint) {
      cancelScheduledSnapshot = scheduleAfterStartupPaint(recoverSnapshot, { frameCount: 2 });
    } else {
      recoverSnapshot();
    }

    return () => {
      cancelled = true;
      cancelScheduledSnapshot?.();
    };
  }, [
    activeSession?.sessionId,
    shouldScheduleBackgroundCommandSnapshotAfterPaint,
    shouldDeferBackgroundCommandSnapshot,
  ]);

  const backgroundCommands = useMemo(
    () => visibleBackgroundCommandActivitiesForSession(
      backgroundCommandActivities,
      activeSession?.sessionId,
    ).map(backgroundCommandSummaryFromActivity),
    [activeSession?.sessionId, backgroundCommandActivities],
  );
  const backgroundSubagents = useMemo(
    () => visibleBackgroundSubagentActivitiesForSession(
      backgroundSubagentActivities,
      activeSession?.sessionId,
    ),
    [activeSession?.sessionId, backgroundSubagentActivities],
  );

  useEffect(() => {
    if (stoppingBackgroundCommandIds.size === 0) {
      return;
    }

    const runningCommandIds = new Set(
      backgroundCommands
        .filter(command => command.status === 'running')
        .map(command => command.execSessionKey),
    );
    if (import.meta.env.DEV && shouldShowMockBackgroundActivities()) {
      for (const command of MOCK_BACKGROUND_COMMANDS) {
        if (command.status === 'running') {
          runningCommandIds.add(command.execSessionKey);
        }
      }
    }
    setStoppingBackgroundCommandIds((previous) => {
      const next = new Set([...previous].filter(commandKey => runningCommandIds.has(commandKey)));
      return next.size === previous.size ? previous : next;
    });
  }, [backgroundCommands, stoppingBackgroundCommandIds.size]);

  useEffect(() => {
    if (stoppingBackgroundSubagentIds.size === 0) {
      return;
    }

    const runningSubagentIds = new Set(backgroundSubagents.map(subagent => subagent.sessionId));
    if (import.meta.env.DEV && shouldShowMockBackgroundActivities()) {
      for (const subagent of MOCK_BACKGROUND_SUBAGENTS) {
        runningSubagentIds.add(subagent.sessionId);
      }
    }

    setStoppingBackgroundSubagentIds((previous) => {
      const next = new Set([...previous].filter(sessionId => runningSubagentIds.has(sessionId)));
      return next.size === previous.size ? previous : next;
    });
  }, [backgroundSubagents, stoppingBackgroundSubagentIds.size]);

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

  const handleStopBackgroundSubagent = useCallback(async (subagent: FlowChatHeaderSubagentSummary) => {
    if (stoppingBackgroundSubagentIds.has(subagent.sessionId)) {
      return;
    }

    setStoppingBackgroundSubagentIds((previous) => new Set(previous).add(subagent.sessionId));

    if (import.meta.env.DEV && subagent.sessionId.startsWith('mock-background-subagent-')) {
      window.setTimeout(() => {
        setStoppingBackgroundSubagentIds((previous) => {
          const next = new Set(previous);
          next.delete(subagent.sessionId);
          return next;
        });
      }, 1200);
      return;
    }

    try {
      await agentAPI.cancelSession(subagent.sessionId);
    } catch (_error) {
      setStoppingBackgroundSubagentIds((previous) => {
        const next = new Set(previous);
        next.delete(subagent.sessionId);
        return next;
      });
      notificationService.error(
        t('flowChatHeader.backgroundSubagentStopFailed'),
        { duration: 5000 },
      );
    }
  }, [stoppingBackgroundSubagentIds, t]);

  const handleOpenBackgroundCommandOutput = useCallback((command: FlowChatHeaderCommandSummary) => {
    createBackgroundCommandOutputTab({
      execSessionKey: command.execSessionKey,
      execSessionId: command.execSessionId,
      remote: command.remote === true,
      title: command.title || t('backgroundCommandOutput.title'),
      command: command.command,
      mockKind: import.meta.env.DEV && command.execSessionKey.startsWith('mock:')
        ? command.execSessionKey.slice('mock:'.length)
        : undefined,
    });
  }, [t]);

  const handleRequestBackgroundCommandInput = useCallback((command: FlowChatHeaderCommandSummary) => {
    if (command.status !== 'running' || command.tty !== true) {
      return;
    }
    setBackgroundCommandInputTarget(command);
  }, []);

  const handleCloseBackgroundCommandInput = useCallback(() => {
    if (isSendingBackgroundCommandInput) {
      return;
    }
    setBackgroundCommandInputTarget(null);
  }, [isSendingBackgroundCommandInput]);

  const handleSendBackgroundCommandInput = useCallback(async (
    request: { chars: string; appendEnter: boolean },
  ) => {
    const command = backgroundCommandInputTarget;
    if (!command) {
      return;
    }

    setIsSendingBackgroundCommandInput(true);
    try {
      if (import.meta.env.DEV && command.execSessionKey.startsWith('mock:')) {
        await new Promise<void>((resolve) => window.setTimeout(resolve, 350));
      } else {
        await agentAPI.sendBackgroundCommandInput({
          execSessionId: command.execSessionId,
          remote: command.remote === true,
          chars: request.chars,
          appendEnter: request.appendEnter,
        });
      }
      setBackgroundCommandInputTarget(null);
      notificationService.success(
        t('backgroundCommandInput.sendSucceeded'),
        { duration: 2500 },
      );
    } catch (_error) {
      notificationService.error(
        t('backgroundCommandInput.sendFailed'),
        { duration: 5000 },
      );
    } finally {
      setIsSendingBackgroundCommandInput(false);
    }
  }, [backgroundCommandInputTarget, t]);

  const handleStopBackgroundCommand = useCallback(async (command: FlowChatHeaderCommandSummary) => {
    if (command.status !== 'running') {
      return;
    }

    setStoppingBackgroundCommandIds((previous) => new Set(previous).add(command.execSessionKey));

    if (import.meta.env.DEV && command.execSessionKey.startsWith('mock:')) {
      window.setTimeout(() => {
        setStoppingBackgroundCommandIds((previous) => {
          const next = new Set(previous);
          next.delete(command.execSessionKey);
          return next;
        });
      }, 1200);
      return;
    }

    try {
      await agentAPI.controlBackgroundCommand({
        execSessionId: command.execSessionId,
        action: 'interrupt',
        remote: command.remote === true,
      });
    } catch (_error) {
      setStoppingBackgroundCommandIds((previous) => {
        const next = new Set(previous);
        next.delete(command.execSessionKey);
        return next;
      });
      notificationService.error(
        t('flowChatHeader.backgroundCommandStopFailed'),
        { duration: 5000 },
      );
    }
  }, [t]);

  const showMockBackgroundActivities = shouldShowMockBackgroundActivities();
  const headerBackgroundSubagents = useMemo(
    () => (showMockBackgroundActivities
      ? [...backgroundSubagents, ...MOCK_BACKGROUND_SUBAGENTS]
      : backgroundSubagents
    ).map(subagent => ({
      ...subagent,
      isStopping: stoppingBackgroundSubagentIds.has(subagent.sessionId),
    })),
    [backgroundSubagents, showMockBackgroundActivities, stoppingBackgroundSubagentIds],
  );
  const headerBackgroundCommands = useMemo(
    () => (showMockBackgroundActivities
      ? [...backgroundCommands, ...MOCK_BACKGROUND_COMMANDS]
      : backgroundCommands
    ).map(command => ({
      ...command,
      isStopping: stoppingBackgroundCommandIds.has(command.execSessionKey),
    })),
    [backgroundCommands, showMockBackgroundActivities, stoppingBackgroundCommandIds],
  );
  const handleStopAllBackgroundSubagents = useCallback(() => {
    for (const subagent of headerBackgroundSubagents) {
      if (subagent.isStopping === true) {
        continue;
      }
      void handleStopBackgroundSubagent(subagent);
    }
  }, [handleStopBackgroundSubagent, headerBackgroundSubagents]);
  const handleStopAllBackgroundCommands = useCallback(() => {
    for (const command of headerBackgroundCommands) {
      if (command.status !== 'running' || command.isStopping === true) {
        continue;
      }
      void handleStopBackgroundCommand(command);
    }
  }, [handleStopBackgroundCommand, headerBackgroundCommands]);

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

  useShortcut(
    'chat.insertNewline',
    { key: 'Enter', ctrl: true, scope: 'chat', allowInInput: true },
    () => {
      document.execCommand('insertLineBreak');
    },
    { priority: 25, description: 'keyboard.shortcuts.chat.insertNewline' }
  );

  return (
    <FlowChatContext.Provider value={contextValue}>
      <div
        ref={chatScopeRef}
        className={`modern-flowchat-container flow-chat-typography ${className}`}
        data-shortcut-scope="chat"
        data-testid="flowchat-container"
        data-session-id={activeSession?.sessionId ?? ''}
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
          backgroundSubagents={headerBackgroundSubagents}
          backgroundCommands={headerBackgroundCommands}
          onOpenBackgroundSubagent={handleOpenBackgroundSubagent}
          onStopBackgroundSubagent={handleStopBackgroundSubagent}
          onStopAllBackgroundSubagents={handleStopAllBackgroundSubagents}
          onOpenBackgroundCommandOutput={handleOpenBackgroundCommandOutput}
          onRequestBackgroundCommandInput={handleRequestBackgroundCommandInput}
          onStopBackgroundCommand={handleStopBackgroundCommand}
          onStopAllBackgroundCommands={handleStopAllBackgroundCommands}
        />

        <BackgroundCommandInputDialog
          command={backgroundCommandInputTarget}
          isSending={isSendingBackgroundCommandInput}
          onClose={handleCloseBackgroundCommandInput}
          onSend={handleSendBackgroundCommandInput}
        />

        <div
          className="modern-flowchat-container__messages"
          data-testid="flowchat-messages"
          data-active-session-id={activeSession?.sessionId ?? ''}
          data-history-state={historyState ?? 'none'}
          data-context-restore-state={activeSession?.contextRestoreState ?? 'none'}
          data-is-partial={activeSession?.isPartial === true ? 'true' : 'false'}
          data-dialog-turn-count={activeSession?.dialogTurns.length ?? 0}
          data-virtual-item-count={virtualItems.length}
          data-show-history-placeholder={showHistoryPlaceholder ? 'true' : 'false'}
          data-show-history-transition-overlay={shouldBlockHistoryTransitionInteraction ? 'true' : 'false'}
          data-show-history-loading-layer={showHistoryLoadingLayer ? 'true' : 'false'}
          data-show-history-open-intent-overlay={showHistoryOpenIntentOverlay ? 'true' : 'false'}
          data-has-pending-history-completion={hasPendingHistoryCompletion ? 'true' : 'false'}
          data-has-deferred-history-projection={hasDeferredHistoryProjection ? 'true' : 'false'}
          data-latest-turn-id={latestTurnId ?? ''}
          data-history-initial-content-ready={
            historyInitialContentKey === null || historyInitialContentReadyKey === historyInitialContentKey
              ? 'true'
              : 'false'
          }
          data-pending-history-open-session-id={pendingHistoryOpenSession?.sessionId ?? ''}
          onClickCapture={blockHistoryOverlayActivation}
          onContextMenuCapture={blockHistoryOverlayActivation}
          onMouseDownCapture={blockHistoryOverlayActivation}
          onPointerDownCapture={blockHistoryOverlayActivation}
        >
          <>
            {showFailedHistoryPlaceholder ? (
              <HistorySessionPlaceholder
                state="failed"
                onRetry={handleRetryHistoryLoad}
              />
            ) : virtualItems.length === 0 ? (
              showHistoryPlaceholder || showHistoryOpenIntentOverlay ? null : (
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
              )
            ) : (
              <>
                <VirtualMessageList
                  ref={virtualListRef}
                  onUserScrollIntent={handleVirtualListUserScrollIntent}
                />
              </>
            )}
            {showHistoryLoadingLayer && (
              <div
                className="modern-flowchat-container__history-overlay"
                role="status"
                aria-label={t('historyState.loadingTitle')}
              >
                <HistorySessionPlaceholder
                  state={historyState === 'metadata-only' ? 'metadata-only' : 'hydrating'}
                />
              </div>
            )}
            {showHistoryOpenIntentOverlay && (
              <div
                className="modern-flowchat-container__history-open-intent-shield"
                role="status"
                aria-label={t('historyState.loadingTitle')}
              >
                <span
                  className="modern-flowchat-container__history-open-intent-spinner"
                  aria-hidden="true"
                />
              </div>
            )}
          </>
        </div>
      </div>
    </FlowChatContext.Provider>
  );
};

ModernFlowChatContainer.displayName = 'ModernFlowChatContainer';
