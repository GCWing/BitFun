/**
 * LiveAppScene — standalone scene tab for a single Live App.
 * Mounts LiveAppRunner; close via overlay home button (does not stop worker).
 */
import React, { useCallback, useEffect, useState } from 'react';
import { RefreshCw, Loader2, AlertTriangle } from 'lucide-react';
import { miniAppAPI } from '@/infrastructure/api/service-api/MiniAppAPI';
import { api } from '@/infrastructure/api/service-api/ApiClient';
import type { MiniApp } from '@/infrastructure/api/service-api/MiniAppAPI';
import { useTheme } from '@/infrastructure/theme/hooks/useTheme';
import { useCurrentWorkspace } from '@/infrastructure/contexts/WorkspaceContext';
import { createLogger } from '@/shared/utils/logger';
import { IconButton, Button } from '@/component-library';
import { useSceneManager } from '@/app/hooks/useSceneManager';
import type { OverlaySceneId } from '@/app/overlay/types';
import { useLiveAppStore } from './live-app/liveAppStore';
import { useI18n } from '@/infrastructure/i18n';
import './LiveAppScene.scss';

const log = createLogger('LiveAppScene');

const LiveAppRunner = React.lazy(() => import('./live-app/components/LiveAppRunner'));

interface LiveAppSceneProps {
  appId: string;
}

const LiveAppScene: React.FC<LiveAppSceneProps> = ({ appId }) => {
  const openApp = useLiveAppStore((state) => state.openApp);
  const closeApp = useLiveAppStore((state) => state.closeApp);
  const { themeType } = useTheme();
  const { workspacePath } = useCurrentWorkspace();
  const { closeScene } = useSceneManager();
  const { t } = useI18n('scenes/apps');

  const [app, setApp] = useState<MiniApp | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [key, setKey] = useState(0);

  useEffect(() => {
    openApp(appId);
    return () => {
      closeApp(appId);
    };
  }, [appId, openApp, closeApp]);

  const load = useCallback(async (id: string) => {
    setLoading(true);
    setError(null);
    try {
      const theme = themeType ?? 'dark';
      const loaded = await miniAppAPI.getMiniApp(id, theme, workspacePath || undefined);
      setApp(loaded);
    } catch (err) {
      log.error('Failed to load live app', err);
      setError(String(err));
    } finally {
      setLoading(false);
    }
  }, [themeType, workspacePath]);

  useEffect(() => {
    if (appId) {
      void load(appId);
    }
  }, [appId, load]);

  useEffect(() => {
    const tabId = `live-app:${appId}` as OverlaySceneId;
    const shouldHandle = (payload?: { id?: string }) => payload?.id === appId;

    const unlistenUpdated = api.listen<{ id?: string }>('miniapp-updated', (payload) => {
      if (shouldHandle(payload)) {
        setKey((value) => value + 1);
        void load(appId);
      }
    });
    const unlistenRecompiled = api.listen<{ id?: string }>('miniapp-recompiled', (payload) => {
      if (shouldHandle(payload)) {
        setKey((value) => value + 1);
        void load(appId);
      }
    });
    const unlistenRolledBack = api.listen<{ id?: string }>('miniapp-rolled-back', (payload) => {
      if (shouldHandle(payload)) {
        setKey((value) => value + 1);
        void load(appId);
      }
    });
    const unlistenRestarted = api.listen<{ id?: string }>('miniapp-worker-restarted', (payload) => {
      if (shouldHandle(payload)) {
        setKey((value) => value + 1);
        void load(appId);
      }
    });
    const unlistenDeleted = api.listen<{ id?: string }>('miniapp-deleted', (payload) => {
      if (shouldHandle(payload)) {
        closeScene(tabId);
      }
    });

    return () => {
      unlistenUpdated();
      unlistenRecompiled();
      unlistenRolledBack();
      unlistenRestarted();
      unlistenDeleted();
    };
  }, [appId, closeScene, load]);

  const handleReload = () => {
    if (appId) {
      setKey((value) => value + 1);
      void load(appId);
    }
  };

  return (
    <div className="live-app-scene">
      <div className="live-app-scene__header">
        <div className="live-app-scene__header-center">
          {app ? (
            <span className="live-app-scene__title">{app.name}</span>
          ) : (
            <span className="live-app-scene__title live-app-scene__title--loading">Live App</span>
          )}
        </div>
        <div className="live-app-scene__header-actions">
          <IconButton
            variant="ghost"
            size="small"
            onClick={handleReload}
            disabled={loading}
            tooltip={t('liveApp.scene.reload')}
          >
            {loading ? (
              <Loader2 size={14} className="live-app-scene__spinning" />
            ) : (
              <RefreshCw size={14} />
            )}
          </IconButton>
        </div>
      </div>
      <div className="live-app-scene__content">
        {loading && !app && (
          <div className="live-app-scene__loading">
            <Loader2 size={28} className="live-app-scene__spinning" strokeWidth={1.5} />
            <span>{t('liveApp.scene.loading')}</span>
          </div>
        )}
        {error && (
          <div className="live-app-scene__error">
            <AlertTriangle size={32} strokeWidth={1.5} />
            <p>{t('liveApp.scene.loadFailed', { error })}</p>
            <Button variant="secondary" size="small" onClick={() => void load(appId)}>
              {t('liveApp.scene.retry')}
            </Button>
          </div>
        )}
        {app && !loading && (
          <React.Suspense fallback={null}>
            <LiveAppRunner key={`${app.id}-${key}`} app={app} />
          </React.Suspense>
        )}
      </div>
    </div>
  );
};

export default LiveAppScene;
