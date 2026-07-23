/**
 * @vitest-environment jsdom
 */

import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { ChatInputWorkspaceStrip } from './ChatInputWorkspaceStrip';

const mocks = vi.hoisted(() => ({
  useGitState: vi.fn(() => ({
    currentBranch: 'main',
    isRepository: true,
  })),
}));

vi.mock('react-i18next', () => ({
  initReactI18next: {
    type: '3rdParty',
    init: vi.fn(),
  },
  useTranslation: () => ({
    t: (key: string) => key,
  }),
}));

vi.mock('@/component-library', () => ({
  IconButton: ({ children, onClick }: { children: React.ReactNode; onClick?: () => void }) => (
    <button type="button" onClick={onClick}>{children}</button>
  ),
  Tooltip: ({ children }: { children: React.ReactNode }) => <>{children}</>,
}));

vi.mock('@/tools/git/hooks/useGitState', () => ({
  useGitState: mocks.useGitState,
}));

describe('ChatInputWorkspaceStrip git refresh behavior', () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
    mocks.useGitState.mockClear();
    mocks.useGitState.mockReturnValue({
      currentBranch: 'main',
      isRepository: true,
    });
  });

  afterEach(() => {
    act(() => {
      root.unmount();
    });
    container.remove();
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

  it('keeps an ask-mode permission entry visible and switches from its menu', async () => {
    const onChange = vi.fn();
    await act(async () => {
      root.render(
        <ChatInputWorkspaceStrip
          repositoryPath=""
          workspaceLabel=""
          permissionControl={{ mode: 'ask', onChange }}
        />
      );
    });

    const trigger = container.querySelector<HTMLButtonElement>('[data-testid="chat-input-permission-trigger"]');
    expect(trigger?.dataset.permissionMode).toBe('ask');

    await act(async () => {
      trigger?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
    });
    expect(container.querySelector('[data-testid="chat-input-permission-menu"]')).not.toBeNull();

    await act(async () => {
      container
        .querySelector<HTMLButtonElement>('[data-testid="chat-input-permission-option-auto"]')
        ?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
    });
    expect(onChange).toHaveBeenCalledWith('auto');
    expect(container.querySelector('[data-testid="chat-input-permission-menu"]')).toBeNull();
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
});
