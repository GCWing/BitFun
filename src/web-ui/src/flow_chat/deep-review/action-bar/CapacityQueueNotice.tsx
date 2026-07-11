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
    defaultValue: 'The model provider rejected another concurrent review request. BitFun will retry after capacity opens.',
  },
  retry_after: {
    key: 'deepReviewActionBar.capacityQueue.reasonDetails.retryAfter',
    defaultValue: 'The model provider asked BitFun to retry later. Waiting here avoids spending Review work while the provider cools down.',
  },
  local_concurrency_cap: {
    key: 'deepReviewActionBar.capacityQueue.reasonDetails.localConcurrencyCap',
    defaultValue: 'The configured strict review capacity is full. Waiting review work will start after active work finishes.',
  },
  launch_batch_blocked: {
    key: 'deepReviewActionBar.capacityQueue.reasonDetails.launchBatchBlocked',
    defaultValue: 'Earlier review work is still running. Waiting preserves the planned review order and prevents later work from overtaking it.',
  },
  temporary_overload: {
    key: 'deepReviewActionBar.capacityQueue.reasonDetails.temporaryOverload',
    defaultValue: 'The model provider reported temporary overload. BitFun will wait briefly and then continue or reduce coverage.',
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
    ? t('deepReviewActionBar.capacityQueue.pausedTitle')
    : capacityQueueWaitMode === 'active_reviewer'
      ? t('deepReviewActionBar.capacityQueue.activeReviewerTitle')
      : capacityQueueWaitMode === 'provider_capacity'
        ? t('deepReviewActionBar.capacityQueue.providerTitle')
        : t('deepReviewActionBar.capacityQueue.title');
  const capacityQueueDetail = capacityQueueWaitMode === 'active_reviewer'
    ? t('deepReviewActionBar.capacityQueue.activeReviewerDetail')
    : capacityQueueWaitMode === 'provider_capacity'
      ? t('deepReviewActionBar.capacityQueue.providerDetail')
      : t('deepReviewActionBar.capacityQueue.detail');
  const showCapacityQueueMeta = Boolean(
    capacityQueueReasonLabel
      || capacityQueueElapsedLabel,
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
                  })}
                </span>
              )}
              {capacityQueueElapsedLabel && (
                <span className="deep-review-action-bar__capacity-queue-chip">
                  {capacityQueueMaxWaitLabel && capacityQueueWaitMode !== 'active_reviewer'
                    ? t('deepReviewActionBar.capacityQueue.elapsedWithMax', {
                      elapsed: capacityQueueElapsedLabel,
                      max: capacityQueueMaxWaitLabel,
                    })
                    : t('deepReviewActionBar.capacityQueue.elapsed', {
                      elapsed: capacityQueueElapsedLabel,
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
              {t('deepReviewActionBar.capacityQueue.longLaunchBatchWaitDetail')}
            </span>
          )}
          {capacityQueueState.sessionConcurrencyHigh && (
            <span className="deep-review-action-bar__capacity-queue-detail">
              {t('deepReviewActionBar.capacityQueue.sessionBusy')}
            </span>
          )}
          {!supportsInlineQueueControls && (
            <span className="deep-review-action-bar__capacity-queue-detail">
              {t('deepReviewActionBar.capacityQueue.stopHint')}
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
                {t('deepReviewActionBar.capacityQueue.continueQueue')}
              </Button>
            ) : (
              <Button
                variant="secondary"
                size="small"
                onClick={() => void onPauseQueue()}
              >
                <Pause size={13} />
                {t('deepReviewActionBar.capacityQueue.pauseQueue')}
              </Button>
            )}
            {(capacityQueueState.optionalReviewerCount ?? 0) > 0 && (
              <Button
                variant="ghost"
                size="small"
                onClick={() => void onSkipOptionalQueuedReviewers()}
              >
                <SkipForward size={13} />
                {t('deepReviewActionBar.capacityQueue.skipOptionalQueued')}
              </Button>
            )}
            <Button
              variant="ghost"
              size="small"
              onClick={() => void onCancelQueuedReviewers()}
            >
              {t('deepReviewActionBar.capacityQueue.cancelQueued')}
            </Button>
          </>
        )}
        <Button
          variant="ghost"
          size="small"
          onClick={() => void onOpenReviewSettings()}
        >
          {t('deepReviewActionBar.capacityQueue.openReviewSettings')}
        </Button>
      </div>
    </div>
  );
};
