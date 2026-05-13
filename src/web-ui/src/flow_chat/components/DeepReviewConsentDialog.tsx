import React, { useCallback, useState } from 'react';
import { AlertTriangle, X } from 'lucide-react';
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
  getReviewStrategyProfile,
  saveReviewTeamProjectStrategyOverride,
} from '@/shared/services/reviewTeamService';
import type { DeepReviewSessionConcurrencyGuard } from '../utils/deepReviewCapacityGuard';
import './DeepReviewConsentDialog.scss';

const log = createLogger('DeepReviewConsentDialog');
const SKIP_DEEP_REVIEW_CONFIRMATION_STORAGE_KEY = 'bitfun.deepReview.skipCostConfirmation';
const MAX_VISIBLE_SKIPPED_REVIEWERS = 3;

interface PendingConsent {
  resolve: (confirmed: boolean) => void;
  preview?: ReviewTeamRunManifest;
  launchContext?: DeepReviewConsentLaunchContext;
}

export interface DeepReviewConsentLaunchContext {
  sessionConcurrencyGuard?: DeepReviewSessionConcurrencyGuard | null;
}

export interface DeepReviewConsentControls {
  confirmDeepReviewLaunch: (
    preview?: ReviewTeamRunManifest,
    launchContext?: DeepReviewConsentLaunchContext,
  ) => Promise<boolean>;
  deepReviewConsentDialog: React.ReactNode;
}

function hasSkippedReviewers(preview?: ReviewTeamRunManifest): boolean {
  return Boolean(preview?.skippedReviewers?.length);
}

function hasSessionConcurrencyWarning(launchContext?: DeepReviewConsentLaunchContext): boolean {
  return Boolean(launchContext?.sessionConcurrencyGuard?.highActivity);
}

function getReviewerLabel(member: ReviewTeamManifestMember): string {
  return member.displayName || member.subagentId;
}

function getReviewTargetFileCount(preview: ReviewTeamRunManifest): number {
  return preview.target.files.filter((file) => {
    if (typeof file === 'string') {
      return true;
    }
    return !file.excluded;
  }).length;
}

function getReviewTargetSummary(preview: ReviewTeamRunManifest, t: ReturnType<typeof useTranslation>['t']): string {
  const targetFileCount = getReviewTargetFileCount(preview);
  if (targetFileCount > 0) {
    return t('deepReviewConsent.targetFiles', {
      count: targetFileCount,
      defaultValue: targetFileCount === 1 ? '{{count}} file' : '{{count}} files',
    });
  }

  switch (preview.target.source) {
    case 'manual_prompt':
      return t('deepReviewConsent.targetSource.manualPrompt', {
        defaultValue: 'Provided context',
      });
    case 'workspace_diff':
      return t('deepReviewConsent.targetSource.workspaceDiff', {
        defaultValue: 'Workspace changes',
      });
    case 'slash_command_git_ref':
      return t('deepReviewConsent.targetSource.gitRef', {
        defaultValue: 'Git reference',
      });
    case 'slash_command_explicit_files':
    case 'session_files':
      return t('deepReviewConsent.targetSource.selectedContext', {
        defaultValue: 'Selected context',
      });
    case 'unknown':
    default:
      return t('deepReviewConsent.targetSource.reviewTarget', {
        defaultValue: 'Review target',
      });
  }
}

function getStrategyLabel(strategyLevel: ReviewStrategyLevel, t: ReturnType<typeof useTranslation>['t']): string {
  return t(`deepReviewConsent.strategyLabels.${strategyLevel}`, {
    defaultValue: getReviewStrategyProfile(strategyLevel).label,
  });
}

function getStrategySummary(strategyLevel: ReviewStrategyLevel, t: ReturnType<typeof useTranslation>['t']): string {
  return t(`deepReviewConsent.strategySummaries.${strategyLevel}`, {
    defaultValue: getReviewStrategyProfile(strategyLevel).summary,
  });
}

