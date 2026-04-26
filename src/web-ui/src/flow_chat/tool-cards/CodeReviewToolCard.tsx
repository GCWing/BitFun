/**
 * CodeReview tool display component
 * Displays structured code review results with collapsible/expandable details
 * Refactored based on BaseToolCard
 */

import React, { useState, useMemo, useCallback, useEffect, useRef } from 'react';
import {
  Loader2,
  AlertTriangle,
  AlertCircle,
  Info,
  Clock,
  ChevronDown,
  ChevronUp,
} from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { Tooltip } from '@/component-library';
import type { ToolCardProps } from '../types/flow-chat';
import { BaseToolCard, ToolCardHeader } from './BaseToolCard';
import { createLogger } from '@/shared/utils/logger';
import { useToolCardHeightContract } from './useToolCardHeightContract';
import {
  buildReviewRemediationItems,
} from '../utils/codeReviewRemediation';
import {
  buildCodeReviewReportSections,
  getDefaultExpandedCodeReviewSectionIds,
  type CodeReviewReportData,
  type CodeReviewReviewer,
  type RemediationGroupId,
  type ReviewReportGroup,
  type ReviewSectionId,
  type StrengthGroupId,
} from '../utils/codeReviewReport';
import { CodeReviewReportExportActions } from './CodeReviewReportExportActions';
import './CodeReviewToolCard.scss';

const log = createLogger('CodeReviewToolCard');

const riskLevelColors: Record<string, string> = {
  low: '#22c55e',
  medium: '#f59e0b',
  high: '#f97316',
  critical: '#ef4444',
};

type Translate = (key: string, options?: Record<string, unknown>) => string;

interface ReviewReportSectionProps {
  title: string;
  summary?: string;
  expanded: boolean;
  onToggle: (event: React.MouseEvent<HTMLButtonElement>) => void;
  children: React.ReactNode;
}

const ReviewReportSection: React.FC<ReviewReportSectionProps> = ({
  title,
  summary,
  expanded,
  onToggle,
  children,
}) => (
  <section className={`review-report-section ${expanded ? 'is-expanded' : ''}`}>
    <button
      type="button"
      className="review-report-section__header"
      onClick={onToggle}
      aria-expanded={expanded}
    >
      <span className="review-report-section__title">{title}</span>
      {summary && <span className="review-report-section__summary">{summary}</span>}
      {expanded ? <ChevronUp size={13} /> : <ChevronDown size={13} />}
    </button>
    {expanded && (
      <div className="review-report-section__body">
        {children}
      </div>
    )}
  </section>
);

function getRemediationGroupTitle(id: RemediationGroupId, t: Translate): string {
  return t(`toolCards.codeReview.groups.${id}`, {
    defaultValue: id,
  });
}

function getStrengthGroupTitle(id: StrengthGroupId, t: Translate): string {
  return t(`toolCards.codeReview.groups.${id}`, {
    defaultValue: id,
  });
}

function formatIssueStats(stats: { critical: number; high: number; medium: number; low: number; info: number; total: number }, t: Translate): string {
  if (stats.total === 0) {
    return t('toolCards.codeReview.noIssues', { defaultValue: 'No issues' });
  }

  return (['critical', 'high', 'medium', 'low', 'info'] as const)
    .filter((severity) => stats[severity] > 0)
    .map((severity) => `${stats[severity]} ${t(`toolCards.codeReview.severities.${severity}`, { defaultValue: severity })}`)
    .join(' · ');
}

function formatReviewerStats(stats: { total: number; completed: number; degraded: number }, t: Translate): string {
  return t('toolCards.codeReview.reviewerTeamSummary', {
    total: stats.total,
    completed: stats.completed,
    degraded: stats.degraded,
    defaultValue: '{{total}} reviewers · {{completed}} completed · {{degraded}} attention',
  });
}

function renderReportGroupList<TId extends RemediationGroupId | StrengthGroupId>(
  groups: Array<ReviewReportGroup<TId>>,
  titleForGroup: (id: TId) => string,
): React.ReactNode {
  return groups.map((group) => (
    <div key={group.id} className="review-report-group">
      <div className="review-report-group__title">{titleForGroup(group.id)}</div>
      <ul className="review-report-group__list">
        {group.items.map((item, index) => (
          <li key={`${group.id}-${index}`}>{item}</li>
        ))}
      </ul>
    </div>
  ));
}

