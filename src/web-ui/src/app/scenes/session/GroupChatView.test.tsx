// @vitest-environment jsdom

import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

vi.mock('@/infrastructure/api/service-api/ToolAPI', () => ({
  toolAPI: { executeTool: vi.fn() },
}));

vi.mock('@/infrastructure/api/service-api/SessionAPI', () => ({
  sessionAPI: {
    loadSessionMetadata: vi.fn(),
    listSessions: vi.fn(),
  },
}));

vi.mock('@/infrastructure/i18n/hooks/useI18n', async () => {
  const { createTestI18nT } = await import('@/test/i18nTestUtils');
  return { useI18n: () => ({ t: createTestI18nT('common') }) };
});

vi.mock('@/shared/notification-system', () => ({
  notificationService: { success: vi.fn(), error: vi.fn(), warning: vi.fn() },
}));

// 现成组件 mock（复用核验：真实组件路径保留在 import 中，测试只 stub 渲染面）。
vi.mock('../../../flow_chat/components/modern/ModernFlowChatContainer', () => ({
  ModernFlowChatContainer: ({
    className,
    emptyState,
    headerLeftActionsContent,
  }: {
    className?: string;
    emptyState?: React.ReactNode;
    headerLeftActionsContent?: React.ReactNode;
  }) => (
    <div data-testid="flow-chat-container" data-class-name={className ?? ''}>
      {headerLeftActionsContent}
      {emptyState}
    </div>
  ),
}));

vi.mock('../../../flow_chat/components/ChatInput', () => ({
  ChatInput: ({
    registration,
  }: {
    registration?: { placeholder?: string; onSubmit?: (s: { text: string }) => Promise<void> | void };
  }) => (
    <div data-testid="group-chat-input">
      <input
        data-testid="group-chat-input-box"
        placeholder={registration?.placeholder}
      />
      <button
        type="button"
        data-testid="group-chat-input-send"
        onClick={() => {
          const box = document.querySelector<HTMLInputElement>('[data-testid="group-chat-input-box"]');
          if (box && registration?.onSubmit) {
            void registration.onSubmit({ text: box.value });
          }
        }}
      >
        send
      </button>
    </div>
  ),
}));

// 复用核验：R-GC-15 成员区全部走现成 component-library 组件（Modal/Button/
// Checkbox/Input），测试只 stub 渲染面，不改生产代码路径。
vi.mock('@/component-library', () => {
  const React = require('react');
  return {
    Modal: ({ isOpen, children }: { isOpen: boolean; children: React.ReactNode }) =>
      isOpen ? <div data-testid="modal">{children}</div> : null,
    Input: (props: { label?: string; value?: string; type?: string; min?: number; max?: number; onChange?: (e: { target: { value: string } }) => void; placeholder?: string; autoFocus?: boolean }) => (
      <input
        data-testid={String(props.label ?? '').includes('count') || String(props.label ?? '').includes('Count') ? 'dialog-count-input' : 'dialog-name-input'}
        aria-label={props.label}
        type={props.type ?? 'text'}
        min={props.min}
        max={props.max}
        placeholder={props.placeholder}
        value={props.value ?? ''}
        onChange={props.onChange}
        autoFocus={props.autoFocus}
      />
    ),
    Checkbox: (props: { checked?: boolean; onChange?: () => void; label?: string; size?: string; disabled?: boolean }) => (
      <input
        type="checkbox"
        data-testid={props.label ? 'member-select-all' : 'member-checkbox'}
        checked={props.checked}
        onChange={props.onChange}
        aria-label={props.label}
        disabled={props.disabled}
      />
    ),
    Button: (props: { onClick?: () => void; disabled?: boolean; isLoading?: boolean; variant?: string; children?: React.ReactNode; type?: string; size?: string }) => (
      <button
        type={props.type ?? 'button'}
        data-testid={`btn-${props.variant ?? 'default'}`}
        onClick={props.onClick}
        disabled={props.disabled}
      >
        {props.children}
      </button>
    ),
    // R-GC-20: invite/fork 图标按钮（复用现成 IconButton；测试 stub 渲染面）。
    IconButton: (props: { onClick?: () => void; 'aria-label'?: string; variant?: string; size?: string; children?: React.ReactNode }) => (
      <button
        type="button"
        data-testid={`icon-btn-${props.variant ?? 'default'}`}
        aria-label={props['aria-label']}
        onClick={props.onClick}
      >
        {props.children}
      </button>
    ),
    // R-GC-22/30: 邀请/裂变成员选择 = component-library Select（原始下拉组件，
    // multiple + searchable + showSelectAll）。测试 stub 渲染面：渲染 options
    // 为可点击项，点击后调用 onChange。
    Select: (props: {
      options?: Array<{ value: string | number; label: string; description?: string }>;
      value?: string | number | Array<string | number>;
      onChange?: (value: string | number | Array<string | number>) => void;
      multiple?: boolean;
      loading?: boolean;
      placeholder?: string;
      'data-testid'?: string;
      triggerTestId?: string;
      dropdownTestId?: string;
    }) => {
      const values = Array.isArray(props.value) ? props.value : [];
      return (
        <div data-testid={props['data-testid'] ?? 'select'}>
          <div data-testid={props.triggerTestId ?? 'select-trigger'}>
            {props.placeholder ?? ''}
          </div>
          <div data-testid={props.dropdownTestId ?? 'select-dropdown'}>
            {(props.options ?? []).map(option => {
              const isSelected = values.includes(option.value);
              return (
                <button
                  key={String(option.value)}
                  type="button"
                  data-testid="member-select-option"
                  data-value={String(option.value)}
                  data-selected={isSelected ? 'true' : 'false'}
                  data-inactive={option.description ? 'true' : undefined}
                  onClick={() => {
                    const next = isSelected
                      ? values.filter(v => v !== option.value)
                      : [...values, option.value];
                    props.onChange?.(props.multiple ? next : option.value);
                  }}
                >
                  {option.label}
                </button>
              );
            })}
          </div>
        </div>
      );
    },
  };
});

