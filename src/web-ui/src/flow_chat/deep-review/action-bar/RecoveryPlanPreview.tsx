import React from 'react';
import { useTranslation } from 'react-i18next';
import {
  CheckCircle,
  RotateCcw,
  SkipForward,
} from 'lucide-react';
import type { RecoveryPlan } from '../../utils/deepReviewExperience';

interface RecoveryPlanPreviewProps {
  recoveryPlan: RecoveryPlan;
}

export const RecoveryPlanPreview: React.FC<RecoveryPlanPreviewProps> = ({
  recoveryPlan,
}) => {
  const { t } = useTranslation('flow-chat');

  return (
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
  );
};
