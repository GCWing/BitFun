// @vitest-environment jsdom

import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { GroupChatsSection } from './GroupChatsSection';
import { useGroupChatStore } from '../../../../../flow_chat/store/groupChatStore';
import type { GroupChatMember, GroupChatRoom } from '../../../../../flow_chat/types/flow-chat';

vi.mock('@/infrastructure/contexts/WorkspaceContext', () => ({
  useWorkspaceContext: () => ({ currentWorkspace: { rootPath: '/ws' } }),
}));

vi.mock('@/infrastructure/api/service-api/ApiClient', () => ({
  api: { invoke: vi.fn() },
}));

vi.mock('@/component-library/components/ConfirmDialog/confirmService', () => ({
  confirmWarning: vi.fn(),
}));

import { api } from '@/infrastructure/api/service-api/ApiClient';
const mockedInvoke = vi.mocked(api.invoke);

import { confirmWarning } from '@/component-library/components/ConfirmDialog/confirmService';
const mockedConfirm = vi.mocked(confirmWarning);

function sampleMember(sessionId: string): GroupChatMember {
  return {
    sessionId,
    role: 'member',
    joinedAt: 1,
    agentType: 'Claw',
    displayName: `Assistant ${sessionId}`,
  };
}

function sampleRoom(roomId: string, name: string, mode: GroupChatRoom['mode'] = 'free'): GroupChatRoom {
  return {
    schemaVersion: 1,
    roomId,
    name,
    owner: { kind: 'master' },
    mode,
    roundRobinCursor: 0,
    createdAt: 1,
    lastActiveAt: mode === 'round_robin' ? 3 : 1,
    status: 'active',
    memberLimit: 50,
  };
}

let container: HTMLDivElement;
let root: Root;

function renderSection() {
  act(() => {
    root.render(<GroupChatsSection workspacePath="/ws" isVisible />);
  });
}

describe('GroupChatsSection', () => {
  beforeEach(() => {
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
    mockedConfirm.mockReset();
    mockedInvoke.mockReset();
    useGroupChatStore.setState({
      rooms: new Map(),
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

  it('shows the empty state when no rooms exist', () => {
    renderSection();
    expect(container.querySelector('[data-bf-part="empty"]')).toBeTruthy();
    expect(container.querySelector('[data-bf-part="items"]')).toBeNull();
  });

  it('renders room rows with name, member count, and mode badge', () => {
    useGroupChatStore.setState({
      rooms: new Map([
        ['room-1', sampleRoom('room-1', 'Alpha', 'free')],
        ['room-2', sampleRoom('room-2', 'Beta', 'round_robin')],
      ]),
      // P1-2 修复：成员数来自 members Map（真实成员数），非 memberLimit。
      members: new Map([
        ['room-1', [sampleMember('m-1'), sampleMember('m-2')]],
        ['room-2', [sampleMember('m-3')]],
      ]),
    });
    renderSection();

    const items = Array.from(container.querySelectorAll('[data-bf-part="item"]'));
    expect(items.length).toBe(2);
    // 按 lastActiveAt 降序：Beta(3) 在 Alpha(1) 前。
    expect(items[0].textContent).toContain('Beta');
    expect(items[0].textContent).toContain('1');
    expect(items[1].textContent).toContain('Alpha');
    expect(items[1].textContent).toContain('2');
  });

  it('activates the room on click', () => {
    useGroupChatStore.setState({
      rooms: new Map([['room-1', sampleRoom('room-1', 'Alpha')]]),
    });
    renderSection();

    const item = container.querySelector('[data-bf-part="item"]') as HTMLElement;
    act(() => {
      item.dispatchEvent(new MouseEvent('click', { bubbles: true }));
    });

    expect(useGroupChatStore.getState().activeRoomId).toBe('room-1');
  });

  it('deletes a room after confirmation (P0-3)', async () => {
    useGroupChatStore.setState({
      rooms: new Map([['room-1', sampleRoom('room-1', 'Alpha')]]),
      activeRoomId: 'room-1',
    });
    mockedConfirm.mockResolvedValueOnce(true);
    mockedInvoke.mockResolvedValueOnce(undefined);
    renderSection();

    const deleteButton = container.querySelector('[data-bf-action="delete-room"]') as HTMLElement;
    expect(deleteButton).toBeTruthy();
    await act(async () => {
      deleteButton.dispatchEvent(new MouseEvent('click', { bubbles: true }));
    });

    expect(mockedConfirm).toHaveBeenCalled();
    expect(mockedInvoke).toHaveBeenCalledWith('group_chat_delete', expect.any(Object));
    expect(useGroupChatStore.getState().rooms.has('room-1')).toBe(false);
    expect(useGroupChatStore.getState().activeRoomId).toBeNull();
  });

  it('keeps the room when the delete confirmation is cancelled', async () => {
    useGroupChatStore.setState({
      rooms: new Map([['room-1', sampleRoom('room-1', 'Alpha')]]),
    });
    mockedConfirm.mockResolvedValueOnce(false);
    renderSection();

    const deleteButton = container.querySelector('[data-bf-action="delete-room"]') as HTMLElement;
    await act(async () => {
      deleteButton.dispatchEvent(new MouseEvent('click', { bubbles: true }));
    });

    expect(useGroupChatStore.getState().rooms.has('room-1')).toBe(true);
  });
});
