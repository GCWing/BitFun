// @vitest-environment jsdom

import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

vi.mock('@/component-library', () => {
  const React = require('react');
  return {
    Modal: ({ isOpen, children }: { isOpen: boolean; children: React.ReactNode }) =>
      isOpen ? <div data-testid="modal">{children}</div> : null,
    Input: (props: { label?: string; value?: string; type?: string; min?: number; max?: number; onChange?: (e: { target: { value: string } }) => void; placeholder?: string; autoFocus?: boolean }) => (
      <input
        data-testid="group-name-input"
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
        data-testid={props.variant === 'primary' ? 'group-create-submit' : 'group-cancel'}
        onClick={props.onClick}
        disabled={props.disabled}
      >
        {props.children}
      </button>
    ),
  };
});

vi.mock('@/infrastructure/appearance/runtime/AppearanceOverlayHost', () => ({
  getAppearanceOverlayHost: () => document.body,
}));

vi.mock('@/infrastructure/i18n/hooks/useI18n', async () => {
  const { createTestI18nT } = await import('@/test/i18nTestUtils');
  return { useI18n: () => ({ t: createTestI18nT('common') }) };
});

vi.mock('@/infrastructure/api/service-api/ToolAPI', () => ({
  toolAPI: {
    executeTool: vi.fn(),
  },
}));

vi.mock('@/infrastructure/api/service-api/SessionAPI', () => ({
  sessionAPI: {
    listSessions: vi.fn(),
  },
}));

vi.mock('@/shared/notification-system', () => ({
  notificationService: {
    success: vi.fn(),
    error: vi.fn(),
    warning: vi.fn(),
  },
}));

import CreateGroupChatDialog from './CreateGroupChatDialog';
import { toolAPI } from '@/infrastructure/api/service-api/ToolAPI';
import { sessionAPI } from '@/infrastructure/api/service-api/SessionAPI';
import { notificationService } from '@/shared/notification-system';
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

