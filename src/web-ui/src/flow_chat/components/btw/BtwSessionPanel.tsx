import React, {useCallback, useEffect, useMemo, useRef, useState} from 'react';
import {useTranslation} from 'react-i18next';
import path from 'path-browserify';
import {CornerUpLeft, Link2, Square, Sparkles} from 'lucide-react';
import {FlowChatContext} from '../modern/FlowChatContext';
import {VirtualItemRenderer} from '../modern/VirtualItemRenderer';
import {ProcessingIndicator} from '../modern/ProcessingIndicator';
import {
  shouldReserveProcessingIndicatorSpace,
  shouldShowProcessingIndicator,
} from '../modern/processingIndicatorVisibility';
import {useExploreGroupState} from '../modern/useExploreGroupState';
import {ScrollToBottomButton} from '@/flow_chat';
import {flowChatStore} from '../../store/FlowChatStore';
import type {FlowChatConfig, FlowChatState, Session} from '../../types/flow-chat';
import {sessionToVirtualItems} from '../../store/modernFlowChatStore';
import {FLOWCHAT_FOCUS_ITEM_EVENT, type FlowChatFocusItemRequest} from '../../events/flowchatNavigation';
import {fileTabManager} from '@/shared/services/FileTabManager';
import {createTab} from '@/shared/utils/tabUtils';
import {IconButton, type LineRange} from '@/component-library';
import {resolveSessionRelationship} from '../../utils/sessionMetadata';
import {agentAPI} from '@/infrastructure/api';
import {globalEventBus} from '@/infrastructure/event-bus';
import {notificationService} from '@/shared/notification-system';
import {createLogger} from '@/shared/utils/logger';
import {settleStoppedReviewSessionState} from '../../utils/reviewSessionStop';
import {findLatestCodeReviewResult} from '../../utils/reviewSessionSummary';
import {deriveDeepReviewInterruption} from '../../utils/deepReviewContinuation';
import {buildReviewRemediationItems, type CodeReviewRemediationData} from '../../utils/codeReviewRemediation';
import {ReviewActionBar} from './DeepReviewActionBar';
import {type ReviewActionMode, type ReviewActionPhase, useReviewActionBarStore} from '../../store/deepReviewActionBarStore';
import {loadPersistedReviewState} from '../../services/ReviewActionBarPersistenceService';
import type {ReviewActionPersistedState} from '@/shared/types/session-history';
import './BtwSessionPanel.scss';

export interface BtwSessionPanelProps {
  childSessionId?: string;
  parentSessionId?: string;
  workspacePath?: string;
}

const PANEL_CONFIG: FlowChatConfig = {
  enableMarkdown: true,
  autoScroll: true,
  showTimestamps: false,
  maxHistoryRounds: 50,
  enableVirtualScroll: false,
  theme: 'dark',
};

const resolveSessionTitle = (session?: Session | null, fallback = 'Side thread') =>
  session?.title?.trim() || fallback;
