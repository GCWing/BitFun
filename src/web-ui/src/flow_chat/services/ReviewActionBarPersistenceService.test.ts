import { describe, expect, it, vi, beforeEach, afterEach } from 'vitest';
import {
  persistReviewActionState,
  clearPersistedReviewState,
  loadPersistedReviewState,
} from './ReviewActionBarPersistenceService';
import { sessionAPI } from '@/infrastructure/api/service-api/SessionAPI';
import { flowChatStore } from '../store/FlowChatStore';

vi.mock('@/infrastructure/api/service-api/SessionAPI', () => ({
  sessionAPI: {
    saveSessionMetadata: vi.fn().mockResolvedValue(undefined),
    loadSessionMetadata: vi.fn().mockResolvedValue(undefined),
  },
}));

vi.mock('../store/FlowChatStore', () => ({
  flowChatStore: {
    getState: vi.fn().mockReturnValue({
      sessions: new Map(),
    }),
  },
}));

describe('ReviewActionBarPersistenceService', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  describe('persistReviewActionState', () => {
    it('does nothing when childSessionId is null', async () => {
      await persistReviewActionState({
        childSessionId: null,
        parentSessionId: null,
        reviewMode: 'deep',
        phase: 'review_completed',
        reviewData: null,
        remediationItems: [],
        selectedRemediationIds: new Set(),
        dismissed: false,
        minimized: false,
        activeAction: null,
        customInstructions: '',
        errorMessage: null,
        interruption: null,
        completedRemediationIds: new Set(),
        fixingRemediationIds: new Set(),
        remainingFixIds: [],
      } as any);

      expect(sessionAPI.saveSessionMetadata).not.toHaveBeenCalled();
    });

    it('does nothing when session is not found in FlowChatStore', async () => {
      await persistReviewActionState({
        childSessionId: 'session-1',
        parentSessionId: null,
        reviewMode: 'deep',
        phase: 'review_completed',
        reviewData: null,
        remediationItems: [],
        selectedRemediationIds: new Set(),
        dismissed: false,
        minimized: false,
        activeAction: null,
        customInstructions: '',
        errorMessage: null,
        interruption: null,
        completedRemediationIds: new Set(),
        fixingRemediationIds: new Set(),
        remainingFixIds: [],
      } as any);

      expect(sessionAPI.saveSessionMetadata).not.toHaveBeenCalled();
    });

    it('saves metadata with reviewActionState when session exists', async () => {
      const mockSession = {
        workspacePath: '/workspace/project',
        remoteConnectionId: undefined,
        remoteSshHost: undefined,
      };

      (flowChatStore.getState as any).mockReturnValue({
        sessions: new Map([['session-1', mockSession]]),
      });

      await persistReviewActionState({
        childSessionId: 'session-1',
        parentSessionId: null,
        reviewMode: 'deep',
        phase: 'review_completed',
        reviewData: null,
        remediationItems: [],
        selectedRemediationIds: new Set(),
        dismissed: false,
        minimized: true,
        activeAction: null,
        customInstructions: 'custom instruction',
        errorMessage: null,
        interruption: null,
        completedRemediationIds: new Set(['remediation-0']),
        fixingRemediationIds: new Set(),
        remainingFixIds: [],
      } as any);

      expect(sessionAPI.saveSessionMetadata).toHaveBeenCalledTimes(1);
      const [metadata, workspacePath] = (sessionAPI.saveSessionMetadata as any).mock.calls[0];
      expect(metadata.sessionId).toBe('session-1');
      expect(metadata.reviewActionState).toEqual({
        version: 1,
        phase: 'review_completed',
        completedRemediationIds: ['remediation-0'],
        minimized: true,
        customInstructions: 'custom instruction',
        persistedAt: expect.any(Number),
      });
      expect(workspacePath).toBe('/workspace/project');
    });

    it('passes remote connection info when available', async () => {
      const mockSession = {
        workspacePath: '/workspace/project',
        remoteConnectionId: 'remote-1',
        remoteSshHost: 'ssh-host-1',
      };

      (flowChatStore.getState as any).mockReturnValue({
        sessions: new Map([['session-1', mockSession]]),
      });

      await persistReviewActionState({
        childSessionId: 'session-1',
        parentSessionId: null,
        reviewMode: 'deep',
        phase: 'fix_running',
        reviewData: null,
        remediationItems: [],
        selectedRemediationIds: new Set(),
        dismissed: false,
        minimized: false,
        activeAction: null,
        customInstructions: '',
        errorMessage: null,
        interruption: null,
        completedRemediationIds: new Set(),
        fixingRemediationIds: new Set(),
        remainingFixIds: [],
      } as any);

      expect(sessionAPI.saveSessionMetadata).toHaveBeenCalledTimes(1);
      const [, , remoteConnectionId, remoteSshHost] = (sessionAPI.saveSessionMetadata as any).mock.calls[0];
      expect(remoteConnectionId).toBe('remote-1');
      expect(remoteSshHost).toBe('ssh-host-1');
    });
  });

  describe('clearPersistedReviewState', () => {
    it('saves metadata with undefined reviewActionState', async () => {
      await clearPersistedReviewState('session-1', '/workspace/project');

      expect(sessionAPI.saveSessionMetadata).toHaveBeenCalledTimes(1);
      const [metadata] = (sessionAPI.saveSessionMetadata as any).mock.calls[0];
      expect(metadata.sessionId).toBe('session-1');
      expect(metadata.reviewActionState).toBeUndefined();
    });
  });

  describe('loadPersistedReviewState', () => {
    it('returns null when no metadata exists', async () => {
      (sessionAPI.loadSessionMetadata as any).mockResolvedValue(undefined);

      const result = await loadPersistedReviewState('session-1', '/workspace/project');
      expect(result).toBeNull();
    });

    it('returns null when metadata has no reviewActionState', async () => {
      (sessionAPI.loadSessionMetadata as any).mockResolvedValue({
        sessionId: 'session-1',
        title: 'Test Session',
      });

      const result = await loadPersistedReviewState('session-1', '/workspace/project');
      expect(result).toBeNull();
    });

    it('returns persisted state when metadata has reviewActionState', async () => {
      const persistedState = {
        version: 1,
        phase: 'fix_running',
        completedRemediationIds: ['remediation-0'],
        minimized: false,
        customInstructions: 'test instruction',
        persistedAt: Date.now(),
      };

      (sessionAPI.loadSessionMetadata as any).mockResolvedValue({
        sessionId: 'session-1',
        reviewActionState: persistedState,
      });

      const result = await loadPersistedReviewState('session-1', '/workspace/project');
      expect(result).toEqual(persistedState);
    });

    it('passes remote connection info when loading', async () => {
      (sessionAPI.loadSessionMetadata as any).mockResolvedValue(undefined);

      await loadPersistedReviewState('session-1', '/workspace/project', 'remote-1', 'ssh-host-1');

      expect(sessionAPI.loadSessionMetadata).toHaveBeenCalledWith(
        'session-1',
        '/workspace/project',
        'remote-1',
        'ssh-host-1',
      );
    });

    it('returns null and does not throw on error', async () => {
      (sessionAPI.loadSessionMetadata as any).mockRejectedValue(new Error('Network error'));

      const result = await loadPersistedReviewState('session-1', '/workspace/project');
      expect(result).toBeNull();
    });
  });
});
