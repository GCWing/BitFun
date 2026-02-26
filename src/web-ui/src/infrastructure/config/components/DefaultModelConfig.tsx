 

import React, { useState, useEffect, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import {
  Layers,
} from 'lucide-react';
import { Select, CubeLoading } from '@/component-library';
import { notificationService } from '@/shared/notification-system';
import { configManager } from '../services/ConfigManager';
import type {
  AIModelConfig,
  DefaultModels,
  OptionalCapabilityModels,
  OptionalCapabilityType,
} from '../types';
import { ConfigPageRow } from './common';
import { createLogger } from '@/shared/utils/logger';
import './DefaultModelConfig.scss';

const log = createLogger('DefaultModelConfig');


const OPTIONAL_CAPABILITY_TYPES: OptionalCapabilityType[] = [
  'image_understanding',
  'image_generation',
  'search',
  'speech_recognition'
];

export const DefaultModelConfig: React.FC = () => {
  const { t } = useTranslation('settings/default-model');
  const renderOptionalLabel = (text: string) => (
    <>
      {text}
      <span className="default-model-config__optional-label">（{t('core.optional')}）</span>
    </>
  );
  
  
  const [loading, setLoading] = useState(true);
  const [models, setModels] = useState<AIModelConfig[]>([]);
  const [defaultModels, setDefaultModels] = useState<DefaultModels>({ primary: null, fast: null });
  const [optionalCapabilities, setOptionalCapabilities] = useState<OptionalCapabilityModels>({});

  
  useEffect(() => {
    loadData();
  }, []);

  const loadData = async () => {
    try {
      setLoading(true);

      
      const [allModels, defaultModelsConfig] = await Promise.all([
        configManager.getConfig<AIModelConfig[]>('ai.models') || [],
        configManager.getConfig<any>('ai.default_models') || {},
      ]);

      setModels(allModels);

      
      setDefaultModels({
        primary: defaultModelsConfig?.primary || null,
        fast: defaultModelsConfig?.fast || null,
      });

      
      setOptionalCapabilities({
        image_understanding: defaultModelsConfig?.image_understanding,
        image_generation: defaultModelsConfig?.image_generation,
        search: defaultModelsConfig?.search,
        speech_recognition: defaultModelsConfig?.speech_recognition,
      });
    } catch (error) {
      log.error('Failed to load data', error);
      notificationService.error(t('messages.loadFailed'));
    } finally {
      setLoading(false);
    }
  };

  
  const getModelName = useCallback((modelId: string | null | undefined): string | undefined => {
    if (!modelId) return undefined;
    return models.find(m => m.id === modelId)?.name;
  }, [models]);

  
  const handleDefaultModelChange = async (slot: 'primary' | 'fast', modelId: string | number) => {
    try {
      const modelIdStr = modelId ? String(modelId) : null;

      
      const currentConfig = await configManager.getConfig<any>('ai.default_models') || {};

      
      await configManager.setConfig('ai.default_models', {
        ...currentConfig,
        [slot]: modelIdStr,
      });

      setDefaultModels(prev => ({
        ...prev,
        [slot]: modelIdStr,
      }));

      const modelName = getModelName(modelIdStr);
      notificationService.success(
        t('messages.modelUpdated', {
          slot: slot === 'primary' ? t('core.primary.label') : t('core.fast.label'),
          name: modelName || modelIdStr,
        }),
        { duration: 2000 }
      );
    } catch (error) {
      log.error('Failed to update default model', { slot, modelId: modelIdStr, error });
      notificationService.error(t('messages.updateFailed'));
    }
  };

  
  const handleCapabilityChange = async (capability: OptionalCapabilityType, modelId: string | number) => {
    try {
      const modelIdStr = modelId ? String(modelId) : null;

      
      const currentConfig = await configManager.getConfig<any>('ai.default_models') || {};

      
      await configManager.setConfig('ai.default_models', {
        ...currentConfig,
        [capability]: modelIdStr || undefined,
      });

      setOptionalCapabilities(prev => ({
        ...prev,
        [capability]: modelIdStr || undefined,
      }));

      notificationService.success(t('messages.capabilityUpdated'), { duration: 2000 });
    } catch (error) {
      log.error('Failed to update capability model', { capability, modelId: modelIdStr, error });
      notificationService.error(t('messages.updateFailed'));
    }
  };

  
  const enabledModels = models.filter(m => m.enabled);
  
  
  const getModelsForCapability = (capability: OptionalCapabilityType): AIModelConfig[] => {
    return enabledModels.filter(m => {
      switch (capability) {
        case 'image_understanding':
          return m.capabilities?.includes('image_understanding') || m.category === 'multimodal';
        case 'image_generation':
          return m.capabilities?.includes('image_generation') || m.category === 'image_generation';
        case 'search':
          return m.capabilities?.includes('search') || m.category === 'search_enhanced';
        case 'speech_recognition':
          return m.capabilities?.includes('speech_recognition') || m.category === 'speech_recognition';
        default:
          return true;
      }
    });
  };

  if (loading) {
    return (
      <div className="default-model-config__loading">
        <CubeLoading size="small" />
        <p>{t('loading')}</p>
      </div>
    );
  }

  if (models.length === 0) {
    return (
      <div className="default-model-config__empty">
        <Layers size={48} />
        <p>{t('empty.noModels')}</p>
      </div>
    );
  }

  return (
    <div className="default-model-config">
      <ConfigPageRow
        label={t('core.primary.label')}
        description={t('core.primary.description')}
        align="center"
      >
        <Select
          value={defaultModels.primary || ''}
          onChange={(value) => handleDefaultModelChange('primary', value)}
          placeholder={t('core.primary.placeholder')}
          options={enabledModels.map(model => ({
            label: model.name,
            value: model.id!,
          }))}
          disabled={enabledModels.length === 0}
          size="small"
        />
      </ConfigPageRow>

      <ConfigPageRow
        label={renderOptionalLabel(t('core.fast.label'))}
        description={t('core.fast.description')}
        align="center"
      >
        <Select
          value={defaultModels.fast || ''}
          onChange={(value) => handleDefaultModelChange('fast', value)}
          placeholder={t('core.fast.placeholder')}
          options={[
            { label: t('core.fast.notSet'), value: '' },
            ...enabledModels.map(model => ({
              label: model.name,
              value: model.id!,
            })),
          ]}
          size="small"
        />
      </ConfigPageRow>

      {OPTIONAL_CAPABILITY_TYPES.map(capability => {
        const availableModels = getModelsForCapability(capability);
        const configuredModelId = optionalCapabilities[capability];

        return (
          <ConfigPageRow
            key={capability}
            label={renderOptionalLabel(t(`optional.capabilities.${capability}.label`))}
            description={t(`optional.capabilities.${capability}.description`)}
            align="center"
          >
            <Select
              value={configuredModelId || ''}
              onChange={(value) => handleCapabilityChange(capability, value)}
              placeholder={t('optional.selectModel')}
              disabled={availableModels.length === 0}
              options={[
                { label: t('optional.notSet'), value: '' },
                ...availableModels.map(model => ({
                  label: model.name,
                  value: model.id!,
                })),
              ]}
              size="small"
            />
          </ConfigPageRow>
        );
      })}
    </div>
  );
};

export default DefaultModelConfig;
