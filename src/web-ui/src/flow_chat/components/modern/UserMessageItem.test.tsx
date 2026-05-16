import React, { act } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { createRoot, type Root } from 'react-dom/client';
import { JSDOM } from 'jsdom';

import { FlowChatContext } from './FlowChatContext';
import { UserMessageItem } from './UserMessageItem';

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string) => {
      const labels: Record<string, string> = {
        'steering.statusPending': '等待触发',
        'steering.statusInjected': '已触发',
        'message.copy': '复制',
        'message.copyFailed': '复制失败',
      };
      return labels[key] ?? key;
    },
  }),
}));

vi.mock('../../store/modernFlowChatStore', () => ({
  useActiveSession: () => null,
}));

vi.mock('../../store/FlowChatStore', () => ({
  flowChatStore: {
    truncateDialogTurnsFrom: vi.fn(),
  },
}));

vi.mock('@/infrastructure/api', () => ({
  snapshotAPI: {
    rollbackToTurn: vi.fn(),
  },
}));

vi.mock('@/shared/notification-system', () => ({
  notificationService: {
    success: vi.fn(),
    error: vi.fn(),
  },
}));

vi.mock('@/infrastructure/event-bus', () => ({
  globalEventBus: {
    emit: vi.fn(),
  },
}));

vi.mock('@/component-library', () => ({
  ReproductionStepsBlock: ({ steps }: { steps: string }) => <div>{steps}</div>,
  Tooltip: ({ children }: { children: React.ReactNode }) => <>{children}</>,
  confirmDanger: vi.fn(),
}));

describe('UserMessageItem steering tag', () => {
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
    vi.stubGlobal('navigator', {
      clipboard: {
        writeText: vi.fn(),
      },
    });

    container = dom.window.document.getElementById('root') as HTMLDivElement;
    root = createRoot(container);
  });

  afterEach(() => {
    act(() => {
      root.unmount();
    });
    vi.unstubAllGlobals();
  });

  it('renders pending steering tag on the right side of the message row', () => {
    act(() => {
      root.render(
        <FlowChatContext.Provider value={{ allowUserMessageRollback: false }}>
          <UserMessageItem
            message={{
              id: 'user-steering-1',
              content: 'Please adjust this now',
              timestamp: 1000,
            }}
            turnId="turn-1"
            steeringStatus="pending"
          />
        </FlowChatContext.Provider>,
      );
    });

    const main = container.querySelector('.user-message-item__main');
    const tag = main?.querySelector('.user-message-item__steering-tag');

    expect(tag?.textContent).toBe('等待触发');
  });

  it('does not render a steering tag after steering is triggered', () => {
    act(() => {
      root.render(
        <FlowChatContext.Provider value={{ allowUserMessageRollback: false }}>
          <UserMessageItem
            message={{
              id: 'user-steering-1',
              content: 'Please adjust this now',
              timestamp: 1000,
            }}
            turnId="turn-1"
            steeringStatus="completed"
          />
        </FlowChatContext.Provider>,
      );
    });

    expect(container.querySelector('.user-message-item__steering-tag')).toBeNull();
  });
});
