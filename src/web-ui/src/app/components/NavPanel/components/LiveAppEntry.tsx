import React, { useMemo } from 'react';
import { LayoutGrid } from 'lucide-react';
import { Tooltip } from '@/component-library';
import { useI18n } from '@/infrastructure/i18n/hooks/useI18n';
import { useLiveAppStore } from '@/app/scenes/apps/live-app/liveAppStore';
import { renderLiveAppIcon, getLiveAppIconGradient } from '@/app/scenes/apps/live-app/liveAppIcons';

const MAX_VISIBLE_RUNNING_APPS = 3;

interface LiveAppEntryProps {
  isActive: boolean;
  activeLiveAppId?: string | null;
  onOpenLiveApps: () => void;
  onOpenLiveApp: (appId: string) => void;
}

const LiveAppEntry: React.FC<LiveAppEntryProps> = ({
  isActive,
  activeLiveAppId = null,
  onOpenLiveApps,
  onOpenLiveApp,
}) => {
  const { t } = useI18n('common');
  const apps = useLiveAppStore((state) => state.apps);
  const runningWorkerIds = useLiveAppStore((state) => state.runningWorkerIds);

  const runningApps = useMemo(() => {
    const appMap = new Map(apps.map((app) => [app.id, app]));
    const list = runningWorkerIds
      .map((id) => appMap.get(id))
      .filter((app): app is NonNullable<typeof app> => !!app);

    if (!activeLiveAppId) {
      return list;
    }

    return [...list].sort((a, b) => {
      if (a.id === activeLiveAppId) return -1;
      if (b.id === activeLiveAppId) return 1;
      return 0;
    });
  }, [activeLiveAppId, apps, runningWorkerIds]);

  const visibleApps = runningApps.slice(0, MAX_VISIBLE_RUNNING_APPS);
  const overflowCount = Math.max(0, runningApps.length - visibleApps.length);

  return (
    <div className="bitfun-nav-panel__live-app-entry-wrap">
      <div
        className={[
          'bitfun-nav-panel__live-app-entry',
          isActive && 'is-active',
          runningApps.length > 0 && 'has-running-apps',
        ].filter(Boolean).join(' ')}
        onClick={onOpenLiveApps}
        onKeyDown={(event) => {
          if (event.currentTarget !== event.target) return;
          if (event.key === 'Enter' || event.key === ' ') {
            event.preventDefault();
            onOpenLiveApps();
          }
        }}
        role="button"
        tabIndex={0}
        aria-label={t('nav.items.liveApps')}
      >
        <span className="bitfun-nav-panel__live-app-entry-lead-icon" aria-hidden>
          <LayoutGrid size={15} />
        </span>
        <span className="bitfun-nav-panel__live-app-entry-main">
          <span className="bitfun-nav-panel__live-app-entry-title">{t('nav.items.liveApps')}</span>
        </span>

        <span className="bitfun-nav-panel__live-app-entry-apps">
          {visibleApps.length > 0 ? (
            <>
              {visibleApps.map((app) => {
                const isAppActive = app.id === activeLiveAppId;
                return (
                  <Tooltip key={app.id} content={app.name} placement="right">
                    <span
                      className={[
                        'bitfun-nav-panel__live-app-bubble',
                        isAppActive && 'is-active',
                      ].filter(Boolean).join(' ')}
                      style={{ background: getLiveAppIconGradient(app.icon || 'box') }}
                      onClick={(event) => {
                        event.stopPropagation();
                        onOpenLiveApp(app.id);
                      }}
                      onMouseDown={(event) => event.stopPropagation()}
                      role="button"
                      tabIndex={0}
                      aria-label={app.name}
                      onKeyDown={(event) => {
                        if (event.key === 'Enter' || event.key === ' ') {
                          event.preventDefault();
                          event.stopPropagation();
                          onOpenLiveApp(app.id);
                        }
                      }}
                    >
                      {renderLiveAppIcon(app.icon || 'box', 14)}
                    </span>
                  </Tooltip>
                );
              })}
              {overflowCount > 0 ? (
                <span className="bitfun-nav-panel__live-app-bubble bitfun-nav-panel__live-app-bubble--more">
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

export default LiveAppEntry;
