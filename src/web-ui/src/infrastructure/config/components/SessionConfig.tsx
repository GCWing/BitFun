import React, { useState, useEffect, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { FolderOpen, RefreshCw, ChevronDown, Plus, Trash2 } from 'lucide-react';
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
  type SelectOption,
} from '@/component-library';
import { ConfigPageHeader, ConfigPageLayout, ConfigPageContent, ConfigPageSection, ConfigPageRow } from './common';
import { aiExperienceConfigService, type AIExperienceSettings } from '../services/AIExperienceConfigService';
import {
  deleteAgentCompanionPetPackage,
  importAgentCompanionPetPackage,
  listAgentCompanionPets,
  releaseAgentCompanionPetPreviewBlobs,
  type AgentCompanionPetPackage,
} from '../services/AgentCompanionPetService';
import { configManager } from '../services/ConfigManager';
import { systemAPI } from '@/infrastructure/api/service-api/SystemAPI';
import { useNotification, notificationService } from '@/shared/notification-system';
import type { AIModelConfig, DebugModeConfig, LanguageDebugTemplate } from '../types';
import {
  LANGUAGE_TEMPLATE_LABELS,
  DEFAULT_DEBUG_MODE_CONFIG,
  ALL_LANGUAGES,
  DEFAULT_LANGUAGE_TEMPLATES,
} from '../types';
import { ModelSelectionRadio } from './ModelSelectionRadio';
import { ChatInputPixelPet } from '@/flow_chat/components/ChatInputPixelPet';
import { ask, open } from '@tauri-apps/plugin-dialog';
import { createLogger } from '@/shared/utils/logger';
import './AIFeaturesConfig.scss';
import './DebugConfig.scss';

const log = createLogger('SessionConfig');

const IS_TAURI_DESKTOP = typeof window !== 'undefined' && '__TAURI__' in window;

const AGENT_SESSION_TITLE = 'session-title-func-agent';

type ComputerUseStatusPayload = {
  computerUseEnabled: boolean;
  accessibilityGranted: boolean;
  screenCaptureGranted: boolean;
  platformNote: string | null;
};

type BrowserControlLaunchResponse = {
  success: boolean;
  status: string;
  message: string | null;
  browserKind: string;
};

const DEFAULT_COMPANION_PET_VALUE = '__default_panda__';

