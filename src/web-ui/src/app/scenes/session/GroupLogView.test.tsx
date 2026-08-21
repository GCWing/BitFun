// @vitest-environment jsdom

import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

vi.mock('@/infrastructure/api/service-api/ToolAPI', () => ({
  toolAPI: { executeTool: vi.fn() },
}));

vi.mock('@/infrastructure/i18n/hooks/useI18n', async () => {
  const { createTestI18nT } = await import('@/test/i18nTestUtils');
  return { useI18n: () => ({ t: createTestI18nT('common') }) };
});

vi.mock('@/shared/notification-system', () => ({
  notificationService: { success: vi.fn(), error: vi.fn(), warning: vi.fn() },
}));

// FlowChatContainer stub: the bubble list host (reused FlowChat pipeline).
vi.mock('../../../flow_chat/components/modern/ModernFlowChatContainer', () => ({
  ModernFlowChatContainer: ({
    className,
    emptyState,
  }: {
    className?: string;
    emptyState?: React.ReactNode;
  }) => (
    <div data-testid="flow-chat-container" data-class-name={className ?? ''}>
      {emptyState}
    </div>
  ),
}));

// flowChatStore stub (history injection surface). FlowChatManager pulls
// additional store members (subscribeSelector) at module scope, so the stub
// mirrors the real singleton's public surface used by the manager.
const flowChatMocks = vi.hoisted(() => ({
  addDialogTurn: vi.fn(),
  getState: vi.fn(() => ({ sessions: new Map(), activeSessionId: null })),
  subscribeSelector: vi.fn(() => () => {}),
  setActiveSessionId: vi.fn(),
  subscribe: vi.fn(() => () => {}),
}));

vi.mock('@/flow_chat/store/FlowChatStore', () => ({
  FlowChatStore: { getInstance: () => flowChatMocks },
  flowChatStore: flowChatMocks,
  getFlowChatStoreInstance: () => flowChatMocks,
}));

import GroupLogView from './GroupLogView';
import { toolAPI } from '@/infrastructure/api/service-api/ToolAPI';
import { flowChatStore } from '@/flow_chat/store/FlowChatStore';

describe('GroupLogView (R-WF-14 read-only group log)', () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
    vi.mocked(toolAPI.executeTool).mockReset();
    flowChatMocks.addDialogTurn.mockClear();
    flowChatMocks.getState.mockClear();
    flowChatMocks.getState.mockReturnValue({ sessions: new Map(), activeSessionId: null });
  });

  afterEach(() => {
    act(() => root.unmount());
    container.remove();
  });

  const renderView = (props?: Partial<React.ComponentProps<typeof GroupLogView>>) => {
    act(() => {
      root.render(
        <GroupLogView groupId="group-1" workspacePath="/workspace-a" {...props} />,
      );
    });
  };

  const flush = async () => {
    await act(async () => { await Promise.resolve(); });
    await act(async () => { await Promise.resolve(); });
  };

  // RO-1: no input box / no member table / no interactive buttons.
  it('RO-1: renders a read-only timeline with no input, member table, or interaction', async () => {
    vi.mocked(toolAPI.executeTool).mockResolvedValue({
      toolName: 'get_group_history',
      success: true,
      result: { messages: [] },
    });
    renderView();
    await flush();

    expect(document.querySelector('[data-testid="flow-chat-container"]')).not.toBeNull();
    expect(document.querySelector('input')).toBeNull();
    expect(document.querySelector('textarea')).toBeNull();
    expect(document.querySelector('button')).toBeNull();
    expect(document.querySelector('[data-testid="group-chat-input"]')).toBeNull();
    expect(document.querySelector('[data-testid="group-chat-member-list"]')).toBeNull();
  });

  // RO-2: bubble timeline shows history + senderBadge metadata (reused FlowChat
  // pipeline: history injected via flowChatStore.addDialogTurn, bubbles rendered
  // by the FlowChat container).
  it('RO-2: loads history through get_group_history and injects turns into the FlowChat bubble pipeline', async () => {
    vi.mocked(toolAPI.executeTool).mockResolvedValue({
      toolName: 'get_group_history',
      success: true,
      result: {
        groupId: 'group-1',
        messages: [
          {
            messageId: 'msg-1',
            groupSessionId: 'group-1',
            author: { sessionId: 'commander-1', role: 'Commander', depth: 0, name: '群主' },
            content: 'hello group',
            timestamp: 1000,
          },
          {
            messageId: 'msg-2',
            groupSessionId: 'group-1',
            author: { sessionId: 'member-2', role: 'Executor', depth: 1, name: '二号' },
            content: 'received',
            timestamp: 2000,
          },
        ],
      },
    });
    renderView();
    await flush();

    expect(toolAPI.executeTool).toHaveBeenCalledTimes(1);
    expect(toolAPI.executeTool).toHaveBeenCalledWith({
      toolName: 'get_group_history',
      parameters: { action: 'history', group_id: 'group-1', limit: 200 },
      workspacePath: '/workspace-a',
    });

    expect(flowChatMocks.addDialogTurn).toHaveBeenCalledTimes(2);
    const [firstTurn, secondTurn] = flowChatMocks.addDialogTurn.mock.calls.map(c => c[1]);
    // senderBadge metadata (senderSessionId/senderName/senderRole/senderDepth)
    // rides the message metadata so the existing UserMessageItem badge renders it.
    expect(firstTurn.userMessage.metadata).toMatchObject({
      groupId: 'group-1',
      senderSessionId: 'commander-1',
      senderName: '群主',
      senderRole: 'Commander',
      senderDepth: 0,
    });
    expect(firstTurn.userMessage.metadata.senderType).toBeUndefined();
    expect(secondTurn.userMessage.metadata.senderName).toBe('二号');
    expect(secondTurn.userMessage.metadata.senderDepth).toBe(1);
  });

  // RO-2: history-load failure surfaces a retry affordance but still no input.
  it('RO-2: shows retry on history load failure (retry button only, no input)', async () => {
    vi.mocked(toolAPI.executeTool).mockRejectedValue(new Error('boom'));
    renderView();
    await flush();

    // Failure branch: the read-only timeline shows the history-failed state
    // with a single retry affordance — no composer, no member table.
    expect(document.querySelector('[data-testid="flow-chat-container"]')).toBeNull();
    const retryButtons = [...document.querySelectorAll('button')];
    expect(retryButtons).toHaveLength(1);
    expect(retryButtons[0]!.textContent).toBe('Retry');
    expect(document.querySelector('input')).toBeNull();
    expect(document.querySelector('textarea')).toBeNull();
    expect(document.querySelector('[data-testid="group-chat-member-list"]')).toBeNull();
  });

  // RO-2: retry re-issues get_group_history through the tool channel.
  it('RO-2: retry re-loads history through get_group_history', async () => {
    vi.mocked(toolAPI.executeTool).mockRejectedValueOnce(new Error('boom'));
    renderView();
    await flush();
    expect(toolAPI.executeTool).toHaveBeenCalledTimes(1);

    vi.mocked(toolAPI.executeTool).mockResolvedValue({
      toolName: 'get_group_history',
      success: true,
      result: { messages: [] },
    });
    const retryBtn = document.querySelector<HTMLButtonElement>('button');
    expect(retryBtn).not.toBeNull();
    act(() => retryBtn!.click());
    await flush();

    expect(toolAPI.executeTool).toHaveBeenCalledTimes(2);
    expect(flowChatMocks.addDialogTurn).not.toHaveBeenCalled();
  });
});
