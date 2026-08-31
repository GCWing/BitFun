import React, { useMemo } from 'react';
import { useI18n } from '@/infrastructure/i18n/hooks/useI18n';
import { useMiniAppStore } from '@/app/scenes/miniapps/miniAppStore';
import { useMiniAppActivity } from '@/app/scenes/miniapps/hooks/useMiniAppActivity';
import { renderMiniAppIcon, getMiniAppIconGradient } from '@/app/scenes/miniapps/utils/miniAppIcons';
import { Icon, Tooltip } from '@bitfun/ui';

const MAX_VISIBLE_ACTIVE_APPS = 3;

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
  const activities = useMiniAppActivity();
  const customizingAppIds = useMiniAppStore((state) => state.customizingAppIds);
  const customizingIdSet = useMemo(() => new Set(customizingAppIds), [customizingAppIds]);
  const hasCustomizingApps = customizingAppIds.length > 0;

  const activeApps = useMemo(() => {
    const list = activities.map((activity) => activity.app);

    if (!activeMiniAppId) {
      return list;
    }

    return [...list].sort((a, b) => {
      if (a.id === activeMiniAppId) return -1;
      if (b.id === activeMiniAppId) return 1;
      return 0;
    });
  }, [activeMiniAppId, activities]);

  const visibleApps = activeApps.slice(0, MAX_VISIBLE_ACTIVE_APPS);
  const overflowCount = Math.max(0, activeApps.length - visibleApps.length);

  return (
    <div className="bitfun-nav-panel__miniapp-entry-wrap">
      <div
        className={[
          'bitfun-nav-panel__miniapp-entry',
          isActive && 'is-active',
          activeApps.length > 0 && 'has-running-apps',
          hasCustomizingApps && 'has-customizing-apps',
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
        aria-label={t('scenes.miniApps')}
        data-testid="nav-miniapps-entry"
      >
        <span className="bitfun-nav-panel__miniapp-entry-main">
          <span className="bitfun-nav-panel__miniapp-entry-icon" aria-hidden="true">
            <Icon name="mini-app" size="md" />
          </span>
          <span className="bitfun-nav-panel__miniapp-entry-copy">
            <span className="bitfun-nav-panel__miniapp-entry-title">{t('scenes.miniApps')}</span>
          </span>
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
                        customizingIdSet.has(app.id) && 'is-customizing',
                      ].filter(Boolean).join(' ')}
                      style={{ background: getMiniAppIconGradient(app.icon || 'box') }}
                      onClick={(event) => {
                        event.stopPropagation();
                        onOpenMiniApp(app.id);
                      }}
                      onMouseDown={(event) => event.stopPropagation()}
                      role="button"
                      tabIndex={0}
                      aria-label={app.name}
                      data-testid="nav-miniapp-activity-item"
                      data-miniapp-id={app.id}
                      onKeyDown={(event) => {
                        if (event.key === 'Enter' || event.key === ' ') {
                          event.preventDefault();
                          event.stopPropagation();
                          onOpenMiniApp(app.id);
                        }
                      }}
                    >
                      {renderMiniAppIcon(app.icon || 'box', 14)}
                      {customizingIdSet.has(app.id) && (
                        <span className="bitfun-nav-panel__miniapp-bubble-customize-dot" aria-hidden="true" />
                      )}
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
