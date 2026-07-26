import React, { useCallback, useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { ExternalLink } from 'lucide-react';
import { Button, ConfigPageLoading, Switch } from '@/component-library';
import { useNotification } from '@/shared/notification-system';
import { createLogger } from '@/shared/utils/logger';
import { systemAPI } from '@/infrastructure/api/service-api/SystemAPI';
import { configManager } from '../services/ConfigManager';
import {
  ConfigPageContent,
  ConfigPageHeader,
  ConfigPageLayout,
  ConfigPageRow,
  ConfigPageSection,
} from './common';

const log = createLogger('HooksConfig');

const CODEX_HOOKS_DOC_URL = 'https://learn.chatgpt.com/docs/hooks';

/** Enablement gates only. Hook declarations live in hooks.json. */
interface AgentHooksConfigShape {
  enabled: boolean;
  project_hooks_enabled: boolean;
}

const DEFAULT_HOOKS_CONFIG: AgentHooksConfigShape = {
  enabled: true,
  project_hooks_enabled: false,
};

function normalizeHooksConfig(
  config: Partial<AgentHooksConfigShape> | null | undefined
): AgentHooksConfigShape {
  return {
    ...DEFAULT_HOOKS_CONFIG,
    ...(config ?? {}),
  };
}

const HooksConfig: React.FC = () => {
  const { t } = useTranslation('settings/hooks');
  const { error: notifyError, success: notifySuccess } = useNotification();

  const [loading, setLoading] = useState(true);
  const [config, setConfig] = useState<AgentHooksConfigShape>(DEFAULT_HOOKS_CONFIG);
  const [savingKey, setSavingKey] = useState<keyof AgentHooksConfigShape | null>(null);

  const loadData = useCallback(async () => {
    setLoading(true);
    try {
      const loaded = await configManager.getConfig<Partial<AgentHooksConfigShape>>('app.hooks');
      setConfig(normalizeHooksConfig(loaded));
    } catch (error) {
      log.error('Failed to load hooks config', error);
      notifyError(error instanceof Error ? error.message : t('messages.loadFailed'));
    } finally {
      setLoading(false);
    }
  }, [notifyError, t]);

  useEffect(() => {
    void loadData();
  }, [loadData]);

  const updateConfig = useCallback(
    async <K extends keyof AgentHooksConfigShape>(key: K, value: AgentHooksConfigShape[K]) => {
      const previous = config;
      const next = { ...config, [key]: value };
      setSavingKey(key);
      setConfig(next);
      try {
        await configManager.setConfig('app.hooks', next);
        notifySuccess(t('messages.saved'));
      } catch (error) {
        log.error('Failed to save hooks config', { key, error });
        setConfig(previous);
        notifyError(error instanceof Error ? error.message : t('messages.saveFailed'));
      } finally {
        setSavingKey(null);
      }
    },
    [config, notifyError, notifySuccess, t]
  );

  const openCodexHooksDoc = useCallback(() => {
    void systemAPI.openExternal(CODEX_HOOKS_DOC_URL).catch((error: unknown) => {
      log.error('Failed to open the Codex hooks documentation', error);
    });
  }, []);

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

  return (
    <ConfigPageLayout>
      <ConfigPageHeader title={t('title')} subtitle={t('subtitle')} />

      <ConfigPageContent>
        <ConfigPageSection title={t('activation.title')} description={t('activation.description')}>
          <ConfigPageRow
            label={t('fields.enabled.label')}
            description={t('fields.enabled.description')}
            align="center"
          >
            <Switch
              checked={config.enabled}
              onChange={(event) => void updateConfig('enabled', event.target.checked)}
              disabled={savingKey !== null}
            />
          </ConfigPageRow>

          <ConfigPageRow
            label={t('fields.projectHooks.label')}
            description={t('fields.projectHooks.description')}
            align="center"
          >
            <Switch
              checked={config.project_hooks_enabled}
              onChange={(event) => void updateConfig('project_hooks_enabled', event.target.checked)}
              disabled={savingKey !== null || !config.enabled}
            />
          </ConfigPageRow>
        </ConfigPageSection>

        <ConfigPageSection title={t('locations.title')} description={t('locations.description')}>
          <ConfigPageRow
            label={t('locations.userFile.label')}
            description={t('locations.userFile.description')}
            align="center"
          >
            {null}
          </ConfigPageRow>

          <ConfigPageRow
            label={t('locations.projectFile.label')}
            description={t('locations.projectFile.description')}
            align="center"
          >
            {null}
          </ConfigPageRow>
        </ConfigPageSection>

        <ConfigPageSection
          title={t('compatibility.title')}
          description={t('compatibility.description')}
        >
          <ConfigPageRow
            label={t('compatibility.reference.label')}
            description={t('compatibility.reference.description')}
            align="center"
          >
            <Button variant="secondary" size="small" onClick={openCodexHooksDoc}>
              <ExternalLink size={14} />
              {t('compatibility.reference.open')}
            </Button>
          </ConfigPageRow>
        </ConfigPageSection>
      </ConfigPageContent>
    </ConfigPageLayout>
  );
};

export default HooksConfig;
