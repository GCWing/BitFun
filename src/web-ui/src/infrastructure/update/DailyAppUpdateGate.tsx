import { useCallback, useEffect, useRef, useState, type ReactElement } from 'react';
import { systemAPI } from '@/infrastructure/api';
import { configManager } from '@/infrastructure/config';
import { createLogger } from '@/shared/utils/logger';
import type { CheckForUpdatesResponse } from '@/infrastructure/api/service-api/SystemAPI';
import { isTauriRuntime } from './tauriEnv';
import {
  recordDailyPromptDismissed,
  recordSkipThisVersion,
  shouldShowDailyUpdatePrompt
} from './appUpdateStorage';
import { installUpdateWithProgress } from './installUpdateWithProgress';
import { UpdateAvailableDialog } from './UpdateAvailableDialog';
import { UpdateInstallProgressModal } from './UpdateInstallProgressModal';

const log = createLogger('DailyAppUpdate');

/**
 * On first launch after a short delay, checks for updates and may show the daily prompt.
 * Renders update dialogs; mount once near the app root (e.g. inside AppLayout).
 */
export function DailyAppUpdateGate(): ReactElement | null {
  const [dailyOpen, setDailyOpen] = useState(false);
  const [dailyData, setDailyData] = useState<CheckForUpdatesResponse | null>(null);
  const [progressOpen, setProgressOpen] = useState(false);
  const [progress, setProgress] = useState<{ downloaded: number; total: number | null }>({
    downloaded: 0,
    total: null
  });
  const [installError, setInstallError] = useState<string | null>(null);
  const [updateInstalled, setUpdateInstalled] = useState(false);
  const dailyCheckTimerRef = useRef<number | null>(null);

  useEffect(() => {
    if (!isTauriRuntime()) {
      return;
    }
    let cancelled = false;
    void (async () => {
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
    })();
    return () => {
      cancelled = true;
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

  const onInstall = useCallback(async () => {
    const v = dailyData?.latestVersion;
    if (v) {
      recordDailyPromptDismissed(v);
    }
    setDailyOpen(false);
    setDailyData(null);
    setInstallError(null);
    setUpdateInstalled(false);
    setProgress({ downloaded: 0, total: null });
    setProgressOpen(true);
    try {
      await installUpdateWithProgress(next => {
        setProgress(next);
      });
      setUpdateInstalled(true);
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      setInstallError(msg);
    }
  }, [dailyData]);

  const onCloseProgressError = useCallback(() => {
    setProgressOpen(false);
    setInstallError(null);
    setUpdateInstalled(false);
  }, []);

  const onCloseInstalled = useCallback(() => {
    setProgressOpen(false);
    setUpdateInstalled(false);
  }, []);

  const onRestart = useCallback(async () => {
    try {
      await systemAPI.restartApp();
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      setInstallError(msg);
      setUpdateInstalled(false);
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
        isOpen={progressOpen}
        error={installError}
        installed={updateInstalled}
        progress={progress}
        onCloseError={onCloseProgressError}
        onCloseInstalled={onCloseInstalled}
        onRestart={onRestart}
      />
    </>
  );
}
