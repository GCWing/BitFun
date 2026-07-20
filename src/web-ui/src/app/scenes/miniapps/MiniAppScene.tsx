/**
 * MiniAppScene — standalone scene tab for a single MiniApp.
 * Mounts MiniAppRunner; close via SceneBar × (does not stop worker).
 */
import React, { useCallback, useEffect, useState } from 'react';
import { RefreshCw, Loader2, AlertTriangle, CheckCircle2, X } from 'lucide-react';
import { miniAppAPI } from '@/infrastructure/api/service-api/MiniAppAPI';
import { api } from '@/infrastructure/api/service-api/ApiClient';
import type { MiniApp, MiniAppDraft } from '@/infrastructure/api/service-api/MiniAppAPI';
import { useTheme } from '@/infrastructure/theme/hooks/useTheme';
import { useCurrentWorkspace } from '@/infrastructure/contexts/WorkspaceContext';
import { createLogger } from '@/shared/utils/logger';
import { IconButton, Button } from '@/component-library';
import { useSceneManager } from '@/app/hooks/useSceneManager';
import type { SceneTabId } from '@/app/components/SceneBar/types';
import { useMiniAppStore } from './miniAppStore';
import { useI18n } from '@/infrastructure/i18n';
import { pickLocalizedString } from './utils/pickLocalizedString';
import MiniAppCustomizeEntry from './customization/MiniAppCustomizeEntry';
import MiniAppCustomizePanel from './customization/MiniAppCustomizePanel';
import MiniAppDraftPreview from './customization/MiniAppDraftPreview';
import { useMiniAppCustomizeHotspot } from './customization/useMiniAppCustomizeHotspot';
import MiniAppRunner from './components/MiniAppRunner';
import './MiniAppScene.scss';

const log = createLogger('MiniAppScene');
const MINIAPP_REFRESH_EVENTS = [
  'miniapp-updated',
  'miniapp-recompiled',
  'miniapp-rolled-back',
  'miniapp-worker-restarted',
] as const;

interface MiniAppSceneProps {
  appId: string;
}

