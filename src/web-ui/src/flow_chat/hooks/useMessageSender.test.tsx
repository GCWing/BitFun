/**
 * @vitest-environment jsdom
 */

import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { useMessageSender } from './useMessageSender';
import { chatInputTurnDirective } from '../utils/chatInputDirective';

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

function Probe() {
  const { sendMessage } = useMessageSender({
    contexts: [],
    onClearContexts: mocks.onClearContexts,
  });
  sendFromProbe = () => sendMessage('hello');
  return null;
}

function DirectiveProbe({ onConsumed }: { onConsumed: () => void }) {
  const { sendMessage } = useMessageSender({
    currentSessionId: 'existing-session',
    contexts: [],
    onClearContexts: mocks.onClearContexts,
    turnDirective: chatInputTurnDirective('Plan'),
    onTurnDirectiveConsumed: onConsumed,
  });
  sendFromProbe = () => sendMessage('hello');
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
      expect.any(Object),
    );
  });

  it('applies a directive to one task without replacing the Session main Agent', async () => {
    const onConsumed = vi.fn();
    await act(async () => {
      root.render(<DirectiveProbe onConsumed={onConsumed} />);
    });
    await act(async () => {
      await sendFromProbe?.();
    });

    expect(mocks.createChatSession).not.toHaveBeenCalled();
    expect(mocks.sendMessage).toHaveBeenCalledWith(
      expect.stringContaining('<task-directive name="Plan">'),
      'existing-session',
      'hello',
      'agentic',
      undefined,
      expect.objectContaining({
        userMessageMetadata: expect.objectContaining({
          taskDirective: { id: 'Plan' },
        }),
      }),
    );
    expect(onConsumed).toHaveBeenCalledTimes(1);
  });

  it('keeps the directive armed when submission fails', async () => {
    const onConsumed = vi.fn();
    mocks.sendMessage.mockRejectedValueOnce(new Error('submission failed'));
    await act(async () => {
      root.render(<DirectiveProbe onConsumed={onConsumed} />);
    });

    await act(async () => {
      await expect(sendFromProbe?.()).rejects.toThrow('submission failed');
    });

    expect(onConsumed).not.toHaveBeenCalled();
  });
});
