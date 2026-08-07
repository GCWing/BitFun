import React from 'react';
import { Button } from '@/component-library';
import { ConfigPageSection } from '../common';
import type { ExternalApplicationView } from './applicationModel';
import type { TFunction } from 'i18next';

export interface ExternalAppDetailProps {
  application: ExternalApplicationView;
  t: TFunction;
  onBack: () => void;
  onOpenAdvanced: () => void;
}

const CAPABILITIES = [
  ['commands', 'commands'],
  ['tools', 'tools'],
  ['agents', 'agents'],
  ['mcps', 'mcps'],
] as const;

/**
 * Result-first application detail. V1 can summarize what was discovered and
 * what needs review; the existing capability controls remain reachable through
 * Advanced settings until the versioned review model can filter every owner by
 * application without guessing.
 */
export const ExternalAppDetail: React.FC<ExternalAppDetailProps> = ({
  application,
  t,
  onBack,
  onOpenAdvanced,
}) => (
  <div className="bitfun-external-sources-config__app-detail">
    <Button variant="ghost" size="small" onClick={onBack}>
      {t('applications.detail.back')}
    </Button>
    <div className="bitfun-external-sources-config__app-detail-heading">
      <div>
        <h2>{application.displayName}</h2>
        <p>{t(`applications.status.${application.status}`)}</p>
        {application.sourceCount > 0 ? (
          <small className="bitfun-external-sources-config__app-location-summary">
            {t('applications.detail.sourceSummary', { count: application.sourceCount })}
          </small>
        ) : null}
      </div>
      <Button variant="secondary" size="small" onClick={onOpenAdvanced}>
        {t('applications.actions.manage')}
      </Button>
    </div>

    {application.attentionCount > 0 ? (
      <button
        type="button"
        className="bitfun-external-sources-config__app-attention"
        onClick={onOpenAdvanced}
      >
        <span>
          <strong>{t('applications.detail.reviewTitle', { count: application.attentionCount })}</strong>
          <small>{t('applications.detail.reviewDescription')}</small>
        </span>
        <span aria-hidden="true">›</span>
      </button>
    ) : null}

      <ConfigPageSection
        title={t('applications.detail.usingTitle')}
        description={t('applications.detail.usingDescription')}
      >
      <div className="bitfun-external-sources-config__app-capabilities">
        {CAPABILITIES.map(([field, label]) => {
          const count = application.counts[field];
          return (
            <div key={field} className="bitfun-external-sources-config__app-capability">
              <span>
                <strong>{t(`applications.detail.capabilities.${label}`)}</strong>
                <small>{t('applications.detail.foundCount', { count })}</small>
              </span>
              <span>
                {application.connectPlan.find((entry) => entry.capabilityId === (
                  field === 'commands' ? 'command'
                    : field === 'tools' ? 'tool'
                      : field === 'agents' ? 'subagent'
                        : 'mcp'
                ))?.recommendedAccess === 'auto'
                  ? t('applications.detail.autoAvailable')
                  : t('applications.detail.managed')}
              </span>
            </div>
          );
        })}
      </div>
    </ConfigPageSection>
  </div>
);
