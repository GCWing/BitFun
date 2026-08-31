import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { JSDOM } from 'jsdom';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { Session } from '@/flow_chat/types/flow-chat';
import { useSnapshotState } from './useSnapshotState';
import type { SessionState, SnapshotFile } from '../core/SnapshotStateManager';
import { activateSurface } from '@/infrastructure/peer-device/deviceSurface';

const mocks = vi.hoisted(() => ({
  sessions: new Map<string, Session>(),
  listeners: new Set<(state: { sessions: Map<string, Session> }) => void>(),
  t: (key: string) => key,
  ensureInitialized: vi.fn(async () => {}),
  refreshSessionState: vi.fn(async () => {}),
  getSessionState: vi.fn<() => SessionState | null>(() => null),
  getSessionFiles: vi.fn<() => SnapshotFile[]>(() => []),
  getFileState: vi.fn(),
  onSessionStateChange: () => () => {},
  onFileStateChange: () => () => {},
}));

vi.mock('react-i18next', () => ({ useTranslation: () => ({ t: mocks.t }) }));
vi.mock('@/flow_chat/store/FlowChatStore', () => ({
  flowChatStore: {
    getState: () => ({ sessions: mocks.sessions }),
    subscribe: (listener: (state: { sessions: Map<string, Session> }) => void) => {
      mocks.listeners.add(listener);
      return () => mocks.listeners.delete(listener);
    },
  },
}));
vi.mock('../core/SnapshotStateManager', () => ({
  SnapshotStateManager: { getInstance: () => mocks },
}));
vi.mock('../core/SnapshotLazyLoader', () => ({ default: mocks }));

describe('snapshot availability while session metadata changes', () => {
  let root: Root;
  let dom: JSDOM;
  let snapshot: ReturnType<typeof useSnapshotState>;
  let renderedFiles: string[][];
  const Probe = () => {
    snapshot = useSnapshotState('session');
    renderedFiles.push(snapshot.files.map(file => file.modifiedContent));
    return null;
  };

  beforeEach(() => {
    activateSurface('local');
    dom = new JSDOM('<div id="root"></div>');
    vi.stubGlobal('window', dom.window);
    vi.stubGlobal('document', dom.window.document);
    vi.stubGlobal('IS_REACT_ACT_ENVIRONMENT', true);
    root = createRoot(dom.window.document.getElementById('root')!);
    mocks.sessions.clear();
    mocks.listeners.clear();
    vi.clearAllMocks();
    mocks.refreshSessionState.mockReset().mockResolvedValue();
    mocks.getSessionState.mockReset().mockReturnValue(null);
    mocks.getSessionFiles.mockReset().mockReturnValue([]);
    renderedFiles = [];
  });

  afterEach(async () => {
    await act(async () => root.unmount());
    dom.window.close();
    vi.unstubAllGlobals();
  });

  it('keeps disconnected legacy remote sessions passive with a stable empty file list', async () => {
    mocks.sessions.set('session', {
      config: { remoteConnectionId: 'saved-ssh' }, historyState: 'ready',
    } as Session);
    await act(async () => root.render(<Probe />));
    expect(snapshot!.snapshotsAvailable).toBe(false);
    expect(mocks.ensureInitialized).not.toHaveBeenCalled();
    expect(mocks.refreshSessionState).not.toHaveBeenCalled();
    expect(snapshot!.getFullDiff('/workspace/shared.ts')).toBeNull();
    expect(mocks.getFileState).not.toHaveBeenCalled();
    const files = snapshot!.files;
    await act(async () => root.render(<Probe />));
    expect(snapshot!.files).toBe(files);
  });

  it('reacts to hydrated SSH identity without requesting more snapshots', async () => {
    mocks.sessions.set('session', { config: {}, historyState: 'ready' } as Session);
    await act(async () => root.render(<Probe />));
    expect(snapshot!.snapshotsAvailable).toBe(true);
    expect(mocks.refreshSessionState).toHaveBeenCalledOnce();
    await act(async () => {
      mocks.sessions.set('session', { remoteConnectionId: 'ssh', historyState: 'ready' } as Session);
      mocks.listeners.forEach(listener => listener({ sessions: mocks.sessions }));
    });
    expect(snapshot!.snapshotsAvailable).toBe(false);
    expect(snapshot!.files).toEqual([]);
    expect(snapshot!.sessionState).toBeNull();
    await act(async () => snapshot!.refreshSession());
    expect(mocks.refreshSessionState).toHaveBeenCalledOnce();
  });

  it('rebinds equal Session ids on a new device without painting old files or accepting old completion', async () => {
    const file = (content: string): SnapshotFile => ({
      filePath: '/workspace/same.ts', sessionId: 'session', originalContent: '',
      modifiedContent: content, fileStatus: 'pending', lastModified: 0,
    });
    mocks.sessions.set('session', { config: {}, historyState: 'ready' } as Session);
    mocks.getSessionFiles.mockReturnValue([file('A contents')]);
    await act(async () => root.render(<Probe />));
    expect(snapshot!.files[0].modifiedContent).toBe('A contents');
    const firstEpoch = snapshot!.surfaceEpoch;

    let finishOld!: () => void;
    let finishNew!: () => void;
    mocks.refreshSessionState.mockImplementationOnce(() => new Promise(resolve => { finishOld = resolve; }));
    let oldRefresh!: Promise<void>;
    await act(async () => { oldRefresh = snapshot!.refreshSession(); });
    mocks.refreshSessionState.mockImplementationOnce(() => new Promise(resolve => { finishNew = resolve; }));
    const beforeSwitch = renderedFiles.length;
    await act(async () => { activateSurface('peer-with-local-workspace'); });
    expect(snapshot!.surfaceEpoch).not.toBe(firstEpoch);
    expect(mocks.refreshSessionState).toHaveBeenCalledTimes(3);
    expect(snapshot!.files).toEqual([]);
    expect(renderedFiles.slice(beforeSwitch).every(files => !files.includes('A contents'))).toBe(true);

    const readsBeforeOldCompletion = mocks.getSessionFiles.mock.calls.length;
    await act(async () => { finishOld(); await oldRefresh; });
    expect(mocks.getSessionFiles).toHaveBeenCalledTimes(readsBeforeOldCompletion);
    expect(snapshot!.files).toEqual([]);

    mocks.getSessionFiles.mockReturnValue([file('B contents')]);
    await act(async () => { finishNew(); });
    expect(snapshot!.files[0].modifiedContent).toBe('B contents');
  });
});
