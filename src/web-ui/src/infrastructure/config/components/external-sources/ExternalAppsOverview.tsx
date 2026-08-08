import React from 'react';
import { Button } from '@/component-library';
import { ConfigPageSection } from '../common';
import type { ExternalApplicationView } from './applicationModel';
import type { TFunction } from 'i18next';

export interface ExternalAppsOverviewProps {
  applications: ExternalApplicationView[];
  t: TFunction;
  onConnect: (application: ExternalApplicationView) => void;
  onManage: (application: ExternalApplicationView) => void;
}

function applicationSummary(application: ExternalApplicationView, t: TFunction): string {
  const parts = [
    application.counts.commands > 0
      ? t('applications.counts.commands', { count: application.counts.commands })
      : null,
    application.counts.tools > 0
      ? t('applications.counts.tools', { count: application.counts.tools })
      : null,
    application.counts.agents > 0
      ? t('applications.counts.agents', { count: application.counts.agents })
      : null,
    application.counts.mcps > 0
      ? t('applications.counts.mcps', { count: application.counts.mcps })
      : null,
  ].filter((part): part is string => Boolean(part));
  if (parts.length > 0) return parts.join(' · ');
  return application.status === 'checking'
    ? t('applications.summary.checking')
    : t('applications.summary.noContent');
}

/** The application-first entry point for external AI compatibility. */
export const ExternalAppsOverview: React.FC<ExternalAppsOverviewProps> = ({
  applications,
  t,
  onConnect,
  onManage,
}) => (
  <ConfigPageSection
    className="bitfun-external-sources-config__apps"
    title={t('applications.title')}
    description={t('applications.description')}
  >
    <div className="bitfun-external-sources-config__app-list">
      {applications.map((application) => {
        const canConnect = application.primaryAction === 'connect';
        const canManage = application.primaryAction === 'manage'
          || application.primaryAction === 'review';
        return (
          <div
            key={application.ecosystemId}
            className="bitfun-external-sources-config__app-row"
            data-bf-component="external-sources-config"
            data-bf-part="application"
            data-bf-state={application.status}
          >
            <div className="bitfun-external-sources-config__app-copy">
              <div className="bitfun-external-sources-config__app-heading">
                <span className="bitfun-external-sources-config__app-name">
                  {application.displayName}
                </span>
                <span className={`bitfun-external-sources-config__app-status is-${application.status}`}>
                  {t(`applications.status.${application.status}`)}
                </span>
              </div>
              <div className="bitfun-external-sources-config__app-summary">
                {application.attentionCount > 0
                  ? t('applications.summary.attention', { count: application.attentionCount })
                  : applicationSummary(application, t)}
              </div>
            </div>
            {canConnect ? (
              <Button size="small" variant="primary" onClick={() => onConnect(application)}>
                {t('applications.actions.connect')}
              </Button>
            ) : canManage ? (
              <Button size="small" variant="secondary" onClick={() => onManage(application)}>
                {application.primaryAction === 'review'
                  ? t('applications.actions.review')
                  : t('applications.actions.manage')}
              </Button>
            ) : null}
          </div>
        );
      })}
    </div>
  </ConfigPageSection>
);
