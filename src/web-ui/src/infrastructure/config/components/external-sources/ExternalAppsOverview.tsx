import { Switch, Tooltip } from '@bitfun/ui';
import React from 'react';
import { CircleAlert } from 'lucide-react';
import type { TFunction } from 'i18next';

import { ConfigPageSection } from '../common';
import type { ExternalApplicationView } from './applicationModel';

export interface ExternalAppsOverviewProps {
  applications: ExternalApplicationView[];
  t: TFunction;
  busy: boolean;
  canMutate: boolean;
  policiesEnabled: boolean;
  onToggle: (application: ExternalApplicationView, enabled: boolean) => void;
  onOpenAttention: (ecosystemId: string) => void;
  onOpenPolicy: () => void;
}

/**
 * A quiet application-level overview. Capability details and decisions remain
 * with their existing owners in Advanced settings; the overview only signals
 * when one of those owners needs the user's permission.
 */
export const ExternalAppsOverview: React.FC<ExternalAppsOverviewProps> = ({
  applications,
  t,
  busy,
  canMutate,
  policiesEnabled,
  onToggle,
  onOpenAttention,
  onOpenPolicy,
}) => (
  <ConfigPageSection
    className="bitfun-external-sources-config__apps"
    title={t('applications.title')}
  >
    <div className="bitfun-external-sources-config__app-list">
      {applications.map((application) => (
        <div
          key={application.ecosystemId}
          className="bitfun-external-sources-config__app-row"
          data-bf-component="external-sources-config"
          data-bf-part="application"
          data-bf-ecosystem={application.ecosystemId}
        >
          <span className="bitfun-external-sources-config__app-name">
            {application.displayName}
          </span>
          {application.attentionCount > 0 ? (
            <Tooltip content={t('applications.attentionRequired')} placement="top">
              <button
                type="button"
                className="bitfun-external-sources-config__app-attention"
                data-bf-component="external-sources-config"
                data-bf-part="appAttention"
                aria-label={t('applications.openAdvanced', {
                  name: application.displayName,
                })}
                onClick={() => onOpenAttention(application.ecosystemId)}
              >
                <CircleAlert size={16} aria-hidden="true" />
              </button>
            </Tooltip>
          ) : null}
          <div
            className="bitfun-external-sources-config__app-toggle"
            data-bf-component="external-sources-config"
            data-bf-part="applicationToggle"
            title={!policiesEnabled ? t('applications.enableInAdvanced') : undefined}
            role={!policiesEnabled ? 'button' : undefined}
            tabIndex={!policiesEnabled ? 0 : undefined}
            aria-label={!policiesEnabled ? t('applications.enableInAdvanced') : undefined}
            onClick={!policiesEnabled
              ? onOpenPolicy
              : undefined}
            onKeyDown={!policiesEnabled
              ? (event) => {
                  if (event.key !== 'Enter' && event.key !== ' ') return;
                  event.preventDefault();
                  onOpenPolicy();
                }
              : undefined}
          >
            <Switch
              checked={application.enabled}
              disabled={!canMutate || busy || !policiesEnabled}
              aria-busy={busy}
              aria-label={t('applications.toggleLabel', { name: application.displayName })}
              onChange={(event) => onToggle(application, event.currentTarget.checked)}
            />
          </div>
        </div>
      ))}
      {applications.length === 0 ? (
        <div className="bitfun-external-sources-config__app-empty">
          {t('applications.empty')}
        </div>
      ) : null}
    </div>
  </ConfigPageSection>
);
