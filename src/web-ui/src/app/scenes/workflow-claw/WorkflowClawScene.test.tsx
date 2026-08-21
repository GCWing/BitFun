// @vitest-environment jsdom

import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { WorkspaceInfo } from '@/shared/types';
import { useWorkspaceContext } from '@/infrastructure/contexts/WorkspaceContext';
import WorkflowClawScene from './WorkflowClawScene';

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

const WORKFLOW_WORKSPACE_A = {
  id: 'workspace-a',
  name: 'Research Bee',
  rootPath: 'C:/Users/me/.bitfun/personal_assistant/workspace-researcher',
  assistantId: 'researcher',
  identity: { name: 'Researcher', emoji: '🔍', creature: 'Research Bee', vibe: 'methodical' },
} as WorkspaceInfo;

const WORKFLOW_WORKSPACE_B = {
  id: 'workspace-b',
  name: 'Review Bee',
  rootPath: 'C:/Users/me/.bitfun/personal_assistant/workspace-reviewer',
  assistantId: 'reviewer',
  identity: { name: 'Reviewer', emoji: '👁️', creature: 'Review Bee', vibe: 'rigorous' },
} as WorkspaceInfo;

const PLAIN_WORKSPACE = {
  id: 'workspace-plain',
  name: 'Personal assistant',
  rootPath: 'C:/Users/me/.bitfun/personal_assistant/workspace-3f2a9c1d',
  assistantId: '3f2a9c1d',
  identity: { name: 'Mira', emoji: '🧭', creature: 'Assistant', vibe: 'helpful' },
} as WorkspaceInfo;

const mockOpenAssistant = vi.fn();
const mockSetSelectedAssistantWorkspaceId = vi.fn();
const mockOpenScene = vi.fn();

vi.mock('@/infrastructure/i18n/hooks/useI18n', () => ({
  useI18n: () => ({ t: (key: string) => key }),
}));

vi.mock('react-i18next', async (importOriginal) => ({
  ...(await importOriginal<typeof import('react-i18next')>()),
  useTranslation: () => ({
    t: (key: string) => key,
  }),
}));

vi.mock('@/component-library', () => ({
  Badge: ({ children }: { children: React.ReactNode }) => <span>{children}</span>,
  Tooltip: ({ children }: { children: React.ReactNode }) => <>{children}</>,
  DotMatrixLoader: () => <div data-testid="dot-matrix-loader" />,
}));

vi.mock('@/app/components', () => ({
  GalleryLayout: ({ children }: { children: React.ReactNode }) => <div>{children}</div>,
  GalleryPageHeader: ({ actions }: { actions?: React.ReactNode }) => (
    <div data-testid="workflow-claw-header">{actions}</div>
  ),
  GalleryZone: ({ children }: { children: React.ReactNode }) => <div>{children}</div>,
  GalleryGrid: ({ children }: { children: React.ReactNode }) => <div>{children}</div>,
  GalleryEmpty: ({ children, testId }: { children?: React.ReactNode; testId?: string }) => <div data-testid={testId}>{children}</div>,
}));

vi.mock('@/app/hooks/useSceneManager', () => ({
  useSceneManager: () => ({ openScene: mockOpenScene }),
}));

vi.mock('../my-agent/myAgentStore', () => ({
  useMyAgentStore: (selector: (state: { setSelectedAssistantWorkspaceId: (id: string) => void }) => unknown) =>
    selector({ setSelectedAssistantWorkspaceId: mockSetSelectedAssistantWorkspaceId }),
}));

vi.mock('../profile/nurseryStore', () => ({
  useNurseryStore: (selector: (state: { openAssistant: (id: string) => void }) => unknown) =>
    selector({ openAssistant: mockOpenAssistant }),
}));

vi.mock('@/infrastructure/contexts/WorkspaceContext', () => ({
  useWorkspaceContext: vi.fn(() => ({
    assistantWorkspacesList: [WORKFLOW_WORKSPACE_A, PLAIN_WORKSPACE, WORKFLOW_WORKSPACE_B],
  })),
}));

describe('WorkflowClawScene (R-WF-18)', () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
  });

  afterEach(() => {
    act(() => root.unmount());
    container.remove();
    vi.clearAllMocks();
  });

  it('renders only workflow-member Claws in the independent list (data-source isolation)', () => {
    act(() => {
      root.render(<WorkflowClawScene />);
    });

    const cards = container.querySelectorAll('[data-bf-component="workflow-claw-card"][data-bf-part="root"]');
    expect(cards.length).toBe(2);
    expect(container.textContent).toContain('Research Bee');
    expect(container.textContent).toContain('Review Bee');
    expect(container.textContent).not.toContain('Mira');
  });

  it('renders a new-workflow entry action on the gallery header (P2-3)', () => {
    act(() => {
      root.render(<WorkflowClawScene />);
    });

    const header = container.querySelector('[data-testid="workflow-claw-header"]');
    expect(header).not.toBeNull();
    const createBtn = header?.querySelector<HTMLButtonElement>('[data-testid="workflow-claw-create-btn"]');
    expect(createBtn).not.toBeNull();
    expect(createBtn?.textContent).toContain('nursery.workflowClaw.gallery.create');
  });

  it('opens the agents scene (workflow orchestration entry) from the header action (P2-3)', () => {
    act(() => {
      root.render(<WorkflowClawScene />);
    });

    const createBtn = container.querySelector<HTMLButtonElement>('[data-testid="workflow-claw-create-btn"]');
    act(() => createBtn?.click());

    expect(mockOpenScene).toHaveBeenCalledWith('agents');
  });

  it('opens the shared AssistantConfigPage (not a duplicated detail page) on card click', () => {
    act(() => {
      root.render(<WorkflowClawScene />);
    });

    const firstCardMain = container.querySelector('[data-bf-component="workflow-claw-card"][data-bf-part="main"]') as HTMLButtonElement;
    act(() => firstCardMain.click());

    expect(mockOpenAssistant).toHaveBeenCalledWith('workspace-a');
    expect(mockSetSelectedAssistantWorkspaceId).toHaveBeenCalledWith('workspace-a');
  });

  it('shows an empty state when no workflow member Claw exists', () => {
    vi.mocked(useWorkspaceContext).mockReturnValueOnce({
      assistantWorkspacesList: [PLAIN_WORKSPACE],
    });
    act(() => {
      root.render(<WorkflowClawScene />);
    });

    expect(container.querySelectorAll('[data-bf-component="workflow-claw-card"][data-bf-part="root"]').length).toBe(0);
    expect(container.querySelector('[data-testid="workflow-claw-empty"]')).not.toBeNull();
  });
});
