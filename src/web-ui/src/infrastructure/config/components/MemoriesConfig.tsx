import React, { useCallback, useEffect, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { FolderOpen, RotateCcw, Trash2 } from 'lucide-react';
import {
  ConfigPageLoading,
  ConfirmDialog,
  IconButton,
  NumberInput,
  Select,
  Switch,
  type SelectOption,
} from '@/component-library';
import { useNotification } from '@/shared/notification-system';
import { createLogger } from '@/shared/utils/logger';
import { agentAPI } from '@/infrastructure/api/service-api/AgentAPI';
import { workspaceAPI } from '@/infrastructure/api/service-api/WorkspaceAPI';
import { configManager } from '../services/ConfigManager';
import { getModelDisplayName } from '../services/modelConfigs';
import type { AIModelConfig, MemoriesConfig as MemoriesConfigShape } from '../types';
import {
  ConfigPageContent,
  ConfigPageHeader,
  ConfigPageLayout,
  ConfigPageRow,
  ConfigPageSection,
} from './common';

const log = createLogger('MemoriesConfig');

const DEFAULT_MEMORIES_CONFIG: MemoriesConfigShape = {
  generate_memories: true,
  use_memories: true,
  external_context_policy: 'clear_tool_results',
  max_raw_memories_for_consolidation: 64,
  max_unused_days: 30,
  max_rollout_age_days: 10,
  max_rollouts_per_startup: 5,
  max_rollouts_scan_limit: 2000,
  min_rollout_idle_hours: 6,
  phase1_max_concurrency: 1,
  phase1_retry_backoff_minutes: 60,
  phase1_lease_seconds: 60 * 60,
  phase2_lease_seconds: 60 * 60,
  phase2_success_cooldown_seconds: 6 * 60 * 60,
  phase2_retry_delay_seconds: 60 * 60,
  extract_model: null,
  consolidation_model: null,
};

function normalizeSelectValue(value: string | number | (string | number)[]): string {
  const resolved = Array.isArray(value) ? value[0] : value;
  return resolved == null ? '' : String(resolved);
}

function normalizeMemoriesConfig(config: Partial<MemoriesConfigShape> | null | undefined): MemoriesConfigShape {
  const normalized = {
    ...DEFAULT_MEMORIES_CONFIG,
    ...(config ?? {}),
  };
  return {
    generate_memories: normalized.generate_memories,
    use_memories: normalized.use_memories,
    external_context_policy: normalized.external_context_policy,
    max_raw_memories_for_consolidation: normalized.max_raw_memories_for_consolidation,
    max_unused_days: normalized.max_unused_days,
    max_rollout_age_days: Math.min(normalized.max_rollout_age_days, normalized.max_unused_days),
    max_rollouts_per_startup: normalized.max_rollouts_per_startup,
    max_rollouts_scan_limit: normalized.max_rollouts_scan_limit,
    min_rollout_idle_hours: normalized.min_rollout_idle_hours,
    phase1_max_concurrency: normalized.phase1_max_concurrency,
    phase1_retry_backoff_minutes: normalized.phase1_retry_backoff_minutes,
    phase1_lease_seconds: normalized.phase1_lease_seconds,
    phase2_lease_seconds: normalized.phase2_lease_seconds,
    phase2_success_cooldown_seconds: normalized.phase2_success_cooldown_seconds,
    phase2_retry_delay_seconds: normalized.phase2_retry_delay_seconds,
    extract_model: normalized.extract_model,
    consolidation_model: normalized.consolidation_model,
  };
}

function isValidMemoryWindowConfig(config: MemoriesConfigShape): boolean {
  return config.max_rollout_age_days <= config.max_unused_days;
}

const MemoriesConfig: React.FC = () => {
  const { t } = useTranslation('settings/memories');
  const { error: notifyError, success: notifySuccess } = useNotification();
  const [loading, setLoading] = useState(true);
  const [config, setConfig] = useState<MemoriesConfigShape>(DEFAULT_MEMORIES_CONFIG);
  const [models, setModels] = useState<AIModelConfig[]>([]);
  const [savingKey, setSavingKey] = useState<keyof MemoriesConfigShape | null>(null);
  const [actionBusy, setActionBusy] = useState<'reset-settings' | 'open-directory' | 'reset-memory' | null>(null);
  const [resetMemoryConfirmOpen, setResetMemoryConfirmOpen] = useState(false);

  const loadData = useCallback(async () => {
    setLoading(true);
    try {
      const [loadedConfig, loadedModels] = await Promise.all([
        configManager.getConfig<Partial<MemoriesConfigShape>>('memories'),
        configManager.getConfig<AIModelConfig[]>('ai.models'),
      ]);
      setConfig(normalizeMemoriesConfig(loadedConfig));
      setModels(Array.isArray(loadedModels) ? loadedModels : []);
    } catch (error) {
      log.error('Failed to load memories config', error);
      notifyError(error instanceof Error ? error.message : t('messages.loadFailed'));
    } finally {
      setLoading(false);
    }
  }, [notifyError, t]);

  useEffect(() => {
    void loadData();
  }, [loadData]);

  const enabledModels = useMemo(() => models.filter((model) => model.enabled && model.id), [models]);

  const buildModelOptions = useCallback((followLabel: string): SelectOption[] => [
    { value: '', label: followLabel },
    { value: 'primary', label: t('models.primary') },
    { value: 'fast', label: t('models.fast') },
    ...enabledModels.map((model) => ({
      value: model.id as string,
      label: getModelDisplayName(model),
    })),
  ], [enabledModels, t]);

  const externalContextPolicyOptions = useMemo<SelectOption[]>(() => [
    { value: 'clear_tool_results', label: t('externalContextPolicy.clearToolResults') },
    { value: 'allow', label: t('externalContextPolicy.allow') },
    { value: 'skip_session', label: t('externalContextPolicy.skipSession') },
  ], [t]);

  const updateConfig = useCallback(async <K extends keyof MemoriesConfigShape>(
    key: K,
    value: MemoriesConfigShape[K],
  ) => {
    const previous = config;
    const next = {
      ...config,
      [key]: value,
    };
    if (!isValidMemoryWindowConfig(next)) {
      notifyError(t('messages.rolloutAgeExceedsRetention'));
      return;
    }
    setSavingKey(key);
    setConfig(next);
    try {
      await configManager.setConfig('memories', next);
      notifySuccess(t('messages.saved'));
    } catch (error) {
      log.error('Failed to save memories config', { key, error });
      setConfig(previous);
      notifyError(error instanceof Error ? error.message : t('messages.saveFailed'));
    } finally {
      setSavingKey(null);
    }
  }, [config, notifyError, notifySuccess, t]);

  const updateModelSelector = useCallback((
    key: 'extract_model' | 'consolidation_model',
    value: string | number | (string | number)[],
  ) => {
    const selector = normalizeSelectValue(value).trim();
    void updateConfig(key, selector ? selector : null);
  }, [updateConfig]);

  const handleResetSettings = useCallback(async () => {
    setActionBusy('reset-settings');
    try {
      await configManager.resetConfig('memories');
      await loadData();
      notifySuccess(t('messages.settingsReset'));
    } catch (error) {
      log.error('Failed to reset memories settings', error);
      notifyError(error instanceof Error ? error.message : t('messages.settingsResetFailed'));
    } finally {
      setActionBusy(null);
    }
  }, [loadData, notifyError, notifySuccess, t]);

  const handleOpenMemoryDirectory = useCallback(async () => {
    setActionBusy('open-directory');
    try {
      const paths = await agentAPI.getMemoryPaths();
      await workspaceAPI.revealInExplorer(paths.memoriesRootDir);
    } catch (error) {
      log.error('Failed to open memory directory', error);
      notifyError(error instanceof Error ? error.message : t('messages.openDirectoryFailed'));
    } finally {
      setActionBusy(null);
    }
  }, [notifyError, t]);

  const handleResetMemory = useCallback(async () => {
    setResetMemoryConfirmOpen(false);
    setActionBusy('reset-memory');
    try {
      await agentAPI.resetMemory();
      notifySuccess(t('messages.memoryReset'));
    } catch (error) {
      log.error('Failed to reset memory', error);
      notifyError(error instanceof Error ? error.message : t('messages.memoryResetFailed'));
    } finally {
      setActionBusy(null);
    }
  }, [notifyError, notifySuccess, t]);

  if (loading) {
    return (
      <ConfigPageLayout>
        <ConfigPageHeader title={t('title')} subtitle={t('subtitle')} />
        <ConfigPageContent>
          <ConfigPageLoading text={t('messages.loading')} />
        </ConfigPageContent>
      </ConfigPageLayout>
    );
  }

  const memoryWorkDisabled = !config.generate_memories;

  return (
    <ConfigPageLayout>
      <ConfigPageHeader
        title={t('title')}
        subtitle={t('subtitle')}
        extra={(
          <>
            <IconButton
              type="button"
              variant="ghost"
              size="small"
              onClick={() => void handleResetSettings()}
              isLoading={actionBusy === 'reset-settings'}
              disabled={actionBusy !== null}
              tooltip={t('actions.resetSettings')}
              tooltipPlacement="bottom"
              aria-label={t('actions.resetSettings')}
            >
              <RotateCcw />
            </IconButton>
            <IconButton
              type="button"
              variant="ghost"
              size="small"
              onClick={() => void handleOpenMemoryDirectory()}
              isLoading={actionBusy === 'open-directory'}
              disabled={actionBusy !== null}
              tooltip={t('actions.openDirectory')}
              tooltipPlacement="bottom"
              aria-label={t('actions.openDirectory')}
            >
              <FolderOpen />
            </IconButton>
            <IconButton
              type="button"
              variant="danger"
              size="small"
              onClick={() => setResetMemoryConfirmOpen(true)}
              isLoading={actionBusy === 'reset-memory'}
              disabled={actionBusy !== null}
              tooltip={t('actions.resetMemory')}
              tooltipPlacement="bottom"
              aria-label={t('actions.resetMemory')}
            >
              <Trash2 />
            </IconButton>
          </>
        )}
      />
      <ConfigPageContent>
        <ConfigPageSection title={t('sections.basic.title')} description={t('sections.basic.description')}>
          <ConfigPageRow
            label={t('fields.generateMemories.label')}
            description={t('fields.generateMemories.description')}
            align="center"
          >
            <Switch
              checked={config.generate_memories}
              onChange={(event) => void updateConfig('generate_memories', event.target.checked)}
              disabled={savingKey === 'generate_memories'}
              size="small"
            />
          </ConfigPageRow>

          <ConfigPageRow
            label={t('fields.useMemories.label')}
            description={t('fields.useMemories.description')}
            align="center"
          >
            <Switch
              checked={config.use_memories}
              onChange={(event) => void updateConfig('use_memories', event.target.checked)}
              disabled={savingKey === 'use_memories'}
              size="small"
            />
          </ConfigPageRow>

          <ConfigPageRow
            label={t('fields.externalContextPolicy.label')}
            description={t('fields.externalContextPolicy.description')}
            align="center"
          >
            <Select
              value={config.external_context_policy}
              onChange={(value) => {
                void updateConfig(
                  'external_context_policy',
                  normalizeSelectValue(value) as MemoriesConfigShape['external_context_policy'],
                );
              }}
              options={externalContextPolicyOptions}
              size="small"
              disabled={savingKey === 'external_context_policy' || memoryWorkDisabled}
            />
          </ConfigPageRow>
        </ConfigPageSection>

        <ConfigPageSection title={t('sections.extraction.title')} description={t('sections.extraction.description')}>
          <ConfigPageRow
            label={t('fields.minRolloutIdleHours.label')}
            description={t('fields.minRolloutIdleHours.description')}
            align="center"
          >
            <NumberInput
              value={config.min_rollout_idle_hours}
              onChange={(value) => void updateConfig('min_rollout_idle_hours', value)}
              min={1}
              max={48}
              step={1}
              unit={t('units.hours')}
              size="small"
              disabled={savingKey === 'min_rollout_idle_hours' || memoryWorkDisabled}
            />
          </ConfigPageRow>

          <ConfigPageRow
            label={t('fields.maxRolloutAgeDays.label')}
            description={t('fields.maxRolloutAgeDays.description')}
            align="center"
          >
            <NumberInput
              value={config.max_rollout_age_days}
              onChange={(value) => void updateConfig('max_rollout_age_days', value)}
              min={0}
              max={config.max_unused_days}
              step={1}
              unit={t('units.days')}
              size="small"
              disabled={savingKey === 'max_rollout_age_days' || memoryWorkDisabled}
            />
          </ConfigPageRow>

          <ConfigPageRow
            label={t('fields.maxRolloutsPerStartup.label')}
            description={t('fields.maxRolloutsPerStartup.description')}
            align="center"
          >
            <NumberInput
              value={config.max_rollouts_per_startup}
              onChange={(value) => void updateConfig('max_rollouts_per_startup', value)}
              min={1}
              max={128}
              step={1}
              size="small"
              disabled={savingKey === 'max_rollouts_per_startup' || memoryWorkDisabled}
            />
          </ConfigPageRow>

          <ConfigPageRow
            label={t('fields.maxRolloutsScanLimit.label')}
            description={t('fields.maxRolloutsScanLimit.description')}
            align="center"
          >
            <NumberInput
              value={config.max_rollouts_scan_limit}
              onChange={(value) => void updateConfig('max_rollouts_scan_limit', value)}
              min={1}
              max={50000}
              step={100}
              size="small"
              disabled={savingKey === 'max_rollouts_scan_limit' || memoryWorkDisabled}
            />
          </ConfigPageRow>

          <ConfigPageRow
            label={t('fields.phase1MaxConcurrency.label')}
            description={t('fields.phase1MaxConcurrency.description')}
            align="center"
          >
            <NumberInput
              value={config.phase1_max_concurrency}
              onChange={(value) => void updateConfig('phase1_max_concurrency', value)}
              min={1}
              max={16}
              step={1}
              size="small"
              disabled={savingKey === 'phase1_max_concurrency' || memoryWorkDisabled}
            />
          </ConfigPageRow>

          <ConfigPageRow
            label={t('fields.extractModel.label')}
            description={t('fields.extractModel.description')}
            align="center"
          >
            <Select
              value={config.extract_model ?? ''}
              onChange={(value) => updateModelSelector('extract_model', value)}
              options={buildModelOptions(t('models.followPrimary'))}
              size="small"
              disabled={savingKey === 'extract_model' || memoryWorkDisabled}
            />
          </ConfigPageRow>
        </ConfigPageSection>

        <ConfigPageSection title={t('sections.consolidation.title')} description={t('sections.consolidation.description')}>
          <ConfigPageRow
            label={t('fields.maxRawMemories.label')}
            description={t('fields.maxRawMemories.description')}
            align="center"
          >
            <NumberInput
              value={config.max_raw_memories_for_consolidation}
              onChange={(value) => void updateConfig('max_raw_memories_for_consolidation', value)}
              min={1}
              max={4096}
              step={1}
              size="small"
              disabled={savingKey === 'max_raw_memories_for_consolidation' || memoryWorkDisabled}
            />
          </ConfigPageRow>

          <ConfigPageRow
            label={t('fields.maxUnusedDays.label')}
            description={t('fields.maxUnusedDays.description')}
            align="center"
          >
            <NumberInput
              value={config.max_unused_days}
              onChange={(value) => void updateConfig('max_unused_days', value)}
              min={config.max_rollout_age_days}
              max={365}
              step={1}
              unit={t('units.days')}
              size="small"
              disabled={savingKey === 'max_unused_days' || memoryWorkDisabled}
            />
          </ConfigPageRow>

          <ConfigPageRow
            label={t('fields.consolidationModel.label')}
            description={t('fields.consolidationModel.description')}
            align="center"
          >
            <Select
              value={config.consolidation_model ?? ''}
              onChange={(value) => updateModelSelector('consolidation_model', value)}
              options={buildModelOptions(t('models.followExtraction'))}
              size="small"
              disabled={savingKey === 'consolidation_model' || memoryWorkDisabled}
            />
          </ConfigPageRow>
        </ConfigPageSection>
      </ConfigPageContent>
      <ConfirmDialog
        isOpen={resetMemoryConfirmOpen}
        onClose={() => setResetMemoryConfirmOpen(false)}
        onConfirm={() => void handleResetMemory()}
        title={t('actions.resetMemory')}
        message={t('actions.resetMemoryConfirm')}
        type="warning"
        confirmDanger
        confirmText={t('actions.resetMemoryConfirmAction')}
      />
    </ConfigPageLayout>
  );
};

export default MemoriesConfig;
