/**
 * MiniAppScene — standalone scene tab for a single MiniApp.
 * Mounts MiniAppRunner; close via overlay home button (does not stop worker).
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
import { useMiniAppStore } from './miniAppStore';
import { useI18n } from '@/infrastructure/i18n';
import './MiniAppScene.scss';

const log = createLogger('MiniAppScene');

const MiniAppRunner = React.lazy(() => import('./components/MiniAppRunner'));

interface MiniAppSceneProps {
  appId: string;
}

const MiniAppScene: React.FC<MiniAppSceneProps> = ({ appId }) => {
  const openApp = useMiniAppStore((state) => state.openApp);
  const closeApp = useMiniAppStore((state) => state.closeApp);
  const { themeType } = useTheme();
  const { workspacePath } = useCurrentWorkspace();
  const { closeScene } = useSceneManager();
  const { t } = useI18n('scenes/miniapp');

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
      log.error('Failed to load app', err);
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
    const tabId = `miniapp:${appId}` as OverlaySceneId;
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
    <div className="miniapp-scene">
      <div className="miniapp-scene__header">
        <div className="miniapp-scene__header-center">
          {app ? (
            <span className="miniapp-scene__title">{app.name}</span>
          ) : (
            <span className="miniapp-scene__title miniapp-scene__title--loading">Mini App</span>
          )}
        </div>
        <div className="miniapp-scene__header-actions">
          <IconButton
            variant="ghost"
            size="small"
            onClick={handleReload}
            disabled={loading}
            tooltip={t('scene.reload')}
          >
            {loading ? (
              <Loader2 size={14} className="miniapp-scene__spinning" />
            ) : (
              <RefreshCw size={14} />
            )}
          </IconButton>
        </div>
      </div>
      <div className="miniapp-scene__content">
        {loading && !app && (
          <div className="miniapp-scene__loading">
            <Loader2 size={28} className="miniapp-scene__spinning" strokeWidth={1.5} />
            <span>{t('scene.loading')}</span>
          </div>
        )}
        {error && (
          <div className="miniapp-scene__error">
            <AlertTriangle size={32} strokeWidth={1.5} />
            <p>{t('scene.loadFailed', { error })}</p>
            <Button variant="secondary" size="small" onClick={() => void load(appId)}>
              {t('scene.retry')}
            </Button>
          </div>
        )}
        {app && !loading && (
          <React.Suspense fallback={null}>
            <MiniAppRunner key={`${app.id}-${key}`} app={app} />
          </React.Suspense>
        )}
      </div>
    </div>
  );
};

export default MiniAppScene;
