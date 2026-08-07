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
  type FlowChatTurnNavigationStatus,
  type HistoryWindowBoundaryIntentResult,
  type HistoryWindowBoundaryIntentOptions,
  type VirtualMessageListRef,
} from './VirtualMessageList';
import {
  FlowChatHeader,
  type FlowChatHeaderCommandSummary,
} from './FlowChatHeader';
import type { SessionTreeSelection } from './SessionTreePopover';
import { FlowChatTurnRail, type FlowChatTurnRailItem } from './FlowChatTurnRail';
import { BackgroundCommandInputDialog } from '../background-command/BackgroundCommandInputDialog';
import { WelcomePanel } from '../WelcomePanel';
import { HistorySessionPlaceholder } from './HistorySessionPlaceholder';
import {
  FlowChatContext,
  FlowChatContextValue,
  FlowChatVolatileContext,
  FlowChatVolatileContextValue,
} from './FlowChatContext';
import { useExploreGroupState } from './useExploreGroupState';
import { useFlowChatFileActions } from './useFlowChatFileActions';
import { useFlowChatNavigation } from './useFlowChatNavigation';
import { useFlowChatCopyDialog } from './useFlowChatCopyDialog';
import { useFlowChatSync } from './useFlowChatSync';
import { useFlowChatToolActions } from './useFlowChatToolActions';
import { useFlowChatSearch } from './useFlowChatSearch';
import {
  sessionToVirtualItems,
  useVirtualItems,
  useActiveSession,
  useVisibleTurnInfo,
  type VisibleTurnInfo,
} from '../../store/modernFlowChatStore';
import type {
  FlowChatConfig,
  DialogTurn,
  Session,
  SessionHistoryPresentation,
} from '../../types/flow-chat';
import type { SessionHistoryWindowDirection } from '../../store/FlowChatStore';
import type { FlowChatFocusItemRequest } from '../../events/flowchatNavigation';
import {
  useBackgroundCommandActivityStore,
  visibleBackgroundCommandActivitiesForSession,
  type BackgroundCommandActivity,
} from '../../store/backgroundCommandActivityStore';
import {
  useBackgroundSubagentActivityStore,
} from '../../store/backgroundSubagentActivityStore';
import type { LineRange } from '@/component-library';
import { isChatPopupActive, subscribeChatPopupChange } from '../chatPopupState';
import { useWorkspaceContext } from '@/infrastructure/contexts/WorkspaceContext';
import { flowChatSessionConfigForCurrentWorkspace } from '@/app/utils/projectSessionWorkspace';
import { createLogger } from '@/shared/utils/logger';
import { parsePullRequestUrl } from '@/shared/utils/pullRequestLinks';
import { createBackgroundCommandOutputTab, createReviewPlatformPullRequestDetailTab } from '@/shared/utils/tabUtils';
import { isAcpFlowSession } from '../../utils/acpSession';
import { flowChatStore } from '../../store/FlowChatStore';
import { openBtwSessionInAuxPane } from '../../services/btwSessionPane';
import { resolveThreadGoalHeaderTitle } from '../../utils/threadGoalDisplay';
import { hasActiveSessionLineageDescendants } from '../../utils/sessionLineage';
import {
  findDialogTurn,
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
import './ModernFlowChatContainer.scss';
import { PermissionRequestPanel } from './PermissionRequestPanel';
import { pendingPermissionToolCallIdsForSession } from './permissionRequestRouting';
import { usePermissionRequests } from './usePermissionRequests';
import {
  buildContinuousHistoryProjection,
  canRetainContinuousHistoryProjection,
} from './continuousHistoryProjection';
import {
  canonicalSessionTurns,
  projectedSessionTurnCount,
  resolveTurnOrdinal,
} from '../../utils/flowChatTurnIdentity';

const log = createLogger('ModernFlowChatContainer');


interface ModernFlowChatContainerProps {
  className?: string;
  config?: Partial<FlowChatConfig>;
  isViewportActive?: boolean;
  permissionPanelAboveChatInput?: boolean;
  /** Host-owned replacement for the ordinary new-session WelcomePanel. */
  emptyState?: React.ReactNode;

  // Callbacks compatible with the legacy version.
  onFileViewRequest?: (filePath: string, fileName: string, lineRange?: LineRange) => void;
  onTabOpen?: (tabInfo: any, sessionId?: string, panelType?: string) => void;
  onOpenVisualization?: (type: string, data: any) => void;
  onSwitchToChatPanel?: () => void;
}

interface FlowChatTurnSummary {
  turnId: string;
  turnIndex: number;
  storageTurnIndex?: number;
}

interface FlowChatHistoryPresentationState extends SessionHistoryPresentation {
  sessionId: string;
  revision: number;
}

type FlowChatViewportIntent =
  | {
      kind: 'live-tail';
      sessionId: string;
    }
  | {
      kind: 'turn';
      sessionId: string;
      ordinal: number;
      turnId: string | null;
      source: 'canonical-tail' | 'history-range';
    };

interface QueuedTurnNavigation {
  ordinal: number;
  turnId: string | null;
}

type FlowChatHistoryBoundaryState = Record<
  SessionHistoryWindowDirection,
  'idle' | 'loading' | 'error'
>;

const IDLE_HISTORY_BOUNDARY_STATE: FlowChatHistoryBoundaryState = {
  before: 'idle',
  after: 'idle',
};

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
const TURN_PIN_RETRY_MAX_ATTEMPTS = 120;
const MOCK_BACKGROUND_COMMANDS_STORAGE_KEY = 'bitfun.flowChat.mockBackgroundCommands';

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

function shouldShowMockBackgroundCommands(): boolean {
  if (!import.meta.env.DEV || typeof window === 'undefined') {
    return false;
  }

  const params = new URLSearchParams(window.location.search);
  return (
    (params.get('mockBackgroundCommands') === '1' || params.get('mockBackgroundActivities') === '1') ||
    window.localStorage?.getItem(MOCK_BACKGROUND_COMMANDS_STORAGE_KEY) === '1'
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
  isViewportActive = true,
  permissionPanelAboveChatInput = false,
  emptyState,
  onFileViewRequest,
  onTabOpen,
  onOpenVisualization,
  onSwitchToChatPanel,
}) => {
  const { t } = useTranslation('flow-chat');
  const canonicalVirtualItems = useVirtualItems();
  const activeSession = useActiveSession();
  const [historyPresentation, setHistoryPresentation] = useState<FlowChatHistoryPresentationState | null>(null);
  const [viewportIntent, setViewportIntent] = useState<FlowChatViewportIntent | null>(null);
  const [continuousProjectionSessionId, setContinuousProjectionSessionId] = useState<string | null>(null);
  const [historyBoundaryState, setHistoryBoundaryState] = useState<FlowChatHistoryBoundaryState>(
    IDLE_HISTORY_BOUNDARY_STATE,
  );
  const historyPresentationRef = useRef<FlowChatHistoryPresentationState | null>(null);
  const viewportIntentRef = useRef<FlowChatViewportIntent | null>(null);
  const updateViewportIntent = useCallback((next: FlowChatViewportIntent | null) => {
    viewportIntentRef.current = next;
    setViewportIntent(next);
  }, []);
  const historyBoundaryRequestsRef = useRef<Record<
    SessionHistoryWindowDirection,
    Promise<HistoryWindowBoundaryIntentResult> | null
  >>({
    before: null,
    after: null,
  });
  const historyPresentationOwnerGenerationRef = useRef(0);
  const activeHistoryPresentation = historyPresentation?.sessionId === activeSession?.sessionId
    ? historyPresentation
    : null;
  const activeViewportIntent = viewportIntent?.sessionId === activeSession?.sessionId
    ? viewportIntent
    : null;
  const activeSessionKnownTurnCount = activeSession
    ? projectedSessionTurnCount(activeSession)
    : 0;
  const activeHistoryPresentationFitsSession = Boolean(
    activeHistoryPresentation
    && activeHistoryPresentation.range.endOrdinalExclusive <= activeSessionKnownTurnCount
  );
  const isShowingHistoryPresentation = Boolean(
    activeHistoryPresentation
    && activeHistoryPresentationFitsSession
    && activeViewportIntent?.kind === 'turn'
    && activeViewportIntent.source === 'history-range'
  );
  const isReadingTurnViewport = activeViewportIntent?.kind === 'turn';
  const canonicalizedHistoryPresentation = useMemo(() => {
    if (!activeSession || !activeHistoryPresentation) {
      return null;
    }

    const canonicalTurnById = new Map(
      activeSession.dialogTurns.map(turn => [turn.id, turn]),
    );
    let changed = false;
    const turns = activeHistoryPresentation.turns.map(turn => {
      const canonicalTurn = canonicalTurnById.get(turn.id);
      if (!canonicalTurn || canonicalTurn === turn) {
        return turn;
      }
      changed = true;
      return canonicalTurn;
    });
    return changed
      ? { ...activeHistoryPresentation, turns }
      : activeHistoryPresentation;
  }, [activeHistoryPresentation, activeSession]);
  const continuousHistoryPresentation = useMemo(() => {
    if (!activeSession || !canonicalizedHistoryPresentation) {
      return null;
    }
    const presentation = buildContinuousHistoryProjection(
      activeSession,
      canonicalizedHistoryPresentation,
    );
    return presentation ? {
      ...presentation,
      sessionId: canonicalizedHistoryPresentation.sessionId,
      revision: canonicalizedHistoryPresentation.revision,
    } : null;
  }, [activeSession, canonicalizedHistoryPresentation]);
  const continuousHistoryVirtualItems = useMemo(() => {
    if (!activeSession || !continuousHistoryPresentation) {
      return null;
    }
    return sessionToVirtualItems({
      ...activeSession,
      dialogTurns: continuousHistoryPresentation.turns,
    });
  }, [activeSession, continuousHistoryPresentation]);
  const continuousHistoryProjectionEligible = canRetainContinuousHistoryProjection(
    continuousHistoryPresentation,
    continuousHistoryVirtualItems?.length ?? Number.POSITIVE_INFINITY,
  );
  const isRetainingContinuousHistoryProjection = Boolean(
    activeSession
    && continuousProjectionSessionId === activeSession.sessionId
    && continuousHistoryProjectionEligible
  );
  const isRenderingContinuousHistoryProjection = Boolean(
    continuousHistoryProjectionEligible
    && (isShowingHistoryPresentation || isRetainingContinuousHistoryProjection)
  );
  const renderedHistoryPresentation = isRenderingContinuousHistoryProjection
    ? continuousHistoryPresentation
    : isShowingHistoryPresentation
      ? canonicalizedHistoryPresentation
      : null;
  const isRenderingHistoryProjection = Boolean(renderedHistoryPresentation);
  const virtualItems = useMemo(() => {
    if (!activeSession || !renderedHistoryPresentation) {
      return canonicalVirtualItems;
    }
    if (
      isRenderingContinuousHistoryProjection
      && continuousHistoryVirtualItems
    ) {
      return continuousHistoryVirtualItems;
    }
    return sessionToVirtualItems({
      ...activeSession,
      dialogTurns: renderedHistoryPresentation.turns,
    });
  }, [
    activeSession,
    canonicalVirtualItems,
    continuousHistoryVirtualItems,
    isRenderingContinuousHistoryProjection,
    renderedHistoryPresentation,
  ]);

  const {
    requests: permissionRequests,
    activeBatch: activePermissionBatch,
    respond: respondPermission,
    respondBatch: respondPermissionBatch,
  } = usePermissionRequests(activeSession?.sessionId);
  const visibleTurnInfo = useVisibleTurnInfo();
  const [queuedTurnNavigation, setQueuedTurnNavigation] = useState<QueuedTurnNavigation | null>(null);
  const [pendingHistoryOpenSession, setPendingHistoryOpenSession] = useState<HistorySessionOpenIntentDetail | null>(null);
  const [searchOpenRequest, setSearchOpenRequest] = useState(0);
  // Track whether a slash-command or @-mention popup is open in ChatInput.
  // When a popup is active, the global Escape shortcut is disabled so the
  // popup can be closed with Escape instead of cancelling the current task.
  const [chatPopupActive, setChatPopupActive] = useState(() => isChatPopupActive());
  const backgroundCommandActivities = useBackgroundCommandActivityStore(state => state.activities);

  useEffect(() => {
    return subscribeChatPopupChange(() => {
      setChatPopupActive(isChatPopupActive());
    });
  }, []);
  const [stoppingBackgroundCommandIds, setStoppingBackgroundCommandIds] = useState<Set<string>>(() => new Set());
  const [backgroundCommandInputTarget, setBackgroundCommandInputTarget] = useState<FlowChatHeaderCommandSummary | null>(null);
  const [isSendingBackgroundCommandInput, setIsSendingBackgroundCommandInput] = useState(false);
  const autoTailTurnKeyRef = useRef<string | null>(null);
  const releasedHistoryCompletionKeyRef = useRef<string | null>(null);
  const visibleTurnInfoRef = useRef<VisibleTurnInfo | null>(visibleTurnInfo);
  const turnSummariesRef = useRef<FlowChatTurnSummary[]>([]);
  const turnRailTurnIdsRef = useRef<Set<string>>(new Set());
  const requestTurnNavigationRef = useRef<((turnId: string) => FlowChatTurnNavigationStatus) | null>(null);
  const searchFullHistorySessionIdRef = useRef<string | null>(null);
  const virtualListRef = useRef<VirtualMessageListRef>(null);
  const chatScopeRef = useRef<HTMLDivElement>(null);
  const activeSessionIdRef = useRef<string | null>(null);
  const [historyInitialContentReadyKey, setHistoryInitialContentReadyKey] = useState<string | null>(null);
  const [historyInitialContentPostPaintKey, setHistoryInitialContentPostPaintKey] = useState<string | null>(null);
  const { workspacePath, activeWorkspace } = useWorkspaceContext();
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
    onSearchChange: setSearchQuery,
    matches: searchMatches,
    matchIndices: searchMatchIndices,
    currentMatchIndex: searchCurrentMatchIndex,
    currentMatchVirtualIndex: searchCurrentMatchVirtualIndex,
    goToNext: handleSearchNext,
    goToPrev: handleSearchPrev,
    clearSearch,
  } = useFlowChatSearch(virtualItems);
  const searchCurrentMatch = searchMatches[searchCurrentMatchIndex];
  const searchCurrentMatchFlowItemId = searchCurrentMatch?.flowItemId;
  const searchCurrentMatchTurnId = searchCurrentMatch?.turnId;
  const searchCurrentMatchVirtualItemIndex = searchCurrentMatch?.virtualItemIndex ?? -1;
  const searchCurrentMatchOccurrenceIndex = searchCurrentMatch?.occurrenceIndex ?? 0;
  const searchCurrentMatchExpandableKey = searchCurrentMatch?.expandableIds?.join('\u0000') ?? '';

  useFlowChatSync();
  useFlowChatCopyDialog();

  const switchToLiveTailForSession = useCallback((
    sessionId: string,
    options?: { discardRecentHistory?: boolean },
  ) => {
    historyPresentationOwnerGenerationRef.current += 1;
    const retainContinuousProjection = (
      options?.discardRecentHistory !== true
      && activeSession?.sessionId === sessionId
      && continuousHistoryProjectionEligible
    );
    if (retainContinuousProjection) {
      setContinuousProjectionSessionId(sessionId);
    } else {
      setContinuousProjectionSessionId(null);
      flowChatStore.restoreSessionTailPresentation(sessionId);
    }
    if (options?.discardRecentHistory === true) {
      historyPresentationRef.current = null;
      setHistoryPresentation(null);
    }
    updateViewportIntent({ kind: 'live-tail', sessionId });
    setHistoryBoundaryState(IDLE_HISTORY_BOUNDARY_STATE);
    setQueuedTurnNavigation(null);
  }, [activeSession?.sessionId, continuousHistoryProjectionEligible, updateViewportIntent]);

  useEffect(() => {
    historyPresentationRef.current = historyPresentation;
  }, [historyPresentation]);

  useLayoutEffect(() => {
    const sessionId = activeSession?.sessionId;
    historyPresentationOwnerGenerationRef.current += 1;
    historyPresentationRef.current = null;
    setHistoryPresentation(null);
    setContinuousProjectionSessionId(null);
    updateViewportIntent(sessionId ? { kind: 'live-tail', sessionId } : null);
    setHistoryBoundaryState(IDLE_HISTORY_BOUNDARY_STATE);
    historyBoundaryRequestsRef.current = { before: null, after: null };
    if (sessionId) {
      flowChatStore.restoreSessionTailPresentation(sessionId);
    }
  }, [activeSession?.sessionId, updateViewportIntent]);

  useEffect(() => {
    const retainedSessionId = continuousProjectionSessionId;
    if (!retainedSessionId) {
      return;
    }
    if (retainedSessionId !== activeSession?.sessionId) {
      flowChatStore.restoreSessionTailPresentation(retainedSessionId);
      setContinuousProjectionSessionId(null);
      return;
    }
    if (continuousHistoryProjectionEligible) {
      return;
    }
    if (activeViewportIntent?.kind === 'live-tail') {
      flowChatStore.restoreSessionTailPresentation(retainedSessionId);
    }
    setContinuousProjectionSessionId(null);
  }, [
    activeSession?.sessionId,
    activeViewportIntent?.kind,
    continuousHistoryProjectionEligible,
    continuousProjectionSessionId,
  ]);

  useEffect(() => {
    if (!activeHistoryPresentation || activeHistoryPresentationFitsSession) {
      return;
    }
    historyPresentationOwnerGenerationRef.current += 1;
    historyPresentationRef.current = null;
    setHistoryPresentation(null);
    setContinuousProjectionSessionId(null);
    if (
      activeViewportIntent?.kind === 'turn'
      && activeViewportIntent.source === 'history-range'
    ) {
      updateViewportIntent(activeSession?.sessionId
        ? { kind: 'live-tail', sessionId: activeSession.sessionId }
        : null);
    }
    setHistoryBoundaryState(IDLE_HISTORY_BOUNDARY_STATE);
  }, [
    activeHistoryPresentation,
    activeHistoryPresentationFitsSession,
    activeSession?.sessionId,
    activeViewportIntent,
    updateViewportIntent,
  ]);

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

  // Scalar session facts for the stable context. Depending on scalars (not the
  // session object) keeps the context value referentially stable across
  // streaming flushes, which produce a new session object ~30x/second.
  const activeSessionId = activeSession?.sessionId;
  const activeSessionWorkspacePath = activeSession?.workspacePath
    || activeSession?.config?.workspacePath;
  const activeSessionRemoteConnectionId = activeSession?.remoteConnectionId
    || activeSession?.config?.remoteConnectionId;
  const activeSessionIsHistorical = activeSession?.isHistorical === true;
  const activeSessionContextRestoreState = activeSession?.contextRestoreState;

  // Reuse the previous Set when the pending permission tool-call ids are
  // content-equal so consumers do not re-render on identity-only changes.
  const pendingPermissionToolCallIdsRef = useRef<ReadonlySet<string>>(new Set());
  const pendingPermissionToolCallIds = useMemo(() => {
    const next = pendingPermissionToolCallIdsForSession(
      permissionRequests,
      activeSessionId,
    );
    const previous = pendingPermissionToolCallIdsRef.current;
    if (previous.size === next.size) {
      let contentEqual = true;
      for (const id of next) {
        if (!previous.has(id)) {
          contentEqual = false;
          break;
        }
      }
      if (contentEqual) {
        return previous;
      }
    }
    pendingPermissionToolCallIdsRef.current = next;
    return next;
  }, [permissionRequests, activeSessionId]);

  const contextValue: FlowChatContextValue = useMemo(() => ({
    onFileViewRequest: handleFileViewRequest,
    onTabOpen,
    onHttpLinkClick: handleHttpLinkClick,
    onOpenVisualization,
    onSwitchToChatPanel,
    onToolConfirm: handleToolConfirm,
    onToolReject: handleToolReject,
    sessionId: activeSessionId,
    workspacePath: activeSessionWorkspacePath,
    remoteConnectionId: activeSessionRemoteConnectionId,
    isHistoricalSession: activeSessionIsHistorical,
    contextRestoreState: activeSessionContextRestoreState,
    allowUserMessageRollback,
    config: {
      enableMarkdown: true,
      autoScroll: true,
      showTimestamps: false,
      maxHistoryRounds: 50,
      enableVirtualScroll: true,
      ...config,
    },
    onExploreGroupToggle: handleExploreGroupToggle,
    onExpandGroup: handleExpandGroup,
    onExpandAllInTurn: handleExpandAllInTurn,
    onCollapseGroup: handleCollapseGroup,
  }), [
    handleFileViewRequest,
    onTabOpen,
    handleHttpLinkClick,
    onOpenVisualization,
    onSwitchToChatPanel,
    handleToolConfirm,
    handleToolReject,
    activeSessionId,
    activeSessionWorkspacePath,
    activeSessionRemoteConnectionId,
    activeSessionIsHistorical,
    activeSessionContextRestoreState,
    allowUserMessageRollback,
    config,
    handleExploreGroupToggle,
    handleExpandGroup,
    handleExpandAllInTurn,
    handleCollapseGroup,
  ]);

  const volatileContextValue: FlowChatVolatileContextValue = useMemo(() => ({
    pendingPermissionToolCallIds,
    exploreGroupStates,
    searchQuery,
    searchMatchIndices,
    searchCurrentMatchVirtualIndex,
  }), [
    pendingPermissionToolCallIds,
    exploreGroupStates,
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

  const turnSummaries = useMemo<FlowChatTurnSummary[]>(() => {
    if (!activeSession) {
      return [];
    }
    return canonicalSessionTurns(activeSession).map((turn, index) => ({
        turnId: turn.id,
        turnIndex: (resolveTurnOrdinal(activeSession, turn) ?? index) + 1,
        storageTurnIndex: turn.storageTurnIndex ?? turn.backendTurnIndex,
      }));
  }, [activeSession]);
  const renderedTurns = useMemo(
    () => renderedHistoryPresentation
      ? renderedHistoryPresentation.turns
      : activeSession?.dialogTurns ?? [],
    [activeSession?.dialogTurns, renderedHistoryPresentation],
  );
  const renderedTurnSummaries = useMemo<FlowChatTurnSummary[]>(() => {
    return renderedTurns
      .filter(turn => turn.userMessage?.metadata?.usageReportProvisional !== true)
      .map((turn, index) => ({
        turnId: turn.id,
        turnIndex: index + 1,
        storageTurnIndex: turn.storageTurnIndex ?? turn.backendTurnIndex,
      }));
  }, [renderedTurns]);
  const activeTurnCatalog = activeSession?.turnCatalog;
  const turnCatalog = activeTurnCatalog?.sessionId === activeSession?.sessionId
    ? activeTurnCatalog
    : undefined;
  const sessionTotalTurnCount = activeSession
    ? projectedSessionTurnCount(activeSession)
    : 0;
  const absoluteRenderedTurnSummaries = useMemo<FlowChatTurnSummary[]>(() => {
    if (renderedHistoryPresentation) {
      return renderedTurnSummaries.map((turn, index) => ({
        ...turn,
        turnIndex: renderedHistoryPresentation.range.startOrdinal + index + 1,
      }));
    }
    return renderedTurnSummaries.map(turn => ({
      ...turn,
      turnIndex: activeSession
        ? (resolveTurnOrdinal(activeSession, turn.turnId) ?? turn.turnIndex - 1) + 1
        : turn.turnIndex,
    }));
  }, [
    activeSession,
    renderedHistoryPresentation,
    renderedTurnSummaries,
  ]);
  const absoluteRenderedTurnSummaryById = useMemo(() => {
    return new Map(absoluteRenderedTurnSummaries.map(turn => [turn.turnId, turn]));
  }, [absoluteRenderedTurnSummaries]);
  const turnRailItems = useMemo<FlowChatTurnRailItem[]>(() => {
    const historyView = activeSession?.sessionId
      ? flowChatStore.getSessionHistoryViewState(activeSession.sessionId)
      : undefined;
    const loadedTurns = [
      ...(activeSession?.dialogTurns ?? []),
      ...(historyView?.loadedRanges.flatMap(range => range.turns) ?? []),
    ];
    const dialogTurnById = new Map(loadedTurns.map(turn => [turn.id, turn]));
    const loadedByStorageIndex = new Map<number, { turnId: string; content: string }>();
    const loadedByOrdinal = new Map<number, { turnId: string; content: string }>();
    for (const range of historyView?.loadedRanges ?? []) {
      range.turns.forEach((turn, index) => {
        const loaded = { turnId: turn.id, content: turn.userMessage?.content ?? '' };
        loadedByOrdinal.set(range.startOrdinal + index, loaded);
        const storageTurnIndex = turn.storageTurnIndex ?? turn.backendTurnIndex;
        if (typeof storageTurnIndex === 'number') {
          loadedByStorageIndex.set(storageTurnIndex, loaded);
        }
      });
    }
    for (const summary of absoluteRenderedTurnSummaries) {
      const loaded = {
        turnId: summary.turnId,
        content: dialogTurnById.get(summary.turnId)?.userMessage?.content ?? '',
      };
      loadedByOrdinal.set(Math.max(0, summary.turnIndex - 1), loaded);
      if (typeof summary.storageTurnIndex === 'number') {
        loadedByStorageIndex.set(summary.storageTurnIndex, loaded);
      }
    }

    const catalogEntryByOrdinal = new Map(
      (turnCatalog?.entries ?? []).map(entry => [entry.ordinal, entry]),
    );
    const itemCount = Math.max(sessionTotalTurnCount, turnCatalog?.entries.length ?? 0);
    const usedLoadedTurnIds = new Set<string>();
    const items = Array.from({ length: itemCount }, (_, ordinal): FlowChatTurnRailItem => {
      const catalogEntry = catalogEntryByOrdinal.get(ordinal);
      const storageTurnIndex = catalogEntry?.storageTurnIndex ?? ordinal;
      const catalogDialogTurn = catalogEntry?.turnId
        ? dialogTurnById.get(catalogEntry.turnId)
        : undefined;
      const loaded = loadedByStorageIndex.get(storageTurnIndex)
        ?? (catalogEntry?.turnId && catalogDialogTurn ? {
          turnId: catalogEntry.turnId,
          content: catalogDialogTurn.userMessage?.content ?? '',
        } : undefined)
        ?? loadedByOrdinal.get(ordinal);
      const turnId = loaded?.turnId ?? catalogEntry?.turnId ?? null;
      if (loaded?.turnId) {
        usedLoadedTurnIds.add(loaded.turnId);
      }
      return {
        itemKey: `storage:${storageTurnIndex}`,
        turnId,
        ordinal,
        turnIndex: ordinal + 1,
        content: loaded?.content ?? catalogEntry?.preview ?? null,
      };
    });

    for (const summary of absoluteRenderedTurnSummaries) {
      if (usedLoadedTurnIds.has(summary.turnId)) {
        continue;
      }
      items.push({
        itemKey: typeof summary.storageTurnIndex === 'number'
          ? `storage:${summary.storageTurnIndex}`
          : `live:${summary.turnId}`,
        turnId: summary.turnId,
        ordinal: Math.max(0, summary.turnIndex - 1),
        turnIndex: summary.turnIndex,
        content: dialogTurnById.get(summary.turnId)?.userMessage?.content ?? '',
      });
    }

    return items.sort((left, right) => left.turnIndex - right.turnIndex);
  }, [
    absoluteRenderedTurnSummaries,
    activeSession?.dialogTurns,
    activeSession?.sessionId,
    sessionTotalTurnCount,
    turnCatalog,
  ]);
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

  const navigationVisibleTurnInfo = useMemo<VisibleTurnInfo | null>(() => {
    if (!visibleTurnInfo) {
      return null;
    }

    const localTurn = renderedTurnSummaries.find(turn => turn.turnId === visibleTurnInfo.turnId);
    if (!localTurn) {
      return visibleTurnInfo;
    }

    return {
      ...visibleTurnInfo,
      turnIndex: localTurn.turnIndex,
      totalTurns: renderedTurnSummaries.length,
    };
  }, [renderedTurnSummaries, visibleTurnInfo]);
  const effectiveVisibleTurnInfo = useMemo<VisibleTurnInfo | null>(() => {
    if (!navigationVisibleTurnInfo) {
      return null;
    }

    return {
      ...navigationVisibleTurnInfo,
      turnIndex: absoluteRenderedTurnSummaryById.get(navigationVisibleTurnInfo.turnId)?.turnIndex
        ?? navigationVisibleTurnInfo.turnIndex,
      totalTurns: sessionTotalTurnCount,
    };
  }, [absoluteRenderedTurnSummaryById, navigationVisibleTurnInfo, sessionTotalTurnCount]);
  useEffect(() => {
    visibleTurnInfoRef.current = visibleTurnInfo;
  }, [visibleTurnInfo]);

  useEffect(() => {
    turnSummariesRef.current = renderedTurnSummaries;
  }, [renderedTurnSummaries]);

  useEffect(() => {
    turnRailTurnIdsRef.current = new Set(
      turnRailItems.flatMap(turn => turn.turnId ? [turn.turnId] : []),
    );
  }, [turnRailItems]);

  const currentHeaderMessage = useMemo(() => {
    const turnId = effectiveVisibleTurnInfo?.turnId;
    if (!turnId) {
      return effectiveVisibleTurnInfo?.userMessage ?? '';
    }
    const turn = renderedTurns.find(item => item.id === turnId);
    const localCommandTitle = resolveLocalCommandHeaderTitle(turn?.userMessage?.metadata);
    if (localCommandTitle) {
      return localCommandTitle;
    }
    return effectiveVisibleTurnInfo?.userMessage ?? '';
  }, [effectiveVisibleTurnInfo?.turnId, effectiveVisibleTurnInfo?.userMessage, renderedTurns, resolveLocalCommandHeaderTitle]);

  const requestTurnNavigation = useCallback((turnId: string): FlowChatTurnNavigationStatus => {
    if (!isViewportActive) {
      return 'rejected';
    }
    return virtualListRef.current?.navigateToTurnWithStatus(turnId, {
      behavior: 'auto',
    }) ?? 'rejected';
  }, [isViewportActive]);
  useEffect(() => {
    requestTurnNavigationRef.current = requestTurnNavigation;
  }, [requestTurnNavigation]);
  const handleVirtualListUserScrollIntent = useCallback(() => {
    setQueuedTurnNavigation(null);
  }, []);

  useEffect(() => {
    if (!isViewportActive || !queuedTurnNavigation) return;

    let cancelled = false;
    let frameId: number | null = null;
    let attempts = 0;

    const retry = () => {
      if (cancelled) return;

      const queuedTurnId = queuedTurnNavigation.turnId
        ?? turnSummariesRef.current.find(
          turn => turn.turnIndex === queuedTurnNavigation.ordinal + 1,
        )?.turnId
        ?? null;
      if (!queuedTurnId) {
        attempts += 1;
        if (attempts >= TURN_PIN_RETRY_MAX_ATTEMPTS) {
          setQueuedTurnNavigation(null);
          return;
        }
        frameId = requestAnimationFrame(retry);
        return;
      }

      if (visibleTurnInfoRef.current?.turnId === queuedTurnId) {
        setQueuedTurnNavigation(null);
        return;
      }

      if (!turnRailTurnIdsRef.current.has(queuedTurnId)) {
        setQueuedTurnNavigation(null);
        return;
      }
      const targetIsLoaded = turnSummariesRef.current.some(turn => turn.turnId === queuedTurnId);
      if (!targetIsLoaded) {
        return;
      }

      const navigationStatus = requestTurnNavigationRef.current?.(queuedTurnId) ?? 'rejected';
      if (navigationStatus === 'settled' || navigationStatus === 'pending') {
        setQueuedTurnNavigation(null);
        return;
      }

      attempts += 1;
      if (attempts >= TURN_PIN_RETRY_MAX_ATTEMPTS) {
        setQueuedTurnNavigation(null);
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
    isViewportActive,
    queuedTurnNavigation,
    renderedTurnSummaries.length,
  ]);

  useLayoutEffect(() => {
    autoTailTurnKeyRef.current = null;
    releasedHistoryCompletionKeyRef.current = null;
    searchFullHistorySessionIdRef.current = null;
  }, [activeSession?.sessionId]);

  useEffect(() => {
    setHistoryInitialContentReadyKey(null);
    setHistoryInitialContentPostPaintKey(null);
    setQueuedTurnNavigation(null);
  }, [activeSession?.sessionId]);

  useLayoutEffect(() => {
    const sessionId = activeSession?.sessionId;
    const latestTurnKey = sessionId && latestTurnId
      ? `${sessionId}:${latestTurnId}:${turnSummaries.length}`
      : null;
    if (
      !isViewportActive ||
      !sessionId ||
      isReadingTurnViewport ||
      !latestTurnId ||
      !latestTurnKey ||
      autoTailTurnKeyRef.current === latestTurnKey
    ) {
      return;
    }

    if (latestTurnUsesFollowOutput) {
      autoTailTurnKeyRef.current = latestTurnKey;
      return;
    }

    let cancelled = false;
    let frameId: number | null = null;
    let attempts = 0;
    const scrollLatestTurnToNaturalEnd = () => {
      if (cancelled) return;
      attempts += 1;
      if (virtualListRef.current?.scrollToTurnEnd(latestTurnId)) {
        autoTailTurnKeyRef.current = latestTurnKey;
        return;
      }
      if (attempts < LATEST_TURN_AUTO_PIN_MAX_ATTEMPTS) {
        frameId = requestAnimationFrame(scrollLatestTurnToNaturalEnd);
      }
    };
    frameId = requestAnimationFrame(scrollLatestTurnToNaturalEnd);
    return () => {
      cancelled = true;
      if (frameId !== null) cancelAnimationFrame(frameId);
    };
  }, [
    activeSession?.sessionId,
    isReadingTurnViewport,
    isViewportActive,
    latestTurnId,
    latestTurnUsesFollowOutput,
    turnSummaries.length,
  ]);

  useEffect(() => {
    const sessionId = activeSession?.sessionId;
    if (
      !isViewportActive ||
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
    isViewportActive,
    latestTurnId,
    turnSummaries.length,
  ]);

  useEffect(() => {
    if (searchCurrentMatchVirtualItemIndex < 0 || !searchQuery.trim()) {
      virtualListRef.current?.clearSearchMatch();
      return;
    }

    const frameId = requestAnimationFrame(() => {
      virtualListRef.current?.scrollToSearchMatch({
        virtualItemIndex: searchCurrentMatchVirtualItemIndex,
        query: searchQuery,
        flowItemId: searchCurrentMatchFlowItemId,
        occurrenceIndex: searchCurrentMatchOccurrenceIndex,
        expandableIds: searchCurrentMatchExpandableKey
          ? searchCurrentMatchExpandableKey.split('\u0000')
          : undefined,
      });
    });
    return () => {
      cancelAnimationFrame(frameId);
    };
  }, [
    searchCurrentMatchFlowItemId,
    searchCurrentMatchTurnId,
    searchCurrentMatchExpandableKey,
    searchCurrentMatchVirtualItemIndex,
    searchCurrentMatchOccurrenceIndex,
    searchQuery,
  ]);

  const applyHistoryPresentation = useCallback((
    sessionId: string,
    presentation: SessionHistoryPresentation,
    options?: {
      completedBoundary?: SessionHistoryWindowDirection;
      viewportTarget?: {
        ordinal: number;
        turnId: string | null;
      };
    },
  ) => {
    historyPresentationOwnerGenerationRef.current += 1;
    setHistoryPresentation(previous => {
      const next: FlowChatHistoryPresentationState = {
        ...presentation,
        sessionId,
        revision: (previous?.sessionId === sessionId ? previous.revision : 0) + 1,
      };
      historyPresentationRef.current = next;
      return next;
    });
    if (options?.viewportTarget) {
      updateViewportIntent({
        kind: 'turn',
        sessionId,
        ordinal: options.viewportTarget.ordinal,
        turnId: options.viewportTarget.turnId,
        source: 'history-range',
      });
    }
    const completedBoundary = options?.completedBoundary;
    if (completedBoundary) {
      setHistoryBoundaryState(previous => ({
        ...previous,
        [completedBoundary]: 'idle',
      }));
    } else {
      setHistoryBoundaryState(IDLE_HISTORY_BOUNDARY_STATE);
    }
  }, [updateViewportIntent]);

  const restoreTailPresentation = useCallback((options?: {
    followLatest?: boolean;
    discardRecentHistory?: boolean;
  }) => {
    const sessionId = activeSession?.sessionId;
    if (!sessionId) {
      return false;
    }

    switchToLiveTailForSession(sessionId, {
      discardRecentHistory: options?.discardRecentHistory,
    });

    if (options?.followLatest) {
      requestAnimationFrame(() => {
        if (activeSessionIdRef.current === sessionId) {
          virtualListRef.current?.scrollToLatestEndPosition();
        }
      });
    }
    return true;
  }, [activeSession?.sessionId, switchToLiveTailForSession]);

  const jumpToLiveTail = useCallback(() => {
    return restoreTailPresentation({ followLatest: true });
  }, [restoreTailPresentation]);

  const handleSearchChange = useCallback((query: string) => {
    setSearchQuery(query);
    const sessionId = activeSession?.sessionId;
    if (
      !query.trim()
      || !sessionId
      || activeSession.isPartial !== true
      || searchFullHistorySessionIdRef.current === sessionId
    ) {
      return;
    }

    searchFullHistorySessionIdRef.current = sessionId;
    void flowChatStore.ensureSessionFullHistory(sessionId, 'flowchat-search').then(ready => {
      if (ready && activeSessionIdRef.current === sessionId) {
        restoreTailPresentation({ discardRecentHistory: true });
      } else if (activeSessionIdRef.current === sessionId) {
        searchFullHistorySessionIdRef.current = null;
      }
    });
  }, [activeSession?.isPartial, activeSession?.sessionId, restoreTailPresentation, setSearchQuery]);

  const navigateToTurn = useCallback(async (target: FlowChatTurnRailItem | string) => {
    const targetItem = typeof target === 'string'
      ? turnRailItems.find(turn => turn.turnId === target)
      : target;
    const sessionId = activeSession?.sessionId;
    if (!targetItem || !sessionId) return false;

    const renderedTargetId = targetItem.turnId;
    const targetIsRendered = Boolean(
      renderedTargetId
      && renderedTurnSummaries.some(turn => turn.turnId === renderedTargetId),
    );
    if (renderedTargetId && targetIsRendered) {
      updateViewportIntent({
        kind: 'turn',
        sessionId,
        ordinal: targetItem.ordinal,
        turnId: renderedTargetId,
        source: isRenderingHistoryProjection ? 'history-range' : 'canonical-tail',
      });
      const navigationStatus = requestTurnNavigation(renderedTargetId);
      if (navigationStatus === 'settled' || navigationStatus === 'pending') {
        setQueuedTurnNavigation(null);
        return true;
      }
      setQueuedTurnNavigation({
        ordinal: targetItem.ordinal,
        turnId: renderedTargetId,
      });
      return true;
    }

    const recentHistoryPresentation = historyPresentationRef.current?.sessionId === sessionId
      ? historyPresentationRef.current
      : null;
    const recentHistoryTurn = recentHistoryPresentation
      ? recentHistoryPresentation.turns[
          targetItem.ordinal - recentHistoryPresentation.range.startOrdinal
        ]
      : undefined;
    const targetIsInRecentHistory = Boolean(
      recentHistoryPresentation
      && targetItem.ordinal >= recentHistoryPresentation.range.startOrdinal
      && targetItem.ordinal < recentHistoryPresentation.range.endOrdinalExclusive
      && recentHistoryTurn
      && (!targetItem.turnId || recentHistoryTurn.id === targetItem.turnId)
    );
    if (recentHistoryPresentation && targetIsInRecentHistory && recentHistoryTurn) {
      const reactivatedPresentation = flowChatStore.reactivateSessionHistoryWindow(
        sessionId,
        recentHistoryPresentation.range,
      );
      if (reactivatedPresentation) {
        const preparedNavigation = virtualListRef.current?.prepareTurnNavigation(recentHistoryTurn.id, {
          behavior: 'auto',
        }) ?? 'rejected';
        if (preparedNavigation !== 'rejected') {
          applyHistoryPresentation(sessionId, reactivatedPresentation, {
            viewportTarget: {
              ordinal: targetItem.ordinal,
              turnId: recentHistoryTurn.id,
            },
          });
          setQueuedTurnNavigation(null);
          return true;
        }
        flowChatStore.restoreSessionTailPresentation(sessionId);
      }
    }

    let result;
    try {
      result = await flowChatStore.loadSessionTurnWindow(sessionId, targetItem.ordinal, {
        source: 'target',
      });
    } catch (error) {
      log.warn('Failed to load the requested session Turn window', {
        sessionId,
        targetOrdinal: targetItem.ordinal,
        error,
      });
      return false;
    }

    if (result.status === 'ready' && result.isCurrent) {
      const targetTurnId = result.targetTurnId
        ?? result.range?.turns[result.targetOrdinal - (result.range?.startOrdinal ?? 0)]?.id
        ?? targetItem.turnId;
      if (!targetTurnId) {
        return false;
      }

      const preparedNavigation = virtualListRef.current?.prepareTurnNavigation(targetTurnId, {
        behavior: 'auto',
      }) ?? 'rejected';
      if (preparedNavigation === 'rejected') {
        return false;
      }
      const presentation = flowChatStore.activateSessionHistoryWindow(
        sessionId,
        result.targetOrdinal,
        result.navigationGeneration,
      );
      if (!presentation) {
        return false;
      }

      applyHistoryPresentation(sessionId, presentation, {
        viewportTarget: {
          ordinal: result.targetOrdinal,
          turnId: targetTurnId,
        },
      });
      setQueuedTurnNavigation(null);
      return true;
    }

    if (result.status === 'unsupported' || result.status === 'not-found') {
      const historyReady = await flowChatStore.ensureSessionFullHistory(
        sessionId,
        'turn-rail-navigation',
      );
      if (historyReady && activeSessionIdRef.current === sessionId) {
        restoreTailPresentation({ discardRecentHistory: true });
        updateViewportIntent({
          kind: 'turn',
          sessionId,
          ordinal: targetItem.ordinal,
          turnId: targetItem.turnId,
          source: 'canonical-tail',
        });
        setQueuedTurnNavigation({
          ordinal: targetItem.ordinal,
          turnId: targetItem.turnId,
        });
        return true;
      }
    }

    return false;
  }, [
    activeSession?.sessionId,
    applyHistoryPresentation,
    isRenderingHistoryProjection,
    renderedTurnSummaries,
    requestTurnNavigation,
    restoreTailPresentation,
    turnRailItems,
    updateViewportIntent,
  ]);

  const handleNavigateToFocusTurn = useCallback(async (request: FlowChatFocusItemRequest) => {
    const sessionId = activeSession?.sessionId;
    if (!sessionId || request.sessionId !== sessionId) {
      return false;
    }

    const requestedTurnId = request.turnId?.trim() || null;
    const targetById = requestedTurnId
      ? turnRailItems.find(turn => turn.turnId === requestedTurnId)
      : undefined;
    if (targetById) {
      return navigateToTurn(targetById);
    }

    if (requestedTurnId) {
      const historyReady = await flowChatStore.ensureSessionFullHistory(
        sessionId,
        'flowchat-focus-navigation',
      );
      if (!historyReady || flowChatStore.getState().activeSessionId !== sessionId) {
        return false;
      }
      const hydratedSession = flowChatStore.getState().sessions.get(sessionId);
      const hydratedTurnIndex = hydratedSession?.dialogTurns.findIndex(
        turn => turn.id === requestedTurnId,
      ) ?? -1;
      if (hydratedTurnIndex < 0) {
        return false;
      }

      restoreTailPresentation({ discardRecentHistory: true });
      updateViewportIntent({
        kind: 'turn',
        sessionId,
        ordinal: hydratedTurnIndex,
        turnId: requestedTurnId,
        source: 'canonical-tail',
      });
      setQueuedTurnNavigation({
        ordinal: hydratedTurnIndex,
        turnId: requestedTurnId,
      });
      return true;
    }

    const requestedOrdinal = typeof request.turnIndex === 'number'
      ? Math.max(0, Math.floor(request.turnIndex) - 1)
      : null;
    if (requestedOrdinal === null) {
      return false;
    }
    const targetByOrdinal = turnRailItems.find(turn => turn.ordinal === requestedOrdinal);
    return targetByOrdinal ? navigateToTurn(targetByOrdinal) : false;
  }, [
    activeSession?.sessionId,
    navigateToTurn,
    restoreTailPresentation,
    turnRailItems,
    updateViewportIntent,
  ]);

  useFlowChatNavigation({
    activeSessionId: activeSession?.sessionId,
    virtualItems,
    virtualListRef,
    onExpandExploreGroup: handleExpandGroup,
    onNavigateToFocusTurn: handleNavigateToFocusTurn,
  });

  const handleRetryHistoryLoad = useCallback(() => {
    const sessionId = activeSession?.sessionId;
    if (!sessionId) return;
    void FlowChatManager.getInstance().switchChatSession(sessionId);
  }, [activeSession?.sessionId]);

  useEffect(() => {
    activeSessionIdRef.current = activeSession?.sessionId ?? null;
  }, [activeSession?.sessionId]);

  const handleHistoryWindowBoundaryIntent = useCallback((
    direction: SessionHistoryWindowDirection,
    options?: HistoryWindowBoundaryIntentOptions,
  ): Promise<HistoryWindowBoundaryIntentResult> => {
    const existingRequest = historyBoundaryRequestsRef.current[direction];
    if (existingRequest) {
      return existingRequest;
    }

    const request = (async () => {
      const currentViewportIntent = viewportIntentRef.current;
      const presentation = currentViewportIntent?.kind === 'turn'
        && currentViewportIntent.source === 'history-range'
        ? historyPresentationRef.current
        : null;
      const sessionId = activeSessionIdRef.current;
      const presentationOwnerGeneration = historyPresentationOwnerGenerationRef.current;
      if (!sessionId || (presentation && presentation.sessionId !== sessionId)) {
        return 'cancelled';
      }

      const session = flowChatStore.getState().sessions.get(sessionId);
      const historyView = flowChatStore.getSessionHistoryViewState(sessionId);
      const totalTurnCount = Math.max(
        historyView?.catalog?.totalTurnCount ?? 0,
        session?.totalTurnCount ?? 0,
      );
      let targetOrdinal: number;
      if (presentation) {
        targetOrdinal = direction === 'before'
          ? presentation.range.startOrdinal - 1
          : presentation.range.endOrdinalExclusive;
      } else {
        if (
          direction !== 'before'
          || session?.isPartial !== true
          || session.turnCatalog?.sessionId !== sessionId
        ) {
          return 'cancelled';
        }
        const canonicalTailRange = flowChatStore.getSessionCanonicalTailRange(sessionId);
        if (!canonicalTailRange) {
          return 'not-ready';
        }
        targetOrdinal = canonicalTailRange.startOrdinal - 1;
      }
      if (targetOrdinal < 0 || targetOrdinal >= totalTurnCount) {
        return 'exhausted';
      }

      setHistoryBoundaryState(previous => ({ ...previous, [direction]: 'loading' }));
      let viewportPreparationStarted = false;
      try {
        const result = await flowChatStore.loadSessionTurnWindow(sessionId, targetOrdinal, {
          source: 'prefetch',
          before: direction === 'before' ? 12 : 4,
          after: direction === 'after' ? 12 : 1,
        });
        if (!result.isCurrent || activeSessionIdRef.current !== sessionId) {
          return 'cancelled';
        }
        if (result.status !== 'ready') {
          if (result.status === 'unsupported') {
            const historyReady = await flowChatStore.ensureSessionFullHistory(
              sessionId,
              'sequential-history-navigation',
            );
            if (historyReady && activeSessionIdRef.current === sessionId) {
              setHistoryBoundaryState(previous => ({ ...previous, [direction]: 'idle' }));
              return 'applied';
            }
          }
          setHistoryBoundaryState(previous => ({ ...previous, [direction]: 'error' }));
          return 'not-ready';
        }

        viewportPreparationStarted = Boolean(options?.prepareViewportForPresentationCommit);
        const preparationResult = await options?.prepareViewportForPresentationCommit?.();
        const activeSessionIsCurrent = activeSessionIdRef.current === sessionId;
        const presentationOwnerIsCurrent = (
          historyPresentationOwnerGenerationRef.current === presentationOwnerGeneration
        );
        if (
          preparationResult === false
          || !activeSessionIsCurrent
          || !presentationOwnerIsCurrent
        ) {
          if (viewportPreparationStarted) {
            options?.cancelViewportPresentationCommit?.();
          }
          if (activeSessionIsCurrent) {
            setHistoryBoundaryState(previous => ({ ...previous, [direction]: 'idle' }));
          }
          return 'cancelled';
        }

        const nextPresentation = presentation
          ? flowChatStore.extendSessionHistoryWindow(sessionId, direction)
          : flowChatStore.activateSessionHistoryWindowFromTail(sessionId, targetOrdinal);
        if (!nextPresentation) {
          if (viewportPreparationStarted) {
            options?.cancelViewportPresentationCommit?.();
          }
          setHistoryBoundaryState(previous => ({ ...previous, [direction]: 'error' }));
          return 'not-ready';
        }
        applyHistoryPresentation(sessionId, nextPresentation, {
          completedBoundary: direction,
          ...(!presentation ? {
            viewportTarget: {
              ordinal: targetOrdinal,
              turnId: nextPresentation.turns[
                targetOrdinal - nextPresentation.range.startOrdinal
              ]?.id ?? null,
            },
          } : {}),
        });
        return 'applied';
      } catch (error) {
        if (viewportPreparationStarted) {
          options?.cancelViewportPresentationCommit?.();
        }
        if (activeSessionIdRef.current === sessionId) {
          setHistoryBoundaryState(previous => ({ ...previous, [direction]: 'error' }));
        }
        log.warn('Failed to prefetch an adjacent session Turn window', {
          sessionId,
          direction,
          targetOrdinal,
          error,
        });
        return 'not-ready';
      }
    })().finally(() => {
      historyBoundaryRequestsRef.current[direction] = null;
    });
    historyBoundaryRequestsRef.current[direction] = request;
    return request;
  }, [applyHistoryPresentation]);

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
  const [hasActiveSessionTreeDescendants, setHasActiveSessionTreeDescendants] = useState(() =>
    hasActiveSessionLineageDescendants(activeSession?.sessionId, flowChatStore.getState().sessions),
  );

  useEffect(() => {
    const rootSessionId = activeSession?.sessionId;
    const updateActivity = (sessions: Map<string, Session>) => {
      setHasActiveSessionTreeDescendants(
        hasActiveSessionLineageDescendants(rootSessionId, sessions),
      );
    };

    updateActivity(flowChatStore.getState().sessions);
    return flowChatStore.subscribeSelector(
      state => hasActiveSessionLineageDescendants(rootSessionId, state.sessions),
      setHasActiveSessionTreeDescendants,
    );
  }, [activeSession?.sessionId]);

  useEffect(() => {
    if (stoppingBackgroundCommandIds.size === 0) {
      return;
    }

    const runningCommandIds = new Set(
      backgroundCommands
        .filter(command => command.status === 'running')
        .map(command => command.execSessionKey),
    );
    if (import.meta.env.DEV && shouldShowMockBackgroundCommands()) {
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

  const handleOpenSessionTreeSession = useCallback((selection: SessionTreeSelection) => {
    if (
      !activeSession?.sessionId ||
      selection.isRoot ||
      selection.sessionId === activeSession.sessionId ||
      !selection.parentSessionId
    ) {
      return;
    }

    openBtwSessionInAuxPane({
      childSessionId: selection.sessionId,
      parentSessionId: selection.parentSessionId,
      workspacePath: selection.workspacePath || activeSession.workspacePath,
      sessionKind: 'subagent',
      sessionTitle: selection.title,
      agentType: selection.agentType,
      parentToolCallId: selection.parentToolCallId,
      subagentType: selection.subagentType,
      remoteConnectionId: selection.remoteConnectionId || activeSession.remoteConnectionId,
      remoteSshHost: selection.remoteSshHost || activeSession.remoteSshHost,
      includeInternal: true,
    });
  }, [activeSession]);

  const handleCancelSessionTreeSession = useCallback(async (selection: SessionTreeSelection) => {
    try {
      const result = await agentAPI.cancelSession(selection.sessionId, { cancelDescendants: false });
      if (!result.cancelled) {
        notificationService.error(
          t('flowChatHeader.agentTreeCancelFailed'),
          { duration: 5000 },
        );
      }
      return result.cancelled;
    } catch (_error) {
      notificationService.error(
        t('flowChatHeader.agentTreeCancelFailed'),
        { duration: 5000 },
      );
      return false;
    }
  }, [t]);

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

  const showMockBackgroundCommands = shouldShowMockBackgroundCommands();
  const headerBackgroundCommands = useMemo(
    () => (showMockBackgroundCommands
      ? [...backgroundCommands, ...MOCK_BACKGROUND_COMMANDS]
      : backgroundCommands
    ).map(command => ({
      ...command,
      isStopping: stoppingBackgroundCommandIds.has(command.execSessionKey),
    })),
    [backgroundCommands, showMockBackgroundCommands, stoppingBackgroundCommandIds],
  );
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
          await FlowChatManager.getInstance().createChatSession(
            flowChatSessionConfigForCurrentWorkspace(activeWorkspace),
            'agentic',
          );
        } catch (error) {
          log.error('Failed to create session from shortcut', { error });
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
      <FlowChatVolatileContext.Provider value={volatileContextValue}>
      <div
        ref={chatScopeRef}
        className={`modern-flowchat-container flow-chat-typography ${className}`}
        data-shortcut-scope="chat"
        data-testid="flowchat-container"
        data-session-id={activeSession?.sessionId ?? ''}
        data-bf-component="modern-flow-chat"
        data-bf-part="root"
      >
        <FlowChatHeader
          currentTurn={effectiveVisibleTurnInfo?.turnIndex ?? 0}
          totalTurns={effectiveVisibleTurnInfo?.totalTurns ?? 0}
          currentUserMessage={currentHeaderMessage}
          visible={virtualItems.length > 0}
          sessionId={activeSession?.sessionId}
          onJumpToCurrentTurn={() => {
            const turnId = effectiveVisibleTurnInfo?.turnId;
            if (turnId) navigateToTurn(turnId);
          }}
          searchQuery={searchQuery}
          onSearchChange={handleSearchChange}
          searchMatchCount={searchMatches.length}
          searchCurrentMatch={searchMatches.length > 0 ? searchCurrentMatchIndex + 1 : 0}
          onSearchNext={handleSearchNext}
          onSearchPrev={handleSearchPrev}
          onSearchClose={clearSearch}
          searchOpenRequest={searchOpenRequest}
          backgroundCommands={headerBackgroundCommands}
          onOpenSessionTreeSession={handleOpenSessionTreeSession}
          hasActiveSessionTreeDescendants={hasActiveSessionTreeDescendants}
          onCancelSessionTreeSession={handleCancelSessionTreeSession}
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

        {activePermissionBatch && (
          <PermissionRequestPanel
            key={`${activePermissionBatch.sessionId}:${activePermissionBatch.roundId}`}
            requests={activePermissionBatch.requests}
            totalPendingCount={permissionRequests.length}
            aboveChatInput={permissionPanelAboveChatInput}
            onRespond={respondPermission}
            onRespondBatch={respondPermissionBatch}
          />
        )}

        <div
          className="modern-flowchat-container__messages"
          data-testid="flowchat-messages"
          data-bf-component="modern-flow-chat"
          data-bf-part="messages"
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
          data-presentation-mode={isRenderingHistoryProjection ? 'history-window' : 'tail'}
          data-viewport-intent={activeViewportIntent?.kind ?? 'live-tail'}
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
                emptyState !== undefined ? emptyState : (
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
              )
            ) : (
              <>
                <VirtualMessageList
                  ref={virtualListRef}
                  items={virtualItems}
                  isViewportActive={isViewportActive}
                  presentationMode={isRenderingHistoryProjection ? 'history-window' : 'tail'}
                  viewportMode={isReadingTurnViewport ? 'history-reading' : 'live-tail'}
                  historyWindow={isShowingHistoryPresentation ? activeHistoryPresentation?.range ?? null : null}
                  presentationRevision={isShowingHistoryPresentation ? activeHistoryPresentation?.revision ?? 0 : 0}
                  historyBoundaryState={historyBoundaryState}
                  onHistoryWindowBoundaryIntent={handleHistoryWindowBoundaryIntent}
                  onRequestJumpToLatest={jumpToLiveTail}
                  onUserScrollIntent={handleVirtualListUserScrollIntent}
                />
              </>
            )}
            {virtualItems.length > 0 ? (
              <FlowChatTurnRail
                turns={turnRailItems}
                currentTurnId={effectiveVisibleTurnInfo?.turnId ?? null}
                visibleTurnIds={effectiveVisibleTurnInfo?.visibleTurnIds ?? []}
                onNavigate={navigateToTurn}
              />
            ) : null}
            {showHistoryLoadingLayer && (
              <div
                className="modern-flowchat-container__history-overlay"
                role="status"
                aria-label={t('historyState.loadingTitle')}
                data-bf-component="modern-flow-chat"
                data-bf-part="historyOverlay"
              >
                <HistorySessionPlaceholder
                  state={historyState === 'metadata-only' ? 'metadata-only' : 'hydrating'}
                />
              </div>
            )}
            {showHistoryOpenIntentOverlay && (
              <div
                className="modern-flowchat-container__history-open-intent-shield"
                data-bf-component="modern-flow-chat"
                data-bf-part="historyOpenIntent"
                role="status"
                aria-label={t('historyState.loadingTitle')}
              >
                <span
                  className="modern-flowchat-container__history-open-intent-spinner"
                  data-bf-component="modern-flow-chat"
                  data-bf-part="historyOpenIntentSpinner"
                  aria-hidden="true"
                />
              </div>
            )}
          </>
        </div>
      </div>
      </FlowChatVolatileContext.Provider>
    </FlowChatContext.Provider>
  );
};

ModernFlowChatContainer.displayName = 'ModernFlowChatContainer';
