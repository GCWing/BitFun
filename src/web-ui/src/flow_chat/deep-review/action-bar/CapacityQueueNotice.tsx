import React from 'react';
import { useTranslation } from 'react-i18next';
import {
  Clock,
  Pause,
  Play,
  SkipForward,
} from 'lucide-react';
import { Button } from '@/component-library';
import type {
  DeepReviewCapacityQueueReason,
  DeepReviewCapacityQueueState,
} from '../../store/deepReviewActionBarStore';
import { formatElapsedTime } from './actionBarFormatting';

interface CapacityQueueNoticeProps {
  capacityQueueState: DeepReviewCapacityQueueState;
  supportsInlineQueueControls: boolean;
  onPauseQueue: () => void | Promise<void>;
  onContinueQueue: () => void | Promise<void>;
  onSkipOptionalQueuedReviewers: () => void | Promise<void>;
  onCancelQueuedReviewers: () => void | Promise<void>;
  onOpenReviewSettings: () => void | Promise<void>;
}

const CAPACITY_QUEUE_REASON_KEYS: Record<DeepReviewCapacityQueueReason, string> = {
  provider_rate_limit: 'deepReviewActionBar.capacityQueue.reasons.providerRateLimit',
  provider_concurrency_limit: 'deepReviewActionBar.capacityQueue.reasons.providerConcurrencyLimit',
  retry_after: 'deepReviewActionBar.capacityQueue.reasons.retryAfter',
  local_concurrency_cap: 'deepReviewActionBar.capacityQueue.reasons.localConcurrencyCap',
  launch_batch_blocked: 'deepReviewActionBar.capacityQueue.reasons.launchBatchBlocked',
  temporary_overload: 'deepReviewActionBar.capacityQueue.reasons.temporaryOverload',
};

const CAPACITY_QUEUE_REASON_DETAIL_KEYS: Record<DeepReviewCapacityQueueReason, {
  key: string;
  defaultValue: string;
}> = {
  provider_rate_limit: {
    key: 'deepReviewActionBar.capacityQueue.reasonDetails.providerRateLimit',
    defaultValue: 'The model provider is rate-limiting requests. BitFun will wait briefly and continue when capacity returns.',
  },
  provider_concurrency_limit: {
    key: 'deepReviewActionBar.capacityQueue.reasonDetails.providerConcurrencyLimit',
    defaultValue: 'The model provider rejected another concurrent reviewer. BitFun will retry after capacity opens.',
  },
  retry_after: {
    key: 'deepReviewActionBar.capacityQueue.reasonDetails.retryAfter',
    defaultValue: 'The model provider asked BitFun to retry later. Waiting here avoids spending reviewer runtime while the provider cools down.',
  },
  local_concurrency_cap: {
    key: 'deepReviewActionBar.capacityQueue.reasonDetails.localConcurrencyCap',
    defaultValue: 'The configured Review Team reviewer limit is full. This reviewer will start after an active reviewer finishes.',
  },
  launch_batch_blocked: {
    key: 'deepReviewActionBar.capacityQueue.reasonDetails.launchBatchBlocked',
    defaultValue: 'An earlier launch batch is still running. Waiting preserves the planned review order and prevents a later batch from overtaking it.',
  },
  temporary_overload: {
    key: 'deepReviewActionBar.capacityQueue.reasonDetails.temporaryOverload',
    defaultValue: 'The model provider reported temporary overload. BitFun will wait briefly and then continue or mark the reviewer as capacity skipped.',
  },
};

type CapacityQueueWaitMode = 'active_reviewer' | 'provider_capacity' | 'generic';

function getCapacityQueueWaitMode(
  capacityQueueState: DeepReviewCapacityQueueState,
): CapacityQueueWaitMode {
  if (
    (capacityQueueState.reason === 'local_concurrency_cap'
      || capacityQueueState.reason === 'launch_batch_blocked')
    && (capacityQueueState.activeReviewerCount ?? 0) > 0
  ) {
    return 'active_reviewer';
  }

  if (
    capacityQueueState.reason === 'provider_rate_limit'
    || capacityQueueState.reason === 'provider_concurrency_limit'
    || capacityQueueState.reason === 'retry_after'
    || capacityQueueState.reason === 'temporary_overload'
  ) {
    return 'provider_capacity';
  }

  return 'generic';
}

