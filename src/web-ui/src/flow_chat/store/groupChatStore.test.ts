import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { api } from '@/infrastructure/api/service-api/ApiClient';
import type {
  GroupChatActor,
  GroupChatMember,
  GroupChatMessage,
  GroupChatRoom,
} from '../types/flow-chat';
import { useGroupChatStore } from './groupChatStore';

vi.mock('@/infrastructure/api/service-api/ApiClient', () => ({
  api: {
    invoke: vi.fn(),
  },
}));

const mockedInvoke = vi.mocked(api.invoke);

function sampleRoom(roomId: string, name: string, mode: GroupChatRoom['mode'] = 'free'): GroupChatRoom {
  return {
    schemaVersion: 1,
    roomId,
    name,
    owner: { kind: 'master' },
    mode,
    roundRobinCursor: 0,
    createdAt: 1,
    lastActiveAt: 1,
    status: 'active',
    memberLimit: 50,
  };
}

function sampleMember(sessionId: string, role: 'owner' | 'member' = 'member'): GroupChatMember {
  return {
    sessionId,
    role,
    joinedAt: 1,
    agentType: 'Claw',
    displayName: `Assistant ${sessionId}`,
  };
}

function sampleMessage(roomId: string, messageId: string): GroupChatMessage {
  return {
    messageId,
    roomId,
    author: { kind: 'master' },
    kind: 'user',
    content: 'hello',
    mentionTargets: [],
    timestamp: 1,
    status: 'delivered',
  };
}

describe('groupChatStore', () => {
  beforeEach(() => {
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
    vi.clearAllMocks();
  });

  it('loadRooms fills the rooms map via group_chat_list', async () => {
    mockedInvoke.mockResolvedValueOnce([sampleRoom('room-1', 'Alpha'), sampleRoom('room-2', 'Beta')]);

    await useGroupChatStore.getState().loadRooms('/ws');

    expect(mockedInvoke).toHaveBeenCalledWith('group_chat_list', { workspace_path: '/ws' });
    const rooms = useGroupChatStore.getState().rooms;
    expect(rooms.size).toBe(2);
    expect(rooms.get('room-1')?.name).toBe('Alpha');
  });

  it('loadMembers fills the members map via group_chat_members (P1-1)', async () => {
    mockedInvoke.mockResolvedValueOnce([sampleMember('m-1', 'owner'), sampleMember('m-2')]);

    const members = await useGroupChatStore.getState().loadMembers('room-1');

    expect(mockedInvoke).toHaveBeenCalledWith('group_chat_members', {
      workspace_path: '',
      room_id: 'room-1',
    });
    expect(members.length).toBe(2);
    expect(useGroupChatStore.getState().members.get('room-1')?.length).toBe(2);
  });

  it('createRoom adds the room via group_chat_create', async () => {
    mockedInvoke.mockResolvedValueOnce(sampleRoom('room-9', 'New Room'));

    const room = await useGroupChatStore
      .getState()
      .createRoom('New Room', { kind: 'master' }, ['m-1'], 'round_robin');

    expect(mockedInvoke).toHaveBeenCalledWith('group_chat_create', {
      workspace_path: '',
      name: 'New Room',
      owner: { kind: 'master' },
      members: ['m-1'],
      mode: 'round_robin',
    });
    expect(room.roomId).toBe('room-9');
    expect(useGroupChatStore.getState().rooms.has('room-9')).toBe(true);
  });

  it('deleteRoom removes the room and its member/message caches (P0-3)', async () => {
    useGroupChatStore.setState({
      rooms: new Map([['room-1', sampleRoom('room-1', 'Alpha')]]),
      activeRoomId: 'room-1',
      members: new Map([['room-1', [sampleMember('m-1')]]]),
      messages: new Map([['room-1', [sampleMessage('room-1', 'msg-1')]]]),
    });
    mockedInvoke.mockResolvedValueOnce(undefined);

    await useGroupChatStore.getState().deleteRoom('room-1', { kind: 'master' });

    expect(mockedInvoke).toHaveBeenCalledWith('group_chat_delete', {
      workspace_path: '',
      room_id: 'room-1',
      actor: { kind: 'master' },
    });
    const state = useGroupChatStore.getState();
    expect(state.rooms.has('room-1')).toBe(false);
    expect(state.members.has('room-1')).toBe(false);
    expect(state.messages.has('room-1')).toBe(false);
    expect(state.activeRoomId).toBeNull();
  });

  it('sendMessage passes author and urgent via group_chat_send (P0-2/P2-4)', async () => {
    mockedInvoke.mockResolvedValueOnce(undefined);

    const mention: GroupChatActor = { kind: 'claw', sessionId: 'm-2', agentType: 'Claw' };
    await useGroupChatStore
      .getState()
      .sendMessage('room-1', { kind: 'master' }, 'hi', [mention], true);

    expect(mockedInvoke).toHaveBeenCalledWith('group_chat_send', {
      workspace_path: '',
      room_id: 'room-1',
      author: { kind: 'master' },
      content: 'hi',
      mention_targets: [mention],
      urgent: true,
    });
  });

  it('setActiveRoom switches the active room id', () => {
    useGroupChatStore.getState().setActiveRoom('room-5');
    expect(useGroupChatStore.getState().activeRoomId).toBe('room-5');
  });
});
