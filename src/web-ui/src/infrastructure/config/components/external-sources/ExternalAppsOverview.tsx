import React, { useState } from 'react';
import { Switch } from '@/component-library';
import { ChevronDown, ChevronRight, Settings2 } from 'lucide-react';
import { ConfigPageSection } from '../common';
import type {
  ExternalApplicationCapabilityPlan,
  ExternalApplicationView,
} from './applicationModel';
import type { TFunction } from 'i18next';

export interface ExternalAppsOverviewProps {
  applications: ExternalApplicationView[];
  t: TFunction;
  totalAttentionCount: number;
  busy: boolean;
  canMutate: boolean;
  /** Master "use external AI applications" switch; per-app toggles are inert while it is off. */
  policiesEnabled: boolean;
  onToggle: (application: ExternalApplicationView, enabled: boolean) => void;
  onOpenAdvanced: () => void;
}

const CAPABILITY_LABEL: Record<string, string> = {
  command: 'applications.capabilities.command',
  tool: 'applications.capabilities.tool',
  subagent: 'applications.capabilities.agents',
  mcp: 'applications.capabilities.mcps',
};

function capabilityAccessLabel(
  capability: ExternalApplicationCapabilityPlan,
  t: TFunction,
): string {
  return t(`applications.capabilityAccess.${capability.effectiveAccess}`);
}

/**
 * The application-first tree entry point for external AI compatibility. Each
 * application is a row with a single recommended-automation switch; opening a
 * row reveals the capability types it exposes. Granular per-owner controls stay
 * in Advanced settings so the default page stays quiet and self-driving.
 */
export const ExternalAppsOverview: React.FC<ExternalAppsOverviewProps> = ({
  applications,
  t,
  totalAttentionCount,
  busy,
  canMutate,
  policiesEnabled,
  onToggle,
  onOpenAdvanced,
}) => {
  const [expanded, setExpanded] = useState<Set<string>>(() => new Set());

  return (
    <ConfigPageSection
      className="bitfun-external-sources-config__apps"
      title={t('applications.title')}
      description={t('applications.description')}
    >
      {totalAttentionCount > 0 ? (
        <button
          type="button"
          className="bitfun-external-sources-config__attention-summary"
          data-bf-component="external-sources-config"
          data-bf-part="attentionSummary"
          data-bf-count={totalAttentionCount}
          onClick={onOpenAdvanced}
        >
          <span>
            <strong>{t('applications.review.title', { count: totalAttentionCount })}</strong>
            <small>{t('applications.review.description')}</small>
          </span>
          <span className="bitfun-external-sources-config__attention-count">
            {totalAttentionCount}
          </span>
        </button>
      ) : null}

      <div className="bitfun-external-sources-config__app-list">
        {applications.map((application) => {
          const isExpanded = expanded.has(application.ecosystemId);
          const capabilityRows = application.connectPlan
            .filter((entry) => entry.count > 0);
          return (
            <div key={application.ecosystemId}>
              <div
                className="bitfun-external-sources-config__app-row"
                data-bf-component="external-sources-config"
                data-bf-part="application"
                data-bf-state={application.status}
                data-bf-ecosystem={application.ecosystemId}
              >
                <button
                  type="button"
                  className="bitfun-external-sources-config__app-expand"
                  aria-expanded={isExpanded}
                  aria-controls={`external-app-capabilities-${application.ecosystemId}`}
                  aria-label={t('applications.expand', { name: application.displayName })}
                  onClick={() => setExpanded((current) => {
                    const next = new Set(current);
                    if (next.has(application.ecosystemId)) next.delete(application.ecosystemId);
                    else next.add(application.ecosystemId);
                    return next;
                  })}
                >
                  {isExpanded
                    ? <ChevronDown size={16} aria-hidden="true" />
                    : <ChevronRight size={16} aria-hidden="true" />}
                </button>
                <div className="bitfun-external-sources-config__app-copy">
                  <div className="bitfun-external-sources-config__app-heading">
                    <span className="bitfun-external-sources-config__app-name">
                      {application.displayName}
                    </span>
                    {application.attentionCount > 0 ? (
                      <span
                        className="bitfun-external-sources-config__app-attention-dot"
                        data-bf-component="external-sources-config"
                        data-bf-part="appAttention"
                        title={t('applications.summary.attention', { count: application.attentionCount })}
                      >
                        {application.attentionCount}
                      </span>
                    ) : null}
                    <span className={`bitfun-external-sources-config__app-status is-${application.status}`}>
                      {t(`applications.status.${application.status}`)}
                    </span>
                  </div>
                  <div className="bitfun-external-sources-config__app-summary">
                    {application.activeCapabilities.length > 0
                      ? application.activeCapabilities.map((capability) => (
                        <span
                          key={capability.capabilityId}
                          className="bitfun-external-sources-config__app-capability-chip"
                        >
                          {t(CAPABILITY_LABEL[capability.capabilityId] ?? capability.capabilityId)}
                          <span>{capability.count}</span>
                        </span>
                      ))
                      : t('applications.summary.noContent')}
                  </div>
                </div>
                <div
                  className="bitfun-external-sources-config__app-toggle"
                  data-bf-component="external-sources-config"
                  data-bf-part="applicationToggle"
                >
                  <Switch
                    size="small"
                    checked={application.enabled}
                    disabled={!canMutate || busy || !policiesEnabled
                      || application.status === 'no_configuration'}
                    loading={busy}
                    aria-label={t('applications.toggleLabel', { name: application.displayName })}
                    onChange={(event) => onToggle(application, event.currentTarget.checked)}
                  />
                </div>
              </div>
              {isExpanded ? (
                <div
                  id={`external-app-capabilities-${application.ecosystemId}`}
                  className="bitfun-external-sources-config__app-capabilities"
                  data-bf-component="external-sources-config"
                  data-bf-part="appCapabilities"
                >
                  {capabilityRows.length > 0 ? (
                    capabilityRows.map((capability) => (
                      <div
                        key={capability.capabilityId}
                        className="bitfun-external-sources-config__app-capability"
                        data-bf-component="external-sources-config"
                        data-bf-part="appCapability"
                      >
                        <span>
                          <strong>
                            {t(CAPABILITY_LABEL[capability.capabilityId] ?? capability.capabilityId)}
                          </strong>
                          <small>
                            {t('applications.detail.foundCount', { count: capability.count })}
                          </small>
                        </span>
                        <span className="bitfun-external-sources-config__app-capability-access">
                          {capabilityAccessLabel(capability, t)}
                        </span>
                      </div>
                    ))
                  ) : (
                    <div className="bitfun-external-sources-config__app-capability-empty">
                      {t('applications.summary.noContent')}
                    </div>
                  )}
                  <button
                    type="button"
                    className="bitfun-external-sources-config__app-capability-manage"
                    onClick={onOpenAdvanced}
                  >
                    <Settings2 size={14} aria-hidden="true" />
                    {t('applications.actions.manage')}
                  </button>
                </div>
              ) : null}
            </div>
          );
        })}
      </div>
    </ConfigPageSection>
  );
};
