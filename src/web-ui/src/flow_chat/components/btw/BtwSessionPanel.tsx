import { Button, IconButton } from '@bitfun/ui';
import React, {useCallback, useEffect, useMemo, useRef, useState} from 'react';
import {useTranslation} from 'react-i18next';
import path from 'path-browserify';
import {CornerUpLeft, Link2, Loader2, Square, Sparkles} from 'lucide-react';
import {FlowChatContext, FlowChatVolatileContext} from '../modern/FlowChatContext';
import {VirtualItemRenderer} from '../modern/VirtualItemRenderer';
import {RuntimeStatusSlot} from '../modern/RuntimeStatusSlot';
import {pendingPermissionToolCallIdsForSession} from '../modern/permissionRequestRouting';
import {usePermissionRequests} from '../modern/usePermissionRequests';
import {useExploreGroupState} from '../modern/useExploreGroupState';
import {ChatInputApprovalBand} from '../ChatInputApprovalBand';
import {ScrollToBottomButton} from '@/flow_chat';
import {flowChatStore} from '../../store/FlowChatStore';
import type {DialogTurn, FlowChatState, Session} from '../../types/flow-chat';
import {sessionToVirtualItems} from '../../store/modernFlowChatStore';
import {FLOWCHAT_FOCUS_ITEM_EVENT, type FlowChatFocusItemRequest} from '../../events/flowchatNavigation';
import {fileTabManager} from '@/shared/services/FileTabManager';
import {createTab} from '@/shared/utils/tabUtils';
import { type LineRange } from '@/shared/editor/LineRange';
import { Tooltip } from '@bitfun/ui';
import { DEFAULT_RETAINED_MOUNT_MS, RetainedMountBoundary } from '@/shared/presence';
import {resolveSessionRelationship} from '../../utils/sessionMetadata';
import {agentAPI} from '@/infrastructure/api';
import {globalEventBus} from '@/infrastructure/event-bus';
import {notificationService} from '@/shared/notification-system';
import {createLogger} from '@/shared/utils/logger';
import {
  deriveReviewDetailProjection,
  filterReviewDetailItems,
  type ReviewDetailContentState,
  type ReviewDetailExecutionState,
} from '../../utils/reviewDetailState';
import {findReviewTaskOutcome} from '../../utils/reviewTaskOutcome';
import {
  loadBtwSessionHistory,
  type BtwSessionViewKind,
} from '../../services/btwSessionPane';
import {findLatestCodeReviewResult, findLatestCodeReviewResultState} from '../../utils/reviewSessionSummary';
import {
  deriveDeepReviewInterruption,
  deriveDeepReviewResultRecoveryInterruption,
  type DeepReviewResultRecoveryReason,
} from '../../utils/deepReviewContinuation';
import {buildReviewRemediationItems, type CodeReviewRemediationData} from '../../utils/codeReviewRemediation';
import {ReviewActionBar} from './DeepReviewActionBar';
import {
  getPendingFollowUpReviewRequestId,
  getReviewActionBarStateForSession,
  isPendingFollowUpReviewSessionId,
  type ReviewActionMode,
  type ReviewActionPhase,
  useReviewActionBarStore,
} from '../../store/deepReviewActionBarStore';
import {loadPersistedReviewState} from '../../services/ReviewActionBarPersistenceService';
import type {ReviewActionPersistedState} from '@/shared/types/session-history';
import {sessionProjectWorkspacePath} from '../../utils/sessionWorkspace';
import {
  collectModifiedFilePathsFromTurns,
  hasOpaqueWorkspaceMutationRisk,
} from '../../utils/modifiedFilePaths';
import { getMotionAwareScrollBehavior } from '../../utils/motionPreference';
import { sessionLineageLifecycleForSession } from '../../utils/sessionLineage';
import {
  SubagentAvatar,
} from '../../subagent-identity';

function findReviewChildByRequestId(
  parentSessionId: string | null | undefined,
  requestId: string,
): string | null {
  if (!parentSessionId) {
    return null;
  }
  for (const [sessionId, session] of flowChatStore.getState().sessions) {
    const relationship = resolveSessionRelationship(session);
    if (
      relationship.isReview &&
      relationship.parentSessionId === parentSessionId &&
      session.btwOrigin?.requestId === requestId
    ) {
      return sessionId;
    }
  }
  return null;
}
import './BtwSessionPanel.scss';

export interface BtwSessionPanelProps {
  childSessionId?: string;
  parentSessionId?: string;
  workspacePath?: string;
  viewKind?: BtwSessionViewKind;
  displayTitle?: string;
}

const resolveSessionTitle = (session?: Session | null, fallback = 'Side thread') =>
  session?.title?.trim() || fallback;
const log = createLogger('BtwSessionPanel');
const REVIEW_ACTION_BOTTOM_BLANK_SPACE_PX = 96;
const EMPTY_ACTION_ID_SET = new Set<string>();
const EMPTY_REMEDIATION_ITEMS: ReturnType<typeof buildReviewRemediationItems> = [];
const REVIEW_DETAIL_CONTENT_STATE_KEYS: Record<ReviewDetailContentState, string> = {
  loading: 'childSession.reviewDetail.loading',
  'load-failed': 'childSession.reviewDetail.loadFailed',
  unavailable: 'childSession.reviewDetail.unavailable',
};
const REVIEW_DETAIL_EXECUTION_STATE_KEYS: Record<ReviewDetailExecutionState, string> = {
  preparing: 'childSession.reviewDetail.preparing',
  'completed-empty': 'childSession.reviewDetail.completedEmpty',
  'partial-timeout': 'childSession.reviewDetail.partialTimedOut',
  stopped: 'childSession.reviewDetail.stopped',
  interrupted: 'childSession.reviewDetail.interrupted',
  'timed-out': 'childSession.reviewDetail.timedOut',
  failed: 'childSession.reviewDetail.failed',
};

