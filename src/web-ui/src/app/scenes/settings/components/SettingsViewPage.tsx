import React, { Suspense, useEffect, useMemo, useState } from 'react';
import { TabGroup } from '@bitfun/ui';
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
  navigationRequestId,
}) => {
  const setActiveView = useSettingsStore((state) => state.setActiveView);
  const allowedViewIds = useMemo(() => new Set(views.map((view) => view.id)), [views]);
  const requestedViewId = viewId && allowedViewIds.has(viewId) ? viewId : defaultViewId;
  const [activeViewId, setActiveViewId] = useState<SettingsViewId>(requestedViewId);

  useEffect(() => {
    setActiveViewId(requestedViewId);
  }, [navigationRequestId, requestedViewId]);

  const handleChange = (nextViewId: string) => {
    if (!allowedViewIds.has(nextViewId as SettingsViewId)) return;
    const next = nextViewId as SettingsViewId;
    setActiveViewId(next);
    setActiveView(next);
  };
  const activeView = views.find((view) => view.id === activeViewId) ?? views[0];

  return (
    <div
      className="bitfun-settings-view-page"
      data-bf-component="settings-view-page"
      data-bf-part="root"
      data-bf-view={activeViewId}
    >
      <TabGroup
        className="bitfun-settings-view-page__tabs"
        items={views.map((view) => ({
          id: `settings-view-tab-${view.id}`,
          label: view.label,
          panelId: `settings-view-panel-${view.id}`,
          value: view.id,
        }))}
        onValueChange={handleChange}
        value={activeViewId}
      />
      <div
        aria-labelledby={`settings-view-tab-${activeViewId}`}
        className="bitfun-settings-view-page__panel"
        id={`settings-view-panel-${activeViewId}`}
        role="tabpanel"
      >
        <Suspense fallback={(
          <div
            className="bitfun-settings-view-page__loading"
            data-bf-component="settings-view-page"
            data-bf-part="loading"
            aria-busy="true"
            aria-hidden="true"
          >
            <span className="bitfun-settings-view-page__loading-line" data-bf-component="settings-view-page" data-bf-part="loadingLine" />
            <span className="bitfun-settings-view-page__loading-line" data-bf-component="settings-view-page" data-bf-part="loadingLine" />
            <span className="bitfun-settings-view-page__loading-block" data-bf-component="settings-view-page" data-bf-part="loadingBlock" />
          </div>
        )}>
          {activeView?.content}
        </Suspense>
      </div>
    </div>
  );
};
