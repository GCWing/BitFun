 

import React, { useState, useEffect, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { Switch, ConfigPageLoading } from '@/component-library';
import { ConfigPageHeader, ConfigPageLayout, ConfigPageContent, ConfigPageSection, ConfigPageRow } from './common';
import { aiExperienceConfigService } from '../services/AIExperienceConfigService';
import { configManager } from '../services/ConfigManager';
import { useNotification, notificationService } from '@/shared/notification-system';
import type { AIModelConfig } from '../types';
import { ModelSelectionRadio } from './ModelSelectionRadio';
import { createLogger } from '@/shared/utils/logger';
import './AIFeaturesConfig.scss';

const log = createLogger('AIFeaturesConfig');


interface AIExperienceSettings {
  enable_session_title_generation: boolean;
  enable_welcome_panel_ai_analysis: boolean;
}

const defaultSettings: AIExperienceSettings = {
  enable_session_title_generation: true,
  enable_welcome_panel_ai_analysis: true,
};


interface FeatureConfig {
  id: string;
  settingKey?: keyof AIExperienceSettings;  
  agentName?: string;  
}


const FEATURE_CONFIGS: FeatureConfig[] = [
  {
    id: 'sessionTitle',
    settingKey: 'enable_session_title_generation',
    agentName: 'startchat-func-agent',
  },
  {
    id: 'welcomeAnalysis',
    settingKey: 'enable_welcome_panel_ai_analysis',
    agentName: 'startchat-func-agent',  
  },
];

const AIFeaturesConfig: React.FC = () => {
  const { t } = useTranslation('settings/ai-features');
  const notification = useNotification();
  
  
  const [settings, setSettings] = useState<AIExperienceSettings>(defaultSettings);
  const [isLoading, setIsLoading] = useState(true);
  
  
  const [models, setModels] = useState<AIModelConfig[]>([]);
  const [funcAgentModels, setFuncAgentModels] = useState<Record<string, string>>({});

  useEffect(() => {
    loadAllData();
  }, []);

  const loadAllData = async () => {
    setIsLoading(true);
    try {
      
      const [
        loadedSettings,
        allModels,
        funcAgentModelsData
      ] = await Promise.all([
        aiExperienceConfigService.getSettingsAsync(),
        configManager.getConfig<AIModelConfig[]>('ai.models') || [],
        configManager.getConfig<Record<string, string>>('ai.func_agent_models') || {}
      ]);

      setSettings(loadedSettings);
      setModels(allModels);
      setFuncAgentModels(funcAgentModelsData);
    } catch (error) {
      log.error('Failed to load data', error);
      setSettings(defaultSettings);
    } finally {
      setIsLoading(false);
    }
  };

  
  const getModelName = useCallback((modelId: string | null | undefined): string | undefined => {
    if (!modelId) return undefined;
    return models.find(m => m.id === modelId)?.name;
  }, [models]);

  const updateSetting = async <K extends keyof AIExperienceSettings>(
    key: K,
    value: AIExperienceSettings[K]
  ) => {
    
    const newSettings = { ...settings, [key]: value };
    setSettings(newSettings);

    
    try {
      await aiExperienceConfigService.saveSettings(newSettings);
      notification.success(t('messages.saveSuccess'));
    } catch (error) {
      log.error('Failed to save AI features settings', error);
      notification.error(`${t('messages.saveFailed')}: ` + (error instanceof Error ? error.message : String(error)));
      
      setSettings(settings);
    }
  };

  
  const handleAgentSelectionChange = async (
    agentName: string,
    modelId: string
  ) => {
    try {
      const currentFuncAgentModels = await configManager.getConfig<Record<string, string>>('ai.func_agent_models') || {};

      const updatedFuncAgentModels = {
        ...currentFuncAgentModels,
        [agentName]: modelId,
      };
      await configManager.setConfig('ai.func_agent_models', updatedFuncAgentModels);

      setFuncAgentModels(updatedFuncAgentModels);

      
      let modelDesc = '';
      if (modelId === 'primary') {
        modelDesc = t('model.primary');
      } else if (modelId === 'fast') {
        modelDesc = t('model.fast');
      } else {
        modelDesc = getModelName(modelId) || modelId || '';
      }

      notificationService.success(
        t('models.updateSuccess', { agentName: t(`features.${getFeatureIdByAgent(agentName)}.title`), modelName: modelDesc }),
        { duration: 2000 }
      );
    } catch (error) {
      log.error('Failed to update agent model', { agentName, modelId, error });
      notificationService.error(t('messages.updateFailed'), { duration: 3000 });
    }
  };

  
  const getFeatureIdByAgent = (agentName: string): string => {
    const feature = FEATURE_CONFIGS.find(f => f.agentName === agentName);
    return feature?.id || agentName;
  };

  
  const enabledModels = models.filter(m => m.enabled);

  if (isLoading) {
    return (
      <ConfigPageLayout className="bitfun-func-agent-config">
        <ConfigPageHeader
          title={t('title')}
          subtitle={t('subtitle')}
        />
        <ConfigPageContent className="bitfun-func-agent-config__content">
          <ConfigPageLoading text={t('loading.text')} />
        </ConfigPageContent>
      </ConfigPageLayout>
    );
  }

  return (
    <ConfigPageLayout className="bitfun-func-agent-config">
      <ConfigPageHeader
        title={t('title')}
        subtitle={t('subtitle')}
      />
      
      <ConfigPageContent className="bitfun-func-agent-config__content">
        {FEATURE_CONFIGS.map((feature) => {
          const hasSwitch = !!feature.settingKey;
          const hasModel = !!feature.agentName;
          const isEnabled = hasSwitch ? settings[feature.settingKey!] : true;
          const configuredModelId = hasModel ? (funcAgentModels[feature.agentName!] || 'fast') : 'fast';
          const warning = t(`features.${feature.id}.warning`, '');

          return (
            <ConfigPageSection
              key={feature.id}
              title={t(`features.${feature.id}.title`)}
              description={t(`features.${feature.id}.subtitle`)}
            >
              {hasSwitch && (
                <ConfigPageRow
                  label={t('common.enable')}
                  description={warning && !isEnabled ? warning : undefined}
                  align="center"
                >
                  <div className="bitfun-func-agent-config__row-control">
                    <Switch
                      checked={isEnabled}
                      onChange={(e) => updateSetting(feature.settingKey!, e.target.checked)}
                      size="small"
                    />
                  </div>
                </ConfigPageRow>
              )}

              {hasModel && (
                <ConfigPageRow
                  className="bitfun-func-agent-config__model-row"
                  label={t('model.label')}
                  description={enabledModels.length === 0 ? t('models.empty') : undefined}
                  align="center"
                >
                  <div className="bitfun-func-agent-config__row-control bitfun-func-agent-config__row-control--model">
                    <ModelSelectionRadio
                      value={configuredModelId}
                      models={enabledModels}
                      onChange={(modelId) => handleAgentSelectionChange(feature.agentName!, modelId)}
                      layout="horizontal"
                      size="small"
                    />
                  </div>
                </ConfigPageRow>
              )}
            </ConfigPageSection>
          );
        })}
      </ConfigPageContent>
    </ConfigPageLayout>
  );
};

export default AIFeaturesConfig;
