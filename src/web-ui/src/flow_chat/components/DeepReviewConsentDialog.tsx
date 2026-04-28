import React, { useCallback, useState } from 'react';
import { AlertTriangle, Clock, Coins, ShieldCheck, Users, X } from 'lucide-react';
import { estimateTokenConsumption, formatTokenCount } from '../utils/deepReviewExperience';
import { useTranslation } from 'react-i18next';
import { Button, Checkbox, Modal } from '@/component-library';
import { createLogger } from '@/shared/utils/logger';
import type {
  ReviewStrategyLevel,
  ReviewTeamManifestMember,
  ReviewTeamManifestMemberReason,
  ReviewTeamRunManifest,
} from '@/shared/services/reviewTeamService';
import {
  REVIEW_STRATEGY_LEVELS,
  getActiveReviewTeamManifestMembers,
  getReviewStrategyProfile,
  saveReviewTeamProjectStrategyOverride,
} from '@/shared/services/reviewTeamService';
import './DeepReviewConsentDialog.scss';

const log = createLogger('DeepReviewConsentDialog');
const SKIP_DEEP_REVIEW_CONFIRMATION_STORAGE_KEY = 'bitfun.deepReview.skipCostConfirmation';

interface PendingConsent {
  resolve: (confirmed: boolean) => void;
  preview?: ReviewTeamRunManifest;
}

export interface DeepReviewConsentControls {
  confirmDeepReviewLaunch: (preview?: ReviewTeamRunManifest) => Promise<boolean>;
  deepReviewConsentDialog: React.ReactNode;
}

function hasSkippedReviewers(preview?: ReviewTeamRunManifest): boolean {
  return Boolean(preview?.skippedReviewers?.length);
}

function getReviewerLabel(member: ReviewTeamManifestMember): string {
  return member.displayName || member.subagentId;
}

