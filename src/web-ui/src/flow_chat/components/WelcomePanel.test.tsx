// @vitest-environment jsdom

import React, { act } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { createRoot, type Root } from 'react-dom/client';
import { WelcomePanel } from './WelcomePanel';

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

const gitApiMock = vi.hoisted(() => ({
  isGitRepository: vi.fn(),
  getStatus: vi.fn(),
}));

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, values?: Record<string, unknown>) => values?.count ?? key,
  }),
}));

vi.mock('../../infrastructure/api', () => ({
  gitAPI: gitApiMock,
}));

vi.mock('../../app/hooks/useApp', () => ({
  useApp: () => ({
    switchLeftPanelTab: vi.fn(),
  }),
}));

vi.mock('@/infrastructure/contexts/WorkspaceContext', () => ({
  useWorkspaceContext: () => ({
    hasWorkspace: true,
    currentWorkspace: {
      id: 'workspace-1',
      name: 'BitFun',
      rootPath: 'D:/workspace/BitFun',
    },
    openedWorkspacesList: [],
    openWorkspace: vi.fn(),
    switchWorkspace: vi.fn(),
  }),
}));

vi.mock('./CoworkExampleCards', () => ({
  default: () => null,
}));

vi.mock('@/app/scenes/my-agent/useAgentIdentityDocument', () => ({
  useAgentIdentityDocument: () => ({ document: { name: '' } }),
}));

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (error: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

describe('WelcomePanel Git summary loading', () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
    gitApiMock.isGitRepository.mockReset();
    gitApiMock.getStatus.mockReset();
    gitApiMock.getStatus.mockResolvedValue({
      current_branch: 'main',
      staged: [],
      unstaged: [],
      untracked: [],
      ahead: 0,
      behind: 0,
    });
  });

  afterEach(() => {
    act(() => {
      root.unmount();
    });
    container.remove();
  });

  it('does not request Git status after the panel unmounts during repository detection', async () => {
    const repositoryProbe = deferred<boolean>();
    gitApiMock.isGitRepository.mockReturnValue(repositoryProbe.promise);

    await act(async () => {
      root.render(<WelcomePanel sessionMode="agentic" />);
    });

    expect(gitApiMock.isGitRepository).toHaveBeenCalledWith('D:/workspace/BitFun');

    act(() => {
      root.unmount();
    });

    await act(async () => {
      repositoryProbe.resolve(true);
      await repositoryProbe.promise;
    });

    expect(gitApiMock.getStatus).not.toHaveBeenCalled();
  });

  it('loads Git status when the panel remains mounted', async () => {
    gitApiMock.isGitRepository.mockResolvedValue(true);

    await act(async () => {
      root.render(<WelcomePanel sessionMode="agentic" />);
    });

    expect(gitApiMock.getStatus).toHaveBeenCalledWith('D:/workspace/BitFun', 'welcome_panel');
  });
});
