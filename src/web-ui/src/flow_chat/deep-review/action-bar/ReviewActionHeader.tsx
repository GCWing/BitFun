import React from 'react';
import { Minus } from 'lucide-react';
import { CodeReviewReportExportActions } from '../../tool-cards/CodeReviewReportExportActions';

type ExportableReviewData = React.ComponentProps<typeof CodeReviewReportExportActions>['reviewData'];

interface ReviewActionHeaderProps {
  reviewData?: ExportableReviewData | null;
  PhaseIcon: React.ComponentType<{
    size?: number | string;
    style?: React.CSSProperties;
    className?: string;
  }>;
  phaseIconClass: string;
  phaseTitle: string;
  errorMessage?: string | null;
  minimizeLabel: string;
  onMinimize: () => void;
}

export const ReviewActionHeader: React.FC<ReviewActionHeaderProps> = ({
  reviewData,
  PhaseIcon,
  phaseIconClass,
  phaseTitle,
  errorMessage,
  minimizeLabel,
  onMinimize,
}) => (
  <>
    <div className="deep-review-action-bar__controls">
      {reviewData && (
        <CodeReviewReportExportActions reviewData={reviewData} />
      )}
      <span className="deep-review-action-bar__controls-divider" />
      <button
        type="button"
        className="deep-review-action-bar__controls-btn"
        onClick={onMinimize}
        aria-label={minimizeLabel}
      >
        <Minus size={14} />
      </button>
    </div>

    <div className="deep-review-action-bar__status">
      <PhaseIcon
        size={18}
        className={`deep-review-action-bar__icon ${phaseIconClass}`}
      />
      <span className="deep-review-action-bar__status-title">{phaseTitle}</span>
      {errorMessage && (
        <span className="deep-review-action-bar__error-message">{errorMessage}</span>
      )}
    </div>
  </>
);
