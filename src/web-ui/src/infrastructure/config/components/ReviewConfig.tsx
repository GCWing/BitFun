import React, { useCallback, useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { RotateCcw } from 'lucide-react';
import { Button, ConfigPageLoading, NumberInput } from '@/component-library';
import {
  ConfigPageContent,
  ConfigPageHeader,
  ConfigPageLayout,
  ConfigPageRow,
  ConfigPageSection,
} from './common';
import { useCurrentWorkspace } from '@/infrastructure/contexts/WorkspaceContext';
import { isTauriRuntime } from '@/infrastructure/runtime';
import { useNotification } from '@/shared/notification-system';
import {
  loadDefaultReviewTeam,
  saveDefaultReviewTeamConcurrencyPolicy,
  type ReviewTeam,
  type ReviewTeamConcurrencyPolicy,
} from '@/shared/services/reviewTeamService';

const ReviewConfig: React.FC = () => {
  const { t } = useTranslation('settings/review');
  const { workspacePath } = useCurrentWorkspace();
  const { error: notifyError, success: notifySuccess } = useNotification();
  const desktopRuntime = isTauriRuntime();

  const [loading, setLoading] = useState(desktopRuntime);
  const [team, setTeam] = useState<ReviewTeam | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [savingConcurrencyKey, setSavingConcurrencyKey] = useState<keyof ReviewTeamConcurrencyPolicy | null>(null);

  const loadData = useCallback(async () => {
    if (!desktopRuntime) return;
    setLoading(true);
    setLoadError(null);
    try {
      const loadedTeam = await loadDefaultReviewTeam(workspacePath || undefined);
      setTeam(loadedTeam);
    } catch (error) {
      const message = error instanceof Error ? error.message : t('messages.loadFailed');
      setLoadError(message);
      notifyError(message);
    } finally {
      setLoading(false);
    }
  }, [desktopRuntime, notifyError, t, workspacePath]);

  useEffect(() => {
    void loadData();
  }, [loadData]);

  const handleConcurrencyPolicyChange = useCallback(async (
    key: keyof ReviewTeamConcurrencyPolicy,
    value: ReviewTeamConcurrencyPolicy[keyof ReviewTeamConcurrencyPolicy],
  ) => {
    if (!team || savingConcurrencyKey !== null) return;

    const nextPolicy = {
      ...team.concurrencyPolicy,
      [key]: value,
    } as ReviewTeamConcurrencyPolicy;
    const previousTeam = team;
    setSavingConcurrencyKey(key);
    setTeam({ ...team, concurrencyPolicy: nextPolicy });
    try {
      await saveDefaultReviewTeamConcurrencyPolicy(nextPolicy);
      notifySuccess(t('messages.saved'));
    } catch (error) {
      setTeam(previousTeam);
      notifyError(error instanceof Error ? error.message : t('messages.saveFailed'));
    } finally {
      setSavingConcurrencyKey(null);
    }
  }, [notifyError, notifySuccess, savingConcurrencyKey, t, team]);

  if (!desktopRuntime) {
    return (
      <ConfigPageLayout>
        <ConfigPageHeader title={t('title')} subtitle={t('subtitle')} />
        <ConfigPageContent>
          <ConfigPageSection
            title={t('desktopOnly.title')}
            description={t('desktopOnly.description')}
          >
            {null}
          </ConfigPageSection>
        </ConfigPageContent>
      </ConfigPageLayout>
    );
  }

  if (loading) {
    return (
      <ConfigPageLayout>
        <ConfigPageHeader title={t('title')} subtitle={t('subtitle')} />
        <ConfigPageContent>
          <ConfigPageLoading text={t('loading')} />
        </ConfigPageContent>
      </ConfigPageLayout>
    );
  }

  if (!team) {
    return (
      <ConfigPageLayout>
        <ConfigPageHeader title={t('title')} subtitle={t('subtitle')} />
        <ConfigPageContent>
          <ConfigPageSection
            title={t('error.title')}
            description={loadError ?? t('messages.loadFailed')}
          >
            <Button variant="secondary" size="small" onClick={() => void loadData()}>
              <RotateCcw size={14} />
              {t('error.retry')}
            </Button>
          </ConfigPageSection>
        </ConfigPageContent>
      </ConfigPageLayout>
    );
  }

  return (
    <ConfigPageLayout>
      <ConfigPageHeader
        title={t('title')}
        subtitle={t('subtitle')}
      />

      <ConfigPageContent>
        <ConfigPageSection title={t('capacity.title')} description={t('capacity.description')}>
          <ConfigPageRow label={t('capacity.maxParallelReviewers.label')} description={t('capacity.maxParallelReviewers.description')} align="center" balanced>
            <NumberInput
              value={team.concurrencyPolicy.maxParallelInstances}
              onChange={(value) => void handleConcurrencyPolicyChange('maxParallelInstances', value)}
              min={1}
              max={16}
              step={1}
              size="small"
              disabled={savingConcurrencyKey !== null}
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
              disabled={savingConcurrencyKey !== null}
            />
          </ConfigPageRow>

        </ConfigPageSection>
      </ConfigPageContent>
    </ConfigPageLayout>
  );
};

export default ReviewConfig;
