import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { Session } from '@/flow_chat/types/flow-chat';
import { SnapshotStateManager } from './SnapshotStateManager';
import { SnapshotEventBus, SNAPSHOT_EVENTS } from './SnapshotEventBus';

const mocks = vi.hoisted(() => ({
  surfaceEpoch: 0,
  sessions: new Map<string, Session>(),
  getSessionStats: vi.fn(async () => ({ total_changes: 0 })),
  getSessionFiles: vi.fn(async () => []),
  getOperationDiff: vi.fn(async () => ({ originalCode: 'old', modifiedCode: 'new' })),
}));

vi.mock('@/flow_chat/store/FlowChatStore', () => ({
  flowChatStore: { getState: () => ({ sessions: mocks.sessions }) },
}));
vi.mock('../services/SnapshotSystemService', () => ({
  SnapshotSystemService: { getInstance: () => mocks },
}));
vi.mock('@/infrastructure/api', () => ({ snapshotAPI: mocks }));
vi.mock('@/infrastructure/event-bus', () => ({ globalEventBus: { on: vi.fn() } }));
vi.mock('@/infrastructure/peer-device/deviceSurface', () => ({
  getActiveSurfaceScope: () => {
    const epoch = mocks.surfaceEpoch;
    return { isCurrent: () => epoch === mocks.surfaceEpoch };
  },
}));

describe('snapshot refresh target routing', () => {
  beforeEach(() => {
    mocks.sessions.clear();
    mocks.surfaceEpoch = 0;
    vi.clearAllMocks();
  });

  it('does not read snapshots for a disconnected remote session, including completion events', async () => {
    mocks.sessions.set('ssh', {
      remoteConnectionId: 'disconnected-host', historyState: 'ready',
    } as Session);
    const manager = SnapshotStateManager.getInstance();
    await manager.refreshSessionState('ssh');
    await manager.refreshFileState('ssh', '/workspace/shared.ts');
    SnapshotEventBus.getInstance().emit(
      SNAPSHOT_EVENTS.FILE_OPERATION_COMPLETED, {}, 'ssh', '/workspace/shared.ts',
    );
    expect(mocks.getSessionStats).not.toHaveBeenCalled();
    expect(mocks.getSessionFiles).not.toHaveBeenCalled();
    expect(mocks.getOperationDiff).not.toHaveBeenCalled();
    expect(manager.getSessionState('ssh')).toBeNull();
  });

  it('still reads local session snapshots on the owning surface', async () => {
    mocks.sessions.set('local', { config: {}, historyState: 'ready' } as Session);
    const manager = SnapshotStateManager.getInstance();
    await manager.refreshSessionState('local');
    await manager.refreshFileState('local', '/workspace/shared.ts');
    expect(mocks.getSessionStats).toHaveBeenCalledWith('local');
    expect(mocks.getSessionFiles).toHaveBeenCalledWith('local');
    expect(mocks.getOperationDiff).toHaveBeenCalledWith('local', '/workspace/shared.ts');
    expect(manager.getSessionState('local')?.files.get('/workspace/shared.ts')?.modifiedContent).toBe('new');
  });

  it.each(['remote binding', 'device surface'])('stops an in-flight refresh after its %s changes', async (change) => {
    const sessionId = `pending-${change}`;
    mocks.sessions.set(sessionId, { config: {}, historyState: 'ready' } as Session);
    let finishStats!: (stats: { total_changes: number }) => void;
    mocks.getSessionStats.mockImplementationOnce(() => new Promise(resolve => { finishStats = resolve; }));
    const manager = SnapshotStateManager.getInstance();
    const pending = manager.refreshSessionState(sessionId);
    if (change === 'remote binding') {
      mocks.sessions.set(sessionId, { remoteConnectionId: 'ssh', historyState: 'ready' } as Session);
    } else {
      mocks.surfaceEpoch += 1;
    }
    finishStats({ total_changes: 1 });
    await pending;
    expect(mocks.getSessionFiles).not.toHaveBeenCalled();
    expect(manager.getSessionState(sessionId)).toBeNull();
  });

  it('does not cache an operation diff delivered after a device switch', async () => {
    mocks.sessions.set('pending-diff', { config: {}, historyState: 'ready' } as Session);
    let finishDiff!: (diff: { originalCode: string; modifiedCode: string }) => void;
    mocks.getOperationDiff.mockImplementationOnce(() => new Promise(resolve => { finishDiff = resolve; }));
    const manager = SnapshotStateManager.getInstance();
    const pending = manager.refreshFileState('pending-diff', '/workspace/pending.ts');
    mocks.surfaceEpoch += 1;
    finishDiff({ originalCode: 'old device', modifiedCode: 'old device edit' });
    await pending;
    expect(manager.getFileState('/workspace/pending.ts')).toBeNull();
  });
});
