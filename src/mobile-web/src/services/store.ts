import { create } from 'zustand';
import type {
  SessionInfo,
  ChatMessage,
  WorkspaceInfo,
  ActiveTurnSnapshot,
  AssistantEntry,
} from './RemoteSessionManager';

export type ConnectionStatus = 'idle' | 'pairing' | 'paired' | 'error';

interface MobileStore {
  connectionStatus: ConnectionStatus;
  setConnectionStatus: (s: ConnectionStatus) => void;

  currentWorkspace: WorkspaceInfo | null;
  setCurrentWorkspace: (w: WorkspaceInfo | null) => void;

  currentAssistant: AssistantEntry | null;
  setCurrentAssistant: (a: AssistantEntry | null) => void;

  /** One-shot hint after pairing so SessionList matches desktop assistant vs project workspace. */
  pairedDisplayMode: 'pro' | 'assistant' | null;
  setPairedDisplayMode: (m: 'pro' | 'assistant' | null) => void;

  authenticatedUserId: string | null;
  setAuthenticatedUserId: (userId: string | null) => void;

  sessions: SessionInfo[];
  setSessions: (s: SessionInfo[]) => void;
  appendSessions: (s: SessionInfo[]) => void;
  updateSessionName: (sessionId: string, name: string) => void;

  activeSessionId: string | null;
  setActiveSessionId: (id: string | null) => void;

  messagesBySession: Record<string, ChatMessage[]>;
  getMessages: (sessionId: string) => ChatMessage[];
  setMessages: (sessionId: string, m: ChatMessage[]) => void;
  appendNewMessages: (sessionId: string, messages: ChatMessage[]) => void;

  activeTurn: ActiveTurnSnapshot | null;
  setActiveTurn: (t: ActiveTurnSnapshot | null) => void;

  error: string | null;
  setError: (e: string | null) => void;
}

export const useMobileStore = create<MobileStore>((set, get) => ({
  connectionStatus: 'idle',
  setConnectionStatus: (connectionStatus) => set({ connectionStatus }),

  currentWorkspace: null,
  setCurrentWorkspace: (currentWorkspace) => set({ currentWorkspace }),

  currentAssistant: null,
  setCurrentAssistant: (currentAssistant) => set({ currentAssistant }),

  pairedDisplayMode: null,
  setPairedDisplayMode: (pairedDisplayMode) => set({ pairedDisplayMode }),

  authenticatedUserId: null,
  setAuthenticatedUserId: (authenticatedUserId) => set({ authenticatedUserId }),

  sessions: [],
  setSessions: (sessions) => set({ sessions }),
  appendSessions: (newSessions) =>
    set((state) => ({ sessions: [...state.sessions, ...newSessions] })),
  updateSessionName: (sessionId, name) =>
    set((state) => ({
      sessions: state.sessions.map((s) =>
        s.session_id === sessionId ? { ...s, name } : s,
      ),
    })),

  activeSessionId: null,
  setActiveSessionId: (activeSessionId) => set({ activeSessionId }),

  messagesBySession: {},
  getMessages: (sessionId: string) => {
    return get().messagesBySession[sessionId] || [];
  },
  setMessages: (sessionId, m) =>
    set((s) => ({
      messagesBySession: { ...s.messagesBySession, [sessionId]: m },
    })),
  appendNewMessages: (sessionId, messages) =>
    set((s) => {
      if (messages.length === 0) return s;
      const prev = s.messagesBySession[sessionId] || [];
      const existingIds = new Set(prev.map((m) => m.id));
      const unique = messages.filter((m) => !existingIds.has(m.id));
      if (unique.length === 0) return s;
      return {
        messagesBySession: {
          ...s.messagesBySession,
          [sessionId]: [...prev, ...unique],
        },
      };
    }),

  activeTurn: null,
  setActiveTurn: (activeTurn) => set({ activeTurn }),

  error: null,
  setError: (error) => set({ error }),
}));
