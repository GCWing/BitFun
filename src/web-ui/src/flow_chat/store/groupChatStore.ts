/**
 * Group chat store — zustand + immer state management for group chat rooms.
 *
 * Contract: type-contract v1.3 §2.2 (R-GC-14).
 * Every action calls the corresponding `group_chat_*` Tauri command
 * (R-GC-12, P2-1: 11 commands unified naming).
 */

import { create } from 'zustand';
import { immer } from 'zustand/middleware/immer';
import { api } from '@/infrastructure/api/service-api/ApiClient';
import type {
  GroupChatActor,
  GroupChatMember,
  GroupChatMessage,
  GroupChatMode,
  GroupChatRoom,
  GroupChatState,
} from '../types/flow-chat';

export interface GroupChatStore extends GroupChatState {
  /** P1-4 修复：统一 workspace_path（各 action 消费，避免 '' 不对称）。 */
  workspacePath: string;
  setWorkspacePath: (workspacePath: string) => void;
  // 列表
  loadRooms: (workspacePath?: string) => Promise<void>;
  // 成员（P1-1 修复：loadMembers 拉取成员列表，消费 group_chat_members command）
  loadMembers: (roomId: string) => Promise<GroupChatMember[]>;
  // 创建/管理
  createRoom: (
    name: string,
    owner: GroupChatActor,
    members: string[],
    mode?: GroupChatMode,
  ) => Promise<GroupChatRoom>; // mode 默认 free
  joinRoom: (roomId: string, sessionId: string, actor: GroupChatActor) => Promise<void>;
  leaveRoom: (roomId: string, sessionId: string, actor: GroupChatActor) => Promise<void>;
  deleteRoom: (roomId: string, actor: GroupChatActor) => Promise<void>; // P0-3 修复
  setMode: (roomId: string, mode: GroupChatMode, actor: GroupChatActor) => Promise<void>;
  // 消息（P0-2 修复：sendMessage 带 author；P2-4 修复：带 urgent）
  sendMessage: (
    roomId: string,
    author: GroupChatActor,
    content: string,
    mentionTargets: GroupChatActor[],
    urgent?: boolean,
  ) => Promise<void>;
  loadMessages: (roomId: string, cursor?: string) => Promise<void>;
  /** P1-1 修复：超时提醒消费端——扫描全部房间超时消息（消费 reply_timeout_secs）。 */
  scanTimeouts: (replyTimeoutSecs: number) => Promise<Array<{ roomId: string; messageId: string; content: string }>>;
  // 状态
  setActiveRoom: (roomId: string) => void;
}

export const useGroupChatStore = create<GroupChatStore>()(
  immer((set, get) => ({
    rooms: new Map<string, GroupChatRoom>(),
    activeRoomId: null,
    members: new Map<string, GroupChatMember[]>(),
    messages: new Map<string, GroupChatMessage[]>(),
    mode: 'free',
    roundRobinCursor: 0,
    workspacePath: '',
    setWorkspacePath: (workspacePath) => {
      if (get().workspacePath === workspacePath) {
        return;
      }
      set((state) => {
        state.workspacePath = workspacePath;
      });
    },

    loadRooms: async (workspacePath?: string) => {
      const effectivePath = workspacePath ?? get().workspacePath;
      const rooms = (await api.invoke('group_chat_list', {
        workspace_path: effectivePath,
      })) as GroupChatRoom[];
      set((state) => {
        state.rooms = new Map(rooms.map((room) => [room.roomId, room]));
        if (workspacePath !== undefined) {
          state.workspacePath = workspacePath;
        }
      });
    },

    loadMembers: async (roomId: string) => {
      const members = (await api.invoke('group_chat_members', {
        workspace_path: get().workspacePath,
        room_id: roomId,
      })) as GroupChatMember[];
      set((state) => {
        state.members.set(roomId, members);
      });
      return members;
    },

    createRoom: async (name, owner, members, mode) => {
      const room = (await api.invoke('group_chat_create', {
        workspace_path: get().workspacePath,
        name,
        owner,
        members,
        mode: mode ?? 'free',
      })) as GroupChatRoom;
      set((state) => {
        state.rooms.set(room.roomId, room);
      });
      return room;
    },

    joinRoom: async (roomId, sessionId, actor) => {
      const room = (await api.invoke('group_chat_join', {
        workspace_path: get().workspacePath,
        room_id: roomId,
        session_id: sessionId,
        actor,
      })) as GroupChatRoom;
      set((state) => {
        state.rooms.set(roomId, room);
      });
    },

    leaveRoom: async (roomId, sessionId, actor) => {
      const room = (await api.invoke('group_chat_leave', {
        workspace_path: get().workspacePath,
        room_id: roomId,
        session_id: sessionId,
        actor,
      })) as GroupChatRoom;
      set((state) => {
        state.rooms.set(roomId, room);
      });
    },

    deleteRoom: async (roomId, actor) => {
      await api.invoke('group_chat_delete', {
        workspace_path: get().workspacePath,
        room_id: roomId,
        actor,
      });
      set((state) => {
        state.rooms.delete(roomId);
        state.members.delete(roomId);
        state.messages.delete(roomId);
        if (state.activeRoomId === roomId) {
          state.activeRoomId = null;
        }
      });
    },

    setMode: async (roomId, mode, actor) => {
      const room = (await api.invoke('group_chat_set_mode', {
        workspace_path: get().workspacePath,
        room_id: roomId,
        mode,
        actor,
      })) as GroupChatRoom;
      set((state) => {
        state.rooms.set(roomId, room);
        state.mode = mode;
        state.roundRobinCursor = room.roundRobinCursor;
      });
    },

    sendMessage: async (roomId, author, content, mentionTargets, urgent = false) => {
      await api.invoke('group_chat_send', {
        workspace_path: get().workspacePath,
        room_id: roomId,
        author,
        content,
        mention_targets: mentionTargets,
        urgent,
      });
    },

    loadMessages: async (roomId, cursor) => {
      const response = (await api.invoke('group_chat_messages', {
        workspace_path: get().workspacePath,
        room_id: roomId,
        limit: cursor ? undefined : 50,
        cursor: cursor ?? undefined,
      })) as { messages: GroupChatMessage[]; nextCursor?: string };
      set((state) => {
        state.messages.set(roomId, response.messages);
      });
    },

    scanTimeouts: async (replyTimeoutSecs: number) => {
      const reminders = (await api.invoke('group_chat_scan_timeouts', {
        workspace_path: get().workspacePath,
        reply_timeout_secs: replyTimeoutSecs,
      })) as Array<{ roomId: string; messageId: string; content: string }>;
      return reminders;
    },

    setActiveRoom: (roomId) => {
      set((state) => {
        state.activeRoomId = roomId;
      });
    },
  })),
);
