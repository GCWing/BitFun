// @vitest-environment jsdom

import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

vi.mock('@/component-library', () => ({
  Tooltip: ({ children }: { children: React.ReactNode }) => <>{children}</>,
}));

vi.mock('@/infrastructure/i18n/hooks/useI18n', async () => {
  const { createTestI18nT } = await import('@/test/i18nTestUtils');
  return { useI18n: () => ({ t: createTestI18nT('common') }) };
});

const openSceneMock = vi.fn();
vi.mock('@/app/hooks/useSceneManager', () => ({
  useSceneManager: () => ({ openScene: openSceneMock }),
}));

const openCreateLegionMock = vi.fn();
vi.mock('@/app/scenes/agents/agentsStore', () => ({
  useAgentsStore: { getState: () => ({ openCreateLegion: openCreateLegionMock }) },
}));

// R-WF-12: empty hint subscription. The store's session map starts empty, so
// GroupChatsSection renders the "no group chats yet" hint; the subscribe
// returns a no-op unsubscribe.
vi.mock('@/flow_chat/store/FlowChatStore', () => ({
  flowChatStore: {
    getState: () => ({ sessions: new Map() }),
    subscribeSelector: () => () => {},
  },
}));

vi.mock('../sessions/SessionsSection', () => {
  const MockSessionsSection = ({ groupChatsOnly, workspacePath }: {
    groupChatsOnly?: boolean;
    workspacePath?: string;
  }) => (
    <div data-testid="mock-sessions-section" data-group-chats-only={groupChatsOnly ? 'true' : undefined} data-workspace-path={workspacePath ?? ''}>
      mock sessions
    </div>
  );
  return { default: MockSessionsSection };
});

import GroupChatsSection from './GroupChatsSection';

describe('GroupChatsSection (R-WF-12)', () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    vi.clearAllMocks();
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
  });

  afterEach(() => {
    act(() => root.unmount());
    container.remove();
  });

  const renderSection = (onCreateGroupChat = vi.fn()) => {
    act(() => {
      root.render(
        <GroupChatsSection
          workspaceId="assistant-1"
          workspacePath="/assistants/1"
          onCreateGroupChat={onCreateGroupChat}
        />,
      );
    });
  };

  it('renders the group-chats section root with the data-bf-section contract (验收断言 1)', () => {
    renderSection();
    const section = container.querySelector('[data-bf-section="group-chats"]');
    expect(section).not.toBeNull();
  });

  it('renders both create entries: new workflow and new group chat (验收断言 2)', () => {
    renderSection();
    expect(container.querySelector('[data-testid="nav-group-chats-create-workflow-btn"]')).not.toBeNull();
    expect(container.querySelector('[data-testid="nav-group-chats-create-group-btn"]')).not.toBeNull();
  });

  it('opens the workflow (legion) creation page when creating a workflow (验收断言 2)', () => {
    renderSection();
    const workflowBtn = container.querySelector<HTMLButtonElement>('[data-testid="nav-group-chats-create-workflow-btn"]')!;
    act(() => workflowBtn.click());

    expect(openCreateLegionMock).toHaveBeenCalledTimes(1);
    expect(openSceneMock).toHaveBeenCalledWith('agents');
  });

  it('forwards the group chat create action to the existing dialog opener (验收断言 2)', () => {
    const onCreateGroupChat = vi.fn();
    renderSection(onCreateGroupChat);
    const groupBtn = container.querySelector<HTMLButtonElement>('[data-testid="nav-group-chats-create-group-btn"]')!;
    act(() => groupBtn.click());
    expect(onCreateGroupChat).toHaveBeenCalledTimes(1);
  });

  it('renders group chats only via SessionsSection groupChatsOnly (验收断言 3)', () => {
    renderSection();
    const mockSection = container.querySelector('[data-testid="mock-sessions-section"]');
    expect(mockSection?.getAttribute('data-group-chats-only')).toBe('true');
  });

  it('renders the empty hint when no group chats exist yet (空态)', () => {
    renderSection();
    const empty = container.querySelector('[data-testid="nav-group-chats-empty"]');
    expect(empty?.textContent).toBe('No group chats yet');
  });
});
