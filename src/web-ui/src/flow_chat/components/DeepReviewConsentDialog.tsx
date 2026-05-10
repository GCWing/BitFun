import React, { useCallback, useState } from 'react';
import { AlertTriangle, ShieldCheck, X } from 'lucide-react';
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
const MAX_VISIBLE_TARGET_TAGS = 3;

const TARGET_TAG_LABELS: Record<string, { key: string; defaultValue: string }> = {
  frontend_ui: { key: 'frontendUi', defaultValue: 'Frontend UI' },
  frontend_style: { key: 'frontendStyle', defaultValue: 'Frontend styles' },
  frontend_i18n: { key: 'frontendI18n', defaultValue: 'Frontend i18n' },
  frontend_contract: { key: 'frontendContract', defaultValue: 'Frontend contract' },
  desktop_contract: { key: 'desktopContract', defaultValue: 'Desktop contract' },
  web_server_contract: { key: 'webServerContract', defaultValue: 'Web server contract' },
  backend_core: { key: 'backendCore', defaultValue: 'Backend core' },
  transport: { key: 'transport', defaultValue: 'Transport' },
  api_layer: { key: 'apiLayer', defaultValue: 'API layer' },
  ai_adapter: { key: 'aiAdapter', defaultValue: 'AI adapter' },
  installer_ui: { key: 'installerUi', defaultValue: 'Installer UI' },
  test: { key: 'test', defaultValue: 'Tests' },
  docs: { key: 'docs', defaultValue: 'Docs' },
  config: { key: 'config', defaultValue: 'Config' },
  generated_or_lock: { key: 'generatedOrLock', defaultValue: 'Generated or lockfile' },
  unknown: { key: 'unknown', defaultValue: 'Unknown area' },
};

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

function getFallbackTargetTagLabel(tag: string): string {
  return tag
    .split('_')
    .filter(Boolean)
    .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
    .join(' ');
}

function getReviewDepthLabel(reviewDepth: string, t: ReturnType<typeof useTranslation>['t']): string {
  return t(`deepReviewConsent.reviewDepthLabels.${reviewDepth}`, {
    defaultValue: {
      high_risk_only: 'High-risk-only',
      risk_expanded: 'Risk-expanded',
      full_depth: 'Full-depth',
    }[reviewDepth] ?? reviewDepth,
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
    const targetFileCount = getReviewTargetFileCount(preview);
    const visibleTargetTags = preview.target.tags.slice(0, MAX_VISIBLE_TARGET_TAGS);
    const hiddenTargetTagCount = Math.max(0, preview.target.tags.length - visibleTargetTags.length);
    const targetTagLabels = visibleTargetTags.map((tag) => {
      const label = TARGET_TAG_LABELS[tag] ?? {
        key: 'unknown',
        defaultValue: getFallbackTargetTagLabel(tag),
      };
      return t(`deepReviewConsent.targetTagLabels.${label.key}`, {
        defaultValue: label.defaultValue,
      });
    });
    const targetTagSummary = targetTagLabels.length > 0
      ? hiddenTargetTagCount > 0
        ? t('deepReviewConsent.targetTagsWithMore', {
          tags: targetTagLabels.join(', '),
          count: hiddenTargetTagCount,
          defaultValue: '{{tags}} +{{count}} more',
        })
        : targetTagLabels.join(', ')
      : t('deepReviewConsent.targetTagLabels.unknown', {
        defaultValue: 'Unknown area',
      });
    const optionalReviewerCount = preview.enabledExtraReviewers.length;

    return (
      <div className="deep-review-consent__summary">
        <div className="deep-review-consent__summary-header">
          <span className="deep-review-consent__fact-title">
            {t('deepReviewConsent.summaryTitle', { defaultValue: 'Launch summary' })}
          </span>
        </div>

        <div className="deep-review-consent__summary-stats">
          <span>
            {t('deepReviewConsent.targetFiles', {
              count: targetFileCount,
              defaultValue: targetFileCount === 1 ? '{{count}} file' : '{{count}} files',
            })}
          </span>
          <span>
            {t('deepReviewConsent.targetRiskTags', {
              tags: targetTagSummary,
              defaultValue: 'Risk areas: {{tags}}',
            })}
          </span>
          <span>
            {t('deepReviewConsent.estimatedCalls', {
              count: preview.tokenBudget.estimatedReviewerCalls,
              defaultValue: '{{count}} reviewer calls',
            })}
          </span>
          {skippedCount > 0 && (
            <span className="deep-review-consent__summary-stat--warning">
              {t('deepReviewConsent.skippedReviewers', {
                count: skippedCount,
              defaultValue: '{{count}} skipped',
            })}
          </span>
          )}
          {optionalReviewerCount > 0 && (
            <span>
              {t('deepReviewConsent.optionalReviewers', {
                count: optionalReviewerCount,
                defaultValue: optionalReviewerCount === 1
                  ? '{{count}} optional reviewer'
                  : '{{count}} optional reviewers',
              })}
            </span>
          )}
          {preview.tokenBudget.largeDiffSummaryFirst && (
            <span>
              {t('deepReviewConsent.summaryFirstReview', {
                defaultValue: 'Summary-first coverage',
              })}
            </span>
          )}
          <span>
            {t('deepReviewConsent.runStrategy', {
              strategy: selectedStrategyLabel,
              defaultValue: 'Run strategy: {{strategy}}',
            })}
          </span>
          {preview.scopeProfile && (
            <span>
              {t('deepReviewConsent.reviewDepth', {
                depth: getReviewDepthLabel(preview.scopeProfile.reviewDepth, t),
                defaultValue: 'Review depth: {{depth}}',
              })}
            </span>
          )}
        </div>

        {preview.workspacePath && (
          <div className="deep-review-consent__strategy-control">
            <div className="deep-review-consent__reviewer-group-title">
              {t('deepReviewConsent.strategyOverrideTitle', {
                defaultValue: 'Run strategy',
              })}
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

        <p className="deep-review-consent__lead">
          {t('deepReviewConsent.body', {
            defaultValue: 'Deep Review launches multiple reviewers and can take longer or use more tokens than a standard review.',
          })}
        </p>

        <div className="deep-review-consent__safety-note">
          <div className="deep-review-consent__fact-icon">
            <ShieldCheck size={16} />
          </div>
          <div>
            <span className="deep-review-consent__fact-title">
              {t('deepReviewConsent.readonlyLabel', { defaultValue: 'Read-only first pass' })}
            </span>
            <p>
              {t('deepReviewConsent.readonly', {
                defaultValue: 'The first pass reports findings and a remediation plan before any code changes.',
              })}
            </p>
          </div>
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