export function useDeepReviewConsent(): DeepReviewConsentControls {
  const { t } = useTranslation('flow-chat');
  const [pendingConsent, setPendingConsent] = useState<PendingConsent | null>(null);
  const [dontShowAgain, setDontShowAgain] = useState(false);
  const [selectedStrategyOverride, setSelectedStrategyOverride] =
    useState<ReviewStrategyLevel | null>(null);
  const [strategySelectionTouched, setStrategySelectionTouched] = useState(false);

  const confirmDeepReviewLaunch = useCallback(async (
    preview?: ReviewTeamRunManifest,
    launchContext?: DeepReviewConsentLaunchContext,
  ) => {
    try {
      if (
        localStorage.getItem(SKIP_DEEP_REVIEW_CONFIRMATION_STORAGE_KEY) === 'true' &&
        !hasSkippedReviewers(preview) &&
        !hasSessionConcurrencyWarning(launchContext)
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
      setPendingConsent({ resolve, preview, launchContext });
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

  const renderLaunchSummary = useCallback((preview: ReviewTeamRunManifest) => {
    const skippedReviewers = preview.skippedReviewers;
    const skippedCount = skippedReviewers.length;
    const visibleSkippedReviewers = skippedReviewers.slice(0, MAX_VISIBLE_SKIPPED_REVIEWERS);
    const hiddenSkippedCount = Math.max(0, skippedCount - visibleSkippedReviewers.length);
    const effectiveStrategy = selectedStrategyOverride ?? preview.strategyLevel;
    const selectedStrategyLabel = getStrategyLabel(effectiveStrategy, t);
    const targetSummary = getReviewTargetSummary(preview, t);
    return (
      <div className="deep-review-consent__summary">
        <div className="deep-review-consent__summary-header">
          <span className="deep-review-consent__fact-title">
            {t('deepReviewConsent.summaryTitle', { defaultValue: 'Launch summary' })}
          </span>
        </div>

        <div className="deep-review-consent__summary-stats">
          <span>{targetSummary}</span>
          {skippedCount > 0 && (
            <span className="deep-review-consent__summary-stat--warning">
              {t('deepReviewConsent.skippedReviewers', {
                count: skippedCount,
                defaultValue: '{{count}} skipped',
              })}
            </span>
          )}
        </div>

        {preview.workspacePath && (
          <div className="deep-review-consent__strategy-control">
            <div className="deep-review-consent__strategy-current">
              <strong>
                {t('deepReviewConsent.runStrategy', {
                  strategy: selectedStrategyLabel,
                  defaultValue: 'Run strategy: {{strategy}}',
                })}
              </strong>
              <span>{getStrategySummary(effectiveStrategy, t)}</span>
            </div>
            <div
              className="deep-review-consent__strategy-options"
              role="group"
              aria-label={t('deepReviewConsent.strategyOverrideTitle', {
                defaultValue: 'Run strategy',
              })}
            >
              {REVIEW_STRATEGY_LEVELS.map((strategyLevel) => {
                const isActive = effectiveStrategy === strategyLevel;
                const profile = getReviewStrategyProfile(strategyLevel);
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
                    <span className="deep-review-consent__strategy-option-header">
                      <span className="deep-review-consent__strategy-option-label">
                        {getStrategyLabel(strategyLevel, t)}
                      </span>
                      {isActive && (
                        <span className="deep-review-consent__strategy-option-badge">
                          {t('deepReviewConsent.selectedStrategy', { defaultValue: 'Selected' })}
                        </span>
                      )}
                    </span>
                    <span className="deep-review-consent__strategy-option-meta">
                      <span>
                        {t('deepReviewConsent.strategyTokenImpact', {
                          tokenImpact: profile.tokenImpact,
                          defaultValue: 'Token: {{tokenImpact}}',
                        })}
                      </span>
                      <span>
                        {t('deepReviewConsent.strategyRuntimeImpact', {
                          runtimeImpact: profile.runtimeImpact,
                          defaultValue: 'Time: {{runtimeImpact}}',
                        })}
                      </span>
                    </span>
                  </button>
                );
              })}
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
              {visibleSkippedReviewers.map((member) => (
                <li key={`skipped-${member.subagentId}`}>
                  <span>{getReviewerLabel(member)}</span>
                  <strong>{getSkippedReasonLabel(member.reason)}</strong>
                </li>
              ))}
              {hiddenSkippedCount > 0 && (
                <li className="deep-review-consent__skipped-more">
                  <span>
                    {t('deepReviewConsent.skippedMore', {
                      count: hiddenSkippedCount,
                      defaultValue: '+{{count}} more',
                    })}
                  </span>
                </li>
              )}
            </ul>
          </div>
        )}
      </div>
    );
  }, [
    getSkippedReasonLabel,
    selectStrategyOverride,
    selectedStrategyOverride,
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
            <h3>{t('deepReviewConsent.title', { defaultValue: 'Start Deep Review?' })}</h3>
          </div>
          <button
            type="button"
            className="deep-review-consent__close"
            aria-label={t('deepReviewConsent.cancel', { defaultValue: 'Cancel' })}
            onClick={() => void settleConsent(false)}
          >
            <X size={16} />
          </button>
        </div>

        {pendingConsent.launchContext?.sessionConcurrencyGuard?.highActivity && (
          <div className="deep-review-consent__capacity-note">
            <div className="deep-review-consent__fact-icon deep-review-consent__fact-icon--warning">
              <AlertTriangle size={16} />
            </div>
            <div>
              <span className="deep-review-consent__fact-title">
                {t('deepReviewConsent.sessionConcurrencyTitle', {
                  defaultValue: 'Active session is busy',
                })}
              </span>
              <p>
                {t('deepReviewConsent.sessionConcurrencyBody', {
                  count: pendingConsent.launchContext.sessionConcurrencyGuard.activeSubagentCount,
                  defaultValue:
                    'The target session already has {{count}} running subagent tasks. Choose a lighter strategy, cancel for now, or continue manually when capacity is free.',
                })}
              </p>
            </div>
          </div>
        )}

        {pendingConsent.preview && renderLaunchSummary(pendingConsent.preview)}

        <div className="deep-review-consent__footer">
          <Checkbox
            className="deep-review-consent__checkbox"
            checked={dontShowAgain}
            onChange={(event) => setDontShowAgain(event.target.checked)}
            label={t('deepReviewConsent.dontShowAgain', {
              defaultValue: 'Do not show this again',
            })}
          />
          <div className="deep-review-consent__actions">
            <Button
              variant="secondary"
              size="small"
              onClick={() => void settleConsent(false)}
            >
              {t('deepReviewConsent.cancel', { defaultValue: 'Cancel' })}
            </Button>
            <Button
              variant="primary"
              size="small"
              onClick={() => void settleConsent(true)}
            >
              {t('deepReviewConsent.confirm', { defaultValue: 'Start Deep Review' })}
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
