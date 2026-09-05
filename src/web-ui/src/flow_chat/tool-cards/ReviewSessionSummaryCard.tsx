import React, { useEffect, useMemo, useState } from 'react';
import { Button } from '@openbitfun/ui';
import { useTranslation } from 'react-i18next';
import type { ToolCardProps } from '../types/flow-chat';
import { ReviewSummaryToolCard } from '@openbitfun/ui/flow-chat';
import { flowChatStore } from '../store/FlowChatStore';
import { openBtwSessionInAuxPane } from '../services/btwSessionPane';
import { openMainSession } from '../services/sessionActivation';
import { snapshotAPI } from '@/infrastructure/api';
import {
  collectReviewChangedFiles,
  findLatestCodeReviewResult,
  summarizeCodeReviewResult,
} from '../utils/reviewSessionSummary';

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
    ? t('toolCards.reviewSessionSummary.deepTitle')
    : t('toolCards.reviewSessionSummary.standardTitle');
  const statusText = failed
    ? t('toolCards.reviewSessionSummary.failed')
    : running
      ? t('toolCards.reviewSessionSummary.running')
      : summary.issueCount > 0
        ? t('toolCards.reviewSessionSummary.issueCount', {
            count: summary.issueCount,
          })
        : t('toolCards.reviewSessionSummary.noIssues');

  const status = failed ? 'error' : running ? 'running' : 'completed';
  const summaryText = summary.summaryText || (running
    ? t('toolCards.reviewSessionSummary.waitingSummary')
    : t('toolCards.reviewSessionSummary.emptySummary'));

  return (
    <ReviewSummaryToolCard
      status={status}
      isExpanded={isExpanded}
      onToggle={() => setIsExpanded((current) => !current)}
      kind={kind === 'deep_review' ? 'deep-review' : 'review'}
      loading={running}
      title={`${reviewLabel}: ${statusText}`}
      summary={summaryText}
      changedFiles={changedFiles}
      fileCountLabel={changedFiles.length > 0 ? t('toolCards.reviewSessionSummary.filesChanged', {
        count: changedFiles.length,
      }) : undefined}
      filesLabel={t('toolCards.reviewSessionSummary.changedFilesTitle')}
      action={(
        <Button
          type="button"
          variant="fill"
          size="sm"
          onClick={async () => {
            if (!childSessionId || !parentSessionId) return;
            await openMainSession(parentSessionId);
            openBtwSessionInAuxPane({
              childSessionId,
              parentSessionId,
            });
          }}
        >
          {t('toolCards.reviewSessionSummary.openReview')}
        </Button>
      )}
    />
  );
});

ReviewSessionSummaryCard.displayName = 'ReviewSessionSummaryCard';
