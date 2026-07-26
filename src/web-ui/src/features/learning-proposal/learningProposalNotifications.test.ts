import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { LearningProposal } from '@/infrastructure/api/service-api/LearningProposalAPI';

const {
  dismissMock,
  openReviewMock,
  persistentMock,
  updateMock,
} = vi.hoisted(() => ({
  dismissMock: vi.fn(),
  openReviewMock: vi.fn(),
  persistentMock: vi.fn(() => 'notification-1'),
  updateMock: vi.fn(),
}));

vi.mock('@/shared/notification-system', () => ({
  notificationService: {
    persistent: persistentMock,
    update: updateMock,
    dismiss: dismissMock,
  },
}));

vi.mock('./learningProposalReviewStore', () => ({
  openLearningProposalReview: openReviewMock,
}));

import {
  resolveLearningProposalNotification,
  showLearningProposalNotification,
} from './learningProposalNotifications';

const t = (key: string) => key;

function proposal(status: LearningProposal['status']): LearningProposal {
  return {
    schemaVersion: 1,
    proposalId: 'proposal-notification-test',
    status,
    source: {
      sessionId: 'session-1',
      workspacePath: 'C:\\workspace',
      selectedText: 'Important correction',
      turnId: 'turn-1',
      sourceKind: 'assistant_text',
    },
    target: {
      kind: 'memory',
      applyMode: 'memory_note',
      displayName: 'Workspace memory',
    },
    createdAt: 1,
    updatedAt: status === 'analyzing' ? 1 : 2,
  };
}

describe('learning proposal notifications', () => {
  beforeEach(() => {
    dismissMock.mockClear();
    openReviewMock.mockClear();
    persistentMock.mockClear();
    updateMock.mockClear();
    persistentMock.mockReturnValue('notification-1');
    resolveLearningProposalNotification('proposal-notification-test');
    dismissMock.mockClear();
    updateMock.mockClear();
  });

  it('updates one persistent entry, opens the latest proposal from history, and clears resolved ids', () => {
    showLearningProposalNotification(proposal('analyzing'), t);
    showLearningProposalNotification(proposal('ready'), t);

    expect(persistentMock).toHaveBeenCalledTimes(1);
    expect(updateMock).toHaveBeenCalledTimes(1);
    const latestUpdate = updateMock.mock.calls[0][1] as {
      metadata: { onClick: () => void };
    };
    latestUpdate.metadata.onClick();
    expect(openReviewMock).toHaveBeenCalledWith(expect.objectContaining({
      proposalId: 'proposal-notification-test',
      notificationId: 'notification-1',
      initialProposal: expect.objectContaining({ status: 'ready' }),
    }));

    resolveLearningProposalNotification('proposal-notification-test');
    expect(dismissMock).toHaveBeenCalledWith('notification-1');

    showLearningProposalNotification(proposal('ready'), t);
    expect(persistentMock).toHaveBeenCalledTimes(2);
  });
});