describe('CreateGroupChatDialog (R-GC-13 / R-GC-19 / R-GC-30)', () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
    vi.mocked(toolAPI.executeTool).mockReset();
    vi.mocked(sessionAPI.listSessions).mockReset();
    vi.mocked(notificationService.success).mockReset();
  });

  afterEach(() => {
    act(() => root.unmount());
    container.remove();
  });

  const renderDialog = (onCreated = vi.fn(), onClose = vi.fn(), props?: Partial<React.ComponentProps<typeof CreateGroupChatDialog>>) => {
    act(() => {
      root.render(
        <CreateGroupChatDialog
          isOpen
          onClose={onClose}
          workspacePath="/workspace-a"
          onCreated={onCreated}
          {...props}
        />,
      );
    });
    return { onCreated, onClose };
  };

  const setGroupName = (value: string) => {
    const input = document.querySelector<HTMLInputElement>('[data-testid="group-name-input"]');
    expect(input).not.toBeNull();
    act(() => {
      const nativeSetter = Object.getOwnPropertyDescriptor(
        window.HTMLInputElement.prototype,
        'value',
      )?.set;
      nativeSetter?.call(input, value);
      input!.dispatchEvent(new Event('input', { bubbles: true }));
    });
  };

  const toggleMember = (index: number) => {
    const checkboxes = [...document.querySelectorAll<HTMLInputElement>('[data-testid="member-checkbox"]')];
    expect(checkboxes[index]).toBeDefined();
    act(() => checkboxes[index]!.click());
  };

  const clickCreate = () => {
    const button = document.querySelector<HTMLButtonElement>('[data-testid="group-create-submit"]');
    expect(button).not.toBeNull();
    act(() => button!.click());
  };

  const getSubmitDisabled = () => {
    const button = document.querySelector<HTMLButtonElement>('[data-testid="group-create-submit"]');
    return button?.disabled ?? true;
  };

  it('creates the group through toolAPI.executeTool with camelCase shape (no direct invoke)', async () => {
    vi.mocked(sessionAPI.listSessions).mockResolvedValue([]);
    vi.mocked(toolAPI.executeTool).mockResolvedValue({
      toolName: 'create_group_chat',
      success: true,
      result: { groupId: 'group-1' },
      error: null,
      validation_error: null,
      duration_ms: 1,
    });
    const { onCreated } = renderDialog();
    await act(async () => { await Promise.resolve(); });

    setGroupName(' 项目群 ');
    expect(getSubmitDisabled()).toBe(false);
    clickCreate();
    await act(async () => { await Promise.resolve(); });

    expect(toolAPI.executeTool).toHaveBeenCalledTimes(1);
    expect(toolAPI.executeTool).toHaveBeenCalledWith({
      toolName: 'create_group_chat',
      parameters: { action: 'create', name: '项目群', members: [], workspace: '/workspace-a' },
      workspacePath: '/workspace-a',
    });
    expect(onCreated).toHaveBeenCalledWith('group-1', '项目群');
    // R-GC-31 (P0): 建群提示单条 = 后端 welcome turn 气泡；前端不再发成功
    // toast（R-GC-29 只精简后端文案未实测，双通道 = 真重复）。
    expect(notificationService.success).not.toHaveBeenCalled();
  });

  it('R-GC-30/R-GC-R6: members = owner-picked multi-select from the runtime list (every real session incl. agentic, not filtered)', async () => {
    // 运行时成员源：listSessions 全部真实会话（R-GC-R6 2026-08-15 主人拍板
    // 不过滤 agentType——含 agentic 的非 Claw 会话也进候选）。
    vi.mocked(sessionAPI.listSessions).mockResolvedValue([
      makeSession('claw-1', 'Claw', 'Assist A'),
      makeSession('claw-2', 'Claw', 'Assist B'),
      makeSession('gen-1', 'GeneralPurpose', 'Agentic C'),
    ]);
    vi.mocked(toolAPI.executeTool).mockResolvedValue({
      toolName: 'create_group_chat',
      success: true,
      result: { groupId: 'group-9' },
    });
    const { onCreated } = renderDialog();
    await act(async () => { await Promise.resolve(); });

    // 无数量输入（R-GC-30 删掉 R-GC-28 误加的数量选择）。
    expect(document.querySelector('[data-testid="member-count-input"]')).toBeNull();
    // 成员列表 = 全部真实会话（Claw + agentic 均进候选，不过滤）。
    const checkboxes = [...document.querySelectorAll<HTMLInputElement>('[data-testid="member-checkbox"]')];
    expect(checkboxes).toHaveLength(3);

    setGroupName('群A');
    toggleMember(0);
    toggleMember(1);
    toggleMember(2);
    clickCreate();
    await act(async () => { await Promise.resolve(); });

    expect(toolAPI.executeTool).toHaveBeenCalledWith(expect.objectContaining({
      parameters: {
        action: 'create',
        name: '群A',
        members: ['claw-1', 'claw-2', 'gen-1'],
        workspace: '/workspace-a',
      },
    }));
    expect(onCreated).toHaveBeenCalledWith('group-9', '群A');
  });

  it('R-GC-33: members = real Claw sessions across ALL assistant workspace roots (no fabricated presets)', async () => {
    // R-GC-33: 每个 assistant workspace rootPath 都被查询，返回该工作区真实
    // 持久化 Claw 会话（含未打开的）。不再伪造 inactive preset 假条目。
    vi.mocked(sessionAPI.listSessions).mockImplementation(async (root: string) => {
      if (root === '/workspace-a') {
        return [makeSession('claw-1', 'Claw', 'Assist A')];
      }
      if (root === '/assistant/ws-preset') {
        return [makeSession('claw-preset-1', 'Claw', '姬梦情-审查官')];
      }
      return [];
    });
    vi.mocked(toolAPI.executeTool).mockResolvedValue({
      toolName: 'create_group_chat',
      success: true,
      result: { groupId: 'group-9' },
    });
    const { onCreated } = renderDialog(vi.fn(), vi.fn(), {
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
    await act(async () => { await Promise.resolve(); });

    // 主工作区 (claw-1) ∪ assistant 工作区 (claw-preset-1) = 2 个真实候选。
    expect(sessionAPI.listSessions).toHaveBeenCalledWith('/assistant/ws-preset');
    const checkboxes = [...document.querySelectorAll<HTMLInputElement>('[data-testid="member-checkbox"]')];
    expect(checkboxes).toHaveLength(2);
    const names = [...document.querySelectorAll<HTMLElement>('.group-chat-dialog__member-name')]
      .map(el => el.textContent);
    expect(names).toContain('姬梦情-审查官');

    setGroupName('预设群');
    toggleMember(1);
    clickCreate();
    await act(async () => { await Promise.resolve(); });

    expect(toolAPI.executeTool).toHaveBeenCalledWith(expect.objectContaining({
      parameters: {
        action: 'create',
        name: '预设群',
        members: ['claw-preset-1'],
        workspace: '/workspace-a',
      },
    }));
    expect(onCreated).toHaveBeenCalledWith('group-9', '预设群');
  });

  it('omits workspace parameter when workspacePath is empty (backend default fallback, R-GC-17)', async () => {
    vi.mocked(sessionAPI.listSessions).mockResolvedValue([]);
    vi.mocked(toolAPI.executeTool).mockResolvedValue({
      toolName: 'create_group_chat',
      success: true,
      result: { groupId: 'group-empty-ws' },
    });
    act(() => {
      root.render(
        <CreateGroupChatDialog
          isOpen
          onClose={() => {}}
          workspacePath=""
          onCreated={() => {}}
        />,
      );
    });
    await act(async () => { await Promise.resolve(); });

    setGroupName('空工作区群');
    clickCreate();
    await act(async () => { await Promise.resolve(); });

    expect(toolAPI.executeTool).toHaveBeenCalledTimes(1);
    expect(toolAPI.executeTool).toHaveBeenCalledWith({
      toolName: 'create_group_chat',
      parameters: { action: 'create', name: '空工作区群', members: [], workspace: undefined },
      workspacePath: '',
    });
  });

  it('surfaces backend failure without calling onCreated', async () => {
    vi.mocked(sessionAPI.listSessions).mockResolvedValue([]);
    vi.mocked(toolAPI.executeTool).mockResolvedValue({
      toolName: 'create_group_chat',
      success: false,
      result: null,
      error: 'name is required for create',
      validation_error: null,
      duration_ms: 1,
    });
    const { onCreated } = renderDialog();
    await act(async () => { await Promise.resolve(); });

    setGroupName('空');
    clickCreate();
    await act(async () => { await Promise.resolve(); });

    expect(toolAPI.executeTool).toHaveBeenCalledTimes(1);
    expect(onCreated).not.toHaveBeenCalled();
  });
});
