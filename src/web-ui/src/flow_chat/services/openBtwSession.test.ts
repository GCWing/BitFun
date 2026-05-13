import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { openBtwSessionInAuxPane } from './openBtwSession';

const mocks = vi.hoisted(() => ({
  createTab: vi.fn(),
  clearSessionUnreadCompletion: vi.fn(),
  findTabByMetadata: vi.fn(),
  switchToTab: vi.fn(),
  closeTab: vi.fn(),
}));

let animationFrameCallbacks: FrameRequestCallback[] = [];

const stubWindowForPanelExpansion = (rightPanelCollapsed: boolean) => {
  const dispatchEvent = vi.fn();
  class TestCustomEvent {
    readonly type: string;
    readonly detail?: unknown;

    constructor(type: string, init?: { detail?: unknown }) {
      this.type = type;
      this.detail = init?.detail;
    }
  }

  vi.stubGlobal('window', {
    CustomEvent: TestCustomEvent,
    dispatchEvent,
    __BITFUN_LAYOUT_STATE__: { rightPanelCollapsed },
  });

  return dispatchEvent;
};

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
      findTabByMetadata: (...args: unknown[]) => mocks.findTabByMetadata(...args),
      switchToTab: (...args: unknown[]) => mocks.switchToTab(...args),
      closeTab: (...args: unknown[]) => mocks.closeTab(...args),
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
    mocks.findTabByMetadata.mockReset();
    mocks.switchToTab.mockClear();
    mocks.closeTab.mockClear();
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

  it('switches to an existing aux pane tab without expanding the right panel again', () => {
    const dispatchEvent = stubWindowForPanelExpansion(false);
    mocks.findTabByMetadata.mockReturnValue({
      tab: { id: 'existing-review-tab' },
      groupId: 'secondary',
    });

    openBtwSessionInAuxPane({
      childSessionId: 'review-child',
      parentSessionId: 'parent-session',
      workspacePath: 'D:\\workspace\\repo',
    });

    expect(mocks.findTabByMetadata).toHaveBeenCalledWith({
      duplicateCheckKey: 'btw-session-review-child',
    });
    expect(mocks.switchToTab).toHaveBeenCalledWith('existing-review-tab', 'secondary');
    expect(mocks.createTab).not.toHaveBeenCalled();
    expect(dispatchEvent).not.toHaveBeenCalledWith(
      expect.objectContaining({ type: 'expand-right-panel' }),
    );
  });

  it('expands the right panel before switching to an existing aux pane tab when collapsed', () => {
    const dispatchEvent = stubWindowForPanelExpansion(true);
    mocks.findTabByMetadata.mockReturnValue({
      tab: { id: 'existing-review-tab' },
      groupId: 'secondary',
    });

    openBtwSessionInAuxPane({
      childSessionId: 'review-child',
      parentSessionId: 'parent-session',
      workspacePath: 'D:\\workspace\\repo',
    });

    expect(mocks.switchToTab).toHaveBeenCalledWith('existing-review-tab', 'secondary');
    expect(mocks.createTab).not.toHaveBeenCalled();
    expect(dispatchEvent).toHaveBeenCalledWith(
      expect.objectContaining({ type: 'expand-right-panel' }),
    );
  });
});