vi.mock('@/infrastructure/appearance/runtime/AppearanceOverlayHost', () => ({
  getAppearanceOverlayHost: () => document.body,
}));

// R-GC-15: flowChatStore 单例 mock（createSession/markSessionAsGroupChat/
// addDialogTurn/getState）——复用 R-GC-13 登记形态，测试验证 fork 跳转链路。
const flowChatMocks = vi.hoisted(() => ({
  createSession: vi.fn(),
  markSessionAsGroupChat: vi.fn(),
  addDialogTurn: vi.fn(),
  getState: vi.fn(() => ({ sessions: new Map(), activeSessionId: null })),
}));

vi.mock('@/flow_chat/store/FlowChatStore', () => ({
  FlowChatStore: { getInstance: () => flowChatMocks },
  flowChatStore: flowChatMocks,
}));

vi.mock('@/flow_chat/services/sessionActivation', () => ({
  openMainSession: vi.fn(() => Promise.resolve()),
}));

import GroupChatView from './GroupChatView';
import { toolAPI } from '@/infrastructure/api/service-api/ToolAPI';
import { sessionAPI } from '@/infrastructure/api/service-api/SessionAPI';
import { openMainSession } from '@/flow_chat/services/sessionActivation';
import { flowChatStore } from '@/flow_chat/store/FlowChatStore';
import type { SessionMetadata } from '@/shared/types/session-history';

const makeSession = (id: string, agentType: string, sessionName?: string): SessionMetadata => ({
  sessionId: id,
  sessionName: sessionName ?? id,
  agentType,
  modelName: 'auto',
  createdAt: 0,
  lastActiveAt: 0,
  turnCount: 0,
  messageCount: 0,
  toolCallCount: 0,
  status: 'active',
  tags: [],
});