export function useDeepReviewConsent(): DeepReviewConsentControls {
  const { t } = useTranslation('flow-chat');
  const [pendingConsent, setPendingConsent] = useState<PendingConsent | null>(null);
  const [dontShowAgain, setDontShowAgain] = useState(false);
  const [selectedStrategyOverride, setSelectedStrategyOverride] =
    useState<ReviewStrategyLevel | null>(null);
  const [strategySelectionTouched, setStrategySelectionTouched] = useState(false);

  const confirmDeepReviewLaunch = useCallback(async (preview?: ReviewTeamRunManifest) => {
    try {
      if (
        localStorage.getItem(SKIP_DEEP_REVIEW_CONFIRMATION_STORAGE_KEY) === 'true' &&
        !hasSkippedReviewers(preview)
      ) {
        return true;
      }
    } catch (error) {
      log.warn('Failed to read Deep Review confirmation preference from local storage', error);
    }

    return new Promise<boolean>((resolve) => {
      setDontShowAgain(false);
      setSelectedStrategyOverride(null);
      setStrategySelectionTouched(false);
      setPendingConsent({ resolve, preview });
    });
  }, []);

  const settleConsent = useCallback(async (confirmed: boolean) => {
    const pending = pendingConsent;
    if (!pending) {
      return;
    }

    if (
      confirmed &&
      strategySelectionTouched &&
      pending.preview?.workspacePath
    ) {
      try {
        await saveReviewTeamProjectStrategyOverride(
          pending.preview.workspacePath,
          selectedStrategyOverride ?? undefined,
        );
      } catch (error) {
        log.warn('Failed to persist Deep Review project strategy override', error);
      }
    }

    if (confirmed && dontShowAgain) {
      try {
        localStorage.setItem(SKIP_DEEP_REVIEW_CONFIRMATION_STORAGE_KEY, 'true');
      } catch (error) {
        log.warn('Failed to persist Deep Review confirmation preference to local storage', error);
      }
    }

    setPendingConsent(null);
    pending.resolve(confirmed);
  }, [dontShowAgain, pendingConsent, selectedStrategyOverride, strategySelectionTouched]);

  const selectStrategyOverride = useCallback((strategyLevel: ReviewStrategyLevel | null) => {
    setSelectedStrategyOverride(strategyLevel);
    setStrategySelectionTouched(true);
  }, []);

  const getSkippedReasonLabel = useCallback((reason?: ReviewTeamManifestMemberReason) => {
    switch (reason) {
      case 'not_applicable':
        return t('deepReviewConsent.skippedReasons.notApplicable', {
          defaultValue: 'Not applicable to this target',
        });
      case 'budget_limited':
        return t('deepReviewConsent.skippedReasons.budgetLimited', {
          defaultValue: 'Limited by token budget',
        });
      case 'invalid_tooling':
        return t('deepReviewConsent.skippedReasons.invalidTooling', {
          defaultValue: 'Configuration issue',
        });
      case 'disabled':
        return t('deepReviewConsent.skippedReasons.disabled', {
          defaultValue: 'Disabled',
        });
      case 'unavailable':
        return t('deepReviewConsent.skippedReasons.unavailable', {
          defaultValue: 'Unavailable',
        });
      default:
        return t('deepReviewConsent.skippedReasons.skipped', {
          defaultValue: 'Skipped',
        });
    }
  }, [t]);

  const renderLineupPreview = useCallback((preview: ReviewTeamRunManifest) => {
    const activeReviewers = getActiveReviewTeamManifestMembers(preview);
    const skippedReviewers = preview.skippedReviewers;
    const activeCount = activeReviewers.length;
    const skippedCount = skippedReviewers.length;
    const selectedStrategy = strategySelectionTouched
      ? selectedStrategyOverride
      : preview.strategyLevel;
    const selectedStrategyLabel = selectedStrategy
      ? t(`deepReviewConsent.strategyLabels.${selectedStrategy}`, {
        defaultValue: getReviewStrategyProfile(selectedStrategy).label,
      })
      : t('deepReviewConsent.teamDefaultStrategy', {
        defaultValue: 'Team default',
      });

    return (
      <div className="deep-review-consent__lineup">
        <div className="deep-review-consent__lineup-header">
          <div>
            <span className="deep-review-consent__fact-title">
              {t('deepReviewConsent.lineupTitle', { defaultValue: 'Review lineup' })}
            </span>
            <p>
              {t('deepReviewConsent.lineupBody', {
                defaultValue: 'This run will use the manifest below before spending review tokens.',
              })}
            </p>
          </div>
          <div className="deep-review-consent__fact-icon">
            <Users size={16} />
          </div>
        </div>

        <div className="deep-review-consent__lineup-stats">
          <span>
            {t('deepReviewConsent.estimatedCalls', {
              count: preview.tokenBudget.estimatedReviewerCalls,
              defaultValue: '{{count}} reviewer calls',
            })}
          </span>
          <span>
            {t('deepReviewConsent.activeReviewers', {
              count: activeCount,
              defaultValue: '{{count}} active',
            })}
          </span>
          <span>
            {t('deepReviewConsent.skippedReviewers', {
              count: skippedCount,
              defaultValue: '{{count}} skipped',
            })}
          </span>
          <span>
            {t('deepReviewConsent.runStrategy', {
              strategy: selectedStrategyLabel,
              defaultValue: 'Run strategy: {{strategy}}',
            })}
          </span>
          {preview.strategyRecommendation && (
            <span>
              {t('deepReviewConsent.recommendedStrategy', {
                strategy: preview.strategyRecommendation.strategyLevel,
                defaultValue: 'Recommended strategy: {{strategy}}',
              })}
            </span>
          )}
        </div>

        {preview.strategyRecommendation && (
          <div className="deep-review-consent__reviewer-group">
            <div className="deep-review-consent__reviewer-group-title">
              {t('deepReviewConsent.recommendationTitle', {
                defaultValue: 'Risk recommendation',
              })}
            </div>
            <p>{preview.strategyRecommendation.rationale}</p>
          </div>
        )}

        {preview.workspacePath && (
          <div className="deep-review-consent__strategy-control">
            <div>
              <div className="deep-review-consent__reviewer-group-title">
                {t('deepReviewConsent.strategyOverrideTitle', {
                  defaultValue: 'Run strategy',
                })}
              </div>
              <p>
                {t('deepReviewConsent.strategyOverrideBody', {
                  defaultValue: 'Choose a project-specific strategy for this launch.',
                })}
              </p>
            </div>
            <div
              className="deep-review-consent__strategy-options"
              role="group"
              aria-label={t('deepReviewConsent.strategyOverrideTitle', {
                defaultValue: 'Run strategy',
              })}
            >
              <button
                type="button"
                className={[
                  'deep-review-consent__strategy-option',
                  strategySelectionTouched && selectedStrategyOverride === null
                    ? 'deep-review-consent__strategy-option--active'
                    : '',
                ].filter(Boolean).join(' ')}
                aria-pressed={strategySelectionTouched && selectedStrategyOverride === null}
                onClick={() => selectStrategyOverride(null)}
              >
                {t('deepReviewConsent.teamDefaultStrategy', {
                  defaultValue: 'Team default',
                })}
              </button>
              {REVIEW_STRATEGY_LEVELS.map((strategyLevel) => {
                const isActive = selectedStrategy === strategyLevel;
                return (
                  <button
                    key={strategyLevel}
                    type="button"
                    className={[
                      'deep-review-consent__strategy-option',
                      isActive ? 'deep-review-consent__strategy-option--active' : '',
                    ].filter(Boolean).join(' ')}
                    aria-pressed={isActive}
                    onClick={() => selectStrategyOverride(strategyLevel)}
                  >
                    {t(`deepReviewConsent.strategyLabels.${strategyLevel}`, {
                      defaultValue: getReviewStrategyProfile(strategyLevel).label,
                    })}
                  </button>
                );
              })}
            </div>
          </div>
        )}

        {activeReviewers.length > 0 && (
          <div className="deep-review-consent__reviewer-group">
            <div className="deep-review-consent__reviewer-group-title">
              {t('deepReviewConsent.activeGroupTitle', { defaultValue: 'Will run' })}
            </div>
            <div className="deep-review-consent__reviewer-chips">
              {activeReviewers.map((member) => (
                <span key={`active-${member.subagentId}`} className="deep-review-consent__reviewer-chip">
                  {getReviewerLabel(member)}
                </span>
              ))}
            </div>
          </div>
        )}

        {skippedReviewers.length > 0 && (
          <div className="deep-review-consent__reviewer-group">
            <div className="deep-review-consent__reviewer-group-title deep-review-consent__reviewer-group-title--warning">
              <AlertTriangle size={13} />
              {t('deepReviewConsent.skippedGroupTitle', { defaultValue: 'Skipped reviewers' })}
            </div>
            <ul className="deep-review-consent__skipped-list">
              {skippedReviewers.map((member) => (
                <li key={`skipped-${member.subagentId}`}>
                  <span>{getReviewerLabel(member)}</span>
                  <strong>{getSkippedReasonLabel(member.reason)}</strong>
                </li>
              ))}
            </ul>
          </div>
        )}
      </div>
    );
  }, [
    getSkippedReasonLabel,
    selectStrategyOverride,
    selectedStrategyOverride,
    strategySelectionTouched,
    t,
  ]);

  const deepReviewConsentDialog = pendingConsent ? (
    <Modal
      isOpen={true}
      onClose={() => void settleConsent(false)}
      size="large"
      closeOnOverlayClick={false}
      showCloseButton={false}
      contentClassName="deep-review-consent-modal"
    >
      <div className="deep-review-consent">
        <div className="deep-review-consent__header">
          <div className="deep-review-consent__heading">
            <span className="deep-review-consent__eyebrow">
              {t('deepReviewConsent.eyebrow', { defaultValue: 'Code review team' })}
            </span>
            <h3>{t('deepReviewConsent.title')}</h3>
          </div>
          <button
            type="button"
            className="deep-review-consent__close"
            aria-label={t('deepReviewConsent.cancel')}
            onClick={() => void settleConsent(false)}
          >
            <X size={16} />
          </button>
        </div>

        <p className="deep-review-consent__lead">{t('deepReviewConsent.body')}</p>

        <div className="deep-review-consent__safety-note">
          <div className="deep-review-consent__fact-icon">
            <ShieldCheck size={16} />
          </div>
          <div>
            <span className="deep-review-consent__fact-title">
              {t('deepReviewConsent.readonlyLabel', { defaultValue: 'Read-only first pass' })}
            </span>
            <p>{t('deepReviewConsent.readonly')}</p>
          </div>
        </div>

        <div className="deep-review-consent__facts" aria-label={t('deepReviewConsent.windowTitle', { defaultValue: 'Deep Review' })}>
          <div className="deep-review-consent__fact">
            <div className="deep-review-consent__fact-icon">
              <Coins size={16} />
            </div>
            <div>
              <span className="deep-review-consent__fact-title">
                {t('deepReviewConsent.costLabel', { defaultValue: 'Higher token usage' })}
              </span>
              <p>{t('deepReviewConsent.cost')}</p>
              <p className="deep-review-consent__token-estimate">
                {(() => {
                  const est = estimateTokenConsumption(5);
                  return t('deepReviewConsent.estimatedTokens', {
                    min: formatTokenCount(est.min),
                    max: formatTokenCount(est.max),
                    defaultValue: 'Estimated: {{min}} - {{max}} tokens',
                  });
                })()}
              </p>
            </div>
          </div>
          <div className="deep-review-consent__fact">
            <div className="deep-review-consent__fact-icon">
              <Clock size={16} />
            </div>
            <div>
              <span className="deep-review-consent__fact-title">
                {t('deepReviewConsent.timeLabel', { defaultValue: 'Longer runtime' })}
              </span>
              <p>{t('deepReviewConsent.time')}</p>
            </div>
          </div>
        </div>

        {pendingConsent.preview && renderLineupPreview(pendingConsent.preview)}

        <div className="deep-review-consent__footer">
          <Checkbox
            className="deep-review-consent__checkbox"
            checked={dontShowAgain}
            onChange={(event) => setDontShowAgain(event.target.checked)}
            label={t('deepReviewConsent.dontShowAgain')}
          />
          <div className="deep-review-consent__actions">
            <Button
              variant="secondary"
              size="small"
              onClick={() => void settleConsent(false)}
            >
              {t('deepReviewConsent.cancel')}
            </Button>
            <Button
              variant="primary"
              size="small"
              onClick={() => void settleConsent(true)}
            >
              {t('deepReviewConsent.confirm')}
            </Button>
          </div>
        </div>
      </div>
    </Modal>
  ) : null;

  return {
    confirmDeepReviewLaunch,
    deepReviewConsentDialog,
  };
}
