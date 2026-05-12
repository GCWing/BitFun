import React, { useCallback, useEffect, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import {
  CheckCircle,
  AlertTriangle,
  AlertCircle,
  Clock,
  Loader2,
  MessageSquare,
} from 'lucide-react';
import {
  useReviewActionBarStore,
  type DeepReviewCapacityQueueAction,
  type DeepReviewCapacityQueueState,
  type ReviewActionPhase,
} from '../../store/deepReviewActionBarStore';
import { buildSelectedReviewRemediationPrompt } from '../../utils/codeReviewRemediation';
import {
  buildDeepReviewRetryPrompt,
  extractDeepReviewRetryableSlices,
  type RemediationGroupId,
} from '../../utils/codeReviewReport';
import { continueDeepReviewSession } from '../../services/DeepReviewContinuationService';
import { flowChatManager } from '../../services/FlowChatManager';
import { globalEventBus } from '@/infrastructure/event-bus';
import { notificationService } from '@/shared/notification-system';
import { createLogger } from '@/shared/utils/logger';
import { getAiErrorPresentation } from '@/shared/ai-errors/aiErrorPresenter';
import { confirmWarning } from '@/component-library/components/ConfirmDialog/confirmService';
import {
  aggregateReviewerProgress,
  buildErrorAttribution,
  buildRecoveryPlan,
  buildReviewerProgressSummary,
  evaluateDegradationOptions,
  extractPartialReviewData,
} from '../../utils/deepReviewExperience';
import { flowChatStore } from '../../store/FlowChatStore';
import { agentAPI } from '@/infrastructure/api/service-api/AgentAPI';
import { useSettingsStore } from '@/app/scenes/settings/settingsStore';
import { useSceneStore } from '@/app/stores/sceneStore';
import type { ConfigTab } from '@/app/scenes/settings/settingsConfig';
import { formatElapsedTime } from './actionBarFormatting';
import { CapacityQueueNotice } from './CapacityQueueNotice';
import { DecisionExecutionGate } from './DecisionExecutionGate';
import { buildInterruptionDiagnostics } from './interruptionDiagnostics';
import { PartialResultsPanel } from './PartialResultsPanel';
import { RemediationSelectionPanel } from './RemediationSelectionPanel';
import { RecoveryPlanPreview } from './RecoveryPlanPreview';
import { ReviewActionControls } from './ReviewActionControls';
import { ReviewActionHeader } from './ReviewActionHeader';
import '../../components/btw/DeepReviewActionBar.scss';

const log = createLogger('DeepReviewActionBar');

function openSettingsTab(tab: ConfigTab) {
  useSettingsStore.getState().setActiveTab(tab);
  useSceneStore.getState().openScene('settings');
}

function normalizeActionErrorMessage(error: unknown): string {
  if (error instanceof Error && error.message.trim()) {
    return error.message.trim();
  }
  if (typeof error === 'string' && error.trim()) {
    return error.trim();
  }
  return 'unknown error';
}

function buildCapacityQueueControlToolIds(
  capacityQueueState: DeepReviewCapacityQueueState,
  action: DeepReviewCapacityQueueAction,
): string[] {
  const waitingReviewers = capacityQueueState.waitingReviewers ?? [];
  const targetReviewers = action === 'skip_optional'
    ? waitingReviewers.filter((reviewer) => reviewer.optional)
    : waitingReviewers;
  const toolIds = targetReviewers
    .map((reviewer) => reviewer.toolId)
    .filter((toolId): toolId is string => Boolean(toolId));

  if (toolIds.length > 0) {
    return [...new Set(toolIds)];
  }

  return capacityQueueState.toolId ? [capacityQueueState.toolId] : [];
}

const stopNestedScrollPropagation = (event: React.WheelEvent | React.TouchEvent) => {
  event.stopPropagation();
  if ('nativeEvent' in event && typeof event.nativeEvent.stopImmediatePropagation === 'function') {
    event.nativeEvent.stopImmediatePropagation();
  }
};

interface PendingDecisionAction {
  rerunReview: boolean;
  selectedIds: Set<string>;
}

const PHASE_CONFIG: Record<ReviewActionPhase, {
  icon: React.ComponentType<{ size?: number | string; style?: React.CSSProperties; className?: string }>;
  iconClass: string;
  variant: 'success' | 'warning' | 'error' | 'info' | 'loading';
}> = {
  idle: { icon: Clock, iconClass: '', variant: 'info' },
  review_completed: { icon: CheckCircle, iconClass: 'deep-review-action-bar__icon--success', variant: 'success' },
  fix_running: { icon: Loader2, iconClass: 'deep-review-action-bar__icon--loading', variant: 'loading' },
  fix_completed: { icon: CheckCircle, iconClass: 'deep-review-action-bar__icon--success', variant: 'success' },
  fix_failed: { icon: AlertCircle, iconClass: 'deep-review-action-bar__icon--error', variant: 'error' },
  fix_timeout: { icon: Clock, iconClass: 'deep-review-action-bar__icon--warning', variant: 'warning' },
  fix_interrupted: { icon: AlertTriangle, iconClass: 'deep-review-action-bar__icon--warning', variant: 'warning' },
  review_waiting_capacity: { icon: Clock, iconClass: 'deep-review-action-bar__icon--warning', variant: 'warning' },
  review_interrupted: { icon: AlertTriangle, iconClass: 'deep-review-action-bar__icon--warning', variant: 'warning' },
  resume_blocked: { icon: AlertTriangle, iconClass: 'deep-review-action-bar__icon--error', variant: 'error' },
  resume_running: { icon: Loader2, iconClass: 'deep-review-action-bar__icon--loading', variant: 'loading' },
  resume_failed: { icon: AlertCircle, iconClass: 'deep-review-action-bar__icon--error', variant: 'error' },
  review_error: { icon: AlertTriangle, iconClass: 'deep-review-action-bar__icon--error', variant: 'error' },
};

export const ReviewActionBar: React.FC = () => {
  const { t } = useTranslation('flow-chat');
  const store = useReviewActionBarStore();
  const {
    childSessionId,
    reviewMode,
    phase,
    reviewData,
    remediationItems,
    selectedRemediationIds,
    dismissed,
    activeAction,
    lastSubmittedAction,
    customInstructions,
    errorMessage,
    interruption,
    completedRemediationIds,
    fixingRemediationIds,
    remainingFixIds,
    decisionSelections,
    capacityQueueState,
  } = store;

  const [showCustomInput, setShowCustomInput] = useState(false);
  const [showRemediationList, setShowRemediationList] = useState(true);
  const [showPartialResults, setShowPartialResults] = useState(false);
  const [expandedDecisionIds, setExpandedDecisionIds] = useState<Set<string>>(new Set());
  const [pendingDecisionAction, setPendingDecisionAction] = useState<PendingDecisionAction | null>(null);
  const [elapsedMs, setElapsedMs] = useState(0);
  const [longRunningNotified, setLongRunningNotified] = useState(false);

  const selectedCount = selectedRemediationIds.size;
  const isFixDisabled = activeAction !== null || selectedCount === 0;
  const isDeepReview = reviewMode === 'deep';
  const hasInterruption = isDeepReview && Boolean(interruption);
  const isResumeRunning = phase === 'resume_running';
  const showInterruptionDetails = hasInterruption && !isResumeRunning;
  const showCapacityQueueNotice = isDeepReview &&
    Boolean(capacityQueueState) &&
    capacityQueueState?.status !== 'running' &&
    capacityQueueState?.status !== 'capacity_skipped';
  const backendQueueControlToolIds = useMemo(
    () => capacityQueueState
      ? buildCapacityQueueControlToolIds(capacityQueueState, 'cancel')
      : [],
    [capacityQueueState],
  );
  const hasBackendQueueControlTarget = Boolean(
    childSessionId &&
    capacityQueueState?.dialogTurnId &&
    backendQueueControlToolIds.length > 0,
  );
  const supportsInlineQueueControls =
    capacityQueueState?.controlMode === 'backend'
      ? hasBackendQueueControlTarget
      : capacityQueueState?.controlMode !== 'session_stop_only';
  const handleCapacityQueueAction = useCallback(async (
    action: DeepReviewCapacityQueueAction,
    applyLocalAction: () => void,
  ) => {
    if (!capacityQueueState) {
      return;
    }

    if (capacityQueueState.controlMode !== 'backend') {
      applyLocalAction();
      return;
    }

    const toolIds = buildCapacityQueueControlToolIds(capacityQueueState, action);
    if (!childSessionId || !capacityQueueState.dialogTurnId || toolIds.length === 0) {
      notificationService.error(t('deepReviewActionBar.capacityQueue.controlFailed', {
        defaultValue: 'Queue control is unavailable because this reviewer is missing session, turn, or tool identifiers. Use Stop to interrupt the review, or wait for the queue state to refresh.',
      }));
      return;
    }

    const dialogTurnId = capacityQueueState.dialogTurnId;
    const controlResults = await Promise.allSettled(toolIds.map((toolId) => agentAPI.controlDeepReviewQueue({
      sessionId: childSessionId,
      dialogTurnId,
      toolId,
      action,
    })));
    const failedResults = controlResults.filter(
      (result): result is PromiseRejectedResult => result.status === 'rejected',
    );
    if (failedResults.length > 0) {
      const reason = normalizeActionErrorMessage(failedResults[0].reason);
      log.warn('Failed to control DeepReview capacity queue', failedResults[0].reason);
      if (failedResults.length < toolIds.length) {
        notificationService.error(t('deepReviewActionBar.capacityQueue.controlPartiallyFailedWithReason', {
          failed: failedResults.length,
          total: toolIds.length,
          reason,
          defaultValue: `Queue control partly applied; ${failedResults.length} of ${toolIds.length} reviewers failed: ${reason}. Wait for the queue state to refresh, then retry or use Stop if it is stuck.`,
        }));
        return;
      }
      notificationService.error(t('deepReviewActionBar.capacityQueue.controlFailedWithReason', {
        reason,
        defaultValue: 'Queue control failed: {{reason}}. Try again, or use Stop to interrupt the review if the queue is stuck.',
      }));
      return;
    }

    applyLocalAction();
  }, [capacityQueueState, childSessionId, t]);

  const handleOpenReviewSettings = useCallback(() => {
    openSettingsTab('review');
  }, []);

  // ---- progress tracking ----
  const sessions = flowChatStore.getState().sessions;
  const childSession = useMemo(() => {
    if (!childSessionId) return null;
    return Array.from(sessions.values()).find((s) => s.sessionId === childSessionId) ?? null;
  }, [sessions, childSessionId]);

  const retryableSlices = useMemo(() => {
    if (!isDeepReview || !reviewData || !childSession?.deepReviewRunManifest) {
      return [];
    }
    return extractDeepReviewRetryableSlices(reviewData, childSession.deepReviewRunManifest);
  }, [isDeepReview, reviewData, childSession?.deepReviewRunManifest]);

  const reviewerProgress = useMemo(() => {
    if (!childSession || childSession.sessionKind !== 'deep_review') return [];
    return aggregateReviewerProgress(childSession);
  }, [childSession]);

  const progressSummary = useMemo(() => {
    if (reviewerProgress.length === 0) return null;
    return buildReviewerProgressSummary(reviewerProgress);
  }, [reviewerProgress]);

  const partialResults = useMemo(() => {
    if (!childSession || childSession.sessionKind !== 'deep_review') return null;
    return extractPartialReviewData(childSession);
  }, [childSession]);

  // ---- error attribution ----
  const errorAttribution = useMemo(() => {
    if (!interruption) return null;
    return buildErrorAttribution(interruption);
  }, [interruption]);

  // ---- recovery plan ----
  const recoveryPlan = useMemo(() => {
    if (!interruption) return null;
    return buildRecoveryPlan(interruption);
  }, [interruption]);

  // ---- degradation options ----
  const degradationOptions = useMemo(() => {
    if (!interruption) return [];
    return evaluateDegradationOptions(interruption);
  }, [interruption]);

  const modelRecoveryAction = useMemo(() => {
    const actionCodes = new Set(interruption?.recommendedActions.map((action) => action.code) ?? []);
    if (actionCodes.has('switch_model')) {
      return 'switch_model';
    }
    if (actionCodes.has('open_model_settings')) {
      return 'open_model_settings';
    }
    return null;
  }, [interruption]);

  // ---- long-running hint ----
  useEffect(() => {
    if (phase !== 'fix_running' && phase !== 'resume_running') {
      setElapsedMs(0);
      setLongRunningNotified(false);
      return;
    }
    const startTime = Date.now();
    const interval = setInterval(() => {
      const elapsed = Date.now() - startTime;
      setElapsedMs(elapsed);
      if (elapsed > 3 * 60 * 1000 && !longRunningNotified) {
        setLongRunningNotified(true);
        notificationService.info(
          t('deepReviewActionBar.longRunningHint', {
            defaultValue: 'Review is still running. This may take a few more minutes.',
          }),
          { duration: 5000 },
        );
      }
    }, 1000);
    return () => clearInterval(interval);
  }, [phase, longRunningNotified, t]);

  const phaseConfig = PHASE_CONFIG[phase];
  const PhaseIcon = phaseConfig.icon;

  const decisionGateItems = useMemo(() => {
    if (!pendingDecisionAction) {
      return [];
    }
    return remediationItems.filter((item) => (
      pendingDecisionAction.selectedIds.has(item.id) &&
      item.requiresDecision &&
      !completedRemediationIds.has(item.id)
    ));
  }, [completedRemediationIds, pendingDecisionAction, remediationItems]);

  const decisionGateMissingSelection = useMemo(() => (
    decisionGateItems.some((item) => (
      (item.decisionContext?.options?.length ?? 0) > 0 &&
      decisionSelections[item.id] == null
    ))
  ), [decisionGateItems, decisionSelections]);

  const handleToggleRemediation = useCallback((id: string) => {
    store.toggleRemediation(id);
  }, [store]);

  const handleToggleAll = useCallback(() => {
    store.toggleAllRemediation();
  }, [store]);

  const handleToggleGroup = useCallback((groupId: string) => {
    if (groupId === 'ungrouped') return;
    store.toggleGroupRemediation(groupId as RemediationGroupId);
  }, [store]);

  const handleToggleDecisionExpansion = useCallback((id: string) => {
    setExpandedDecisionIds((prev) => {
      const next = new Set(prev);
      if (next.has(id)) {
        next.delete(id);
      } else {
        next.add(id);
      }
      return next;
    });
  }, []);

  const handleStartFixing = useCallback(async (
    rerunReview: boolean,
    overrideSelectedIds?: Set<string>,
    options?: { skipDecisionGate?: boolean },
  ) => {
    if (!reviewData || !childSessionId) return;

    const idsToFix = overrideSelectedIds ?? selectedRemediationIds;
    const selectedDecisionItems = remediationItems.filter((item) => (
      idsToFix.has(item.id) &&
      item.requiresDecision &&
      !completedRemediationIds.has(item.id)
    ));
    if (!options?.skipDecisionGate && selectedDecisionItems.length > 0) {
      const decisionIds = selectedDecisionItems.map((item) => item.id);
      setPendingDecisionAction({ rerunReview, selectedIds: new Set(idsToFix) });
      setShowRemediationList(true);
      setExpandedDecisionIds((current) => new Set([...current, ...decisionIds]));
      return;
    }

    const action = rerunReview ? 'fix-review' : 'fix';
    let prompt = buildSelectedReviewRemediationPrompt({
      reviewData,
      selectedIds: idsToFix,
      rerunReview,
      reviewMode,
      completedItems: [...completedRemediationIds],
      decisionSelections: store.decisionSelections,
    });

    if (!prompt) return;

    if (customInstructions.trim()) {
      prompt = `${prompt}\n\n## User Instructions\n${customInstructions.trim()}`;
    }

    const baselineTurnId = childSession?.dialogTurns.at(-1)?.id ?? null;
    store.setActiveAction(action, { baselineTurnId });
    store.updatePhase('fix_running');

    try {
      await flowChatManager.sendMessage(
        prompt,
        childSessionId,
        rerunReview
          ? t(isDeepReview
              ? 'reviewActionBar.fixAndReviewRequestDisplayDeep'
              : 'reviewActionBar.fixAndReviewRequestDisplayStandard', {
              defaultValue: isDeepReview
                ? 'Fix Deep Review findings and re-review'
                : 'Fix Code Review findings and re-review',
            })
          : t(isDeepReview
              ? 'reviewActionBar.fixRequestDisplayDeep'
              : 'reviewActionBar.fixRequestDisplayStandard', {
              defaultValue: isDeepReview
                ? 'Start fixing Deep Review findings'
                : 'Start fixing Code Review findings',
            }),
        isDeepReview ? 'DeepReview' : 'CodeReview',
        'agentic',
      );
    } catch (error) {
      log.error('Failed to start review remediation', { childSessionId, reviewMode, rerunReview, error });
      const msg = error instanceof Error ? error.message : String(error);
      const isTimeout = /timeout/i.test(msg);
      store.updatePhase(isTimeout ? 'fix_timeout' : 'fix_failed', msg);
      notificationService.error(
        error instanceof Error
          ? error.message
          : t('toolCards.codeReview.reviewFailed', {
              error: t('toolCards.codeReview.unknownError'),
            }),
        { duration: 5000 },
      );
    } finally {
      store.setActiveAction(null);
    }
  }, [reviewData, childSessionId, childSession, selectedRemediationIds, remediationItems, completedRemediationIds, customInstructions, reviewMode, isDeepReview, store, t]);

  const handleConfirmDecisionGate = useCallback(async () => {
    if (!pendingDecisionAction || decisionGateMissingSelection) {
      return;
    }

    const action = pendingDecisionAction;
    setPendingDecisionAction(null);
    await handleStartFixing(action.rerunReview, action.selectedIds, { skipDecisionGate: true });
  }, [decisionGateMissingSelection, handleStartFixing, pendingDecisionAction]);

  const handleCancelDecisionGate = useCallback(() => {
    setPendingDecisionAction(null);
  }, []);

  const handleRetryIncompleteSlices = useCallback(async () => {
    if (!childSessionId || retryableSlices.length === 0) return;

    store.setActiveAction('retry');
    try {
      await flowChatManager.sendMessage(
        buildDeepReviewRetryPrompt(retryableSlices),
        childSessionId,
        t('deepReviewActionBar.retryIncompleteRequestDisplay', {
          count: retryableSlices.length,
          defaultValue: `Retry ${retryableSlices.length} incomplete Deep Review slice(s)`,
        }),
        'DeepReview',
        'agentic',
      );
      store.minimize();
    } catch (error) {
      log.error('Failed to start DeepReview retry slices', { childSessionId, error });
      const message = error instanceof Error
        ? error.message
        : t('deepReviewActionBar.retryIncompleteFailed', {
          defaultValue: 'Unable to retry incomplete Deep Review slices.',
        });
      notificationService.error(message, { duration: 5000 });
    } finally {
      store.setActiveAction(null);
    }
  }, [childSessionId, retryableSlices, store, t]);

  const handleFillBackInput = useCallback(async () => {
    if (!reviewData) return;

    let prompt = buildSelectedReviewRemediationPrompt({
      reviewData,
      selectedIds: selectedRemediationIds,
      rerunReview: false,
      reviewMode,
      decisionSelections: store.decisionSelections,
    });

    if (customInstructions.trim()) {
      prompt = `${prompt}\n\n## User Instructions\n${customInstructions.trim()}`;
    }

    if (!prompt) return;

    // Check if chat input already has content — require confirmation before replacing
    const currentInputRequest: { getValue?: () => string } = {};
    globalEventBus.emit('chat-input:get-state', currentInputRequest);
    const currentInput = currentInputRequest.getValue?.() ?? '';

    if (currentInput.trim()) {
      const confirmed = await confirmWarning(
        t('deepReviewActionBar.replaceInputConfirmTitle', {
          defaultValue: 'Replace current input?',
        }),
        t('deepReviewActionBar.replaceInputConfirmMessage', {
          defaultValue: 'The chat input already has text. Filling this plan will replace the current draft.',
        }),
        {
          confirmText: t('deepReviewActionBar.replaceInputConfirmAction', {
            defaultValue: 'Replace input',
          }),
        },
      );
      if (!confirmed) return;
    }

    globalEventBus.emit('fill-chat-input', {
      content: prompt,
      mode: 'replace',
    });

    store.dismiss();
  }, [reviewData, selectedRemediationIds, customInstructions, reviewMode, store, t]);

  const handleMinimize = useCallback(() => {
    store.minimize();
  }, [store]);

  const handleContinueReview = useCallback(async () => {
    if (!interruption) return;

    if (!interruption.canResume) {
      const confirmed = await confirmWarning(
        t('deepReviewActionBar.resumeBlockedConfirmTitle', {
          defaultValue: 'Continue review?',
        }),
        t('deepReviewActionBar.resumeBlockedConfirmMessage', {
          defaultValue: 'The error that interrupted the review has not been resolved. Continuing may fail again. Do you want to proceed?',
        }),
        {
          confirmText: t('deepReviewActionBar.resumeBlockedConfirmAction', {
            defaultValue: 'Continue anyway',
          }),
        },
      );
      if (!confirmed) return;
    }

    const resumeBaselineTurnId = childSession?.dialogTurns.at(-1)?.id ?? null;
    store.setActiveAction('resume', { baselineTurnId: resumeBaselineTurnId });
    store.updatePhase('resume_running');
    store.minimize();
    try {
      await continueDeepReviewSession(interruption, t('deepReviewActionBar.resumeRequestDisplay', {
        defaultValue: 'Continue interrupted Deep Review',
      }), { force: !interruption.canResume });
    } catch (error) {
      log.error('Failed to continue interrupted Deep Review', { childSessionId, error });
      const message = t('deepReviewActionBar.resumeFailedMessage', {
        defaultValue: 'Unable to continue Deep Review. Check the model settings or try again later.',
      });
      store.updatePhase('resume_failed', message);
      store.restore();
      notificationService.error(message, { duration: 5000 });
    } finally {
      store.setActiveAction(null);
    }
  }, [childSession, childSessionId, interruption, store, t]);

  const handleContinueFix = useCallback(async () => {
    if (!reviewData || !childSessionId || remainingFixIds.length === 0) return;

    const remainingSet = new Set(remainingFixIds);
    store.setSelectedRemediationIds(remainingSet);

    await handleStartFixing(false, remainingSet);
  }, [reviewData, childSessionId, remainingFixIds, store, handleStartFixing]);

  const handleOpenModelSettings = useCallback(async () => {
    if (!interruption) return;
    openSettingsTab('models');
  }, [interruption]);

  const handleViewPartialResults = useCallback(() => {
    setShowPartialResults(true);
  }, []);

  const handleDegradationAction = useCallback((type: string) => {
    if (type === 'view_partial') {
      setShowPartialResults(true);
    } else if (type === 'reduce_reviewers') {
      notificationService.info(
        t('deepReviewActionBar.degradation.reduceReviewersPending', {
          defaultValue: 'Reduced reviewer mode will be supported in a future update.',
        }),
        { duration: 3000 },
      );
    } else if (type === 'compress_context') {
      notificationService.info(
        t('deepReviewActionBar.degradation.compressContextPending', {
          defaultValue: 'Context compression will be supported in a future update.',
        }),
        { duration: 3000 },
      );
    }
  }, [t]);

  const handleCopyDiagnostics = useCallback(async () => {
    const detail = interruption?.errorDetail;
    if (!detail) return;

    const diagnostics = buildInterruptionDiagnostics(detail, getAiErrorPresentation(detail), t);

    try {
      await navigator.clipboard.writeText(diagnostics);
      notificationService.success(t('deepReviewActionBar.diagnosticsCopied', {
        defaultValue: 'Diagnostics copied',
      }), { duration: 2500 });
    } catch {
      notificationService.error(t('deepReviewActionBar.diagnosticsCopyFailed', {
        defaultValue: 'Failed to copy diagnostics',
      }), { duration: 2500 });
    }
  }, [interruption, t]);

  const phaseTitle = useMemo(() => {
    if (hasInterruption && interruption?.errorDetail && errorAttribution) {
      const categoryLabel = t(errorAttribution.title, { defaultValue: errorAttribution.category });
      if (phase === 'review_interrupted') {
        return t('deepReviewActionBar.reviewInterruptedWithReason', {
          reason: categoryLabel,
          defaultValue: `Deep review interrupted: ${categoryLabel}`,
        });
      }
      if (phase === 'resume_blocked') {
        return t('deepReviewActionBar.resumeBlockedWithReason', {
          reason: categoryLabel,
          defaultValue: `Cannot continue: ${categoryLabel}`,
        });
      }
      if (phase === 'resume_failed') {
        return t('deepReviewActionBar.resumeFailedWithReason', {
          reason: categoryLabel,
          defaultValue: `Continue failed: ${categoryLabel}`,
        });
      }
      if (phase === 'review_error') {
        return t('deepReviewActionBar.reviewErrorWithReason', {
          reason: categoryLabel,
          defaultValue: `Review error: ${categoryLabel}`,
        });
      }
    }

    switch (phase) {
      case 'review_completed':
        return t(isDeepReview ? 'reviewActionBar.reviewCompletedDeep' : 'reviewActionBar.reviewCompletedStandard', {
          defaultValue: isDeepReview ? 'Deep review completed' : 'Review completed',
        });
      case 'fix_running':
        if (lastSubmittedAction === 'fix-review') {
          return t('deepReviewActionBar.fixAndReviewRunning', {
            defaultValue: 'Fixing and preparing re-review...',
          });
        }
        return t('deepReviewActionBar.fixRunning', {
          defaultValue: 'Fixing in progress...',
        });
      case 'fix_completed':
        return t('deepReviewActionBar.fixCompleted', {
          defaultValue: 'Fix completed',
        });
      case 'fix_failed':
        return t('deepReviewActionBar.fixFailed', {
          defaultValue: 'Fix failed',
        });
      case 'fix_timeout':
        return t('deepReviewActionBar.fixTimeout', {
          defaultValue: 'Fix timed out',
        });
      case 'review_waiting_capacity':
        return t('deepReviewActionBar.reviewWaitingCapacity', {
          defaultValue: 'Review queue waiting',
        });
      case 'review_interrupted':
        return t('deepReviewActionBar.reviewInterrupted', {
          defaultValue: 'Deep review interrupted',
        });
      case 'resume_blocked':
        return t('deepReviewActionBar.resumeBlocked', {
          defaultValue: 'Action required before continuing',
        });
      case 'resume_running':
        return t('deepReviewActionBar.resumeRunning', {
          defaultValue: 'Continuing review...',
        });
      case 'resume_failed':
        return t('deepReviewActionBar.resumeFailed', {
          defaultValue: 'Continue failed',
        });
      case 'review_error':
        return t('deepReviewActionBar.reviewError', {
          defaultValue: 'Review error',
        });
      default:
        return '';
    }
  }, [phase, isDeepReview, t, hasInterruption, interruption, errorAttribution, lastSubmittedAction]);

  if (dismissed || phase === 'idle' || !childSessionId) {
    return null;
  }

  return (
    <div
      className={`deep-review-action-bar deep-review-action-bar--${phaseConfig.variant}`}
      onWheel={stopNestedScrollPropagation}
      onTouchMove={stopNestedScrollPropagation}
    >
      <ReviewActionHeader
        reviewData={reviewData}
        PhaseIcon={PhaseIcon}
        phaseIconClass={phaseConfig.iconClass}
        phaseTitle={phaseTitle}
        errorMessage={errorMessage}
        minimizeLabel={t('deepReviewActionBar.minimize', { defaultValue: 'Minimize' })}
        onMinimize={handleMinimize}
      />

      {/* Running progress */}
      {(phase === 'fix_running' || phase === 'resume_running') && progressSummary && (
        <div className="deep-review-action-bar__progress">
          <span className="deep-review-action-bar__progress-text">
            {progressSummary.text}
          </span>
          {elapsedMs > 0 && (
            <span className="deep-review-action-bar__elapsed">
              {t('deepReviewActionBar.elapsedTime', {
                time: formatElapsedTime(elapsedMs),
                defaultValue: `Running for ${formatElapsedTime(elapsedMs)}`,
              })}
            </span>
          )}
        </div>
      )}

      {/* Capacity queue notice */}
      {showCapacityQueueNotice && capacityQueueState && (
        <CapacityQueueNotice
          capacityQueueState={capacityQueueState}
          supportsInlineQueueControls={supportsInlineQueueControls}
          onContinueQueue={() => handleCapacityQueueAction(
            'continue',
            store.continueCapacityQueue,
          )}
          onPauseQueue={() => handleCapacityQueueAction(
            'pause',
            store.pauseCapacityQueue,
          )}
          onSkipOptionalQueuedReviewers={() => handleCapacityQueueAction(
            'skip_optional',
            store.skipOptionalQueuedReviewers,
          )}
          onCancelQueuedReviewers={() => handleCapacityQueueAction(
            'cancel',
            store.cancelQueuedReviewers,
          )}
          onOpenReviewSettings={handleOpenReviewSettings}
        />
      )}

      <PartialResultsPanel
        progressSummary={showInterruptionDetails ? progressSummary : null}
        partialResults={partialResults}
        showPartialResults={showPartialResults}
        onTogglePartialResults={() => setShowPartialResults(!showPartialResults)}
      />

      {/* Error attribution card */}
      {showInterruptionDetails && errorAttribution && (
        <div
          className={`deep-review-action-bar__attribution deep-review-action-bar__attribution--${errorAttribution.severity}`}
          role="status"
          aria-live="polite"
        >
          <span className="deep-review-action-bar__attribution-message">
            {t(errorAttribution.description, { defaultValue: '' })}
          </span>
        </div>
      )}

      {/* Recovery plan preview */}
      {showInterruptionDetails && recoveryPlan && (
        <RecoveryPlanPreview recoveryPlan={recoveryPlan} />
      )}

      {/* Context overflow degradation options */}
      {showInterruptionDetails && interruption?.errorDetail?.category === 'context_overflow' && (
        <div className="deep-review-action-bar__degradation">
          <span className="deep-review-action-bar__degradation-title">
            {t('deepReviewActionBar.contextOverflowTitle', {
              defaultValue: 'Context limit reached. Choose how to proceed:',
            })}
          </span>
          {degradationOptions.map((option) => (
            <button
              key={option.type}
              type="button"
              className="deep-review-action-bar__degradation-option"
              disabled={!option.enabled}
              onClick={() => handleDegradationAction(option.type)}
            >
              <span className="deep-review-action-bar__degradation-label">
                {t(option.labelKey, { defaultValue: option.type })}
              </span>
              <span className="deep-review-action-bar__degradation-desc">
                {t(option.descriptionKey, { defaultValue: '' })}
              </span>
            </button>
          ))}
        </div>
      )}

      {/* Remediation selection stays visible while fixes run so progress remains inspectable. */}
      {['review_completed', 'fix_running', 'fix_completed', 'fix_interrupted'].includes(phase) && remediationItems.length > 0 && (
        <RemediationSelectionPanel
          remediationItems={remediationItems}
          selectedRemediationIds={selectedRemediationIds}
          completedRemediationIds={completedRemediationIds}
          fixingRemediationIds={fixingRemediationIds}
          decisionSelections={decisionSelections}
          showRemediationList={showRemediationList}
          expandedDecisionIds={expandedDecisionIds}
          selectionDisabled={phase === 'fix_running' || phase === 'fix_completed'}
          onToggleRemediation={handleToggleRemediation}
          onToggleAll={handleToggleAll}
          onToggleGroup={handleToggleGroup}
          onToggleList={() => setShowRemediationList(!showRemediationList)}
          onToggleDecisionExpansion={handleToggleDecisionExpansion}
          onSetDecisionSelection={store.setDecisionSelection}
        />
      )}

      {phase === 'review_completed' && pendingDecisionAction && decisionGateItems.length > 0 && (
        <DecisionExecutionGate
          items={decisionGateItems}
          decisionSelections={decisionSelections}
          customInstructions={customInstructions}
          rerunReview={pendingDecisionAction.rerunReview}
          confirmDisabled={decisionGateMissingSelection}
          onSelectDecision={store.setDecisionSelection}
          onCustomInstructionsChange={store.setCustomInstructions}
          onConfirm={handleConfirmDecisionGate}
          onCancel={handleCancelDecisionGate}
        />
      )}

      {/* Friendly message when review completed with no remediation items */}
      {phase === 'review_completed' && remediationItems.length === 0 && (
        <div className="deep-review-action-bar__no-issues">
          <CheckCircle size={18} className="deep-review-action-bar__no-issues-icon" />
          <span className="deep-review-action-bar__no-issues-text">
            {t('reviewActionBar.noIssuesFound', {
              defaultValue: 'No issues found. Great job!',
            })}
          </span>
        </div>
      )}

      {/* Fix completed — show success message */}
      {phase === 'fix_completed' && (
        <div className="deep-review-action-bar__fix-done">
          <CheckCircle size={16} className="deep-review-action-bar__fix-done-icon" />
          <span className="deep-review-action-bar__fix-done-text">
            {t('deepReviewActionBar.fixCompletedMessage', {
              defaultValue: 'All fixes applied successfully.',
            })}
          </span>
        </div>
      )}

      {/* Custom instructions input */}
      {phase === 'review_completed' && remediationItems.length > 0 && !pendingDecisionAction && (
        <div className="deep-review-action-bar__custom">
          <button
            type="button"
            className="deep-review-action-bar__custom-toggle"
            onClick={() => setShowCustomInput(!showCustomInput)}
          >
            <MessageSquare size={14} />
            <span>
              {showCustomInput
                ? t('deepReviewActionBar.hideCustomInput', { defaultValue: 'Hide instructions' })
                : t('deepReviewActionBar.showCustomInput', { defaultValue: 'Add instructions' })}
            </span>
          </button>
          {showCustomInput && (
            <textarea
              className="deep-review-action-bar__custom-textarea"
              placeholder={t('deepReviewActionBar.customInstructionsPlaceholder', {
                defaultValue: 'Describe additional requirements or context for the fix...',
              })}
              value={customInstructions}
              onChange={(e) => store.setCustomInstructions(e.target.value)}
              rows={2}
            />
          )}
        </div>
      )}

      <ReviewActionControls
        phase={phase}
        isDeepReview={isDeepReview}
        retryableSliceCount={retryableSlices.length}
        remediationItemCount={remediationItems.length}
        hasInterruption={showInterruptionDetails}
        partialResultsAvailable={Boolean(partialResults?.hasPartialResults)}
        activeAction={activeAction}
        isFixDisabled={isFixDisabled}
        isResumeRunning={isResumeRunning}
        remainingFixIds={remainingFixIds}
        modelRecoveryAction={modelRecoveryAction}
        onRetryIncompleteSlices={handleRetryIncompleteSlices}
        onStartFixing={handleStartFixing}
        onFillBackInput={handleFillBackInput}
        onContinueReview={handleContinueReview}
        onOpenModelSettings={handleOpenModelSettings}
        onCopyDiagnostics={handleCopyDiagnostics}
        onViewPartialResults={handleViewPartialResults}
        onContinueFix={handleContinueFix}
        onSkipRemainingFixes={store.skipRemainingFixes}
        onMinimize={handleMinimize}
      />
    </div>
  );
};

export const DeepReviewActionBar = ReviewActionBar;
