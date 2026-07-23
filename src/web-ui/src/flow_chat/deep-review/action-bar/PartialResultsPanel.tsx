import React from 'react';
import { useTranslation } from 'react-i18next';
import { Eye } from 'lucide-react';
import type {
  PartialReviewData,
  ReviewerProgressSummary,
} from '../../utils/deepReviewExperience';

interface PartialResultsPanelProps {
  progressSummary: ReviewerProgressSummary | null;
  partialResults: PartialReviewData | null;
  showPartialResults: boolean;
  onTogglePartialResults: () => void;
}

export const PartialResultsPanel: React.FC<PartialResultsPanelProps> = ({
  progressSummary,
  partialResults,
  showPartialResults,
  onTogglePartialResults,
}) => {
  const { t } = useTranslation('flow-chat');
  const hasPartialDetails = Boolean(partialResults?.hasPartialResults);
  const showSummary = Boolean(progressSummary && progressSummary.completed > 0 && hasPartialDetails);

  return (
    <>
      {showSummary && progressSummary && (
        <div className="deep-review-action-bar__partial-summary">
          <span className="deep-review-action-bar__partial-count">
            {t('deepReviewActionBar.partialResultsDescription', {
              completed: progressSummary.completed,
              total: progressSummary.total,
            })}
          </span>
          <button
            type="button"
            className="deep-review-action-bar__partial-link"
            onClick={onTogglePartialResults}
          >
            <Eye size={12} />
            {showPartialResults
              ? t('deepReviewActionBar.hidePartialResults')
              : t('deepReviewActionBar.viewPartialResults')}
          </button>
        </div>
      )}

      {showPartialResults && partialResults && (
        <div className="deep-review-action-bar__partial-detail">
          {partialResults.completedIssues.length > 0 && (
            <div className="deep-review-action-bar__partial-section">
              <span className="deep-review-action-bar__partial-section-title">
                {t('deepReviewActionBar.partialIssues', {
                  count: partialResults.completedIssues.length,
                })}
              </span>
            </div>
          )}
          {partialResults.completedRemediationItems.length > 0 && (
            <div className="deep-review-action-bar__partial-section">
              <span className="deep-review-action-bar__partial-section-title">
                {t('deepReviewActionBar.partialRemediationItems', {
                  count: partialResults.completedRemediationItems.length,
                })}
              </span>
            </div>
          )}
          {partialResults.completedReviewerSummaries.length > 0 && (
            <div className="deep-review-action-bar__partial-section">
              <span className="deep-review-action-bar__partial-section-title">
                {t('deepReviewActionBar.partialReviewerSummaries', {
                  count: partialResults.completedReviewerSummaries.length,
                })}
              </span>
              <ul className="deep-review-action-bar__partial-list">
                {partialResults.completedReviewerSummaries.map((summary, index) => (
                  <li key={`${index}-${summary}`} className="deep-review-action-bar__partial-text">
                    {summary}
                  </li>
                ))}
              </ul>
            </div>
          )}
        </div>
      )}
    </>
  );
};
