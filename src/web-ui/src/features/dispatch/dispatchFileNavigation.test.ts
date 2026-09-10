import { beforeEach, describe, expect, it, vi } from 'vitest';
import { activateSurface } from '@/infrastructure/peer-device/deviceSurface';

const mocks = vi.hoisted(() => ({
  sessions: new Map<string, any>(),
  jobs: {} as Record<string, any>,
  readFile: vi.fn(),
  createTab: vi.fn(),
  error: vi.fn(),
}));
vi.mock('@/flow_chat/store/FlowChatStore', () => ({ flowChatStore: { getState: () => ({ sessions: mocks.sessions }) } }));
vi.mock('./dispatchJobStore', () => ({ dispatchJobStore: { getState: () => ({ jobs: mocks.jobs }) } }));
vi.mock('./dispatchApi', () => ({ dispatchApi: { readFile: mocks.readFile } }));
vi.mock('@/shared/utils/tabUtils', () => ({ createTab: mocks.createTab }));
vi.mock('@/shared/notification-system', () => ({ notificationService: { error: mocks.error } }));

import { isDispatchFileSession, openDispatchSessionFile } from './dispatchFileNavigation';

describe('dispatch file navigation', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    activateSurface('local');
    mocks.sessions.clear();
    mocks.jobs = {};
    mocks.sessions.set('session-1', { config: { dispatchJobId: 'job-1' } });
    mocks.readFile.mockResolvedValue({ kind: 'readFile', jobId: 'job-1', sessionId: 'session-1', filePath: '/target/result.txt', content: 'latest target bytes' });
  });

  it('opens target bytes in a read-only memory editor with a host-scoped identity', async () => {
    expect(isDispatchFileSession('session-1')).toBe(true);
    await openDispatchSessionFile('session-1', '/target/result.txt', 'result.txt', { start: 3 });
    expect(mocks.readFile).toHaveBeenCalledWith('job-1', '/target/result.txt');
    expect(mocks.createTab).toHaveBeenCalledWith(expect.objectContaining({
      data: expect.objectContaining({ filePath: 'dispatch-file://job-1/%2Ftarget%2Fresult.txt', initialContent: 'latest target bytes', readOnly: true, jumpToRange: { start: 3 } }),
      replaceExisting: true,
    }));
  });

  it('routes child projections and not-yet-bound observed sessions to the owning job', async () => {
    mocks.sessions.set('child', { parentSessionId: 'session-1' });
    await openDispatchSessionFile('child', 'result.txt', 'result.txt');
    expect(mocks.readFile).toHaveBeenLastCalledWith('job-1', 'result.txt');
    mocks.sessions.clear();
    mocks.jobs['job-1'] = { jobId: 'job-1', sessionId: 'session-1' };
    expect(isDispatchFileSession('session-1')).toBe(true);
    await openDispatchSessionFile('session-1', 'result.txt', 'result.txt');
    expect(mocks.readFile).toHaveBeenCalledTimes(2);
  });

  it('does not fall back when an older target rejects file preview', async () => {
    mocks.readFile.mockRejectedValue(new Error('Update target CLI or sync changes'));
    await openDispatchSessionFile('session-1', 'result.txt', 'result.txt');
    expect(mocks.createTab).not.toHaveBeenCalled();
    expect(mocks.error).toHaveBeenCalledWith('Update target CLI or sync changes');
  });

  it('drops an in-flight reply after leaving and returning to the same surface', async () => {
    let finish!: (value: unknown) => void;
    mocks.readFile.mockReturnValue(new Promise(resolve => { finish = resolve; }));
    const opened = openDispatchSessionFile('session-1', 'result.txt', 'result.txt');
    activateSurface('peer:device-b');
    activateSurface('local');
    finish({ kind: 'readFile', jobId: 'job-1', filePath: '/target/result.txt', content: 'stale' });
    await opened;
    expect(mocks.createTab).not.toHaveBeenCalled();
    expect(mocks.error).not.toHaveBeenCalled();
  });
});
