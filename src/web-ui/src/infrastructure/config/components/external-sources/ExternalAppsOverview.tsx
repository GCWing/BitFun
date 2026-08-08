import React, { useState } from 'react';
import { Switch } from '@/component-library';
import { ChevronDown, ChevronRight, Settings2 } from 'lucide-react';
import { ConfigPageSection } from '../common';
import type {
  ExternalApplicationCapabilityPlan,
  ExternalApplicationView,
} from './applicationModel';
import type {
  ExternalApplicationReviewItemResultV2,
  ExternalApplicationReviewItemRefV2,
  ExternalApplicationReviewItemV2,
} from '@/infrastructure/api/service-api/ExternalSourcesAPI';
import type { TFunction } from 'i18next';

export interface ExternalApplicationReviewView {
  open: boolean;
  loading: boolean;
  items: ExternalApplicationReviewItemV2[];
  selected: Record<string, boolean>;
  selectedCount: number;
  totalCount: number;
  maxSelectionCount: number;
  nextCursor?: string;
  itemResults: ExternalApplicationReviewItemResultV2[];
  completed: boolean;
  canSubmit: boolean;
  onClose: () => void;
  onToggleItem: (item: ExternalApplicationReviewItemV2, selected: boolean) => void;
  onLoadMore: () => void;
  onSubmit: () => void;
}

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
  onOpenReview?: () => void;
  review?: ExternalApplicationReviewView;
}

function reviewItemKey(item: ExternalApplicationReviewItemV2): string {
  return reviewItemRefKey(item.itemRef);
}

function reviewItemRefKey(itemRef: ExternalApplicationReviewItemRefV2): string {
  return `${itemRef.kind}:${itemRef.stableId}`;
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

function v2ApplicationSummary(
  application: ExternalApplicationView,
  t: TFunction,
): string {
  const facts = [t('applications.summary.enabledCount', { count: application.enabledCount })];
  if (application.health && application.health !== 'healthy') {
    facts.push(t(`applications.summary.health.${application.health}`));
  }
  if ((application.blockedCount ?? 0) > 0) {
    facts.push(t('applications.summary.blockedCount', { count: application.blockedCount }));
  }
  if ((application.conflictCount ?? 0) > 0) {
    facts.push(t('applications.summary.conflictCount', { count: application.conflictCount }));
  }
  application.recoveryActions?.forEach((action) => {
    facts.push(t(`recoveryActions.${action.type}`));
  });
  return facts.join(' · ');
}

/**
 * The application-first tree entry point for external AI compatibility. Each
 * application is a row with a single recommended-automation switch. Legacy
 * rows can reveal their inferred capability types; V2 rows render only Host
 * aggregates. Granular per-owner controls stay in Advanced settings.
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
  onOpenReview,
  review,
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
          onClick={onOpenReview ?? onOpenAdvanced}
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

      {review?.open ? (
        <div className="bitfun-external-sources-config__review">
          <div className="bitfun-external-sources-config__review-toolbar">
            <button type="button" onClick={review.onClose}>
              {t('applications.review.back')}
            </button>
            <span aria-live="polite">
              {t('applications.review.selectionCount', {
                selected: review.selectedCount,
                maximum: review.maxSelectionCount,
              })}
            </span>
          </div>
          <div className="bitfun-external-sources-config__app-list">
            {review.items.map((item) => {
              const key = reviewItemKey(item);
              const selected = review.selected[key] ?? item.recommended;
              const result = review.itemResults.find(
                (candidate) => reviewItemRefKey(candidate.itemRef) === key,
              );
              return (
                <label
                  key={key}
                  className="bitfun-external-sources-config__app-row"
                  data-bf-component="external-sources-config"
                  data-bf-part="reviewItem"
                >
                  <input
                    type="checkbox"
                    checked={selected}
                    disabled={busy
                      || review.completed
                      || item.safetyCeiling === 'blocked'
                      || (!selected && review.selectedCount >= review.maxSelectionCount)}
                    onChange={(event) => review.onToggleItem(item, event.currentTarget.checked)}
                  />
                  <span className="bitfun-external-sources-config__app-copy">
                    <strong>{item.displayName}</strong>
                    <small>{item.displaySummary}</small>
                    {result ? (
                      <small>
                        {t(`applications.review.itemOutcome.${result.outcome}`)}
                      </small>
                    ) : null}
                  </span>
                  <span className={`bitfun-external-sources-config__app-status is-${item.riskLevel}`}>
                    {item.safetyCeiling === 'blocked'
                      ? t('applications.review.safety.blocked')
                      : t(`applications.review.risk.${item.riskLevel}`)}
                  </span>
                </label>
              );
            })}
          </div>
          {review.loading ? (
            <div role="status">{t('applications.review.loading')}</div>
          ) : null}
          <div className="bitfun-external-sources-config__review-actions">
            {review.nextCursor ? (
              <button
                type="button"
                data-bf-component="external-sources-config"
                data-bf-part="loadMoreReview"
                disabled={busy || review.loading || review.completed}
                onClick={review.onLoadMore}
              >
                {t('applications.review.loadMore')}
              </button>
            ) : null}
            <button
              type="button"
              data-bf-component="external-sources-config"
              data-bf-part="submitReview"
              disabled={busy
                || !review.canSubmit
                || review.loading
                || review.completed
                || review.items.length === 0}
              onClick={review.onSubmit}
            >
              {t('applications.review.apply')}
            </button>
          </div>
        </div>
      ) : (
        <div className="bitfun-external-sources-config__app-list">
          {applications.map((application) => {
            const isExpanded = expanded.has(application.ecosystemId);
            const hasCapabilityDetails = application.enabledCount === undefined;
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
                  {hasCapabilityDetails ? (
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
                  ) : null}
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
                      {application.enabledCount !== undefined
                        ? v2ApplicationSummary(application, t)
                        : application.activeCapabilities.length > 0
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
                        || application.status === 'no_configuration'
                        || (!application.enabled && application.primaryAction !== 'connect')}
                      loading={busy}
                      aria-label={t('applications.toggleLabel', { name: application.displayName })}
                      onChange={(event) => onToggle(application, event.currentTarget.checked)}
                    />
                  </div>
                </div>
                {hasCapabilityDetails && isExpanded ? (
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
      )}
    </ConfigPageSection>
  );
};
