import { beforeEach, describe, expect, it, vi } from 'vitest';

const rollbackSessionToTurn = vi.hoisted(() => vi.fn(async () => ({
  restoredFiles: [],
  fromTurnIndex: 0,
  composerText: 'original',
})));
const sessions = vi.hoisted(() => new Map<string, any>());
const historyViews = vi.hoisted(() => new Map<string, any>());

vi.mock('../store/FlowChatStore', () => ({
  flowChatStore: {
    getState: () => ({ sessions }),
    getSessionHistoryViewState: (sessionId: string) => historyViews.get(sessionId),
  },
}));
vi.mock('./SessionRollbackService', () => ({ rollbackSessionToTurn }));

import { editAndRerunUserMessage } from './UserMessageEditService';
import {
  assertSessionSubmissionAllowed,
  useSessionMutationStore,
} from '../store/sessionMutationStore';

describe('UserMessageEditService', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    sessions.clear();
    historyViews.clear();
    sessions.set('session-1', {
      sessionId: 'session-1',
      dialogTurns: [{
        id: 'turn-7',
        userMessage: { metadata: { triggerSource: 'desktop_ui' } },
      }],
    });
    useSessionMutationStore.setState({ mutations: new Map(), retiredTurnIds: new Map() });
  });

  it('holds one lease through rollback, reload, and rerun admission', async () => {
    let admitRerun!: () => void;
    const rerun = vi.fn(() => new Promise<void>(resolve => {
      admitRerun = resolve;
    }));

    const editing = editAndRerunUserMessage({
      sessionId: 'session-1',
      turnId: 'turn-7',
      originalContent: 'original',
      editedContent: 'edited',
      agentType: 'agentic',
      rerun,
    });

    await vi.waitFor(() => expect(rerun).toHaveBeenCalledTimes(1));
    const mutation = useSessionMutationStore.getState().mutations.get('session-1');
    expect(mutation).toMatchObject({ kind: 'edit-rerun', phase: 'active' });
    expect(rollbackSessionToTurn).toHaveBeenCalledWith(expect.objectContaining({
      lease: expect.objectContaining({ leaseId: mutation?.leaseId }),
    }));
    expect(rerun).toHaveBeenCalledWith('edited', 'agentic', mutation?.leaseId);
    expect(() => assertSessionSubmissionAllowed('session-1')).toThrow('history mutation');
    expect(() => assertSessionSubmissionAllowed('session-1', mutation?.leaseId)).not.toThrow();

    admitRerun();
    await editing;
    expect(useSessionMutationStore.getState().mutations.has('session-1')).toBe(false);
  });

  it('releases the lease when rollback fails before reconciliation', async () => {
    rollbackSessionToTurn.mockRejectedValueOnce(new Error('stale target'));

    await expect(editAndRerunUserMessage({
      sessionId: 'session-1',
      turnId: 'turn-7',
      originalContent: 'original',
      editedContent: 'edited',
      rerun: vi.fn(async () => undefined),
    })).rejects.toThrow('stale target');

    expect(useSessionMutationStore.getState().mutations.has('session-1')).toBe(false);
  });

  it('edits a message materialized only in a history window', async () => {
    sessions.get('session-1').dialogTurns = [];
    historyViews.set('session-1', {
      catalog: null,
      loadedRanges: [{
        startOrdinal: 0,
        endOrdinalExclusive: 1,
        turns: [{
          id: 'turn-7',
          storageTurnIndex: 7,
          userMessage: { metadata: { triggerSource: 'desktop_ui' } },
        }],
      }],
    });
    const rerun = vi.fn(async () => undefined);

    await editAndRerunUserMessage({
      sessionId: 'session-1',
      turnId: 'turn-7',
      originalContent: 'original',
      editedContent: 'edited',
      rerun,
    });

    expect(rollbackSessionToTurn).toHaveBeenCalled();
    expect(rerun).toHaveBeenCalledWith('edited', undefined, expect.any(String));
  });
});
