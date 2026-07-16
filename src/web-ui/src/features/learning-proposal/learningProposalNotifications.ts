import type { LearningProposal } from '@/infrastructure/api/service-api/LearningProposalAPI';
import { notificationService } from '@/shared/notification-system';
import { openLearningProposalReview } from './learningProposalReviewStore';

type Translate = (key: string, options?: Record<string, unknown>) => string;

const notificationIdsByProposal = new Map<string, string>();

function notificationCopy(proposal: LearningProposal, t: Translate): {
  type: 'info' | 'warning' | 'error';
  title: string;
  message: string;
} | null {
  const target = proposal.target?.displayName || t('learningProposal.target.pending');
  switch (proposal.status) {
    case 'analyzing':
      return {
        type: 'info',
        title: t('learningProposal.notification.analyzingTitle'),
        message: t('learningProposal.notification.analyzingMessage'),
      };
    case 'ready':
      return {
        type: 'info',
        title: t('learningProposal.notification.readyTitle'),
        message: t('learningProposal.notification.readyMessage', { target }),
      };
    case 'stale':
      return {
        type: 'warning',
        title: t('learningProposal.notification.staleTitle'),
        message: t('learningProposal.notification.staleMessage', { target }),
      };
    case 'failed':
      return {
        type: 'error',
        title: t('learningProposal.notification.failedTitle'),
        message: t('learningProposal.notification.failedMessage'),
      };
    default:
      return null;
  }
}

export function showLearningProposalNotification(
  proposal: LearningProposal,
  t: Translate,
): string | null {
  const copy = notificationCopy(proposal, t);
  if (!copy) {
    return null;
  }

  const existingId = notificationIdsByProposal.get(proposal.proposalId);
  const notificationId = existingId || '';
  const openReview = () => openLearningProposalReview({
    proposalId: proposal.proposalId,
    initialProposal: proposal,
    notificationId: existingId || notificationId,
  });
  const updates = {
    ...copy,
    actions: [{
      label: t('learningProposal.actions.review'),
      variant: 'primary' as const,
      onClick: openReview,
    }],
    metadata: {
      source: 'learning-proposal',
      proposalId: proposal.proposalId,
      status: proposal.status,
      onClick: openReview,
    },
  };
  if (existingId) {
    notificationService.update(existingId, updates);
    return existingId;
  }

  let createdNotificationId = '';
  const openCreatedReview = () => openLearningProposalReview({
    proposalId: proposal.proposalId,
    initialProposal: proposal,
    notificationId: createdNotificationId,
  });
  createdNotificationId = notificationService.persistent({
    ...copy,
    actions: [{
      label: t('learningProposal.actions.review'),
      variant: 'primary',
      onClick: openCreatedReview,
    }],
    metadata: {
      source: 'learning-proposal',
      proposalId: proposal.proposalId,
      status: proposal.status,
      onClick: openCreatedReview,
    },
  });
  notificationIdsByProposal.set(proposal.proposalId, createdNotificationId);
  return createdNotificationId;
}

export function resolveLearningProposalNotification(
  proposalId: string,
  notificationId?: string,
): void {
  const id = notificationId || notificationIdsByProposal.get(proposalId);
  if (!id) {
    return;
  }
  notificationService.update(id, { actions: [] });
  notificationService.dismiss(id);
  notificationIdsByProposal.delete(proposalId);
}
