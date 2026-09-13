import React, { act } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { createRoot, type Root } from 'react-dom/client';
import { JSDOM } from 'jsdom';

import { ForkSessionButton } from './ForkSessionButton';
import { flowChatManager } from '../../services/FlowChatManager';
import { notificationService } from '@/shared/notification-system';

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

vi.mock('@openbitfun/ui', async importOriginal => ({
  IconButton: (await importOriginal<typeof import('@openbitfun/ui')>()).IconButton,
  Icon: ({ name, className }: { name: string; className?: string }) => (
    <span className={className} data-openbitfun-component="icon" data-openbitfun-name={name} />
  ),
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
    vi.clearAllMocks();
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

    const button = container.querySelector('.model-round-item__fork-btn');
    expect(button).not.toBeNull();
    expect(button?.querySelector('[data-openbitfun-name="git"]')).not.toBeNull();
  });

  it('keeps the original pending icon and disables duplicate forks until completion', async () => {
    let complete!: () => void;
    vi.mocked(flowChatManager.forkChatSession).mockImplementationOnce(
      () => new Promise<string>(resolve => { complete = () => resolve('forked-session'); }),
    );
    act(() => {
      root.render(<ForkSessionButton sessionId="main-session" turnId="turn-1" />);
    });

    const button = container.querySelector<HTMLButtonElement>('.model-round-item__fork-btn')!;
    act(() => button.click());

    expect(flowChatManager.forkChatSession).toHaveBeenCalledExactlyOnceWith('main-session', 'turn-1');
    expect(button.disabled).toBe(true);
    expect(button.querySelector('.spinning[data-openbitfun-name="progress-25"]')).not.toBeNull();
    expect(button.getAttribute('data-loading')).toBe('false');
    act(() => button.click());
    expect(flowChatManager.forkChatSession).toHaveBeenCalledTimes(1);

    await act(async () => complete());
    expect(button.disabled).toBe(false);
    expect(button.querySelector('[data-openbitfun-name="git"]')).not.toBeNull();
  });

  it('restores the action after a failed fork without changing its error notification', async () => {
    vi.mocked(flowChatManager.forkChatSession).mockRejectedValueOnce(new Error('Fork failed'));
    act(() => {
      root.render(<ForkSessionButton sessionId="main-session" turnId="turn-1" />);
    });

    const button = container.querySelector<HTMLButtonElement>('.model-round-item__fork-btn')!;
    await act(async () => button.click());

    expect(button.disabled).toBe(false);
    expect(button.querySelector('[data-openbitfun-name="git"]')).not.toBeNull();
    expect(notificationService.error).toHaveBeenCalledExactlyOnceWith('modelRound.forkFailed', { duration: 3500 });
  });
});
