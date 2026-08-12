// @vitest-environment jsdom
/**
 * P0-1 挂载接线测试：点击群聊列表项 → activeRoomId 设置 → GroupChatPane 渲染。
 * 模拟 MainNav 群聊区块的接线（GroupChatsSection + 条件渲染 Pane），
 * 验证「点击 → 面板出现」的宿主链路（审查 P0-1 修复要求）。
 */

import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { GroupChatsSection } from './GroupChatsSection';
import { GroupChatPane } from '../../../../../flow_chat/components/GroupChatPane';
import { useGroupChatStore } from '../../../../../flow_chat/store/groupChatStore';
import type { GroupChatRoom } from '../../../../../flow_chat/types/flow-chat';

vi.mock('@/infrastructure/contexts/WorkspaceContext', () => ({
  useWorkspaceContext: () => ({
    currentWorkspace: { rootPath: '/ws' },
    workspacePath: '/ws',
    workspaceName: 'ws',
    activeWorkspace: null,
    loading: false,
    error: null,
    hasWorkspace: true,
    openedWorkspaces: { values: () => [] },
  }),
  useOptionalWorkspaceContext: () => ({ workspacePath: '/ws' }),
  useCurrentWorkspace: () => ({
    workspace: null,
    loading: false,
    error: null,
    hasWorkspace: true,
    workspaceName: 'ws',
    workspacePath: '/ws',
  }),
}));

vi.mock('@/infrastructure/api/service-api/ApiClient', () => ({
  api: { invoke: vi.fn() },
}));

// The shared ChatInput is a heavy composer with deep store dependencies; stub
// it so the wiring test stays focused on the nav -> pane mount chain.
vi.mock('../../../../../flow_chat/components/ChatInput', () => ({
  ChatInput: () => React.createElement('div', { 'data-testid': 'chat-input-stub' }),
}));

vi.mock('@/component-library/components/ConfirmDialog/confirmService', () => ({
  confirmWarning: vi.fn(() => Promise.resolve(false)),
}));

import { api } from '@/infrastructure/api/service-api/ApiClient';
const mockedInvoke = vi.mocked(api.invoke);

function sampleRoom(roomId: string, name: string): GroupChatRoom {
  return {
    schemaVersion: 1,
    roomId,
    name,
    owner: { kind: 'master' },
    mode: 'free',
    roundRobinCursor: 0,
    createdAt: 1,
    lastActiveAt: 1,
    status: 'active',
    memberLimit: 50,
  };
}

let container: HTMLDivElement;
let root: Root;

describe('GroupChat wiring (P0-1)', () => {
  beforeEach(() => {
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
    mockedInvoke.mockReset();
    mockedInvoke.mockResolvedValue([sampleRoom('room-1', 'Alpha')]);
    useGroupChatStore.setState({
      rooms: new Map([['room-1', sampleRoom('room-1', 'Alpha')]]),
      activeRoomId: null,
      members: new Map(),
      messages: new Map(),
      mode: 'free',
      roundRobinCursor: 0,
      workspacePath: '',
    });
  });

  afterEach(() => {
    act(() => {
      root.unmount();
    });
    container.remove();
  });

  it('clicking a room row activates it and renders the GroupChatPane', async () => {
    // 模拟 MainNav 群聊区块接线：列表 + 条件渲染 Pane。
    function Wiring() {
      const activeRoomId = useGroupChatStore((state) => state.activeRoomId);
      return (
        <div>
          <GroupChatsSection workspacePath="/ws" isVisible />
          {activeRoomId ? <GroupChatPane roomId={activeRoomId} isViewportActive /> : null}
        </div>
      );
    }

    act(() => {
      root.render(<Wiring />);
    });

    // 初始：列表可见，无 Pane。
    expect(container.querySelector('[data-bf-component="group-chats-section"]')).toBeTruthy();
    expect(container.querySelector('[data-bf-component="group-chat-pane"]')).toBeNull();

    // 点击列表项 → activeRoomId 设置 → Pane 渲染。
    const item = container.querySelector('[data-bf-part="item"]') as HTMLElement;
    await act(async () => {
      item.dispatchEvent(new MouseEvent('click', { bubbles: true }));
    });

    expect(useGroupChatStore.getState().activeRoomId).toBe('room-1');
    const pane = container.querySelector('[data-bf-component="group-chat-pane"]');
    expect(pane).toBeTruthy();
    // Pane 头部显示群名（P0-1 修复：点击 → 面板出现）。
    expect(pane?.textContent ?? '').toContain('Alpha');
    // Pane 内成员管理入口可达（R-GC-19 接线）。
    expect(container.querySelector('[data-bf-part="memberToggle"]')).toBeTruthy();
  });
});