function reviewerLabel(
  reviewer: NonNullable<DeepReviewCapacityQueueState['waitingReviewers']>[number],
): string {
  return reviewer.displayName || reviewer.subagentType || 'Reviewer';
}

export const CapacityQueueNotice: React.FC<CapacityQueueNoticeProps> = ({
  capacityQueueState,
  supportsInlineQueueControls,
  onPauseQueue,
  onContinueQueue,
  onSkipOptionalQueuedReviewers,
  onCancelQueuedReviewers,
  onOpenReviewSettings,
}) => {
  const { t } = useTranslation('flow-chat');
  const capacityQueueReasonLabel = capacityQueueState.reason
    ? t(CAPACITY_QUEUE_REASON_KEYS[capacityQueueState.reason], {
      defaultValue: capacityQueueState.reason.split('_').join(' '),
    })
    : null;
  const capacityQueueReasonDetail = capacityQueueState.reason
    ? t(CAPACITY_QUEUE_REASON_DETAIL_KEYS[capacityQueueState.reason].key, {
      defaultValue: CAPACITY_QUEUE_REASON_DETAIL_KEYS[capacityQueueState.reason].defaultValue,
    })
    : null;
  const capacityQueueElapsedLabel = capacityQueueState.queueElapsedMs !== undefined
    ? formatElapsedTime(capacityQueueState.queueElapsedMs)
    : null;
  const capacityQueueMaxWaitLabel = capacityQueueState.maxQueueWaitSeconds !== undefined
    ? formatElapsedTime(capacityQueueState.maxQueueWaitSeconds * 1000)
    : null;
  const capacityQueueWaitMode = getCapacityQueueWaitMode(capacityQueueState);
  const activeReviewerCount = capacityQueueState.activeReviewerCount ?? 0;
  const isLongLaunchBatchWait = capacityQueueState.reason === 'launch_batch_blocked'
    && activeReviewerCount > 0
    && capacityQueueState.queueElapsedMs !== undefined
    && capacityQueueState.maxQueueWaitSeconds !== undefined
    && capacityQueueState.queueElapsedMs > capacityQueueState.maxQueueWaitSeconds * 1000;
  const capacityQueueTitle = capacityQueueState.status === 'paused_by_user'
    ? t('deepReviewActionBar.capacityQueue.pausedTitle', {
      defaultValue: 'Queue paused',
    })
    : capacityQueueWaitMode === 'active_reviewer'
      ? t('deepReviewActionBar.capacityQueue.activeReviewerTitle', {
        defaultValue: 'Waiting for running reviewers',
      })
      : capacityQueueWaitMode === 'provider_capacity'
        ? t('deepReviewActionBar.capacityQueue.providerTitle', {
          defaultValue: 'Waiting for model capacity',
        })
        : t('deepReviewActionBar.capacityQueue.title', {
          defaultValue: 'Reviewers waiting for capacity',
        });
  const capacityQueueDetail = capacityQueueWaitMode === 'active_reviewer'
    ? t('deepReviewActionBar.capacityQueue.activeReviewerDetail', {
      defaultValue: 'Queued reviewers start when a running reviewer frees capacity. Queue wait does not count against reviewer runtime.',
    })
    : capacityQueueWaitMode === 'provider_capacity'
      ? t('deepReviewActionBar.capacityQueue.providerDetail', {
        defaultValue: 'BitFun is waiting for temporary model capacity. This wait does not count against reviewer runtime.',
      })
      : t('deepReviewActionBar.capacityQueue.detail', {
        defaultValue: 'Queue wait does not count against reviewer runtime.',
      });
  const waitingReviewers = capacityQueueState.waitingReviewers ?? [];
  const showCapacityQueueMeta = Boolean(
    capacityQueueReasonLabel
      || capacityQueueElapsedLabel
      || capacityQueueWaitMode === 'active_reviewer',
  );

  return (
    <div className="deep-review-action-bar__capacity-queue" aria-live="polite">
      <div className="deep-review-action-bar__capacity-queue-main">
        <Clock size={14} className="deep-review-action-bar__capacity-queue-icon" />
        <div className="deep-review-action-bar__capacity-queue-copy">
          <span className="deep-review-action-bar__capacity-queue-title">
            {capacityQueueTitle}
          </span>
          <span className="deep-review-action-bar__capacity-queue-detail">
            {capacityQueueDetail}
          </span>
          {showCapacityQueueMeta && (
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
                  {capacityQueueMaxWaitLabel && capacityQueueWaitMode !== 'active_reviewer'
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
              {capacityQueueWaitMode === 'active_reviewer' && activeReviewerCount > 0 && (
                <span className="deep-review-action-bar__capacity-queue-chip">
                  {t('deepReviewActionBar.capacityQueue.activeReviewerCount', {
                    count: activeReviewerCount,
                    defaultValue: `Running reviewers: ${activeReviewerCount}`,
                  })}
                </span>
              )}
            </span>
          )}
          {capacityQueueReasonDetail && (
            <span className="deep-review-action-bar__capacity-queue-detail">
              {capacityQueueReasonDetail}
            </span>
          )}
          {isLongLaunchBatchWait && (
            <span className="deep-review-action-bar__capacity-queue-detail">
              {t('deepReviewActionBar.capacityQueue.longLaunchBatchWaitDetail', {
                defaultValue: 'This reviewer has waited longer than the configured queue window because an earlier reviewer batch is still running. You can keep waiting, pause the queue, cancel queued reviewers, or open Review settings.',
              })}
            </span>
          )}
          {capacityQueueState.sessionConcurrencyHigh && (
            <span className="deep-review-action-bar__capacity-queue-detail">
              {t('deepReviewActionBar.capacityQueue.sessionBusy', {
                defaultValue: 'Your active session is busy. Pause Deep Review or continue later.',
              })}
            </span>
          )}
          {waitingReviewers.length > 0 && (
            <div className="deep-review-action-bar__capacity-queue-reviewers">
              <span className="deep-review-action-bar__capacity-queue-reviewers-title">
                {t('deepReviewActionBar.capacityQueue.waitingReviewersTitle', {
                  defaultValue: 'Waiting reviewers',
                })}
              </span>
              <div className="deep-review-action-bar__capacity-queue-reviewer-list">
                {waitingReviewers.map((reviewer) => {
                  const label = reviewerLabel(reviewer);
                  const reviewerElapsed = reviewer.queueElapsedMs !== undefined
                    ? formatElapsedTime(reviewer.queueElapsedMs)
                    : null;
                  const statusLabel = reviewer.status === 'paused_by_user'
                    ? t('deepReviewActionBar.capacityQueue.reviewerStatusPaused', {
                      defaultValue: 'Paused',
                    })
                    : t('deepReviewActionBar.capacityQueue.reviewerStatusQueued', {
                      defaultValue: 'Waiting',
                    });
                  return (
                    <span
                      key={reviewer.toolId || reviewer.subagentType || label}
                      className="deep-review-action-bar__capacity-queue-reviewer"
                    >
                      <span className="deep-review-action-bar__capacity-queue-reviewer-name">
                        {label}
                      </span>
                      <span className="deep-review-action-bar__capacity-queue-reviewer-meta">
                        {statusLabel}
                        {reviewer.optional && (
                          <>
                            {' / '}
                            {t('deepReviewActionBar.capacityQueue.optionalReviewer', {
                              defaultValue: 'Optional',
                            })}
                          </>
                        )}
                        {reviewerElapsed && (
                          <>
                            {' / '}
                            {t('deepReviewActionBar.capacityQueue.elapsed', {
                              elapsed: reviewerElapsed,
                              defaultValue: `Waited ${reviewerElapsed}`,
                            })}
                          </>
                        )}
                      </span>
                    </span>
                  );
                })}
              </div>
            </div>
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
                onClick={() => void onContinueQueue()}
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
                onClick={() => void onPauseQueue()}
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
                onClick={() => void onSkipOptionalQueuedReviewers()}
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
              onClick={() => void onCancelQueuedReviewers()}
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
          onClick={() => void onOpenReviewSettings()}
        >
          {t('deepReviewActionBar.capacityQueue.openReviewSettings', {
            defaultValue: 'Open Review settings',
          })}
        </Button>
      </div>
    </div>
  );
};
