/**
 * @vitest-environment jsdom
 */

import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import type { SessionExecutionProfile } from '@/infrastructure/api/service-api/AgentAPI';
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

function Probe({ executionProfile }: { executionProfile: SessionExecutionProfile }) {
  const { sendMessage } = useMessageSender({
    newSessionExecutionProfile: executionProfile,
    contexts: [],
    onClearContexts: mocks.onClearContexts,
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

  it('carries a staged Harness Profile into canonical first-session creation', async () => {
    const executionProfile: SessionExecutionProfile = {
      harnessProfileId: 'minimal',
      schemaVersion: 1,
      selectedBy: 'user',
    };

    await act(async () => {
      root.render(<Probe executionProfile={executionProfile} />);
    });
    await act(async () => {
      await sendFromProbe?.();
    });

    expect(mocks.createChatSession).toHaveBeenCalledWith(
      {
        workspacePath: '/workspace/project',
        executionProfile,
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
});
