import { useCallback, useEffect, useRef, useState, type ReactElement } from 'react';
import { systemAPI } from '@/infrastructure/api';
import { configManager } from '@/infrastructure/config';
import { createLogger } from '@/shared/utils/logger';
import { scheduleAfterStartupSignal } from '@/shared/utils/startupTaskScheduling';
import type { CheckForUpdatesResponse } from '@/infrastructure/api/service-api/SystemAPI';
import { isTauriRuntime } from './tauriEnv';
import {
  recordDailyPromptDismissed,
  recordSkipThisVersion,
  shouldShowDailyUpdatePrompt
} from './appUpdateStorage';
import { UpdateAvailableDialog } from './UpdateAvailableDialog';
import { UpdateInstallProgressModal } from './UpdateInstallProgressModal';
import { useUpdateInstallStore } from './updateInstallStore';

const log = createLogger('DailyAppUpdate');

/**
 * On first launch after a short delay, checks for updates and may show the daily prompt.
 * Renders update dialogs; mount once near the app root (e.g. inside AppLayout).
 */
export function DailyAppUpdateGate(): ReactElement | null {
  const [dailyOpen, setDailyOpen] = useState(false);
  const [dailyData, setDailyData] = useState<CheckForUpdatesResponse | null>(null);
  const dailyCheckTimerRef = useRef<number | null>(null);
  const updateStatus = useUpdateInstallStore(state => state.status);
  const updateProgress = useUpdateInstallStore(state => state.progress);
  const updateError = useUpdateInstallStore(state => state.error);
  const startUpdateInstall = useUpdateInstallStore(state => state.startInstall);
  const clearUpdateError = useUpdateInstallStore(state => state.clearError);
  const clearUpdateInstalled = useUpdateInstallStore(state => state.clearInstalled);

  useEffect(() => {
    if (!isTauriRuntime()) {
      return;
    }
    let cancelled = false;
    const runDailyCheck = async () => {
      let autoUpdate = true;
      try {
        const v = await configManager.getConfig<boolean>('app.auto_update');
        if (v === false) {
          autoUpdate = false;
        }
      } catch (e) {
        log.warn('Failed to read app.auto_update; defaulting to enabled', e);
      }
      if (cancelled || !autoUpdate) {
        return;
      }
      dailyCheckTimerRef.current = window.setTimeout(() => {
        void (async () => {
          try {
            const autoAtCheck = await configManager.getConfig<boolean>('app.auto_update');
            if (cancelled || autoAtCheck === false) {
              return;
            }
            const res = await systemAPI.checkForUpdates();
            if (cancelled) {
              return;
            }
            if (!res.updateAvailable || !res.latestVersion) {
              return;
            }
            if (!shouldShowDailyUpdatePrompt(res.latestVersion)) {
              return;
            }
            setDailyData(res);
            setDailyOpen(true);
          } catch (e) {
            log.warn('Daily update check failed', e);
          }
        })();
      }, 900);
    };
    const cancelStartupSchedule = scheduleAfterStartupSignal(() => {
      void runDailyCheck();
    }, {
      signalName: 'bitfun:interactive-shell-ready',
      fallbackTimeoutMs: 10000,
      frameCount: 1,
      onError: error => {
        log.warn('Failed to schedule daily update check after startup', error);
      },
    });
    return () => {
      cancelled = true;
      cancelStartupSchedule();
      if (dailyCheckTimerRef.current != null) {
        window.clearTimeout(dailyCheckTimerRef.current);
        dailyCheckTimerRef.current = null;
      }
    };
  }, []);

  const closeDaily = useCallback(() => {
    setDailyOpen(false);
    setDailyData(null);
  }, []);

  const onLater = useCallback(() => {
    const v = dailyData?.latestVersion;
    if (v) {
      recordDailyPromptDismissed(v);
    }
    closeDaily();
  }, [closeDaily, dailyData]);

  const onSkip = useCallback(() => {
    const v = dailyData?.latestVersion;
    if (v) {
      recordSkipThisVersion(v);
    }
    closeDaily();
  }, [closeDaily, dailyData]);

  const onInstall = useCallback(() => {
    const v = dailyData?.latestVersion;
    if (v) {
      recordDailyPromptDismissed(v);
    }
    setDailyOpen(false);
    setDailyData(null);
    void startUpdateInstall();
  }, [dailyData, startUpdateInstall]);

  const onCloseProgressError = useCallback(() => {
    clearUpdateError();
  }, [clearUpdateError]);

  const onCloseInstalled = useCallback(() => {
    clearUpdateInstalled();
  }, [clearUpdateInstalled]);

  const onRestart = useCallback(async () => {
    try {
      await systemAPI.restartApp();
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      useUpdateInstallStore.setState({ status: 'error', error: msg });
    }
  }, []);

  if (!isTauriRuntime()) {
    return null;
  }

  return (
    <>
      <UpdateAvailableDialog
        isOpen={dailyOpen}
        variant="daily"
        data={dailyData}
        onLater={onLater}
        onSkip={onSkip}
        onInstall={onInstall}
      />
      <UpdateInstallProgressModal
        isOpen={updateStatus === 'error' || updateStatus === 'installed'}
        error={updateError}
        installed={updateStatus === 'installed'}
        progress={updateProgress}
        onCloseError={onCloseProgressError}
        onCloseInstalled={onCloseInstalled}
        onRestart={onRestart}
      />
    </>
  );
}
