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
    IconButton: ReactModule.forwardRef<HTMLButtonElement, React.ButtonHTMLAttributes<HTMLButtonElement> & {
      size?: string;
      tooltip?: string;
      variant?: string;
    }>(({
      children,
      size,
      tooltip,
      variant,
      ...props
    }, ref) => (
      <button ref={ref} type="button" title={tooltip} {...props}>
        {children}
      </button>
    )),
    Input: ReactModule.forwardRef<HTMLInputElement, React.InputHTMLAttributes<HTMLInputElement>>((props, ref) => (
      <input ref={ref} {...props} />
    )),
    DotMatrixLoader: () => <span data-testid="dot-matrix-loader" />,
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

vi.mock('./SessionTreePopover', () => ({
  SessionTreePopover: () => (
    <div className="session-tree-popover">
      <button type="button" data-testid="flowchat-header-session-tree" />
    </div>
  ),
}));

function createProps(overrides: Partial<FlowChatHeaderProps> = {}): FlowChatHeaderProps {
  return {
    currentTurn: 1,
    totalTurns: 2,
    currentUserMessage: 'First prompt',
    visible: true,
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
    document.querySelector('[data-bf-overlay-host="true"]')?.remove();
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
    expect(header?.style.getPropertyValue('--bf-appearance-token-flowchat-header-side-width')).toBe('196px');
  });

  it('omits the list, previous-turn, and next-turn navigation controls', () => {
    act(() => {
      root.render(<FlowChatHeader {...createProps()} />);
    });

    expect(container.querySelector('[data-testid="flowchat-header-turn-list"]')).toBeNull();
    expect(container.querySelector('[data-testid="flowchat-header-turn-prev"]')).toBeNull();
    expect(container.querySelector('[data-testid="flowchat-header-turn-next"]')).toBeNull();
  });

  it('places the Agent tree entry immediately before background commands', () => {
    act(() => {
      root.render(<FlowChatHeader {...createProps({ sessionId: 'session-1' })} />);
    });

    const treeButton = container.querySelector('[data-testid="flowchat-header-session-tree"]');
    const commandButton = container.querySelector('[data-testid="flowchat-header-background-commands"]');
    const treeContainer = treeButton?.closest('.session-tree-popover');
    const commandContainer = commandButton?.closest('.flowchat-header__background-command-nav');

    expect(treeContainer?.nextElementSibling).toBe(commandContainer);
  });

  it('opens the background command panel when no commands exist', () => {
    act(() => {
      root.render(<FlowChatHeader {...createProps()} />);
    });

    const commandButton = container.querySelector<HTMLButtonElement>(
      '[data-testid="flowchat-header-background-commands"]',
    );
    expect(commandButton?.disabled).toBe(false);

    act(() => {
      commandButton?.click();
    });

    const panel = document.querySelector<HTMLElement>('.flowchat-header__background-command-panel');
    expect(panel?.textContent).toContain('flowChatHeader.backgroundCommandEmpty');
    expect(panel?.parentElement?.getAttribute('data-bf-overlay-host')).toBe('true');
    expect(panel?.style.visibility).toBe('visible');
  });

  it('renders background command menus in a portal outside the scrollable panel', () => {
    const onStopBackgroundCommand = vi.fn();
    const onStopAllBackgroundCommands = vi.fn();

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
            onStopAllBackgroundCommands,
          })}
        />,
      );
    });

    const commandButton = container.querySelector<HTMLButtonElement>(
      '[data-testid="flowchat-header-background-commands"]',
    );
    act(() => {
      commandButton?.click();
    });

    const panel = document.querySelector('.flowchat-header__background-command-panel');
    const menuButton = panel?.querySelector<HTMLButtonElement>(
      '.flowchat-header__background-command-panel-header-actions [aria-label="flowChatHeader.backgroundCommandActions"]',
    );
    expect(panel?.querySelector('.flowchat-header__background-section-title')).toBeNull();
    expect(menuButton?.closest('.flowchat-header__background-command-panel-header')).not.toBeNull();
    act(() => {
      menuButton?.click();
    });

    const menu = document.querySelector<HTMLDivElement>('[data-testid="flowchat-header-background-menu"]');
    expect(menu).not.toBeNull();
    expect(panel?.contains(menu ?? null)).toBe(false);
    expect(menu?.classList.contains('flowchat-header__background-command-menu--portal')).toBe(true);

    act(() => {
      menu?.dispatchEvent(new MouseEvent('mousedown', { bubbles: true }));
    });
    expect(document.querySelector('.flowchat-header__background-command-panel')).not.toBeNull();

    const stopButton = menu?.querySelector<HTMLButtonElement>('[role="menuitem"]');
    act(() => {
      stopButton?.click();
    });

    expect(onStopAllBackgroundCommands).toHaveBeenCalledTimes(1);
  });
});
