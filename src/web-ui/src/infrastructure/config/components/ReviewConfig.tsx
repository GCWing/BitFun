import React, { useCallback, useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Badge, ConfigPageLoading, NumberInput } from '@/component-library';
import {
  ConfigPageContent,
  ConfigPageHeader,
  ConfigPageLayout,
  ConfigPageRow,
  ConfigPageSection,
} from './common';
import { useCurrentWorkspace } from '@/infrastructure/contexts/WorkspaceContext';
import { useNotification } from '@/shared/notification-system';
import {
  loadDefaultReviewTeam,
  REVIEW_STRATEGY_LEVELS,
  saveDefaultReviewTeamConcurrencyPolicy,
  saveDefaultReviewTeamStrategyLevel,
  type ReviewStrategyLevel,
  type ReviewTeam,
  type ReviewTeamConcurrencyPolicy,
} from '@/shared/services/reviewTeamService';
import './ReviewConfig.scss';

function updateTeamStrategy(team: ReviewTeam, strategyLevel: ReviewStrategyLevel): ReviewTeam {
  return {
    ...team,
    strategyLevel,
  };
}

const ReviewConfig: React.FC = () => {
  const { t } = useTranslation('settings/review');
  const { workspacePath } = useCurrentWorkspace();
  const { error: notifyError, success: notifySuccess } = useNotification();

  const [loading, setLoading] = useState(true);
  const [team, setTeam] = useState<ReviewTeam | null>(null);
  const [savingConcurrencyKey, setSavingConcurrencyKey] = useState<keyof ReviewTeamConcurrencyPolicy | null>(null);
  const [savingStrategyTarget, setSavingStrategyTarget] = useState<string | null>(null);

  const loadData = useCallback(async () => {
    setLoading(true);
    try {
      const loadedTeam = await loadDefaultReviewTeam(workspacePath || undefined);
      setTeam(loadedTeam);
    } catch (error) {
      notifyError(error instanceof Error ? error.message : t('messages.loadFailed'));
    } finally {
      setLoading(false);
    }
  }, [notifyError, t, workspacePath]);

  useEffect(() => {
    void loadData();
  }, [loadData]);

  const getStrategyLabel = useCallback((level: ReviewStrategyLevel) => (
    t(`strategy.${level}.label`)
  ), [t]);

  const getStrategySummary = useCallback((level: ReviewStrategyLevel) => (
    t(`strategy.${level}.summary`)
  ), [t]);

  const handleTeamStrategyChange = useCallback(async (strategyLevel: ReviewStrategyLevel) => {
    if (!team || team.strategyLevel === strategyLevel) return;

    setSavingStrategyTarget('team');
    setTeam(updateTeamStrategy(team, strategyLevel));
    try {
      await saveDefaultReviewTeamStrategyLevel(strategyLevel);
      notifySuccess(t('messages.saved'));
    } catch (error) {
      await loadData();
      notifyError(error instanceof Error ? error.message : t('messages.saveFailed'));
    } finally {
      setSavingStrategyTarget(null);
    }
  }, [loadData, notifyError, notifySuccess, t, team]);

  const handleConcurrencyPolicyChange = useCallback(async (
    key: keyof ReviewTeamConcurrencyPolicy,
    value: ReviewTeamConcurrencyPolicy[keyof ReviewTeamConcurrencyPolicy],
  ) => {
    if (!team) return;

    const nextPolicy = {
      ...team.concurrencyPolicy,
      [key]: value,
    } as ReviewTeamConcurrencyPolicy;
    setSavingConcurrencyKey(key);
    setTeam({ ...team, concurrencyPolicy: nextPolicy });
    try {
      await saveDefaultReviewTeamConcurrencyPolicy(nextPolicy);
      notifySuccess(t('messages.saved'));
    } catch (error) {
      await loadData();
      notifyError(error instanceof Error ? error.message : t('messages.saveFailed'));
    } finally {
      setSavingConcurrencyKey(null);
    }
  }, [loadData, notifyError, notifySuccess, t, team]);

  if (loading || !team) {
    return (
      <ConfigPageLayout>
        <ConfigPageLoading text={t('loading')} />
      </ConfigPageLayout>
    );
  }

  return (
    <ConfigPageLayout className="review-config">
      <ConfigPageHeader
        title={t('title')}
        subtitle={t('subtitle')}
      />

      <ConfigPageContent>
        <ConfigPageSection
          title={t('overview.title')}
          description={t('overview.description')}
        >
          <div className="review-config__overview-grid">
            <div className="review-config__overview-item">
              <span className="review-config__overview-label">{t('overview.command.title')}</span>
              <p className="review-config__overview-copy">{t('overview.command.description')}</p>
            </div>
            <div className="review-config__overview-item">
              <span className="review-config__overview-label">{t('overview.reviewers.title')}</span>
              <p className="review-config__overview-copy">{t('overview.reviewers.description')}</p>
            </div>
            <div className="review-config__overview-item">
              <span className="review-config__overview-label">{t('overview.qualityGate.title')}</span>
              <p className="review-config__overview-copy">{t('overview.qualityGate.description')}</p>
            </div>
          </div>
        </ConfigPageSection>

        <ConfigPageSection
          title={t('strategy.title')}
          description={t('strategy.description')}
          titleSuffix={<Badge variant="neutral">{getStrategyLabel(team.strategyLevel)}</Badge>}
        >
          <div className="review-config__strategy-options">
            {REVIEW_STRATEGY_LEVELS.map((level) => {
              const isSelected = team.strategyLevel === level;
              return (
                <button
                  key={level}
                  type="button"
                  className={`review-config__strategy-option${isSelected ? ' is-selected' : ''}`}
                  aria-pressed={isSelected}
                  disabled={savingStrategyTarget === 'team'}
                  onClick={() => void handleTeamStrategyChange(level)}
                >
                  <span className="review-config__strategy-title">{getStrategyLabel(level)}</span>
                  <span className="review-config__strategy-summary">{getStrategySummary(level)}</span>
                </button>
              );
            })}
          </div>
        </ConfigPageSection>

        <ConfigPageSection title={t('capacity.title')} description={t('capacity.description')}>
          <ConfigPageRow label={t('capacity.maxParallelReviewers.label')} description={t('capacity.maxParallelReviewers.description')} align="center" balanced>
            <NumberInput
              value={team.concurrencyPolicy.maxParallelInstances}
              onChange={(value) => void handleConcurrencyPolicyChange('maxParallelInstances', value)}
              min={1}
              max={16}
              step={1}
              size="small"
              disabled={savingConcurrencyKey === 'maxParallelInstances'}
            />
          </ConfigPageRow>

          <ConfigPageRow label={t('capacity.maxQueueWaitSeconds.label')} description={t('capacity.maxQueueWaitSeconds.description')} align="center" balanced>
            <NumberInput
              value={team.concurrencyPolicy.maxQueueWaitSeconds}
              onChange={(value) => void handleConcurrencyPolicyChange('maxQueueWaitSeconds', value)}
              min={0}
              max={3600}
              step={60}
              unit="s"
              size="small"
              disabled={savingConcurrencyKey === 'maxQueueWaitSeconds'}
            />
          </ConfigPageRow>

        </ConfigPageSection>
      </ConfigPageContent>
    </ConfigPageLayout>
  );
};

export default ReviewConfig;
