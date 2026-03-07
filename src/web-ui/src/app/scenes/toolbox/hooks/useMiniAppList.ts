/**
 * useMiniAppList — loads miniapps from backend and listens for create/update/delete events.
 */
import { useEffect, useCallback } from 'react';
import { miniAppAPI } from '@/infrastructure/api/service-api/MiniAppAPI';
import { api } from '@/infrastructure/api/service-api/ApiClient';
import { useToolboxStore } from '../toolboxStore';
import { createLogger } from '@/shared/utils/logger';

const log = createLogger('useMiniAppList');

export function useMiniAppList() {
  const setApps = useToolboxStore((s) => s.setApps);
  const setLoading = useToolboxStore((s) => s.setLoading);

  const refresh = useCallback(async () => {
    setLoading(true);
    try {
      const apps = await miniAppAPI.listMiniApps();
      setApps(apps);
    } catch (error) {
      log.error('Failed to load miniapps', error);
    } finally {
      setLoading(false);
    }
  }, [setApps, setLoading]);

  useEffect(() => {
    refresh();

    // Listen for backend events triggered by AI GenerateMiniApp / EditMiniApp
    const unlistenCreated = api.listen('miniapp-created', () => {
      log.info('miniapp-created event received, refreshing');
      refresh();
    });
    const unlistenUpdated = api.listen('miniapp-updated', () => {
      log.info('miniapp-updated event received, refreshing');
      refresh();
    });
    const unlistenDeleted = api.listen('miniapp-deleted', () => {
      log.info('miniapp-deleted event received, refreshing');
      refresh();
    });

    return () => {
      unlistenCreated();
      unlistenUpdated();
      unlistenDeleted();
    };
  }, [refresh]);

  return { refresh };
}
