// @vitest-environment jsdom

import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { FlowChatHeader, type FlowChatHeaderProps } from './FlowChatHeader';

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, values?: Record<string, unknown>) => {
      if (key === 'flowChatHeader.turnBadge') {
        return `Turn ${values?.current ?? ''}`;
      }
      return key;
    },
  }),
}));

vi.mock('@/component-library', async () => {
  const ReactModule = await import('react');

  return {
    Tooltip: ({ children }: { children: React.ReactNode }) => (
      <ReactModule.Fragment>{children}</ReactModule.Fragment>
    ),
    IconButton: ({
      children,
      size,
      tooltip,
      variant,
      ...props
    }: React.ButtonHTMLAttributes<HTMLButtonElement> & {
      size?: string;
      tooltip?: string;
      variant?: string;
    }) => (
      <button type="button" title={tooltip} {...props}>
        {children}
      </button>
    ),
    Input: ReactModule.forwardRef<HTMLInputElement, React.InputHTMLAttributes<HTMLInputElement>>((props, ref) => (
      <input ref={ref} {...props} />
    )),
  };
});

vi.mock('@/infrastructure/contexts/WorkspaceContext', () => ({
  useWorkspaceContext: () => ({
    currentWorkspace: { rootPath: '/workspace' },
  }),
}));

vi.mock('@/shared/utils/tabUtils', () => ({
  createReviewPlatformTab: vi.fn(),
}));

vi.mock('./SessionFilesBadge', () => ({
  SessionFilesBadge: () => <div data-testid="session-files-badge" />,
}));

function createProps(overrides: Partial<FlowChatHeaderProps> = {}): FlowChatHeaderProps {
  return {
    currentTurn: 1,
    totalTurns: 2,
    currentUserMessage: 'First prompt',
    visible: true,
    turns: [
      { turnId: 'turn-1', turnIndex: 1, title: 'First prompt' },
      { turnId: 'turn-2', turnIndex: 2, title: 'Second prompt' },
    ],
    onJumpToTurn: vi.fn(),
    ...overrides,
  };
}

describe('FlowChatHeader', () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
  });

  afterEach(() => {
    act(() => {
      root.unmount();
    });
    container.remove();
    vi.restoreAllMocks();
  });

  it('reserves the larger action group width on both sides of the centered title', () => {
    vi.spyOn(HTMLElement.prototype, 'getBoundingClientRect').mockImplementation(function (
      this: HTMLElement,
    ) {
      const width = this.classList.contains('flowchat-header__actions--left')
        ? 32
        : this.classList.contains('flowchat-header__actions')
          ? 196
          : 0;

      return {
        x: 0,
        y: 0,
        width,
        height: 36,
        top: 0,
        right: width,
        bottom: 36,
        left: 0,
        toJSON: () => ({}),
      };
    });

    act(() => {
      root.render(<FlowChatHeader {...createProps()} visible={false} totalTurns={0} />);
    });

    expect(container.querySelector('.flowchat-header')).toBeNull();

    act(() => {
      root.render(<FlowChatHeader {...createProps()} />);
    });

    const header = container.querySelector<HTMLElement>('.flowchat-header');
    expect(header?.style.getPropertyValue('--flowchat-header-side-width')).toBe('196px');
  });

  it('closes the turn list as soon as a different turn selection is accepted', () => {
    const onJumpToTurn = vi.fn(() => true);
    const initialProps = createProps({ onJumpToTurn });

    act(() => {
      root.render(<FlowChatHeader {...initialProps} />);
    });

    const turnListButton = container.querySelector<HTMLButtonElement>('[data-testid="flowchat-header-turn-list"]');
    expect(turnListButton).not.toBeNull();

    act(() => {
      turnListButton?.click();
    });

    expect(container.querySelector('[role="dialog"]')).not.toBeNull();

    const turnItems = Array.from(container.querySelectorAll<HTMLButtonElement>('.flowchat-header__turn-list-item'));
    expect(turnItems).toHaveLength(2);

    act(() => {
      turnItems[1]?.click();
    });

    expect(onJumpToTurn).toHaveBeenCalledWith('turn-2');
    expect(container.querySelector('[role="dialog"]')).toBeNull();

    act(() => {
      root.render(<FlowChatHeader {...initialProps} currentTurn={2} />);
    });

    expect(container.querySelector('[role="dialog"]')).toBeNull();
  });

  it('closes the turn list and notifies the container when selecting the current turn', () => {
    const onJumpToTurn = vi.fn(() => true);

    act(() => {
      root.render(<FlowChatHeader {...createProps({ onJumpToTurn })} />);
    });

    const turnListButton = container.querySelector<HTMLButtonElement>('[data-testid="flowchat-header-turn-list"]');
    act(() => {
      turnListButton?.click();
    });

    const currentTurnItem = container.querySelector<HTMLButtonElement>('.flowchat-header__turn-list-item');
    act(() => {
      currentTurnItem?.click();
    });

    expect(onJumpToTurn).toHaveBeenCalledWith('turn-1');
    expect(container.querySelector('[role="dialog"]')).toBeNull();
  });

  it('keeps the turn list open when the container rejects the selection', () => {
    const onJumpToTurn = vi.fn(() => false);

    act(() => {
      root.render(<FlowChatHeader {...createProps({ onJumpToTurn })} />);
    });

    const turnListButton = container.querySelector<HTMLButtonElement>('[data-testid="flowchat-header-turn-list"]');
    act(() => {
      turnListButton?.click();
    });

    const turnItems = Array.from(container.querySelectorAll<HTMLButtonElement>('.flowchat-header__turn-list-item'));
    act(() => {
      turnItems[1]?.click();
    });

    expect(onJumpToTurn).toHaveBeenCalledWith('turn-2');
    expect(container.querySelector('[role="dialog"]')).not.toBeNull();
  });

  it('renders background activity menus in a portal outside the scrollable panel', () => {
    const onStopBackgroundCommand = vi.fn();

    act(() => {
      root.render(
        <FlowChatHeader
          {...createProps({
            backgroundCommands: [{
              execSessionKey: 'command-1',
              execSessionId: 1,
              title: 'Long-running command',
              command: 'pnpm dev',
              status: 'running',
            }],
            onStopBackgroundCommand,
          })}
        />,
      );
    });

    const activityButton = container.querySelector<HTMLButtonElement>(
      '[data-testid="flowchat-header-background-activities"]',
    );
    act(() => {
      activityButton?.click();
    });

    const panel = container.querySelector('.flowchat-header__background-activity-panel');
    const menuButton = panel?.querySelector<HTMLButtonElement>(
      '[aria-label="flowChatHeader.backgroundCommandActions"]',
    );
    act(() => {
      menuButton?.click();
    });

    const menu = document.querySelector<HTMLDivElement>('[data-testid="flowchat-header-background-menu"]');
    expect(menu).not.toBeNull();
    expect(panel?.contains(menu ?? null)).toBe(false);

    act(() => {
      menu?.dispatchEvent(new MouseEvent('mousedown', { bubbles: true }));
    });
    expect(container.querySelector('.flowchat-header__background-activity-panel')).not.toBeNull();

    const stopButton = menu?.querySelector<HTMLButtonElement>('[role="menuitem"]');
    act(() => {
      stopButton?.click();
    });

    expect(onStopBackgroundCommand).toHaveBeenCalledTimes(1);
  });
});
