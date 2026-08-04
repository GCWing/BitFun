import { beforeEach, describe, expect, it, vi } from 'vitest';

const getAccessState = vi.fn();
const listFeedbackRecords = vi.fn();

vi.mock('@/infrastructure/api', () => ({
  feedbackAPI: { getAccessState, listFeedbackRecords },
  normalizeFeedbackError: (error: unknown) => error,
}));

describe('feedbackInboxStore', () => {
  beforeEach(async () => {
    vi.resetModules();
    getAccessState.mockReset();
    listFeedbackRecords.mockReset();
  });

  it('does not inspect or query access in privacy-not-accepted mode', async () => {
    const { useFeedbackInboxStore } = await import('./feedbackInboxStore');
    await useFeedbackInboxStore.getState().initializeForMode('privacy_not_accepted');
    expect(getAccessState).not.toHaveBeenCalled();
    expect(listFeedbackRecords).not.toHaveBeenCalled();
  });

  it('checks once but does not enroll or query when there is no history', async () => {
    getAccessState.mockResolvedValue({
      hasHistory: false,
      canReuseAccess: false,
      cachedInbox: { items: [], hasMore: false },
    });
    const { useFeedbackInboxStore } = await import('./feedbackInboxStore');
    await useFeedbackInboxStore.getState().initializeForMode('full');
    await useFeedbackInboxStore.getState().initializeForMode('full');
    expect(getAccessState).toHaveBeenCalledTimes(1);
    expect(listFeedbackRecords).not.toHaveBeenCalled();
  });

  it('preserves cached records when an active refresh fails', async () => {
    const cached = {
      feedbackId: 'feedback-1',
      category: 'other',
      status: 'waiting_user',
      hasNewReply: true,
      createdAt: '2026-07-28T01:00:00Z',
      updatedAt: '2026-07-28T02:00:00Z',
      canOpen: true,
    };
    getAccessState.mockResolvedValue({
      hasHistory: true,
      canReuseAccess: true,
      cachedInbox: { items: [cached], nextCursor: 'cached-cursor', hasMore: true },
    });
    listFeedbackRecords.mockRejectedValue({ code: 'NETWORK_ERROR' });
    const { useFeedbackInboxStore } = await import('./feedbackInboxStore');

    expect(await useFeedbackInboxStore.getState().refresh(true)).toBe(false);
    expect(useFeedbackInboxStore.getState().records).toEqual([cached]);
    expect(useFeedbackInboxStore.getState().nextCursor).toBe('cached-cursor');
  });

  it('performs one startup Inbox query when full mode has reusable history', async () => {
    getAccessState.mockResolvedValue({
      hasHistory: true,
      canReuseAccess: true,
      cachedInbox: { items: [], hasMore: false },
    });
    listFeedbackRecords.mockResolvedValue({ items: [], hasMore: false });
    const { useFeedbackInboxStore } = await import('./feedbackInboxStore');

    await useFeedbackInboxStore.getState().initializeForMode('full');
    await useFeedbackInboxStore.getState().initializeForMode('full');

    expect(listFeedbackRecords).toHaveBeenCalledTimes(1);
    expect(listFeedbackRecords).toHaveBeenCalledWith({}, { userInitiated: false });
  });

  it('clears the unread marker when a conversation result is committed', async () => {
    const record = {
      feedbackId: 'feedback-1',
      category: 'other' as const,
      status: 'waiting_user' as const,
      hasNewReply: true,
      createdAt: '2026-07-28T01:00:00Z',
      updatedAt: '2026-07-28T02:00:00Z',
      canOpen: true,
    };
    const { useFeedbackInboxStore } = await import('./feedbackInboxStore');
    useFeedbackInboxStore.setState({ records: [record] });

    useFeedbackInboxStore.getState().applyServerStatus('feedback-1', 'in_progress');

    expect(useFeedbackInboxStore.getState().records[0]).toMatchObject({
      status: 'in_progress',
      hasNewReply: false,
    });
  });

  it('does not surface an unread reply that cannot be opened or acknowledged', async () => {
    const record = {
      feedbackId: 'feedback-1',
      category: 'other' as const,
      status: 'waiting_user' as const,
      hasNewReply: true,
      createdAt: '2026-07-28T01:00:00Z',
      updatedAt: '2026-07-28T02:00:00Z',
      canOpen: true,
    };
    const { hasActionableUnreadReply, useFeedbackInboxStore } = await import('./feedbackInboxStore');
    useFeedbackInboxStore.setState({ records: [record] });

    expect(hasActionableUnreadReply(record)).toBe(true);

    useFeedbackInboxStore.getState().markInaccessible('feedback-1');

    expect(hasActionableUnreadReply(useFeedbackInboxStore.getState().records[0])).toBe(false);
    expect(useFeedbackInboxStore.getState().records[0].hasNewReply).toBe(true);
  });
});
