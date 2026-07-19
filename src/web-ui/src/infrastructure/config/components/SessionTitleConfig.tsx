import React, { useCallback, useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Switch } from '@/component-library';
import { useNotification, notificationService } from '@/shared/notification-system';
import { createLogger } from '@/shared/utils/logger';
import { aiExperienceConfigService, type AIExperienceSettings } from '../services/AIExperienceConfigService';
import { configManager } from '../services/ConfigManager';
import type { AIModelConfig } from '../types';
import { ConfigPageRow, ConfigPageSection } from './common';
import { ModelSelectionRadio } from './ModelSelectionRadio';
import './AIFeaturesConfig.scss';

const log = createLogger('SessionTitleConfig');

const AGENT_SESSION_TITLE = 'session-title-func-agent';

export const SessionTitleConfig: React.FC = () => {
  const { t } = useTranslation('settings/ai-model');
  const { success: notifySuccess, error: notifyError } = useNotification();
  const [isLoading, setIsLoading] = useState(true);
  const [settings, setSettings] = useState<AIExperienceSettings | null>(null);
  const [models, setModels] = useState<AIModelConfig[]>([]);
  const [funcAgentModels, setFuncAgentModels] = useState<Record<string, string>>({});

  const loadData = useCallback(async () => {
    setIsLoading(true);
    try {
      const [loadedSettings, allModels, funcAgentModelsData] = await Promise.all([
        aiExperienceConfigService.getSettingsAsync(),
        configManager.getConfig<AIModelConfig[]>('ai.models') || [],
        configManager.getConfig<Record<string, string>>('ai.func_agent_models') || {},
      ]);
      setSettings(loadedSettings);
      setModels(allModels ?? []);
      setFuncAgentModels(funcAgentModelsData ?? {});
    } catch (error) {
      log.error('Failed to load session title config', error);
      notifyError(t('sessionTitle.loadFailed'));
    } finally {
      setIsLoading(false);
    }
  }, [notifyError, t]);

  useEffect(() => {
    void loadData();
    const unwatchModels = configManager.watch('ai.models', () => void loadData());
    const unwatchFuncAgentModels = configManager.watch('ai.func_agent_models', () => void loadData());
    const unwatchSettings = aiExperienceConfigService.addChangeListener((next) => {
      setSettings(next);
    });
    return () => {
      unwatchModels();
      unwatchFuncAgentModels();
      unwatchSettings();
    };
  }, [loadData]);

  const enabledModels = models.filter((model) => model.enabled);
  const sessionTitleModelId = funcAgentModels[AGENT_SESSION_TITLE] || 'fast';

  const updateEnabled = async (checked: boolean) => {
    if (!settings) return;
    const previous = settings;
    const next = { ...settings, enable_session_title_generation: checked };
    setSettings(next);
    try {
      await aiExperienceConfigService.saveSettings(next);
      notifySuccess(t('sessionTitle.messages.saveSuccess'));
    } catch (error) {
      log.error('Failed to save session title enable setting', error);
      notifyError(t('sessionTitle.messages.saveFailed'));
      setSettings(previous);
    }
  };

  const getModelName = useCallback((modelId: string | null | undefined): string | undefined => {
    if (!modelId) return undefined;
    return models.find((model) => model.id === modelId)?.name;
  }, [models]);

  const handleModelChange = async (modelId: string) => {
    try {
      const current = await configManager.getConfig<Record<string, string>>('ai.func_agent_models') || {};
      const updated = { ...current, [AGENT_SESSION_TITLE]: modelId };
      await configManager.setConfig('ai.func_agent_models', updated);
      setFuncAgentModels(updated);

      let modelDesc = '';
      if (modelId === 'primary') {
        modelDesc = t('sessionTitle.model.primary');
      } else if (modelId === 'fast') {
        modelDesc = t('sessionTitle.model.fast');
      } else {
        modelDesc = getModelName(modelId) || modelId || '';
      }

      notificationService.success(
        t('sessionTitle.models.updateSuccess', {
          agentName: t('sessionTitle.title'),
          modelName: modelDesc,
        }),
        { duration: 2000 },
      );
    } catch (error) {
      log.error('Failed to update session title model', { modelId, error });
      notificationService.error(t('sessionTitle.messages.updateFailed'), { duration: 3000 });
    }
  };

  return (
    <ConfigPageSection
      className="bitfun-func-agent-config"
      title={t('sessionTitle.title')}
      description={t('sessionTitle.subtitle')}
    >
      <ConfigPageRow label={t('sessionTitle.enable')} align="center">
        <div className="bitfun-func-agent-config__row-control">
          <Switch
            checked={settings?.enable_session_title_generation ?? false}
            onChange={(e) => void updateEnabled(e.target.checked)}
            size="small"
            disabled={isLoading || !settings}
          />
        </div>
      </ConfigPageRow>
      <ConfigPageRow
        className="bitfun-func-agent-config__model-row"
        label={t('sessionTitle.model.label')}
        description={enabledModels.length === 0 ? t('sessionTitle.models.empty') : undefined}
        align="center"
      >
        <div className="bitfun-func-agent-config__row-control bitfun-func-agent-config__row-control--model">
          <ModelSelectionRadio
            value={sessionTitleModelId}
            models={enabledModels}
            onChange={(modelId) => void handleModelChange(modelId)}
            layout="horizontal"
            size="small"
            disabled={isLoading}
          />
        </div>
      </ConfigPageRow>
    </ConfigPageSection>
  );
};

export default SessionTitleConfig;
