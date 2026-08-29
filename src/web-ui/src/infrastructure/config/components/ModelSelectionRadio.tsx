import React, { useId, useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { Radio, Select } from '@bitfun/ui';
import type { AIModelConfig } from '../types';
import { getModelDisplayName } from '../services/modelConfigs';
import './ModelSelectionRadio.scss';

export interface ModelSelectionRadioProps {
  value: string;
  models: AIModelConfig[];
  onChange: (modelId: string) => void;
  disabled?: boolean;
  layout?: 'horizontal' | 'vertical';
  size?: 'small' | 'medium';
}

const isSpecialModel = (value: string): value is 'primary' | 'fast' => {
  return value === 'primary' || value === 'fast';
};

export const ModelSelectionRadio: React.FC<ModelSelectionRadioProps> = ({
  value,
  models,
  onChange,
  disabled = false,
  layout = 'horizontal',
  size = 'medium',
}) => {
  const { t } = useTranslation('settings/default-model');
  const uniqueId = useId();
  const radioName = `model-selection-${uniqueId}`;

  const selectionType = useMemo<'primary' | 'fast' | 'custom'>(() => {
    if (value === 'primary') return 'primary';
    if (value === 'fast') return 'fast';
    return 'custom';
  }, [value]);

  const customModelId = useMemo(() => {
    return isSpecialModel(value) ? undefined : value;
  }, [value]);

  const handleSelectionChange = (selection: 'primary' | 'fast' | 'custom') => {
    if (selection === 'custom') {
      const newModelId = customModelId || models[0]?.id || 'primary';
      onChange(newModelId);
    } else {
      onChange(selection);
    }
  };

  const handleCustomModelChange = (modelId: string | number | (string | number)[]) => {
    if (Array.isArray(modelId)) {
      onChange(String(modelId[0]));
    } else {
      onChange(String(modelId));
    }
  };

  const enabledModels = models.filter(m => m.enabled);

  return (
    <div
      className={`model-selection-radio model-selection-radio--${layout} model-selection-radio--${size}`}
      data-bf-component="config"
      data-bf-part="modelSelection"
    >
      <div
        className={`model-selection-radio__option ${selectionType === 'primary' ? 'model-selection-radio__option--selected' : ''}`}
        data-bf-component="config"
        data-bf-part="modelSelectionOption"
      >
        <Radio
          className="model-selection-radio__choice"
          name={radioName}
          value="primary"
          checked={selectionType === 'primary'}
          onCheckedChange={(checked) => checked && handleSelectionChange('primary')}
          disabled={disabled}
          size={size}
          label={<span className="model-selection-radio__label" data-bf-component="config" data-bf-part="modelSelectionLabel">{t('selection.primary')}</span>}
        />
      </div>

      <div
        className={`model-selection-radio__option ${selectionType === 'fast' ? 'model-selection-radio__option--selected' : ''}`}
        data-bf-component="config"
        data-bf-part="modelSelectionOption"
      >
        <Radio
          className="model-selection-radio__choice"
          name={radioName}
          value="fast"
          checked={selectionType === 'fast'}
          onCheckedChange={(checked) => checked && handleSelectionChange('fast')}
          disabled={disabled}
          size={size}
          label={<span className="model-selection-radio__label" data-bf-component="config" data-bf-part="modelSelectionLabel">{t('selection.fast')}</span>}
        />
      </div>

      <div
        className={`model-selection-radio__option model-selection-radio__option--custom ${selectionType === 'custom' ? 'model-selection-radio__option--selected' : ''}`}
        data-bf-component="config"
        data-bf-part="modelSelectionOption"
      >
        <Radio
          className="model-selection-radio__choice"
          name={radioName}
          value="custom"
          checked={selectionType === 'custom'}
          onCheckedChange={(checked) => checked && handleSelectionChange('custom')}
          disabled={disabled}
          size={size}
          label={<span className="model-selection-radio__label" data-bf-component="config" data-bf-part="modelSelectionLabel">{t('selection.custom')}</span>}
        />

        {selectionType === 'custom' && (
          <div className="model-selection-radio__dropdown" data-bf-component="config" data-bf-part="modelSelectionDropdown">
            <Select
              value={customModelId || ''}
              onValueChange={handleCustomModelChange}
              disabled={disabled}
              placeholder={t('selection.selectModel')}
              options={enabledModels.map(model => ({
                label: getModelDisplayName(model),
                value: model.id!,
              }))}
              size="sm"
            />
          </div>
        )}
      </div>
    </div>
  );
};

export default ModelSelectionRadio;
