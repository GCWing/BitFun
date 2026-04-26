import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { openBtwSessionInAuxPane } from './openBtwSession';

const mocks = vi.hoisted(() => ({
  createTab: vi.fn(),
  clearSessionUnreadCompletion: vi.fn(),
}));

let animationFrameCallbacks: FrameRequestCallback[] = [];

vi.mock('@/infrastructure/i18n', () => ({
  i18nService: {
    t: (_key: string, options?: { defaultValue?: string }) => options?.defaultValue ?? 'Side thread',
  },
}));

vi.mock('@/app/services/AppManager', () => ({
  appManager: {
    updateLayout: vi.fn(),
  },
}));

vi.mock('@/app/stores/sceneStore', () => ({
  useSceneStore: {
    getState: () => ({
      openScene: vi.fn(),
    }),
  },
}));

vi.mock('@/shared/utils/tabUtils', () => ({
  createTab: (...args: unknown[]) => mocks.createTab(...args),
}));

vi.mock('@/app/components/panels/content-canvas/stores', () => ({
  useAgentCanvasStore: {
    getState: () => ({
      activeGroupId: 'primary',
      primaryGroup: { activeTabId: null, tabs: [] },
      secondaryGroup: { activeTabId: null, tabs: [] },
      tertiaryGroup: { activeTabId: null, tabs: [] },
      findTabByMetadata: vi.fn(),
      closeTab: vi.fn(),
    }),
  },
}));

vi.mock('../store/FlowChatStore', () => ({
  flowChatStore: {
    getState: () => ({
      sessions: new Map(),
    }),
    clearSessionUnreadCompletion: (...args: unknown[]) =>
      mocks.clearSessionUnreadCompletion(...args),
  },
}));

vi.mock('./FlowChatManager', () => ({
  flowChatManager: {
    switchChatSession: vi.fn(),
  },
}));

vi.mock('./storeSync', () => ({
  syncSessionToModernStore: vi.fn(),
}));

describe('openBtwSessionInAuxPane', () => {
  beforeEach(() => {
    vi.restoreAllMocks();
    vi.unstubAllGlobals();
    animationFrameCallbacks = [];
    mocks.createTab.mockClear();
    mocks.clearSessionUnreadCompletion.mockClear();
    vi.stubGlobal('requestAnimationFrame', vi.fn((callback: FrameRequestCallback) => {
      animationFrameCallbacks.push(callback);
      return animationFrameCallbacks.length;
    }));
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it('clears the child session unread completion marker after opening the aux pane', () => {
    openBtwSessionInAuxPane({
      childSessionId: 'review-child',
      parentSessionId: 'parent-session',
      workspacePath: 'D:\\workspace\\repo',
      expand: false,
    });

    expect(mocks.createTab).toHaveBeenCalledWith(
      expect.objectContaining({
        type: 'btw-session',
        data: expect.objectContaining({
          childSessionId: 'review-child',
          parentSessionId: 'parent-session',
        }),
      }),
    );

    expect(mocks.clearSessionUnreadCompletion).not.toHaveBeenCalled();
    expect(animationFrameCallbacks).toHaveLength(1);

    animationFrameCallbacks.shift()?.(0);
    expect(mocks.clearSessionUnreadCompletion).not.toHaveBeenCalled();
    expect(animationFrameCallbacks).toHaveLength(1);

    animationFrameCallbacks.shift()?.(16);
    expect(mocks.clearSessionUnreadCompletion).toHaveBeenCalledWith('review-child');
  });
});
