/**
 * MiniAppPanel — renders a MiniApp inside the auxiliary panel tab.
 * Shares MiniAppRunner, loading/error states, and refresh-event handling
 * with MiniAppScene, but without scene management or customization.
 */
import React, { useCallback, useEffect, useState } from 'react';
import { RefreshCw, Loader2, AlertTriangle } from 'lucide-react';
import { miniAppAPI } from '@/infrastructure/api/service-api/MiniAppAPI';
import { api } from '@/infrastructure/api/service-api/ApiClient';
import type { MiniApp } from '@/infrastructure/api/service-api/MiniAppAPI';
import { useAppearance } from '@/infrastructure/appearance';
import { createLogger } from '@/shared/utils/logger';
import { IconButton, Button } from '@/component-library';
import { useMiniAppStore } from '@/app/scenes/miniapps/miniAppStore';
import { useI18n } from '@/infrastructure/i18n';
import { pickLocalizedString } from '@/app/scenes/miniapps/utils/pickLocalizedString';
import MiniAppRunner from '@/app/scenes/miniapps/components/MiniAppRunner';
import './MiniAppPanel.scss';

const log = createLogger('MiniAppPanel');
const MINIAPP_REFRESH_EVENTS = [
  'miniapp-updated',
  'miniapp-recompiled',
  'miniapp-rolled-back',
  'miniapp-worker-restarted',
] as const;

interface MiniAppPanelProps {
  appId?: string;
  workspacePath?: string;
}

const MiniAppPanel: React.FC<MiniAppPanelProps> = ({ appId, workspacePath }) => {
  const openApp = useMiniAppStore((state) => state.openApp);
  const closeApp = useMiniAppStore((state) => state.closeApp);
  const { current: appearance } = useAppearance();
  const appearanceMode = appearance?.mode ?? 'dark';
  const { t, currentLanguage } = useI18n('scenes/miniapp');

  const [app, setApp] = useState<MiniApp | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [strictRuntime, setStrictRuntime] = useState(false);
  const [key, setKey] = useState(0);

  useEffect(() => {
    if (appId) {
      openApp(appId);
    }
    return () => {
      if (appId) {
        closeApp(appId);
      }
    };
  }, [appId, openApp, closeApp]);

  const load = useCallback(async (id: string) => {
    setLoading(true);
    setError(null);
    try {
      const loaded = await miniAppAPI.getMiniApp(id, appearanceMode, workspacePath || undefined);
      if (!loaded.compiled_html?.trim()) {
        log.error('MiniApp loaded without compiled_html', { appId: id });
        setError('MiniApp compiled_html is empty');
        setApp(null);
        return;
      }
      setStrictRuntime(loaded.runtime_profile === 'market_strict');
      setApp(loaded);
    } catch (err) {
      log.error('Failed to load app', err);
      setError(String(err));
    } finally {
      setLoading(false);
    }
  }, [appearanceMode, workspacePath]);

  useEffect(() => {
    if (appId) {
      void load(appId);
    }
  }, [appId, load]);

  useEffect(() => {
    if (!appId) return;
    const shouldHandle = (payload?: { id?: string }) => payload?.id === appId;
    const refresh = () => {
      setKey((value) => value + 1);
      void load(appId);
    };

    const refreshUnlisteners = MINIAPP_REFRESH_EVENTS.map((eventName) =>
      api.listen<{ id?: string }>(eventName, (payload) => {
        if (shouldHandle(payload)) {
          refresh();
        }
      }),
    );
    const unlistenDeleted = api.listen<{ id?: string }>('miniapp-deleted', (payload) => {
      if (shouldHandle(payload)) {
        setApp(null);
        setError('MiniApp has been deleted');
      }
    });

    return () => {
      refreshUnlisteners.forEach((unlisten) => unlisten());
      unlistenDeleted();
    };
  }, [appId, load]);

  const handleReload = () => {
    if (appId) {
      setKey((value) => value + 1);
      void load(appId);
    }
  };

  if (!appId) {
    return (
      <div className="miniapp-panel miniapp-panel--no-app" data-bf-component="miniapp-panel" data-bf-part="root" data-bf-state="no-app">
        <AlertTriangle size={32} strokeWidth={1.5} />
        <p>No MiniApp specified</p>
      </div>
    );
  }

  const appName = app ? pickLocalizedString(app, currentLanguage, 'name') : 'Mini App';

  return (
    <div className="miniapp-panel" data-bf-component="miniapp-panel" data-bf-part="root">
      <div className="miniapp-panel__toolbar" data-bf-part="toolbar">
        <span className="miniapp-panel__title">{appName}</span>
        <IconButton
          variant="ghost"
          size="small"
          onClick={handleReload}
          disabled={loading}
          tooltip={t('scene.reload')}
        >
          {loading ? (
            <Loader2 size={14} className="miniapp-panel__spinning" />
          ) : (
            <RefreshCw size={14} />
          )}
        </IconButton>
      </div>
      <div className="miniapp-panel__content" data-bf-part="content">
        {loading && !app && (
          <div className="miniapp-panel__loading" data-bf-part="loading">
            <Loader2 size={28} className="miniapp-panel__spinning" strokeWidth={1.5} />
            <span>{t('scene.loading')}</span>
          </div>
        )}
        {error && !app && (
          <div className="miniapp-panel__error" data-bf-part="error">
            <AlertTriangle size={32} strokeWidth={1.5} />
            <p>{t('scene.loadFailed', { error })}</p>
            <Button variant="secondary" size="small" onClick={() => void load(appId)}>
              {t('scene.retry')}
            </Button>
          </div>
        )}
        {app && (
          <div className="miniapp-panel__runner-shell" data-bf-part="runner">
            {loading && (
              <div className="miniapp-panel__refresh-overlay" role="status" aria-live="polite">
                <Loader2 size={20} className="miniapp-panel__spinning" strokeWidth={1.5} />
              </div>
            )}
            <MiniAppRunner
              key={`${app.id}-${key}`}
              app={app}
              strictRuntime={strictRuntime}
            />
          </div>
        )}
      </div>
    </div>
  );
};

export default MiniAppPanel;
