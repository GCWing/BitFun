import React from 'react';
import { useTranslation } from 'react-i18next';
import { AlertTriangle, Copy, Eye, Play, RotateCcw } from 'lucide-react';
import { Button, Tooltip } from '@/component-library';
import type { ReviewActionPhase } from '../../store/deepReviewActionBarStore';
import { CodeReviewReportExportActions } from '../../tool-cards/CodeReviewReportExportActions';

type ExportableReviewData = React.ComponentProps<typeof CodeReviewReportExportActions>['reviewData'];
type ExportableRunManifest = React.ComponentProps<typeof CodeReviewReportExportActions>['runManifest'];

interface ReviewActionControlsProps {
  phase: ReviewActionPhase;
  isDeepReview: boolean;
  retryableSliceCount: number;
  remediationItemCount: number;
  hasInterruption: boolean;
  partialResultsAvailable: boolean;
  activeAction: 'fix' | 'fix-review' | 'resume' | 'retry' | null;
  isFixDisabled: boolean;
  isResumeRunning: boolean;
  remainingFixIds: string[];
  modelRecoveryAction: 'switch_model' | 'open_model_settings' | null;
  reviewData?: ExportableReviewData | null;
  runManifest?: ExportableRunManifest;
  onRetryIncompleteSlices: () => void | Promise<void>;
  onStartFixing: (rerunReview: boolean) => void | Promise<void>;
  onFillBackInput: () => void | Promise<void>;
  onContinueReview: () => void | Promise<void>;
  onOpenModelSettings: () => void | Promise<void>;
  onCopyDiagnostics: () => void | Promise<void>;
  onViewPartialResults: () => void;
  onContinueFix: () => void | Promise<void>;
  onSkipRemainingFixes: () => void;
  onMinimize: () => void;
}

export const ReviewActionControls: React.FC<ReviewActionControlsProps> = ({
  phase,
  isDeepReview,
  retryableSliceCount,
  remediationItemCount,
  hasInterruption,
  partialResultsAvailable,
  activeAction,
  isFixDisabled,
  isResumeRunning,
  remainingFixIds,
  modelRecoveryAction,
  reviewData,
  runManifest,
  onRetryIncompleteSlices,
  onStartFixing,
  onFillBackInput,
  onContinueReview,
  onOpenModelSettings,
  onCopyDiagnostics,
  onViewPartialResults,
  onContinueFix,
  onSkipRemainingFixes,
  onMinimize,
}) => {
  const { t } = useTranslation('flow-chat');

  return (
    <div className="deep-review-action-bar__actions">
      {phase === 'review_completed' && isDeepReview && retryableSliceCount > 0 && (
        <Button
          variant="secondary"
          size="small"
          isLoading={activeAction === 'retry'}
          disabled={activeAction !== null}
          onClick={() => void onRetryIncompleteSlices()}
        >
          <RotateCcw size={14} />
          {t('deepReviewActionBar.retryIncompleteSlices', {
            count: retryableSliceCount,
          })}
        </Button>
      )}
      {phase === 'review_completed' && remediationItemCount > 0 && (
        <>
          <Button
            variant="primary"
            size="small"
            isLoading={activeAction === 'fix'}
            disabled={isFixDisabled}
            onClick={() => void onStartFixing(false)}
          >
            {t('toolCards.codeReview.remediationActions.startFix')}
          </Button>
          <Button
            variant="secondary"
            size="small"
            isLoading={activeAction === 'fix-review'}
            disabled={isFixDisabled}
            onClick={() => void onStartFixing(true)}
          >
            {t('toolCards.codeReview.remediationActions.fixAndReview')}
          </Button>
          <Tooltip content={t('deepReviewActionBar.fillBackInputHint')}>
            <Button
              variant="ghost"
              size="small"
              disabled={isFixDisabled}
              onClick={() => void onFillBackInput()}
            >
              {t('deepReviewActionBar.fillBackInput')}
            </Button>
          </Tooltip>
        </>
      )}

      {phase === 'review_completed' && reviewData && (
        <CodeReviewReportExportActions
          reviewData={reviewData}
          runManifest={runManifest}
          actions={['open']}
          variant="footer"
        />
      )}

      {hasInterruption && (
        <>
          <Button
            variant="primary"
            size="small"
            isLoading={activeAction === 'resume'}
            disabled={activeAction !== null || isResumeRunning}
            onClick={() => void onContinueReview()}
          >
            <Play size={14} />
            {t('deepReviewActionBar.resumeReview')}
          </Button>
          {modelRecoveryAction && (
            <Button
              variant="secondary"
              size="small"
              disabled={activeAction !== null}
              onClick={() => void onOpenModelSettings()}
            >
              {modelRecoveryAction === 'switch_model'
                ? t('deepReviewActionBar.switchModel')
                : t('deepReviewActionBar.openModelSettings')}
            </Button>
          )}
          <Button
            variant="ghost"
            size="small"
            onClick={() => void onCopyDiagnostics()}
          >
            <Copy size={14} />
            {t('deepReviewActionBar.copyDiagnostics')}
          </Button>
          {partialResultsAvailable && (
            <Button
              variant="ghost"
              size="small"
              onClick={onViewPartialResults}
            >
              <Eye size={14} />
              {t('deepReviewActionBar.viewPartialResults')}
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
                count: remainingFixIds.length,
              })}
            </span>
          </div>
          <Button
            variant="primary"
            size="small"
            onClick={() => void onContinueFix()}
          >
            <Play size={14} />
            {t('deepReviewActionBar.continueFix', {
              count: remainingFixIds.length,
            })}
          </Button>
          <Button
            variant="secondary"
            size="small"
            onClick={onSkipRemainingFixes}
          >
            {t('deepReviewActionBar.skipRemaining')}
          </Button>
        </>
      )}

      {(phase === 'fix_completed' || phase === 'fix_failed' || phase === 'fix_timeout' || phase === 'review_error') && (
        <Button
          variant="ghost"
          size="small"
          onClick={onMinimize}
        >
          {t('deepReviewActionBar.minimize')}
        </Button>
      )}
    </div>
  );
};
