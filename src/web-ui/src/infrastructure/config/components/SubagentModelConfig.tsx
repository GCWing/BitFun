import React, { useCallback, useEffect, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Settings } from 'lucide-react';
import { IconButton, Select } from '@/component-library';
import { useSceneStore } from '@/app/stores/sceneStore';
import { useNotification } from '@/shared/notification-system';
import { configManager } from '../services/ConfigManager';
import type { AgentModelDefaultsConfig, AIModelConfig, SubagentModelSelection } from '../types';
import { ConfigPageRow } from './common';
import { type ModelSelectOption, useModelSelectPresentation } from './ModelSelectPresentation';
import './SubagentModelConfig.scss';

const DEFAULT_SUBAGENT_SELECTION: SubagentModelSelection = { kind: 'fixed', model_id: 'fast' };

function normalizeSelectValue(value: string | number | (string | number)[]): string {
  return String(Array.isArray(value) ? (value[0] ?? '') : value);
}

function selectionFromValue(value: string): SubagentModelSelection {
  return value === 'inherit'
    ? { kind: 'inherit' }
    : { kind: 'fixed', model_id: value };
}

function selectionValue(selection: SubagentModelSelection): string {
  return selection.kind === 'inherit' ? 'inherit' : selection.model_id;
}

export const SubagentModelConfig: React.FC = () => {
  const { t } = useTranslation('settings/ai-model');
  const { error: notifyError } = useNotification();
  const [isLoading, setIsLoading] = useState(true);
  const [isSaving, setIsSaving] = useState(false);
  const [models, setModels] = useState<AIModelConfig[]>([]);
  const [selection, setSelection] = useState<SubagentModelSelection>(DEFAULT_SUBAGENT_SELECTION);
  const { buildModelOption, renderModelOption, renderModelValue } = useModelSelectPresentation();
  const openScene = useSceneStore((state) => state.openScene);

  const loadData = useCallback(async () => {
    setIsLoading(true);
    try {
      const [configuredModels, defaults] = await Promise.all([
        configManager.getConfig<AIModelConfig[]>('ai.models'),
        configManager.getConfig<AgentModelDefaultsConfig>('ai.agent_model_defaults'),
      ]);
      setModels(configuredModels ?? []);
      setSelection(defaults?.subagents?.default ?? DEFAULT_SUBAGENT_SELECTION);
    } catch {
      notifyError(t('subagentModels.loadFailed'));
    } finally {
      setIsLoading(false);
    }
  }, [notifyError, t]);

  useEffect(() => {
    void loadData();
    const unwatchModels = configManager.watch('ai.models', () => void loadData());
    const unwatchDefaults = configManager.watch('ai.agent_model_defaults', () => void loadData());
    return () => {
      unwatchModels();
      unwatchDefaults();
    };
  }, [loadData]);

  const modelOptions = useMemo<ModelSelectOption[]>(() => [
    { label: t('subagentModels.options.inherit'), value: 'inherit' },
    { label: t('subagentModels.options.fast'), value: 'fast' },
    { label: t('subagentModels.options.primary'), value: 'primary' },
    { label: t('subagentModels.options.auto'), value: 'auto' },
    ...models
      .filter((model): model is AIModelConfig & { id: string } => (
        typeof model.id === 'string'
        && model.id.trim().length > 0
        && model.enabled !== false
        && (model.capabilities ?? []).includes('text_chat')
      ))
      .map(buildModelOption),
  ], [buildModelOption, models, t]);

  const handleChange = useCallback(async (
    value: string | number | (string | number)[],
  ) => {
    const nextSelection = selectionFromValue(normalizeSelectValue(value));
    setIsSaving(true);
    try {
      await configManager.setConfig('ai.agent_model_defaults.subagents.default', nextSelection);
      setSelection(nextSelection);
    } catch {
      notifyError(t('subagentModels.default.updateFailed'));
    } finally {
      setIsSaving(false);
    }
  }, [notifyError, t]);

  const openSubagentCustomization = useCallback((event: React.MouseEvent) => {
    event.preventDefault();
    event.stopPropagation();
    openScene('agents');
  }, [openScene]);

  return (
    <ConfigPageRow
      className="subagent-model-config__row"
      label={(
        <span className="subagent-model-config__label">
          <span>{t('subagentModels.default.label')}</span>
          <IconButton
            type="button"
            variant="ghost"
            size="xs"
            className="subagent-model-config__configure"
            tooltip={t('subagentModels.default.configureTooltip')}
            tooltipPlacement="top"
            tooltipFollowCursor={false}
            aria-label={t('subagentModels.default.configureTooltip')}
            onClick={openSubagentCustomization}
          >
            <Settings aria-hidden="true" />
          </IconButton>
        </span>
      )}
      description={t('subagentModels.default.description')}
      align="center"
    >
      <Select
        size="small"
        searchable
        className="model-select-presentation__select"
        options={modelOptions}
        value={selectionValue(selection)}
        onChange={(value) => void handleChange(value)}
        renderOption={renderModelOption}
        renderValue={renderModelValue}
        disabled={isLoading || isSaving}
        triggerTestId="settings-subagent-default-model-select"
      />
    </ConfigPageRow>
  );
};

export default SubagentModelConfig;
