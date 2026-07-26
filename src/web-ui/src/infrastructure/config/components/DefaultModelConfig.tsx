 

import React, { useState, useEffect, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { Layers } from 'lucide-react';
import { Select, CubeLoading } from '@/component-library';
import { notificationService } from '@/shared/notification-system';
import { configManager } from '../services/ConfigManager';
import type {
  AIModelConfig,
  DefaultModels,
} from '../types';
import { ConfigPageRow } from './common';
import { createLogger } from '@/shared/utils/logger';
import { useModelSelectPresentation } from './ModelSelectPresentation';
import './DefaultModelConfig.scss';

const log = createLogger('DefaultModelConfig');

const normalizeSelectValue = (value: string | number | (string | number)[]): string | number =>
  Array.isArray(value) ? (value[0] ?? '') : value;

type DefaultModelSlot = 'primary' | 'fast' | 'image_understanding' | 'speech_recognition';

export const DefaultModelConfig: React.FC = () => {
  const { t } = useTranslation('settings/default-model');
  const { buildModelOption, renderModelOption, renderModelValue } = useModelSelectPresentation();
  const renderOptionalLabel = (text: string) => (
    <>
      {text}
      <span className="default-model-config__optional-label">（{t('core.optional')}）</span>
    </>
  );
  
  
  const [loading, setLoading] = useState(true);
  const [models, setModels] = useState<AIModelConfig[]>([]);
  const [defaultModels, setDefaultModels] = useState<DefaultModels>({
    primary: null,
    fast: null,
    image_understanding: null,
    speech_recognition: null,
  });

  const loadData = useCallback(async () => {
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
        image_understanding: defaultModelsConfig?.image_understanding || null,
        speech_recognition: defaultModelsConfig?.speech_recognition || null,
      });
    } catch (error) {
      log.error('Failed to load data', error);
      notificationService.error(t('messages.loadFailed'));
    } finally {
      setLoading(false);
    }
  }, [t]);

  useEffect(() => {
    void loadData();

    const unsubscribeModels = configManager.watch('ai.models', () => {
      void loadData();
    });
    const unsubscribeDefaultModels = configManager.watch('ai.default_models', () => {
      void loadData();
    });

    return () => {
      unsubscribeModels();
      unsubscribeDefaultModels();
    };
  }, [loadData]);

  
  const getModelName = useCallback((modelId: string | null | undefined): string | undefined => {
    if (!modelId) return undefined;
    const model = models.find(m => m.id === modelId);
    return model?.model_name;
  }, [models]);

  
  const slotLabel = useCallback((slot: DefaultModelSlot): string => {
    switch (slot) {
      case 'primary':
        return t('core.primary.label');
      case 'fast':
        return t('core.fast.label');
      case 'image_understanding':
        return t('optional.capabilities.image_understanding.label');
      case 'speech_recognition':
        return t('optional.capabilities.speech_recognition.label');
      default: {
        const exhaustive: never = slot;
        return exhaustive;
      }
    }
  }, [t]);

  const handleDefaultModelChange = async (slot: DefaultModelSlot, modelId: string | number) => {
    const modelIdStr = modelId ? String(modelId) : null;
    try {
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
          slot: slotLabel(slot),
          name: modelName || modelIdStr,
        }),
        { duration: 2000 }
      );
    } catch (error) {
      log.error('Failed to update default model', { slot, modelId: modelIdStr, error });
      notificationService.error(t('messages.updateFailed'));
    }
  };

  
  const enabledModels = models.filter(m => m.enabled);
  const imageUnderstandingModels = enabledModels.filter(model => {
    const capabilities = Array.isArray(model.capabilities) ? model.capabilities : [];
    return model.category === 'multimodal' || capabilities.includes('image_understanding');
  });
  const speechRecognitionModels = enabledModels.filter(model => {
    const capabilities = Array.isArray(model.capabilities) ? model.capabilities : [];
    return model.category === 'speech_recognition' || capabilities.includes('speech_recognition');
  });

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
          onChange={(value) => handleDefaultModelChange('primary', normalizeSelectValue(value))}
          placeholder={t('core.primary.placeholder')}
          options={enabledModels.map(buildModelOption)}
          renderOption={renderModelOption}
          renderValue={renderModelValue}
          className="model-select-presentation__select"
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
          onChange={(value) => handleDefaultModelChange('fast', normalizeSelectValue(value))}
          placeholder={t('core.fast.placeholder')}
          options={[
            { label: t('core.fast.notSet'), value: '' },
            ...enabledModels.map(buildModelOption),
          ]}
          renderOption={renderModelOption}
          renderValue={renderModelValue}
          className="model-select-presentation__select"
          size="small"
        />
      </ConfigPageRow>

      <ConfigPageRow
        label={renderOptionalLabel(t('optional.capabilities.image_understanding.label'))}
        description={t('optional.capabilities.image_understanding.description')}
        align="center"
      >
        <Select
          value={defaultModels.image_understanding || ''}
          onChange={(value) => handleDefaultModelChange('image_understanding', normalizeSelectValue(value))}
          placeholder={t('optional.selectModel')}
          options={[
            { label: t('optional.notSet'), value: '' },
            ...imageUnderstandingModels.map(buildModelOption),
          ]}
          renderOption={renderModelOption}
          renderValue={renderModelValue}
          className="model-select-presentation__select"
          size="small"
        />
      </ConfigPageRow>

      <ConfigPageRow
        label={renderOptionalLabel(t('optional.capabilities.speech_recognition.label'))}
        description={t('optional.capabilities.speech_recognition.description')}
        align="center"
      >
        <Select
          value={defaultModels.speech_recognition || ''}
          onChange={(value) => handleDefaultModelChange('speech_recognition', normalizeSelectValue(value))}
          placeholder={t('optional.notSet')}
          options={[
            { label: t('optional.notSet'), value: '' },
            ...speechRecognitionModels.map(buildModelOption),
          ]}
          renderOption={renderModelOption}
          renderValue={renderModelValue}
          className="default-model-config__model-select"
          disabled={speechRecognitionModels.length === 0}
          size="small"
        />
      </ConfigPageRow>
    </div>
  );
};

export default DefaultModelConfig;
