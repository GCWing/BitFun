import { useCallback, useEffect, useRef, useState } from 'react';
import { useI18n } from '@/infrastructure/i18n';
import {
  learningProposalAPI,
  type LearningProposal,
} from '@/infrastructure/api/service-api/LearningProposalAPI';
import { notificationService } from '@/shared/notification-system';
import { createLogger } from '@/shared/utils/logger';
import { LearningProposalReviewDialog, type LearningProposalReviewBusyAction } from './LearningProposalReviewDialog';
import {
  resolveLearningProposalNotification,
  showLearningProposalNotification,
} from './learningProposalNotifications';
import { useLearningProposalReviewStore } from './learningProposalReviewStore';
import {
  canApplyLearningProposal,
  learningProposalRequest,
} from './learningProposalUtils';

const log = createLogger('LearningProposalReviewHost');

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

export function LearningProposalReviewHost() {
  const { t } = useI18n('flow-chat');
  const request = useLearningProposalReviewStore(state => state.request);
  const close = useLearningProposalReviewStore(state => state.close);
  const [proposal, setProposal] = useState<LearningProposal | null>(null);
  const [busyAction, setBusyAction] = useState<LearningProposalReviewBusyAction>(null);
  const [clientError, setClientError] = useState<string | undefined>();
  const restoredRef = useRef(false);

  useEffect(() => {
    if (restoredRef.current) {
      return;
    }
    restoredRef.current = true;
    void learningProposalAPI.list({ includeResolved: false })
      .then((proposals) => {
        proposals
          .filter(item => (
            item.status === 'analyzing'
            || item.status === 'ready'
            || item.status === 'stale'
            || item.status === 'failed'
          ))
          .forEach(item => showLearningProposalNotification(item, t));
      })
      .catch((error) => {
        log.warn('Failed to restore unresolved learning proposals', { error: errorMessage(error) });
      });
  }, [t]);

  useEffect(() => {
    if (!request) {
      setProposal(null);
      setBusyAction(null);
      setClientError(undefined);
      return;
    }

    let cancelled = false;
    setProposal(request.initialProposal ?? null);
    setClientError(undefined);
    setBusyAction('loading');
    const getRequest = request.initialProposal
      ? learningProposalRequest(request.initialProposal)
      : { proposalId: request.proposalId };

    void learningProposalAPI.get(getRequest)
      .then((latest) => {
        if (cancelled) {
          return;
        }
        setProposal(latest);
        if (latest.status === 'applied' || latest.status === 'rejected') {
          resolveLearningProposalNotification(latest.proposalId, request.notificationId);
        } else {
          showLearningProposalNotification(latest, t);
        }
      })
      .catch((error) => {
        if (!cancelled) {
          setClientError(errorMessage(error));
        }
      })
      .finally(() => {
        if (!cancelled) {
          setBusyAction(null);
        }
      });

    return () => {
      cancelled = true;
    };
  }, [request, t]);

  const runAction = useCallback(async (
    action: Exclude<LearningProposalReviewBusyAction, 'loading' | null>,
    execute: (current: LearningProposal) => Promise<LearningProposal>,
  ) => {
    if (!proposal || busyAction !== null) {
      return;
    }
    setBusyAction(action);
    setClientError(undefined);
    try {
      const latest = await execute(proposal);
      setProposal(latest);
      if (latest.status === 'applied') {
        resolveLearningProposalNotification(latest.proposalId, request?.notificationId);
        notificationService.success(t('learningProposal.notification.appliedMessage'));
      } else if (latest.status === 'rejected') {
        resolveLearningProposalNotification(latest.proposalId, request?.notificationId);
        notificationService.success(t('learningProposal.notification.rejectedMessage'));
        close();
      } else {
        showLearningProposalNotification(latest, t);
      }
    } catch (error) {
      setClientError(errorMessage(error));
    } finally {
      setBusyAction(null);
    }
  }, [busyAction, close, proposal, request?.notificationId, t]);

  const handleRefresh = useCallback(() => {
    void runAction('refreshing', current => (
      learningProposalAPI.refresh(learningProposalRequest(current))
    ));
  }, [runAction]);

  const handleApprove = useCallback(() => {
    if (!proposal || !canApplyLearningProposal(proposal) || !proposal.baseHash || !proposal.diffHash) {
      return;
    }
    void runAction('approving', current => learningProposalAPI.approve({
      ...learningProposalRequest(current),
      baseHash: proposal.baseHash!,
      diffHash: proposal.diffHash!,
    }));
  }, [proposal, runAction]);

  const handleReject = useCallback(() => {
    void runAction('rejecting', current => (
      learningProposalAPI.reject(learningProposalRequest(current))
    ));
  }, [runAction]);

  if (!request) {
    return null;
  }

  return (
    <LearningProposalReviewDialog
      proposal={proposal}
      busyAction={busyAction}
      clientError={clientError}
      onClose={close}
      onRefresh={handleRefresh}
      onApprove={handleApprove}
      onReject={handleReject}
    />
  );
}