const SessionConfig: React.FC = () => {
  const { t } = useTranslation('settings/session-config');
  const { t: tTools } = useTranslation('settings/agentic-tools');
  const { t: tDebug } = useTranslation('settings/debug');
  const notification = useNotification();

  // ── Session config state ─────────────────────────────────────────────────
  const [isLoading, setIsLoading] = useState(true);
  const [settings, setSettings] = useState<AIExperienceSettings | null>(null);
  const [companionPets, setCompanionPets] = useState<AgentCompanionPetPackage[]>([]);
  const [companionPetsLoading, setCompanionPetsLoading] = useState(false);
  const [companionPetImporting, setCompanionPetImporting] = useState(false);
  const [companionPetDeletingPath, setCompanionPetDeletingPath] = useState<string | null>(null);
  const [models, setModels] = useState<AIModelConfig[]>([]);
  const [funcAgentModels, setFuncAgentModels] = useState<Record<string, string>>({});
  const [skipToolConfirmation, setSkipToolConfirmation] = useState(true);
  const [executionTimeout, setExecutionTimeout] = useState('');
  const [confirmationTimeout, setConfirmationTimeout] = useState('');
  const [maxRounds, setMaxRounds] = useState<number>(200);
  const [toolExecConfigLoading, setToolExecConfigLoading] = useState(false);

  const [computerUseEnabled, setComputerUseEnabled] = useState(false);
  const [computerUseAccess, setComputerUseAccess] = useState(false);
  const [computerUseScreen, setComputerUseScreen] = useState(false);
  const [computerUseBusy, setComputerUseBusy] = useState(false);
  const [computerUseStatusLoading, setComputerUseStatusLoading] = useState(false);

  // ── Browser control state ───────────────────────────────────────────────
  const [browserCdpAvailable, setBrowserCdpAvailable] = useState(false);
  const [browserKind, setBrowserKind] = useState('');
  const [browserVersion, setBrowserVersion] = useState<string | null>(null);
  const [browserPageCount, setBrowserPageCount] = useState(0);
  const [browserControlBusy, setBrowserControlBusy] = useState(false);
  const [browserStatusLoading, setBrowserStatusLoading] = useState(false);
  const [platform, setPlatform] = useState<string>('');
  const [browserRestartPrompt, setBrowserRestartPrompt] = useState<BrowserControlLaunchResponse | null>(null);

  // ── Debug mode config state ──────────────────────────────────────────────
  const [debugConfig, setDebugConfig] = useState<DebugModeConfig>(DEFAULT_DEBUG_MODE_CONFIG);
  const [debugHasChanges, setDebugHasChanges] = useState(false);
  const [debugSaving, setDebugSaving] = useState(false);
  const [expandedTemplates, setExpandedTemplates] = useState<Set<string>>(new Set());
  const [isTemplatesModalOpen, setIsTemplatesModalOpen] = useState(false);

  const refreshComputerUseStatus = useCallback(async (): Promise<boolean> => {
    if (!IS_TAURI_DESKTOP) return false;
    setComputerUseStatusLoading(true);
    try {
      const { invoke } = await import('@tauri-apps/api/core');
      const s = await invoke<ComputerUseStatusPayload>('computer_use_get_status');
      setComputerUseEnabled(s.computerUseEnabled);
      setComputerUseAccess(s.accessibilityGranted);
      setComputerUseScreen(s.screenCaptureGranted);
      return true;
    } catch (error) {
      log.error('computer_use_get_status failed', error);
      return false;
    } finally {
      setComputerUseStatusLoading(false);
    }
  }, []);

  const refreshBrowserControlStatus = useCallback(async () => {
    if (!IS_TAURI_DESKTOP) return;
    setBrowserStatusLoading(true);
    try {
      const { invoke } = await import('@tauri-apps/api/core');
      const s = await invoke<{
        cdpAvailable: boolean;
        browserKind: string;
        browserVersion: string | null;
        port: number;
        pageCount: number;
      }>('browser_control_get_status', { request: { port: 9222 } });
      setBrowserCdpAvailable(s.cdpAvailable);
      setBrowserKind(s.browserKind);
      setBrowserVersion(s.browserVersion);
      setBrowserPageCount(s.pageCount);
    } catch (error) {
      log.error('browser_control_get_status failed', error);
    } finally {
      setBrowserStatusLoading(false);
    }
  }, []);

  const refreshDesktopStatus = useCallback((computerUseCfg: boolean | null | undefined) => {
    if (!IS_TAURI_DESKTOP) {
      setComputerUseEnabled(computerUseCfg ?? false);
      return;
    }

    void refreshComputerUseStatus().then((ok) => {
      if (!ok) setComputerUseEnabled(computerUseCfg ?? false);
    });

    void refreshBrowserControlStatus();

    void systemAPI.getSystemInfo()
      .then((info) => setPlatform(info.platform || ''))
      .catch((error) => log.warn('getSystemInfo failed', error));
  }, [refreshComputerUseStatus, refreshBrowserControlStatus]);

  const loadAllData = useCallback(async () => {
    setIsLoading(true);
    try {
      const [
        loadedSettings,
        allModels,
        funcAgentModelsData,
        skipConfirm,
        execTimeout,
        confirmTimeout,
        debugConfigData,
        computerUseCfg,
        maxRoundsValue,
        loadedCompanionPets,
      ] = await Promise.all([
        aiExperienceConfigService.getSettingsAsync(),
        configManager.getConfig<AIModelConfig[]>('ai.models') || [],
        configManager.getConfig<Record<string, string>>('ai.func_agent_models') || {},
        configManager.getConfig<boolean>('ai.skip_tool_confirmation'),
        configManager.getConfig<number | null>('ai.tool_execution_timeout_secs'),
        configManager.getConfig<number | null>('ai.tool_confirmation_timeout_secs'),
        configManager.getConfig<DebugModeConfig>('ai.debug_mode_config'),
        configManager.getConfig<boolean>('ai.computer_use_enabled'),
        configManager.getConfig<number>('ai.max_rounds'),
        listAgentCompanionPets(),
      ]);

      setSettings(loadedSettings);
      setCompanionPets(loadedCompanionPets);
      setModels(allModels as AIModelConfig[]);
      setFuncAgentModels(funcAgentModelsData as Record<string, string>);
      setSkipToolConfirmation(skipConfirm ?? true);
      setExecutionTimeout(execTimeout != null ? String(execTimeout) : '');
      setConfirmationTimeout(confirmTimeout != null ? String(confirmTimeout) : '');
      setMaxRounds(maxRoundsValue ?? 200);
      if (debugConfigData) setDebugConfig(debugConfigData);

      refreshDesktopStatus(computerUseCfg);
    } catch (error) {
      log.error('Failed to load session config data', error);
      setSettings(await aiExperienceConfigService.getSettingsAsync());
    } finally {
      setIsLoading(false);
    }
  }, [refreshDesktopStatus]);

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

  const handleRefreshCompanionPets = async () => {
    setCompanionPetsLoading(true);
    try {
      setCompanionPets(await listAgentCompanionPets());
    } finally {
      setCompanionPetsLoading(false);
    }
  };

  const handleImportCompanionPet = async () => {
    if (!IS_TAURI_DESKTOP) return;
    setCompanionPetImporting(true);
    try {
      const selected = await open({
        directory: false,
        multiple: false,
        title: t('features.agentCompanion.importDialogTitle'),
        filters: [{ name: 'Petdex', extensions: ['zip'] }],
      });
      if (!selected || Array.isArray(selected)) return;
      const imported = await importAgentCompanionPetPackage(selected);
      const refreshed = await listAgentCompanionPets();
      setCompanionPets(refreshed);
      await updateSetting('agent_companion_pet', {
        id: imported.id,
        displayName: imported.displayName,
        description: imported.description,
        source: imported.source,
        packagePath: imported.packagePath,
        spritesheetPath: imported.spritesheetPath,
        spritesheetMimeType: imported.spritesheetMimeType,
      });
    } catch (error) {
      log.error('Failed to import Agent companion pet', error);
      notification.error(t('features.agentCompanion.importFailed'));
    } finally {
      setCompanionPetImporting(false);
    }
  };

  const handleDeleteCompanionPet = async (event: React.MouseEvent, pet: AgentCompanionPetPackage) => {
    event.preventDefault();
    event.stopPropagation();
    if (!IS_TAURI_DESKTOP || pet.source !== 'user' || !settings) return;
    const confirmed = await ask(t('features.agentCompanion.deleteConfirmBody'), {
      title: t('features.agentCompanion.deleteConfirmTitle'),
      kind: 'warning',
    });
    if (!confirmed) return;
    setCompanionPetDeletingPath(pet.packagePath);
    try {
      await deleteAgentCompanionPetPackage(pet.packagePath);
      releaseAgentCompanionPetPreviewBlobs(pet.packagePath, pet.spritesheetPath);
      const refreshed = await listAgentCompanionPets();
      setCompanionPets(refreshed);
      if (settings.agent_companion_pet?.packagePath === pet.packagePath) {
        const next = { ...settings, agent_companion_pet: null };
        setSettings(next);
        await aiExperienceConfigService.saveSettings(next);
      }
      notification.success(t('features.agentCompanion.deleteSuccess'));
    } catch (error) {
      log.error('Failed to delete Agent companion pet', error);
      notification.error(t('features.agentCompanion.deleteFailed'));
    } finally {
      setCompanionPetDeletingPath(null);
    }
  };

  const companionPetOptions: SelectOption[] = [
    {
      value: DEFAULT_COMPANION_PET_VALUE,
      label: t('features.agentCompanion.defaultPet'),
      group: t('features.agentCompanion.groupBuiltin'),
    },
    ...companionPets.map(pet => ({
      value: pet.packagePath,
      label: pet.displayName,
      description: pet.description ?? undefined,
      group: pet.source === 'preset'
        ? t('features.agentCompanion.groupPreset')
        : t('features.agentCompanion.groupImported'),
    })),
  ];

  const selectedCompanionPet = settings?.agent_companion_pet
    ? companionPets.find(pet => pet.packagePath === settings.agent_companion_pet?.packagePath)
      ?? settings.agent_companion_pet
    : null;

  const handleCompanionPetChange = async (value: string | number | (string | number)[]) => {
    const selectedValue = String(Array.isArray(value) ? value[0] : value);
    if (selectedValue === DEFAULT_COMPANION_PET_VALUE) {
      await updateSetting('agent_companion_pet', null);
      return;
    }
    const pet = companionPets.find(item => item.packagePath === selectedValue);
    if (!pet) return;
    await updateSetting('agent_companion_pet', {
      id: pet.id,
      displayName: pet.displayName,
      description: pet.description,
      source: pet.source,
      packagePath: pet.packagePath,
      spritesheetPath: pet.spritesheetPath,
      spritesheetMimeType: pet.spritesheetMimeType,
    });
  };

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

  const handleComputerUseEnabledChange = async (checked: boolean) => {
    setComputerUseBusy(true);
    setComputerUseEnabled(checked);
    try {
      await configManager.setConfig('ai.computer_use_enabled', checked);
      const { globalEventBus } = await import('@/infrastructure/event-bus');
      globalEventBus.emit('mode:config:updated');
      notificationService.success(
        checked ? t('messages.saveSuccess') : t('messages.saveSuccess'),
        { duration: 2000 }
      );
      await refreshComputerUseStatus();
    } catch (error) {
      log.error('Failed to save computer_use_enabled', error);
      notificationService.error(t('messages.saveFailed'));
      setComputerUseEnabled(!checked);
    } finally {
      setComputerUseBusy(false);
    }
  };

  const handleComputerUseOpenSettings = async (pane: 'accessibility' | 'screen_capture') => {
    try {
      const { invoke } = await import('@tauri-apps/api/core');
      await invoke('computer_use_open_system_settings', { request: { pane } });
    } catch (error) {
      log.error('computer_use_open_system_settings failed', error);
      notificationService.error(t('messages.saveFailed'));
    }
  };

  const handleBrowserControlLaunch = async () => {
    setBrowserControlBusy(true);
    try {
      const { invoke } = await import('@tauri-apps/api/core');
      const result = await invoke<BrowserControlLaunchResponse>('browser_control_launch', { request: { port: 9222 } });
      if (result.success) {
        notificationService.success(
          t('browserControl.connectSuccess', { browser: result.browserKind }),
          { duration: 3000 }
        );
      } else if (result.status === 'needs_restart') {
        setBrowserRestartPrompt(result);
      } else if (result.message) {
        notificationService.info(result.message, { duration: 8000 });
      }
      await refreshBrowserControlStatus();
    } catch (error) {
      log.error('browser_control_launch failed', error);
      notificationService.error(t('browserControl.connectFailed'));
    } finally {
      setBrowserControlBusy(false);
    }
  };

  const handleBrowserControlRestart = async () => {
    if (!browserRestartPrompt) return;
    setBrowserControlBusy(true);
    try {
      const { invoke } = await import('@tauri-apps/api/core');
      const result = await invoke<BrowserControlLaunchResponse>('browser_control_restart_with_cdp', {
        request: { port: 9222 },
      });
      if (result.success) {
        notificationService.success(
          t('browserControl.restartSuccess', { browser: result.browserKind }),
          { duration: 3000 }
        );
        setBrowserRestartPrompt(null);
      } else if (result.message) {
        notificationService.info(result.message, { duration: 8000 });
      }
      await refreshBrowserControlStatus();
    } catch (error) {
      log.error('browser_control_restart_with_cdp failed', error);
      notificationService.error(t('browserControl.restartFailed'));
    } finally {
      setBrowserControlBusy(false);
    }
  };

  const handleBrowserControlCreateLauncher = async () => {
    setBrowserControlBusy(true);
    try {
      const { invoke } = await import('@tauri-apps/api/core');
      const path = await invoke<string>('browser_control_create_launcher');
      notificationService.success(
        t('browserControl.createLauncherSuccess', { path }),
        { duration: 5000 }
      );
    } catch (error) {
      log.error('browser_control_create_launcher failed', error);
      notificationService.error(t('browserControl.createLauncherFailed'));
    } finally {
      setBrowserControlBusy(false);
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

  const handleMaxRoundsChange = async (value: string) => {
    const trimmedValue = value.trim();
    if (trimmedValue !== '') {
      const numValue = parseInt(trimmedValue, 10);
      if (Number.isNaN(numValue) || numValue < 1) return;
      setMaxRounds(numValue);
      try {
        await configManager.setConfig('ai.max_rounds', numValue);
        notificationService.success(t('messages.saveSuccess'), { duration: 2000 });
      } catch (error) {
        log.error('Failed to save max_rounds config', error);
        notificationService.error(t('messages.saveFailed'));
      }
    }
  };

  // ── Debug config handlers ────────────────────────────────────────────────

  const updateDebugConfig = useCallback((updates: Partial<DebugModeConfig>) => {
    setDebugConfig(prev => ({ ...prev, ...updates }));
    setDebugHasChanges(true);
  }, []);

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

  const handleSelectLogPath = async () => {
    try {
      const selected = await open({
        multiple: false,
        directory: false,
        filters: [{ name: tDebug('fileDialog.logFile'), extensions: ['log', 'txt', 'ndjson'] }],
      });
      if (selected) {
        updateDebugConfig({ log_path: selected });
        notificationService.success(tDebug('messages.logPathUpdated'), { duration: 2000 });
      }
    } catch (error) {
      notificationService.error(
        `${tDebug('messages.selectFileFailed')}: ${error instanceof Error ? error.message : String(error)}`
      );
    }
  };

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
  const computerUseAccessLabel = computerUseStatusLoading
    ? t('loading.text')
    : computerUseAccess ? t('computerUse.granted') : t('computerUse.notGranted');
  const computerUseScreenLabel = computerUseStatusLoading
    ? t('loading.text')
    : computerUseScreen ? t('computerUse.granted') : t('computerUse.notGranted');
  const browserStatusLabel = browserCdpAvailable
    ? `${browserKind} · ${browserPageCount} ${t('browserControl.tabs')}`
    : browserStatusLoading ? t('loading.text') : t('browserControl.notConnected');

  if (isLoading || !settings) {
    return (
      <ConfigPageLayout className="bitfun-func-agent-config">
        <ConfigPageHeader title={t('title')} subtitle={t('subtitle')} />
        <ConfigPageContent className="bitfun-func-agent-config__content">
          <ConfigPageLoading text={t('loading.text')} />
        </ConfigPageContent>
      </ConfigPageLayout>
    );
  }

  return (
    <ConfigPageLayout className="bitfun-func-agent-config">
      <ConfigPageHeader title={t('title')} subtitle={t('subtitle')} />

      <ConfigPageContent className="bitfun-func-agent-config__content">

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

        {/* ── Agent companion (collapsed input) ─────────────────── */}
        <ConfigPageSection
          title={t('features.agentCompanion.title')}
          description={t('features.agentCompanion.subtitle')}
        >
          <ConfigPageRow label={t('features.agentCompanion.enable')} align="center">
            <div className="bitfun-func-agent-config__row-control">
              <Switch
                checked={settings.enable_agent_companion}
                onChange={(e) => updateSetting('enable_agent_companion', e.target.checked)}
                size="small"
              />
            </div>
          </ConfigPageRow>
          <ConfigPageRow
            label={t('features.agentCompanion.petLabel')}
            description={t('features.agentCompanion.petDescription')}
            align="start"
            multiline
            className="bitfun-func-agent-config__pet-row"
          >
            <div className="bitfun-func-agent-config__pet-picker">
              <div className="bitfun-func-agent-config__pet-picker-preview" aria-hidden>
                <ChatInputPixelPet
                  mood="rest"
                  pet={selectedCompanionPet}
                  className="bitfun-func-agent-config__pet-picker-pet"
                />
              </div>
              <Select
                className="bitfun-func-agent-config__pet-select"
                size="small"
                searchable
                options={companionPetOptions}
                value={settings.agent_companion_pet?.packagePath ?? DEFAULT_COMPANION_PET_VALUE}
                onChange={(value) => { void handleCompanionPetChange(value); }}
                placeholder={t('features.agentCompanion.petPlaceholder')}
                renderOption={(option) => {
                  const pet = companionPets.find(item => item.packagePath === option.value);
                  const isUserPet = pet?.source === 'user';
                  const isDeleting = !!pet && companionPetDeletingPath === pet.packagePath;
                  return (
                    <div className="bitfun-func-agent-config__pet-select-option">
                      <div className="bitfun-func-agent-config__pet-select-option-main">
                        <span className="bitfun-func-agent-config__pet-select-thumb" aria-hidden>
                          {pet ? (
                            <span
                              className="bitfun-func-agent-config__pet-preview-sprite"
                              style={{ '--bitfun-pet-preview-src': `url("${pet.previewSrc}")` } as React.CSSProperties}
                            />
                          ) : (
                            <ChatInputPixelPet mood="rest" className="bitfun-func-agent-config__pet-select-panda" />
                          )}
                        </span>
                        <span className="bitfun-func-agent-config__pet-select-text">
                          <span className="bitfun-func-agent-config__pet-select-label">{option.label}</span>
                          {option.description && (
                            <span className="bitfun-func-agent-config__pet-select-description">{option.description}</span>
                          )}
                        </span>
                      </div>
                      {isUserPet && IS_TAURI_DESKTOP && pet && (
                        <IconButton
                          type="button"
                          size="small"
                          variant="ghost"
                          className="bitfun-func-agent-config__pet-select-delete"
                          disabled={isDeleting}
                          aria-label={t('features.agentCompanion.delete')}
                          tooltip={t('features.agentCompanion.delete')}
                          onClick={(e) => void handleDeleteCompanionPet(e, pet)}
                        >
                          <Trash2 size={14} />
                        </IconButton>
                      )}
                    </div>
                  );
                }}
                renderValue={(option) => {
                  const selectedOption = Array.isArray(option) ? option[0] : option;
                  return selectedOption ? (
                    <span className="bitfun-func-agent-config__pet-select-value">{selectedOption.label}</span>
                  ) : undefined;
                }}
              />
              <div className="bitfun-func-agent-config__pet-actions">
                <IconButton
                  type="button"
                  size="small"
                  variant="ghost"
                  onClick={() => void handleRefreshCompanionPets()}
                  disabled={companionPetsLoading}
                  aria-label={t('features.agentCompanion.refresh')}
                  tooltip={t('features.agentCompanion.refresh')}
                >
                  <RefreshCw size={14} />
                </IconButton>
                <Button
                  size="small"
                  variant="secondary"
                  onClick={() => void handleImportCompanionPet()}
                  disabled={!IS_TAURI_DESKTOP || companionPetImporting}
                  title={t('features.agentCompanion.importHint')}
                >
                  <Plus size={14} />
                  {companionPetImporting ? t('features.agentCompanion.importing') : t('features.agentCompanion.import')}
                </Button>
              </div>
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
          <ConfigPageRow label={tTools('config.confirmTimeout')} description={tTools('config.confirmTimeoutDesc')} align="center">
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
          <ConfigPageRow label={tTools('config.executionTimeout')} description={tTools('config.executionTimeoutDesc')} align="center">
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
          <ConfigPageRow label={t('maxRounds.label')} description={t('maxRounds.description')} align="center">
            <div className="bitfun-func-agent-config__row-control">
              <NumberInput
                value={maxRounds}
                onChange={(val) => handleMaxRoundsChange(String(val))}
                min={1}
                max={10000}
                step={10}
                size="small"
                variant="compact"
              />
            </div>
          </ConfigPageRow>
        </ConfigPageSection>

        {/* ── Computer use (desktop) ─────────────────────────────── */}
        <ConfigPageSection
          title={t('computerUse.sectionTitle')}
          description={
            IS_TAURI_DESKTOP ? t('computerUse.sectionDescription') : t('computerUse.desktopOnly')
          }
        >
          {IS_TAURI_DESKTOP ? (
            <>
              <ConfigPageRow label={t('computerUse.enable')} description={t('computerUse.enableDesc')} align="center">
                <div className="bitfun-func-agent-config__row-control">
                  <Switch
                    checked={computerUseEnabled}
                    onChange={(e) => handleComputerUseEnabledChange(e.target.checked)}
                    disabled={computerUseBusy || computerUseStatusLoading}
                    size="small"
                  />
                </div>
              </ConfigPageRow>
              <ConfigPageRow
                label={t('computerUse.accessibility')}
                description={t('computerUse.accessibilityDesc')}
                align="center"
                balanced
              >
                <div
                  className="bitfun-func-agent-config__row-control"
                  style={{
                    display: 'flex',
                    flexDirection: 'row',
                    flexWrap: 'nowrap',
                    alignItems: 'center',
                    justifyContent: 'flex-end',
                    gap: 8,
                  }}
                >
                  <span style={{ display: 'inline-flex', alignItems: 'center', gap: 6, flexShrink: 0 }}>
                    <span className={!computerUseStatusLoading && computerUseAccess ? 'bitfun-func-agent-config__perm-status--granted' : undefined}>
                      {computerUseAccessLabel}
                    </span>
                    <IconButton
                      type="button"
                      size="small"
                      variant="ghost"
                      aria-label={t('computerUse.refreshStatus')}
                      tooltip={t('computerUse.refreshStatus')}
                      disabled={computerUseBusy || computerUseStatusLoading}
                      onClick={() => void refreshComputerUseStatus()}
                    >
                      <RefreshCw size={14} />
                    </IconButton>
                  </span>
                  <Button
                    className="bitfun-func-agent-config__row-action-btn"
                    size="small"
                    variant="secondary"
                    disabled={computerUseBusy || computerUseStatusLoading}
                    onClick={() => void handleComputerUseOpenSettings('accessibility')}
                  >
                    {t('computerUse.openSettings')}
                  </Button>
                </div>
              </ConfigPageRow>
              <ConfigPageRow
                label={t('computerUse.screenCapture')}
                description={t('computerUse.screenCaptureDesc')}
                align="center"
                balanced
              >
                <div
                  className="bitfun-func-agent-config__row-control"
                  style={{
                    display: 'flex',
                    flexDirection: 'row',
                    flexWrap: 'nowrap',
                    alignItems: 'center',
                    justifyContent: 'flex-end',
                    gap: 8,
                  }}
                >
                  <span style={{ display: 'inline-flex', alignItems: 'center', gap: 6, flexShrink: 0 }}>
                    <span className={!computerUseStatusLoading && computerUseScreen ? 'bitfun-func-agent-config__perm-status--granted' : undefined}>
                      {computerUseScreenLabel}
                    </span>
                    <IconButton
                      type="button"
                      size="small"
                      variant="ghost"
                      aria-label={t('computerUse.refreshStatus')}
                      tooltip={t('computerUse.refreshStatus')}
                      disabled={computerUseBusy || computerUseStatusLoading}
                      onClick={() => void refreshComputerUseStatus()}
                    >
                      <RefreshCw size={14} />
                    </IconButton>
                  </span>
                  <Button
                    className="bitfun-func-agent-config__row-action-btn"
                    size="small"
                    variant="secondary"
                    disabled={computerUseBusy || computerUseStatusLoading}
                    onClick={() => void handleComputerUseOpenSettings('screen_capture')}
                  >
                    {t('computerUse.openSettings')}
                  </Button>
                </div>
              </ConfigPageRow>
            </>
          ) : null}
        </ConfigPageSection>

        {/* ── Browser control (CDP) ──────────────────────────────── */}
        <ConfigPageSection
          title={t('browserControl.sectionTitle')}
          description={
            IS_TAURI_DESKTOP ? t('browserControl.sectionDescription') : t('browserControl.desktopOnly')
          }
        >
          {IS_TAURI_DESKTOP ? (
            <>
              <ConfigPageRow
                label={t('browserControl.status')}
                description={t('browserControl.statusDesc') || undefined}
                align="center"
                balanced
              >
                <div
                  className="bitfun-func-agent-config__row-control"
                  style={{
                    display: 'flex',
                    flexDirection: 'row',
                    flexWrap: 'wrap',
                    alignItems: 'center',
                    justifyContent: 'flex-end',
                    gap: 8,
                    minWidth: 0,
                  }}
                >
                  <span
                    style={{
                      display: 'inline-flex',
                      alignItems: 'center',
                      gap: 6,
                      minWidth: 0,
                      maxWidth: '100%',
                    }}
                    title={browserCdpAvailable && browserVersion ? `${browserKind} ${browserVersion}` : undefined}
                  >
                    <span
                      className={!browserStatusLoading && browserCdpAvailable ? 'bitfun-func-agent-config__perm-status--granted' : undefined}
                      style={{ overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap', minWidth: 0 }}
                    >
                      {browserStatusLabel}
                    </span>
                    <IconButton
                      type="button"
                      size="small"
                      variant="ghost"
                      aria-label={t('browserControl.refreshStatus')}
                      tooltip={t('browserControl.refreshStatus')}
                      disabled={browserControlBusy || browserStatusLoading}
                      onClick={() => void refreshBrowserControlStatus()}
                    >
                      <RefreshCw size={14} />
                    </IconButton>
                  </span>
                  {!browserCdpAvailable && (
                    <Button
                      className="bitfun-func-agent-config__row-action-btn"
                      size="small"
                      variant="secondary"
                      disabled={browserControlBusy || browserStatusLoading}
                      onClick={() => void handleBrowserControlLaunch()}
                    >
                      {t('browserControl.connect')}
                    </Button>
                  )}
                </div>
              </ConfigPageRow>
              {platform === 'macos' && (
                <ConfigPageRow
                  label={t('browserControl.createLauncher')}
                  description={t('browserControl.createLauncherDesc')}
                  align="center"
                >
                  <div className="bitfun-func-agent-config__row-control">
                    <Button
                      className="bitfun-func-agent-config__row-action-btn"
                      size="small"
                      variant="secondary"
                      disabled={browserControlBusy}
                      onClick={() => void handleBrowserControlCreateLauncher()}
                    >
                      {t('browserControl.createLauncher')}
                    </Button>
                  </div>
                </ConfigPageRow>
              )}
            </>
          ) : null}
        </ConfigPageSection>

        {/* ── Debug mode settings ───────────────────────────────── */}
        <ConfigPageSection
          title={tDebug('sections.combined')}
          description={tDebug('sections.combinedDescription')}
        >
          {/* Basic settings: log path + ingest port */}
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

          {/* Save / cancel for basic settings changes (not shown while modal is open) */}
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

          {/* Language templates entry row */}
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
        </ConfigPageSection>

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

        <Modal
          isOpen={browserRestartPrompt !== null}
          onClose={() => {
            if (!browserControlBusy) setBrowserRestartPrompt(null);
          }}
          title={t('browserControl.restartModal.title')}
          size="small"
          closeOnOverlayClick={!browserControlBusy}
        >
          <div className="bitfun-debug-config__modal-body">
            <p>{t('browserControl.restartModal.description', { browser: browserRestartPrompt?.browserKind || browserKind })}</p>
            <p>{t('browserControl.restartModal.warning')}</p>
            {browserRestartPrompt?.message ? (
              <p className="bitfun-func-agent-config__hint">{browserRestartPrompt.message}</p>
            ) : null}
          </div>
          <div className="bitfun-debug-config__modal-footer">
            <Button
              variant="secondary"
              size="small"
              onClick={() => setBrowserRestartPrompt(null)}
              disabled={browserControlBusy}
            >
              {t('browserControl.restartModal.cancel')}
            </Button>
            <Button
              variant="primary"
              size="small"
              onClick={() => void handleBrowserControlRestart()}
              disabled={browserControlBusy}
            >
              {browserControlBusy
                ? t('browserControl.restartModal.restarting')
                : t('browserControl.restartModal.confirm')}
            </Button>
          </div>
        </Modal>

      </ConfigPageContent>
    </ConfigPageLayout>
  );
};

export default SessionConfig;
