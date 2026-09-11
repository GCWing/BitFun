// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { activateSurface } from '@/infrastructure/peer-device/deviceSurface';

const mocks = vi.hoisted(() => ({
  sessions: new Map<string, any>(),
  jobs: {} as Record<string, any>,
  readFile: vi.fn(),
  readFileChunk: vi.fn(),
  createTab: vi.fn(),
  error: vi.fn(),
}));
vi.mock('@/flow_chat/store/FlowChatStore', () => ({ flowChatStore: { getState: () => ({ sessions: mocks.sessions }) } }));
vi.mock('./dispatchJobStore', () => ({ dispatchJobStore: { getState: () => ({ jobs: mocks.jobs }) } }));
vi.mock('./dispatchApi', () => ({ dispatchApi: { readFile: mocks.readFile, readFileChunk: mocks.readFileChunk } }));
vi.mock('@/shared/utils/tabUtils', () => ({ createTab: mocks.createTab }));
vi.mock('@/shared/notification-system', () => ({ notificationService: { error: mocks.error } }));

import { isDispatchFileSession, openDispatchSessionFile, readDispatchSessionImage, downloadDispatchSessionFile } from './dispatchFileNavigation';

describe('dispatch file navigation', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    activateSurface('local');
    mocks.sessions.clear();
    mocks.jobs = {};
    mocks.sessions.set('session-1', { config: { dispatchJobId: 'job-1' } });
    mocks.readFile.mockResolvedValue({ kind: 'readFile', jobId: 'job-1', sessionId: 'session-1', filePath: '/target/result.txt', content: 'latest target bytes' });
    mocks.readFileChunk.mockReset();
  });

  afterEach(() => { vi.unstubAllGlobals(); vi.useRealTimers(); vi.restoreAllMocks(); });

  function chunk(offset: number, bytes: string, overrides = {}) {
    return { kind: 'readFileChunk', jobId: 'job-1', sessionId: 'session-1', filePath: '/target/output.png',
      name: 'output.png', mimeType: 'image/png', totalSize: 3, offset, chunkSize: bytes.length,
      contentBase64: btoa(bytes), revision: 'revision-1', ...overrides };
  }

  it('joins padded binary chunks from the target and preserves revision on the next request', async () => {
    mocks.readFileChunk.mockResolvedValueOnce(chunk(0, 'a')).mockResolvedValueOnce(chunk(1, 'bc'));
    expect(await readDispatchSessionImage('session-1', 'output.png', true)).toBe('data:image/png;base64,YWJj');
    expect(mocks.readFileChunk).toHaveBeenLastCalledWith('job-1', 'output.png', {
      offset: 1, limit: 256 * 1024, expectedRevision: 'revision-1',
    });
    expect(mocks.readFile).not.toHaveBeenCalled();
  });

  it('reads new bytes when a later result reuses the same output path', async () => {
    mocks.readFileChunk.mockResolvedValueOnce(chunk(0, 'abc'));
    expect(await readDispatchSessionImage('session-1', 'reused.png')).toBe('data:image/png;base64,YWJj');
    mocks.readFileChunk.mockResolvedValueOnce(chunk(0, 'def', { revision: 'revision-2' }));
    expect(await readDispatchSessionImage('session-1', 'reused.png')).toBe('data:image/png;base64,ZGVm');
    expect(mocks.readFileChunk).toHaveBeenCalledTimes(2);
  });

  it('resumes a disconnected binary transfer at the same offset and revision', async () => {
    mocks.readFileChunk.mockResolvedValueOnce(chunk(0, 'a'))
      .mockRejectedValueOnce(new Error('Connection interrupted'))
      .mockResolvedValueOnce(chunk(1, 'bc'));
    expect(await readDispatchSessionImage('session-1', 'resumed.png', true)).toBe('data:image/png;base64,YWJj');
    expect(mocks.readFileChunk.mock.calls.map(call => call[2].offset)).toEqual([0, 1, 1]);
    expect(mocks.readFileChunk.mock.calls[2][2].expectedRevision).toBe('revision-1');
  });

  it('downloads target bytes with the target filename and releases the temporary URL', async () => {
    vi.useFakeTimers();
    const createObjectURL = vi.fn((_blob: Blob) => 'blob:target-output');
    const revokeObjectURL = vi.fn();
    vi.stubGlobal('URL', { createObjectURL, revokeObjectURL });
    const click = vi.spyOn(HTMLAnchorElement.prototype, 'click').mockImplementation(function () {
      expect(this.download).toBe('report.pdf');
      expect(this.href).toBe('blob:target-output');
    });
    mocks.readFileChunk.mockResolvedValue(chunk(0, 'abc', {name:'report.pdf',mimeType:'application/pdf'}));
    await downloadDispatchSessionFile('session-1','report.pdf');
    expect(createObjectURL.mock.calls[0][0]).toMatchObject({ size:3, type:'application/pdf' });
    expect(click).toHaveBeenCalledOnce();
    expect(mocks.readFile).not.toHaveBeenCalled();
    vi.runAllTimers();
    expect(revokeObjectURL).toHaveBeenCalledWith('blob:target-output');
  });

  it.each([{ revision: 'changed' }, { offset: 0 }, { chunkSize: 99 }, { contentBase64: '' }])(
    'rejects inconsistent output chunks %o', async overrides => {
      mocks.readFileChunk.mockResolvedValueOnce(chunk(0, 'a')).mockResolvedValueOnce(chunk(1, 'bc', overrides));
      await expect(readDispatchSessionImage('session-1', 'broken.png', true)).rejects.toThrow();
    },
  );

  it('does not request another chunk after switching the device surface', async () => {
    mocks.readFileChunk.mockImplementationOnce(async () => {
      activateSurface('peer:device-b');
      return chunk(0, 'a');
    });
    await expect(readDispatchSessionImage('session-1', 'stale.png', true)).rejects.toThrow();
    expect(mocks.readFileChunk).toHaveBeenCalledTimes(1);
  });

  it('opens supplied image bytes in the image viewer without a controller file path', async () => {
    mocks.readFileChunk.mockResolvedValue(chunk(0, 'abc'));
    await openDispatchSessionFile('session-1', 'output.png', 'output.png');
    expect(mocks.createTab).toHaveBeenCalledWith(expect.objectContaining({
      type: 'image-viewer', data: expect.objectContaining({ imageSource: { dataUrl: 'data:image/png;base64,YWJj', size: 3 } }),
    }));
    expect(mocks.readFile).not.toHaveBeenCalled();
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
