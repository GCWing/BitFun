// @vitest-environment jsdom

import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { GroupChatPane, buildGroupChatSubmission } from './GroupChatPane';
import { useGroupChatStore } from '../store/groupChatStore';
import type { GroupChatMember, GroupChatMessage, GroupChatRoom } from '../types/flow-chat';
import type { ChatInputSubmission } from './chatInputRegistration';
import type { SessionReferenceContext } from '@/shared/types/context';

vi.mock('@/infrastructure/api/service-api/ApiClient', () => ({
  api: { invoke: vi.fn() },
}));

// The shared ChatInput is a heavy composer; stub it here so the pane-level
// tests stay focused on GroupChatPane wiring (Task A: full ChatInput reuse
// is verified by the registration contract + buildGroupChatSubmission tests).
vi.mock('./ChatInput', () => ({
  ChatInput: (props: { registration?: { onSubmit?: unknown } }) =>
    React.createElement('div', {
      'data-testid': 'chat-input-textarea',
      'data-registration': props.registration ? 'present' : undefined,
    }),
}));

vi.mock('@/infrastructure/contexts/WorkspaceContext', () => ({
  useOptionalWorkspaceContext: () => ({ workspacePath: '/ws' }),
  useWorkspaceContext: () => ({
    activeWorkspace: null,
    loading: false,
    error: null,
    hasWorkspace: true,
    workspaceName: 'ws',
    workspacePath: '/ws',
    openedWorkspaces: { values: () => [] },
  }),
  useCurrentWorkspace: () => ({
    workspace: null,
    loading: false,
    error: null,
    hasWorkspace: true,
    workspaceName: 'ws',
    workspacePath: '/ws',
  }),
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

  it('renders the shared ChatInput in the input footer (Task A)', () => {
    renderPane();
    // The full composer is reused instead of the legacy plain-text input.
    expect(container.querySelector('[data-bf-part="textInput"]')).toBeNull();
    expect(container.querySelector('[data-testid="chat-input-textarea"]')).toBeTruthy();
  });

  it('normalizes group-member mentions into body + mention targets (buildGroupChatSubmission)', () => {
    const memberContext: SessionReferenceContext = {
      id: 'group-member-m-1',
      timestamp: 1,
      type: 'session-reference',
      sessionId: 'm-1',
      sessionName: 'Assistant One',
      workspacePath: '/ws',
      workspaceLabel: '@all',
      metadata: { groupChatMention: { kind: 'claw', sessionId: 'm-1', agentType: 'Claw' } },
    };
    const submission: ChatInputSubmission = {
      text: 'Please review [session-ref:1] the draft',
      displayText: 'Please review [Session reference: Assistant One] the draft',
      contexts: [memberContext],
      composerPresentation: null,
    };
    const result = buildGroupChatSubmission(submission, []);
    expect(result.text).toBe('Please review @Assistant One the draft');
    expect(result.mentionTargets).toEqual([{ kind: 'claw', sessionId: 'm-1', agentType: 'Claw' }]);
  });

  it('deduplicates pending and context mention targets', () => {
    const memberContext: SessionReferenceContext = {
      id: 'group-member-m-2',
      timestamp: 1,
      type: 'session-reference',
      sessionId: 'm-2',
      sessionName: 'Assistant Two',
      workspacePath: '/ws',
      workspaceLabel: '@all',
      metadata: { groupChatMention: { kind: 'claw', sessionId: 'm-2', agentType: 'Claw' } },
    };
    const submission: ChatInputSubmission = {
      text: 'hi',
      displayText: 'hi',
      contexts: [memberContext],
      composerPresentation: null,
    };
    const result = buildGroupChatSubmission(submission, [{ kind: 'claw', sessionId: 'm-2', agentType: 'Claw' }]);
    expect(result.mentionTargets).toEqual([{ kind: 'claw', sessionId: 'm-2', agentType: 'Claw' }]);
  });

  it('sends a message via sendMessage with master author', async () => {
    // useEffect 触发 loadMembers + loadMessages；通过 store 直接驱动发送路径。
    mockedInvoke.mockResolvedValue([]);
    renderPane();

    await useGroupChatStore.getState().sendMessage('room-1', { kind: 'master' }, 'hi group', []);

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
