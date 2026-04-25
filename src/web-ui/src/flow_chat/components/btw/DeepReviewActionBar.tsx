import React, { useCallback, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import {
  CheckCircle,
  AlertTriangle,
  AlertCircle,
  Clock,
  Loader2,
  ChevronDown,
  ChevronUp,
  X,
  MessageSquare,
  Play,
  Copy,
} from 'lucide-react';
import { Button, Checkbox, Tooltip } from '@/component-library';
import { useReviewActionBarStore, type ReviewActionPhase } from '../../store/deepReviewActionBarStore';
import type { ReviewRemediationItem } from '../../utils/codeReviewRemediation';
import { buildSelectedReviewRemediationPrompt } from '../../utils/codeReviewRemediation';
import { continueDeepReviewSession } from '../../services/DeepReviewContinuationService';
import { flowChatManager } from '../../services/FlowChatManager';
import { globalEventBus } from '@/infrastructure/event-bus';
import { notificationService } from '@/shared/notification-system';
import { createLogger } from '@/shared/utils/logger';
import { getAiErrorPresentation } from '@/shared/ai-errors/aiErrorPresenter';
import { confirmWarning } from '@/component-library/components/ConfirmDialog/confirmService';
import './DeepReviewActionBar.scss';

const log = createLogger('DeepReviewActionBar');

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
    customInstructions,
    errorMessage,
    interruption,
  } = store;

  const [showCustomInput, setShowCustomInput] = useState(false);
  const [showRemediationList, setShowRemediationList] = useState(false);

  const selectedCount = selectedRemediationIds.size;
  const totalCount = remediationItems.length;
  const allSelected = totalCount > 0 && selectedCount === totalCount;
  const isFixDisabled = activeAction !== null || selectedCount === 0;
  const isDeepReview = reviewMode === 'deep';
  const hasInterruption = isDeepReview && Boolean(interruption);

  const phaseConfig = PHASE_CONFIG[phase];
  const PhaseIcon = phaseConfig.icon;

  const handleToggleRemediation = useCallback((id: string) => {
    store.toggleRemediation(id);
  }, [store]);

  const handleToggleAll = useCallback(() => {
    store.toggleAllRemediation();
  }, [store]);

  const handleStartFixing = useCallback(async (rerunReview: boolean) => {
    if (!reviewData || !childSessionId) return;

    const action = rerunReview ? 'fix-review' : 'fix';
    let prompt = buildSelectedReviewRemediationPrompt({
      reviewData,
      selectedIds: selectedRemediationIds,
      rerunReview,
      reviewMode,
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
  }, [reviewData, childSessionId, selectedRemediationIds, customInstructions, reviewMode, isDeepReview, store, t]);

  const handleFillBackInput = useCallback(() => {
    if (!reviewData) return;

    let prompt = buildSelectedReviewRemediationPrompt({
      reviewData,
      selectedIds: selectedRemediationIds,
      rerunReview: false,
      reviewMode,
    });

    if (customInstructions.trim()) {
      prompt = `${prompt}\n\n## User Instructions\n${customInstructions.trim()}`;
    }

    if (!prompt) return;

    globalEventBus.emit('fill-chat-input', {
      content: prompt,
      mode: 'replace',
    });

    store.dismiss();
  }, [reviewData, selectedRemediationIds, customInstructions, reviewMode, store]);

  const handleDismiss = useCallback(() => {
    store.dismiss();
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

  const handleCopyDiagnostics = useCallback(() => {
    const detail = interruption?.errorDetail;
    if (!detail) return;

    const presentation = getAiErrorPresentation(detail);

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
    if (detail.providerMessage) lines.push(`  - provider message: ${detail.providerMessage}`);
    if (detail.httpStatus) lines.push(`  - HTTP status: ${detail.httpStatus}`);
    if (detail.requestId) lines.push(`  - request ID: ${detail.requestId}`);
    if (detail.rawMessage) {
      lines.push(`  - raw message: ${detail.rawMessage}`);
    }

    const diagnostics = lines.join('\n');
    void navigator.clipboard?.writeText(diagnostics);
    notificationService.success(t('deepReviewActionBar.diagnosticsCopied', {
      defaultValue: 'Diagnostics copied',
    }), { duration: 2500 });
  }, [interruption, t]);

  const phaseTitle = useMemo(() => {
    switch (phase) {
      case 'review_completed':
        return t(isDeepReview ? 'reviewActionBar.reviewCompletedDeep' : 'reviewActionBar.reviewCompletedStandard', {
          defaultValue: isDeepReview ? 'Deep review completed' : 'Review completed',
        });
      case 'fix_running':
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
  }, [phase, isDeepReview, t]);

  if (dismissed || phase === 'idle' || !childSessionId) {
    return null;
  }

  return (
    <div className={`deep-review-action-bar deep-review-action-bar--${phaseConfig.variant}`}>
      <button
        type="button"
        className="deep-review-action-bar__close"
        onClick={handleDismiss}
        aria-label={t('deepReviewActionBar.dismiss', { defaultValue: 'Dismiss' })}
      >
        <X size={14} />
      </button>

      {/* Phase status header */}
      <div className="deep-review-action-bar__status">
        <PhaseIcon
          size={16}
          className={`deep-review-action-bar__icon ${phaseConfig.iconClass}`}
        />
        <span className="deep-review-action-bar__status-title">{phaseTitle}</span>
        {errorMessage && (
          <span className="deep-review-action-bar__error-message">{errorMessage}</span>
        )}
      </div>

      {/* Remediation selection (only when review completed) */}
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
            {showRemediationList ? <ChevronUp size={12} /> : <ChevronDown size={12} />}
          </button>

          {showRemediationList && (
            <div className="deep-review-action-bar__remediation-list">
              {remediationItems.map((item: ReviewRemediationItem) => (
                <label key={item.id} className="deep-review-action-bar__remediation-item">
                  <Checkbox
                    checked={selectedRemediationIds.has(item.id)}
                    onChange={() => handleToggleRemediation(item.id)}
                    size="small"
                  />
                  <span className="deep-review-action-bar__remediation-text" title={item.plan}>
                    {item.requiresDecision && (
                      <span className="deep-review-action-bar__remediation-tag">
                        {t('reviewActionBar.needsDecisionTag', { defaultValue: 'Decision' })}
                      </span>
                    )}
                    {item.plan}
                  </span>
                </label>
              ))}
            </div>
          )}

          {selectedCount === 0 && (
            <div className="deep-review-action-bar__empty-selection">
              {t('toolCards.codeReview.remediationActions.noSelectionHint', {
                defaultValue: 'Select at least one remediation item to start fixing.',
              })}
            </div>
          )}
        </div>
      )}

      {/* Custom instructions input */}
      {phase === 'review_completed' && (
        <div className="deep-review-action-bar__custom">
          <button
            type="button"
            className="deep-review-action-bar__custom-toggle"
            onClick={() => setShowCustomInput(!showCustomInput)}
          >
            <MessageSquare size={12} />
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
        {phase === 'review_completed' && (
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
                onClick={handleFillBackInput}
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
              disabled={activeAction !== null}
              onClick={() => void handleContinueReview()}
            >
              <Play size={13} />
              {t('deepReviewActionBar.resumeReview', { defaultValue: 'Continue review' })}
            </Button>
            <Button
              variant="ghost"
              size="small"
              onClick={handleCopyDiagnostics}
            >
              <Copy size={13} />
              {t('deepReviewActionBar.copyDiagnostics', { defaultValue: 'Copy diagnostics' })}
            </Button>
          </>
        )}

        {(phase === 'fix_completed' || phase === 'fix_failed' || phase === 'fix_timeout' || phase === 'review_error' || phase === 'resume_failed') && (
          <Button
            variant="ghost"
            size="small"
            onClick={handleDismiss}
          >
            {t('deepReviewActionBar.close', { defaultValue: 'Close' })}
          </Button>
        )}
      </div>
    </div>
  );
};

export const DeepReviewActionBar = ReviewActionBar;