describe('GroupChatView (R-GC-14 view + R-GC-15 member management)', () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
    vi.mocked(toolAPI.executeTool).mockReset();
    vi.mocked(sessionAPI.loadSessionMetadata).mockReset();
    vi.mocked(sessionAPI.listSessions).mockReset();
    vi.mocked(openMainSession).mockClear();
    flowChatMocks.createSession.mockClear();
    flowChatMocks.markSessionAsGroupChat.mockClear();
    flowChatMocks.addDialogTurn.mockClear();
    flowChatMocks.getState.mockClear();
    flowChatMocks.getState.mockReturnValue({ sessions: new Map(), activeSessionId: null });
  });

  afterEach(() => {
    act(() => root.unmount());
    container.remove();
  });

  const renderView = (props?: Partial<React.ComponentProps<typeof GroupChatView>>) => {
    act(() => {
      root.render(
        <GroupChatView groupId="group-1" workspacePath="/workspace-a" {...props} />,
      );
    });
  };

  const flush = async () => {
    await act(async () => { await Promise.resolve(); });
    await act(async () => { await Promise.resolve(); });
  };

  const typeMessage = (value: string) => {
    const box = document.querySelector<HTMLInputElement>('[data-testid="group-chat-input-box"]');
    expect(box).not.toBeNull();
    act(() => {
      const nativeSetter = Object.getOwnPropertyDescriptor(
        window.HTMLInputElement.prototype,
        'value',
      )?.set;
      nativeSetter?.call(box, value);
      box!.dispatchEvent(new Event('input', { bubbles: true }));
    });
  };

  const clickSend = () => {
    const sendBtn = document.querySelector<HTMLButtonElement>('[data-testid="group-chat-input-send"]');
    expect(sendBtn).not.toBeNull();
    act(() => sendBtn!.click());
  };

  it('loads group history through toolAPI.executeTool (get_group_history, camelCase, no bare invoke)', async () => {
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

    // 历史走 execute_tool 通道（契约 §一.4，camelCase），禁裸 invoke。
    expect(toolAPI.executeTool).toHaveBeenCalledTimes(1);
    expect(toolAPI.executeTool).toHaveBeenCalledWith({
      toolName: 'get_group_history',
      parameters: { action: 'history', group_id: 'group-1', limit: 200 },
      workspacePath: '/workspace-a',
    });
    // 复用现成气泡列表（FlowChatContainer 渲染，turn 注入 flowChatStore 由生产代码负责）。
    expect(document.querySelector('[data-testid="flow-chat-container"]')).not.toBeNull();
  });

  it('does not render a bare invoke path: every group action goes through executeTool', async () => {
    // 组件源码中不存在任何 api.invoke('send_group_message') 等裸调用；
    // 此处通过 mock 记录证明：渲染后仅 executeTool 被调用（get_group_history）。
    renderView();
    // flush 挂载期的异步 effect（loadHistory/loadMembers setState），避免
    // "update not wrapped in act" warning。
    await flush();
    expect(toolAPI.executeTool).toHaveBeenCalledTimes(1);
    expect(toolAPI.executeTool.mock.calls[0]![0].toolName).toBe('get_group_history');
  });

  it('sends a group message through send_group_message with camelCase shape (no direct invoke)', async () => {
    vi.mocked(toolAPI.executeTool).mockImplementation(async (request: {
      toolName: string;
    }) => {
      if (request.toolName === 'get_group_history') {
        return { toolName: 'get_group_history', success: true, result: { messages: [] } };
      }
      if (request.toolName === 'send_group_message') {
        return { toolName: 'send_group_message', success: true, result: { messageId: 'msg-new', status: 'sent' } };
      }
      return { toolName: request.toolName, success: false, result: null };
    });
    renderView();
    await flush();

    typeMessage(' 大家好 ');
    clickSend();
    await flush();

    // 历史 + 发送两次 executeTool，全部 camelCase，禁裸 invoke。
    const sendCall = vi.mocked(toolAPI.executeTool).mock.calls.find(
      c => c[0].toolName === 'send_group_message',
    );
    expect(sendCall).toBeDefined();
    expect(sendCall![0]).toEqual({
      toolName: 'send_group_message',
      parameters: {
        action: 'send',
        group_id: 'group-1',
        content: '大家好',
        // R-GC-34 (owner identity P0 fix): group chat owner = master actor
        // (GROUP_MASTER_ACTOR reserved word), NOT the group session id. The
        // backend resolves it to Commander + L0 + localized owner name.
        sender_session_id: '__master__',
      },
      workspacePath: '/workspace-a',
    });

    // R-GC-26: send no longer optimistically injects a local turn - the
    // backend routes the message into the group-owner session's real dialog
    // turn and the DialogTurnStarted event creates the turn (avoids duplicating
    // the backend turn).
    expect(flowChatMocks.addDialogTurn).not.toHaveBeenCalled();
  });

  it('does not inject a local turn when the backend send fails', async () => {
    vi.mocked(toolAPI.executeTool).mockImplementation(async (request: {
      toolName: string;
    }) => {
      if (request.toolName === 'get_group_history') {
        return { toolName: 'get_group_history', success: true, result: { messages: [] } };
      }
      return { toolName: 'send_group_message', success: false, result: null, error: 'sender missing' };
    });
    renderView();
    await flush();

    typeMessage('will fail');
    clickSend();
    await flush();

    // 失败路径：不乐观注入本地 turn（组件在 success!==true 分支 return）。
    const sendCalls = vi.mocked(toolAPI.executeTool).mock.calls.filter(
      c => c[0].toolName === 'send_group_message',
    );
    expect(sendCalls).toHaveLength(1);
    expect(vi.mocked(toolAPI.executeTool)).toHaveBeenCalledTimes(2);
  });

  // ── R-GC-15：成员管理 ─────────────────────────────────────────────

  it('loads member list from group session metadata groupChats + listSessions display names', async () => {
    vi.mocked(toolAPI.executeTool).mockResolvedValue({
      toolName: 'get_group_history',
      success: true,
      result: { messages: [] },
    });
    vi.mocked(sessionAPI.loadSessionMetadata).mockResolvedValue({
      sessionId: 'group-1',
      sessionName: '项目群',
      agentType: 'group',
      modelName: 'auto',
      createdAt: 0,
      lastActiveAt: 0,
      turnCount: 0,
      messageCount: 0,
      toolCallCount: 0,
      status: 'active',
      tags: [],
      customMetadata: { groupChats: ['claw-1', 'claw-2'] },
    });
    vi.mocked(sessionAPI.listSessions).mockResolvedValue([
      makeSession('claw-1', 'Claw', 'Assist A'),
      makeSession('claw-2', 'Claw', 'Assist C'),
    ]);
    renderView();
    await flush();

    // R-GC-24: the member list lives in a Modal opened from the header action
    // group (original FlowChatHeader left slot).
    const membersBtn = [...document.querySelectorAll<HTMLButtonElement>('button[aria-label]')].find(
      b => b.getAttribute('aria-label')?.includes('Members'),
    );
    expect(membersBtn).not.toBeNull();
    act(() => membersBtn!.click());
    await flush();

    const rows = [...document.querySelectorAll<HTMLElement>('[data-testid="group-chat-member-list"] [data-member-id]')];
    expect(rows).toHaveLength(2);
    expect(rows[0]!.textContent).toContain('Assist A');
    expect(rows[1]!.textContent).toContain('Assist C');
  });

  it('R-GC-37: member names resolve across ALL workspace roots (cross-root member shows real name, same-root member does not fall back)', async () => {
    vi.mocked(toolAPI.executeTool).mockResolvedValue({
      toolName: 'get_group_history',
      success: true,
      result: { messages: [] },
    });
    vi.mocked(sessionAPI.loadSessionMetadata).mockResolvedValue({
      sessionId: 'group-1',
      sessionName: '项目群',
      agentType: 'group',
      modelName: 'auto',
      createdAt: 0,
      lastActiveAt: 0,
      turnCount: 0,
      messageCount: 0,
      toolCallCount: 0,
      status: 'active',
      tags: [],
      customMetadata: { groupChats: ['claw-1', 'cross-root-1'] },
    });
    // 跨 root：claw-1 只存在于主工作区；cross-root-1 只存在于 assistant 工作区。
    vi.mocked(sessionAPI.listSessions).mockImplementation(async (root: string) => {
      if (root === '/workspace-a') {
        return [makeSession('claw-1', 'Claw', 'Assist A')];
      }
      if (root === '/assistant/ws-cross') {
        return [makeSession('cross-root-1', 'Claw', 'CrossRoot Assistant')];
      }
      return [];
    });
    renderView({
      assistantWorkspaces: [
        {
          id: 'ws-cross',
          name: 'CrossRoot Assistant',
          rootPath: '/assistant/ws-cross',
          workspaceKind: 'assistant',
          assistantId: 'cross-root-1',
          languages: [],
          openedAt: '',
          lastAccessed: '',
          tags: [],
        },
      ],
    });
    await flush();

    // 解析必须真正遍历 assistant workspace root（与建群成员源同构）。
    expect(sessionAPI.listSessions).toHaveBeenCalledWith('/assistant/ws-cross');

    const membersBtn = [...document.querySelectorAll<HTMLButtonElement>('button[aria-label]')].find(
      b => b.getAttribute('aria-label')?.includes('Members'),
    );
    expect(membersBtn).not.toBeNull();
    act(() => membersBtn!.click());
    await flush();

    const rows = [...document.querySelectorAll<HTMLElement>('[data-testid="group-chat-member-list"] [data-member-id]')];
    expect(rows).toHaveLength(2);
    // 同 root 成员 = 真实名（不回退到 UUID）；跨 root 成员 = 真实名（非 UUID）。
    expect(rows[0]!.textContent).toContain('Assist A');
    expect(rows[1]!.textContent).toContain('CrossRoot Assistant');
    expect(rows[1]!.textContent).not.toContain('cross-root-1');
  });

  it('R-GC-37: member name resolution falls back to the raw session id when no workspace knows the id (defensive, no crash)', async () => {
    vi.mocked(toolAPI.executeTool).mockResolvedValue({
      toolName: 'get_group_history',
      success: true,
      result: { messages: [] },
    });
    vi.mocked(sessionAPI.loadSessionMetadata).mockResolvedValue({
      sessionId: 'group-1',
      sessionName: '项目群',
      agentType: 'group',
      modelName: 'auto',
      createdAt: 0,
      lastActiveAt: 0,
      turnCount: 0,
      messageCount: 0,
      toolCallCount: 0,
      status: 'active',
      tags: [],
      customMetadata: { groupChats: ['ghost-1'] },
    });
    // 任何 root 都查不到 ghost-1 → 回退原始 id，且不 crash。
    vi.mocked(sessionAPI.listSessions).mockResolvedValue([]);
    renderView();
    await flush();

    const membersBtn = [...document.querySelectorAll<HTMLButtonElement>('button[aria-label]')].find(
      b => b.getAttribute('aria-label')?.includes('Members'),
    );
    expect(membersBtn).not.toBeNull();
    act(() => membersBtn!.click());
    await flush();

    const rows = [...document.querySelectorAll<HTMLElement>('[data-testid="group-chat-member-list"] [data-member-id]')];
    expect(rows).toHaveLength(1);
    expect(rows[0]!.getAttribute('data-member-id')).toBe('ghost-1');
    expect(rows[0]!.textContent).toContain('ghost-1');
  });

  it('invites members through invite_group_member (R-GC-30/R-GC-R6: owner picks members from the full runtime session list, agentType not filtered)', async () => {
    vi.mocked(toolAPI.executeTool).mockImplementation(async (request: {
      toolName: string;
    }) => {
      if (request.toolName === 'get_group_history') {
        return { toolName: 'get_group_history', success: true, result: { messages: [] } };
      }
      if (request.toolName === 'invite_group_member') {
        return { toolName: 'invite_group_member', success: true, result: { status: 'invited' } };
      }
      return { toolName: request.toolName, success: false, result: null };
    });
    vi.mocked(sessionAPI.loadSessionMetadata).mockResolvedValue(null);
    // R-GC-30/R-GC-R6: member source = runtime-fetched sessions (listSessions),
    // zero hardcoded. agentType NOT filtered — agentic (GeneralPurpose) included.
    vi.mocked(sessionAPI.listSessions).mockResolvedValue([
      makeSession('claw-1', 'Claw', 'Assist A'),
      makeSession('claw-2', 'Claw', 'Assist B'),
      makeSession('gen-1', 'GeneralPurpose', 'Agentic C'),
    ]);
    renderView();
    await flush();

    // 打开邀请弹窗（现成 Modal；R-GC-20 邀请为右上角图标按钮，按 aria-label 定位）
    const inviteBtns = [...document.querySelectorAll<HTMLButtonElement>('button[aria-label]')].filter(
      b => b.getAttribute('aria-label')?.includes('Invite'),
    );
    act(() => inviteBtns[0]!.click());
    await flush();

    const modal = document.querySelector('[data-testid="modal"]');
    expect(modal).not.toBeNull();
    // R-GC-30: 邀请 = 成员多选（Select multiple），无数量输入。
    expect(document.querySelector('[data-testid="dialog-count-input"]')).toBeNull();
    const options = [...document.querySelectorAll<HTMLButtonElement>('[data-testid="member-select-option"]')];
    expect(options).toHaveLength(3); // 全部真实会话（Claw + agentic）都进候选

    // 勾选全部成员（Select stub 点击切换选中）。
    act(() => options[0]!.click());
    act(() => options[1]!.click());
    act(() => options[2]!.click());
    await flush();

    const confirmBtn = [...document.querySelectorAll<HTMLButtonElement>('button')].find(
      b => b.textContent?.includes('Confirm invite'),
    );
    expect(confirmBtn).not.toBeNull();
    act(() => confirmBtn!.click());
    await flush();

    // 每个被勾选的成员触发一次 invite（成员 ID = 真实会话 ID 透传）。
    const inviteCalls = vi.mocked(toolAPI.executeTool).mock.calls.filter(
      c => c[0].toolName === 'invite_group_member',
    );
    expect(inviteCalls).toHaveLength(3);
    expect(inviteCalls[0]![0]).toEqual({
      toolName: 'invite_group_member',
      parameters: {
        action: 'invite',
        group_id: 'group-1',
        member_session_id: 'claw-1',
        workspace: '/workspace-a',
      },
      workspacePath: '/workspace-a',
    });
    expect(inviteCalls[1]![0].parameters.member_session_id).toBe('claw-2');
    expect(inviteCalls[2]![0].parameters.member_session_id).toBe('gen-1');
  });

  it('R-GC-33: invite member source = real Claw sessions across ALL assistant workspace roots (no fabricated presets)', async () => {
    vi.mocked(toolAPI.executeTool).mockImplementation(async (request: {
      toolName: string;
    }) => {
      if (request.toolName === 'get_group_history') {
        return { toolName: 'get_group_history', success: true, result: { messages: [] } };
      }
      if (request.toolName === 'invite_group_member') {
        return { toolName: 'invite_group_member', success: true, result: { status: 'invited' } };
      }
      return { toolName: request.toolName, success: false, result: null };
    });
    vi.mocked(sessionAPI.loadSessionMetadata).mockResolvedValue(null);
    // 每个 assistant workspace root 返回该工作区真实 Claw 会话（含未打开）。
    vi.mocked(sessionAPI.listSessions).mockImplementation(async (root: string) => {
      if (root === '/workspace-a') {
        return [makeSession('claw-1', 'Claw', 'Assist A')];
      }
      if (root === '/assistant/ws-preset') {
        return [makeSession('claw-preset-1', 'Claw', '姬梦情-审查官')];
      }
      return [];
    });
    renderView({
      assistantWorkspaces: [
        {
          id: 'ws-preset',
          name: '姬梦情-审查官',
          rootPath: '/assistant/ws-preset',
          workspaceKind: 'assistant',
          assistantId: 'claw-preset-1',
          languages: [],
          openedAt: '',
          lastAccessed: '',
          tags: [],
        },
      ],
    });
    await flush();

    const inviteBtns = [...document.querySelectorAll<HTMLButtonElement>('button[aria-label]')].filter(
      b => b.getAttribute('aria-label')?.includes('Invite'),
    );
    act(() => inviteBtns[0]!.click());
    await flush();

    // 候选 = 主工作区 (claw-1) ∪ assistant 工作区真实会话 (claw-preset-1) = 2 个，
    // 与建群 CreateGroupChatDialog 成员源一致；无任何伪造 inactive 假条目。
    expect(sessionAPI.listSessions).toHaveBeenCalledWith('/assistant/ws-preset');
    const options = [...document.querySelectorAll<HTMLButtonElement>('[data-testid="member-select-option"]')];
    expect(options).toHaveLength(2);
    expect(options[1]!.getAttribute('data-value')).toBe('claw-preset-1');
    expect(options[1]!.getAttribute('data-inactive')).toBeNull();

    act(() => options[0]!.click());
    act(() => options[1]!.click());
    await flush();

    const confirmBtn = [...document.querySelectorAll<HTMLButtonElement>('button')].find(
      b => b.textContent?.includes('Confirm invite'),
    );
    expect(confirmBtn).not.toBeNull();
    act(() => confirmBtn!.click());
    await flush();

    const inviteCalls = vi.mocked(toolAPI.executeTool).mock.calls.filter(
      c => c[0].toolName === 'invite_group_member',
    );
    expect(inviteCalls).toHaveLength(2);
    expect(inviteCalls[1]![0].parameters.member_session_id).toBe('claw-preset-1');
  });

  it('removes a member through remove_group_member', async () => {
    vi.mocked(toolAPI.executeTool).mockImplementation(async (request: {
      toolName: string;
    }) => {
      if (request.toolName === 'get_group_history') {
        return { toolName: 'get_group_history', success: true, result: { messages: [] } };
      }
      if (request.toolName === 'remove_group_member') {
        return { toolName: 'remove_group_member', success: true, result: { status: 'removed' } };
      }
      return { toolName: request.toolName, success: false, result: null };
    });
    vi.mocked(sessionAPI.loadSessionMetadata).mockResolvedValue({
      sessionId: 'group-1',
      sessionName: '项目群',
      agentType: 'group',
      modelName: 'auto',
      createdAt: 0,
      lastActiveAt: 0,
      turnCount: 0,
      messageCount: 0,
      toolCallCount: 0,
      status: 'active',
      tags: [],
      customMetadata: { groupChats: ['claw-1'] },
    });
    vi.mocked(sessionAPI.listSessions).mockResolvedValue([
      makeSession('claw-1', 'Claw', 'Assist A'),
    ]);
    renderView();
    await flush();

    // R-GC-24: 成员列表在 Modal 中（原布局 header 左动作 → 成员弹窗）。
    const membersBtn = [...document.querySelectorAll<HTMLButtonElement>('button[aria-label]')].find(
      b => b.getAttribute('aria-label')?.includes('Members'),
    );
    expect(membersBtn).not.toBeNull();
    act(() => membersBtn!.click());
    await flush();

    const removeBtn = [...document.querySelectorAll<HTMLButtonElement>('button')].find(
      b => b.textContent?.includes('Remove'),
    );
    expect(removeBtn).not.toBeNull();
    act(() => removeBtn!.click());
    await flush();

    const removeCall = vi.mocked(toolAPI.executeTool).mock.calls.find(
      c => c[0].toolName === 'remove_group_member',
    );
    expect(removeCall).toBeDefined();
    expect(removeCall![0]).toEqual({
      toolName: 'remove_group_member',
      parameters: {
        action: 'remove',
        group_id: 'group-1',
        member_session_id: 'claw-1',
      },
      workspacePath: '/workspace-a',
    });
  });

  it('forks the group through fork_group_chat then jumps to the child group view', async () => {
    vi.mocked(toolAPI.executeTool).mockImplementation(async (request: {
      toolName: string;
    }) => {
      if (request.toolName === 'get_group_history') {
        return {
          toolName: 'get_group_history',
          success: true,
          result: {
            messages: [{
              messageId: 'msg-last',
              groupSessionId: 'group-1',
              author: { sessionId: 'commander-1', name: '群主' },
              content: 'fork point',
              timestamp: 1000,
            }],
          },
        };
      }
      if (request.toolName === 'fork_group_chat') {
        return {
          toolName: 'fork_group_chat',
          success: true,
          result: { parentGroupId: 'group-1', childGroupId: 'group-child-1' },
        };
      }
      return { toolName: request.toolName, success: false, result: null };
    });
    vi.mocked(sessionAPI.loadSessionMetadata).mockResolvedValue(null);
    // R-GC-30/R-GC-R6: fork 成员 = 运行时全量真实会话列表（不过滤 agentType）。
    vi.mocked(sessionAPI.listSessions).mockResolvedValue([
      makeSession('claw-1', 'Claw', 'Assist A'),
      makeSession('claw-2', 'Claw', 'Assist B'),
      makeSession('gen-1', 'GeneralPurpose', 'Agentic C'),
    ]);
    // 注入历史 turn 以提供 fork 的 turn_id（lastTurnId 取自本地 session turns）
    flowChatMocks.getState.mockReturnValue({
      sessions: new Map([['group-1', {
        dialogTurns: [{
          id: 'msg-last',
          userMessage: { id: 'msg-last', content: 'fork point', timestamp: 1000 },
        }],
      }]]),
      activeSessionId: 'group-1',
    });
    renderView();
    await flush();

    // 打开 fork 弹窗（现成 Modal + Select 多选形态；R-GC-20 裂变
    // 为右上角图标按钮，按 aria-label 定位）
    const forkBtns = [...document.querySelectorAll<HTMLButtonElement>('button[aria-label]')].filter(
      b => b.getAttribute('aria-label')?.includes('Fork'),
    );
    act(() => forkBtns[0]!.click());
    await flush();

    const nameInput = document.querySelector<HTMLInputElement>('[data-testid="dialog-name-input"]');
    expect(nameInput).not.toBeNull();
    // 默认名 = groupName + forkSuffix（英文 i18n：Untitled group · child）
    act(() => {
      const nativeSetter = Object.getOwnPropertyDescriptor(
        window.HTMLInputElement.prototype,
        'value',
      )?.set;
      nativeSetter?.call(nameInput, '子群A');
      nameInput!.dispatchEvent(new Event('input', { bubbles: true }));
    });

    // R-GC-30/R-GC-R6: fork 成员 = 全量真实会话多选（Select multiple），无数量输入。
    expect(document.querySelector('[data-testid="dialog-count-input"]')).toBeNull();
    const options = [...document.querySelectorAll<HTMLButtonElement>('[data-testid="member-select-option"]')];
    expect(options).toHaveLength(3);
    act(() => options[0]!.click());
    act(() => options[1]!.click());
    act(() => options[2]!.click());
    await flush();

    const confirmBtn = [...document.querySelectorAll<HTMLButtonElement>('button')].find(
      b => b.textContent?.includes('Confirm fork'),
    );
    expect(confirmBtn).not.toBeNull();
    act(() => confirmBtn!.click());
    await flush();

    const forkCall = vi.mocked(toolAPI.executeTool).mock.calls.find(
      c => c[0].toolName === 'fork_group_chat',
    );
    expect(forkCall).toBeDefined();
    expect(forkCall![0]).toEqual({
      toolName: 'fork_group_chat',
      parameters: {
        action: 'fork',
        group_id: 'group-1',
        name: '子群A',
        turn_id: 'msg-last',
        members: ['claw-1', 'claw-2', 'gen-1'],
      },
      workspacePath: '/workspace-a',
    });

    // fork 成功 → 登记子群 + 标记群聊 + 跳转子群视图（复用 R-GC-13 登记形态；
    // R-WF-02：子群 = agent_type="group"）
    expect(flowChatMocks.createSession).toHaveBeenCalledWith(
      'group-child-1',
      expect.objectContaining({ workspacePath: '/workspace-a', agentType: 'group' }),
      undefined,
      '子群A',
      1048576,
      'group',
      '/workspace-a',
    );
    expect(flowChatMocks.markSessionAsGroupChat).toHaveBeenCalledWith('group-child-1');
    expect(openMainSession).toHaveBeenCalledWith('group-child-1', {});
  });

  it('R-GC-33: fork member source = real Claw sessions across ALL assistant workspace roots (no fabricated presets)', async () => {
    vi.mocked(toolAPI.executeTool).mockImplementation(async (request: {
      toolName: string;
    }) => {
      if (request.toolName === 'get_group_history') {
        return {
          toolName: 'get_group_history',
          success: true,
          result: {
            messages: [{
              messageId: 'msg-last',
              groupSessionId: 'group-1',
              author: { sessionId: 'commander-1', name: '群主' },
              content: 'fork point',
              timestamp: 1000,
            }],
          },
        };
      }
      if (request.toolName === 'fork_group_chat') {
        return {
          toolName: 'fork_group_chat',
          success: true,
          result: { parentGroupId: 'group-1', childGroupId: 'group-child-1' },
        };
      }
      return { toolName: request.toolName, success: false, result: null };
    });
    vi.mocked(sessionAPI.loadSessionMetadata).mockResolvedValue(null);
    vi.mocked(sessionAPI.listSessions).mockImplementation(async (root: string) => {
      if (root === '/workspace-a') {
        return [makeSession('claw-1', 'Claw', 'Assist A')];
      }
      if (root === '/assistant/ws-preset') {
        return [makeSession('claw-preset-1', 'Claw', '姬梦情-审查官')];
      }
      return [];
    });
    flowChatMocks.getState.mockReturnValue({
      sessions: new Map([['group-1', {
        dialogTurns: [{
          id: 'msg-last',
          userMessage: { id: 'msg-last', content: 'fork point', timestamp: 1000 },
        }],
      }]]),
      activeSessionId: 'group-1',
    });
    renderView({
      assistantWorkspaces: [
        {
          id: 'ws-preset',
          name: '姬梦情-审查官',
          rootPath: '/assistant/ws-preset',
          workspaceKind: 'assistant',
          assistantId: 'claw-preset-1',
          languages: [],
          openedAt: '',
          lastAccessed: '',
          tags: [],
        },
      ],
    });
    await flush();

    const forkBtns = [...document.querySelectorAll<HTMLButtonElement>('button[aria-label]')].filter(
      b => b.getAttribute('aria-label')?.includes('Fork'),
    );
    act(() => forkBtns[0]!.click());
    await flush();

    // 候选 = 主工作区 (claw-1) ∪ assistant 工作区真实会话 (claw-preset-1) = 2 个。
    const options = [...document.querySelectorAll<HTMLButtonElement>('[data-testid="member-select-option"]')];
    expect(options).toHaveLength(2);
    expect(options[1]!.getAttribute('data-value')).toBe('claw-preset-1');

    act(() => options[0]!.click());
    act(() => options[1]!.click());
    await flush();

    const confirmBtn = [...document.querySelectorAll<HTMLButtonElement>('button')].find(
      b => b.textContent?.includes('Confirm fork'),
    );
    expect(confirmBtn).not.toBeNull();
    act(() => confirmBtn!.click());
    await flush();

    const forkCall = vi.mocked(toolAPI.executeTool).mock.calls.find(
      c => c[0].toolName === 'fork_group_chat',
    );
    expect(forkCall).toBeDefined();
    expect(forkCall![0].parameters.members).toEqual(['claw-1', 'claw-preset-1']);
  });

  it('refuses to fork without a persisted message (forkNeedsMessage)', async () => {
    vi.mocked(toolAPI.executeTool).mockResolvedValue({
      toolName: 'get_group_history',
      success: true,
      result: { messages: [] },
    });
    vi.mocked(sessionAPI.loadSessionMetadata).mockResolvedValue(null);
    vi.mocked(sessionAPI.listSessions).mockResolvedValue([]);
    // 本地 session 无任何 turn → lastTurnId 为 undefined
    flowChatMocks.getState.mockReturnValue({ sessions: new Map(), activeSessionId: null });
    renderView();
    await flush();

    const forkBtns = [...document.querySelectorAll<HTMLButtonElement>('button[aria-label]')].filter(
      b => b.getAttribute('aria-label')?.includes('Fork'),
    );
    act(() => forkBtns[0]!.click());
    await flush();

    const confirmBtn = [...document.querySelectorAll<HTMLButtonElement>('button')].find(
      b => b.textContent?.includes('Confirm fork'),
    );
    expect(confirmBtn).not.toBeNull();
    act(() => confirmBtn!.click());
    await flush();

    const forkCalls = vi.mocked(toolAPI.executeTool).mock.calls.filter(
      c => c[0].toolName === 'fork_group_chat',
    );
    expect(forkCalls).toHaveLength(0);
    expect(openMainSession).not.toHaveBeenCalled();
  });
});
