/**
 * @vitest-environment jsdom
 */

import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { ChatInputWorkspaceStrip } from './ChatInputWorkspaceStrip';

const mocks = vi.hoisted(() => ({
  refreshBasic: vi.fn(async () => undefined),
  useGitState: vi.fn(() => ({
    currentBranch: 'main',
    isRepository: true,
    refreshBasic: vi.fn(async () => undefined),
  })),
}));

vi.mock('react-i18next', () => ({
  initReactI18next: {
    type: '3rdParty',
    init: vi.fn(),
  },
  useTranslation: () => ({
    t: (key: string, options?: { defaultValue?: string }) => ({
      'deepReviewConsent.strategyLabels.normal': 'Standard',
      'reasoningSelector.auto': 'Auto',
      'chatInput.permissionMode.ask.label': 'Ask',
    } as Record<string, string>)[key] ?? options?.defaultValue ?? key,
  }),
}));

vi.mock('@/component-library', () => ({
  IconButton: ({
    children,
    onClick,
    className,
    'data-testid': testId,
  }: {
    children: React.ReactNode;
    onClick?: () => void;
    className?: string;
    'data-testid'?: string;
  }) => (
    <button type="button" className={className} data-testid={testId} onClick={onClick}>{children}</button>
  ),
  Tooltip: ({ children }: { children: React.ReactNode }) => <>{children}</>,
}));

vi.mock('@/tools/git/hooks/useGitState', () => ({
  useGitState: mocks.useGitState,
}));

// The real picker pulls in account state, SSH dialogs and a lazy remote-connect
// route. This suite only asserts whether the strip mounts it at all.
vi.mock('@/features/dispatch/DispatchTargetPicker', () => ({
  DispatchTargetPicker: ({ locked }: { locked: boolean }) => (
    <div data-testid="chat-input-dispatch-trigger" data-locked={locked ? 'true' : 'false'} />
  ),
}));

