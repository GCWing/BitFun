/**
 * @vitest-environment jsdom
 */

import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { useMessageSender } from './useMessageSender';

const mocks = vi.hoisted(() => {
  const createChatSession = vi.fn();
  const sendMessage = vi.fn();
  const manager = {
    createChatSession,
    sendMessage,
    getFlowChatState: () => ({
      sessions: new Map([['created-session', { mode: 'agentic' }]]),
    }),
  };

  return {
    createChatSession,
    sendMessage,
    manager,
    onClearContexts: vi.fn(),
  };
});

vi.mock('../services/FlowChatManager', () => ({
  FlowChatManager: {
    getInstance: () => mocks.manager,
  },
}));

vi.mock('@/app/utils/projectSessionWorkspace', () => ({
  flowChatSessionConfigForCurrentWorkspace: () => ({ workspacePath: '/workspace/project' }),
}));

vi.mock('../utils/imagePayload', () => ({
  buildImagePayload: vi.fn().mockResolvedValue(undefined),
}));

vi.mock('@/shared/notification-system', () => ({
  notificationService: { error: vi.fn() },
}));

vi.mock('@/shared/utils/logger', () => ({
  createLogger: () => ({
    debug: vi.fn(),
    info: vi.fn(),
    error: vi.fn(),
  }),
}));

let sendFromProbe: (() => Promise<void>) | undefined;
let sendDraftFromProbe: (() => Promise<void>) | undefined;

function Probe() {
  const { sendMessage } = useMessageSender({
    contexts: [],
    onClearContexts: mocks.onClearContexts,
  });
  sendFromProbe = () => sendMessage('hello');
  sendDraftFromProbe = () => sendMessage('expanded paste content', {
    displayMessage: '[Pasted text #1]',
    composerDraft: {
      value: '[Pasted text #1]',
      pendingLargePastes: { '[Pasted text #1]': 'expanded paste content' },
    },
  });
  return null;
}

describe('useMessageSender', () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    globalThis.IS_REACT_ACT_ENVIRONMENT = true;
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
    mocks.createChatSession.mockResolvedValue('created-session');
    mocks.sendMessage.mockResolvedValue(undefined);
  });

  afterEach(() => {
    act(() => root.unmount());
    container.remove();
    sendFromProbe = undefined;
    sendDraftFromProbe = undefined;
    vi.clearAllMocks();
  });

  it('creates a Session with the selected Agent type and no Harness overlay', async () => {
    await act(async () => {
      root.render(<Probe />);
    });
    await act(async () => {
      await sendFromProbe?.();
    });

    expect(mocks.createChatSession).toHaveBeenCalledWith(
      {
        workspacePath: '/workspace/project',
      },
      'agentic',
    );
    expect(mocks.sendMessage).toHaveBeenCalledWith(
      'hello',
      'created-session',
      'hello',
      'agentic',
      undefined,
      expect.objectContaining({
        pendingQueueDraft: {
          value: 'hello',
          contexts: [],
          pendingLargePastes: {},
        },
      }),
    );
  });

  it('preserves the original large-paste composer draft for queued restoration', async () => {
    await act(async () => {
      root.render(<Probe />);
    });
    await act(async () => {
      await sendDraftFromProbe?.();
    });

    expect(mocks.sendMessage).toHaveBeenCalledWith(
      'expanded paste content',
      'created-session',
      '[Pasted text #1]',
      'agentic',
      undefined,
      expect.objectContaining({
        pendingQueueDraft: {
          value: '[Pasted text #1]',
          contexts: [],
          pendingLargePastes: { '[Pasted text #1]': 'expanded paste content' },
        },
      }),
    );
  });
});
