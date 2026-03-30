/**
 * CodeReview tool display component
 * Displays structured code review results with collapsible/expandable details
 * Refactored based on BaseToolCard
 */

import React, { useState, useMemo, useCallback } from 'react';
import { Loader2, CheckCircle, AlertTriangle, AlertCircle, Info, Clock, ChevronDown, ChevronUp } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { Tooltip } from '@/component-library';
import type { ToolCardProps } from '../types/flow-chat';
import { BaseToolCard, ToolCardHeader } from './BaseToolCard';
import { createLogger } from '@/shared/utils/logger';
import { useToolCardHeightContract } from './useToolCardHeightContract';
import './CodeReviewToolCard.scss';

const log = createLogger('CodeReviewToolCard');

interface CodeReviewSummary {
  overall_assessment: string;
  risk_level: 'low' | 'medium' | 'high' | 'critical';
  recommended_action: 'approve' | 'approve_with_suggestions' | 'request_changes' | 'block';
  confidence_note?: string;
}

interface CodeReviewIssue {
  severity: 'critical' | 'high' | 'medium' | 'low' | 'info';
  certainty: 'confirmed' | 'likely' | 'possible';
  category: string;
  file: string;
  line: number | null;
  title: string;
  description: string;
  suggestion: string | null;
}

interface CodeReviewResult {
  summary: CodeReviewSummary;
  issues: CodeReviewIssue[];
  positive_points: string[];
}

const riskLevelColors: Record<string, string> = {
  low: '#22c55e',
  medium: '#f59e0b',
  high: '#f97316',
  critical: '#ef4444'
};

export const CodeReviewToolCard: React.FC<ToolCardProps> = React.memo(({
  toolItem
}) => {
  const { t } = useTranslation('flow-chat');
  const { toolResult, status } = toolItem;
  const [isExpanded, setIsExpanded] = useState(false);
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

  const reviewData = useMemo<CodeReviewResult | null>(() => {
    if (!toolResult?.result) return null;

    try {
      const result = toolResult.result;

      if (typeof result === 'string') {
        const parsed = JSON.parse(result);
        return parsed;
      }

      if (typeof result === 'object' && result.summary) {
        return result as CodeReviewResult;
      }

      return null;
    } catch (error) {
      log.error('Failed to parse result', error);
      return null;
    }
  }, [toolResult?.result]);

  const issueStats = useMemo(() => {
    if (!reviewData) return null;

    const stats = {
      critical: 0,
      high: 0,
      medium: 0,
      low: 0,
      info: 0,
      total: 0
    };

    reviewData.issues.forEach(issue => {
      stats[issue.severity]++;
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
      case 'critical': return 'critical';
      case 'high': return 'high';
      case 'medium': return 'medium';
      case 'low': return 'low';
      case 'info': return 'info';
      default: return 'info';
    }
  };

  const hasIssues = issueStats && issueStats.total > 0;
  const hasData = reviewData !== null;

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

  const renderContent = () => {
    if (status === 'completed' && reviewData) {
      const { risk_level } = reviewData.summary;

      if (hasIssues) {
        const parts: React.ReactNode[] = [];
        if (issueStats!.critical > 0) {
          parts.push(
            <span key="critical" style={{ color: riskLevelColors.critical }}>
              {issueStats!.critical} {t('toolCards.codeReview.severities.critical')}
            </span>
          );
        }
        if (issueStats!.high > 0) {
          parts.push(
            <span key="high" style={{ color: riskLevelColors.high }}>
              {issueStats!.high} {t('toolCards.codeReview.severities.high')}
            </span>
          );
        }
        if (issueStats!.medium > 0) {
          parts.push(
            <span key="medium" style={{ color: riskLevelColors.medium }}>
              {issueStats!.medium} {t('toolCards.codeReview.severities.medium')}
            </span>
          );
        }
        if (issueStats!.low > 0) {
          parts.push(
            <span key="low" style={{ color: riskLevelColors.low }}>
              {issueStats!.low} {t('toolCards.codeReview.severities.low')}
            </span>
          );
        }

        return (
          <>
            {t('toolCards.codeReview.reviewResult')} —{' '}
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
          {t('toolCards.codeReview.reviewResult')} — {t(`toolCards.codeReview.riskLevels.${risk_level}`)}
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
        extra={
          <>
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
        }
        statusIcon={getStatusIcon()}
      />
    );
  };

  const expandedContent = useMemo(() => {
    if (!reviewData) return null;

    const { summary, issues, positive_points } = reviewData;

    return (
      <div className="code-review-details">
        {/* Summary section — vertical layout */}
        <div className="review-summary">
          <div className="summary-header">{t('toolCards.codeReview.overallAssessment')}</div>
          <div className="summary-rows">
            <div className="summary-row">
              <span className="summary-label">{t('toolCards.codeReview.riskLevel')}</span>
              <span
                className="summary-value risk-level"
                style={{ color: riskLevelColors[summary.risk_level] }}
              >
                {getSeverityIcon(summary.risk_level)}
                <span>{t(`toolCards.codeReview.riskLevels.${summary.risk_level}`)}</span>
              </span>
            </div>
            <div className="summary-row">
              <span className="summary-label">{t('toolCards.codeReview.recommendedAction')}</span>
              <span className="summary-value">{t(`toolCards.codeReview.actions.${summary.recommended_action}`)}</span>
            </div>
            <div className="summary-row summary-row--full">
              <span className="summary-label">{t('toolCards.codeReview.overallAssessment')}</span>
              <span className="summary-value">{summary.overall_assessment}</span>
            </div>
            {summary.confidence_note && (
              <div className="summary-row summary-row--full">
                <span className="summary-label">{t('toolCards.codeReview.contextLimitations')}</span>
                <span className="summary-value note">{summary.confidence_note}</span>
              </div>
            )}
          </div>
        </div>

        {/* Issues section */}
        {issues.length > 0 && (
          <div className="review-issues">
            <div className="issues-header">
              {t('toolCards.codeReview.issuesCount', { count: issues.length })}
            </div>
            <div className="issues-list">
              {issues.map((issue, index) => (
                <div
                  key={index}
                  className={`review-issue-item severity-${getSeverityClass(issue.severity)}`}
                >
                  <div className="issue-header">
                    <div className="issue-left">
                      {getSeverityIcon(issue.severity)}
                      <span className="issue-category">[{issue.category}]</span>
                      <span className="issue-location">
                        {issue.file}{issue.line ? `:${issue.line}` : ''}
                      </span>
                    </div>
                    <span className="issue-certainty">
                      {t(`toolCards.codeReview.certainties.${issue.certainty}`)}
                    </span>
                  </div>
                  <div className="issue-title">{issue.title}</div>
                  <div className="issue-description">{issue.description}</div>
                  {issue.suggestion && (
                    <div className="issue-suggestion">
                      <span className="suggestion-label">{t('toolCards.codeReview.suggestion')}:</span>
                      <span className="suggestion-text">{issue.suggestion}</span>
                    </div>
                  )}
                </div>
              ))}
            </div>
          </div>
        )}

        {/* Positive points section */}
        {positive_points.length > 0 && (
          <div className="review-positive">
            <div className="positive-header">{t('toolCards.codeReview.codeStrengths')}</div>
            <div className="positive-list">
              {positive_points.map((point, index) => (
                <div key={index} className="positive-item">
                  <CheckCircle size={12} style={{ color: '#22c55e', flexShrink: 0, marginTop: '2px' }} />
                  <span>{point}</span>
                </div>
              ))}
            </div>
          </div>
        )}
      </div>
    );
  }, [reviewData, t]);

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