describe('ChatInputWorkspaceStrip git refresh behavior', () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    (globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
    mocks.useGitState.mockClear();
    mocks.useGitState.mockReturnValue({
      currentBranch: 'main',
      isRepository: true,
      refreshBasic: mocks.refreshBasic,
    });
  });

  afterEach(() => {
    act(() => {
      root.unmount();
    });
    container.remove();
    document.querySelector('[data-bf-overlay-host="true"]')?.remove();
    vi.clearAllMocks();
  });

  it('uses cached git state without passive refresh while historical restore is pending', async () => {
    await act(async () => {
      root.render(
        <ChatInputWorkspaceStrip
          repositoryPath="D:/workspace/BitFun"
          workspaceLabel="BitFun"
          deferPassiveGitRefresh
        />
      );
    });

    expect(mocks.useGitState).toHaveBeenCalledWith(expect.objectContaining({
      repositoryPath: 'D:/workspace/BitFun',
      layers: ['basic'],
      isActive: false,
      refreshOnMount: false,
      refreshOnActive: false,
    }));
    expect(container.textContent).toContain('BitFun');
  });

  it('keeps passive git refresh enabled for normal sessions', async () => {
    await act(async () => {
      root.render(
        <ChatInputWorkspaceStrip
          repositoryPath="D:/workspace/BitFun"
          workspaceLabel="BitFun"
        />
      );
    });

    expect(mocks.useGitState).toHaveBeenCalledWith(expect.objectContaining({
      repositoryPath: 'D:/workspace/BitFun',
      isActive: true,
      refreshOnMount: true,
      refreshOnActive: false,
    }));
  });

  it('keeps reasoning intensity and context usage in one runtime group', async () => {
    let reasoningHost: HTMLDivElement | null = null;
    await act(async () => {
      root.render(
        <ChatInputWorkspaceStrip
          repositoryPath="D:/workspace/BitFun"
          workspaceLabel="BitFun"
          harnessControl={{ onActivateBalanced: vi.fn() }}
          reasoningControl={{
            visible: true,
            hostRef: node => { reasoningHost = node; },
          }}
          usageReport={{ visible: true, percentage: 12, onOpen: vi.fn() }}
        />
      );
    });

    expect(container.textContent).toContain('BitFun');
    expect(container.textContent).not.toContain('chatInput.harness.menuTitle');
    expect(container.textContent).toContain('12%');
    expect(container.querySelector('[data-bf-part="harness"]')).not.toBeNull();
    expect(container.querySelector('.bitfun-chat-input-workspace-strip__goal-summary--inactive')).not.toBeNull();
    const runtime = container.querySelector<HTMLElement>('[data-bf-part="runtime"]');
    expect(runtime).not.toBeNull();
    expect(reasoningHost).not.toBeNull();
    expect(runtime?.contains(reasoningHost)).toBe(true);
    expect(runtime?.querySelector('[data-bf-part="usageAction"]')).not.toBeNull();
    expect(runtime?.querySelector('.bitfun-chat-input-workspace-strip__runtime-divider')).not.toBeNull();
  });

  it('keeps the three-tier Harness list but drops the heavy menu chrome', async () => {
    await act(async () => {
      root.render(
        <ChatInputWorkspaceStrip
          repositoryPath="D:/workspace/BitFun"
          workspaceLabel="BitFun"
          harnessControl={{ onActivateBalanced: vi.fn() }}
        />
      );
    });

    await act(async () => {
      container.querySelector<HTMLButtonElement>('[data-testid="harness-profile-selector"]')
        ?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
    });

    const menu = document.querySelector<HTMLElement>('.bitfun-harness-selector__menu');
    expect(menu).not.toBeNull();
    expect(menu?.querySelectorAll('[data-bf-part="profile"]').length).toBe(3);
    // Simplified list: no header, hint, footer, icons, or descriptions.
    expect(menu?.querySelector('.bitfun-harness-selector__header')).toBeNull();
    expect(menu?.querySelector('.bitfun-harness-selector__footer')).toBeNull();
    expect(menu?.querySelector('.bitfun-harness-selector__profile-icon')).toBeNull();
    expect(menu?.querySelector('.bitfun-harness-selector__profile-description')).toBeNull();
    expect(menu?.querySelector('[data-bf-part="profile"][data-bf-profile="balanced"]'))
      .not.toBeNull();
  });

  it('activates the Balanced harness from the simplified list', async () => {
    const onActivateBalanced = vi.fn();
    await act(async () => {
      root.render(
        <ChatInputWorkspaceStrip
          repositoryPath="D:/workspace/BitFun"
          workspaceLabel="BitFun"
          harnessControl={{ onActivateBalanced }}
        />
      );
    });

    await act(async () => {
      container.querySelector<HTMLButtonElement>('[data-testid="harness-profile-selector"]')
        ?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
    });
    const balancedRow = document.querySelector<HTMLButtonElement>(
      '[data-bf-part="profile"][data-bf-profile="balanced"]',
    );
    expect(balancedRow?.dataset.bfState).toBe('current');

    await act(async () => {
      balancedRow?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
    });
    expect(onActivateBalanced).toHaveBeenCalledTimes(1);
    expect(document.querySelector('.bitfun-harness-selector__menu')).toBeNull();
  });

  it('shows goal text only while the goal is active', async () => {
    await act(async () => {
      root.render(
        <ChatInputWorkspaceStrip
          repositoryPath="D:/workspace/BitFun"
          workspaceLabel="BitFun"
          harnessControl={{ onActivateBalanced: vi.fn() }}
          threadGoal={{
            visible: true,
            goal: null,
            onOpen: vi.fn(),
          }}
        />
      );
    });

    const goalButton = container.querySelector<HTMLElement>('[data-testid="thread-goal-strip-button"]');
    expect(goalButton?.textContent).toBe('');
    expect(goalButton?.classList.contains('bitfun-chat-input-workspace-strip__goal-btn--none')).toBe(true);

    await act(async () => {
      root.render(
        <ChatInputWorkspaceStrip
          repositoryPath="D:/workspace/BitFun"
          workspaceLabel="BitFun"
          harnessControl={{ onActivateBalanced: vi.fn() }}
          threadGoal={{
            visible: true,
            goal: { objective: 'Optimize input interaction', status: 'active' },
            onOpen: vi.fn(),
          }}
        />
      );
    });

    expect(
      container.querySelector<HTMLElement>('[data-testid="thread-goal-strip-button"]')?.textContent,
    ).toBe('Optimize input interaction');
  });

  it('places automatic approval immediately after the Harness profile', async () => {
    await act(async () => {
      root.render(
        <ChatInputWorkspaceStrip
          repositoryPath="D:/workspace/BitFun"
          workspaceLabel="BitFun"
          harnessControl={{ onActivateBalanced: vi.fn() }}
          permissionControl={{ mode: 'auto', onChange: vi.fn() }}
        />
      );
    });

    const harness = container.querySelector<HTMLElement>('[data-bf-part="harness"]');
    const trigger = container.querySelector<HTMLElement>(
      '[data-testid="chat-input-permission-trigger"]',
    );
    expect(trigger?.textContent).toContain('chatInput.permissionMode.auto.label');
    expect(harness?.contains(trigger ?? null)).toBe(true);
    expect(
      container.querySelector('.bitfun-chat-input-workspace-strip__actions--portal-only'),
    ).not.toBeNull();
  });

  it('keeps an ask-mode permission entry visible and switches from its menu', async () => {
    const onChange = vi.fn();
    const onHide = vi.fn();
    await act(async () => {
      root.render(
        <ChatInputWorkspaceStrip
          repositoryPath=""
          workspaceLabel=""
          permissionControl={{ mode: 'ask', onChange, onHide }}
        />
      );
    });

    const trigger = container.querySelector<HTMLButtonElement>('[data-testid="chat-input-permission-trigger"]');
    expect(trigger?.dataset.permissionMode).toBe('ask');
    expect(trigger?.textContent).toContain('Ask');

    await act(async () => {
      trigger?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
    });
    const permissionMenu = document.querySelector<HTMLElement>(
      '[data-testid="chat-input-permission-menu"]',
    );
    expect(permissionMenu).not.toBeNull();
    expect(permissionMenu?.style.visibility).toBe('visible');

    await act(async () => {
      document
        .querySelector<HTMLButtonElement>('[data-testid="chat-input-permission-option-auto"]')
        ?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
    });
    expect(onChange).toHaveBeenCalledWith('auto');
    expect(document.querySelector('[data-testid="chat-input-permission-menu"]')).toBeNull();

    await act(async () => {
      trigger?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
    });
    await act(async () => {
      document
        .querySelector<HTMLButtonElement>('[data-testid="chat-input-permission-hide-control"]')
        ?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
    });
    expect(onHide).toHaveBeenCalledOnce();
    expect(document.querySelector('[data-testid="chat-input-permission-menu"]')).toBeNull();
  });

  it('chooses the scope per click instead of through a separate toggle', async () => {
    const onChange = vi.fn();
    const onChangeForNextTurn = vi.fn();
    await act(async () => {
      root.render(
        <ChatInputWorkspaceStrip
          repositoryPath=""
          workspaceLabel=""
          permissionControl={{
            mode: 'ask',
            nextTurnMode: null,
            onChange,
            onChangeForNextTurn,
          }}
        />
      );
    });

    const trigger = container.querySelector<HTMLButtonElement>(
      '[data-testid="chat-input-permission-trigger"]',
    );
    await act(async () => {
      trigger?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
    });

    // The row body is the session scope.
    await act(async () => {
      document
        .querySelector<HTMLButtonElement>('[data-testid="chat-input-permission-option-auto"]')
        ?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
    });
    expect(onChange).toHaveBeenCalledWith('auto');
    expect(onChangeForNextTurn).not.toHaveBeenCalled();

    // The trailing button is the one-off scope, and never writes the session.
    await act(async () => {
      trigger?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
    });
    await act(async () => {
      document
        .querySelector<HTMLButtonElement>(
          '[data-testid="chat-input-permission-next-turn-full_access"]',
        )
        ?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
    });
    expect(onChangeForNextTurn).toHaveBeenCalledWith('full_access');
    expect(onChange).toHaveBeenCalledTimes(1);
  });

  it('keeps the mode descriptions out of the row and in the accessible name', async () => {
    await act(async () => {
      root.render(
        <ChatInputWorkspaceStrip
          repositoryPath=""
          workspaceLabel=""
          permissionControl={{ mode: 'ask', onChange: vi.fn(), onChangeForNextTurn: vi.fn() }}
        />
      );
    });
    await act(async () => {
      container
        .querySelector<HTMLButtonElement>('[data-testid="chat-input-permission-trigger"]')
        ?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
    });

    const option = document.querySelector<HTMLButtonElement>(
      '[data-testid="chat-input-permission-option-ask"]',
    );
    // The row itself stays single-line; the description lives in the tooltip.
    expect(option?.textContent).toBe('Ask');
    expect(option?.getAttribute('aria-label')).toContain(
      'chatInput.permissionMode.ask.description',
    );
  });

  it('marks the armed one-off mode and omits the affordance without a handler', async () => {
    await act(async () => {
      root.render(
        <ChatInputWorkspaceStrip
          repositoryPath=""
          workspaceLabel=""
          permissionControl={{
            mode: 'full_access',
            nextTurnMode: 'full_access',
            overridden: true,
            onChange: vi.fn(),
            onChangeForNextTurn: vi.fn(),
          }}
        />
      );
    });

    const trigger = container.querySelector<HTMLButtonElement>(
      '[data-testid="chat-input-permission-trigger"]',
    );
    expect(trigger?.dataset.permissionOverridden).toBe('true');
    await act(async () => {
      trigger?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
    });
    expect(
      document
        .querySelector('[data-testid="chat-input-permission-next-turn-full_access"]')
        ?.getAttribute('aria-checked'),
    ).toBe('true');
    expect(
      document
        .querySelector('[data-testid="chat-input-permission-next-turn-ask"]')
        ?.getAttribute('aria-checked'),
    ).toBe('false');

    // Surfaces without a one-off handler (dispatch, ACP) keep the plain rows.
    await act(async () => {
      root.render(
        <ChatInputWorkspaceStrip
          repositoryPath=""
          workspaceLabel=""
          permissionControl={{ mode: 'ask', onChange: vi.fn() }}
        />
      );
    });
    await act(async () => {
      container
        .querySelector<HTMLButtonElement>('[data-testid="chat-input-permission-trigger"]')
        ?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
    });
    expect(
      document.querySelector('[data-testid="chat-input-permission-next-turn-ask"]'),
    ).toBeNull();
  });

  it('separates the session checkmark from an armed one-off override', async () => {
    await act(async () => {
      root.render(
        <ChatInputWorkspaceStrip
          repositoryPath=""
          workspaceLabel=""
          permissionControl={{
            // Session runs on auto; only the next message is full access.
            mode: 'auto',
            nextTurnMode: 'full_access',
            overridden: true,
            onChange: vi.fn(),
            onChangeForNextTurn: vi.fn(),
          }}
        />
      );
    });

    // The trigger reports what the next submission will run with.
    const trigger = container.querySelector<HTMLButtonElement>(
      '[data-testid="chat-input-permission-trigger"]',
    );
    expect(trigger?.dataset.permissionMode).toBe('full_access');

    await act(async () => {
      trigger?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
    });

    // The checkmark stays on the session's own mode, not the armed one.
    expect(
      document.querySelector('[data-testid="chat-input-permission-selected-auto"]'),
    ).not.toBeNull();
    expect(
      document.querySelector('[data-testid="chat-input-permission-selected-full_access"]'),
    ).toBeNull();
    expect(
      document
        .querySelector('[data-testid="chat-input-permission-next-turn-full_access"]')
        ?.getAttribute('aria-checked'),
    ).toBe('true');
  });

  it('marks a session-scoped override and offers a reset to the default', async () => {
    const onResetToDefault = vi.fn();
    await act(async () => {
      root.render(
        <ChatInputWorkspaceStrip
          repositoryPath=""
          workspaceLabel=""
          permissionControl={{
            mode: 'full_access',
            overridden: true,
            scopeLabel: 'This session',
            onChange: vi.fn(),
            onResetToDefault,
          }}
        />
      );
    });

    const trigger = container.querySelector<HTMLButtonElement>(
      '[data-testid="chat-input-permission-trigger"]',
    );
    expect(trigger?.dataset.permissionOverridden).toBe('true');
    // A session-level choice is not pending, so it gets no dot; the trigger
    // label already names the mode it runs with.
    expect(
      container.querySelector('[data-testid="chat-input-permission-next-turn-dot"]'),
    ).toBeNull();

    await act(async () => {
      trigger?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
    });
    expect(document.body.textContent).toContain('This session');

    await act(async () => {
      document
        .querySelector<HTMLButtonElement>('[data-testid="chat-input-permission-reset-default"]')
        ?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
    });
    expect(onResetToDefault).toHaveBeenCalledOnce();
  });

  it('dots the trigger only while a one-off override is pending', async () => {
    const onOpenDefaultSettings = vi.fn();
    await act(async () => {
      root.render(
        <ChatInputWorkspaceStrip
          repositoryPath=""
          workspaceLabel=""
          permissionControl={{
            mode: 'auto',
            nextTurnMode: 'full_access',
            overridden: true,
            onChange: vi.fn(),
            onChangeForNextTurn: vi.fn(),
            onResetToDefault: vi.fn(),
            onOpenDefaultSettings,
          }}
        />
      );
    });

    const trigger = container.querySelector<HTMLButtonElement>(
      '[data-testid="chat-input-permission-trigger"]',
    );
    expect(trigger?.dataset.permissionNextTurn).toBe('true');
    expect(
      container.querySelector('[data-testid="chat-input-permission-next-turn-dot"]'),
    ).not.toBeNull();

    // The reset row reaches the settings page that owns the default.
    await act(async () => {
      trigger?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
    });
    await act(async () => {
      document
        .querySelector<HTMLButtonElement>(
          '[data-testid="chat-input-permission-open-default-settings"]',
        )
        ?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
    });
    expect(onOpenDefaultSettings).toHaveBeenCalledOnce();
  });

  it('hides the override affordances when the session follows the default', async () => {
    await act(async () => {
      root.render(
        <ChatInputWorkspaceStrip
          repositoryPath=""
          workspaceLabel=""
          permissionControl={{
            mode: 'ask',
            overridden: false,
            onChange: vi.fn(),
            onResetToDefault: vi.fn(),
          }}
        />
      );
    });

    const trigger = container.querySelector<HTMLButtonElement>(
      '[data-testid="chat-input-permission-trigger"]',
    );
    expect(trigger?.dataset.permissionOverridden).toBeUndefined();
    expect(
      container.querySelector('[data-testid="chat-input-permission-next-turn-dot"]'),
    ).toBeNull();

    await act(async () => {
      trigger?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
    });
    expect(
      document.querySelector('[data-testid="chat-input-permission-reset-default"]'),
    ).toBeNull();
  });

  it('shows ACP ownership without exposing native permission choices', async () => {
    await act(async () => {
      root.render(
        <ChatInputWorkspaceStrip
          repositoryPath="D:/workspace/BitFun"
          workspaceLabel="BitFun"
          permissionControl={{ mode: 'acp' }}
        />
      );
    });

    const trigger = container.querySelector<HTMLButtonElement>('[data-testid="chat-input-permission-trigger"]');
    expect(trigger?.disabled).toBe(true);
    expect(trigger?.dataset.permissionMode).toBe('acp');
    expect(container.querySelector('[data-testid="chat-input-permission-menu"]')).toBeNull();
  });

  it('keeps the ACP strip groups in one row with the policy and runtime column layout', async () => {
    await act(async () => {
      root.render(
        <ChatInputWorkspaceStrip
          repositoryPath="D:/workspace/BitFun"
          workspaceLabel="BitFun"
          permissionControl={{ mode: 'acp' }}
          usageReport={{ visible: true, percentage: 42, onOpen: vi.fn() }}
        />
      );
    });

    const strip = container.querySelector<HTMLElement>('[data-testid="chat-input-workspace-strip"]');
    expect(strip).not.toBeNull();
    // ACP sessions have no Harness group, but the approval and runtime groups
    // must still be placed on the strip's single grid row instead of stacking
    // into additional lines below the workspace breadcrumb.
    expect(strip?.className).toContain('bitfun-chat-input-workspace-strip--with-workspace');
    expect(strip?.className).toContain('bitfun-chat-input-workspace-strip--with-policy');
    expect(strip?.className).toContain('bitfun-chat-input-workspace-strip--with-runtime');
    expect(strip?.className).not.toContain('bitfun-chat-input-workspace-strip--with-harness');
    expect(strip?.querySelector('[data-bf-part="main"]')).not.toBeNull();
    expect(strip?.querySelector('[data-bf-part="actions"]')).not.toBeNull();
    expect(strip?.querySelector('[data-bf-part="runtime"]')).not.toBeNull();
  });

  it('keeps the ACP strip on a single row without a runtime group', async () => {
    await act(async () => {
      root.render(
        <ChatInputWorkspaceStrip
          repositoryPath="D:/workspace/BitFun"
          workspaceLabel="BitFun"
          permissionControl={{ mode: 'acp' }}
        />
      );
    });

    const strip = container.querySelector<HTMLElement>('[data-testid="chat-input-workspace-strip"]');
    expect(strip?.className).toContain('bitfun-chat-input-workspace-strip--with-workspace');
    expect(strip?.className).toContain('bitfun-chat-input-workspace-strip--with-policy');
    expect(strip?.className).not.toContain('bitfun-chat-input-workspace-strip--with-runtime');
    expect(strip?.className).not.toContain('bitfun-chat-input-workspace-strip--with-harness');
  });

  it('reuses the permission control with dispatch-scoped choices', async () => {
    const onChange = vi.fn();
    await act(async () => {
      root.render(
        <ChatInputWorkspaceStrip
          repositoryPath="/repo"
          workspaceLabel="repo"
          permissionControl={{
            mode: 'reject',
            options: ['ask', 'auto', 'reject'],
            scopeLabel: 'This dispatched session',
            onChange,
          }}
        />
      );
    });

    const trigger = container.querySelector<HTMLButtonElement>(
      '[data-testid="chat-input-permission-trigger"]',
    );
    expect(trigger?.dataset.permissionMode).toBe('reject');
    await act(async () => {
      trigger?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
    });
    expect(document.body.textContent).toContain('This dispatched session');
    expect(document.querySelector(
      '[data-testid="chat-input-permission-option-full_access"]',
    )).toBeNull();

    await act(async () => {
      document.querySelector<HTMLButtonElement>(
        '[data-testid="chat-input-permission-option-auto"]',
      )?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
    });
    expect(onChange).toHaveBeenCalledWith('auto');
  });

  it('offers the worktree toggle for a Git workspace and reports the new state', async () => {
    const onChange = vi.fn(async () => undefined);
    await act(async () => {
      root.render(
        <ChatInputWorkspaceStrip
          repositoryPath="/repo"
          workspaceLabel="repo"
          worktreeControl={{ enabled: false, locked: false, onChange }}
        />
      );
    });

    const toggle = container.querySelector<HTMLButtonElement>('[data-testid="chat-input-worktree-toggle"]');
    expect(toggle).not.toBeNull();
    expect(toggle?.dataset.worktreeEnabled).toBe('false');
    expect(toggle?.disabled).toBe(false);

    await act(async () => {
      toggle?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
    });
    expect(onChange).toHaveBeenCalledWith(true);
  });

  it('updates repeated clicks optimistically without waiting for Git work', async () => {
    const onChange = vi.fn();
    await act(async () => {
      root.render(
        <ChatInputWorkspaceStrip
          repositoryPath="/repo"
          workspaceLabel="repo"
          worktreeControl={{ enabled: false, locked: false, onChange }}
        />
      );
    });

    const toggle = container.querySelector<HTMLButtonElement>('[data-testid="chat-input-worktree-toggle"]');
    act(() => {
      toggle?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      toggle?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
    });

    expect(onChange).toHaveBeenNthCalledWith(1, true);
    expect(onChange).toHaveBeenNthCalledWith(2, false);
  });

  it('shows an armed worktree as checked before it is materialized', async () => {
    await act(async () => {
      root.render(
        <ChatInputWorkspaceStrip
          repositoryPath="/repo"
          workspaceLabel="repo"
          worktreeControl={{ enabled: true, locked: false, onChange: vi.fn() }}
        />
      );
    });

    const toggle = container.querySelector<HTMLButtonElement>('[data-testid="chat-input-worktree-toggle"]');
    expect(toggle?.dataset.worktreeEnabled).toBe('true');
    expect(toggle?.dataset.worktreeMaterialized).toBe('false');
  });

  it('shows the toggle as on inside a worktree and asks to turn it off', async () => {
    const onChange = vi.fn(async () => undefined);
    await act(async () => {
      root.render(
        <ChatInputWorkspaceStrip
          repositoryPath="/worktrees/wt-1"
          workspaceLabel="wt-1"
          executionTarget={{
            kind: 'managedWorktree',
            worktreeId: 'wt-1',
            rootPath: '/worktrees/wt-1',
            baseCommit: '0123456789abcdef',
            branch: 'bitfun/isolated',
            lifecycle: 'managed',
          }}
          worktreeControl={{ enabled: true, locked: false, onChange }}
        />
      );
    });

    const toggle = container.querySelector<HTMLButtonElement>('[data-testid="chat-input-worktree-toggle"]');
    expect(toggle?.dataset.worktreeEnabled).toBe('true');
    expect(container.textContent).toContain('bitfun/isolated');

    await act(async () => {
      toggle?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
    });
    expect(onChange).toHaveBeenCalledWith(false);
  });

  it('locks the toggle once the session has a transcript', async () => {
    const onChange = vi.fn(async () => undefined);
    await act(async () => {
      root.render(
        <ChatInputWorkspaceStrip
          repositoryPath="/repo"
          workspaceLabel="repo"
          worktreeControl={{ enabled: false, locked: true, onChange }}
        />
      );
    });

    const toggle = container.querySelector<HTMLButtonElement>('[data-testid="chat-input-worktree-toggle"]');
    expect(toggle?.disabled).toBe(true);

    await act(async () => {
      toggle?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
    });
    expect(onChange).not.toHaveBeenCalled();
  });

  it('refetches Git state when the execution root moves into a worktree', async () => {
    const onChange = vi.fn(async () => undefined);
    await act(async () => {
      root.render(
        <ChatInputWorkspaceStrip
          repositoryPath="/repo"
          workspaceLabel="repo"
          worktreeControl={{ enabled: false, locked: false, onChange }}
        />
      );
    });
    expect(mocks.refreshBasic).not.toHaveBeenCalled();

    await act(async () => {
      root.render(
        <ChatInputWorkspaceStrip
          repositoryPath="/worktrees/wt-1"
          workspaceLabel="repo"
          executionTarget={{
            kind: 'managedWorktree',
            worktreeId: 'wt-1',
            rootPath: '/worktrees/wt-1',
            lifecycle: 'managed',
          }}
          worktreeControl={{ enabled: true, locked: false, onChange }}
        />
      );
    });

    expect(mocks.refreshBasic).toHaveBeenCalled();
  });

  it('omits the toggle when the session cannot host a worktree', async () => {
    await act(async () => {
      root.render(
        <ChatInputWorkspaceStrip
          repositoryPath="/repo"
          workspaceLabel="repo"
        />
      );
    });

    expect(container.querySelector('[data-testid="chat-input-worktree-toggle"]')).toBeNull();
  });

  it('shows the dispatch picker and the worktree toggle together in a Git workspace', async () => {
    await act(async () => {
      root.render(
        <ChatInputWorkspaceStrip
          repositoryPath="/repo"
          workspaceLabel="repo"
          worktreeControl={{ enabled: false, locked: false, onChange: vi.fn() }}
          dispatchControl={{
            target: { kind: 'local' },
            locked: false,
            onSelectTarget: vi.fn(),
          }}
        />
      );
    });

    expect(container.querySelector('[data-testid="chat-input-worktree-toggle"]')).not.toBeNull();
    expect(container.querySelector('[data-testid="chat-input-dispatch-trigger"]')).not.toBeNull();
  });

  it('shows the dispatched branch instead of the source branch once dispatch is locked', async () => {
    await act(async () => {
      root.render(
        <ChatInputWorkspaceStrip
          repositoryPath="/repo"
          workspaceLabel="repo"
          worktreeControl={{
            enabled: true,
            locked: true,
            lockedReason: 'dispatch',
            onChange: vi.fn(),
          }}
          dispatchControl={{
            target: {
              kind: 'ssh',
              connectionId: 'ssh-1',
              workspacePath: '',
              displayName: 'build-host',
            },
            locked: true,
            branch: 'bitfun/dispatch/job-1',
            onSelectTarget: vi.fn(),
          }}
        />
      );
    });

    expect(container.textContent).toContain('bitfun/dispatch/job-1');
    expect(container.textContent).not.toContain('main');
  });

  it('keeps the local execution breadcrumb visible but locked outside a Git workspace', async () => {
    mocks.useGitState.mockReturnValue({
      currentBranch: '',
      isRepository: false,
      refreshBasic: mocks.refreshBasic,
    });

    await act(async () => {
      root.render(
        <ChatInputWorkspaceStrip
          repositoryPath="/plain-folder"
          workspaceLabel="plain-folder"
          worktreeControl={{ enabled: false, locked: false, onChange: vi.fn() }}
          dispatchControl={{
            target: { kind: 'local' },
            locked: false,
            onSelectTarget: vi.fn(),
          }}
        />
      );
    });

    expect(container.querySelector('[data-testid="chat-input-worktree-toggle"]')).toBeNull();
    const dispatchTrigger = container.querySelector<HTMLElement>(
      '[data-testid="chat-input-dispatch-trigger"]',
    );
    expect(dispatchTrigger).not.toBeNull();
    expect(dispatchTrigger?.dataset.locked).toBe('true');
  });
});
