import { useState, useEffect, useCallback } from 'react';
import { open } from '@tauri-apps/plugin-dialog';
import { useTranslation } from 'react-i18next';
import { aiExperienceConfigService, type AIExperienceSettings } from '../services/AIExperienceConfigService';
import { configManager } from '../services/ConfigManager';
import { systemAPI } from '@/infrastructure/api/service-api/SystemAPI';
import { notificationService } from '@/shared/notification-system';
import type { AIModelConfig, DebugModeConfig, LanguageDebugTemplate } from '../types';
import {
  DEFAULT_DEBUG_MODE_CONFIG,
  ALL_LANGUAGES,
  DEFAULT_LANGUAGE_TEMPLATES,
} from '../types';
import { createLogger } from '@/shared/utils/logger';

const log = createLogger('SessionSettingsConfig');

export const IS_TAURI_DESKTOP = typeof window !== 'undefined' && '__TAURI__' in window;
export const AGENT_SESSION_TITLE = 'session-title-func-agent';

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

export function useSessionSettingsConfig() {
  const { t } = useTranslation('settings/personalization');
  const { t: tTools } = useTranslation('settings/agentic-tools');
  const { t: tDebug } = useTranslation('settings/debug');
  const { t: tPermissions } = useTranslation('settings/permissions');

  const [isLoading, setIsLoading] = useState(true);
  const [settings, setSettings] = useState<AIExperienceSettings | null>(null);
  const [models, setModels] = useState<AIModelConfig[]>([]);
  const [funcAgentModels, setFuncAgentModels] = useState<Record<string, string>>({});
  const [skipToolConfirmation, setSkipToolConfirmation] = useState(false);
  const [executionTimeout, setExecutionTimeout] = useState('');
  const [confirmationTimeout, setConfirmationTimeout] = useState('');
  const [toolExecConfigLoading, setToolExecConfigLoading] = useState(false);

  const [computerUseEnabled, setComputerUseEnabled] = useState(false);
  const [computerUseAccess, setComputerUseAccess] = useState(false);
  const [computerUseScreen, setComputerUseScreen] = useState(false);
  const [computerUseBusy, setComputerUseBusy] = useState(false);

  const [browserCdpAvailable, setBrowserCdpAvailable] = useState(false);
  const [browserKind, setBrowserKind] = useState('');
  const [browserVersion, setBrowserVersion] = useState<string | null>(null);
  const [browserPageCount, setBrowserPageCount] = useState(0);
  const [browserControlBusy, setBrowserControlBusy] = useState(false);
  const [browserRestartPrompt, setBrowserRestartPrompt] = useState<BrowserControlLaunchResponse | null>(null);
  const [platform, setPlatform] = useState<string>('');

  const [debugConfig, setDebugConfig] = useState<DebugModeConfig>(DEFAULT_DEBUG_MODE_CONFIG);
  const [debugHasChanges, setDebugHasChanges] = useState(false);
  const [debugSaving, setDebugSaving] = useState(false);
  const [expandedTemplates, setExpandedTemplates] = useState<Set<string>>(new Set());
  const [isTemplatesModalOpen, setIsTemplatesModalOpen] = useState(false);

  const refreshComputerUseStatus = useCallback(async (): Promise<boolean> => {
    if (!IS_TAURI_DESKTOP) return false;
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
    }
  }, []);

  const refreshBrowserControlStatus = useCallback(async () => {
    if (!IS_TAURI_DESKTOP) return;
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
    }
  }, []);

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
      ] = await Promise.all([
        aiExperienceConfigService.getSettingsAsync(),
        configManager.getConfig<AIModelConfig[]>('ai.models') || [],
        configManager.getConfig<Record<string, string>>('ai.func_agent_models') || {},
        configManager.getConfig<boolean>('ai.skip_tool_confirmation'),
        configManager.getConfig<number | null>('ai.tool_execution_timeout_secs'),
        configManager.getConfig<number | null>('ai.tool_confirmation_timeout_secs'),
        configManager.getConfig<DebugModeConfig>('ai.debug_mode_config'),
        configManager.getConfig<boolean>('ai.computer_use_enabled'),
      ]);

      setSettings(loadedSettings);
      setModels(allModels as AIModelConfig[]);
      setFuncAgentModels(funcAgentModelsData as Record<string, string>);
      setSkipToolConfirmation(skipConfirm || false);
      setExecutionTimeout(execTimeout != null ? String(execTimeout) : '');
      setConfirmationTimeout(confirmTimeout != null ? String(confirmTimeout) : '');
      if (debugConfigData) setDebugConfig(debugConfigData);

      if (IS_TAURI_DESKTOP) {
        const ok = await refreshComputerUseStatus();
        if (!ok) setComputerUseEnabled(computerUseCfg ?? false);
        await refreshBrowserControlStatus();
        try {
          const info = await systemAPI.getSystemInfo();
          setPlatform(info.platform || '');
        } catch (error) {
          log.warn('getSystemInfo failed', error);
        }
      } else {
        setComputerUseEnabled(computerUseCfg ?? false);
      }
    } catch (error) {
      log.error('Failed to load session settings data', error);
      setSettings(await aiExperienceConfigService.getSettingsAsync());
    } finally {
      setIsLoading(false);
    }
  }, [refreshBrowserControlStatus, refreshComputerUseStatus]);

  useEffect(() => {
    loadAllData();
  }, [loadAllData]);

  const updateSetting = async <K extends keyof AIExperienceSettings>(
    key: K,
    value: AIExperienceSettings[K]
  ) => {
    if (!settings) return;
    const newSettings = { ...settings, [key]: value };
    setSettings(newSettings);
    try {
      await aiExperienceConfigService.saveSettings(newSettings);
      notificationService.success(t('messages.saveSuccess'), { duration: 2000 });
    } catch (error) {
      log.error('Failed to save personalization settings', error);
      notificationService.error(t('messages.saveFailed'));
      setSettings(settings);
    }
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
        `${tTools('messages.saveFailed')}: ${error instanceof Error ? error.message : String(error)}`
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
      notificationService.success(t('messages.saveSuccess'), { duration: 2000 });
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
          tPermissions('browserControl.connectSuccess', { browser: result.browserKind }),
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
      notificationService.error(tPermissions('browserControl.connectFailed'));
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
          tPermissions('browserControl.restartSuccess', { browser: result.browserKind }),
          { duration: 3000 }
        );
        setBrowserRestartPrompt(null);
      } else if (result.message) {
        notificationService.info(result.message, { duration: 8000 });
      }
      await refreshBrowserControlStatus();
    } catch (error) {
      log.error('browser_control_restart_with_cdp failed', error);
      notificationService.error(tPermissions('browserControl.restartFailed'));
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
        tPermissions('browserControl.createLauncherSuccess', { path }),
        { duration: 5000 }
      );
    } catch (error) {
      log.error('browser_control_create_launcher failed', error);
      notificationService.error(tPermissions('browserControl.createLauncherFailed'));
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
      if (next.has(language)) next.delete(language);
      else next.add(language);
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

  return {
    isLoading,
    settings,
    enabledModels: models.filter((m: AIModelConfig) => m.enabled),
    sessionTitleModelId: funcAgentModels[AGENT_SESSION_TITLE] || 'fast',
    skipToolConfirmation,
    executionTimeout,
    confirmationTimeout,
    toolExecConfigLoading,
    computerUseEnabled,
    computerUseAccess,
    computerUseScreen,
    computerUseBusy,
    browserCdpAvailable,
    browserKind,
    browserVersion,
    browserPageCount,
    browserControlBusy,
    browserRestartPrompt,
    platform,
    debugConfig,
    debugHasChanges,
    debugSaving,
    expandedTemplates,
    isTemplatesModalOpen,
    templateEntries: getTemplateEntries(),
    updateSetting,
    handleAgentModelChange,
    handleSkipToolConfirmationChange,
    handleComputerUseEnabledChange,
    handleComputerUseOpenSettings,
    refreshComputerUseStatus,
    refreshBrowserControlStatus,
    handleBrowserControlLaunch,
    handleBrowserControlRestart,
    handleBrowserControlCreateLauncher,
    setBrowserRestartPrompt,
    handleToolTimeoutChange,
    updateDebugConfig,
    saveDebugConfig,
    cancelDebugChanges,
    handleModalSave,
    handleModalCancel,
    resetDebugTemplates,
    updateTemplate,
    toggleTemplateEnabled,
    toggleTemplateExpand,
    handleSelectLogPath,
    setIsTemplatesModalOpen,
    tDebug,
    tTools,
  };
}
