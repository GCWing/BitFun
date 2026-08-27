/**
 * @vitest-environment jsdom
 */

import { act, type ButtonHTMLAttributes, type ReactNode } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { PendingQueuePanel } from './PendingQueuePanel';

const mocks = vi.hoisted(() => ({
  drainPendingQueueForSession: vi.fn(),
  insertSteeringItemIfAbsent: vi.fn(),
  promoteForExplicitDrain: vi.fn(() => true),
  queueRemove: vi.fn(),
  queueSetStatus: vi.fn(),
  steerDialogTurn: vi.fn(),
}));

vi.mock('react-i18next', () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));

vi.mock('@/component-library', () => ({
  Tooltip: ({ children }: { children: ReactNode }) => children,
  IconButton: ({ children, ...props }: ButtonHTMLAttributes<HTMLButtonElement>) => (
    <button {...props}>{children}</button>
  ),
}));

vi.mock('@/infrastructure/api/service-api/AgentAPI', () => ({
  agentAPI: { steerDialogTurn: mocks.steerDialogTurn },
}));

vi.mock('../state-machine', () => ({
  stateMachineManager: {
    get: () => ({
      getContext: () => ({ currentDialogTurnId: 'turn-1' }),
    }),
  },
}));

vi.mock('../store/FlowChatStore', () => ({
  FlowChatStore: {
    getInstance: () => ({
      getState: () => ({ sessions: new Map() }),
      subscribe: () => () => undefined,
    }),
  },
}));

vi.mock('../services/flow-chat-manager/PendingQueueModule', () => ({
  pendingQueueManager: {
    list: () => [
      {
        id: 'queued-1',
        sessionId: 'session-1',
        content: 'steer this turn',
        timestamp: 1,
        status: 'queued',
        retryCount: 0,
      },
    ],
    subscribe: () => () => undefined,
    setStatus: mocks.queueSetStatus,
    remove: mocks.queueRemove,
    promoteForExplicitDrain: mocks.promoteForExplicitDrain,
  },
}));

vi.mock('../services/FlowChatManager', () => ({
  FlowChatManager: {
    getInstance: () => ({
      drainPendingQueueForSession: mocks.drainPendingQueueForSession,
    }),
  },
}));

vi.mock('../services/interruptedTurnRecoveryGate', () => ({
  interruptedTurnRecoveryGate: {
    subscribe: () => () => undefined,
    getSnapshot: () => 0,
    isSessionInFlight: () => false,
  },
}));

vi.mock('../services/flow-chat-manager/EventHandlerModule', () => ({
  insertSteeringItemIfAbsent: mocks.insertSteeringItemIfAbsent,
}));

vi.mock('../../shared/notification-system', () => ({
  notificationService: {
    warning: vi.fn(),
    error: vi.fn(),
  },
}));

vi.mock('@/shared/utils/logger', () => ({
  createLogger: () => ({
    warn: vi.fn(),
    error: vi.fn(),
  }),
}));

vi.mock('../utils/acpSession', () => ({
  isAcpFlowSession: () => false,
}));

describe('PendingQueuePanel', () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    globalThis.IS_REACT_ACT_ENVIRONMENT = true;
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
    mocks.drainPendingQueueForSession.mockResolvedValue(undefined);
  });

  afterEach(() => {
    act(() => root.unmount());
    container.remove();
    vi.clearAllMocks();
  });

  it('submits only one steering request while send-now is in flight', async () => {
    let resolveSteering: ((value: { steeringId: string }) => void) | undefined;
    mocks.steerDialogTurn.mockImplementation(
      () => new Promise(resolve => {
        resolveSteering = resolve;
      }),
    );

    await act(async () => {
      root.render(<PendingQueuePanel sessionId="session-1" />);
    });
    const sendNowButton = container.querySelector<HTMLButtonElement>(
      'button[aria-label="pendingQueue.actions.sendNow"]',
    );
    expect(sendNowButton).not.toBeNull();

    act(() => {
      sendNowButton!.click();
      sendNowButton!.click();
    });

    expect(mocks.steerDialogTurn).toHaveBeenCalledTimes(1);
    expect(mocks.queueSetStatus).toHaveBeenCalledTimes(1);

    await act(async () => {
      resolveSteering?.({ steeringId: 'steering-1' });
      await Promise.resolve();
    });
  });
});