const isActiveReviewTurnStatus = (status?: DialogTurn['status']) =>
  status === 'pending' ||
  status === 'image_analyzing' ||
  status === 'processing' ||
  status === 'finishing';

type DeepReviewActionData = CodeReviewRemediationData & {
  review_mode?: 'standard' | 'deep';
};

const isSameReviewResult = (left: unknown, right: unknown): boolean => {
  if (left === right) {
    return true;
  }
  try {
    return JSON.stringify(left) === JSON.stringify(right);
  } catch {
    return false;
  }
};

export const BtwSessionPanel: React.FC<BtwSessionPanelProps> = ({
  childSessionId,
  parentSessionId,
  workspacePath,
  viewKind,
  displayTitle,
}) => {
  const { t } = useTranslation('flow-chat');
  const [flowChatState, setFlowChatState] = useState<FlowChatState>(() => flowChatStore.getState());
  const [stoppingReview, setStoppingReview] = useState(false);
  const [showScrollToBottom, setShowScrollToBottom] = useState(false);
  const scrollContainerRef = useRef<HTMLDivElement>(null);
  const actionBarRef = useRef<HTMLDivElement>(null);
  const [actionBarHeight, setActionBarHeight] = useState(0);
  const shouldAutoScrollRef = useRef(true);

  // Embedded child sessions live outside the primary composer. Give direct
  // child requests an actionable surface here, while delegated requests stay
  // exclusively owned by the parent session.
  const {
    requests: permissionRequests,
    ownedRequests: ownedPermissionRequests,
    ownedActiveBatch: activePermissionBatch,
    respond: respondPermission,
    respondBatch: respondPermissionBatch,
  } = usePermissionRequests(childSessionId);
  const pendingPermissionToolCallIds = useMemo(
    () => pendingPermissionToolCallIdsForSession(permissionRequests, childSessionId),
    [permissionRequests, childSessionId],
  );

  useEffect(() => {
    return flowChatStore.subscribe(setFlowChatState);
  }, []);

  const childSession = childSessionId ? flowChatState.sessions.get(childSessionId) : undefined;
  const parentSession = parentSessionId ? flowChatState.sessions.get(parentSessionId) : undefined;
  const childRelationship = resolveSessionRelationship(childSession);
  const childKind = childRelationship.kind === 'review' ||
    childRelationship.kind === 'deep_review' ||
    childRelationship.kind === 'miniapp' ||
    childRelationship.kind === 'subagent'
    ? childRelationship.kind
    : 'btw';
  const subagentAvatarStatus = childKind === 'subagent' && childSession
    ? sessionLineageLifecycleForSession(childSession)
    : 'idle';
  const childBadgeLabel = viewKind === 'review-check'
    ? t('toolCards.taskTool.reviewCoverageLabel')
    : t(`childSession.kinds.${childKind}.short`, {
    defaultValue: childKind === 'deep_review'
      ? 'Strict'
      : childKind === 'review'
        ? 'Review'
        : childKind === 'subagent'
          ? 'Agent'
        : childKind === 'miniapp'
          ? 'MiniApp'
          : t('btw.shortLabel'),
    });
  const childTitleFallback = t(`childSession.kinds.${childKind}.title`, {
    defaultValue: t('btw.threadLabel'),
  });
  const childOriginLabel = t(`childSession.kinds.${childKind}.origin`, {
    defaultValue: t('btw.origin'),
  });
  const showOriginMeta = childKind !== 'miniapp' && childKind !== 'subagent';
  const sessionVirtualItems = useMemo(
    () => sessionToVirtualItems(childSession ?? null),
    [childSession],
  );
  const virtualItems = useMemo(
    () => viewKind === 'review-check'
      ? filterReviewDetailItems(sessionVirtualItems)
      : sessionVirtualItems,
    [sessionVirtualItems, viewKind],
  );
  const {
    exploreGroupStates,
    onExploreGroupToggle,
    onExpandGroup,
    onExpandAllInTurn,
    onCollapseGroup,
  } = useExploreGroupState(virtualItems);
  const isReviewDetail = viewKind === 'review-check' || childKind === 'review' || childKind === 'deep_review';
  const reviewTaskOutcome = viewKind === 'review-check'
    ? findReviewTaskOutcome(parentSession, childSession?.parentToolCallId)
    : null;
  const reviewDetailProjection = isReviewDetail
    ? deriveReviewDetailProjection(childSession, virtualItems.length > 0, reviewTaskOutcome)
    : null;
  const reviewDetailNotices = reviewDetailProjection
    ? [
        ...(reviewDetailProjection.execution
          ? [{
              state: reviewDetailProjection.execution,
              key: REVIEW_DETAIL_EXECUTION_STATE_KEYS[reviewDetailProjection.execution],
            }]
          : []),
        ...(reviewDetailProjection.content
          ? [{
              state: reviewDetailProjection.content,
              key: REVIEW_DETAIL_CONTENT_STATE_KEYS[reviewDetailProjection.content],
            }]
          : []),
      ]
    : [];
  const canRetryReviewDetailLoad = virtualItems.length === 0 && Boolean(
    reviewDetailProjection?.content === 'load-failed' ||
    reviewDetailProjection?.content === 'unavailable' ||
    reviewDetailProjection?.execution === 'partial-timeout'
  );

  // Load history for historical sessions that have not yet had their turns loaded.
  const loadChildHistory = useCallback(async () => {
    if (!childSessionId || !childSession) return;

    const path = workspacePath ?? childSession.workspacePath ?? parentSession?.workspacePath;
    if (!path) return;

    await loadBtwSessionHistory({
      childSessionId,
      ...(!childSession.workspacePath
        ? {
            workspacePath: path,
            remoteConnectionId: childSession.remoteConnectionId || parentSession?.remoteConnectionId,
            remoteSshHost: childSession.remoteSshHost || parentSession?.remoteSshHost,
          }
        : {}),
    });
  }, [childSessionId, childSession, parentSession, workspacePath]);

  useEffect(() => {
    if (!childSession?.isHistorical || childSession.historyState !== 'metadata-only') return;
    void loadChildHistory().catch(() => undefined);
  }, [childSession?.historyState, childSession?.isHistorical, loadChildHistory]);

  const updateScrollAffordance = useCallback(() => {
    const container = scrollContainerRef.current;
    if (!container) return;
    const distanceFromBottom = container.scrollHeight - container.scrollTop - container.clientHeight;
    setShowScrollToBottom(distanceFromBottom > 120);
    if (distanceFromBottom < 80) {
      shouldAutoScrollRef.current = true;
    }
  }, []);

  useEffect(() => {
    const container = scrollContainerRef.current;
    if (!container) return;
    const handleWheel = (e: WheelEvent) => {
      if (e.deltaY < 0) {
        shouldAutoScrollRef.current = false;
      } else if (e.deltaY > 0) {
        const { scrollTop, scrollHeight, clientHeight } = container;
        const distanceFromBottom = scrollHeight - scrollTop - clientHeight;
        if (distanceFromBottom < 100) {
          shouldAutoScrollRef.current = true;
        }
      }
    };

    container.addEventListener('wheel', handleWheel, { passive: true });
    container.addEventListener('scroll', updateScrollAffordance, { passive: true });
    updateScrollAffordance();
    return () => {
      container.removeEventListener('wheel', handleWheel);
      container.removeEventListener('scroll', updateScrollAffordance);
    };
  }, [updateScrollAffordance]);

  useEffect(() => {
    const container = scrollContainerRef.current;
    if (!container || !shouldAutoScrollRef.current) return;
    requestAnimationFrame(() => {
      container.scrollTop = container.scrollHeight;
      setShowScrollToBottom(false);
    });
  }, [virtualItems]);

  const handleScrollToBottom = useCallback(() => {
    const container = scrollContainerRef.current;
    if (!container) return;
    shouldAutoScrollRef.current = true;
    container.scrollTo({
      top: container.scrollHeight,
      behavior: getMotionAwareScrollBehavior('smooth'),
    });
    setShowScrollToBottom(false);
  }, []);

  const handleFileViewRequest = useCallback((
    filePath: string,
    fileName: string,
    lineRange?: LineRange
  ) => {
    let absoluteFilePath = filePath;
    const isWindowsAbsolutePath = /^[A-Za-z]:[\\/]/.test(filePath);

    if (!isWindowsAbsolutePath && !path.isAbsolute(filePath) && workspacePath) {
      absoluteFilePath = path.join(workspacePath, filePath);
    }

    fileTabManager.openFile({
      filePath: absoluteFilePath,
      fileName,
      workspacePath,
      jumpToRange: lineRange,
      mode: 'agent',
    });
  }, [workspacePath]);

  const handleTabOpen = useCallback((tabInfo: any) => {
    if (!tabInfo?.type) return;
    createTab({
      type: tabInfo.type,
      title: tabInfo.title || 'New Tab',
      data: tabInfo.data,
      metadata: tabInfo.metadata,
      checkDuplicate: !!tabInfo.metadata?.duplicateCheckKey,
      duplicateCheckKey: tabInfo.metadata?.duplicateCheckKey,
      replaceExisting: false,
      mode: 'agent',
    });
  }, []);

  const contextValue = useMemo(() => ({
    onFileViewRequest: handleFileViewRequest,
    onTabOpen: handleTabOpen,
    sessionId: childSessionId,
    activeSessionOverride: childSession ?? null,
    allowUserMessageEdit: false,
    allowTranscriptExport: viewKind !== 'review-check',
    onExploreGroupToggle,
    onExpandGroup,
    onExpandAllInTurn,
    onCollapseGroup,
  }), [
    childSession,
    childSessionId,
    handleFileViewRequest,
    handleTabOpen,
    onExploreGroupToggle,
    onExpandGroup,
    onExpandAllInTurn,
    onCollapseGroup,
    viewKind,
  ]);

  const volatileContextValue = useMemo(() => ({
    exploreGroupStates,
    pendingPermissionToolCallIds,
  }), [exploreGroupStates, pendingPermissionToolCallIds]);

  const lastDialogTurn = childSession?.dialogTurns[childSession.dialogTurns.length - 1];
  const isTurnProcessing = isActiveReviewTurnStatus(lastDialogTurn?.status);

  const canStopReviewSession =
    (viewKind === 'review-check' || childKind === 'review' || childKind === 'deep_review') &&
    isTurnProcessing &&
    !stoppingReview;

  // ---- Review action bar integration ----
  const actionBarState = useReviewActionBarStore((s) =>
    getReviewActionBarStateForSession(s, childSessionId),
  );
  const actionBarPhase = actionBarState?.phase ?? 'idle';
  const actionBarMinimized = actionBarState?.minimized ?? false;
  const actionBarChildSessionId = actionBarState?.childSessionId ?? null;
  const actionBarCompletedIds = actionBarState?.completedRemediationIds ?? EMPTY_ACTION_ID_SET;
  const actionBarRemediationItems = actionBarState?.remediationItems ?? EMPTY_REMEDIATION_ITEMS;
  const actionBarSelectedIds = actionBarState?.selectedRemediationIds ?? EMPTY_ACTION_ID_SET;
  const actionBarFixingIds = actionBarState?.fixingRemediationIds ?? EMPTY_ACTION_ID_SET;
  const actionBarLastSubmittedAction = actionBarState?.lastSubmittedAction ?? null;
  const isDeepReview = childKind === 'deep_review';
  const isReviewSession = childKind === 'review' || childKind === 'deep_review';
  const canReturnToParentSession = (viewKind === 'review-check' || isReviewSession) && Boolean(parentSessionId);
  const btwOrigin = childSession?.btwOrigin;
  const showReviewActionBar =
    isReviewSession &&
    actionBarChildSessionId === childSessionId &&
    actionBarPhase !== 'idle' &&
    !actionBarMinimized;

  const [retainedReviewActionBarOwnerId, setRetainedReviewActionBarOwnerId] = useState<string | null>(
    showReviewActionBar ? childSessionId ?? null : null,
  );
  useEffect(() => {
    const ownerId = childSessionId ?? null;
    if (showReviewActionBar && ownerId) {
      setRetainedReviewActionBarOwnerId(ownerId);
      return undefined;
    }
    if (!ownerId || retainedReviewActionBarOwnerId !== ownerId) {
      if (retainedReviewActionBarOwnerId !== null) {
        setRetainedReviewActionBarOwnerId(null);
      }
      return undefined;
    }

    const timer = window.setTimeout(() => {
      setRetainedReviewActionBarOwnerId((currentOwnerId) =>
        currentOwnerId === ownerId ? null : currentOwnerId,
      );
    }, DEFAULT_RETAINED_MOUNT_MS);
    return () => window.clearTimeout(timer);
  }, [childSessionId, retainedReviewActionBarOwnerId, showReviewActionBar]);
  const retainsReviewActionBarLayout = Boolean(
    showReviewActionBar ||
    (childSessionId && retainedReviewActionBarOwnerId === childSessionId),
  );

  const showMinimizedIndicator =
    isReviewSession &&
    actionBarChildSessionId === childSessionId &&
    actionBarPhase !== 'idle' &&
    actionBarMinimized;
  const reviewActionBottomPadding = retainsReviewActionBarLayout
    ? actionBarHeight + REVIEW_ACTION_BOTTOM_BLANK_SPACE_PX
    : showMinimizedIndicator
      ? REVIEW_ACTION_BOTTOM_BLANK_SPACE_PX
      : 0;
  const parentLabel = resolveSessionTitle(parentSession, t('btw.parent'));
  const backTooltip = btwOrigin?.parentTurnIndex
    ? t('flowChatHeader.btwBackTooltipWithTurn', {
        title: parentLabel,
        turn: btwOrigin.parentTurnIndex,
      })
    : t('flowChatHeader.btwBackTooltipWithoutTurn', {
        title: parentLabel,
      });

  const remainingCount = actionBarRemediationItems.length - actionBarCompletedIds.size;
  const totalCount = actionBarRemediationItems.length;
  const fixScopedIds = actionBarFixingIds.size > 0 ? actionBarFixingIds : actionBarSelectedIds;
  const fixScopedCompletedCount = [...fixScopedIds].filter((id) => actionBarCompletedIds.has(id)).length;
  const minimizedCountLabel = (
    ['fix_running', 'fix_completed', 'fix_failed', 'fix_timeout', 'fix_interrupted'].includes(actionBarPhase) &&
    fixScopedIds.size > 0
  )
    ? `${fixScopedCompletedCount}/${fixScopedIds.size}`
    : `${remainingCount}/${totalCount}`;
  const minimizedActionLabel = useMemo(() => {
    switch (actionBarPhase) {
      case 'review_running':
        return isDeepReview
          ? t('deepReviewActionBar.minimizedReviewRunningDeep')
          : t('deepReviewActionBar.minimizedReviewRunningStandard');
      case 'fix_running':
        return actionBarLastSubmittedAction === 'fix-review'
          ? t('deepReviewActionBar.minimizedFixReview')
          : t('deepReviewActionBar.minimizedFix');
      case 'fix_completed':
        return t('deepReviewActionBar.minimizedFixCompleted');
      case 'fix_failed':
      case 'fix_timeout':
      case 'review_error':
        return t('deepReviewActionBar.minimizedFixFailed');
      case 'review_interrupted':
      case 'resume_blocked':
      case 'resume_failed':
        return t('deepReviewActionBar.minimizedReviewInterrupted');
      case 'resume_running':
        return t('deepReviewActionBar.minimizedResume');
      default:
        return isDeepReview
          ? t('shared:features.deepReview')
          : t('deepReviewActionBar.minimizedStandard');
    }
  }, [actionBarPhase, actionBarLastSubmittedAction, isDeepReview, t]);

  // Detect when a review completes with a remediation plan and auto-show the action bar.
  useEffect(() => {
    if (!isReviewSession || !childSessionId || !childSession) return;

    const latestReviewResultState = findLatestCodeReviewResultState(childSession);
    const latestReviewData = latestReviewResultState.status === 'valid'
      ? latestReviewResultState.result as DeepReviewActionData
      : null;
    const reviewMode: ReviewActionMode = isDeepReview ? 'deep' : 'standard';
    const latestReviewMode = latestReviewData?.review_mode ?? 'standard';
    const lastTurn = childSession.dialogTurns[childSession.dialogTurns.length - 1];
    const turnStatus = lastTurn?.status;
    const isComplete = turnStatus === 'completed';
    const isError = turnStatus === 'error' || Boolean(childSession.error);
    const isReviewRunning = isActiveReviewTurnStatus(turnStatus);
    const deepReviewInterruption = isDeepReview
      ? deriveDeepReviewInterruption(childSession)
      : null;
    const resultRecoveryReason: DeepReviewResultRecoveryReason | null =
      isDeepReview && isComplete
        ? latestReviewResultState.status === 'missing'
          ? 'missing_submit_code_review'
          : latestReviewResultState.status === 'invalid'
            ? 'invalid_submit_code_review'
            : latestReviewData && latestReviewMode !== 'deep'
              ? 'wrong_review_mode'
              : null
        : null;
    const resultRecoveryInterruption = resultRecoveryReason
      ? deriveDeepReviewResultRecoveryInterruption(childSession, resultRecoveryReason)
      : null;

    const store = useReviewActionBarStore.getState();
    const currentActionState = store.getSessionState(childSessionId);
    const isCurrentResumeRunning =
      currentActionState?.phase === 'resume_running';
    if (isCurrentResumeRunning) {
      const resumeTurnHasStarted =
        !currentActionState.resumeBaselineTurnId ||
        lastTurn?.id !== currentActionState.resumeBaselineTurnId;

      if (!resumeTurnHasStarted) {
        return;
      }

      if (turnStatus === 'error') {
        store.updatePhase('resume_failed', lastTurn?.error ?? childSession.error ?? undefined, childSessionId);
        store.restore(childSessionId);
        return;
      }

      if (turnStatus === 'cancelled' && deepReviewInterruption) {
        store.showInterruptedActionBar({
          childSessionId,
          parentSessionId: parentSessionId ?? null,
          interruption: deepReviewInterruption,
        });
        store.restore(childSessionId);
        return;
      }

      if (turnStatus !== 'completed') {
        return;
      }
    }

    if (isReviewRunning) {
      const canShowRunningAction =
        !currentActionState ||
        currentActionState.phase === 'idle';

      if (canShowRunningAction) {
        store.showRunningActionBar({
          childSessionId,
          parentSessionId: parentSessionId ?? null,
          reviewMode,
        });
      }
      return;
    }

    if (resultRecoveryInterruption) {
      const canShowResultRecovery =
        !currentActionState ||
        currentActionState.phase === 'idle' ||
        currentActionState.phase === 'review_waiting_capacity' ||
        currentActionState.phase === 'resume_running';

      if (canShowResultRecovery) {
        store.showInterruptedActionBar({
          childSessionId,
          parentSessionId: parentSessionId ?? null,
          interruption: resultRecoveryInterruption,
        });
      }
      return;
    }

    if (isDeepReview && (!latestReviewData || latestReviewMode !== 'deep') && deepReviewInterruption) {
      store.showInterruptedActionBar({
        childSessionId,
        parentSessionId: parentSessionId ?? null,
        interruption: deepReviewInterruption,
      });
      return;
    }

    if (!isDeepReview && !latestReviewData && currentActionState) {
      const terminalWithoutReport = isComplete || isError || turnStatus === 'cancelled';
      if (terminalWithoutReport) {
        const message = lastTurn?.error ?? childSession.error ?? (
          turnStatus === 'cancelled'
            ? t('deepReviewActionBar.reviewCancelledWithoutReport')
            : t('deepReviewActionBar.reviewEndedWithoutReport')
        );
        store.setActiveAction(null, undefined, childSessionId);
        store.updatePhase('review_error', message, childSessionId);
        store.restore(childSessionId);
      }
      return;
    }

    if (!latestReviewData) return;
    if (isDeepReview && latestReviewMode !== 'deep') return;
    if (!isDeepReview && latestReviewMode === 'deep') return;

    const hasRemediationPlan = buildReviewRemediationItems(latestReviewData).length > 0;

    // Only activate if the action bar is idle or not yet shown for this session
    if (currentActionState && currentActionState.phase !== 'idle') {
      // A fix request briefly coexists with the previous completed review turn
      // until FlowChatManager creates the new fix turn; ignore that stale terminal state.
      const currentFixTurnHasStarted = currentActionState.phase !== 'fix_running' ||
        !currentActionState.fixingBaselineTurnId ||
        lastTurn?.id !== currentActionState.fixingBaselineTurnId;

      if (currentActionState.phase === 'fix_running' && !currentFixTurnHasStarted && (isComplete || isError)) {
        return;
      }

      if (
        currentActionState.phase === 'fix_running' &&
        currentFixTurnHasStarted &&
        (isComplete || isError || turnStatus === 'cancelled')
      ) {
        store.setRemediationModifiedFilePaths(
          collectModifiedFilePathsFromTurns(
            childSession.dialogTurns,
            currentActionState.fixingBaselineTurnId,
            childSession.workspacePath,
          ),
          childSessionId,
        );
        store.setRemediationScopeRequiresWorkspaceFallback(
          hasOpaqueWorkspaceMutationRisk(
            childSession.dialogTurns,
            currentActionState.fixingBaselineTurnId,
          ),
          childSessionId,
        );
      }

      // Update phase based on turn status if currently showing
      if (turnStatus === 'cancelled' && currentActionState.phase === 'fix_running') {
        const fixScopeIds = currentActionState.fixingRemediationIds.size > 0
          ? currentActionState.fixingRemediationIds
          : currentActionState.selectedRemediationIds;
        const remainingFixIds = [...fixScopeIds].filter((id) => !currentActionState.completedRemediationIds.has(id));
        store.setRemainingFixIds(remainingFixIds, childSessionId);
        store.setActiveAction(null, undefined, childSessionId);
        store.updatePhase('fix_interrupted', undefined, childSessionId);
        store.restore(childSessionId);
      } else if (isError && currentActionState.phase === 'resume_running') {
        store.updatePhase('resume_failed', childSession.error ?? undefined, childSessionId);
      } else if (
        isError &&
        currentActionState.phase !== 'fix_failed' &&
        currentActionState.phase !== 'review_error' &&
        currentActionState.phase !== 'fix_interrupted'
      ) {
        store.updatePhase(
          currentActionState.phase === 'fix_running' ? 'fix_failed' : 'review_error',
          childSession.error ?? undefined,
          childSessionId,
        );
      } else if (isComplete && currentActionState.phase === 'fix_running') {
        if (hasRemediationPlan && !isSameReviewResult(currentActionState.reviewData, latestReviewData)) {
          store.showActionBar({
            childSessionId,
            parentSessionId: parentSessionId ?? null,
            reviewData: latestReviewData,
            reviewMode,
            phase: 'review_completed',
            completedRemediationIds: currentActionState.completedRemediationIds,
          });
        } else {
          // Fix completed with no further remediation needed — update phase to
          // show completion state in the action bar instead of dismissing it.
          store.updatePhase('fix_completed', undefined, childSessionId);
        }
      } else if (isComplete && currentActionState.phase === 'resume_running') {
        store.showActionBar({
          childSessionId,
          parentSessionId: parentSessionId ?? null,
          reviewData: latestReviewData,
          reviewMode,
          phase: 'review_completed',
          completedRemediationIds: currentActionState.completedRemediationIds,
        });
      } else if (isComplete && currentActionState.phase === 'review_running') {
        store.showActionBar({
          childSessionId,
          parentSessionId: parentSessionId ?? null,
          reviewData: latestReviewData,
          reviewMode,
          phase: 'review_completed',
          completedRemediationIds: currentActionState.completedRemediationIds,
        });
      } else if (isComplete && currentActionState.phase === 'review_waiting_capacity') {
        store.showActionBar({
          childSessionId,
          parentSessionId: parentSessionId ?? null,
          reviewData: latestReviewData,
          reviewMode,
          phase: 'review_completed',
        });
      }
      return;
    }

    if (!isComplete && !isError) return;

    if (isError) {
      store.showActionBar({
        childSessionId,
        parentSessionId: parentSessionId ?? null,
        reviewData: latestReviewData,
        reviewMode,
        phase: 'review_error',
      });
      return;
    }

    store.showActionBar({
      childSessionId,
      parentSessionId: parentSessionId ?? null,
      reviewData: latestReviewData,
      reviewMode,
      phase: 'review_completed',
    });
  }, [
    childSession,
    childSessionId,
    parentSessionId,
    isReviewSession,
    isDeepReview,
    actionBarPhase,
    actionBarChildSessionId,
    t,
  ]);

  // Restore persisted review action state on mount
  useEffect(() => {
    if (!isReviewSession || !childSessionId || !childSession) return;

    const store = useReviewActionBarStore.getState();
    const currentActionState = store.getSessionState(childSessionId);
    const canReplaceDerivedReviewState = currentActionState && [
      'review_running',
      'review_completed',
      'review_interrupted',
    ].includes(currentActionState.phase);
    // Initial session projection may finish before metadata is loaded. Persisted
    // action state is more specific for fix/review recovery than that projection.
    if (!canReplaceDerivedReviewState && currentActionState && currentActionState.phase !== 'idle') return;

    const workspacePath = sessionProjectWorkspacePath(childSession);
    if (!workspacePath) return;

    let cancelled = false;

    loadPersistedReviewState(
      childSessionId,
      workspacePath,
      childSession.remoteConnectionId,
      childSession.remoteSshHost,
    ).then((persisted: ReviewActionPersistedState | null) => {
      if (cancelled || !persisted) return;

      const latestReviewData = findLatestCodeReviewResult(childSession) as DeepReviewActionData | null;
      const reviewMode: ReviewActionMode = isDeepReview ? 'deep' : 'standard';

      // Detect fix interruption
      let phase: ReviewActionPhase = persisted.phase as ReviewActionPhase;
      let remainingFixIds: string[] = [];
      const fixingBaselineTurnId = persisted.fixingBaselineTurnId ?? null;
      let remediationModifiedFilePaths = persisted.remediationModifiedFilePaths ?? [];
      let remediationScopeRequiresWorkspaceFallback =
        persisted.remediationScopeRequiresWorkspaceFallback ?? false;

      if (persisted.phase === 'fix_running') {
        const lastTurn = childSession.dialogTurns[childSession.dialogTurns.length - 1];
        const isStillRunning = isActiveReviewTurnStatus(lastTurn?.status);

        if (!isStillRunning) {
          // Fix was interrupted — determine remaining items
          phase = 'fix_interrupted';
          const latestItems = latestReviewData ? buildReviewRemediationItems(latestReviewData) : [];
          const latestIds = new Set(latestItems.map((i) => i.id));
          // Items that were being fixed but still exist in latest review data
          const completedIds = new Set(persisted.completedRemediationIds);
          remainingFixIds = (persisted.fixingRemediationIds ?? [])
            .filter((id: string) => latestIds.has(id) && !completedIds.has(id));
        }
        remediationModifiedFilePaths = [
          ...new Set([
            ...remediationModifiedFilePaths,
            ...collectModifiedFilePathsFromTurns(
              childSession.dialogTurns,
              fixingBaselineTurnId,
              childSession.workspacePath,
            ),
          ]),
        ];
        remediationScopeRequiresWorkspaceFallback =
          remediationScopeRequiresWorkspaceFallback ||
          hasOpaqueWorkspaceMutationRisk(
            childSession.dialogTurns,
            fixingBaselineTurnId,
          );
      }

      store.showActionBar({
        childSessionId,
        parentSessionId: parentSessionId ?? null,
        reviewData: latestReviewData ?? ({} as CodeReviewRemediationData),
        reviewMode,
        phase,
        completedRemediationIds: new Set(persisted.completedRemediationIds),
      });

      // Apply additional restored state
      store.setCustomInstructions(persisted.customInstructions, childSessionId);
      if (persisted.reviewTargetFilePaths?.length) {
        store.setReviewTargetFilePaths(persisted.reviewTargetFilePaths, childSessionId);
      }
      if (remediationModifiedFilePaths.length) {
        store.setRemediationModifiedFilePaths(
          remediationModifiedFilePaths,
          childSessionId,
        );
      }
      store.setRemediationScopeRequiresWorkspaceFallback(
        remediationScopeRequiresWorkspaceFallback,
        childSessionId,
      );
      if (phase === 'fix_running' && fixingBaselineTurnId) {
        store.setFixingBaselineTurnId(fixingBaselineTurnId, childSessionId);
      }
      if (persisted.followUpReviewSessionId) {
        const pendingRequestId = getPendingFollowUpReviewRequestId(
          persisted.followUpReviewSessionId,
        );
        const followUpSessionId = pendingRequestId
          ? findReviewChildByRequestId(parentSessionId, pendingRequestId)
          : isPendingFollowUpReviewSessionId(persisted.followUpReviewSessionId)
            ? null
            : persisted.followUpReviewSessionId;
        if (
          followUpSessionId &&
          flowChatStore.getState().sessions.has(followUpSessionId)
        ) {
          store.setFollowUpReviewSessionId(followUpSessionId, childSessionId);
        } else if (pendingRequestId) {
          store.setFollowUpReviewSessionId(
            persisted.followUpReviewSessionId,
            childSessionId,
          );
        }
      }
      if (persisted.minimized) {
        store.minimize(childSessionId);
      }
      if (remainingFixIds.length > 0) {
        store.setRemainingFixIds(remainingFixIds, childSessionId);
      }
    }).catch(() => {
      // Ignore persistence load errors
    });

    return () => {
      cancelled = true;
    };
  }, [childSession, childSessionId, parentSessionId, isReviewSession, isDeepReview]);

  // Observe action bar height to adjust body padding dynamically
  useEffect(() => {
    if (!retainsReviewActionBarLayout) {
      setActionBarHeight(0);
      return;
    }

    const el = actionBarRef.current;
    if (!el) return;
    const measuredEl =
      el.querySelector<HTMLElement>('.deep-review-action-bar') ?? el;

    const observer = new ResizeObserver((entries) => {
      for (const entry of entries) {
        const h = entry.borderBoxSize?.[0]?.blockSize ?? entry.contentRect.height;
        setActionBarHeight(h);
      }
    });

    observer.observe(measuredEl);
    // Initial measurement
    setActionBarHeight(measuredEl.getBoundingClientRect().height);

    return () => {
      observer.disconnect();
    };
  }, [retainsReviewActionBarLayout]);

  const handleStopReviewSession = useCallback(async () => {
    if (!childSessionId || stoppingReview || !isTurnProcessing) {
      return;
    }

    setStoppingReview(true);
    const reportUnconfirmedStop = async () => {
      await loadChildHistory().catch(() => undefined);
      const latestSession = flowChatStore.getState().sessions.get(childSessionId);
      const latestTurn = latestSession?.dialogTurns[latestSession.dialogTurns.length - 1];
      if (isActiveReviewTurnStatus(latestTurn?.status)) {
        notificationService.error(
          t(viewKind === 'review-check'
            ? 'toolCards.taskDetailPanel.stopReviewWorkFailed'
            : 'childSession.stopReviewFailed'),
        );
      }
    };
    try {
      const result = await agentAPI.cancelSession(childSessionId);
      if (!result.cancelled) {
        await reportUnconfirmedStop();
      } else {
        await loadChildHistory().catch(() => undefined);
      }
    } catch (error) {
      log.error('Failed to stop review session', { childSessionId, error });
      await reportUnconfirmedStop();
    } finally {
      setStoppingReview(false);
    }
  }, [childSessionId, stoppingReview, isTurnProcessing, loadChildHistory, t, viewKind]);

  const stopReviewLabel = viewKind === 'review-check'
    ? t('toolCards.taskDetailPanel.stopReviewWork')
    : t('childSession.stopReview');
  const stoppingReviewLabel = viewKind === 'review-check'
    ? t('toolCards.taskDetailPanel.stoppingReviewWork')
    : t('childSession.stoppingReview');
  const returnToParentLabel = viewKind === 'review-check'
    ? t('childSession.backToReview')
    : t('btw.backToParent');

  const handleReturnToParentSession = useCallback(() => {
    const resolvedParentSessionId = btwOrigin?.parentSessionId || parentSessionId;
    if (!resolvedParentSessionId) {
      return;
    }

    const requestId = btwOrigin?.requestId;
    const request: FlowChatFocusItemRequest = {
      sessionId: resolvedParentSessionId,
      turnId: btwOrigin?.parentDialogTurnId,
      turnIndex: btwOrigin?.parentTurnIndex,
      itemId: requestId ? `btw_marker_${requestId}` : undefined,
      source: 'btw-back',
    };

    globalEventBus.emit(
      FLOWCHAT_FOCUS_ITEM_EVENT,
      request,
      'BtwSessionPanel',
    );
  }, [btwOrigin, parentSessionId]);

  if (!childSessionId || !childSession) {
    return (
      <div className="btw-session-panel btw-session-panel--empty" data-bf-component="btw-session-panel" data-bf-part="root" data-bf-view="empty">
        <div className="btw-session-panel__empty-state" data-bf-component="btw-session-panel" data-bf-part="empty">
          {t('btw.emptyThreadLabel', { label: t('btw.threadLabel') })}
        </div>
      </div>
    );
  }

  return (
    <FlowChatContext.Provider value={contextValue}>
      <FlowChatVolatileContext.Provider value={volatileContextValue}>
      <div
        className={`btw-session-panel${retainsReviewActionBarLayout ? ' btw-session-panel--has-action-bar' : ''}`}
        data-bf-component="btw-session-panel"
        data-bf-part="root"
        data-bf-view="session"
        data-bf-state={retainsReviewActionBarLayout ? 'hasActionBar' : undefined}
      >
        <div className="btw-session-panel__header" data-bf-component="btw-session-panel" data-bf-part="header">
          <div className="btw-session-panel__header-left" data-bf-component="btw-session-panel" data-bf-part="headerMain">
            {childKind === 'subagent' ? (
              <SubagentAvatar
                sessionId={childSessionId}
                name={displayTitle}
                size={24}
                status={subagentAvatarStatus}
              />
            ) : null}
            <span className="btw-session-panel__badge" data-bf-component="btw-session-panel" data-bf-part="badge">{childBadgeLabel}</span>
          </div>
          <div className="btw-session-panel__header-title-wrap">
            <span className="btw-session-panel__title" data-bf-component="btw-session-panel" data-bf-part="title">
              {displayTitle?.trim() || (viewKind === 'review-check'
                ? childBadgeLabel
                : resolveSessionTitle(childSession, childTitleFallback))}
            </span>
          </div>
          <div className="btw-session-panel__header-right" data-bf-component="btw-session-panel" data-bf-part="actions">
            {showOriginMeta && (
              <div className="btw-session-panel__meta" data-bf-component="btw-session-panel" data-bf-part="meta">
                <span className="btw-session-panel__meta-label">{childOriginLabel}</span>
                <Link2 size={11} />
                <span className="btw-session-panel__meta-title">{resolveSessionTitle(parentSession, t('btw.parent'))}</span>
              </div>
            )}
            {(viewKind === 'review-check' || childKind === 'review' || childKind === 'deep_review') && (
              <Tooltip content={stoppingReview
                  ? stoppingReviewLabel
                  : stopReviewLabel}>
                <IconButton
                  className="btw-session-panel__stop-button"
                  size="sm"
                  onClick={() => void handleStopReviewSession()}
                  disabled={!canStopReviewSession}
                  aria-label={stoppingReview
                    ? stoppingReviewLabel
                    : stopReviewLabel}
                  data-testid="btw-session-panel-stop-review"
                  icon={stoppingReview ? (
                    <Loader2
                      className="btw-session-panel__stop-spinner"
                      size={11}
                      data-testid="btw-session-panel-stop-spinner"
                    />
                  ) : (
                    <Square size={11} />
                  )}
                />
              </Tooltip>
            )}
            {canReturnToParentSession && (
              <Tooltip content={viewKind === 'review-check' ? returnToParentLabel : backTooltip}>
                <IconButton
                  className="btw-session-panel__origin-button"
                  size="sm"
                  onClick={handleReturnToParentSession}
                  aria-label={returnToParentLabel}
                  data-testid="btw-session-panel-origin-button"
                  icon={<CornerUpLeft size={12} />}
                />
              </Tooltip>
            )}
          </div>
        </div>

        {activePermissionBatch ? (
          <div className="btw-session-panel__permission-approval">
            <ChatInputApprovalBand
              key={`${activePermissionBatch.sessionId}:${activePermissionBatch.roundId}`}
              requests={activePermissionBatch.requests}
              totalPendingCount={ownedPermissionRequests.length}
              onRespond={respondPermission}
              onRespondBatch={respondPermissionBatch}
            />
          </div>
        ) : null}

        <div
          ref={scrollContainerRef}
          tabIndex={-1}
          className="btw-session-panel__body"
          data-bf-component="btw-session-panel"
          data-bf-part="body"
          style={reviewActionBottomPadding > 0 ? { paddingBottom: `${reviewActionBottomPadding}px` } : undefined}
        >
          {isReviewDetail && reviewDetailNotices.length > 0 && (
            <div
              className={`btw-session-panel__empty-state${virtualItems.length > 0 ? ' btw-session-panel__empty-state--with-content' : ''}`}
              data-bf-component="btw-session-panel"
              data-bf-part="empty"
              role={reviewDetailNotices.some(({ state }) =>
                state === 'load-failed' || state === 'failed' || state === 'timed-out')
                ? 'alert'
                : 'status'}
              aria-live="polite"
            >
              {reviewDetailNotices.map(({ state, key }) => (
                <span key={state}>{t(key, { label: childBadgeLabel })}</span>
              ))}
              {canRetryReviewDetailLoad && (
                <Button
                  variant="outline"
                  size="sm"
                  onClick={loadChildHistory}
                >
                  {t('childSession.reviewDetail.retryLoad')}
                </Button>
              )}
            </div>
          )}
          {virtualItems.length === 0 ? (
            !isReviewDetail || reviewDetailNotices.length === 0 ? (
              <div className="btw-session-panel__empty-state" data-bf-component="btw-session-panel" data-bf-part="empty">{t('session.empty')}</div>
            ) : null
          ) : (
            virtualItems.map((item, index) => (
              <VirtualItemRenderer
                key={`${item.turnId}-${item.type}-${index}`}
                item={item}
                index={index}
              />
            ))
          )}
          <RuntimeStatusSlot
            sessionId={childSessionId}
            className="btw-session-panel__runtime-status"
          />
        </div>
        <ScrollToBottomButton
          visible={showScrollToBottom}
          onClick={handleScrollToBottom}
          focusReturnRef={scrollContainerRef}
          className="btw-session-panel__scroll-to-bottom"
          data-bf-component="btw-session-panel"
          data-bf-part="scrollToBottom"
        />
        <div
          className="btw-session-panel__minimized-indicator"
          data-bf-component="btw-session-panel"
          data-bf-part="minimized"
          data-bf-state={showMinimizedIndicator ? 'minimized' : 'hidden'}
          aria-hidden={!showMinimizedIndicator}
          {...(!showMinimizedIndicator ? { inert: '' } : {})}
        >
            <button
              type="button"
              onClick={() => useReviewActionBarStore.getState().restore(childSessionId)}
              className="btw-session-panel__minimized-button"
              aria-label={t('deepReviewActionBar.restore', {
                label: minimizedActionLabel,
              })}
            >
              <Sparkles size={14} />
              <span className="btw-session-panel__minimized-text">
                {minimizedActionLabel}
              </span>
              {totalCount > 0 && (
                <span className="btw-session-panel__minimized-count">
                  {minimizedCountLabel}
                </span>
              )}
            </button>
        </div>

        <RetainedMountBoundary present={showReviewActionBar}>
          <div
            ref={actionBarRef}
            className="btw-session-panel__action-bar-wrapper"
            data-bf-component="btw-session-panel"
            data-bf-part="actionBar"
            data-visible={showReviewActionBar ? 'true' : 'false'}
            aria-hidden={!showReviewActionBar}
            {...(!showReviewActionBar ? { inert: '' } : {})}
          >
            <ReviewActionBar childSessionId={childSessionId} />
          </div>
        </RetainedMountBoundary>
      </div>
      </FlowChatVolatileContext.Provider>
    </FlowChatContext.Provider>
  );
};

BtwSessionPanel.displayName = 'BtwSessionPanel';
