import React, { Suspense, useMemo } from 'react';
import { TabGroup } from '@openbitfun/ui';
import { useSettingsStore } from '../settingsStore';
import type { SettingsPageProps, SettingsViewId } from '../settingsTypes';
import './SettingsViewPage.scss';

export interface SettingsViewDefinition {
  id: SettingsViewId;
  label: React.ReactNode;
  content: React.ReactNode;
}

interface SettingsViewPageProps extends SettingsPageProps {
  defaultViewId: SettingsViewId;
  views: readonly SettingsViewDefinition[];
}

export const SettingsViewPage: React.FC<SettingsViewPageProps> = ({
  defaultViewId,
  views,
  viewId,
}) => {
  const setActiveView = useSettingsStore((state) => state.setActiveView);
  const allowedViewIds = useMemo(() => new Set(views.map((view) => view.id)), [views]);
  const activeViewId = viewId && allowedViewIds.has(viewId) ? viewId : defaultViewId;
  const activeView = views.find((view) => view.id === activeViewId) ?? views[0];
  const tabItems = views.map((view) => ({
    id: `settings-view-${view.id}-tab`,
    label: view.label,
    panelId: `settings-view-${view.id}-panel`,
    value: view.id,
  }));

  const handleChange = (nextViewId: string) => {
    if (!allowedViewIds.has(nextViewId as SettingsViewId)) return;
    const next = nextViewId as SettingsViewId;
    setActiveView(next);
  };

  return (
    <div
      className="openbitfun-settings-view-page"
      data-openbitfun-component="settings-view-page"
      data-openbitfun-part="root"
      data-openbitfun-view={activeViewId}
    >
      <div className="openbitfun-settings-view-page__tabs">
        <TabGroup
          className="openbitfun-settings-view-page__tab-list"
          items={tabItems}
          onValueChange={handleChange}
          value={activeViewId}
        />
        {activeView && (
          <div
            aria-labelledby={`settings-view-${activeView.id}-tab`}
            className="openbitfun-settings-view-page__tab-content"
            id={`settings-view-${activeView.id}-panel`}
            role="tabpanel"
          >
            <Suspense fallback={(
              <div
                className="openbitfun-settings-view-page__loading"
                data-openbitfun-component="settings-view-page"
                data-openbitfun-part="loading"
                aria-busy="true"
                aria-hidden="true"
              >
                <span className="openbitfun-settings-view-page__loading-line" data-openbitfun-component="settings-view-page" data-openbitfun-part="loadingLine" />
                <span className="openbitfun-settings-view-page__loading-line" data-openbitfun-component="settings-view-page" data-openbitfun-part="loadingLine" />
                <span className="openbitfun-settings-view-page__loading-block" data-openbitfun-component="settings-view-page" data-openbitfun-part="loadingBlock" />
              </div>
            )}>
              {activeView.content}
            </Suspense>
          </div>
        )}
      </div>
    </div>
  );
};
