import React, { useCallback, useEffect, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import {
  CheckCircle,
  AlertTriangle,
  AlertCircle,
  Clock,
  Loader2,
  ChevronDown,
  ChevronUp,
  MessageSquare,
  Play,
  Pause,
  Copy,
  Info,
  SkipForward,
  RotateCcw,
  Eye,
  Minus,
  Settings,
} from 'lucide-react';
import { Button, Checkbox, Tooltip } from '@/component-library';
import {
  useReviewActionBarStore,
  type DeepReviewCapacityQueueAction,
  type DeepReviewCapacityQueueReason,
  type ReviewActionPhase,
} from '../../store/deepReviewActionBarStore';
import type { ReviewRemediationItem } from '../../utils/codeReviewRemediation';
import { buildSelectedReviewRemediationPrompt, REMEDIATION_GROUP_ORDER } from '../../utils/codeReviewRemediation';
import {
  buildDeepReviewRetryPrompt,
  extractDeepReviewRetryableSlices,
  type RemediationGroupId,
} from '../../utils/codeReviewReport';
import { continueDeepReviewSession } from '../../services/DeepReviewContinuationService';
import { flowChatManager } from '../../services/FlowChatManager';
import { globalEventBus } from '@/infrastructure/event-bus';
import { DEEP_REVIEW_SCROLL_TO_EVENT } from '../../events/flowchatNavigation';
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
import { CodeReviewReportExportActions } from '../../tool-cards/CodeReviewReportExportActions';
import { agentAPI } from '@/infrastructure/api/service-api/AgentAPI';
import { lowerDefaultReviewTeamMaxParallelReviewers } from '@/shared/services/reviewTeamService';
import { useSettingsStore } from '@/app/scenes/settings/settingsStore';
import { useSceneStore } from '@/app/stores/sceneStore';
import type { ConfigTab } from '@/app/scenes/settings/settingsConfig';
import '../../components/btw/DeepReviewActionBar.scss';

const log = createLogger('DeepReviewActionBar');

function openSettingsTab(tab: ConfigTab) {
  useSettingsStore.getState().setActiveTab(tab);
  useSceneStore.getState().openScene('settings');
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

const GROUP_PRIORITY_META: Record<RemediationGroupId, { color: string }> = {
  must_fix: { color: 'var(--color-error, #ef4444)' },
  should_improve: { color: 'var(--color-warning, #f59e0b)' },
  needs_decision: { color: 'var(--color-accent-500, #60a5fa)' },
  verification: { color: 'var(--color-success, #22c55e)' },
};

const CAPACITY_QUEUE_REASON_KEYS: Record<DeepReviewCapacityQueueReason, string> = {
  provider_rate_limit: 'deepReviewActionBar.capacityQueue.reasons.providerRateLimit',
  provider_concurrency_limit: 'deepReviewActionBar.capacityQueue.reasons.providerConcurrencyLimit',
  retry_after: 'deepReviewActionBar.capacityQueue.reasons.retryAfter',
  local_concurrency_cap: 'deepReviewActionBar.capacityQueue.reasons.localConcurrencyCap',
  temporary_overload: 'deepReviewActionBar.capacityQueue.reasons.temporaryOverload',
};

const stopNestedScrollPropagation = (event: React.WheelEvent | React.TouchEvent) => {
  event.stopPropagation();
  if ('nativeEvent' in event && typeof event.nativeEvent.stopImmediatePropagation === 'function') {
    event.nativeEvent.stopImmediatePropagation();
  }
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
    remainingFixIds,
    decisionSelections,
    capacityQueueState,
  } = store;

  const [showCustomInput, setShowCustomInput] = useState(false);
  const [showRemediationList, setShowRemediationList] = useState(true);
  const [showPartialResults, setShowPartialResults] = useState(false);
  const [expandedDecisionIds, setExpandedDecisionIds] = useState<Set<string>>(new Set());
  const [elapsedMs, setElapsedMs] = useState(0);
  const [longRunningNotified, setLongRunningNotified] = useState(false);

  const selectedCount = selectedRemediationIds.size;
  const totalCount = remediationItems.length;
  const allSelected = totalCount > 0 && selectedCount === totalCount;
  const isFixDisabled = activeAction !== null || selectedCount === 0;
  const isDeepReview = reviewMode === 'deep';
  const hasInterruption = isDeepReview && Boolean(interruption);
  const isResumeRunning = phase === 'resume_running';
  const showCapacityQueueNotice = isDeepReview &&
    Boolean(capacityQueueState) &&
    capacityQueueState?.status !== 'running' &&
    capacityQueueState?.status !== 'capacity_skipped';
  const hasBackendQueueControlTarget = Boolean(
    childSessionId &&
    capacityQueueState?.dialogTurnId &&
    capacityQueueState?.toolId,
  );
  const supportsInlineQueueControls =
    capacityQueueState?.controlMode === 'backend'
      ? hasBackendQueueControlTarget
      : capacityQueueState?.controlMode !== 'session_stop_only';
  const capacityQueueReasonLabel = capacityQueueState?.reason
    ? t(CAPACITY_QUEUE_REASON_KEYS[capacityQueueState.reason], {
      defaultValue: capacityQueueState.reason.split('_').join(' '),
    })
    : null;
  const capacityQueueElapsedLabel = capacityQueueState?.queueElapsedMs !== undefined
    ? formatElapsedTime(capacityQueueState.queueElapsedMs)
    : null;
  const capacityQueueMaxWaitLabel = capacityQueueState?.maxQueueWaitSeconds !== undefined
    ? formatElapsedTime(capacityQueueState.maxQueueWaitSeconds * 1000)
    : null;

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

    if (!childSessionId || !capacityQueueState.dialogTurnId || !capacityQueueState.toolId) {
      notificationService.error(t('deepReviewActionBar.capacityQueue.controlFailed', {
        defaultValue: 'Queue control is unavailable for this reviewer.',
      }));
      return;
    }

    try {
      await agentAPI.controlDeepReviewQueue({
        sessionId: childSessionId,
        dialogTurnId: capacityQueueState.dialogTurnId,
        toolId: capacityQueueState.toolId,
        action,
      });
      applyLocalAction();
    } catch (error) {
      log.warn('Failed to control DeepReview capacity queue', error);
      notificationService.error(t('deepReviewActionBar.capacityQueue.controlFailed', {
        defaultValue: 'Queue control failed. Please try again or stop the review.',
      }));
    }
  }, [capacityQueueState, childSessionId, t]);

  const handleRunSlowerNextTime = useCallback(async () => {
    try {
      const nextPolicy = await lowerDefaultReviewTeamMaxParallelReviewers();
      notificationService.success(t('deepReviewActionBar.capacityQueue.runSlowerSaved', {
        count: nextPolicy.maxParallelInstances,
        defaultValue: `Next Deep Review will use up to ${nextPolicy.maxParallelInstances} parallel reviewers.`,
      }));
    } catch (error) {
      log.warn('Failed to lower DeepReview max parallel reviewers', error);
      notificationService.error(t('deepReviewActionBar.capacityQueue.runSlowerFailed', {
        defaultValue: 'Failed to update Review settings.',
      }));
    }
  }, [t]);

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

  // Group items by priority
  const groupedItems = useMemo(() => {
    const groups: Record<string, ReviewRemediationItem[]> = {};
    for (const item of remediationItems) {
      const gid = item.groupId ?? 'ungrouped';
      if (!groups[gid]) groups[gid] = [];
      groups[gid].push(item);
    }
    return groups;
  }, [remediationItems]);

  const groupOrder = useMemo(() => {
    const ordered: string[] = [];
    for (const gid of REMEDIATION_GROUP_ORDER) {
      if (groupedItems[gid]?.length) ordered.push(gid);
    }
    if (groupedItems.ungrouped?.length) ordered.push('ungrouped');
    return ordered;
  }, [groupedItems]);

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

  const handleStartFixing = useCallback(async (rerunReview: boolean, overrideSelectedIds?: Set<string>) => {
    if (!reviewData || !childSessionId) return;

    const idsToFix = overrideSelectedIds ?? selectedRemediationIds;
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

    store.setActiveAction(action);
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
  }, [reviewData, childSessionId, selectedRemediationIds, customInstructions, reviewMode, isDeepReview, store, t, completedRemediationIds]);

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

    store.setActiveAction('resume');
    store.updatePhase('resume_running');
    try {
      await continueDeepReviewSession(interruption, t('deepReviewActionBar.resumeRequestDisplay', {
        defaultValue: 'Continue interrupted Deep Review',
      }), { force: !interruption.canResume });
      store.minimize();
    } catch (error) {
      log.error('Failed to continue interrupted Deep Review', { childSessionId, error });
      const message = t('deepReviewActionBar.resumeFailedMessage', {
        defaultValue: 'Unable to continue Deep Review. Check the model settings or try again later.',
      });
      store.updatePhase('resume_failed', message);
      notificationService.error(message, { duration: 5000 });
    } finally {
      store.setActiveAction(null);
    }
  }, [childSessionId, interruption, store, t]);

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

    const presentation = getAiErrorPresentation(detail);

    // Prefer the sanitized diagnostics from aiErrorPresenter if available
    let diagnostics = presentation.diagnostics;
    if (!diagnostics) {
      const lines: string[] = [];
      lines.push(t('deepReviewActionBar.diagnosticsTitle', { defaultValue: '=== Deep Review Interruption Diagnostics ===' }));
      lines.push('');

      const categoryLabel = t(presentation.titleKey, { defaultValue: presentation.category });
      const categoryMessage = t(presentation.messageKey, { defaultValue: '' });
      lines.push(`${t('deepReviewActionBar.diagnosticsErrorType', { defaultValue: 'Error type' })}: ${categoryLabel} (${presentation.category})`);
      if (categoryMessage) {
        lines.push(`${t('deepReviewActionBar.diagnosticsDescription', { defaultValue: 'Description' })}: ${categoryMessage}`);
      }
      lines.push('');

      if (presentation.actions.length > 0) {
        const actionLabels = presentation.actions.map((action) => {
          return t(action.labelKey, { defaultValue: action.code });
        });
        lines.push(`${t('deepReviewActionBar.diagnosticsSuggestedActions', { defaultValue: 'Suggested actions' })}: ${actionLabels.join(', ')}`);
        lines.push('');
      }

      lines.push(`${t('deepReviewActionBar.diagnosticsTechnicalDetails', { defaultValue: 'Technical details' })}:`);
      lines.push(`  - category: ${detail.category ?? 'unknown'}`);
      if (detail.provider) lines.push(`  - provider: ${detail.provider}`);
      if (detail.providerCode) lines.push(`  - provider code: ${detail.providerCode}`);
      if (detail.providerMessage) {
        const msg = detail.providerMessage.length > 500
          ? `${detail.providerMessage.slice(0, 500)}... [truncated]`
          : detail.providerMessage;
        lines.push(`  - provider message: ${msg}`);
      }
      if (detail.httpStatus) lines.push(`  - HTTP status: ${detail.httpStatus}`);
      if (detail.requestId) lines.push(`  - request ID: ${detail.requestId}`);
      if (detail.rawMessage) {
        const raw = detail.rawMessage.length > 500
          ? `${detail.rawMessage.slice(0, 500)}... [truncated]`
          : detail.rawMessage;
        lines.push(`  - raw message: ${raw}`);
      }

      diagnostics = lines.join('\n');
    }

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
      {/* Top-right controls: export actions + minimize */}
      <div className="deep-review-action-bar__controls">
        {reviewData && (
          <CodeReviewReportExportActions reviewData={reviewData} />
        )}
        <span className="deep-review-action-bar__controls-divider" />
        <button
          type="button"
          className="deep-review-action-bar__controls-btn"
          onClick={handleMinimize}
          aria-label={t('deepReviewActionBar.minimize', { defaultValue: 'Minimize' })}
        >
          <Minus size={14} />
        </button>
      </div>

      {/* Phase status header */}
      <div className="deep-review-action-bar__status">
        <PhaseIcon
          size={18}
          className={`deep-review-action-bar__icon ${phaseConfig.iconClass}`}
        />
        <span className="deep-review-action-bar__status-title">{phaseTitle}</span>
        {errorMessage && (
          <span className="deep-review-action-bar__error-message">{errorMessage}</span>
        )}
      </div>

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
        <div className="deep-review-action-bar__capacity-queue" aria-live="polite">
          <div className="deep-review-action-bar__capacity-queue-main">
            <Clock size={14} className="deep-review-action-bar__capacity-queue-icon" />
            <div className="deep-review-action-bar__capacity-queue-copy">
              <span className="deep-review-action-bar__capacity-queue-title">
                {capacityQueueState.status === 'paused_by_user'
                  ? t('deepReviewActionBar.capacityQueue.pausedTitle', {
                    defaultValue: 'Queue paused',
                  })
                  : t('deepReviewActionBar.capacityQueue.title', {
                    defaultValue: 'Reviewers waiting for capacity',
                  })}
              </span>
              <span className="deep-review-action-bar__capacity-queue-detail">
                {t('deepReviewActionBar.capacityQueue.detail', {
                  defaultValue: 'Queue wait does not count against reviewer runtime.',
                })}
              </span>
              {(capacityQueueReasonLabel || capacityQueueElapsedLabel) && (
                <span className="deep-review-action-bar__capacity-queue-meta">
                  {capacityQueueReasonLabel && (
                    <span className="deep-review-action-bar__capacity-queue-chip">
                      {t('deepReviewActionBar.capacityQueue.reason', {
                        reason: capacityQueueReasonLabel,
                        defaultValue: `Reason: ${capacityQueueReasonLabel}`,
                      })}
                    </span>
                  )}
                  {capacityQueueElapsedLabel && (
                    <span className="deep-review-action-bar__capacity-queue-chip">
                      {capacityQueueMaxWaitLabel
                        ? t('deepReviewActionBar.capacityQueue.elapsedWithMax', {
                          elapsed: capacityQueueElapsedLabel,
                          max: capacityQueueMaxWaitLabel,
                          defaultValue: `Waited ${capacityQueueElapsedLabel} of ${capacityQueueMaxWaitLabel}`,
                        })
                        : t('deepReviewActionBar.capacityQueue.elapsed', {
                          elapsed: capacityQueueElapsedLabel,
                          defaultValue: `Waited ${capacityQueueElapsedLabel}`,
                        })}
                    </span>
                  )}
                </span>
              )}
              {capacityQueueState.sessionConcurrencyHigh && (
                <span className="deep-review-action-bar__capacity-queue-detail">
                  {t('deepReviewActionBar.capacityQueue.sessionBusy', {
                    defaultValue: 'Your active session is busy. Pause Deep Review or continue later.',
                  })}
                </span>
              )}
              {!supportsInlineQueueControls && (
                <span className="deep-review-action-bar__capacity-queue-detail">
                  {t('deepReviewActionBar.capacityQueue.stopHint', {
                    defaultValue: 'Use Stop to interrupt this review queue.',
                  })}
                </span>
              )}
            </div>
          </div>
          <div className="deep-review-action-bar__capacity-queue-actions">
            {supportsInlineQueueControls && (
              <>
                {capacityQueueState.status === 'paused_by_user' ? (
                  <Button
                    variant="secondary"
                    size="small"
                    onClick={() => void handleCapacityQueueAction(
                      'continue',
                      store.continueCapacityQueue,
                    )}
                  >
                    <Play size={13} />
                    {t('deepReviewActionBar.capacityQueue.continueQueue', {
                      defaultValue: 'Continue queue',
                    })}
                  </Button>
                ) : (
                  <Button
                    variant="secondary"
                    size="small"
                    onClick={() => void handleCapacityQueueAction(
                      'pause',
                      store.pauseCapacityQueue,
                    )}
                  >
                    <Pause size={13} />
                    {t('deepReviewActionBar.capacityQueue.pauseQueue', {
                      defaultValue: 'Pause queue',
                    })}
                  </Button>
                )}
                {(capacityQueueState.optionalReviewerCount ?? 0) > 0 && (
                  <Button
                    variant="ghost"
                    size="small"
                    onClick={() => void handleCapacityQueueAction(
                      'skip_optional',
                      store.skipOptionalQueuedReviewers,
                    )}
                  >
                    <SkipForward size={13} />
                    {t('deepReviewActionBar.capacityQueue.skipOptionalQueued', {
                      defaultValue: 'Skip optional extras',
                    })}
                  </Button>
                )}
                <Button
                  variant="ghost"
                  size="small"
                  onClick={() => void handleCapacityQueueAction(
                    'cancel',
                    store.cancelQueuedReviewers,
                  )}
                >
                  {t('deepReviewActionBar.capacityQueue.cancelQueued', {
                    defaultValue: 'Cancel queued reviewers',
                  })}
                </Button>
              </>
            )}
            <Button
              variant="ghost"
              size="small"
              onClick={() => void handleRunSlowerNextTime()}
            >
              <Settings size={13} />
              {t('deepReviewActionBar.capacityQueue.runSlowerNextTime', {
                defaultValue: 'Run slower next time',
              })}
            </Button>
            <Button
              variant="ghost"
              size="small"
              onClick={handleOpenReviewSettings}
            >
              {t('deepReviewActionBar.capacityQueue.openReviewSettings', {
                defaultValue: 'Open Review settings',
              })}
            </Button>
          </div>
        </div>
      )}

      {/* Partial results summary on interruption */}
      {hasInterruption && progressSummary && progressSummary.completed > 0 && (
        <div className="deep-review-action-bar__partial-summary">
          <span className="deep-review-action-bar__partial-count">
            {t('deepReviewActionBar.partialResultsDescription', {
              completed: progressSummary.completed,
              total: progressSummary.total,
              defaultValue: '{{completed}}/{{total}} reviewers completed',
            })}
          </span>
          <button
            type="button"
            className="deep-review-action-bar__partial-link"
            onClick={() => setShowPartialResults(!showPartialResults)}
          >
            <Eye size={12} />
            {showPartialResults
              ? t('deepReviewActionBar.hidePartialResults', { defaultValue: 'Hide partial results' })
              : t('deepReviewActionBar.viewPartialResults', { defaultValue: 'View partial results' })}
          </button>
        </div>
      )}

      {/* Partial results detail */}
      {showPartialResults && partialResults && (
        <div className="deep-review-action-bar__partial-detail">
          {partialResults.completedIssues.length > 0 && (
            <div className="deep-review-action-bar__partial-section">
              <span className="deep-review-action-bar__partial-section-title">
                {t('deepReviewActionBar.partialIssues', {
                  count: partialResults.completedIssues.length,
                  defaultValue: '{{count}} issues found',
                })}
              </span>
            </div>
          )}
          {partialResults.completedRemediationItems.length > 0 && (
            <div className="deep-review-action-bar__partial-section">
              <span className="deep-review-action-bar__partial-section-title">
                {t('deepReviewActionBar.partialRemediationItems', {
                  count: partialResults.completedRemediationItems.length,
                  defaultValue: '{{count}} remediation items',
                })}
              </span>
            </div>
          )}
        </div>
      )}

      {/* Error attribution card */}
      {hasInterruption && errorAttribution && (
        <div className={`deep-review-action-bar__attribution deep-review-action-bar__attribution--${errorAttribution.severity}`}>
          <span className="deep-review-action-bar__attribution-message">
            {t(errorAttribution.description, { defaultValue: '' })}
          </span>
          {errorAttribution.actions.length > 0 && (
            <div className="deep-review-action-bar__attribution-actions">
              {errorAttribution.actions.map((action) => (
                <Button
                  key={action.code}
                  variant="secondary"
                  size="small"
                  onClick={() => {
                    if (action.code === 'open_model_settings') {
                      openSettingsTab('models');
                    } else if (action.code === 'switch_model') {
                      openSettingsTab('models');
                    } else if (action.code === 'retry' || action.code === 'continue') {
                      void handleContinueReview();
                    } else if (action.code === 'wait_and_retry') {
                      void handleContinueReview();
                    } else if (action.code === 'copy_diagnostics') {
                      void handleCopyDiagnostics();
                    }
                  }}
                >
                  {t(action.labelKey, { defaultValue: action.code })}
                </Button>
              ))}
            </div>
          )}
        </div>
      )}

      {/* Recovery plan preview */}
      {hasInterruption && recoveryPlan && (
        <div className="deep-review-action-bar__recovery-plan">
          <div className="deep-review-action-bar__recovery-plan-detail">
            {recoveryPlan.willPreserve.length > 0 && (
              <div className="deep-review-action-bar__recovery-item">
                <CheckCircle size={12} className="deep-review-action-bar__recovery-icon--preserve" />
                <span>
                  {t('deepReviewActionBar.recoveryPreserve', {
                    count: recoveryPlan.willPreserve.length,
                    defaultValue: '{{count}} completed reviewers will be preserved',
                  })}
                </span>
              </div>
            )}
            {recoveryPlan.willRerun.length > 0 && (
              <div className="deep-review-action-bar__recovery-item">
                <RotateCcw size={12} className="deep-review-action-bar__recovery-icon--rerun" />
                <span>
                  {t('deepReviewActionBar.recoveryRerun', {
                    count: recoveryPlan.willRerun.length,
                    defaultValue: '{{count}} reviewers will be rerun',
                  })}
                </span>
              </div>
            )}
            {recoveryPlan.willSkip.length > 0 && (
              <div className="deep-review-action-bar__recovery-item">
                <SkipForward size={12} className="deep-review-action-bar__recovery-icon--skip" />
                <span>
                  {t('deepReviewActionBar.recoverySkip', {
                    count: recoveryPlan.willSkip.length,
                    defaultValue: '{{count}} reviewers will be skipped',
                  })}
                </span>
              </div>
            )}
          </div>
        </div>
      )}

      {/* Context overflow degradation options */}
      {hasInterruption && interruption?.errorDetail?.category === 'context_overflow' && (
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

      {/* Remediation selection (only when review completed and has items) */}
      {phase === 'review_completed' && remediationItems.length > 0 && (
        <div className="deep-review-action-bar__remediation">
          <button
            type="button"
            className="deep-review-action-bar__remediation-toggle"
            onClick={() => setShowRemediationList(!showRemediationList)}
          >
            <Checkbox
              checked={allSelected}
              indeterminate={!allSelected && selectedCount > 0}
              onChange={handleToggleAll}
              size="small"
            />
            <span className="deep-review-action-bar__remediation-label">
              {t('toolCards.codeReview.remediationActions.selectionCount', {
                selected: selectedCount,
                total: totalCount,
                defaultValue: '{{selected}}/{{total}} selected',
              })}
            </span>
            {showRemediationList ? <ChevronUp size={14} /> : <ChevronDown size={14} />}
          </button>

          {showRemediationList && (
            <div
              className="deep-review-action-bar__remediation-list"
              onWheel={stopNestedScrollPropagation}
              onTouchMove={stopNestedScrollPropagation}
            >
              {groupOrder.map((groupId) => {
                const items = groupedItems[groupId]!;
                const groupSelectedCount = items.filter((i) => selectedRemediationIds.has(i.id)).length;
                const groupAllSelected = groupSelectedCount === items.length;
                const groupPartial = groupSelectedCount > 0 && !groupAllSelected;
                const groupTitle = groupId === 'ungrouped'
                  ? t('toolCards.codeReview.remediationActions.ungrouped', { defaultValue: 'Other' })
                  : t(`toolCards.codeReview.groups.${groupId}`, { defaultValue: groupId });
                const groupMeta = groupId !== 'ungrouped' ? GROUP_PRIORITY_META[groupId as RemediationGroupId] : undefined;

                return (
                  <div key={groupId} className="deep-review-action-bar__remediation-group">
                    <button
                      type="button"
                      className="deep-review-action-bar__remediation-group-header"
                      onClick={() => handleToggleGroup(groupId)}
                    >
                      <Checkbox
                        checked={groupAllSelected}
                        indeterminate={groupPartial}
                        onChange={() => handleToggleGroup(groupId)}
                        size="small"
                      />
                      <span
                        className="deep-review-action-bar__remediation-group-title"
                        style={groupMeta ? { color: groupMeta.color } : undefined}
                      >
                        {groupTitle}
                      </span>
                      <span className="deep-review-action-bar__remediation-group-count">
                        {groupSelectedCount}/{items.length}
                      </span>
                    </button>
                    <div className="deep-review-action-bar__remediation-group-items">
                      {items.map((item: ReviewRemediationItem) => {
                        const isCompleted = completedRemediationIds.has(item.id);
                        return (
                          <label
                            key={item.id}
                            className={`deep-review-action-bar__remediation-item ${
                              isCompleted ? 'deep-review-action-bar__remediation-item--completed' : ''
                            }`}
                          >
                            <Checkbox
                              checked={selectedRemediationIds.has(item.id)}
                              onChange={() => !isCompleted && handleToggleRemediation(item.id)}
                              disabled={isCompleted}
                              size="small"
                            />
                            <span
                              className="deep-review-action-bar__remediation-text"
                              title={item.decisionContext ? item.plan : undefined}
                            >
                              {isCompleted && (
                                <CheckCircle size={12} className="deep-review-action-bar__completed-icon" />
                              )}
                              {item.requiresDecision && (
                                <span className="deep-review-action-bar__remediation-tag">
                                  {t('reviewActionBar.needsDecisionTag', { defaultValue: 'Decision' })}
                                </span>
                              )}
                              {item.groupId === 'verification' ? (
                                <span className="deep-review-action-bar__remediation-text-plain">
                                  {item.decisionContext?.question ?? item.plan}
                                </span>
                              ) : (
                                <a
                                  className="deep-review-action-bar__remediation-link"
                                  href={`#review-remediation-${item.groupId ?? 'ungrouped'}-${item.groupIndex}`}
                                  onClick={(e) => {
                                    e.preventDefault();
                                    e.stopPropagation();
                                    if (item.groupId != null) {
                                      globalEventBus.emit(DEEP_REVIEW_SCROLL_TO_EVENT, {
                                        groupId: item.groupId,
                                        groupIndex: item.groupIndex,
                                        issueIndex: item.issueIndex ?? -1,
                                      });
                                    }
                                  }}
                                >
                                  {item.decisionContext?.question ?? item.plan}
                                </a>
                              )}
                              {item.decisionContext?.tradeoffs && (
                                <span className="deep-review-action-bar__remediation-tradeoffs">
                                  {item.decisionContext.tradeoffs}
                                </span>
                              )}
                              {item.decisionContext?.options && item.decisionContext.options.length > 0 && (
                                <button
                                  type="button"
                                  className="deep-review-action-bar__decision-toggle"
                                  onClick={(e) => {
                                    e.preventDefault();
                                    e.stopPropagation();
                                    setExpandedDecisionIds((prev) => {
                                      const next = new Set(prev);
                                      if (next.has(item.id)) {
                                        next.delete(item.id);
                                      } else {
                                        next.add(item.id);
                                      }
                                      return next;
                                    });
                                  }}
                                >
                                  {expandedDecisionIds.has(item.id)
                                    ? t('toolCards.codeReview.remediationActions.collapseOptions', { defaultValue: 'Hide options' })
                                    : t('toolCards.codeReview.remediationActions.expandOptions', { defaultValue: 'Show options' })
                                  }
                                </button>
                              )}
                              {expandedDecisionIds.has(item.id) && item.decisionContext?.options && (
                                <ul className="deep-review-action-bar__decision-options">
                                  {item.decisionContext.options.map((opt, oi) => {
                                    const isSelected = decisionSelections[item.id] === oi;
                                    const isRecommended = item.decisionContext?.recommendation === oi;
                                    return (
                                      <li key={oi} className={`deep-review-action-bar__decision-option ${isSelected ? 'is-selected' : ''} ${isRecommended ? 'is-recommended' : ''}`}>
                                        <button
                                          type="button"
                                          onClick={(e) => {
                                            e.preventDefault();
                                            e.stopPropagation();
                                            store.setDecisionSelection(item.id, oi);
                                          }}
                                        >
                                          <span className="deep-review-action-bar__decision-option-marker">
                                            {isSelected ? '\u25CF' : '\u25CB'}
                                          </span>
                                          <span className="deep-review-action-bar__decision-option-text">
                                            {opt}
                                            {isRecommended ? ` (${t('toolCards.codeReview.remediationActions.recommended', { defaultValue: 'recommended' })})` : ''}
                                          </span>
                                        </button>
                                      </li>
                                    );
                                  })}
                                </ul>
                              )}
                            </span>
                          </label>
                        );
                      })}
                    </div>
                  </div>
                );
              })}
            </div>
          )}

          {/* Empty selection hint */}
          {selectedCount === 0 && (
            <div className="deep-review-action-bar__empty-selection" role="note">
              <Info size={14} className="deep-review-action-bar__empty-selection-icon" />
              <span>
                {t('toolCards.codeReview.remediationActions.noSelectionHint', {
                  defaultValue: 'Select at least one remediation item to start fixing.',
                })}
              </span>
            </div>
          )}
        </div>
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
      {phase === 'review_completed' && remediationItems.length > 0 && (
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

      {/* Action buttons */}
      <div className="deep-review-action-bar__actions">
        {phase === 'review_completed' && isDeepReview && retryableSlices.length > 0 && (
          <Button
            variant="secondary"
            size="small"
            isLoading={activeAction === 'retry'}
            disabled={activeAction !== null}
            onClick={() => void handleRetryIncompleteSlices()}
          >
            <RotateCcw size={14} />
            {t('deepReviewActionBar.retryIncompleteSlices', {
              count: retryableSlices.length,
              defaultValue: `Retry incomplete slices (${retryableSlices.length})`,
            })}
          </Button>
        )}
        {phase === 'review_completed' && remediationItems.length > 0 && (
          <>
            <Button
              variant="primary"
              size="small"
              isLoading={activeAction === 'fix'}
              disabled={isFixDisabled}
              onClick={() => void handleStartFixing(false)}
            >
              {t('toolCards.codeReview.remediationActions.startFix', { defaultValue: 'Start fixing' })}
            </Button>
            <Button
              variant="secondary"
              size="small"
              isLoading={activeAction === 'fix-review'}
              disabled={isFixDisabled}
              onClick={() => void handleStartFixing(true)}
            >
              {t('toolCards.codeReview.remediationActions.fixAndReview', { defaultValue: 'Fix and re-review' })}
            </Button>
            <Tooltip content={t('deepReviewActionBar.fillBackInputHint', {
              defaultValue: 'Copy selected fix plan to the input box for manual editing',
            })}>
              <Button
                variant="ghost"
                size="small"
                disabled={isFixDisabled}
                onClick={() => void handleFillBackInput()}
              >
                {t('deepReviewActionBar.fillBackInput', { defaultValue: 'Fill to input' })}
              </Button>
            </Tooltip>
          </>
        )}

        {hasInterruption && (
          <>
            <Button
              variant="primary"
              size="small"
              isLoading={activeAction === 'resume'}
              disabled={activeAction !== null || isResumeRunning}
              onClick={() => void handleContinueReview()}
            >
              <Play size={14} />
              {t('deepReviewActionBar.resumeReview', { defaultValue: 'Continue review' })}
            </Button>
            {modelRecoveryAction && (
              <Button
                variant="secondary"
                size="small"
                disabled={activeAction !== null}
                onClick={() => void handleOpenModelSettings()}
              >
                {modelRecoveryAction === 'switch_model'
                  ? t('deepReviewActionBar.switchModel', { defaultValue: 'Switch model' })
                  : t('deepReviewActionBar.openModelSettings', { defaultValue: 'Open model settings' })}
              </Button>
            )}
            <Button
              variant="ghost"
              size="small"
              onClick={handleCopyDiagnostics}
            >
              <Copy size={14} />
              {t('deepReviewActionBar.copyDiagnostics', { defaultValue: 'Copy diagnostics' })}
            </Button>
            {partialResults?.hasPartialResults && (
              <Button
                variant="ghost"
                size="small"
                onClick={handleViewPartialResults}
              >
                <Eye size={14} />
                {t('deepReviewActionBar.viewPartialResults', { defaultValue: 'View partial results' })}
              </Button>
            )}
          </>
        )}

        {phase === 'fix_interrupted' && (
          <>
            <div className="deep-review-action-bar__interruption-notice">
              <AlertTriangle size={16} className="deep-review-action-bar__interruption-icon" />
              <span>
                {t('deepReviewActionBar.fixInterrupted', {
                  defaultValue: 'Fix was interrupted. {{count}} items remain.',
                  count: remainingFixIds.length,
                })}
              </span>
            </div>
            <Button
              variant="primary"
              size="small"
              onClick={() => void handleContinueFix()}
            >
              <Play size={14} />
              {t('deepReviewActionBar.continueFix', {
                defaultValue: 'Continue fixing {{count}} items',
                count: remainingFixIds.length,
              })}
            </Button>
            <Button
              variant="secondary"
              size="small"
              onClick={() => store.skipRemainingFixes()}
            >
              {t('deepReviewActionBar.skipRemaining', { defaultValue: 'Skip remaining' })}
            </Button>
          </>
        )}

        {(phase === 'fix_completed' || phase === 'fix_failed' || phase === 'fix_timeout' || phase === 'review_error') && (
          <Button
            variant="ghost"
            size="small"
            onClick={handleMinimize}
          >
            {t('deepReviewActionBar.minimize', { defaultValue: 'Minimize' })}
          </Button>
        )}
      </div>
    </div>
  );
};

function formatElapsedTime(ms: number): string {
  const seconds = Math.floor(ms / 1000);
  const minutes = Math.floor(seconds / 60);
  const remainingSeconds = seconds % 60;
  if (minutes > 0) {
    return `${minutes}m ${remainingSeconds}s`;
  }
  return `${seconds}s`;
}

export const DeepReviewActionBar = ReviewActionBar;