export const CodeReviewToolCard: React.FC<ToolCardProps> = React.memo(({
  toolItem,
  sessionId: _sessionId,
}) => {
  const { t } = useTranslation('flow-chat');
  const { toolResult, status } = toolItem;
  const [isExpanded, setIsExpanded] = useState(false);
  const [expandedRemediationIds, setExpandedRemediationIds] = useState<Set<string>>(new Set());
  const [expandedReportSectionIds, setExpandedReportSectionIds] = useState<Set<ReviewSectionId>>(new Set());
  const autoExpandedResultRef = useRef<string | null>(null);
  const toolId = toolItem.id ?? toolItem.toolCall?.id;
  const { cardRootRef, applyExpandedState } = useToolCardHeightContract({
    toolId,
    toolName: toolItem.toolName,
  });

  const getStatusIcon = () => {
    switch (status) {
      case 'running':
      case 'streaming':
        return <Loader2 className="animate-spin" size={12} />;
      case 'completed':
        return null;
      case 'pending':
      default:
        return <Clock size={12} />;
    }
  };

  const reviewData = useMemo<CodeReviewReportData | null>(() => {
    if (!toolResult?.result) return null;

    try {
      const result = toolResult.result;

      if (typeof result === 'string') {
        const parsed = JSON.parse(result);
        return parsed;
      }

      if (typeof result === 'object' && result.summary) {
        return result as CodeReviewReportData;
      }

      return null;
    } catch (error) {
      log.error('Failed to parse result', error);
      return null;
    }
  }, [toolResult?.result]);

  useEffect(() => {
    setExpandedRemediationIds(new Set());
    setExpandedReportSectionIds(new Set(reviewData ? getDefaultExpandedCodeReviewSectionIds(reviewData) : []));
  }, [reviewData, toolResult?.result]);

  const issueStats = useMemo(() => {
    if (!reviewData) return null;

    const stats = {
      critical: 0,
      high: 0,
      medium: 0,
      low: 0,
      info: 0,
      total: 0,
    };

    (reviewData.issues ?? []).forEach(issue => {
      stats[issue.severity ?? 'info']++;
      stats.total++;
    });

    return stats;
  }, [reviewData]);

  const getSeverityIcon = (severity: string) => {
    switch (severity) {
      case 'critical':
        return <AlertCircle size={14} style={{ color: riskLevelColors.critical }} />;
      case 'high':
        return <AlertTriangle size={14} style={{ color: riskLevelColors.high }} />;
      case 'medium':
        return <AlertTriangle size={14} style={{ color: riskLevelColors.medium }} />;
      case 'low':
        return <Info size={14} style={{ color: riskLevelColors.low }} />;
      case 'info':
        return <Info size={14} style={{ color: '#6b7280' }} />;
      default:
        return <Info size={14} style={{ color: '#6b7280' }} />;
    }
  };

  const getSeverityClass = (severity: string) => {
    switch (severity) {
      case 'critical':
        return 'critical';
      case 'high':
        return 'high';
      case 'medium':
        return 'medium';
      case 'low':
        return 'low';
      case 'info':
      default:
        return 'info';
    }
  };

  const hasIssues = issueStats && issueStats.total > 0;
  const hasData = reviewData !== null;
  const remediationItems = useMemo(
    () => reviewData ? buildReviewRemediationItems(reviewData) : [],
    [reviewData],
  );

  useEffect(() => {
    const resultKey = typeof toolResult?.result === 'string'
      ? toolResult.result
      : JSON.stringify(toolResult?.result ?? null);
    const shouldAutoExpand =
      status === 'completed' &&
      reviewData?.review_mode === 'deep' &&
      buildReviewRemediationItems(reviewData).length > 0 &&
      autoExpandedResultRef.current !== resultKey;

    if (shouldAutoExpand) {
      autoExpandedResultRef.current = resultKey;
      setIsExpanded(true);
    }
  }, [reviewData, status, toolResult?.result]);

  const toggleExpanded = useCallback(() => {
    applyExpandedState(isExpanded, !isExpanded, setIsExpanded);
  }, [applyExpandedState, isExpanded]);

  const handleCardClick = useCallback((e: React.MouseEvent) => {
    const target = e.target as HTMLElement;
    if (target.closest('.preview-toggle-btn')) {
      return;
    }

    if (hasData) {
      toggleExpanded();
    }
  }, [hasData, toggleExpanded]);

  const handleToggleExpand = useCallback((e: React.MouseEvent) => {
    e.stopPropagation();
    toggleExpanded();
  }, [toggleExpanded]);

  const handleToggleRemediationDetails = useCallback((itemId: string) => {
    setExpandedRemediationIds((current) => {
      const next = new Set(current);
      if (next.has(itemId)) {
        next.delete(itemId);
      } else {
        next.add(itemId);
      }
      return next;
    });
  }, []);

  const handleToggleReportSection = useCallback((sectionId: ReviewSectionId) => (
    event: React.MouseEvent<HTMLButtonElement>
  ) => {
    event.stopPropagation();
    setExpandedReportSectionIds((current) => {
      const next = new Set(current);
      if (next.has(sectionId)) {
        next.delete(sectionId);
      } else {
        next.add(sectionId);
      }
      return next;
    });
  }, []);

  const renderContent = () => {
    if (status === 'completed' && reviewData) {
      const riskLevel = reviewData.summary?.risk_level ?? 'low';
      const reviewLabel = reviewData.review_mode === 'deep'
        ? t('toolCards.codeReview.deepReviewResult', { defaultValue: 'Deep Review Result' })
        : t('toolCards.codeReview.reviewResult');

      if (hasIssues) {
        const parts: React.ReactNode[] = [];
        if (issueStats!.critical > 0) {
          parts.push(
            <span key="critical" style={{ color: riskLevelColors.critical }}>
              {issueStats!.critical} {t('toolCards.codeReview.severities.critical')}
            </span>,
          );
        }
        if (issueStats!.high > 0) {
          parts.push(
            <span key="high" style={{ color: riskLevelColors.high }}>
              {issueStats!.high} {t('toolCards.codeReview.severities.high')}
            </span>,
          );
        }
        if (issueStats!.medium > 0) {
          parts.push(
            <span key="medium" style={{ color: riskLevelColors.medium }}>
              {issueStats!.medium} {t('toolCards.codeReview.severities.medium')}
            </span>,
          );
        }
        if (issueStats!.low > 0) {
          parts.push(
            <span key="low" style={{ color: riskLevelColors.low }}>
              {issueStats!.low} {t('toolCards.codeReview.severities.low')}
            </span>,
          );
        }

        return (
          <>
            {reviewLabel} -{' '}
            {parts.reduce<React.ReactNode[]>((acc, part, i) => {
              if (i > 0) acc.push(<span key={`sep-${i}`}>, </span>);
              acc.push(part);
              return acc;
            }, [])}
          </>
        );
      }

      return (
        <>
          {reviewLabel} - {t(`toolCards.codeReview.riskLevels.${riskLevel}`)}
        </>
      );
    }

    if (status === 'running' || status === 'streaming') {
      return <>{t('toolCards.codeReview.reviewingCode')}</>;
    }

    if (status === 'pending') {
      return <>{t('toolCards.codeReview.preparingReview')}</>;
    }

    if (status === 'error') {
      return <>{t('toolCards.codeReview.reviewFailed', { error: toolResult?.error || t('toolCards.codeReview.unknownError') })}</>;
    }

    return null;
  };

  const renderHeader = () => {
    return (
      <ToolCardHeader
        icon={null}
        iconClassName="code-review-icon"
        content={renderContent()}
        extra={(
          <>
            {hasData && reviewData && (
              <CodeReviewReportExportActions reviewData={reviewData} />
            )}
            {hasData && (
              <Tooltip
                content={isExpanded ? t('toolCards.codeReview.collapseDetails') : t('toolCards.codeReview.expandDetails')}
                placement="top"
              >
                <button
                  className="preview-toggle-btn"
                  onClick={handleToggleExpand}
                >
                  {isExpanded ? <ChevronUp size={14} /> : <ChevronDown size={14} />}
                </button>
              </Tooltip>
            )}
          </>
        )}
        statusIcon={getStatusIcon()}
      />
    );
  };

  const expandedContent = useMemo(() => {
    if (!reviewData) return null;

    const summary = reviewData.summary ?? {};
    const issues = reviewData.issues ?? [];
    const review_mode = reviewData.review_mode;
    const review_scope = reviewData.review_scope;
    const reviewers = reviewData.reviewers ?? [];
    const reportSections = buildCodeReviewReportSections(reviewData);
    const riskLevel = summary.risk_level ?? 'low';
    const recommendedAction = summary.recommended_action ?? 'approve';
    const remediationItemCount = reportSections.remediationGroups
      .reduce((total, group) => total + group.items.length, 0);
    const strengthItemCount = reportSections.strengthGroups
      .reduce((total, group) => total + group.items.length, 0);
    const remediationExpanded = expandedReportSectionIds.has('remediation');
    const issuesExpanded = expandedReportSectionIds.has('issues');
    const strengthsExpanded = expandedReportSectionIds.has('strengths');
    const teamExpanded = expandedReportSectionIds.has('team');
    const coverageExpanded = expandedReportSectionIds.has('coverage');

    return (
      <div className="code-review-details">
        <div className="review-summary">
          <div className="summary-header">{t('toolCards.codeReview.overallAssessment')}</div>
          <div className="summary-rows">
            <div className="summary-row">
              <span className="summary-label">{t('toolCards.codeReview.riskLevel')}</span>
              <span
                className="summary-value risk-level"
                style={{ color: riskLevelColors[riskLevel] }}
              >
                {getSeverityIcon(riskLevel)}
                <span>{t(`toolCards.codeReview.riskLevels.${riskLevel}`)}</span>
              </span>
            </div>
            <div className="summary-row">
              <span className="summary-label">{t('toolCards.codeReview.recommendedAction')}</span>
              <span className="summary-value">{t(`toolCards.codeReview.actions.${recommendedAction}`)}</span>
            </div>
            {review_mode && (
              <div className="summary-row">
                <span className="summary-label">{t('toolCards.codeReview.reviewMode', { defaultValue: 'Review Mode' })}</span>
                <span className="summary-value">{t(`toolCards.codeReview.reviewModes.${review_mode}`, { defaultValue: review_mode })}</span>
              </div>
            )}
            {review_scope && (
              <div className="summary-row summary-row--full">
                <span className="summary-label">{t('toolCards.codeReview.reviewScope', { defaultValue: 'Scope' })}</span>
                <span className="summary-value">{review_scope}</span>
              </div>
            )}
            {reportSections.executiveSummary.length > 0 && (
              <div className="summary-row summary-row--full">
                <span className="summary-label">
                  {t('toolCards.codeReview.sections.summary', { defaultValue: 'Executive Summary' })}
                </span>
                <span className="summary-value">
                  {reportSections.executiveSummary.join(' ')}
                </span>
              </div>
            )}
            {summary.confidence_note && (
              <div className="summary-row summary-row--full">
                <span className="summary-label">{t('toolCards.codeReview.contextLimitations')}</span>
                <span className="summary-value note">{summary.confidence_note}</span>
              </div>
            )}
          </div>
        </div>

        {issues.length > 0 && (
          <ReviewReportSection
            title={t('toolCards.codeReview.issuesCount', { count: issues.length })}
            summary={formatIssueStats(reportSections.issueStats, t)}
            expanded={issuesExpanded}
            onToggle={handleToggleReportSection('issues')}
          >
            <div className="issues-list">
              {issues.map((issue, index) => (
                <div
                  key={index}
                  className={`review-issue-item severity-${getSeverityClass(issue.severity ?? 'info')}`}
                >
                  <div className="issue-header">
                    <div className="issue-left">
                      {getSeverityIcon(issue.severity ?? 'info')}
                      {issue.category && (
                        <span className="issue-category">[{issue.category}]</span>
                      )}
                      {issue.source_reviewer && (
                        <span className="issue-source">{issue.source_reviewer}</span>
                      )}
                      {issue.file && (
                        <span className="issue-location">
                          {issue.file}{issue.line ? `:${issue.line}` : ''}
                        </span>
                      )}
                    </div>
                    <span className="issue-certainty">
                      {t(`toolCards.codeReview.certainties.${issue.certainty ?? 'possible'}`)}
                    </span>
                  </div>
                  <div className="issue-title">{issue.title}</div>
                  <div className="issue-description">{issue.description}</div>
                  {issue.validation_note && (
                    <div className="issue-validation-note">
                      {issue.validation_note}
                    </div>
                  )}
                  {issue.suggestion && (
                    <div className="issue-suggestion">
                      <span className="suggestion-label">{t('toolCards.codeReview.suggestion')}:</span>
                      <span className="suggestion-text">{issue.suggestion}</span>
                    </div>
                  )}
                </div>
              ))}
            </div>
          </ReviewReportSection>
        )}

        {remediationItemCount > 0 && (
          <ReviewReportSection
            title={t('toolCards.codeReview.sections.remediation', { defaultValue: 'Remediation Plan' })}
            summary={t('toolCards.codeReview.sectionItemCount', {
              count: remediationItemCount,
              defaultValue: '{{count}} items',
            })}
            expanded={remediationExpanded}
            onToggle={handleToggleReportSection('remediation')}
          >
            <div className="review-remediation">
            <div className="remediation-header-row">
              <div>
                <div className="remediation-header">
                  {t('toolCards.codeReview.remediationPlan', { defaultValue: 'Remediation Plan' })}
                </div>
              </div>
            </div>
            {review_mode === 'deep' ? (
              <div className="review-remediation__groups">
                {renderReportGroupList(
                  reportSections.remediationGroups,
                  (id) => getRemediationGroupTitle(id, t),
                )}
              </div>
            ) : (
              <div className="remediation-list">
                {remediationItems.map((item) => {
                const issue = item.issue;
                const expanded = expandedRemediationIds.has(item.id);
                const location = issue?.file
                  ? `${issue.file}${issue.line ? `:${issue.line}` : ''}`
                  : null;

                return (
                  <div
                    key={item.id}
                    className="remediation-item"
                  >
                    <div className="remediation-item__topline">
                      <span className="remediation-item__label">
                        <span className="remediation-index">{item.index + 1}</span>
                        <span>{item.plan}</span>
                      </span>
                      <button
                        type="button"
                        className="remediation-item__expand"
                        onClick={(event) => {
                          event.stopPropagation();
                          handleToggleRemediationDetails(item.id);
                        }}
                        aria-expanded={expanded}
                      >
                        {expanded ? <ChevronUp size={13} /> : <ChevronDown size={13} />}
                        <span>
                          {expanded
                            ? t('toolCards.codeReview.remediationActions.collapsePlan', {
                                defaultValue: 'Collapse',
                              })
                            : t('toolCards.codeReview.remediationActions.expandPlan', {
                                defaultValue: 'Details',
                              })}
                        </span>
                      </button>
                    </div>
                    {expanded && (
                      <div className="remediation-item__details">
                        {issue ? (
                          <>
                            <div className="remediation-detail-row">
                              <span>{t('toolCards.codeReview.remediationActions.relatedIssue', { defaultValue: 'Related issue' })}</span>
                              <strong>{issue.title}</strong>
                            </div>
                            <div className="remediation-detail-grid">
                              {issue.severity && (
                                <div>
                                  <span>{t('toolCards.codeReview.remediationActions.severity', { defaultValue: 'Severity' })}</span>
                                  <strong>{t(`toolCards.codeReview.severities.${issue.severity}`, { defaultValue: issue.severity })}</strong>
                                </div>
                              )}
                              {issue.certainty && (
                                <div>
                                  <span>{t('toolCards.codeReview.remediationActions.certainty', { defaultValue: 'Certainty' })}</span>
                                  <strong>{t(`toolCards.codeReview.certainties.${issue.certainty}`, { defaultValue: issue.certainty })}</strong>
                                </div>
                              )}
                              {location && (
                                <div>
                                  <span>{t('toolCards.codeReview.remediationActions.location', { defaultValue: 'Location' })}</span>
                                  <strong>{location}</strong>
                                </div>
                              )}
                            </div>
                            {issue.description && (
                              <p>{issue.description}</p>
                            )}
                            {issue.suggestion && (
                              <p className="remediation-item__suggestion">
                                <span>{t('toolCards.codeReview.suggestion')}:</span>
                                {issue.suggestion}
                              </p>
                            )}
                            {issue.validation_note && (
                              <p className="remediation-item__validation">{issue.validation_note}</p>
                            )}
                          </>
                        ) : (
                          <p>
                            {t('toolCards.codeReview.remediationActions.noRelatedIssue', {
                              defaultValue: 'No directly-linked issue was provided for this remediation item. Use the plan text itself as the implementation scope.',
                            })}
                          </p>
                        )}
                      </div>
                    )}
                  </div>
                );
              })}
              </div>
            )}
            {/* Review remediation actions are rendered as the shared floating bar at
                the bottom of the BtwSessionPanel. */}
            </div>
          </ReviewReportSection>
        )}

        {strengthItemCount > 0 && (
          <ReviewReportSection
            title={t('toolCards.codeReview.sections.strengths', { defaultValue: 'Code Strengths' })}
            summary={t('toolCards.codeReview.sectionItemCount', {
              count: strengthItemCount,
              defaultValue: '{{count}} items',
            })}
            expanded={strengthsExpanded}
            onToggle={handleToggleReportSection('strengths')}
          >
            <div className="review-positive">
              {renderReportGroupList(
                reportSections.strengthGroups,
                (id) => getStrengthGroupTitle(id, t),
              )}
            </div>
          </ReviewReportSection>
        )}

        {reviewers.length > 0 && (
          <ReviewReportSection
            title={t('toolCards.codeReview.reviewerTeam', { defaultValue: 'Code Review Team' })}
            summary={formatReviewerStats(reportSections.reviewerStats, t)}
            expanded={teamExpanded}
            onToggle={handleToggleReportSection('team')}
          >
            <div className="team-list">
              {reviewers.map((reviewer: CodeReviewReviewer, index: number) => (
                <div key={`${reviewer.name}-${index}`} className="reviewer-item">
                  <div className="reviewer-topline">
                    <div className="reviewer-identity">
                      <span className="reviewer-name">{reviewer.name}</span>
                      <span className="reviewer-specialty">{reviewer.specialty}</span>
                    </div>
                    <div className="reviewer-metrics">
                      <span className="reviewer-status">{reviewer.status}</span>
                      <span className="reviewer-issues">
                        {typeof reviewer.issue_count === 'number'
                          ? t('toolCards.codeReview.reviewerIssues', {
                              count: reviewer.issue_count,
                              defaultValue: '{{count}} issues',
                            })
                          : t('toolCards.codeReview.reviewerIssuesUnknown', {
                              defaultValue: 'Issue count unavailable',
                            })}
                      </span>
                    </div>
                  </div>
                  <div className="reviewer-summary">{reviewer.summary}</div>
                </div>
              ))}
            </div>
          </ReviewReportSection>
        )}

        {reportSections.coverageNotes.length > 0 && (
          <ReviewReportSection
            title={t('toolCards.codeReview.sections.coverage', { defaultValue: 'Coverage Notes' })}
            summary={t('toolCards.codeReview.sectionItemCount', {
              count: reportSections.coverageNotes.length,
              defaultValue: '{{count}} items',
            })}
            expanded={coverageExpanded}
            onToggle={handleToggleReportSection('coverage')}
          >
            <ul className="review-report-group__list">
              {reportSections.coverageNotes.map((note, index) => (
                <li key={index}>{note}</li>
              ))}
            </ul>
          </ReviewReportSection>
        )}
      </div>
    );
  }, [
    expandedRemediationIds,
    expandedReportSectionIds,
    handleToggleRemediationDetails,
    handleToggleReportSection,
    remediationItems,
    reviewData,
    t,
  ]);

  const normalizedStatus = status === 'analyzing' ? 'running' : status;

  return (
    <div ref={cardRootRef} data-tool-card-id={toolId ?? ''}>
      <BaseToolCard
        status={normalizedStatus as 'pending' | 'preparing' | 'streaming' | 'running' | 'completed' | 'error' | 'cancelled'}
        isExpanded={isExpanded}
        onClick={handleCardClick}
        className="code-review-card"
        header={renderHeader()}
        expandedContent={expandedContent ?? undefined}
      />
    </div>
  );
});
