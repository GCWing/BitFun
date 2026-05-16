import React from 'react';
import { useTranslation } from 'react-i18next';
import { AlertTriangle } from 'lucide-react';
import { Button } from '@/component-library';
import type { ReviewRemediationItem } from '../../utils/codeReviewRemediation';

interface DecisionExecutionGateProps {
  items: ReviewRemediationItem[];
  decisionSelections: Record<string, number>;
  customInstructions: string;
  rerunReview: boolean;
  confirmDisabled: boolean;
  onSelectDecision: (itemId: string, optionIndex: number) => void;
  onCustomInstructionsChange: (value: string) => void;
  onConfirm: () => void | Promise<void>;
  onCancel: () => void;
}

export const DecisionExecutionGate: React.FC<DecisionExecutionGateProps> = ({
  items,
  decisionSelections,
  customInstructions,
  rerunReview,
  confirmDisabled,
  onSelectDecision,
  onCustomInstructionsChange,
  onConfirm,
  onCancel,
}) => {
  const { t } = useTranslation('flow-chat');

  return (
    <div className="deep-review-action-bar__decision-gate" role="dialog" aria-modal="false">
      <div className="deep-review-action-bar__decision-gate-header">
        <AlertTriangle size={16} className="deep-review-action-bar__decision-gate-icon" />
        <div>
          <div className="deep-review-action-bar__decision-gate-title">
            {t('deepReviewActionBar.decisionGate.title', {
              defaultValue: 'Confirm decision items before fixing',
            })}
          </div>
          <div className="deep-review-action-bar__decision-gate-desc">
            {t('deepReviewActionBar.decisionGate.description', {
              defaultValue: 'Choose how each selected decision should be handled before BitFun starts editing.',
            })}
          </div>
        </div>
      </div>

      <div className="deep-review-action-bar__decision-gate-items">
        {items.map((item) => {
          const decision = item.decisionContext;
          const options = decision?.options ?? [];
          const selectedOption = decisionSelections[item.id];

          return (
            <section key={item.id} className="deep-review-action-bar__decision-card">
              <div className="deep-review-action-bar__decision-question">
                {decision?.question ?? item.plan}
              </div>
              {decision?.plan && decision.plan !== decision.question && (
                <div className="deep-review-action-bar__decision-plan">
                  {decision.plan}
                </div>
              )}
              {decision?.tradeoffs && (
                <div className="deep-review-action-bar__decision-tradeoffs">
                  {decision.tradeoffs}
                </div>
              )}
              {options.length > 0 ? (
                <div className="deep-review-action-bar__decision-gate-options">
                  {options.map((option, optionIndex) => {
                    const isSelected = selectedOption === optionIndex;
                    const isRecommended = decision?.recommendation === optionIndex;
                    return (
                      <button
                        key={optionIndex}
                        type="button"
                        className={`deep-review-action-bar__decision-gate-option ${
                          isSelected ? 'is-selected' : ''
                        } ${isRecommended ? 'is-recommended' : ''}`}
                        onClick={() => onSelectDecision(item.id, optionIndex)}
                      >
                        <span className="deep-review-action-bar__decision-gate-option-marker">
                          {isSelected ? '\u25CF' : '\u25CB'}
                        </span>
                        <span className="deep-review-action-bar__decision-gate-option-text">
                          {option}
                          {isRecommended ? ` (${t('toolCards.codeReview.remediationActions.recommended', { defaultValue: 'recommended' })})` : ''}
                        </span>
                      </button>
                    );
                  })}
                </div>
              ) : (
                <div className="deep-review-action-bar__decision-gate-note">
                  {t('deepReviewActionBar.decisionGate.noOptionsHint', {
                    defaultValue: 'This item has no structured options. Confirm only if you want the selected plan to be executed as written.',
                  })}
                </div>
              )}
            </section>
          );
        })}
      </div>

      <label className="deep-review-action-bar__decision-gate-supplement">
        <span>
          {t('deepReviewActionBar.decisionGate.supplementLabel', {
            defaultValue: 'Additional requirements',
          })}
        </span>
        <textarea
          value={customInstructions}
          onChange={(event) => onCustomInstructionsChange(event.target.value)}
          placeholder={t('deepReviewActionBar.decisionGate.supplementPlaceholder', {
            defaultValue: 'Optional: add constraints, scope limits, or implementation preferences before starting.',
          })}
          rows={2}
        />
      </label>

      {confirmDisabled && (
        <div className="deep-review-action-bar__decision-gate-warning" role="note">
          {t('deepReviewActionBar.decisionGate.missingSelection', {
            defaultValue: 'Choose an option for each selected decision item before starting.',
          })}
        </div>
      )}

      <div className="deep-review-action-bar__decision-gate-actions">
        <Button
          variant="primary"
          size="small"
          disabled={confirmDisabled}
          onClick={() => void onConfirm()}
        >
          {t(rerunReview
            ? 'deepReviewActionBar.decisionGate.confirmFixAndReview'
            : 'deepReviewActionBar.decisionGate.confirmFix', {
            defaultValue: rerunReview ? 'Confirm and re-review' : 'Confirm and start',
          })}
        </Button>
        <Button
          variant="secondary"
          size="small"
          onClick={onCancel}
        >
          {t('deepReviewActionBar.decisionGate.cancel', {
            defaultValue: 'Back to selection',
          })}
        </Button>
      </div>
    </div>
  );
};
