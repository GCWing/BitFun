import {
  Button,
  Icon,
  IconButton,
  NumberInput,
  Select,
  Switch,
  Tooltip,
  ScrollArea,
  type ComboboxOption,
  type SelectOption,
  Dialog,
  DialogBody,
  DialogClose,
  DialogHeader,
  DialogHeading,
  DialogTitle,
} from '@bitfun/ui';
import React, { useState, useEffect, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { ConfigLoadingState } from '@/infrastructure/config/components/common';
import { confirmDanger } from '@/infrastructure/confirm-dialog';
import { ConfigPageHeader, ConfigPageLayout, ConfigPageContent, ConfigPageSection, ConfigPageRow } from './common';
import { aiExperienceConfigService, type AIExperienceSettings } from '../services/AIExperienceConfigService';
import {
  DEFAULT_AGENT_COMPANION_PET,
  deleteAgentCompanionPetPackage,
  importAgentCompanionPetPackage,
  listAgentCompanionPets,
  releaseAgentCompanionPetPreviewBlobs,
  type AgentCompanionPetPackage,
} from '../services/AgentCompanionPetService';
import { configManager } from '../services/ConfigManager';
import { useComputerUseEnabled } from '../hooks/useComputerUseEnabled';
import {
  DEFAULT_TOOL_PERMISSION_CONFIG,
  normalizeToolPermissionConfig,
  permissionConfigService,
} from '../services/PermissionConfigService';
import { systemAPI } from '@/infrastructure/api/service-api/SystemAPI';
import { api } from '@/infrastructure/api/service-api/ApiClient';
import { useNotification, notificationService } from '@/shared/notification-system';
import type {
  PermissionRule,
  ToolPermissionConfig,
} from '../types';
import { GlobalPermissionRulesDialog } from './GlobalPermissionRulesDialog';
import { ChatInputPixelPet } from '@/flow_chat/components/ChatInputPixelPet';
import SessionTitleConfig from './SessionTitleConfig';
import ReviewCapacitySection from './ReviewCapacitySection';
import ToolJsonRepairSection from './ToolJsonRepairSection';
import { ask, open } from '@tauri-apps/plugin-dialog';
import { createLogger } from '@/shared/utils/logger';
import { usePeerDeviceModeOptional } from '@/infrastructure/peer-device/peerDeviceContextState';
import './RuntimeSettingsPages.scss';

type DescribedSelectOption = SelectOption & { description?: string };

const log = createLogger('RuntimeSettings');

const IS_TAURI_DESKTOP = typeof window !== 'undefined' && '__TAURI__' in window;

/**
 * A peer host that refuses Browser Control / Computer Use (CLI Peer returns
 * "local-only and cannot run on peer"; Desktop Peer would surface a different
 * error). We detect that string so the settings section can show an explicit
 * "unsupported on this peer" notice instead of firing invokes that silently
 * fail on every refresh. See PR #2428 review #4 issue #1.
 */
function isPeerUnsupportedBrowserControlError(error: unknown): boolean {
  const message = error instanceof Error ? error.message : String(error ?? '');
  return /is local-only and cannot run on peer|is not supported on CLI peer host/i.test(message);
}

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
  setupUrl?: string;
};

type BrowserControlDisconnectResponse = {
  success: boolean;
  status: string;
  browserKind: string;
};

type BrowserControlBrowserOption = {
  value: string;
  label: string;
  installed: boolean;
};

type SubagentBatchExecutionPolicy = 'safe_only' | 'force_parallel' | 'serial';
type ToolPermissionMode = 'ask' | 'auto' | 'full_access';

const DEFAULT_SUBAGENT_BATCH_EXECUTION_POLICY: SubagentBatchExecutionPolicy = 'force_parallel';
const DEFAULT_SUBAGENT_MAX_CONCURRENCY = 5;
const DEFAULT_SWARM_MAX_CONCURRENCY = 16;
const SHOW_PERMISSION_MODE_CONTROL_CONFIG_PATH = 'app.flow_chat.show_permission_mode_control';

function normalizeSubagentBatchExecutionPolicy(value: unknown): SubagentBatchExecutionPolicy {
  return value === 'force_parallel' || value === 'serial' || value === 'safe_only'
    ? value
    : DEFAULT_SUBAGENT_BATCH_EXECUTION_POLICY;
}

function resolveToolPermissionMode(config: ToolPermissionConfig): ToolPermissionMode {
  if (config.policy.preset === 'full_access') return 'full_access';
  return config.interaction.auto_approve_ask ? 'auto' : 'ask';
}

function browserSetupUrlFallback(browser: string): string {
  const normalized = browser.toLowerCase();
  if (normalized.includes('edge')) return 'edge://inspect/#remote-debugging';
  if (normalized.includes('chrome')) return 'chrome://inspect/#remote-debugging';
  return '';
}

const DEFAULT_BROWSER_CONTROL_BROWSER = 'default';

export type RuntimeSettingsPageKind =
  | 'pet'
  | 'session-workspace'
  | 'execution'
  | 'execution-control'
  | 'device-control';

interface RuntimeSettingsPageProps {
  page: RuntimeSettingsPageKind;
}

