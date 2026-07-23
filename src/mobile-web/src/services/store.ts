import { create } from 'zustand';
import type {
  SessionInfo,
  ChatMessage,
  WorkspaceInfo,
  ActiveTurnSnapshot,
  AssistantEntry,
} from './RemoteSessionManager';

export type ConnectionStatus = 'idle' | 'pairing' | 'paired' | 'error';
export type ConnectionHealth = 'unpaired' | 'checking' | 'connected' | 'unreachable';

interface MobileStore {
  connectionStatus: ConnectionStatus;
  setConnectionStatus: (s: ConnectionStatus) => void;
  connectionHealth: ConnectionHealth;
  setConnectionHealth: (h: ConnectionHealth) => void;

  currentWorkspace: WorkspaceInfo | null;
  setCurrentWorkspace: (w: WorkspaceInfo | null) => void;

  currentAssistant: AssistantEntry | null;
  setCurrentAssistant: (a: AssistantEntry | null) => void;

  /** One-shot hint after pairing so SessionList matches desktop assistant vs project workspace. */
  pairedDisplayMode: 'pro' | 'assistant' | null;
  setPairedDisplayMode: (m: 'pro' | 'assistant' | null) => void;

  /** Canonical account identity used for ownership checks. Never render this value. */
  authenticatedUserId: string | null;
  setAuthenticatedUserId: (userId: string | null) => void;
  /** Username in account mode, or the user-entered pairing id in legacy mode. */
  authenticatedUserLabel: string | null;
  setAuthenticatedUserLabel: (label: string | null) => void;

  /**
   * Current same-account control target (delegated identity flow).
   * `isHome` marks the QR-paired desktop this mobile session started from.
   */
  controlTarget: { deviceId: string; deviceName: string | null; isHome: boolean } | null;
  setControlTarget: (
    target: { deviceId: string; deviceName: string | null; isHome: boolean } | null,
  ) => void;

  sessions: SessionInfo[];
  setSessions: (s: SessionInfo[]) => void;
  appendSessions: (s: SessionInfo[]) => void;
  updateSessionName: (sessionId: string, name: string) => void;
  removeSession: (sessionId: string) => void;

  activeSessionId: string | null;
  setActiveSessionId: (id: string | null) => void;

  messagesBySession: Record<string, ChatMessage[]>;
  deletedMessageIds: Record<string, Set<string>>;
  getMessages: (sessionId: string) => ChatMessage[];
  setMessages: (sessionId: string, m: ChatMessage[]) => void;
  appendNewMessages: (sessionId: string, messages: ChatMessage[]) => void;
  deleteMessage: (sessionId: string, messageId: string) => void;

  activeTurn: ActiveTurnSnapshot | null;
  setActiveTurn: (t: ActiveTurnSnapshot | null) => void;

  error: string | null;
  setError: (e: string | null) => void;

  resetConnectionState: () => void;
  /** Clear per-device UI state when switching the control target device. */
  resetForDeviceSwitch: () => void;
}

export const useMobileStore = create<MobileStore>((set, get) => ({
  connectionStatus: 'idle',
  setConnectionStatus: (connectionStatus) => set({ connectionStatus }),
  connectionHealth: 'unpaired',
  setConnectionHealth: (connectionHealth) => set({ connectionHealth }),

  currentWorkspace: null,
  setCurrentWorkspace: (currentWorkspace) => set({ currentWorkspace }),

  currentAssistant: null,
  setCurrentAssistant: (currentAssistant) => set({ currentAssistant }),

  pairedDisplayMode: null,
  setPairedDisplayMode: (pairedDisplayMode) => set({ pairedDisplayMode }),

  authenticatedUserId: null,
  setAuthenticatedUserId: (authenticatedUserId) => set({ authenticatedUserId }),
  authenticatedUserLabel: null,
  setAuthenticatedUserLabel: (authenticatedUserLabel) => set({ authenticatedUserLabel }),

  controlTarget: null,
  setControlTarget: (controlTarget) => set({ controlTarget }),

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
  removeSession: (sessionId) =>
    set((state) => {
      const { [sessionId]: _, ...rest } = state.messagesBySession;
      return {
        sessions: state.sessions.filter((s) => s.session_id !== sessionId),
        messagesBySession: rest,
      };
    }),

  activeSessionId: null,
  setActiveSessionId: (activeSessionId) => set({ activeSessionId }),

  messagesBySession: {},
  deletedMessageIds: {},
  getMessages: (sessionId: string) => {
    const msgs = get().messagesBySession[sessionId] || [];
    const deleted = get().deletedMessageIds[sessionId];
    return deleted ? msgs.filter((m) => !deleted.has(m.id)) : msgs;
  },
  setMessages: (sessionId, m) =>
    set((s) => {
      const deleted = s.deletedMessageIds[sessionId];
      const filtered = deleted ? m.filter((msg) => !deleted.has(msg.id)) : m;
      return { messagesBySession: { ...s.messagesBySession, [sessionId]: filtered } };
    }),
  appendNewMessages: (sessionId, messages) =>
    set((s) => {
      if (messages.length === 0) return s;
      const prev = s.messagesBySession[sessionId] || [];
      const existingIds = new Set(prev.map((m) => m.id));
      const deleted = s.deletedMessageIds[sessionId];
      const unique = messages.filter((m) => !existingIds.has(m.id) && !deleted?.has(m.id));
      if (unique.length === 0) return s;
      return {
        messagesBySession: {
          ...s.messagesBySession,
          [sessionId]: [...prev, ...unique],
        },
      };
    }),
  deleteMessage: (sessionId, messageId) =>
    set((s) => {
      const prev = s.messagesBySession[sessionId];
      const deleted = new Set(s.deletedMessageIds[sessionId] || []);
      deleted.add(messageId);
      return {
        deletedMessageIds: { ...s.deletedMessageIds, [sessionId]: deleted },
        messagesBySession: prev
          ? { ...s.messagesBySession, [sessionId]: prev.filter((m) => m.id !== messageId) }
          : s.messagesBySession,
      };
    }),

  activeTurn: null,
  setActiveTurn: (activeTurn) => set({ activeTurn }),

  error: null,
  setError: (error) => set({ error }),

  resetConnectionState: () =>
    set({
      connectionStatus: 'idle',
      currentWorkspace: null,
      currentAssistant: null,
      pairedDisplayMode: null,
      authenticatedUserId: null,
      authenticatedUserLabel: null,
      controlTarget: null,
      sessions: [],
      activeSessionId: null,
      messagesBySession: {},
      deletedMessageIds: {},
      activeTurn: null,
      error: null,
    }),

  resetForDeviceSwitch: () =>
    set({
      currentWorkspace: null,
      currentAssistant: null,
      pairedDisplayMode: null,
      sessions: [],
      activeSessionId: null,
      messagesBySession: {},
      deletedMessageIds: {},
      activeTurn: null,
      error: null,
    }),
}));
