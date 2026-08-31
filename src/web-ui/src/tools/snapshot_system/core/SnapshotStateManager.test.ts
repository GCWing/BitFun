import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { Session } from '@/flow_chat/types/flow-chat';
import { SnapshotStateManager } from './SnapshotStateManager';
import { SnapshotEventBus, SNAPSHOT_EVENTS } from './SnapshotEventBus';
import { activateSurface, getActiveSurfaceId } from '@/infrastructure/peer-device/deviceSurface';

const mocks = vi.hoisted(() => ({
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
describe('snapshot refresh target routing', () => {
  let testSurface = 0;
  beforeEach(() => {
    mocks.sessions.clear();
    activateSurface(`snapshot-test-${++testSurface}`);
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
      activateSurface(`${getActiveSurfaceId()}-next`);
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
    activateSurface(`${getActiveSurfaceId()}-next`);
    finishDiff({ originalCode: 'old device', modifiedCode: 'old device edit' });
    await pending;
    expect(manager.getFileState('/workspace/pending.ts')).toBeNull();
  });

  it('preserves separate local-workspace snapshots for equal Session ids and paths on two devices', async () => {
    const firstSurface = getActiveSurfaceId();
    const secondSurface = `${firstSurface}-other`;
    mocks.sessions.set('same-session', { config: {}, historyState: 'ready' } as Session);
    const manager = SnapshotStateManager.getInstance();
    await manager.refreshSessionState('same-session');
    mocks.getOperationDiff.mockResolvedValueOnce({ originalCode: 'A before', modifiedCode: 'A after' });
    await manager.refreshFileState('same-session', '/workspace/same.ts');

    activateSurface(secondSurface);
    expect(manager.getSessionState('same-session')).toBeNull();
    expect(manager.getFileState('/workspace/same.ts')).toBeNull();
    await manager.refreshSessionState('same-session');
    mocks.getOperationDiff.mockResolvedValueOnce({ originalCode: 'B before', modifiedCode: 'B after' });
    await manager.refreshFileState('same-session', '/workspace/same.ts');
    expect(manager.getSessionFiles('same-session')[0].modifiedContent).toBe('B after');

    activateSurface(firstSurface);
    expect(manager.getSessionFiles('same-session')[0].modifiedContent).toBe('A after');
    expect(manager.getFileState('/workspace/same.ts')?.originalContent).toBe('A before');
    manager.clearSession('same-session');
    activateSurface(secondSurface);
    expect(manager.getSessionFiles('same-session')[0].modifiedContent).toBe('B after');
  });

  it('does not overwrite either device with an old activation response after switching away and back', async () => {
    const firstSurface = getActiveSurfaceId();
    mocks.sessions.set('same-session', { config: {}, historyState: 'ready' } as Session);
    const manager = SnapshotStateManager.getInstance();
    mocks.getOperationDiff.mockResolvedValueOnce({ originalCode: 'A', modifiedCode: 'A saved' });
    await manager.refreshFileState('same-session', '/workspace/same.ts');
    let finish!: (diff: { originalCode: string; modifiedCode: string }) => void;
    mocks.getOperationDiff.mockImplementationOnce(() => new Promise(resolve => { finish = resolve; }));
    const pending = manager.refreshFileState('same-session', '/workspace/same.ts');
    activateSurface(`${firstSurface}-other`);
    mocks.getOperationDiff.mockResolvedValueOnce({ originalCode: 'B', modifiedCode: 'B saved' });
    await manager.refreshFileState('same-session', '/workspace/same.ts');
    activateSurface(firstSurface);
    finish({ originalCode: 'stale A', modifiedCode: 'stale A' });
    await pending;
    expect(manager.getFileState('/workspace/same.ts')?.modifiedContent).toBe('A saved');
    activateSurface(`${firstSurface}-other`);
    expect(manager.getFileState('/workspace/same.ts')?.modifiedContent).toBe('B saved');
  });
});