const MiniAppScene: React.FC<MiniAppSceneProps> = ({ appId }) => {
  const openApp = useMiniAppStore((state) => state.openApp);
  const closeApp = useMiniAppStore((state) => state.closeApp);
  const markCustomizationActive = useMiniAppStore((state) => state.markCustomizationActive);
  const markCustomizationIdle = useMiniAppStore((state) => state.markCustomizationIdle);
  const { themeType } = useTheme();
  const { workspace, workspacePath } = useCurrentWorkspace();
  const { closeScene } = useSceneManager();
  const { t, currentLanguage } = useI18n('scenes/miniapp');

  const [app, setApp] = useState<MiniApp | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [key, setKey] = useState(0);
  const [customizeOpen, setCustomizeOpen] = useState(false);
  const [customizeNotice, setCustomizeNotice] = useState<string | null>(null);
  const [customizePreview, setCustomizePreview] = useState<{
    draft: MiniAppDraft;
    previewKey: number;
  } | null>(null);

  useEffect(() => {
    openApp(appId);
    return () => {
      closeApp(appId);
      markCustomizationIdle(appId);
    };
  }, [appId, openApp, closeApp, markCustomizationIdle]);

  useEffect(() => {
    setCustomizePreview(null);
  }, [appId]);

  useEffect(() => {
    if (customizeOpen) {
      markCustomizationActive(appId);
      return;
    }

    markCustomizationIdle(appId);
  }, [appId, customizeOpen, markCustomizationActive, markCustomizationIdle]);

  const load = useCallback(async (id: string) => {
    setLoading(true);
    setError(null);
    try {
      const theme = themeType ?? 'dark';
      const loaded = await miniAppAPI.getMiniApp(id, theme, workspacePath || undefined);
      if (!loaded.compiled_html?.trim()) {
        log.error('MiniApp loaded without compiled_html', { appId: id });
        setError('MiniApp compiled_html is empty');
        setApp(null);
        return;
      }
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
    const tabId = `miniapp:${appId}` as SceneTabId;
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
        closeScene(tabId);
      }
    });

    return () => {
      refreshUnlisteners.forEach((unlisten) => unlisten());
      unlistenDeleted();
    };
  }, [appId, closeScene, load]);

  const handleReload = () => {
    if (appId) {
      setKey((value) => value + 1);
      void load(appId);
    }
  };

  const handleOpenCustomize = useCallback(() => {
    setCustomizeNotice(null);
    setCustomizeOpen(true);
  }, []);

  useMiniAppCustomizeHotspot({
    enabled: Boolean(app) && !customizeOpen,
    onOpen: handleOpenCustomize,
  });

  const appName = app ? pickLocalizedString(app, currentLanguage, 'name') : 'Mini App';

  return (
    <div className="miniapp-scene">
      <div className="miniapp-scene__header">
        <div className="miniapp-scene__header-center">
          {app ? (
            <span className="miniapp-scene__title">{appName}</span>
          ) : (
            <span className="miniapp-scene__title miniapp-scene__title--loading">Mini App</span>
          )}
        </div>
        <div className="miniapp-scene__header-actions">
          <MiniAppCustomizeEntry
            disabled={!app || loading}
            onOpen={handleOpenCustomize}
          />
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
      <div className={[
        'miniapp-scene__content',
        customizeOpen && app && 'miniapp-scene__content--customizing',
      ].filter(Boolean).join(' ')}>
        {loading && !app && (
          <div className="miniapp-scene__loading">
            <Loader2 size={28} className="miniapp-scene__spinning" strokeWidth={1.5} />
            <span>{t('scene.loading')}</span>
          </div>
        )}
        {error && !app && (
          <div className="miniapp-scene__error">
            <AlertTriangle size={32} strokeWidth={1.5} />
            <p>{t('scene.loadFailed', { error })}</p>
            <Button variant="secondary" size="small" onClick={() => void load(appId)}>
              {t('scene.retry')}
            </Button>
          </div>
        )}
        {app && (
          <div className="miniapp-scene__runner-shell">
            {loading && (
              <div className="miniapp-scene__refresh-overlay" role="status" aria-live="polite">
                <Loader2 size={20} className="miniapp-scene__spinning" strokeWidth={1.5} />
              </div>
            )}
            <MiniAppRunner key={`${app.id}-${key}`} app={app} />
            {customizePreview && (
              <div className="miniapp-scene__preview-stage" role="region" aria-label={t('customize.previewTitle')}>
                <div className="miniapp-scene__preview-stage-header">
                  <div>
                    <span>{t('customize.previewTitle')}</span>
                    <small>{t('customize.previewHint')}</small>
                  </div>
                  <IconButton
                    variant="ghost"
                    size="small"
                    onClick={() => setCustomizePreview(null)}
                    tooltip={t('customize.hidePreview')}
                    aria-label={t('customize.hidePreview')}
                  >
                    <X size={14} />
                  </IconButton>
                </div>
                <div className="miniapp-scene__preview-stage-body">
                  <MiniAppDraftPreview
                    draft={customizePreview.draft}
                    previewKey={customizePreview.previewKey}
                  />
                </div>
              </div>
            )}
          </div>
        )}
        {customizeNotice && (
          <div className="miniapp-scene__customize-notice" role="status">
            <CheckCircle2 size={16} />
            <span>{customizeNotice}</span>
          </div>
        )}
        {app && (
          <MiniAppCustomizePanel
            open={customizeOpen}
            app={app}
            appName={appName}
            themeType={themeType ?? 'dark'}
            workspacePath={workspacePath || undefined}
            remoteConnectionId={workspace?.connectionId}
            remoteSshHost={workspace?.sshHost}
            previewOpen={Boolean(customizePreview)}
            onPreviewChange={setCustomizePreview}
            onClose={() => setCustomizeOpen(false)}
            onApplied={(updatedApp) => {
              setApp(updatedApp);
              setKey((value) => value + 1);
              setCustomizeNotice(t('customize.applySaved'));
            }}
          />
        )}
      </div>
    </div>
  );
};

export default MiniAppScene;