const log = createLogger('BtwSessionPanel');

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
}) => {
  const { t } = useTranslation('flow-chat');
  const [flowChatState, setFlowChatState] = useState<FlowChatState>(() => flowChatStore.getState());
  const [stoppingReview, setStoppingReview] = useState(false);
  const [showScrollToBottom, setShowScrollToBottom] = useState(false);
  const scrollContainerRef = useRef<HTMLDivElement>(null);
  const actionBarRef = useRef<HTMLDivElement>(null);
  const [actionBarHeight, setActionBarHeight] = useState(0);
  const shouldAutoScrollRef = useRef(true);

  useEffect(() => {
    return flowChatStore.subscribe(setFlowChatState);
  }, []);

  const childSession = childSessionId ? flowChatState.sessions.get(childSessionId) : undefined;
  const parentSession = parentSessionId ? flowChatState.sessions.get(parentSessionId) : undefined;
  const childRelationship = resolveSessionRelationship(childSession);
  const childKind = childRelationship.kind === 'review' || childRelationship.kind === 'deep_review'
    ? childRelationship.kind
    : 'btw';
  const childBadgeLabel = t(`childSession.kinds.${childKind}.short`, {
    defaultValue: childKind === 'deep_review' ? 'Deep' : childKind === 'review' ? 'Review' : t('btw.shortLabel'),
  });
  const childTitleFallback = t(`childSession.kinds.${childKind}.title`, {
    defaultValue: t('btw.threadLabel'),
  });
  const childOriginLabel = t(`childSession.kinds.${childKind}.origin`, {
    defaultValue: t('btw.origin'),
  });
  const virtualItems = useMemo(() => sessionToVirtualItems(childSession ?? null), [childSession]);
  const {
    exploreGroupStates,
    onExploreGroupToggle,
    onExpandGroup,
    onExpandAllInTurn,
    onCollapseGroup,
  } = useExploreGroupState(virtualItems);

  // Load history for historical sessions that have not yet had their turns loaded.
  const isLoadingRef = useRef(false);
  useEffect(() => {
    if (!childSessionId || !childSession) return;
    if (!childSession.isHistorical) return;
    if (isLoadingRef.current) return;

    const path = workspacePath ?? childSession.workspacePath;
    if (!path) return;

    isLoadingRef.current = true;
    flowChatStore.loadSessionHistory(childSessionId, path).finally(() => {
      isLoadingRef.current = false;
    });
  }, [childSessionId, childSession, workspacePath]);

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
    container.scrollTo({ top: container.scrollHeight, behavior: 'smooth' });
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
    config: PANEL_CONFIG,
    exploreGroupStates,
    onExploreGroupToggle,
    onExpandGroup,
    onExpandAllInTurn,
    onCollapseGroup,
  }), [
    childSession,
    childSessionId,
    handleFileViewRequest,
    handleTabOpen,
    exploreGroupStates,
    onExploreGroupToggle,
    onExpandGroup,
    onExpandAllInTurn,
    onCollapseGroup,
  ]);

  const lastDialogTurn = childSession?.dialogTurns[childSession.dialogTurns.length - 1];
  const lastModelRound = lastDialogTurn?.modelRounds[lastDialogTurn.modelRounds.length - 1];
  const lastItem = lastModelRound?.items[lastModelRound.items.length - 1];
  const lastItemContent = lastItem && 'content' in lastItem ? String((lastItem as any).content || '') : '';
  const isTurnProcessing =
    lastDialogTurn?.status === 'processing' ||
    lastDialogTurn?.status === 'finishing' ||
    lastDialogTurn?.status === 'image_analyzing';
  const [isContentGrowing, setIsContentGrowing] = useState(true);
  const lastContentRef = useRef(lastItemContent);
  const contentTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    if (lastItemContent !== lastContentRef.current) {
      lastContentRef.current = lastItemContent;
      setIsContentGrowing(true);
      if (contentTimeoutRef.current) clearTimeout(contentTimeoutRef.current);
      contentTimeoutRef.current = setTimeout(() => {
        setIsContentGrowing(false);
      }, 500);
    }

    return () => {
      if (contentTimeoutRef.current) {
        clearTimeout(contentTimeoutRef.current);
      }
    };
  }, [lastItemContent]);

  useEffect(() => {
    if (!isTurnProcessing) {
      setIsContentGrowing(false);
    }
  }, [isTurnProcessing]);

  const showProcessingIndicator = useMemo(() => {
    return shouldShowProcessingIndicator({
      isTurnProcessing,
      lastItem,
      isContentGrowing,
    });
  }, [isTurnProcessing, lastItem, isContentGrowing]);

  const reserveProcessingIndicatorSpace = useMemo(() => {
    return shouldReserveProcessingIndicatorSpace({
      isTurnProcessing,
      lastItem,
      isContentGrowing,
    });
  }, [isTurnProcessing, lastItem, isContentGrowing]);

  const canStopReviewSession =
    (childKind === 'review' || childKind === 'deep_review') &&
    isTurnProcessing &&
    !stoppingReview;

  // ---- Review action bar integration ----
  const actionBarPhase = useReviewActionBarStore((s) => s.phase);
  const actionBarDismissed = useReviewActionBarStore((s) => s.dismissed);
  const actionBarMinimized = useReviewActionBarStore((s) => s.minimized);
  const actionBarChildSessionId = useReviewActionBarStore((s) => s.childSessionId);
  const actionBarCompletedIds = useReviewActionBarStore((s) => s.completedRemediationIds);
  const actionBarRemediationItems = useReviewActionBarStore((s) => s.remediationItems);
  const actionBarLastSubmittedAction = useReviewActionBarStore((s) => s.lastSubmittedAction);
  const isDeepReview = childKind === 'deep_review';
  const isReviewSession = childKind === 'review' || childKind === 'deep_review';
  const canReturnToParentSession = isReviewSession && Boolean(parentSessionId);
  const btwOrigin = childSession?.btwOrigin;
  const showReviewActionBar =
    isReviewSession &&
    actionBarChildSessionId === childSessionId &&
    actionBarPhase !== 'idle' &&
    !actionBarDismissed &&
    !actionBarMinimized;

  const showMinimizedIndicator =
    isReviewSession &&
    actionBarChildSessionId === childSessionId &&
    actionBarPhase !== 'idle' &&
    !actionBarDismissed &&
    actionBarMinimized;
  const parentLabel = resolveSessionTitle(parentSession, t('btw.parent'));
  const backTooltip = btwOrigin?.parentTurnIndex
    ? t('flowChatHeader.btwBackTooltipWithTurn', {
        title: parentLabel,
        turn: btwOrigin.parentTurnIndex,
        defaultValue: `Go back to the source session: ${parentLabel} (Turn ${btwOrigin.parentTurnIndex})`,
      })
    : t('flowChatHeader.btwBackTooltipWithoutTurn', {
        title: parentLabel,
        defaultValue: `Go back to the source session: ${parentLabel}`,
      });

  const remainingCount = actionBarRemediationItems.length - actionBarCompletedIds.size;
  const totalCount = actionBarRemediationItems.length;
  const minimizedActionLabel = useMemo(() => {
    switch (actionBarPhase) {
      case 'fix_running':
        return actionBarLastSubmittedAction === 'fix-review'
          ? t('deepReviewActionBar.minimizedFixReview', {
              defaultValue: 'Fixing and re-reviewing',
            })
          : t('deepReviewActionBar.minimizedFix', {
              defaultValue: 'Fixing',
            });
      case 'fix_completed':
        return t('deepReviewActionBar.minimizedFixCompleted', {
          defaultValue: 'Fix completed',
        });
      case 'fix_failed':
      case 'fix_timeout':
      case 'review_error':
        return t('deepReviewActionBar.minimizedFixFailed', {
          defaultValue: 'Needs attention',
        });
      case 'review_interrupted':
      case 'resume_blocked':
      case 'resume_failed':
        return t('deepReviewActionBar.minimizedReviewInterrupted', {
          defaultValue: 'Review interrupted',
        });
      case 'resume_running':
        return t('deepReviewActionBar.minimizedResume', {
          defaultValue: 'Continuing review',
        });
      default:
        return isDeepReview
          ? t('deepReviewActionBar.minimizedDeep', {
              defaultValue: 'Deep Review',
            })
          : t('deepReviewActionBar.minimizedStandard', {
              defaultValue: 'Code Review',
            });
    }
  }, [actionBarPhase, actionBarLastSubmittedAction, isDeepReview, t]);

  // Detect when a review completes with a remediation plan and auto-show the action bar.
  useEffect(() => {
    if (!isReviewSession || !childSessionId || !childSession) return;

    const latestReviewData = findLatestCodeReviewResult(childSession) as DeepReviewActionData | null;
    const reviewMode: ReviewActionMode = isDeepReview ? 'deep' : 'standard';
    const latestReviewMode = latestReviewData?.review_mode ?? 'standard';
    const lastTurn = childSession.dialogTurns[childSession.dialogTurns.length - 1];
    const turnStatus = lastTurn?.status;
    const isComplete = turnStatus === 'completed';
    const isError = turnStatus === 'error' || Boolean(childSession.error);

    const store = useReviewActionBarStore.getState();

    if (isDeepReview && (!latestReviewData || latestReviewMode !== 'deep') && isError) {
      const interruption = deriveDeepReviewInterruption(childSession);
      if (interruption) {
        store.showInterruptedActionBar({
          childSessionId,
          parentSessionId: parentSessionId ?? null,
          interruption,
        });
      }
      return;
    }

    if (!latestReviewData) return;
    if (isDeepReview && latestReviewMode !== 'deep') return;
    if (!isDeepReview && latestReviewMode === 'deep') return;

    const hasRemediationPlan = buildReviewRemediationItems(latestReviewData).length > 0;

    // Only activate if the action bar is idle or not yet shown for this session
    if (store.childSessionId === childSessionId && store.phase !== 'idle') {
      // Update phase based on turn status if currently showing
      if (isError && store.phase !== 'fix_failed' && store.phase !== 'review_error' && store.phase !== 'fix_interrupted') {
        store.updatePhase(
          store.phase === 'fix_running' ? 'fix_failed' : 'review_error',
          childSession.error ?? undefined,
        );
      } else if (isComplete && store.phase === 'fix_running') {
        if (hasRemediationPlan && !isSameReviewResult(store.reviewData, latestReviewData)) {
          store.showActionBar({
            childSessionId,
            parentSessionId: parentSessionId ?? null,
            reviewData: latestReviewData,
            reviewMode,
            phase: 'review_completed',
            completedRemediationIds: store.completedRemediationIds,
          });
        } else {
          // Fix completed with no further remediation needed — update phase to
          // show completion state in the action bar instead of dismissing it.
          store.updatePhase('fix_completed');
        }
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

    if (hasRemediationPlan) {
      store.showActionBar({
        childSessionId,
        parentSessionId: parentSessionId ?? null,
        reviewData: latestReviewData,
        reviewMode,
        phase: 'review_completed',
      });
    }
  }, [childSession, childSessionId, parentSessionId, isReviewSession, isDeepReview]);

  // Restore persisted review action state on mount
  useEffect(() => {
    if (!isReviewSession || !childSessionId || !childSession) return;

    const store = useReviewActionBarStore.getState();
    // Only restore if store is idle for this session
    if (store.phase !== 'idle' || store.childSessionId) return;

    const workspacePath = childSession.workspacePath;
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

      if (persisted.phase === 'fix_running') {
        const lastTurn = childSession.dialogTurns[childSession.dialogTurns.length - 1];
        const isStillRunning = lastTurn?.status === 'processing' || lastTurn?.status === 'finishing';

        if (!isStillRunning) {
          // Fix was interrupted — determine remaining items
          phase = 'fix_interrupted';
          const latestItems = latestReviewData ? buildReviewRemediationItems(latestReviewData) : [];
          const latestIds = new Set(latestItems.map((i) => i.id));
          // Items that were being fixed but still exist in latest review data
          remainingFixIds = persisted.completedRemediationIds.filter((id: string) => latestIds.has(id));
        }
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
      store.setCustomInstructions(persisted.customInstructions);
      if (persisted.minimized) {
        store.minimize();
      }
      if (remainingFixIds.length > 0) {
        // Set remaining fix IDs in the store
        // We need to access the store state directly to set this
        const currentState = useReviewActionBarStore.getState();
        // Use a type-safe approach
        (currentState as unknown as { remainingFixIds: string[] }).remainingFixIds = remainingFixIds;
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
    if (!showReviewActionBar) {
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
  }, [showReviewActionBar]);

  const handleStopReviewSession = useCallback(async () => {
    if (!childSessionId || stoppingReview || !isTurnProcessing) {
      return;
    }

    setStoppingReview(true);
    try {
      const cancelRequest = agentAPI.cancelSession(childSessionId);
      await settleStoppedReviewSessionState(childSessionId);
      await cancelRequest;
    } catch (error) {
      log.error('Failed to stop review session', { childSessionId, error });
      notificationService.error(
        t('childSession.stopReviewFailed', {
          defaultValue: 'Failed to stop the review session.',
        }),
      );
    } finally {
      setStoppingReview(false);
    }
  }, [childSessionId, stoppingReview, isTurnProcessing, t]);

  const handleReturnToParentSession = useCallback(() => {
    const resolvedParentSessionId = btwOrigin?.parentSessionId || parentSessionId;
    if (!resolvedParentSessionId) {
      return;
    }

    const requestId = btwOrigin?.requestId;
    const request: FlowChatFocusItemRequest = {
      sessionId: resolvedParentSessionId,
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
      <div className="btw-session-panel btw-session-panel--empty">
        <div className="btw-session-panel__empty-state">
          {t('btw.emptyThreadLabel', { label: t('btw.threadLabel') })}
        </div>
      </div>
    );
  }

  return (
    <FlowChatContext.Provider value={contextValue}>
      <div className={`btw-session-panel${showReviewActionBar ? ' btw-session-panel--has-action-bar' : ''}`}>
        <div className="btw-session-panel__header">
          <div className="btw-session-panel__header-left">
            <span className="btw-session-panel__badge">{childBadgeLabel}</span>
          </div>
          <div className="btw-session-panel__header-title-wrap">
            <span className="btw-session-panel__title">{resolveSessionTitle(childSession, childTitleFallback)}</span>
          </div>
          <div className="btw-session-panel__header-right">
            <div className="btw-session-panel__meta">
              <span className="btw-session-panel__meta-label">{childOriginLabel}</span>
              <Link2 size={11} />
              <span className="btw-session-panel__meta-title">{resolveSessionTitle(parentSession, t('btw.parent'))}</span>
            </div>
            {(childKind === 'review' || childKind === 'deep_review') && (
              <IconButton
                className="btw-session-panel__stop-button"
                variant="ghost"
                size="xs"
                onClick={() => void handleStopReviewSession()}
                disabled={!canStopReviewSession}
                tooltip={stoppingReview
                  ? t('childSession.stoppingReview', { defaultValue: 'Stopping review...' })
                  : t('childSession.stopReview', { defaultValue: 'Stop review' })}
                aria-label={stoppingReview
                  ? t('childSession.stoppingReview', { defaultValue: 'Stopping review...' })
                  : t('childSession.stopReview', { defaultValue: 'Stop review' })}
                data-testid="btw-session-panel-stop-review"
              >
                <Square size={11} />
              </IconButton>
            )}
            {canReturnToParentSession && (
              <IconButton
                className="btw-session-panel__origin-button"
                variant="ghost"
                size="xs"
                onClick={handleReturnToParentSession}
                tooltip={backTooltip}
                aria-label={t('btw.backToParent')}
                data-testid="btw-session-panel-origin-button"
              >
                <CornerUpLeft size={12} />
              </IconButton>
            )}
          </div>
        </div>

        <div
          ref={scrollContainerRef}
          className="btw-session-panel__body"
          style={actionBarHeight > 0 ? { paddingBottom: `${actionBarHeight + 20}px` } : undefined}
        >
          {virtualItems.length === 0 ? (
            <div className="btw-session-panel__empty-state">{t('session.empty')}</div>
          ) : (
            <>
              {virtualItems.map((item, index) => (
                <VirtualItemRenderer
                  key={`${item.turnId}-${item.type}-${index}`}
                  item={item}
                  index={index}
                />
              ))}
              <ProcessingIndicator
                visible={showProcessingIndicator}
                reserveSpace={reserveProcessingIndicatorSpace}
              />
            </>
          )}
        </div>
        <ScrollToBottomButton
          visible={showScrollToBottom}
          onClick={handleScrollToBottom}
          className="btw-session-panel__scroll-to-bottom"
        />
        {showMinimizedIndicator && (
          <div className="btw-session-panel__minimized-indicator">
            <button
              type="button"
              onClick={() => useReviewActionBarStore.getState().restore()}
              className="btw-session-panel__minimized-button"
              aria-label={t('deepReviewActionBar.restore', {
                label: minimizedActionLabel,
                defaultValue: `Open ${minimizedActionLabel}`,
              })}
            >
              <Sparkles size={14} />
              <span className="btw-session-panel__minimized-text">
                {minimizedActionLabel}
              </span>
              {totalCount > 0 && (
                <span className="btw-session-panel__minimized-count">
                  {remainingCount}/{totalCount}
                </span>
              )}
            </button>
          </div>
        )}

        {showReviewActionBar && (
          <div ref={actionBarRef} className="btw-session-panel__action-bar-wrapper">
            <ReviewActionBar />
          </div>
        )}
      </div>
    </FlowChatContext.Provider>
  );
};

BtwSessionPanel.displayName = 'BtwSessionPanel';
