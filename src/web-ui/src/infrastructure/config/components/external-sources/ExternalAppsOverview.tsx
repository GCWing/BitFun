import React, { useState } from 'react';
import { Switch, Tooltip } from '@/component-library';
import { ArrowLeft, ChevronDown, ChevronRight, CircleAlert, Settings2 } from 'lucide-react';
import { ConfigPageRow, ConfigPageSection } from '../common';
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
  recommendedCount: number;
  totalCount: number;
  maxSelectionCount: number;
  applicationNames: string[];
  nextCursor?: string;
  itemResults: ExternalApplicationReviewItemResultV2[];
  completed: boolean;
  canSubmit: boolean;
  onClose: () => void;
  onToggleItem: (item: ExternalApplicationReviewItemV2, selected: boolean) => void;
  onLoadMore: () => void;
  onSubmit: (
    baseline: 'recommended' | 'none',
    immediateSelection?: { item: ExternalApplicationReviewItemV2; selected: boolean },
  ) => void;
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

const REVIEW_CATEGORY_LABEL: Record<ExternalApplicationReviewItemRefV2['kind'], string> = {
  command: 'applications.review.category.command',
  tool: 'applications.review.category.tool',
  subagent: 'applications.review.category.subagent',
  mcp: 'applications.review.category.mcp',
  conflict: 'applications.review.category.conflict',
};

const REVIEW_REASON_LABEL: Record<string, string> = {
  process_or_resource_access: 'applications.review.riskReason.processOrResourceAccess',
  process_or_network_access: 'applications.review.riskReason.processOrNetworkAccess',
  delegated_tool_access: 'applications.review.riskReason.delegatedToolAccess',
  ambiguous_runtime_route: 'applications.review.riskReason.ambiguousRuntimeRoute',
};

function capabilityAccessLabel(
  capability: ExternalApplicationCapabilityPlan,
  t: TFunction,
): string {
  return t(`applications.capabilityAccess.${capability.effectiveAccess}`);
}

