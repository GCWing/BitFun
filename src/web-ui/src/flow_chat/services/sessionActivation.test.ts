import { beforeEach, describe, expect, it, vi } from 'vitest';

const mocks = vi.hoisted(() => ({
  flowChatState: {
    activeSessionId: null as string | null,
    sessions: new Map<string, unknown>(),
  },
  sceneState: {
    openTabs: [] as Array<{ id: string }>,
    closeScene: vi.fn(),
  },
}));

vi.mock('@/app/services/AppManager', () => ({
  appManager: { updateLayout: vi.fn() },
}));

vi.mock('@/app/stores/sceneStore', () => ({
  useSceneStore: {
    getState: () => mocks.sceneState,
  },
}));

vi.mock('../store/FlowChatStore', () => ({
  flowChatStore: {
    getState: () => mocks.flowChatState,
  },
}));

vi.mock('./FlowChatManager', () => ({
  flowChatManager: { switchChatSession: vi.fn() },
}));

vi.mock('./storeSync', () => ({
  syncSessionToModernStore: vi.fn(),
}));

import { closeSessionSceneAfterActiveSessionArchive } from './sessionActivation';

describe('closeSessionSceneAfterActiveSessionArchive', () => {
  beforeEach(() => {
    mocks.flowChatState.activeSessionId = null;
    mocks.flowChatState.sessions = new Map();
    mocks.sceneState.openTabs = [{ id: 'session' }];
    mocks.sceneState.closeScene.mockReset();
  });

  it('closes the Session scene after the archived active session is removed', () => {
    expect(closeSessionSceneAfterActiveSessionArchive('active-1')).toBe(true);
    expect(mocks.sceneState.closeScene).toHaveBeenCalledWith('session');
  });

  it('keeps the Session scene when another session became active', () => {
    mocks.flowChatState.activeSessionId = 'replacement-1';

    expect(closeSessionSceneAfterActiveSessionArchive('active-1')).toBe(false);
    expect(mocks.sceneState.closeScene).not.toHaveBeenCalled();
  });

  it('keeps the Session scene when the archive did not remove the active session', () => {
    mocks.flowChatState.activeSessionId = 'active-1';
    mocks.flowChatState.sessions.set('active-1', {});

    expect(closeSessionSceneAfterActiveSessionArchive('active-1')).toBe(false);
    expect(mocks.sceneState.closeScene).not.toHaveBeenCalled();
  });

  it('does nothing when a non-active session was archived', () => {
    expect(closeSessionSceneAfterActiveSessionArchive(null)).toBe(false);
    expect(mocks.sceneState.closeScene).not.toHaveBeenCalled();
  });
});
