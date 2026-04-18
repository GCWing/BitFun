import React, { useMemo } from 'react';
import { LayoutGrid } from 'lucide-react';
import { Tooltip } from '@/component-library';
import { useI18n } from '@/infrastructure/i18n/hooks/useI18n';
import { useLiveAppStore } from '@/app/scenes/apps/live-app/liveAppStore';
import { renderLiveAppIcon, getLiveAppIconGradient } from '@/app/scenes/apps/live-app/liveAppIcons';

const MAX_VISIBLE_RUNNING_APPS = 3;

interface MiniAppEntryProps {
  isActive: boolean;
  activeMiniAppId?: string | null;
  onOpenMiniApps: () => void;
  onOpenMiniApp: (appId: string) => void;
}

const MiniAppEntry: React.FC<MiniAppEntryProps> = ({
  isActive,
  activeMiniAppId = null,
  onOpenMiniApps,
  onOpenMiniApp,
}) => {
  const { t } = useI18n('common');
  const apps = useLiveAppStore((state) => state.apps);
  const runningWorkerIds = useLiveAppStore((state) => state.runningWorkerIds);

  const runningApps = useMemo(() => {
    const appMap = new Map(apps.map((app) => [app.id, app]));
    const list = runningWorkerIds
      .map((id) => appMap.get(id))
      .filter((app): app is NonNullable<typeof app> => !!app);

    if (!activeMiniAppId) {
      return list;
    }

    return [...list].sort((a, b) => {
      if (a.id === activeMiniAppId) return -1;
      if (b.id === activeMiniAppId) return 1;
      return 0;
    });
  }, [activeMiniAppId, apps, runningWorkerIds]);

  const visibleApps = runningApps.slice(0, MAX_VISIBLE_RUNNING_APPS);
  const overflowCount = Math.max(0, runningApps.length - visibleApps.length);

  return (
    <div className="bitfun-nav-panel__miniapp-entry-wrap">
      <div
        className={[
          'bitfun-nav-panel__miniapp-entry',
          isActive && 'is-active',
          runningApps.length > 0 && 'has-running-apps',
        ].filter(Boolean).join(' ')}
        onClick={onOpenMiniApps}
        onKeyDown={(event) => {
          if (event.currentTarget !== event.target) return;
          if (event.key === 'Enter' || event.key === ' ') {
            event.preventDefault();
            onOpenMiniApps();
          }
        }}
        role="button"
        tabIndex={0}
        aria-label={t('nav.items.miniApps')}
      >
        <span className="bitfun-nav-panel__miniapp-entry-lead-icon" aria-hidden>
          <LayoutGrid size={15} />
        </span>
        <span className="bitfun-nav-panel__miniapp-entry-main">
          <span className="bitfun-nav-panel__miniapp-entry-title">{t('nav.items.miniApps')}</span>
        </span>

        <span className="bitfun-nav-panel__miniapp-entry-apps">
          {visibleApps.length > 0 ? (
            <>
              {visibleApps.map((app) => {
                const isAppActive = app.id === activeMiniAppId;
                return (
                  <Tooltip key={app.id} content={app.name} placement="right">
                    <span
                      className={[
                        'bitfun-nav-panel__miniapp-bubble',
                        isAppActive && 'is-active',
                      ].filter(Boolean).join(' ')}
                      style={{ background: getLiveAppIconGradient(app.icon || 'box') }}
                      onClick={(event) => {
                        event.stopPropagation();
                        onOpenMiniApp(app.id);
                      }}
                      onMouseDown={(event) => event.stopPropagation()}
                      role="button"
                      tabIndex={0}
                      aria-label={app.name}
                      onKeyDown={(event) => {
                        if (event.key === 'Enter' || event.key === ' ') {
                          event.preventDefault();
                          event.stopPropagation();
                          onOpenMiniApp(app.id);
                        }
                      }}
                    >
                      {renderLiveAppIcon(app.icon || 'box', 14)}
                    </span>
                  </Tooltip>
                );
              })}
              {overflowCount > 0 ? (
                <span className="bitfun-nav-panel__miniapp-bubble bitfun-nav-panel__miniapp-bubble--more">
                  +{overflowCount}
                </span>
              ) : null}
            </>
          ) : null}
        </span>
      </div>
    </div>
  );
};

export default MiniAppEntry;
