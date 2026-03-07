/**
 * MiniAppScene — standalone scene tab for a single MiniApp.
 * Mounts MiniAppRunner; close via SceneBar × (does not stop worker).
 */
import React, { useEffect, useState } from 'react';
import { RefreshCw, Loader2, AlertTriangle } from 'lucide-react';
import { useToolboxStore } from './toolboxStore';
import { miniAppAPI } from '@/infrastructure/api/service-api/MiniAppAPI';
import { api } from '@/infrastructure/api/service-api/ApiClient';
import type { MiniApp } from '@/infrastructure/api/service-api/MiniAppAPI';
import { useTheme } from '@/infrastructure/theme/hooks/useTheme';
import { createLogger } from '@/shared/utils/logger';
import { IconButton, Button } from '@/component-library';
import { useSceneManager } from '@/app/hooks/useSceneManager';
import './MiniAppScene.scss';

const log = createLogger('MiniAppScene');

const MiniAppRunner = React.lazy(() => import('./components/MiniAppRunner'));

interface MiniAppSceneProps {
  appId: string;
}

const MiniAppScene: React.FC<MiniAppSceneProps> = ({ appId }) => {
  const openApp = useToolboxStore((s) => s.openApp);
  const closeApp = useToolboxStore((s) => s.closeApp);
  const { themeType } = useTheme();
  const { closeScene } = useSceneManager();

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

  const load = async (id: string) => {
    setLoading(true);
    setError(null);
    try {
      const theme = themeType ?? 'dark';
      const loaded = await miniAppAPI.getMiniApp(id, theme);
      setApp(loaded);
    } catch (err) {
      log.error('Failed to load app', err);
      setError(String(err));
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    if (appId) {
      load(appId);
    }
  }, [appId, themeType]);

  useEffect(() => {
    const tabId = `miniapp:${appId}`;
    const shouldHandle = (payload?: { id?: string }) => payload?.id === appId;

    const unlistenUpdated = api.listen<{ id?: string }>('miniapp-updated', (payload) => {
      if (shouldHandle(payload)) {
        setKey((k) => k + 1);
        void load(appId);
      }
    });
    const unlistenRecompiled = api.listen<{ id?: string }>('miniapp-recompiled', (payload) => {
      if (shouldHandle(payload)) {
        setKey((k) => k + 1);
        void load(appId);
      }
    });
    const unlistenRolledBack = api.listen<{ id?: string }>('miniapp-rolled-back', (payload) => {
      if (shouldHandle(payload)) {
        setKey((k) => k + 1);
        void load(appId);
      }
    });
    const unlistenRestarted = api.listen<{ id?: string }>('miniapp-worker-restarted', (payload) => {
      if (shouldHandle(payload)) {
        setKey((k) => k + 1);
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
  }, [appId, closeScene]);

  const handleReload = () => {
    if (appId) {
      setKey((k) => k + 1);
      load(appId);
    }
  };

  return (
    <div className="miniapp-scene">
      <div className="miniapp-scene__header">
        <div className="miniapp-scene__header-center">
          {app ? (
            <span className="miniapp-scene__title">{app.name}</span>
          ) : (
            <span className="miniapp-scene__title miniapp-scene__title--loading">MiniApp</span>
          )}
        </div>
        <div className="miniapp-scene__header-actions">
          <IconButton
            variant="ghost"
            size="small"
            onClick={handleReload}
            disabled={loading}
            tooltip="重新加载"
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
            <span>加载中…</span>
          </div>
        )}
        {error && (
          <div className="miniapp-scene__error">
            <AlertTriangle size={32} strokeWidth={1.5} />
            <p>加载失败：{error}</p>
            <Button variant="secondary" size="small" onClick={() => load(appId)}>
              重试
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
