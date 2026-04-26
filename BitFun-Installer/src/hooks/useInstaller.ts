import { useState, useEffect, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import i18n from '../i18n';
import { detectInstallerUiLanguage, mapUiLanguageToAppLanguage } from '../i18n/languages';
import type {
  InstallStep,
  InstallOptions,
  InstallProgress,
  DiskSpaceInfo,
  ModelConfig,
  ConnectionTestResult,
  LaunchContext,
  InstallPathValidation,
} from '../types/installer';
import { DEFAULT_OPTIONS } from '../types/installer';

export interface UseInstallerReturn {
  step: InstallStep;
  goTo: (step: InstallStep) => void;
  next: () => void;
  back: () => void;
  options: InstallOptions;
  setOptions: React.Dispatch<React.SetStateAction<InstallOptions>>;
  progress: InstallProgress;
  isInstalling: boolean;
  installationCompleted: boolean;
  error: string | null;
  diskSpace: DiskSpaceInfo | null;
  install: () => Promise<void>;
  canConfirmProgress: boolean;
  confirmProgress: () => void;
  retryInstall: () => Promise<void>;
  backToOptions: () => void;
  saveModelConfig: () => Promise<void>;
  testModelConnection: (modelConfig: ModelConfig) => Promise<ConnectionTestResult>;
  launchApp: () => Promise<void>;
  closeInstaller: () => void;
  refreshDiskSpace: (path: string) => Promise<void>;
  clearInstallError: () => void;
  isUninstallMode: boolean;
  isUninstalling: boolean;
  uninstallCompleted: boolean;
  uninstallError: string | null;
  uninstallProgress: number;
  startUninstall: () => Promise<void>;
}

const STEPS: InstallStep[] = ['lang', 'options', 'progress', 'model', 'theme'];
const MOCK_INSTALL_FOR_DEBUG = import.meta.env.DEV && import.meta.env.VITE_MOCK_INSTALL === 'true';

export function useInstaller(): UseInstallerReturn {
  const [step, setStep] = useState<InstallStep>('lang');
  const [options, setOptions] = useState<InstallOptions>(DEFAULT_OPTIONS);
  const [progress, setProgress] = useState<InstallProgress>({
    step: '',
    percent: 0,
    message: '',
  });
  const [isInstalling, setIsInstalling] = useState(false);
  const [installationCompleted, setInstallationCompleted] = useState(false);
  const [canConfirmProgress, setCanConfirmProgress] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [diskSpace, setDiskSpace] = useState<DiskSpaceInfo | null>(null);
  const [isUninstallMode, setIsUninstallMode] = useState(false);
  const [isUninstalling, setIsUninstalling] = useState(false);
  const [uninstallCompleted, setUninstallCompleted] = useState(false);
  const [uninstallError, setUninstallError] = useState<string | null>(null);
  const [uninstallProgress, setUninstallProgress] = useState(0);

  useEffect(() => {
    let mounted = true;
    (async () => {
      try {
        const context = await invoke<LaunchContext>('get_launch_context');
        if (!mounted) return;
        const uiLanguage = detectInstallerUiLanguage(context.appLanguage ?? null);
        await i18n.changeLanguage(uiLanguage);
        if (!mounted) return;
        setOptions((prev) => ({
          ...prev,
          appLanguage: mapUiLanguageToAppLanguage(uiLanguage),
        }));
        if (context.mode === 'uninstall') {
          setIsUninstallMode(true);
          setStep('uninstall');
          const uninstallPath = context.uninstallPath;
          if (uninstallPath) {
            setOptions((prev) => ({ ...prev, installPath: uninstallPath }));
          }
          return;
        }
      } catch (err) {
        console.warn('Failed to detect launch context:', err);
      }

      try {
        const path = await invoke<string>('get_initial_install_path');
        if (mounted) {
          setOptions((prev) => ({ ...prev, installPath: path }));
        }
      } catch (err) {
        console.warn('Failed to get default install path:', err);
      }
    })();
    return () => { mounted = false; };
  }, []);

  useEffect(() => {
    const unlisten = listen<InstallProgress>('install-progress', (event) => {
      setProgress(event.payload);
    });
    return () => { unlisten.then((fn) => fn()); };
  }, []);

  const clearInstallError = useCallback(() => {
    setError(null);
  }, []);

  const goTo = useCallback((s: InstallStep) => setStep(s), []);

  const next = useCallback(() => {
    const idx = STEPS.indexOf(step);
    if (idx < STEPS.length - 1) setStep(STEPS[idx + 1]);
  }, [step]);

  const back = useCallback(() => {
    const idx = STEPS.indexOf(step);
    if (idx > 0) setStep(STEPS[idx - 1]);
  }, [step]);

  const refreshDiskSpace = useCallback(async (path: string) => {
    try {
      const info = await invoke<DiskSpaceInfo>('get_disk_space', { path });
      setDiskSpace(info);
    } catch (err) {
      console.warn('Failed to get disk space:', err);
    }
  }, []);

  const install = useCallback(async () => {
    setError(null);

    if (MOCK_INSTALL_FOR_DEBUG) {
      setIsInstalling(true);
      setInstallationCompleted(false);
      setCanConfirmProgress(false);
      setStep('progress');
      setProgress({ step: 'prepare', percent: 0, message: '' });

      const durationMs = 5000;
      const startedAt = Date.now();

      await new Promise<void>((resolve) => {
        const timer = window.setInterval(() => {
          const elapsed = Date.now() - startedAt;
          const ratio = Math.min(elapsed / durationMs, 1);
          const percent = Math.round(ratio * 100);
          const mockStep =
            percent < 20 ? 'prepare' :
            percent < 50 ? 'extract' :
            percent < 75 ? 'config' :
            percent < 100 ? 'complete' :
            'complete';

          setProgress({ step: mockStep, percent, message: '' });

          if (ratio >= 1) {
            window.clearInterval(timer);
            resolve();
          }
        }, 100);
      });

      setIsInstalling(false);
      setInstallationCompleted(true);
      setCanConfirmProgress(true);
      return;
    }

    setIsInstalling(true);
    setInstallationCompleted(false);
    setCanConfirmProgress(false);
    try {
      const validated = await invoke<InstallPathValidation>('validate_install_path', {
        path: options.installPath,
      });
      const effectiveOptions = {
        ...options,
        installPath: validated.installPath,
      };
      if (validated.installPath !== options.installPath) {
        setOptions((prev) => ({ ...prev, installPath: validated.installPath }));
      }
      setStep('progress');
      setProgress({ step: 'prepare', percent: 0, message: '' });
      await invoke('start_installation', { options: effectiveOptions });
      setInstallationCompleted(true);
      setStep('model');
    } catch (err: any) {
      const raw = typeof err === 'string' ? err : err?.message;
      setError((raw && String(raw).trim()) ? String(raw) : i18n.t('errors.install.failed'));
    } finally {
      setIsInstalling(false);
    }
  }, [options]);

  const confirmProgress = useCallback(() => {
    if (!canConfirmProgress) return;
    setCanConfirmProgress(false);
    setStep('model');
  }, [canConfirmProgress]);

  const retryInstall = useCallback(async () => {
    if (isInstalling) return;
    await install();
  }, [install, isInstalling]);

  const backToOptions = useCallback(() => {
    if (isInstalling) return;
    setError(null);
    setCanConfirmProgress(false);
    setStep('options');
  }, [isInstalling]);

  const saveModelConfig = useCallback(async () => {
    if (!options.modelConfig) return;
    await invoke('set_model_config', { modelConfig: options.modelConfig });
  }, [options.modelConfig]);

  const testModelConnection = useCallback(async (modelConfig: ModelConfig) => {
    return invoke<ConnectionTestResult>('test_model_config_connection', { modelConfig });
  }, []);

  const launchApp = useCallback(async () => {
    await invoke('launch_application', { installPath: options.installPath });
  }, [options.installPath]);

  const closeInstaller = useCallback(() => {
    invoke('close_installer');
  }, []);

  const startUninstall = useCallback(async () => {
    if (isUninstalling) return;
    setUninstallError(null);
    setUninstallCompleted(false);
    setIsUninstalling(true);
    setUninstallProgress(0);
    try {
      await new Promise<void>((resolve) => {
        const durationMs = 1800;
        const startedAt = Date.now();
        const timer = window.setInterval(() => {
          const elapsed = Date.now() - startedAt;
          const ratio = Math.min(elapsed / durationMs, 1);
          const percent = Math.round(ratio * 85);
          setUninstallProgress(percent);
          if (ratio >= 1) {
            window.clearInterval(timer);
            resolve();
          }
        }, 80);
      });

      await invoke('uninstall', { installPath: options.installPath });
      setUninstallProgress(100);
      setUninstallCompleted(true);
      window.setTimeout(() => {
        closeInstaller();
      }, 600);
    } catch (err: any) {
      setUninstallError(typeof err === 'string' ? err : err.message || 'Uninstall failed');
      setUninstallProgress(0);
    } finally {
      setIsUninstalling(false);
    }
  }, [closeInstaller, isUninstalling, options.installPath]);

  return {
    step, goTo, next, back,
    options, setOptions,
    progress, isInstalling, installationCompleted, error, diskSpace,
    install, canConfirmProgress, confirmProgress, retryInstall, backToOptions,
    saveModelConfig, testModelConnection, launchApp, closeInstaller, refreshDiskSpace, clearInstallError,
    isUninstallMode, isUninstalling, uninstallCompleted, uninstallError, uninstallProgress, startUninstall,
  };
}
