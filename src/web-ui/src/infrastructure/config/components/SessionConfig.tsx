import React, { useState, useEffect, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
// import { FolderOpen, RefreshCw, ChevronDown, Info } from 'lucide-react';
import { RefreshCw, ChevronDown, Info } from 'lucide-react';
import {
  Switch,
  NumberInput,
  Button,
  Input,
  Textarea,
  Card,
  CardBody,
  IconButton,
  ConfigPageLoading,
  Modal,
  Select,
  Tooltip,
  type SelectOption,
} from '@/component-library';
import { ConfigPageHeader, ConfigPageLayout, ConfigPageContent, ConfigPageSection, ConfigPageRow } from './common';
import { aiExperienceConfigService, type AIExperienceSettings } from '../services/AIExperienceConfigService';
import { configManager } from '../services/ConfigManager';
import { useNotification, notificationService } from '@/shared/notification-system';
import type { AIModelConfig, DebugModeConfig, LanguageDebugTemplate } from '../types';
import {
  LANGUAGE_TEMPLATE_LABELS,
  DEFAULT_DEBUG_MODE_CONFIG,
  ALL_LANGUAGES,
  DEFAULT_LANGUAGE_TEMPLATES,
} from '../types';
import { ModelSelectionRadio } from './ModelSelectionRadio';
// import { open } from '@tauri-apps/plugin-dialog';
import { createLogger } from '@/shared/utils/logger';
import './AIFeaturesConfig.scss';
import './DebugConfig.scss';

const log = createLogger('SessionSettingsPanels');

const AGENT_SESSION_TITLE = 'session-title-func-agent';

type SubagentBatchExecutionPolicy = 'safe_only' | 'force_parallel' | 'serial';

const DEFAULT_SUBAGENT_BATCH_EXECUTION_POLICY: SubagentBatchExecutionPolicy = 'force_parallel';
const DEFAULT_SUBAGENT_MAX_CONCURRENCY = 5;

function normalizeSubagentBatchExecutionPolicy(value: unknown): SubagentBatchExecutionPolicy {
  return value === 'force_parallel' || value === 'serial' || value === 'safe_only'
    ? value
    : DEFAULT_SUBAGENT_BATCH_EXECUTION_POLICY;
}

export type SessionSettingsPanelVariant = 'personalization' | 'permissions';

interface SessionSettingsPanelsProps {
  variant: SessionSettingsPanelVariant;
}


const SessionSettingsPanels: React.FC<SessionSettingsPanelsProps> = ({ variant }) => {
  const { t } = useTranslation('settings/session-config');
  const { t: tTools } = useTranslation('settings/agentic-tools');
  const { t: tDebug } = useTranslation('settings/debug');
  const notification = useNotification();

  // ── Session config state ─────────────────────────────────────────────────
  const [isLoading, setIsLoading] = useState(true);
  const [settings, setSettings] = useState<AIExperienceSettings | null>(null);
  const [models, setModels] = useState<AIModelConfig[]>([]);
  const [funcAgentModels, setFuncAgentModels] = useState<Record<string, string>>({});
  const [skipToolConfirmation, setSkipToolConfirmation] = useState(true);
  const [subagentMaxConcurrency, setSubagentMaxConcurrency] = useState(DEFAULT_SUBAGENT_MAX_CONCURRENCY);
  const [executionTimeout, setExecutionTimeout] = useState('');
  const [confirmationTimeout, setConfirmationTimeout] = useState('');
  const [subagentBatchExecutionPolicy, setSubagentBatchExecutionPolicy] =
    useState<SubagentBatchExecutionPolicy>(DEFAULT_SUBAGENT_BATCH_EXECUTION_POLICY);
  const [toolExecConfigLoading, setToolExecConfigLoading] = useState(false);

  // ── Debug mode config state ──────────────────────────────────────────────
  const [debugConfig, setDebugConfig] = useState<DebugModeConfig>(DEFAULT_DEBUG_MODE_CONFIG);
  const [debugHasChanges, setDebugHasChanges] = useState(false);
  const [debugSaving, setDebugSaving] = useState(false);
  const [expandedTemplates, setExpandedTemplates] = useState<Set<string>>(new Set());
  const [isTemplatesModalOpen, setIsTemplatesModalOpen] = useState(false);

  const loadAllData = useCallback(async () => {
    setIsLoading(true);
    try {
      const [
        loadedSettings,
        allModels,
        funcAgentModelsData,
        skipConfirm,
        loadedSubagentMaxConcurrency,
        execTimeout,
        confirmTimeout,
        loadedSubagentBatchExecutionPolicy,
        debugConfigData,
      ] = await Promise.all([
        aiExperienceConfigService.getSettingsAsync(),
        configManager.getConfig<AIModelConfig[]>('ai.models') || [],
        configManager.getConfig<Record<string, string>>('ai.func_agent_models') || {},
        configManager.getConfig<boolean>('ai.skip_tool_confirmation'),
        configManager.getConfig<number | null>('ai.subagent_max_concurrency'),
        configManager.getConfig<number | null>('ai.tool_execution_timeout_secs'),
        configManager.getConfig<number | null>('ai.tool_confirmation_timeout_secs'),
        configManager.getConfig<SubagentBatchExecutionPolicy>('ai.subagent_batch_execution_policy'),
        configManager.getConfig<DebugModeConfig>('ai.debug_mode_config'),
      ]);

      setSettings(loadedSettings);
      setModels(allModels as AIModelConfig[]);
      setFuncAgentModels(funcAgentModelsData as Record<string, string>);
      setSkipToolConfirmation(skipConfirm ?? true);
      setSubagentMaxConcurrency(loadedSubagentMaxConcurrency != null
        ? loadedSubagentMaxConcurrency
        : DEFAULT_SUBAGENT_MAX_CONCURRENCY);
      setExecutionTimeout(execTimeout != null ? String(execTimeout) : '');
      setConfirmationTimeout(confirmTimeout != null ? String(confirmTimeout) : '');
      setSubagentBatchExecutionPolicy(normalizeSubagentBatchExecutionPolicy(loadedSubagentBatchExecutionPolicy));
      if (debugConfigData) setDebugConfig(debugConfigData);
    } catch (error) {
      log.error('Failed to load session config data', error);
      setSettings(await aiExperienceConfigService.getSettingsAsync());
    } finally {
      setIsLoading(false);
    }
  }, []);

  useEffect(() => {
    loadAllData();
  }, [loadAllData]);

  // ── Session config handlers ──────────────────────────────────────────────

  const updateSetting = async <K extends keyof AIExperienceSettings>(
    key: K,
    value: AIExperienceSettings[K]
  ) => {
    if (!settings) return;
    const newSettings = { ...settings, [key]: value };
    setSettings(newSettings);
    try {
      await aiExperienceConfigService.saveSettings(newSettings);
      notification.success(t('messages.saveSuccess'));
    } catch (error) {
      log.error('Failed to save AI features settings', error);
      notification.error(t('messages.saveFailed'));
      setSettings(settings);
    }
  };

  const subagentBatchExecutionPolicyOptions: SelectOption[] = [
    {
      value: 'safe_only',
      label: tTools('config.subagentBatchPolicy.safeOnly'),
    },
    {
      value: 'force_parallel',
      label: tTools('config.subagentBatchPolicy.forceParallel'),
    },
  ];

  const subagentBatchPolicyLabel = (
    <span className="bitfun-func-agent-config__label-with-tooltip">
      <span>{tTools('config.subagentBatchPolicy.label')}</span>
      <Tooltip
        content={
          <span className="bitfun-func-agent-config__policy-tooltip">
            <strong>{tTools('config.subagentBatchPolicy.safeOnly')}</strong>
            <span>{tTools('config.subagentBatchPolicy.safeOnlyDesc')}</span>
            <strong>{tTools('config.subagentBatchPolicy.forceParallel')}</strong>
            <span>{tTools('config.subagentBatchPolicy.forceParallelDesc')}</span>
          </span>
        }
        placement="top"
      >
        <span className="bitfun-func-agent-config__label-tooltip-icon" aria-label={tTools('config.subagentBatchPolicy.tooltipLabel')}>
          <Info size={14} />
        </span>
      </Tooltip>
    </span>
  );

  const getModelName = useCallback((modelId: string | null | undefined): string | undefined => {
    if (!modelId) return undefined;
    return models.find(m => m.id === modelId)?.name;
  }, [models]);

  const handleAgentModelChange = async (agentKey: string, featureTitleKey: string, modelId: string) => {
    try {
      const current = await configManager.getConfig<Record<string, string>>('ai.func_agent_models') || {};
      const updated = { ...current, [agentKey]: modelId };
      await configManager.setConfig('ai.func_agent_models', updated);
      setFuncAgentModels(updated);

      let modelDesc = '';
      if (modelId === 'primary') {
        modelDesc = t('model.primary');
      } else if (modelId === 'fast') {
        modelDesc = t('model.fast');
      } else {
        modelDesc = getModelName(modelId) || modelId || '';
      }

      notificationService.success(
        t('models.updateSuccess', { agentName: t(featureTitleKey), modelName: modelDesc }),
        { duration: 2000 }
      );
    } catch (error) {
      log.error('Failed to update agent model', { agentKey, modelId, error });
      notificationService.error(t('messages.updateFailed'), { duration: 3000 });
    }
  };

  const handleSkipToolConfirmationChange = async (checked: boolean) => {
    setSkipToolConfirmation(checked);
    setToolExecConfigLoading(true);
    try {
      await configManager.setConfig('ai.skip_tool_confirmation', checked);
      notificationService.success(
        checked ? tTools('messages.autoExecuteEnabled') : tTools('messages.autoExecuteDisabled'),
        { duration: 2000 }
      );
      const { globalEventBus } = await import('@/infrastructure/event-bus');
      globalEventBus.emit('mode:config:updated');
    } catch (error) {
      log.error('Failed to save skip_tool_confirmation', error);
      notificationService.error(
        `${tTools('messages.saveFailed')}: ` + (error instanceof Error ? error.message : String(error))
      );
      setSkipToolConfirmation(!checked);
    } finally {
      setToolExecConfigLoading(false);
    }
  };

  const handleSubagentBatchExecutionPolicyChange = async (value: string | number | (string | number)[]) => {
    const nextPolicy = normalizeSubagentBatchExecutionPolicy(Array.isArray(value) ? value[0] : value);
    const previousPolicy = subagentBatchExecutionPolicy;
    setSubagentBatchExecutionPolicy(nextPolicy);
    setToolExecConfigLoading(true);
    try {
      await configManager.setConfig('ai.subagent_batch_execution_policy', nextPolicy);
      notificationService.success(tTools('messages.saveSuccess'), { duration: 2000 });
      const { globalEventBus } = await import('@/infrastructure/event-bus');
      globalEventBus.emit('mode:config:updated');
    } catch (error) {
      log.error('Failed to save subagent_batch_execution_policy', error);
      notificationService.error(
        `${tTools('messages.saveFailed')}: ` + (error instanceof Error ? error.message : String(error))
      );
      setSubagentBatchExecutionPolicy(previousPolicy);
    } finally {
      setToolExecConfigLoading(false);
    }
  };

  const handleSubagentMaxConcurrencyChange = async (value: number) => {
    if (Number.isNaN(value) || value < 1) return;
    setSubagentMaxConcurrency(value);
    try {
      await configManager.setConfig('ai.subagent_max_concurrency', value);
    } catch (error) {
      log.error('Failed to save subagent_max_concurrency', error);
      notificationService.error(tTools('messages.saveFailed'));
    }
  };

  const handleToolTimeoutChange = async (type: 'execution' | 'confirmation', value: string) => {
    const configKey =
      type === 'execution' ? 'ai.tool_execution_timeout_secs' : 'ai.tool_confirmation_timeout_secs';
    const trimmedValue = value.trim();
    if (trimmedValue !== '') {
      const numValue = parseInt(trimmedValue, 10);
      if (Number.isNaN(numValue) || numValue < 0) return;
    }
    if (type === 'execution') setExecutionTimeout(trimmedValue);
    else setConfirmationTimeout(trimmedValue);
    const numValue = trimmedValue === '' ? null : parseInt(trimmedValue, 10);
    try {
      await configManager.setConfig(configKey, numValue);
    } catch (error) {
      log.error('Failed to save tool timeout config', { type, error });
      notificationService.error(tTools('messages.saveFailed'));
    }
  };

  // ── Debug config handlers ────────────────────────────────────────────────

  // const updateDebugConfig = useCallback((updates: Partial<DebugModeConfig>) => {
  //   setDebugConfig(prev => ({ ...prev, ...updates }));
  //   setDebugHasChanges(true);
  // }, []);

  const saveDebugConfig = async () => {
    try {
      setDebugSaving(true);
      await configManager.setConfig('ai.debug_mode_config', debugConfig);
      setDebugHasChanges(false);
      notificationService.success(tDebug('messages.saveSuccess'), { duration: 2000 });
    } catch (error) {
      log.error('Failed to save debug config', error);
      notificationService.error(tDebug('messages.saveFailed'));
    } finally {
      setDebugSaving(false);
    }
  };

  const cancelDebugChanges = async () => {
    const data = await configManager.getConfig<DebugModeConfig>('ai.debug_mode_config');
    setDebugConfig(data ?? DEFAULT_DEBUG_MODE_CONFIG);
    setDebugHasChanges(false);
  };

  const handleModalSave = async () => {
    await saveDebugConfig();
    setIsTemplatesModalOpen(false);
  };

  const handleModalCancel = async () => {
    await cancelDebugChanges();
    setIsTemplatesModalOpen(false);
  };

  const resetDebugTemplates = async () => {
    try {
      await configManager.resetConfig('ai.debug_mode_config');
      const data = await configManager.getConfig<DebugModeConfig>('ai.debug_mode_config');
      setDebugConfig(data ?? DEFAULT_DEBUG_MODE_CONFIG);
      setDebugHasChanges(false);
      notificationService.success(tDebug('messages.resetSuccess'), { duration: 2000 });
    } catch (error) {
      log.error('Failed to reset debug config', error);
      notificationService.error(tDebug('messages.resetFailed'));
    }
  };

  const updateTemplate = useCallback((language: string, updates: Partial<LanguageDebugTemplate>) => {
    setDebugConfig(prev => ({
      ...prev,
      language_templates: {
        ...prev.language_templates,
        [language]: { ...prev.language_templates[language], ...updates },
      },
    }));
    setDebugHasChanges(true);
  }, []);

  const toggleTemplateEnabled = useCallback(async (language: string, currentEnabled: boolean) => {
    const newEnabled = !currentEnabled;
    const newConfig = {
      ...debugConfig,
      language_templates: {
        ...debugConfig.language_templates,
        [language]: { ...debugConfig.language_templates[language], enabled: newEnabled },
      },
    };
    setDebugConfig(newConfig);
    try {
      await configManager.setConfig('ai.debug_mode_config', newConfig);
      const templateName = debugConfig.language_templates[language]?.display_name || language;
      notificationService.success(
        newEnabled
          ? tDebug('messages.templateEnabled', { name: templateName })
          : tDebug('messages.templateDisabled', { name: templateName }),
        { duration: 2000 }
      );
    } catch (error) {
      log.error('Failed to save template toggle', { language, error });
      setDebugConfig(debugConfig);
      notificationService.error(tDebug('messages.saveFailed'));
    }
  }, [debugConfig, tDebug]);

  const toggleTemplateExpand = useCallback((language: string) => {
    setExpandedTemplates(prev => {
      const next = new Set(prev);
      if (next.has(language)) {
        next.delete(language);
      } else {
        next.add(language);
      }
      return next;
    });
  }, []);

  // const handleSelectLogPath = async () => {
  //   try {
  //     const selected = await open({
  //       multiple: false,
  //       directory: false,
  //       filters: [{ name: tDebug('fileDialog.logFile'), extensions: ['log', 'txt', 'ndjson'] }],
  //     });
  //     if (selected) {
  //       updateDebugConfig({ log_path: selected });
  //       notificationService.success(tDebug('messages.logPathUpdated'), { duration: 2000 });
  //     }
  //   } catch (error) {
  //     notificationService.error(
  //       `${tDebug('messages.selectFileFailed')}: ${error instanceof Error ? error.message : String(error)}`
  //     );
  //   }
  // };

  const getTemplateEntries = useCallback((): [string, LanguageDebugTemplate][] => {
    const entries: [string, LanguageDebugTemplate][] = [];
    for (const lang of ALL_LANGUAGES) {
      const template = debugConfig.language_templates?.[lang] ?? DEFAULT_LANGUAGE_TEMPLATES[lang];
      if (template) entries.push([lang, template]);
    }
    return entries;
  }, [debugConfig.language_templates]);

  // ── Derived values ───────────────────────────────────────────────────────

  const enabledModels = models.filter((m: AIModelConfig) => m.enabled);
  const sessionTitleModelId = funcAgentModels[AGENT_SESSION_TITLE] || 'fast';
  const templateEntries = getTemplateEntries();

  const pageTitle = variant === 'personalization'
    ? t('personalizationPage.title')
    : t('permissionsPage.title');
  const pageSubtitle = variant === 'personalization'
    ? t('personalizationPage.subtitle')
    : t('permissionsPage.subtitle');

  if (isLoading || !settings) {
    return (
      <ConfigPageLayout className="bitfun-func-agent-config">
        <ConfigPageHeader title={pageTitle} subtitle={pageSubtitle} />
        <ConfigPageContent className="bitfun-func-agent-config__content">
          <ConfigPageLoading text={t('loading.text')} />
        </ConfigPageContent>
      </ConfigPageLayout>
    );
  }

  return (
    <ConfigPageLayout className="bitfun-func-agent-config">
      <ConfigPageHeader title={pageTitle} subtitle={pageSubtitle} />

      <ConfigPageContent className="bitfun-func-agent-config__content">

        {variant === 'personalization' ? (
          <>

        {/* ── Auto session title ─────────────────────────────────── */}
        <ConfigPageSection
          title={t('features.sessionTitle.title')}
          description={t('features.sessionTitle.subtitle')}
        >
          <ConfigPageRow label={t('common.enable')} align="center">
            <div className="bitfun-func-agent-config__row-control">
              <Switch
                checked={settings.enable_session_title_generation}
                onChange={(e) => updateSetting('enable_session_title_generation', e.target.checked)}
                size="small"
              />
            </div>
          </ConfigPageRow>
          <ConfigPageRow
            className="bitfun-func-agent-config__model-row"
            label={t('model.label')}
            description={enabledModels.length === 0 ? t('models.empty') : undefined}
            align="center"
          >
            <div className="bitfun-func-agent-config__row-control bitfun-func-agent-config__row-control--model">
              <ModelSelectionRadio
                value={sessionTitleModelId}
                models={enabledModels}
                onChange={(modelId) => handleAgentModelChange(AGENT_SESSION_TITLE, 'features.sessionTitle.title', modelId)}
                layout="horizontal"
                size="small"
              />
            </div>
          </ConfigPageRow>
        </ConfigPageSection>

          </>
        ) : null}

        {variant === 'permissions' ? (
          <>

        {/* ── Accelerated workspace search ───────────────────────── */}
        <ConfigPageSection
          title={t('features.workspaceSearch.title')}
          description={t('features.workspaceSearch.subtitle')}
        >
          <ConfigPageRow label={t('features.workspaceSearch.enable')} align="center">
            <div className="bitfun-func-agent-config__row-control">
              <Switch
                checked={settings.enable_workspace_search}
                onChange={(e) => updateSetting('enable_workspace_search', e.target.checked)}
                size="small"
              />
            </div>
          </ConfigPageRow>
        </ConfigPageSection>

        {/* ── Tool execution behavior ────────────────────────────── */}
        <ConfigPageSection
          title={t('toolExecution.sectionTitle')}
          description={t('toolExecution.sectionDescription')}
        >
          <ConfigPageRow label={tTools('config.autoExecute')} description={tTools('config.autoExecuteDesc')} align="center">
            <div className="bitfun-func-agent-config__row-control">
              <Switch
                checked={skipToolConfirmation}
                onChange={(e) => handleSkipToolConfirmationChange(e.target.checked)}
                disabled={toolExecConfigLoading}
                size="small"
              />
            </div>
          </ConfigPageRow>
          <ConfigPageRow
            label={(
              <span className="bitfun-func-agent-config__inline-label">
                <span>{tTools('config.confirmTimeout')}</span>
                <Tooltip content={tTools('config.confirmTimeoutHint')} placement="top">
                  <span
                    className="bitfun-func-agent-config__inline-info"
                    role="button"
                    tabIndex={0}
                    aria-label={tTools('config.confirmTimeoutHint')}
                  >
                    <Info size={14} />
                  </span>
                </Tooltip>
              </span>
            )}
            description={tTools('config.confirmTimeoutDesc')}
            align="center"
          >
            <div className="bitfun-func-agent-config__row-control">
              <NumberInput
                value={confirmationTimeout === '' ? 0 : parseInt(confirmationTimeout, 10)}
                onChange={(val) => handleToolTimeoutChange('confirmation', val === 0 ? '' : String(val))}
                min={0}
                max={3600}
                step={5}
                unit={tTools('config.seconds')}
                size="small"
                variant="compact"
              />
            </div>
          </ConfigPageRow>
          <ConfigPageRow
            label={(
              <span className="bitfun-func-agent-config__inline-label">
                <span>{tTools('config.executionTimeout')}</span>
                <Tooltip content={tTools('config.executionTimeoutHint')} placement="top">
                  <span
                    className="bitfun-func-agent-config__inline-info"
                    role="button"
                    tabIndex={0}
                    aria-label={tTools('config.executionTimeoutHint')}
                  >
                    <Info size={14} />
                  </span>
                </Tooltip>
              </span>
            )}
            description={tTools('config.executionTimeoutDesc')}
            align="center"
          >
            <div className="bitfun-func-agent-config__row-control">
              <NumberInput
                value={executionTimeout === '' ? 0 : parseInt(executionTimeout, 10)}
                onChange={(val) => handleToolTimeoutChange('execution', val === 0 ? '' : String(val))}
                min={0}
                max={3600}
                step={5}
                unit={tTools('config.seconds')}
                size="small"
                variant="compact"
              />
            </div>
          </ConfigPageRow>
          <ConfigPageRow label={subagentBatchPolicyLabel} description={tTools('config.subagentBatchPolicy.desc')} align="center">
            <div className="bitfun-func-agent-config__row-control">
              <Select
                value={subagentBatchExecutionPolicy}
                options={subagentBatchExecutionPolicyOptions}
                size="small"
                disabled={toolExecConfigLoading}
                onChange={handleSubagentBatchExecutionPolicyChange}
              />
            </div>
          </ConfigPageRow>
          <ConfigPageRow
            label={(
              <span className="bitfun-func-agent-config__inline-label">
                <span>{tTools('config.subagentMaxConcurrency')}</span>
              </span>
            )}
            description={tTools('config.subagentMaxConcurrencyDesc')}
            align="center"
          >
            <div className="bitfun-func-agent-config__row-control">
              <NumberInput
                value={subagentMaxConcurrency}
                onChange={(val) => void handleSubagentMaxConcurrencyChange(val)}
                min={1}
                max={100}
                step={1}
                size="small"
                variant="compact"
              />
            </div>
          </ConfigPageRow>
        </ConfigPageSection>

        {/* ── Debug mode settings ───────────────────────────────── */}
        {/* <ConfigPageSection
          title={tDebug('sections.combined')}
          description={tDebug('sections.combinedDescription')}
        >
          <ConfigPageRow
            label={tDebug('settings.logPath.label')}
            description={tDebug('settings.logPath.description')}
          >
            <div className="bitfun-debug-config__input-group">
              <Input
                value={debugConfig.log_path}
                onChange={(e) => updateDebugConfig({ log_path: e.target.value })}
                placeholder={tDebug('settings.logPath.placeholder')}
                variant="outlined"
                inputSize="small"
              />
              <IconButton
                variant="default"
                size="small"
                onClick={handleSelectLogPath}
                tooltip={tDebug('settings.logPath.browse')}
              >
                <FolderOpen size={16} />
              </IconButton>
            </div>
          </ConfigPageRow>

          <ConfigPageRow
            label={tDebug('settings.ingestPort.label')}
            description={tDebug('settings.ingestPort.description')}
            align="center"
          >
            <NumberInput
              value={debugConfig.ingest_port}
              onChange={(v) => updateDebugConfig({ ingest_port: v })}
              min={1024}
              max={65535}
              step={1}
              size="small"
            />
          </ConfigPageRow>

          {debugHasChanges && !isTemplatesModalOpen && (
            <ConfigPageRow label={tDebug('actions.save')} align="center">
              <div className="bitfun-debug-config__settings-actions">
                <Button
                  variant="primary"
                  size="small"
                  onClick={saveDebugConfig}
                  disabled={debugSaving}
                >
                  {debugSaving ? tDebug('actions.saving') : tDebug('actions.save')}
                </Button>
                <Button
                  variant="secondary"
                  size="small"
                  onClick={cancelDebugChanges}
                  disabled={debugSaving}
                >
                  {tDebug('actions.cancel')}
                </Button>
              </div>
            </ConfigPageRow>
          )}

          <ConfigPageRow
            label={tDebug('sections.templates')}
            description={tDebug('templates.description')}
            align="center"
          >
            <Button
              variant="secondary"
              size="small"
              onClick={() => setIsTemplatesModalOpen(true)}
            >
              {tDebug('templates.configure')}
            </Button>
          </ConfigPageRow>
        </ConfigPageSection> */}

        {/* ── Language templates modal ───────────────────────────── */}
        <Modal
          isOpen={isTemplatesModalOpen}
          onClose={() => setIsTemplatesModalOpen(false)}
          title={tDebug('sections.templates')}
          titleExtra={(
            <IconButton
              type="button"
              variant="ghost"
              size="xs"
              className="bitfun-debug-config__modal-reset-icon"
              onClick={resetDebugTemplates}
              tooltip={tDebug('templates.reset')}
              aria-label={tDebug('templates.reset')}
            >
              <RefreshCw size={12} strokeWidth={2} />
            </IconButton>
          )}
          size="large"
        >
          <div className="bitfun-debug-config__modal-body">
            {templateEntries.map(([language, template]) => {
              const isExpanded = expandedTemplates.has(language);
              return (
                <Card
                  key={language}
                  variant="default"
                  padding="none"
                  interactive
                  className={`bitfun-debug-config__template-card${isExpanded ? ' is-expanded' : ''}`}
                >
                  <div
                    className="bitfun-debug-config__template-header"
                    onClick={() => toggleTemplateExpand(language)}
                  >
                    <div className="bitfun-debug-config__template-info">
                      <div onClick={(e) => e.stopPropagation()}>
                        <Switch
                          checked={template.enabled}
                          onChange={() => toggleTemplateEnabled(language, template.enabled)}
                          size="small"
                        />
                      </div>
                      <span className="bitfun-debug-config__template-name">
                        {template.display_name || LANGUAGE_TEMPLATE_LABELS[language] || language}
                      </span>
                    </div>
                    <ChevronDown
                      size={16}
                      className={`bitfun-debug-config__template-arrow${isExpanded ? ' is-expanded' : ''}`}
                    />
                  </div>

                  {isExpanded && (
                    <CardBody className="bitfun-debug-config__template-content">
                      <div className="bitfun-debug-config__template-field">
                        <Textarea
                          label={tDebug('templates.instrumentation.label')}
                          value={template.instrumentation_template}
                          onChange={(e) => updateTemplate(language, { instrumentation_template: e.target.value })}
                          placeholder={tDebug('templates.instrumentation.placeholder')}
                          hint={`${tDebug('templates.instrumentation.placeholders')}: {LOCATION}, {MESSAGE}, {DATA}, {PORT}, {SESSION_ID}, {HYPOTHESIS_ID}, {RUN_ID}, {LOG_PATH}`}
                          variant="outlined"
                          autoResize
                        />
                      </div>
                      <div className="bitfun-debug-config__template-field">
                        <label className="bitfun-debug-config__template-label">
                          {tDebug('templates.region.label')}
                        </label>
                        <div className="bitfun-debug-config__region-inputs">
                          <Input
                            value={template.region_start}
                            onChange={(e) => updateTemplate(language, { region_start: e.target.value })}
                            placeholder={tDebug('templates.region.startPlaceholder')}
                            variant="outlined"
                            inputSize="small"
                          />
                          <Input
                            value={template.region_end}
                            onChange={(e) => updateTemplate(language, { region_end: e.target.value })}
                            placeholder={tDebug('templates.region.endPlaceholder')}
                            variant="outlined"
                            inputSize="small"
                          />
                        </div>
                      </div>
                      {template.notes && template.notes.length > 0 && (
                        <div className="bitfun-debug-config__template-field">
                          <label className="bitfun-debug-config__template-label">
                            {tDebug('templates.notes')}
                          </label>
                          <div className="bitfun-debug-config__template-notes">
                            {template.notes.map((note, idx) => (
                              <span key={idx} className="bitfun-debug-config__template-note">
                                {note}
                              </span>
                            ))}
                          </div>
                        </div>
                      )}
                    </CardBody>
                  )}
                </Card>
              );
            })}
          </div>

          {debugHasChanges && (
            <div className="bitfun-debug-config__modal-footer">
              <Button
                variant="primary"
                size="small"
                onClick={handleModalSave}
                disabled={debugSaving}
              >
                {debugSaving ? tDebug('actions.saving') : tDebug('actions.save')}
              </Button>
              <Button
                variant="secondary"
                size="small"
                onClick={handleModalCancel}
                disabled={debugSaving}
              >
                {tDebug('actions.cancel')}
              </Button>
            </div>
          )}
        </Modal>

          </>
        ) : null}

      </ConfigPageContent>
    </ConfigPageLayout>
  );
};

export function SessionPersonalizationConfig(): React.ReactElement {
  return <SessionSettingsPanels variant="personalization" />;
}

export function SessionPermissionsConfig(): React.ReactElement {
  return <SessionSettingsPanels variant="permissions" />;
}
