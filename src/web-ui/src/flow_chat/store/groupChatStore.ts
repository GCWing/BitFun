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
  /** P1-4 fix: unified workspace_path (consumed by every action, avoids '' asymmetry). */
  workspacePath: string;
  setWorkspacePath: (workspacePath: string) => void;
  // listing
  loadRooms: (workspacePath?: string) => Promise<void>;
  // members (P1-1 fix: loadMembers fetches the member list via group_chat_members)
  loadMembers: (roomId: string) => Promise<GroupChatMember[]>;
  // create/manage
  createRoom: (
    name: string,
    owner: GroupChatActor,
    members: string[],
    mode?: GroupChatMode,
  ) => Promise<GroupChatRoom>; // mode defaults to free
  joinRoom: (roomId: string, sessionId: string, actor: GroupChatActor) => Promise<void>;
  leaveRoom: (roomId: string, sessionId: string, actor: GroupChatActor) => Promise<void>;
  deleteRoom: (roomId: string, actor: GroupChatActor) => Promise<void>; // P0-3 fix
  setMode: (roomId: string, mode: GroupChatMode, actor: GroupChatActor) => Promise<void>;
  // messages (P0-2 fix: sendMessage carries author; P2-4 fix: carries urgent)
  sendMessage: (
    roomId: string,
    author: GroupChatActor,
    content: string,
    mentionTargets: GroupChatActor[],
    urgent?: boolean,
  ) => Promise<void>;
  loadMessages: (roomId: string, cursor?: string) => Promise<void>;
  /** P1-1/P2-4 fix: timeout-reminder consumer — scan one room (or all when roomId is omitted). */
  scanTimeouts: (replyTimeoutSecs: number, roomId?: string) => Promise<Array<{ roomId: string; messageId: string; content: string }>>;
  /** P0-3 fix: ingest a member's reply — mark the message Replied and append the reply body to the room stream. */
  ingestReply: (
    roomId: string,
    messageId: string,
    replyContent: string,
    author: GroupChatActor,
    timestamp: number,
  ) => Promise<void>;
  // state
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
      const previous = get().workspacePath;
      if (previous === workspacePath) {
        return;
      }
      set((state) => {
        state.workspacePath = workspacePath;
        // P2-9: switching BETWEEN workspaces clears stale room state (rooms
        // reloaded by loadRooms; members/messages/activeRoomId must not leak
        // across workspaces). The '' → first-path transition is initialization
        // and must NOT wipe pre-seeded data (tests / first mount).
        if (previous !== '') {
          state.rooms = new Map();
          state.members = new Map();
          state.messages = new Map();
          state.activeRoomId = null;
          state.mode = 'free';
          state.roundRobinCursor = 0;
        }
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

    // P0-3: reply ingestion — marks the message Replied server-side and
    // appends the reply body; the room message list is refreshed so the UI
    // shows the reply text and the Replied status.
    ingestReply: async (roomId, messageId, replyContent, author, timestamp) => {
      await api.invoke('group_chat_ingest_reply', {
        workspace_path: get().workspacePath,
        room_id: roomId,
        message_id: messageId,
        reply_content: replyContent,
        author,
        timestamp,
      });
      await get().loadMessages(roomId);
    },

    setActiveRoom: (roomId) => {
      set((state) => {
        state.activeRoomId = roomId;
      });
    },
  })),
);
