import React, { act } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { createRoot, type Root } from 'react-dom/client';
import { JSDOM } from 'jsdom';

import { ForkSessionButton } from './ForkSessionButton';

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

const flowState = {
  sessions: new Map<string, any>(),
};

vi.mock('react-i18next', () => ({
  initReactI18next: {
    type: '3rdParty',
    init: () => undefined,
  },
  useTranslation: () => ({
    t: (key: string, options?: { defaultValue?: string }) =>
      options?.defaultValue ?? key,
  }),
}));

vi.mock('@/component-library', () => ({
  Tooltip: ({ children }: { children: React.ReactNode }) => <>{children}</>,
}));

vi.mock('../../services/FlowChatManager', () => ({
  flowChatManager: {
    forkChatSession: vi.fn(),
  },
}));

vi.mock('../../store/FlowChatStore', () => ({
  flowChatStore: {
    getState: () => flowState,
  },
}));

vi.mock('@/shared/notification-system', () => ({
  notificationService: {
    error: vi.fn(),
  },
}));

describe('ForkSessionButton', () => {
  let dom: JSDOM;
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    dom = new JSDOM('<!doctype html><html><body><div id="root"></div></body></html>', {
      pretendToBeVisual: true,
    });
    vi.stubGlobal('window', dom.window);
    vi.stubGlobal('document', dom.window.document);
    vi.stubGlobal('HTMLElement', dom.window.HTMLElement);

    container = dom.window.document.getElementById('root') as HTMLDivElement;
    root = createRoot(container);
    flowState.sessions = new Map();
  });

  afterEach(() => {
    act(() => {
      root.unmount();
    });
    vi.unstubAllGlobals();
  });

  it('hides the fork button for subagent sessions', () => {
    flowState.sessions = new Map([
      ['subagent-session', { sessionId: 'subagent-session', sessionKind: 'subagent' }],
    ]);

    act(() => {
      root.render(<ForkSessionButton sessionId="subagent-session" turnId="turn-1" />);
    });

    expect(container.querySelector('.model-round-item__fork-btn')).toBeNull();
  });

  it('renders the fork button for normal sessions', () => {
    flowState.sessions = new Map([
      ['main-session', { sessionId: 'main-session', sessionKind: 'normal' }],
    ]);

    act(() => {
      root.render(<ForkSessionButton sessionId="main-session" turnId="turn-1" />);
    });

    expect(container.querySelector('.model-round-item__fork-btn')).not.toBeNull();
  });
});