function v2ApplicationFacts(
  application: ExternalApplicationView,
  t: TFunction,
): string {
  const facts: string[] = [];
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
  const openingReview = review?.open && review.loading && review.items.length === 0;
  const singleReviewItem = review?.totalCount === 1 && review.items.length === 1
    ? review.items[0]
    : undefined;
  const reviewApplicationLabel = review?.applicationNames.length
    ? review.applicationNames.join(', ')
    : t('applications.review.unknownApplication');
  const hasSelectableReviewItem = review?.items.some(
    (item) => item.safetyCeiling !== 'blocked',
  ) ?? false;
  const canCustomizeReview = Boolean(review
    && hasSelectableReviewItem
    && review.totalCount > 1);
  const reviewSubmitDisabled = Boolean(!review
    || busy
    || !review.canSubmit
    || review.loading
    || review.completed
    || review.items.length === 0);
  const singleReviewReason = singleReviewItem?.riskReasonCodes
    .map((code) => REVIEW_REASON_LABEL[code])
    .find(Boolean);

  const reviewDescription = review && singleReviewItem ? (
    <>
      <span>
        {t(REVIEW_CATEGORY_LABEL[singleReviewItem.itemRef.kind])}
        {' · '}
        {singleReviewItem.displayName}
        {' · '}
        {singleReviewItem.safetyCeiling === 'blocked'
          ? t('applications.review.safety.blocked')
          : t(`applications.review.risk.${singleReviewItem.riskLevel}`)}
      </span>
      <br />
      <span>
        {singleReviewReason ? `${t(singleReviewReason)} ` : ''}
        {singleReviewItem.safetyCeiling === 'blocked'
          ? t('applications.review.recommendation.blocked')
          : singleReviewItem.recommended
          ? t('applications.review.recommendation.enable')
          : t('applications.review.recommendation.keepDisabled')}
      </span>
    </>
  ) : review ? (
    t('applications.review.recommendation.multiple', {
      count: review.totalCount,
      recommended: review.recommendedCount,
    })
  ) : null;

  return (
    <ConfigPageSection
      className="bitfun-external-sources-config__apps"
      title={t('applications.title')}
    >
      {totalAttentionCount > 0 && !review?.open ? (
        <button
          type="button"
          className="bitfun-external-sources-config__attention-summary"
          data-bf-component="external-sources-config"
          data-bf-part="attentionSummary"
          data-bf-count={totalAttentionCount}
          onClick={onOpenReview ?? onOpenAdvanced}
        >
          <strong>{t('applications.review.title', { count: totalAttentionCount })}</strong>
        </button>
      ) : null}

      {review?.open ? (
        <div className="bitfun-external-sources-config__review">
          <div className="bitfun-external-sources-config__review-toolbar">
            <button
              type="button"
              className="btn btn-ghost btn-sm"
              onClick={review.onClose}
            >
              <ArrowLeft size={14} aria-hidden="true" />
              {t('applications.review.back')}
            </button>
          </div>
          {openingReview ? (
            <ConfigPageRow
              className="bitfun-external-sources-config__review-loading"
              label={<span role="status">{t('applications.review.loading')}</span>}
              multiline
            >
              {null}
            </ConfigPageRow>
          ) : null}
          {!openingReview ? (
            <>
              <ConfigPageRow
                className="bitfun-external-sources-config__review-decision"
                label={reviewApplicationLabel}
                description={reviewDescription}
                align="center"
              >
                <div className="bitfun-external-sources-config__review-actions">
                  {singleReviewItem ? (
                    <>
                      <button
                        type="button"
                        className={singleReviewItem.safetyCeiling === 'blocked'
                          ? 'btn btn-primary btn-sm'
                          : 'btn btn-secondary btn-sm'}
                        data-bf-component="external-sources-config"
                        data-bf-part="submitReview"
                        data-review-baseline="none"
                        disabled={reviewSubmitDisabled}
                        onClick={() => review.onSubmit('none')}
                      >
                        {t(singleReviewItem.recommended
                          && singleReviewItem.safetyCeiling !== 'blocked'
                          ? 'applications.review.doNotEnable'
                          : 'applications.review.keepDisabled')}
                      </button>
                      {singleReviewItem.safetyCeiling !== 'blocked' ? (
                        <button
                          type="button"
                          className="btn btn-primary btn-sm"
                          data-bf-component="external-sources-config"
                          data-bf-part="submitReview"
                          data-review-baseline="recommended"
                          disabled={reviewSubmitDisabled}
                          onClick={() => review.onSubmit('recommended', {
                            item: singleReviewItem,
                            selected: true,
                          })}
                        >
                          {t('applications.review.enableThisItem')}
                        </button>
                      ) : null}
                    </>
                  ) : (
                    <>
                      {review.selectedCount > 0 ? (
                        <button
                          type="button"
                          className="btn btn-secondary btn-sm"
                          data-bf-component="external-sources-config"
                          data-bf-part="submitReview"
                          data-review-baseline="none"
                          disabled={reviewSubmitDisabled}
                          onClick={() => review.onSubmit('none')}
                        >
                          {t('applications.review.doNotEnableAny')}
                        </button>
                      ) : null}
                      <button
                        type="button"
                        className="btn btn-primary btn-sm"
                        data-bf-component="external-sources-config"
                        data-bf-part="submitReview"
                        data-review-baseline={review.selectedCount > 0 ? 'recommended' : 'none'}
                        disabled={reviewSubmitDisabled}
                        onClick={() => review.onSubmit(
                          review.selectedCount > 0 ? 'recommended' : 'none',
                        )}
                      >
                        {t(review.selectedCount === 0
                          ? 'applications.review.keepDisabled'
                          : Object.keys(review.selected).length > 0
                          ? 'applications.review.enableSelected'
                          : 'applications.review.enableRecommended', {
                          count: review.selectedCount,
                        })}
                      </button>
                    </>
                  )}
                </div>
              </ConfigPageRow>
              {canCustomizeReview ? (
                <details className="bitfun-external-sources-config__review-adjustments">
                  <summary>{t('applications.review.customize')}</summary>
                <div aria-live="polite">
                  {t('applications.review.selectionCount', {
                    selected: review.selectedCount,
                    maximum: review.maxSelectionCount,
                  })}
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
                {review.nextCursor ? (
                  <div className="bitfun-external-sources-config__review-actions">
                    <button
                      type="button"
                      className="btn btn-secondary btn-sm"
                      data-bf-component="external-sources-config"
                      data-bf-part="loadMoreReview"
                      disabled={busy || review.loading || review.completed}
                      onClick={review.onLoadMore}
                    >
                      {t('applications.review.loadMore')}
                    </button>
                  </div>
                ) : null}
                </details>
              ) : null}
            </>
          ) : null}
        </div>
      ) : (
        <div className="bitfun-external-sources-config__app-list">
          {applications.map((application) => {
            const isExpanded = expanded.has(application.ecosystemId);
            const hasCapabilityDetails = application.enabledCount === undefined;
            const capabilityRows = application.connectPlan
              .filter((entry) => entry.count > 0);
            const applicationFacts = application.enabledCount !== undefined
              ? v2ApplicationFacts(application, t)
              : '';
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
                      <span className={`bitfun-external-sources-config__app-status is-${application.status}`}>
                        {t(`applications.status.${application.status}`)}
                      </span>
                      {applicationFacts ? (
                        <Tooltip content={applicationFacts} placement="top">
                          <span
                            className="bitfun-external-sources-config__app-facts"
                            data-bf-component="external-sources-config"
                            data-bf-part="applicationFacts"
                            role="img"
                            tabIndex={0}
                            aria-label={applicationFacts}
                          >
                            <CircleAlert size={14} aria-hidden="true" />
                          </span>
                        </Tooltip>
                      ) : null}
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
