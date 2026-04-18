/**
 * useLiveAppCatalogSync — keeps Live App catalog and runtime state in sync.
 */
import { useCallback, useEffect } from 'react';
import { api } from '@/infrastructure/api/service-api/ApiClient';
import { liveAppAPI } from '@/infrastructure/api/service-api/LiveAppAPI';
import { createLogger } from '@/shared/utils/logger';
import { useLiveAppStore } from '../liveAppStore';

const log = createLogger('useLiveAppCatalogSync');

export function useLiveAppCatalogSync() {
  const setApps = useLiveAppStore((state) => state.setApps);
  const setLoading = useLiveAppStore((state) => state.setLoading);
  const setRunningWorkerIds = useLiveAppStore((state) => state.setRunningWorkerIds);
  const markWorkerRunning = useLiveAppStore((state) => state.markWorkerRunning);
  const markWorkerStopped = useLiveAppStore((state) => state.markWorkerStopped);

  const refreshApps = useCallback(async () => {
    setLoading(true);
    try {
      const apps = await liveAppAPI.listLiveApps();
      setApps(apps);
    } catch (error) {
      log.error('Failed to load live apps', error);
    } finally {
      setLoading(false);
    }
  }, [setApps, setLoading]);

  const refreshRunningWorkers = useCallback(async () => {
    try {
      const running = await liveAppAPI.workerListRunning();
      setRunningWorkerIds(running);
    } catch (error) {
      log.error('Failed to load running live app workers', error);
    }
  }, [setRunningWorkerIds]);

  useEffect(() => {
    void refreshApps();
    void refreshRunningWorkers();

    const unlistenCreated = api.listen('liveapp-created', () => {
      void refreshApps();
    });
    const unlistenUpdated = api.listen('liveapp-updated', () => {
      void refreshApps();
    });
    const unlistenDeleted = api.listen<{ id?: string }>('liveapp-deleted', (payload) => {
      if (payload?.id) {
        markWorkerStopped(payload.id);
      }
      void refreshApps();
    });
    const unlistenRestarted = api.listen<{ id?: string }>('liveapp-worker-restarted', (payload) => {
      if (payload?.id) {
        markWorkerRunning(payload.id);
      }
    });
    const unlistenStopped = api.listen<{ id?: string }>('liveapp-worker-stopped', (payload) => {
      if (payload?.id) {
        markWorkerStopped(payload.id);
      }
    });

    return () => {
      unlistenCreated();
      unlistenUpdated();
      unlistenDeleted();
      unlistenRestarted();
      unlistenStopped();
    };
  }, [markWorkerRunning, markWorkerStopped, refreshApps, refreshRunningWorkers]);

  return {
    refreshApps,
    refreshRunningWorkers,
  };
}