const RuntimeSettingsPage: React.FC<RuntimeSettingsPageProps> = ({ page }) => {
  const { t } = useTranslation('settings/runtime');
  const { t: tNavigation } = useTranslation('settings');
  const { t: tTools } = useTranslation('settings/agentic-tools');
  const notification = useNotification();

  // ── Session config state ─────────────────────────────────────────────────
  const [isLoading, setIsLoading] = useState(true);
  const [settings, setSettings] = useState<AIExperienceSettings | null>(null);
  const [companionPets, setCompanionPets] = useState<AgentCompanionPetPackage[]>([]);
  const [companionPetsLoading, setCompanionPetsLoading] = useState(false);
  const [companionPetImporting, setCompanionPetImporting] = useState(false);
  const [companionPetDeletingPath, setCompanionPetDeletingPath] = useState<string | null>(null);
  const [companionPetListExpanded, setCompanionPetListExpanded] = useState(false);
  const [enableDeferredToolLoading, setEnableDeferredToolLoading] = useState(true);
  const [subagentMaxConcurrency, setSubagentMaxConcurrency] = useState(DEFAULT_SUBAGENT_MAX_CONCURRENCY);
  const [swarmMaxConcurrency, setSwarmMaxConcurrency] = useState(DEFAULT_SWARM_MAX_CONCURRENCY);
  const [executionTimeout, setExecutionTimeout] = useState('');
  const [subagentBatchExecutionPolicy, setSubagentBatchExecutionPolicy] =
    useState<SubagentBatchExecutionPolicy>(DEFAULT_SUBAGENT_BATCH_EXECUTION_POLICY);
  const [toolExecConfigLoading, setToolExecConfigLoading] = useState(false);
  const [deferredToolLoadingConfigSaving, setDeferredToolLoadingConfigSaving] = useState(false);
  const [toolPermissionConfig, setToolPermissionConfig] = useState<ToolPermissionConfig>(DEFAULT_TOOL_PERMISSION_CONFIG);
  const [permissionConfigSaving, setPermissionConfigSaving] = useState(false);
  const [showPermissionModeControl, setShowPermissionModeControl] = useState(true);
  const [permissionModeControlVisibilitySaving, setPermissionModeControlVisibilitySaving] = useState(false);
  const [isGlobalPermissionRulesDialogOpen, setIsGlobalPermissionRulesDialogOpen] = useState(false);

  const { computerUseEnabled, setComputerUseEnabled } = useComputerUseEnabled();
  // Browser Control / Computer Use run on the host that runs the Tool. Desktop
  // Peer B executes them on B; CLI Peer refuses them (deny.rs). When the
  // rendered peer is a CLI Peer the refresh invokes return "local-only and
  // cannot run on peer" — we surface that as an explicit unsupported notice
  // instead of silently degrading. Null = controller local (no peer) or peer
  // capabilities not yet probed (optimistically supported). See PR #2428 #1.
  const peerDevice = usePeerDeviceModeOptional();
  const peerModeActive = peerDevice?.peerMode.active === true;
  const [peerBrowserControlUnsupported, setPeerBrowserControlUnsupported] = useState(false);
  const [computerUseAccess, setComputerUseAccess] = useState(false);
  const [computerUseScreen, setComputerUseScreen] = useState(false);
  const [computerUseBusy, setComputerUseBusy] = useState(false);
  const [computerUseStatusLoading, setComputerUseStatusLoading] = useState(false);
  const [computerUsePlatformNote, setComputerUsePlatformNote] = useState<string | null>(null);

  // ── Browser control state ───────────────────────────────────────────────
  const [browserCdpAvailable, setBrowserCdpAvailable] = useState(false);
  const [browserReady, setBrowserReady] = useState(false);
  const [browserAutoConnectOnStartup, setBrowserAutoConnectOnStartup] = useState(false);
  const [browserDefaultCdpSupported, setBrowserDefaultCdpSupported] = useState(false);
  const [browserDefaultCdpEnabled, setBrowserDefaultCdpEnabled] = useState(false);
  const [browserSetupUrl, setBrowserSetupUrl] = useState('');
  const [browserKind, setBrowserKind] = useState('');
  const [browserVersion, setBrowserVersion] = useState<string | null>(null);
  const [browserPageCount, setBrowserPageCount] = useState(0);
  const [browserOptions, setBrowserOptions] = useState<BrowserControlBrowserOption[]>([]);
  const [preferredBrowser, setPreferredBrowser] = useState(DEFAULT_BROWSER_CONTROL_BROWSER);
  const [browserControlBusy, setBrowserControlBusy] = useState(false);
  const [browserStatusLoading, setBrowserStatusLoading] = useState(false);
  const [platform, setPlatform] = useState<string>('');
  const [browserRestartPrompt, setBrowserRestartPrompt] = useState<BrowserControlLaunchResponse | null>(null);

  const refreshComputerUseStatus = useCallback(async (): Promise<boolean> => {
    if (!IS_TAURI_DESKTOP) return false;
    setComputerUseStatusLoading(true);
    try {
      const s = await api.invoke<ComputerUseStatusPayload>('computer_use_get_status');
      setPeerBrowserControlUnsupported(false);
      setComputerUseEnabled(s.computerUseEnabled);
      setComputerUseAccess(s.accessibilityGranted);
      setComputerUseScreen(s.screenCaptureGranted);
      setComputerUsePlatformNote(s.platformNote);
      return true;
    } catch (error) {
      if (isPeerUnsupportedBrowserControlError(error)) {
        setPeerBrowserControlUnsupported(true);
        return false;
      }
      log.error('computer_use_get_status failed', error);
      return false;
    } finally {
      setComputerUseStatusLoading(false);
    }
  }, [setComputerUseEnabled]);

  const refreshBrowserControlStatus = useCallback(async () => {
    if (!IS_TAURI_DESKTOP) return;
    setBrowserStatusLoading(true);
    try {
      const [s, browsers] = await Promise.all([
        api.invoke<{
          cdpAvailable: boolean;
          defaultCdpSupported: boolean;
          defaultCdpEnabled: boolean;
          setupUrl?: string;
          browserReady: boolean;
          browserKind: string;
          browserVersion: string | null;
          port: number;
          pageCount: number;
        }>('browser_control_get_status', { request: { port: 9222 } }),
        api.invoke<{ options: BrowserControlBrowserOption[] }>('browser_control_list_browsers'),
      ]);
      setPeerBrowserControlUnsupported(false);
      setBrowserCdpAvailable(s.cdpAvailable);
      setBrowserDefaultCdpSupported(s.defaultCdpSupported);
      setBrowserDefaultCdpEnabled(s.defaultCdpEnabled);
      setBrowserSetupUrl(s.setupUrl ?? browserSetupUrlFallback(s.browserKind));
      setBrowserReady(s.browserReady);
      setBrowserKind(s.browserKind);
      setBrowserVersion(s.browserVersion);
      setBrowserPageCount(s.pageCount);
      setBrowserOptions(browsers.options);
    } catch (error) {
      if (isPeerUnsupportedBrowserControlError(error)) {
        setPeerBrowserControlUnsupported(true);
      } else {
        log.error('browser_control_get_status failed', error);
      }
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
  }, [refreshComputerUseStatus, refreshBrowserControlStatus, setComputerUseEnabled]);

  // Browser Control / Computer Use route to the rendered host. Re-probe on every
  // surface switch (local ↔ peer A ↔ peer B): a CLI Peer returns unsupported,
  // a Desktop Peer / local host returns status. Resets the unsupported flag so
  // a switch away from a CLI Peer re-shows controls instead of the notice.
  // The current peer's deviceId is part of the dep so A→B (both peers) fires.
  const renderedPeerDeviceId = peerDevice?.peerMode.active
    ? peerDevice.peerMode.deviceId
    : null;
  useEffect(() => {
    if (!IS_TAURI_DESKTOP) return;
    setPeerBrowserControlUnsupported(false);
    void refreshComputerUseStatus();
    void refreshBrowserControlStatus();
  }, [peerModeActive, renderedPeerDeviceId, refreshComputerUseStatus, refreshBrowserControlStatus]);

  const loadPageData = useCallback(async () => {
    setIsLoading(true);
    try {
      if (page === 'pet') {
        const [loadedSettings, loadedCompanionPets] = await Promise.all([
          aiExperienceConfigService.getSettingsAsync(),
          listAgentCompanionPets(),
        ]);
        setSettings(loadedSettings);
        setCompanionPets(loadedCompanionPets);
        return;
      }

      if (page === 'session-workspace') {
        setSettings(await aiExperienceConfigService.getSettingsAsync());
        return;
      }

      if (page === 'execution' || page === 'execution-control') {
        const [
          deferredToolLoadingEnabled,
          loadedSubagentMaxConcurrency,
          loadedSwarmMaxConcurrency,
          execTimeout,
          loadedSubagentBatchExecutionPolicy,
          loadedToolPermissionConfig,
          loadedPermissionModeControlVisibility,
        ] = await Promise.all([
          configManager.getConfig<boolean>('ai.enable_deferred_tool_loading'),
          configManager.getConfig<number | null>('ai.subagent_max_concurrency'),
          configManager.getConfig<number | null>('ai.swarm_max_concurrency'),
          configManager.getConfig<number | null>('ai.tool_execution_timeout_secs'),
          configManager.getConfig<SubagentBatchExecutionPolicy>('ai.subagent_batch_execution_policy'),
          permissionConfigService.getConfig(),
          configManager.getOptionalConfig<boolean>(SHOW_PERMISSION_MODE_CONTROL_CONFIG_PATH),
        ]);
        setEnableDeferredToolLoading(deferredToolLoadingEnabled ?? true);
        setSubagentMaxConcurrency(loadedSubagentMaxConcurrency != null
          ? loadedSubagentMaxConcurrency
          : DEFAULT_SUBAGENT_MAX_CONCURRENCY);
        setSwarmMaxConcurrency(loadedSwarmMaxConcurrency != null
          ? loadedSwarmMaxConcurrency
          : DEFAULT_SWARM_MAX_CONCURRENCY);
        setExecutionTimeout(execTimeout != null ? String(execTimeout) : '');
        setSubagentBatchExecutionPolicy(normalizeSubagentBatchExecutionPolicy(loadedSubagentBatchExecutionPolicy));
        setToolPermissionConfig(normalizeToolPermissionConfig(loadedToolPermissionConfig));
        setShowPermissionModeControl(loadedPermissionModeControlVisibility !== false);
        if (page === 'execution') return;
      }

      const [computerUseCfg, browserControlPreferredBrowser, browserControlAutoConnect] = await Promise.all([
        configManager.getConfig<boolean>('ai.computer_use_enabled'),
        configManager.getConfig<string>('ai.browser_control_preferred_browser'),
        configManager.getConfig<boolean>('ai.browser_control_auto_connect_on_startup'),
      ]);
      setPreferredBrowser(browserControlPreferredBrowser || DEFAULT_BROWSER_CONTROL_BROWSER);
      setBrowserAutoConnectOnStartup(browserControlAutoConnect === true);
      refreshDesktopStatus(computerUseCfg);
    } catch (error) {
      log.error('Failed to load settings page data', { page, error });
      if (page === 'pet' || page === 'session-workspace') {
        setSettings(await aiExperienceConfigService.getSettingsAsync());
      }
    } finally {
      setIsLoading(false);
    }
  }, [page, refreshDesktopStatus]);

  const saveToolPermissionConfig = async (
    nextConfig: ToolPermissionConfig,
    previousConfig: ToolPermissionConfig,
  ): Promise<boolean> => {
    setToolPermissionConfig(nextConfig);
    setPermissionConfigSaving(true);
    try {
      await permissionConfigService.saveConfig(nextConfig);
      notificationService.success(t('messages.saveSuccess'), { duration: 2000 });
      return true;
    } catch (error) {
      log.error('Failed to save tool permission config', error);
      setToolPermissionConfig(previousConfig);
      notificationService.error(t('messages.saveFailed'));
      return false;
    } finally {
      setPermissionConfigSaving(false);
    }
  };

  const handlePermissionModeChange = async (value: string | number | (string | number)[]) => {
    const nextModeValue = String(Array.isArray(value) ? value[0] : value);
    const nextMode: ToolPermissionMode = nextModeValue === 'full_access'
      ? 'full_access'
      : nextModeValue === 'auto'
        ? 'auto'
        : 'ask';
    const previousConfig = toolPermissionConfig;
    const currentMode = resolveToolPermissionMode(previousConfig);
    if (nextMode === currentMode) return;

    if (nextMode === 'full_access') {
      const confirmed = await confirmDanger(
        t('permissionPolicy.fullAccessWarningTitle'),
        t('permissionPolicy.fullAccessWarningMessage'),
        {
          confirmText: t('permissionPolicy.fullAccessConfirm'),
          cancelText: t('permissionPolicy.cancel'),
        },
      );
      if (!confirmed) return;
    }

    await saveToolPermissionConfig(
      {
        policy: {
          ...previousConfig.policy,
          preset: nextMode === 'full_access' ? 'full_access' : 'ask',
        },
        interaction: {
          ...previousConfig.interaction,
          auto_approve_ask: nextMode === 'auto',
        },
      },
      previousConfig,
    );
  };

  const handleSaveGlobalPermissionRules = async (rules: PermissionRule[]): Promise<boolean> => {
    const previousConfig = toolPermissionConfig;
    return saveToolPermissionConfig(
      { ...previousConfig, policy: { ...previousConfig.policy, rules } },
      previousConfig,
    );
  };

  const handlePermissionModeControlVisibilityChange = async (visible: boolean) => {
    const previousVisibility = showPermissionModeControl;
    setShowPermissionModeControl(visible);
    setPermissionModeControlVisibilitySaving(true);
    try {
      await configManager.setConfig(SHOW_PERMISSION_MODE_CONTROL_CONFIG_PATH, visible);
      notificationService.success(t('messages.saveSuccess'), { duration: 2000 });
    } catch (error) {
      log.error('Failed to save permission mode control visibility', error);
      setShowPermissionModeControl(previousVisibility);
      notificationService.error(t('messages.saveFailed'));
    } finally {
      setPermissionModeControlVisibilitySaving(false);
    }
  };

  useEffect(() => {
    void loadPageData();
  }, [loadPageData]);

  // ── Session config handlers ──────────────────────────────────────────────

  const updateSetting = async <K extends keyof AIExperienceSettings>(
    key: K,
    value: AIExperienceSettings[K]
  ) => {
    if (!settings) return;
    const newSettings = { ...settings, [key]: value };
    setSettings(newSettings);
    try {
      await aiExperienceConfigService.saveSettings({ [key]: value });
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
        title: t('features.pet.importDialogTitle'),
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
      notification.error(t('features.pet.importFailed'));
    } finally {
      setCompanionPetImporting(false);
    }
  };

  const handleDeleteCompanionPet = async (event: React.MouseEvent, pet: AgentCompanionPetPackage) => {
    event.preventDefault();
    event.stopPropagation();
    if (!IS_TAURI_DESKTOP || pet.source !== 'user' || !settings) return;
    const confirmed = await ask(t('features.pet.deleteConfirmBody'), {
      title: t('features.pet.deleteConfirmTitle'),
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
        const next = { ...settings, agent_companion_pet: DEFAULT_AGENT_COMPANION_PET };
        setSettings(next);
        await aiExperienceConfigService.saveSettings({ agent_companion_pet: DEFAULT_AGENT_COMPANION_PET });
      }
      notification.success(t('features.pet.deleteSuccess'));
    } catch (error) {
      log.error('Failed to delete Agent companion pet', error);
      notification.error(t('features.pet.deleteFailed'));
    } finally {
      setCompanionPetDeletingPath(null);
    }
  };

  const companionPetOptions: ComboboxOption[] = companionPets.map(pet => {
    const displayName = pet.source === 'preset' && pet.id === 'blue-golden'
      ? t('features.pet.presets.blueGolden.name')
      : pet.displayName;
    return {
      value: pet.packagePath,
      label: displayName,
      group: pet.source === 'preset'
        ? t('features.pet.groupPreset')
        : t('features.pet.groupImported'),
    };
  });

  const companionDisplayModeOptions: DescribedSelectOption[] = [
    {
      value: 'desktop',
      label: t('features.pet.displayDesktop'),
      description: t('features.pet.displayDesktopDesc'),
    },
    {
      value: 'input',
      label: t('features.pet.displayInput'),
      description: t('features.pet.displayInputDesc'),
    },
  ];

  const subagentBatchExecutionPolicyOptions: DescribedSelectOption[] = [
    {
      value: 'safe_only',
      label: tTools('config.subagentBatchPolicy.safeOnly'),
      description: tTools('config.subagentBatchPolicy.safeOnlyDesc'),
    },
    {
      value: 'force_parallel',
      label: tTools('config.subagentBatchPolicy.forceParallel'),
      description: tTools('config.subagentBatchPolicy.forceParallelDesc'),
    },
  ];

  const selectedCompanionPetPackage = settings?.agent_companion_pet
    ? companionPets.find(pet => pet.packagePath === settings.agent_companion_pet?.packagePath) ?? null
    : null;
  const selectedCompanionPet = selectedCompanionPetPackage ?? settings?.agent_companion_pet ?? DEFAULT_AGENT_COMPANION_PET;
  const selectedCompanionPetValue = selectedCompanionPet.packagePath;
  const selectedCompanionPetOption = companionPetOptions.find(option => option.value === selectedCompanionPetValue)
    ?? companionPetOptions[0];

  const handleCompanionPetChange = async (value: string | number | (string | number)[]) => {
    const selectedValue = String(Array.isArray(value) ? value[0] : value);
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
    setCompanionPetListExpanded(false);
  };

  const handleDeferredToolLoadingChange = async (checked: boolean) => {
    const previous = enableDeferredToolLoading;
    setEnableDeferredToolLoading(checked);
    setDeferredToolLoadingConfigSaving(true);
    try {
      await configManager.setConfig('ai.enable_deferred_tool_loading', checked);
      notificationService.success(t('messages.saveSuccess'), { duration: 2000 });
    } catch (error) {
      log.error('Failed to save enable_deferred_tool_loading', error);
      notificationService.error(
        `${t('messages.saveFailed')}: ` + (error instanceof Error ? error.message : String(error))
      );
      setEnableDeferredToolLoading(previous);
    } finally {
      setDeferredToolLoadingConfigSaving(false);
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

  const handleSwarmMaxConcurrencyChange = async (value: number) => {
    if (Number.isNaN(value) || value < 1) return;
    setSwarmMaxConcurrency(value);
    try {
      await configManager.setConfig('ai.swarm_max_concurrency', value);
    } catch (error) {
      log.error('Failed to save swarm_max_concurrency', error);
      notificationService.error(tTools('messages.saveFailed'));
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
      if (checked) {
        // Proactively surface the OS permission prompt (macOS Accessibility /
        // Screen Recording) the moment the user opts in, instead of waiting
        // for the first agent tool call to fail with a permission error.
        try {
          await api.invoke('computer_use_request_permissions');
        } catch (permError) {
          log.warn('computer_use_request_permissions failed', permError);
        }
      }
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
      await api.invoke('computer_use_open_system_settings', { request: { pane } });
    } catch (error) {
      log.error('computer_use_open_system_settings failed', error);
      notificationService.error(t('messages.saveFailed'));
    }
  };

  const handleBrowserControlBrowserChange = async (value: string | number) => {
    const nextValue = String(value || DEFAULT_BROWSER_CONTROL_BROWSER);
    const previousValue = preferredBrowser;
    setPreferredBrowser(nextValue);
    setBrowserControlBusy(true);
    try {
      await configManager.setConfig(
        'ai.browser_control_preferred_browser',
        nextValue === DEFAULT_BROWSER_CONTROL_BROWSER ? '' : nextValue,
      );
      await refreshBrowserControlStatus();
    } catch (error) {
      log.error('Failed to save browser_control_preferred_browser', error);
      setPreferredBrowser(previousValue);
      notificationService.error(
        `${tTools('messages.saveFailed')}: ` + (error instanceof Error ? error.message : String(error))
      );
    } finally {
      setBrowserControlBusy(false);
    }
  };

  const handleBrowserAutoConnectChange = async (checked: boolean) => {
    const previousValue = browserAutoConnectOnStartup;
    setBrowserAutoConnectOnStartup(checked);
    try {
      await configManager.setConfig('ai.browser_control_auto_connect_on_startup', checked);
    } catch (error) {
      log.error('Failed to save browser_control_auto_connect_on_startup', error);
      setBrowserAutoConnectOnStartup(previousValue);
      notificationService.error(
        `${tTools('messages.saveFailed')}: ` + (error instanceof Error ? error.message : String(error))
      );
    }
  };

  const presentBrowserControlLaunchResult = (result: BrowserControlLaunchResponse) => {
    const setupUrl = result.setupUrl
      || browserSetupUrl
      || browserSetupUrlFallback(result.browserKind);
    if (result.success) {
      notificationService.success(
        t('browserControl.connectSuccess', { browser: result.browserKind }),
        { duration: 3000 }
      );
    } else if (result.status === 'requires_user_profile_setup') {
      notificationService.info(
        t('browserControl.userProfileSetupRequired', {
          browser: result.browserKind,
          url: setupUrl,
        }),
        { duration: 12000 }
      );
    } else if (result.status === 'requires_manual_user_profile_setup') {
      // The platform could not open the settings page, so the URL itself is
      // the actionable part of the message.
      notificationService.info(
        t('browserControl.userProfileSetupManual', {
          browser: result.browserKind,
          url: setupUrl,
        }),
        { duration: 20000 }
      );
    } else if (result.status === 'user_profile_connection_failed') {
      notificationService.info(
        t('browserControl.userProfileConnectionFailed', { browser: result.browserKind }),
        { duration: 12000 }
      );
    } else if (result.status === 'needs_restart') {
      setBrowserRestartPrompt(result);
    } else if (result.message) {
      notificationService.info(result.message, { duration: 8000 });
    }
  };

  const handleBrowserControlLaunch = async () => {
    setBrowserControlBusy(true);
    try {
      const result = await api.invoke<BrowserControlLaunchResponse>('browser_control_launch', { request: { port: 9222 } });
      presentBrowserControlLaunchResult(result);
      await refreshBrowserControlStatus();
    } catch (error) {
      log.error('browser_control_launch failed', error);
      notificationService.error(t('browserControl.connectFailed'));
    } finally {
      setBrowserControlBusy(false);
    }
  };

  const handleBrowserControlEnableDefaultCdp = async () => {
    setBrowserControlBusy(true);
    const setupUrl = browserSetupUrl || browserSetupUrlFallback(browserKind);
    const promptNotificationId = notificationService.info(
      t(
        browserDefaultCdpEnabled
          ? 'browserControl.defaultCdpConnectPrompt'
          : 'browserControl.defaultCdpEnablePrompt',
        { browser: browserKind, url: setupUrl },
      ),
      { duration: 0 },
    );
    try {
      const result = await api.invoke<BrowserControlLaunchResponse>(
        'browser_control_enable_default_cdp',
        { request: { port: 9222 } },
      );
      notificationService.dismiss(promptNotificationId);
      presentBrowserControlLaunchResult(result);
      await refreshBrowserControlStatus();
    } catch (error) {
      log.error('browser_control_enable_default_cdp failed', error);
      notificationService.error(t('browserControl.connectFailed'));
    } finally {
      notificationService.dismiss(promptNotificationId);
      setBrowserControlBusy(false);
    }
  };

  const handleBrowserControlDisconnect = async () => {
    setBrowserControlBusy(true);
    try {
      const result = await api.invoke<BrowserControlDisconnectResponse>(
        'browser_control_disconnect',
        { request: { port: 9222 } },
      );
      notificationService.success(
        t('browserControl.disconnectSuccess', { browser: result.browserKind }),
        { duration: 4000 },
      );
      await refreshBrowserControlStatus();
    } catch (error) {
      log.error('browser_control_disconnect failed', error);
      notificationService.error(t('browserControl.disconnectFailed'));
    } finally {
      setBrowserControlBusy(false);
    }
  };

  const handleBrowserControlRestart = async () => {
    if (!browserRestartPrompt) return;
    setBrowserControlBusy(true);
    try {
      const result = await api.invoke<BrowserControlLaunchResponse>('browser_control_restart_with_cdp', {
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

  const handleToolTimeoutChange = async (value: string) => {
    const configKey = 'ai.tool_execution_timeout_secs';
    const trimmedValue = value.trim();
    if (trimmedValue !== '') {
      const numValue = parseInt(trimmedValue, 10);
      if (Number.isNaN(numValue) || numValue < 0) return;
    }
    setExecutionTimeout(trimmedValue);
    const numValue = trimmedValue === '' ? null : parseInt(trimmedValue, 10);
    try {
      await configManager.setConfig(configKey, numValue);
    } catch (error) {
      log.error('Failed to save tool timeout config', { error });
      notificationService.error(tTools('messages.saveFailed'));
    }
  };

  // ── Derived values ───────────────────────────────────────────────────────

  const computerUseAccessLabel = computerUseStatusLoading
    ? t('loading.text')
    : computerUseAccess ? t('computerUse.granted') : t('computerUse.notGranted');
  const computerUseScreenLabel = computerUseStatusLoading
    ? t('loading.text')
    : computerUseScreen ? t('computerUse.granted') : t('computerUse.notGranted');
  const computerUsePlatformMessage = computerUsePlatformNote
    ? platform === 'macos'
      ? t('computerUse.platformNotes.macos')
      : platform === 'windows'
        ? t('computerUse.platformNotes.windows')
        : platform === 'linux'
          ? t('computerUse.platformNotes.linux')
          : t('computerUse.platformNotes.generic')
    : null;
  // A ready browser is not a failure state: BitFun attaches to it the moment
  // something needs it, so say that rather than the bare "not connected".
  const browserStatusLabel = browserCdpAvailable
    ? `${browserKind} · ${browserPageCount} ${t('browserControl.tabs')}`
    : browserStatusLoading
      ? t('loading.text')
      : browserReady
        ? t('browserControl.readyNotConnected')
        : t('browserControl.notConnected');
  const browserSelectOptions: ComboboxOption[] = browserOptions.map((option) => ({
    value: option.value,
    label: option.installed ? option.label : `${option.label} (${t('browserControl.notInstalled')})`,
    disabled: !option.installed,
  }));

  const pageCopyKey = page === 'session-workspace'
    ? 'sessionWorkspace'
    : page === 'execution-control'
      ? 'executionControl'
      : page === 'device-control'
        ? 'deviceControl'
        : page;
  const pageTitle = tNavigation(`navigation.pages.${pageCopyKey}.label`);
  const pageSubtitle = tNavigation(`navigation.pages.${pageCopyKey}.description`);

  const requiresExperienceSettings = page === 'pet' || page === 'session-workspace';
  if (isLoading || (requiresExperienceSettings && !settings)) {
    return (
      <ConfigPageLayout className="bitfun-runtime-settings" data-bf-component="runtime-settings" data-bf-part="root" data-bf-view={page}>
        <ConfigPageHeader title={pageTitle} subtitle={pageSubtitle} />
        <ConfigPageContent className="bitfun-runtime-settings__content" data-bf-component="runtime-settings" data-bf-part="content">
          <ConfigLoadingState label={t('loading.text')} />
        </ConfigPageContent>
      </ConfigPageLayout>
    );
  }

  return (
    <ConfigPageLayout className="bitfun-runtime-settings" data-bf-component="runtime-settings" data-bf-part="root" data-bf-view={page}>
      <ConfigPageHeader title={pageTitle} subtitle={pageSubtitle} />

      <ConfigPageContent className="bitfun-runtime-settings__content" data-bf-component="runtime-settings" data-bf-part="content">

        {page === 'pet' && settings ? (
          <>

        {/* ── Agent companion (collapsed input) ─────────────────── */}
        <ConfigPageSection
          title={t('features.pet.title')}
          description={t('features.pet.subtitle')}
        >
          <ConfigPageRow label={t('features.pet.enable')} align="center">
            <div className="bitfun-runtime-settings__row-control" data-bf-component="runtime-settings" data-bf-part="control">
              <Switch
                checked={settings.enable_agent_companion}
                onChange={(e) => updateSetting('enable_agent_companion', e.target.checked)}
              />
            </div>
          </ConfigPageRow>
          <ConfigPageRow
            label={t('features.pet.displayModeLabel')}
            description={t('features.pet.displayModeDescription')}
            align="center"
          >
            <Select
              className="bitfun-runtime-settings__pet-select"
              size="sm"
              options={companionDisplayModeOptions.map(option => ({
                disabled: option.disabled,
                label: option.description ? `${option.label} — ${option.description}` : option.label,
                value: option.value,
              }))}
              value={settings.agent_companion_display_mode}
              onValueChange={(value) => {
                const selectedValue = String(value);
                void updateSetting(
                  'agent_companion_display_mode',
                  selectedValue === 'desktop' ? 'desktop' : 'input',
                );
              }}
            />
          </ConfigPageRow>
          <ConfigPageRow
            label={(
              <span className="bitfun-runtime-settings__pet-row-heading">
                <span className="bitfun-runtime-settings__pet-row-copy">
                  <span className="bitfun-runtime-settings__pet-row-title">
                    {t('features.pet.petLabel')}
                  </span>
                  <span className="bitfun-runtime-settings__pet-row-description">
                    {t('features.pet.petDescription')}
                  </span>
                </span>
                <span className="bitfun-runtime-settings__pet-actions" data-bf-component="runtime-settings" data-bf-part="petActions">
                  <Tooltip content={t('features.pet.refresh')}>
                    <IconButton
                      type="button"
                      size="sm"
                      onClick={() => void handleRefreshCompanionPets()}
                      disabled={companionPetsLoading}
                      aria-label={t('features.pet.refresh')}
                      icon={<Icon name="refresh" size="sm" />}
                    />
                  </Tooltip>
                  <Button
                    size="sm"
                    variant="outline"
                    onClick={() => void handleImportCompanionPet()}
                    disabled={!IS_TAURI_DESKTOP || companionPetImporting}
                    title={t('features.pet.importHint')}
                    leadingIcon={<Icon name="plus" size="sm" />}
                  >

                    {companionPetImporting ? t('features.pet.importing') : t('features.pet.import')}
                  </Button>
                </span>
              </span>
            )}
            align="start"
            multiline
            className="bitfun-runtime-settings__pet-row"
          >
            <div className="bitfun-runtime-settings__pet-picker" data-bf-component="runtime-settings" data-bf-part="petPicker">
              <div className="bitfun-runtime-settings__pet-chooser" data-bf-component="runtime-settings" data-bf-part="petChooser">
                <button
                  type="button"
                  className="bitfun-runtime-settings__pet-expand-button"
                  data-bf-component="runtime-settings"
                  data-bf-part="petTrigger"
                  data-bf-state={companionPetListExpanded ? 'expanded' : ''}
                  aria-expanded={companionPetListExpanded}
                  aria-controls="bitfun-companion-pet-list"
                  onClick={() => setCompanionPetListExpanded((expanded) => !expanded)}
                >
                  <span className="bitfun-runtime-settings__pet-expand-current">
                    <span className="bitfun-runtime-settings__pet-select-thumb" aria-hidden>
                      {selectedCompanionPetPackage ? (
                        <span
                          className="bitfun-runtime-settings__pet-preview-sprite"
                          style={{ '--bitfun-pet-preview-src': `url("${selectedCompanionPetPackage.previewSrc}")` } as React.CSSProperties}
                        />
                      ) : (
                        <ChatInputPixelPet mood="rest" pet={selectedCompanionPet} className="bitfun-runtime-settings__pet-select-panda" />
                      )}
                    </span>
                    <span className="bitfun-runtime-settings__pet-select-value">
                      {selectedCompanionPetOption?.label ?? t('features.pet.petPlaceholder')}
                    </span>
                  </span>
                  <Icon name="chevron-down" size="sm" className={companionPetListExpanded ? 'bitfun-runtime-settings__pet-expand-chevron--open' : undefined} />
                </button>
                {companionPetListExpanded && (
                  <ScrollArea
                    id="bitfun-companion-pet-list"
                    className="bitfun-runtime-settings__pet-list"
                    data-bf-component="runtime-settings"
                    data-bf-part="petList"
                    role="radiogroup"
                    aria-label={t('features.pet.petLabel')}
                  >
                    {companionPetOptions.map((option, index) => {
                      const pet = companionPets.find(item => item.packagePath === option.value);
                      const isUserPet = pet?.source === 'user';
                      const isDeleting = !!pet && companionPetDeletingPath === pet.packagePath;
                      const isSelected = option.value === selectedCompanionPetValue;
                      const showGroup = option.group && option.group !== companionPetOptions[index - 1]?.group;
                      return (
                        <React.Fragment key={String(option.value)}>
                          {showGroup && (
                            <div className="bitfun-runtime-settings__pet-list-group" data-bf-component="runtime-settings" data-bf-part="petGroup">
                              {option.group}
                            </div>
                          )}
                          <div
                            className={`bitfun-runtime-settings__pet-select-option${isSelected ? ' bitfun-runtime-settings__pet-select-option--selected' : ''}`}
                            data-bf-component="runtime-settings"
                            data-bf-part="petOption"
                            data-bf-state={isSelected ? 'selected' : ''}
                            role="radio"
                            tabIndex={0}
                            aria-checked={isSelected}
                            onClick={() => void handleCompanionPetChange(option.value)}
                            onKeyDown={(event) => {
                              if (event.key === 'Enter' || event.key === ' ') {
                                event.preventDefault();
                                void handleCompanionPetChange(option.value);
                              }
                            }}
                          >
                            <div className="bitfun-runtime-settings__pet-select-option-main" data-bf-component="runtime-settings" data-bf-part="petOptionMain">
                              <span className="bitfun-runtime-settings__pet-select-thumb" aria-hidden>
                                {pet ? (
                                  <span
                                    className="bitfun-runtime-settings__pet-preview-sprite"
                                    style={{ '--bitfun-pet-preview-src': `url("${pet.previewSrc}")` } as React.CSSProperties}
                                  />
                                ) : (
                                  <ChatInputPixelPet
                                    mood="rest"
                                    pet={DEFAULT_AGENT_COMPANION_PET}
                                    className="bitfun-runtime-settings__pet-select-panda"
                                  />
                                )}
                              </span>
                              <span className="bitfun-runtime-settings__pet-select-text">
                                <span className="bitfun-runtime-settings__pet-select-label">{option.label}</span>
                              </span>
                            </div>
                            <div className={`bitfun-runtime-settings__pet-select-actions${isUserPet && IS_TAURI_DESKTOP && pet ? ' bitfun-runtime-settings__pet-select-actions--deletable' : ''}`} data-bf-component="runtime-settings" data-bf-part="petActions">
                              {isSelected && (
                                <Icon name="check-line" size="sm" className="bitfun-runtime-settings__pet-select-check" aria-hidden />
                              )}
                              {isUserPet && IS_TAURI_DESKTOP && pet && (
                                <Tooltip content={t('features.pet.delete')}>
                                  <IconButton
                                    type="button"
                                    size="sm"
                                    tone="danger"
                                    className="bitfun-runtime-settings__pet-select-delete"
                                    disabled={isDeleting}
                                    aria-label={t('features.pet.delete')}
                                    onClick={(e) => void handleDeleteCompanionPet(e, pet)}
                                    icon={<Icon name="delete" size="sm" />}
                                  />
                                </Tooltip>
                              )}
                            </div>
                          </div>
                        </React.Fragment>
                      );
                    })}
                  </ScrollArea>
                )}
              </div>
            </div>
          </ConfigPageRow>
        </ConfigPageSection>

          </>
        ) : null}

        {page === 'session-workspace' && settings ? (
          <>

        {/* ── Accelerated workspace search ───────────────────────── */}
        <ConfigPageSection
          title={t('features.workspaceSearch.title')}
          description={t('features.workspaceSearch.subtitle')}
        >
          <ConfigPageRow label={t('features.workspaceSearch.enable')} align="center">
            <div className="bitfun-runtime-settings__row-control" data-bf-component="runtime-settings" data-bf-part="control">
              <Switch
                checked={settings.enable_workspace_search}
                onChange={(e) => updateSetting('enable_workspace_search', e.target.checked)}
              />
            </div>
          </ConfigPageRow>
        </ConfigPageSection>

        <SessionTitleConfig />

          </>
        ) : null}

        {page === 'execution' || page === 'execution-control' ? (
          <>

        <ConfigPageSection
          title={t('permissionPolicy.sectionTitle')}
          description={t('permissionPolicy.sectionDescription')}
        >
          <ConfigPageRow
            label={t('permissionPolicy.mode')}
            description={`${resolveToolPermissionMode(toolPermissionConfig) === 'full_access'
              ? t('permissionPolicy.fullAccessDescription')
              : resolveToolPermissionMode(toolPermissionConfig) === 'auto'
                ? t('permissionPolicy.autoApproveDescription')
                : t('permissionPolicy.askDescription')} ${t('permissionPolicy.modeDescription')}`}
            align="center"
          >
            <div className="bitfun-runtime-settings__row-control" data-bf-component="runtime-settings" data-bf-part="control">
              <Select
                size="sm"
                value={resolveToolPermissionMode(toolPermissionConfig)}
                options={[
                  { value: 'ask', label: t('permissionPolicy.ask') },
                  { value: 'auto', label: t('permissionPolicy.autoApprove') },
                  { value: 'full_access', label: t('permissionPolicy.fullAccess') },
                ]}
                disabled={permissionConfigSaving}
                onValueChange={handlePermissionModeChange}
              />
            </div>
          </ConfigPageRow>
          <ConfigPageRow
            label={t('permissionPolicy.showInChatInput')}
            description={t('permissionPolicy.showInChatInputDescription')}
            align="center"
          >
            <div className="bitfun-runtime-settings__row-control">
              <Switch
                checked={showPermissionModeControl}
                disabled={permissionModeControlVisibilitySaving}
                onChange={event => void handlePermissionModeControlVisibilityChange(event.target.checked)}
              />
            </div>
          </ConfigPageRow>
          <ConfigPageRow
            label={t('permissionPolicy.globalRules')}
            description={t('permissionPolicy.globalRulesDescription')}
            align="center"
          >
            <div className="bitfun-runtime-settings__row-control" data-bf-component="runtime-settings" data-bf-part="control">
              <Button
                type="button"
                size="sm"
                variant="outline"
                disabled={permissionConfigSaving}
                onClick={() => setIsGlobalPermissionRulesDialogOpen(true)}
              >
                {t('permissionPolicy.manageGlobalRules')}
              </Button>
            </div>
          </ConfigPageRow>
        </ConfigPageSection>

        <GlobalPermissionRulesDialog
          isOpen={isGlobalPermissionRulesDialogOpen}
          rules={toolPermissionConfig.policy.rules}
          isSaving={permissionConfigSaving}
          onSave={handleSaveGlobalPermissionRules}
          onClose={() => setIsGlobalPermissionRulesDialogOpen(false)}
        />

        {/* ── Tool execution behavior ────────────────────────────── */}
        <ConfigPageSection
          title={t('toolExecution.sectionTitle')}
          description={t('toolExecution.sectionDescription')}
        >
          <ConfigPageRow
            label={tTools('config.executionTimeout')}
            description={tTools('config.executionTimeoutDesc')}
            align="center"
          >
            <div className="bitfun-runtime-settings__row-control" data-bf-component="runtime-settings" data-bf-part="control">
              <NumberInput
                value={executionTimeout === '' ? 0 : parseInt(executionTimeout, 10)}
                onValueChange={(val) => handleToolTimeoutChange(val === 0 ? '' : String(val))}
                min={0}
                max={3600}
                step={5}
                unit={tTools('config.seconds')}
                size="sm"
                variant="compact"
              />
            </div>
          </ConfigPageRow>
          <ConfigPageRow
            label={tTools('config.subagentBatchPolicy.label')}
            description={tTools('config.subagentBatchPolicy.desc')}
            align="center"
          >
            <div className="bitfun-runtime-settings__row-control" data-bf-component="runtime-settings" data-bf-part="control">
              <Select
                value={subagentBatchExecutionPolicy}
                options={subagentBatchExecutionPolicyOptions.map(option => ({
                  disabled: option.disabled,
                  label: option.description ? `${option.label} — ${option.description}` : option.label,
                  value: option.value,
                }))}
                size="sm"
                disabled={toolExecConfigLoading}
                onValueChange={handleSubagentBatchExecutionPolicyChange}
              />
            </div>
          </ConfigPageRow>
          <ConfigPageRow
            label={tTools('config.subagentMaxConcurrency')}
            description={tTools('config.subagentMaxConcurrencyDesc')}
            align="center"
          >
            <div className="bitfun-runtime-settings__row-control" data-bf-component="runtime-settings" data-bf-part="control">
              <NumberInput
                value={subagentMaxConcurrency}
                onValueChange={(val) => void handleSubagentMaxConcurrencyChange(val)}
                min={1}
                max={100}
                step={1}
                size="sm"
                variant="compact"
              />
            </div>
          </ConfigPageRow>
          <ConfigPageRow
            label={tTools('config.swarmMaxConcurrency')}
            description={tTools('config.swarmMaxConcurrencyDesc')}
            align="center"
          >
            <div className="bitfun-runtime-settings__row-control" data-bf-component="runtime-settings" data-bf-part="control">
              <NumberInput
                value={swarmMaxConcurrency}
                onValueChange={(val) => void handleSwarmMaxConcurrencyChange(val)}
                min={1}
                max={100}
                step={1}
                size="sm"
                variant="compact"
              />
            </div>
          </ConfigPageRow>
        </ConfigPageSection>

        <ConfigPageSection
          title={t('deferredToolLoading.sectionTitle')}
          description={t('deferredToolLoading.sectionDescription')}
        >
          <ConfigPageRow
            label={t('common.enable')}
            description={!enableDeferredToolLoading ? t('deferredToolLoading.warning') : undefined}
            align="center"
          >
            <div className="bitfun-runtime-settings__row-control" data-bf-component="runtime-settings" data-bf-part="control">
              <Switch
                checked={enableDeferredToolLoading}
                onChange={(event) => handleDeferredToolLoadingChange(event.target.checked)}
                disabled={deferredToolLoadingConfigSaving}
              />
            </div>
          </ConfigPageRow>
        </ConfigPageSection>

        <ToolJsonRepairSection />
        <ReviewCapacitySection />

          </>
        ) : null}

        {page === 'device-control' || page === 'execution-control' ? (
          <>

        {/* ── Computer use (desktop) ─────────────────────────────── */}
        <ConfigPageSection
          title={t('computerUse.sectionTitle')}
          description={
            IS_TAURI_DESKTOP ? t('computerUse.sectionDescription') : t('computerUse.desktopOnly')
          }
        >
          {IS_TAURI_DESKTOP && !peerBrowserControlUnsupported ? (
            <>
              <ConfigPageRow label={t('computerUse.enable')} description={t('computerUse.enableDesc')} align="center">
                <div className="bitfun-runtime-settings__row-control" data-bf-component="runtime-settings" data-bf-part="control">
                  <Switch
                    checked={computerUseEnabled}
                    onChange={(e) => handleComputerUseEnabledChange(e.target.checked)}
                    disabled={computerUseBusy || computerUseStatusLoading}
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
                  className="bitfun-runtime-settings__row-control"
                  data-bf-component="runtime-settings"
                  data-bf-part="control"
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
                    <span className={!computerUseStatusLoading && computerUseAccess ? 'bitfun-runtime-settings__perm-status--granted' : undefined}>
                      {computerUseAccessLabel}
                    </span>
                    <Tooltip content={t('computerUse.refreshStatus')}>
                      <IconButton
                        type="button"
                        size="sm"
                        aria-label={t('computerUse.refreshStatus')}
                        disabled={computerUseBusy || computerUseStatusLoading}
                        onClick={() => void refreshComputerUseStatus()}
                        icon={<Icon name="refresh" size="sm" />}
                      />
                    </Tooltip>
                  </span>
                  {platform === 'macos' && (
                    <Button
                      className="bitfun-runtime-settings__row-action-btn"
                      size="sm"
                      variant="outline"
                      disabled={computerUseBusy || computerUseStatusLoading}
                      onClick={() => void handleComputerUseOpenSettings('accessibility')}
                    >
                      {t('computerUse.openSettings')}
                    </Button>
                  )}
                </div>
              </ConfigPageRow>
              <ConfigPageRow
                label={t('computerUse.screenCapture')}
                description={t('computerUse.screenCaptureDesc')}
                align="center"
                balanced
              >
                <div
                  className="bitfun-runtime-settings__row-control"
                  data-bf-component="runtime-settings"
                  data-bf-part="control"
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
                    <span className={!computerUseStatusLoading && computerUseScreen ? 'bitfun-runtime-settings__perm-status--granted' : undefined}>
                      {computerUseScreenLabel}
                    </span>
                    <Tooltip content={t('computerUse.refreshStatus')}>
                      <IconButton
                        type="button"
                        size="sm"
                        aria-label={t('computerUse.refreshStatus')}
                        disabled={computerUseBusy || computerUseStatusLoading}
                        onClick={() => void refreshComputerUseStatus()}
                        icon={<Icon name="refresh" size="sm" />}
                      />
                    </Tooltip>
                  </span>
                  {platform === 'macos' && (
                    <Button
                      className="bitfun-runtime-settings__row-action-btn"
                      size="sm"
                      variant="outline"
                      disabled={computerUseBusy || computerUseStatusLoading}
                      onClick={() => void handleComputerUseOpenSettings('screen_capture')}
                    >
                      {t('computerUse.openSettings')}
                    </Button>
                  )}
                </div>
              </ConfigPageRow>
              {computerUsePlatformMessage && (
                <div
                  className="bitfun-runtime-settings__platform-note"
                  data-bf-component="runtime-settings"
                  data-bf-part="platformNote"
                  style={{
                    display: 'flex',
                    alignItems: 'flex-start',
                    gap: 6,
                    padding: '8px 0 4px',
                  }}
                >
                  <Icon name="info" size="sm" style={{ flexShrink: 0, marginTop: 2, opacity: 0.7 }} />
                  <p className="bitfun-config-page-row__description" style={{ margin: 0 }}>
                    <strong>{t('computerUse.platformNote')}: </strong>
                    {computerUsePlatformMessage}
                  </p>
                </div>
              )}
            </>
          ) : peerBrowserControlUnsupported ? (
            <ConfigPageRow
              label={t('computerUse.peerUnsupported')}
              description=""
              align="center"
            >
              <span />
            </ConfigPageRow>
          ) : null}
        </ConfigPageSection>

        {/* ── Browser control (CDP) ──────────────────────────────── */}
        <ConfigPageSection
          title={t('browserControl.sectionTitle')}
          description={
            IS_TAURI_DESKTOP ? t('browserControl.sectionDescription') : t('browserControl.desktopOnly')
          }
        >
          {IS_TAURI_DESKTOP && !peerBrowserControlUnsupported ? (
            <>
              {/* Only show browser selector when CDP is not connected */}
              {!browserCdpAvailable && (
              <ConfigPageRow
                label={t('browserControl.preferredBrowser')}
                description={t('browserControl.preferredBrowserDesc')}
                align="center"
                balanced
              >
                <div className="bitfun-runtime-settings__row-control" data-bf-component="runtime-settings" data-bf-part="control">
                  <Select
                    value={preferredBrowser}
                    options={browserSelectOptions}
                    size="sm"
                    disabled={browserControlBusy || browserStatusLoading || browserSelectOptions.length === 0}
                    onValueChange={(value) => void handleBrowserControlBrowserChange(value)}
                  />
                </div>
              </ConfigPageRow>
              )}
              {browserDefaultCdpSupported && (
                <ConfigPageRow
                  label={t('browserControl.defaultCdp')}
                  description={t('browserControl.defaultCdpDesc')}
                  align="center"
                  balanced
                >
                  <div
                    className="bitfun-runtime-settings__row-control"
                    data-bf-component="runtime-settings"
                    data-bf-part="control"
                    style={{
                      display: 'flex',
                      flexDirection: 'row',
                      flexWrap: 'nowrap',
                      alignItems: 'center',
                      justifyContent: 'flex-end',
                      gap: 8,
                    }}
                  >
                    <span className={browserDefaultCdpEnabled ? 'bitfun-runtime-settings__perm-status--granted' : undefined}>
                      {t(browserDefaultCdpEnabled
                        ? 'browserControl.defaultCdpEnabled'
                        : 'browserControl.defaultCdpDisabled')}
                    </span>
                    {!browserCdpAvailable && (
                      <Button
                        className="bitfun-runtime-settings__row-action-btn"
                        size="sm"
                        variant="outline"
                        disabled={browserControlBusy || browserStatusLoading}
                        onClick={() => void handleBrowserControlEnableDefaultCdp()}
                      >
                        {t(browserDefaultCdpEnabled
                          ? 'browserControl.connect'
                          : 'browserControl.enableDefaultCdp')}
                      </Button>
                    )}
                  </div>
                </ConfigPageRow>
              )}
              {browserDefaultCdpSupported && (
                <ConfigPageRow
                  label={t('browserControl.autoConnectOnStartup')}
                  description={t('browserControl.autoConnectOnStartupDesc')}
                  align="center"
                  balanced
                >
                  <div className="bitfun-runtime-settings__row-control" data-bf-component="runtime-settings" data-bf-part="control">
                    <Switch
                      checked={browserAutoConnectOnStartup}
                      onChange={(e) => void handleBrowserAutoConnectChange(e.target.checked)}
                    />
                  </div>
                </ConfigPageRow>
              )}
              <ConfigPageRow
                label={t('browserControl.status')}
                description={t('browserControl.statusDesc') || undefined}
                align="center"
                balanced
              >
                <div
                  className="bitfun-runtime-settings__row-control"
                  data-bf-component="runtime-settings"
                  data-bf-part="control"
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
                      className={!browserStatusLoading && browserCdpAvailable ? 'bitfun-runtime-settings__perm-status--granted' : undefined}
                      style={{ overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap', minWidth: 0 }}
                    >
                      {browserStatusLabel}
                    </span>
                    <Tooltip content={t('browserControl.refreshStatus')}>
                      <IconButton
                        type="button"
                        size="sm"
                        aria-label={t('browserControl.refreshStatus')}
                        disabled={browserControlBusy || browserStatusLoading}
                        onClick={() => void refreshBrowserControlStatus()}
                        icon={<Icon name="refresh" size="sm" />}
                      />
                    </Tooltip>
                  </span>
                  {browserCdpAvailable ? (
                    <Button
                      className="bitfun-runtime-settings__row-action-btn"
                      size="sm"
                      variant="outline"
                      disabled={browserControlBusy || browserStatusLoading}
                      onClick={() => void handleBrowserControlDisconnect()}
                    >
                      {t('browserControl.disconnect')}
                    </Button>
                  ) : !browserDefaultCdpSupported ? (
                    <Button
                      className="bitfun-runtime-settings__row-action-btn"
                      size="sm"
                      variant="outline"
                      disabled={browserControlBusy || browserStatusLoading}
                      onClick={() => void handleBrowserControlLaunch()}
                    >
                      {t('browserControl.connect')}
                    </Button>
                  ) : null}
                </div>
              </ConfigPageRow>
            </>
          ) : peerBrowserControlUnsupported ? (
            <ConfigPageRow
              label={t('browserControl.peerUnsupported')}
              description=""
              align="center"
            >
              <span />
            </ConfigPageRow>
          ) : null}
        </ConfigPageSection>

        <Dialog
          open={browserRestartPrompt !== null}
          onOpenChange={(nextOpen) => {
            if (!nextOpen && !browserControlBusy) setBrowserRestartPrompt(null);
          }}
          size="sm"
          closeOnPointerOutside={!browserControlBusy}
        >
          <DialogHeader>
            <DialogHeading>
              <DialogTitle>{t('browserControl.restartModal.title')}</DialogTitle>
            </DialogHeading>
            <DialogClose />
          </DialogHeader>
          <DialogBody inset="none">
          <div className="bitfun-debug-config__modal-body" data-bf-component="runtime-settings" data-bf-part="restartModal">
            <p>{t('browserControl.restartModal.description', { browser: browserRestartPrompt?.browserKind || browserKind })}</p>
            <p>{t('browserControl.restartModal.warning')}</p>
            {browserRestartPrompt?.message ? (
              <p className="bitfun-runtime-settings__hint">{browserRestartPrompt.message}</p>
            ) : null}
          </div>
          <div className="bitfun-debug-config__modal-footer" data-bf-component="runtime-settings" data-bf-part="modalFooter">
            <Button
              variant="outline"
              size="sm"
              onClick={() => setBrowserRestartPrompt(null)}
              disabled={browserControlBusy}
            >
              {t('browserControl.restartModal.cancel')}
            </Button>
            <Button
              variant="fill"
              size="sm"
              onClick={() => void handleBrowserControlRestart()}
              disabled={browserControlBusy}
            >
              {browserControlBusy
                ? t('browserControl.restartModal.restarting')
                : t('browserControl.restartModal.confirm')}
            </Button>
          </div>
                  </DialogBody>
        </Dialog>

          </>
        ) : null}

      </ConfigPageContent>
    </ConfigPageLayout>
  );
};

export function PetSettingsPage(): React.ReactElement {
  return <RuntimeSettingsPage page="pet" />;
}

export function SessionWorkspaceSettingsPage(): React.ReactElement {
  return <RuntimeSettingsPage page="session-workspace" />;
}

export function ExecutionSettingsPage(): React.ReactElement {
  return <RuntimeSettingsPage page="execution" />;
}

export function ExecutionControlSettingsPage(): React.ReactElement {
  return <RuntimeSettingsPage page="execution-control" />;
}

export function DeviceControlSettingsPage(): React.ReactElement {
  return <RuntimeSettingsPage page="device-control" />;
}
