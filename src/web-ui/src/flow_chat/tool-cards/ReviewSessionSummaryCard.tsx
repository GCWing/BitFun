import React, { useEffect, useMemo, useState } from 'react';
import { ChevronDown, ChevronUp, FileText, Loader2, SearchCheck, Sparkles } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import type { ToolCardProps } from '../types/flow-chat';
import { BaseToolCard, ToolCardHeader } from './BaseToolCard';
import { flowChatStore } from '../store/FlowChatStore';
import { openBtwSessionInAuxPane, openMainSession } from '../services/openBtwSession';
import { snapshotAPI } from '@/infrastructure/api';
import {
  collectReviewChangedFiles,
  findLatestCodeReviewResult,
  summarizeCodeReviewResult,
} from '../utils/reviewSessionSummary';
import './ReviewSessionSummaryCard.scss';

interface ReviewSessionSummaryInput {
  childSessionId?: string;
  parentSessionId?: string;
  kind?: 'review' | 'deep_review';
  title?: string;
  requestedFiles?: string[];
}

function isReviewRunning(status?: string): boolean {
  return status === 'pending' ||
    status === 'image_analyzing' ||
    status === 'processing' ||
    status === 'finishing';
}

export const ReviewSessionSummaryCard: React.FC<ToolCardProps> = React.memo(({
  toolItem,
  sessionId,
}) => {
  const { t } = useTranslation('flow-chat');
  const [isExpanded, setIsExpanded] = useState(false);
  const [flowState, setFlowState] = useState(() => flowChatStore.getState());
  const [snapshotFiles, setSnapshotFiles] = useState<string[]>([]);

  const input = (toolItem.toolCall?.input || {}) as ReviewSessionSummaryInput;
  const childSessionId = input.childSessionId ?? '';
  const parentSessionId = input.parentSessionId || sessionId || '';
  const kind = input.kind === 'deep_review' ? 'deep_review' : 'review';
  const childSession = childSessionId ? flowState.sessions.get(childSessionId) : undefined;
  const reviewResult = useMemo(() => findLatestCodeReviewResult(childSession), [childSession]);
  const summary = useMemo(() => summarizeCodeReviewResult(reviewResult), [reviewResult]);
  const childTurn = childSession?.dialogTurns[childSession.dialogTurns.length - 1];
  const running = !reviewResult && isReviewRunning(childTurn?.status);
  const failed = Boolean(childSession?.error || childTurn?.status === 'error');
  const changedFiles = useMemo(() => collectReviewChangedFiles({
    snapshotFiles,
    reviewResult,
    requestedFiles: input.requestedFiles ?? [],
  }), [input.requestedFiles, reviewResult, snapshotFiles]);

  useEffect(() => flowChatStore.subscribe(setFlowState), []);

  useEffect(() => {
    let cancelled = false;
    if (!childSessionId) {
      setSnapshotFiles([]);
      return;
    }

    snapshotAPI.getSessionFiles(childSessionId)
      .then((files) => {
        if (!cancelled) {
          setSnapshotFiles(files);
        }
      })
      .catch(() => {
        if (!cancelled) {
          setSnapshotFiles([]);
        }
      });

    return () => {
      cancelled = true;
    };
  }, [childSessionId, childSession?.lastActiveAt, childSession?.lastFinishedAt]);

  const reviewLabel = kind === 'deep_review'
    ? t('toolCards.reviewSessionSummary.deepTitle', { defaultValue: 'Deep review' })
    : t('toolCards.reviewSessionSummary.standardTitle', { defaultValue: 'Review' });
  const statusText = failed
    ? t('toolCards.reviewSessionSummary.failed', { defaultValue: 'failed' })
    : running
      ? t('toolCards.reviewSessionSummary.running', { defaultValue: 'in progress' })
      : summary.issueCount > 0
        ? t('toolCards.reviewSessionSummary.issueCount', {
            count: summary.issueCount,
            defaultValue: '{{count}} issues',
          })
        : t('toolCards.reviewSessionSummary.noIssues', { defaultValue: 'no blocking issues' });

  const status = failed ? 'error' : running ? 'running' : 'completed';
  const Icon = kind === 'deep_review' ? Sparkles : SearchCheck;

  return (
    <BaseToolCard
      status={status}
      isExpanded={isExpanded}
      onClick={() => setIsExpanded((current) => !current)}
      className="review-session-summary-card"
      header={(
        <ToolCardHeader
          icon={<Icon size={13} />}
          iconClassName="review-session-summary-card__icon"
          content={(
            <span>
              {reviewLabel}: {statusText}
            </span>
          )}
          extra={changedFiles.length > 0 ? (
            <span className="review-session-summary-card__file-count">
              <FileText size={12} />
              {t('toolCards.reviewSessionSummary.filesChanged', {
                count: changedFiles.length,
                defaultValue: '{{count}} files',
              })}
            </span>
          ) : null}
          statusIcon={running ? <Loader2 className="animate-spin" size={12} /> : (
            isExpanded ? <ChevronUp size={13} /> : <ChevronDown size={13} />
          )}
        />
      )}
      expandedContent={(
        <div className="review-session-summary-card__details" onClick={(event) => event.stopPropagation()}>
          {summary.summaryText ? (
            <p className="review-session-summary-card__summary">{summary.summaryText}</p>
          ) : (
            <p className="review-session-summary-card__summary">
              {running
                ? t('toolCards.reviewSessionSummary.waitingSummary', {
                    defaultValue: 'The review is still running. Results will appear here when the code review team finishes.',
                  })
                : t('toolCards.reviewSessionSummary.emptySummary', {
                    defaultValue: 'No structured review summary is available yet.',
                  })}
            </p>
          )}
          {changedFiles.length > 0 ? (
            <div className="review-session-summary-card__files">
              <span className="review-session-summary-card__section-label">
                {t('toolCards.reviewSessionSummary.changedFilesTitle', {
                  defaultValue: 'Files from this review',
                })}
              </span>
              <ul>
                {changedFiles.map((filePath) => (
                  <li key={filePath}>{filePath}</li>
                ))}
              </ul>
            </div>
          ) : null}
          <button
            type="button"
            className="review-session-summary-card__open"
            onClick={async () => {
              if (!childSessionId || !parentSessionId) return;
              await openMainSession(parentSessionId);
              openBtwSessionInAuxPane({
                childSessionId,
                parentSessionId,
              });
            }}
          >
            {t('toolCards.reviewSessionSummary.openReview', {
              defaultValue: 'Open review page',
            })}
          </button>
        </div>
      )}
    />
  );
});

ReviewSessionSummaryCard.displayName = 'ReviewSessionSummaryCard';
