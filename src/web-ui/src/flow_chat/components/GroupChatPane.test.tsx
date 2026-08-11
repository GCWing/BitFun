// @vitest-environment jsdom

import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { GroupChatPane } from './GroupChatPane';
import { useGroupChatStore } from '../store/groupChatStore';
import type { GroupChatMember, GroupChatMessage, GroupChatRoom } from '../types/flow-chat';

vi.mock('@/infrastructure/api/service-api/ApiClient', () => ({
  api: { invoke: vi.fn() },
}));

import { api } from '@/infrastructure/api/service-api/ApiClient';
const mockedInvoke = vi.mocked(api.invoke);

function sampleRoom(roomId: string): GroupChatRoom {
  return {
    schemaVersion: 1,
    roomId,
    name: 'Alpha Room',
    owner: { kind: 'master' },
    mode: 'free',
    roundRobinCursor: 0,
    createdAt: 1,
    lastActiveAt: 1,
    status: 'active',
    memberLimit: 50,
  };
}

function sampleMembers(): GroupChatMember[] {
  return [
    { sessionId: 'm-1', role: 'owner', joinedAt: 1, agentType: 'Claw', displayName: 'Assistant One' },
    { sessionId: 'm-2', role: 'member', joinedAt: 1, agentType: 'Claw', displayName: 'Assistant Two' },
  ];
}

function sampleMessages(): GroupChatMessage[] {
  return [
    {
      messageId: 'msg-1',
      roomId: 'room-1',
      author: { kind: 'master' },
      kind: 'user',
      content: 'hello from master',
      mentionTargets: [],
      timestamp: 1,
      status: 'delivered',
    },
    {
      messageId: 'msg-2',
      roomId: 'room-1',
      author: { kind: 'claw', sessionId: 'm-1', agentType: 'Claw' },
      kind: 'agent',
      content: 'reply from assistant',
      mentionTargets: [],
      timestamp: 2,
      status: 'replied',
    },
  ];
}

let container: HTMLDivElement;
let root: Root;

function renderPane() {
  act(() => {
    root.render(<GroupChatPane roomId="room-1" isViewportActive />);
  });
}

describe('GroupChatPane', () => {
  beforeEach(() => {
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
    mockedInvoke.mockReset();
    useGroupChatStore.setState({
      rooms: new Map([['room-1', sampleRoom('room-1')]]),
      activeRoomId: 'room-1',
      members: new Map([['room-1', sampleMembers()]]),
      messages: new Map([['room-1', sampleMessages()]]),
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

  it('renders the header with room name, member count, and mode toggle', () => {
    renderPane();
    const header = container.querySelector('[data-bf-part="header"]');
    expect(header?.textContent).toContain('Alpha Room');
    expect(container.querySelector('[data-bf-part="memberCount"]')?.textContent).toContain('2');
    expect(container.querySelector('[data-bf-part="modeToggle"]')).toBeTruthy();
  });

  it('renders messages with sender labels (主人 / member displayName)', () => {
    renderPane();
    const messages = Array.from(container.querySelectorAll('[data-bf-part="message"]'));
    expect(messages.length).toBe(2);
    expect(messages[0].textContent).toContain('主人');
    expect(messages[0].textContent).toContain('hello from master');
    expect(messages[1].textContent).toContain('Assistant One');
    expect(messages[1].textContent).toContain('reply from assistant');
  });

  it('sends a message via sendMessage with master author', async () => {
    // useEffect 触发 loadMembers + loadMessages；Enter 触发 send。
    mockedInvoke.mockResolvedValue([]);
    renderPane();

    const input = container.querySelector('[data-bf-part="textInput"]') as HTMLInputElement;
    // 受控 input：用 native setter + input 事件驱动 React 状态。
    const setter = Object.getOwnPropertyDescriptor(window.HTMLInputElement.prototype, 'value')?.set;
    act(() => {
      setter?.call(input, 'hi group');
      input.dispatchEvent(new Event('input', { bubbles: true }));
    });
    act(() => {
      input.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }));
    });

    const sendCall = mockedInvoke.mock.calls.find(([command]) => command === 'group_chat_send');
    expect(sendCall).toBeTruthy();
    expect(sendCall?.[1]).toEqual(expect.objectContaining({ content: 'hi group' }));
  });

  it('toggles mode via setMode', async () => {
    mockedInvoke.mockResolvedValue(sampleRoom('room-1'));
    renderPane();

    const toggle = container.querySelector('[data-bf-part="modeToggle"]') as HTMLElement;
    await act(async () => {
      toggle.dispatchEvent(new MouseEvent('click', { bubbles: true }));
    });

    const setModeCall = mockedInvoke.mock.calls.find(([command]) => command === 'group_chat_set_mode');
    expect(setModeCall).toBeTruthy();
    expect(setModeCall?.[1]).toEqual(expect.objectContaining({ mode: 'round_robin' }));
  });
});
