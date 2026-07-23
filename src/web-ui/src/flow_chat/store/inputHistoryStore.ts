/**
 * Input history store for navigating previously sent messages.
 * Provides terminal-like up/down arrow navigation through message history.
 * History is now session-scoped - each session maintains its own input history.
 */

import { create } from 'zustand';
import { persist } from 'zustand/middleware';

export interface InputHistoryState {
  /** Map of sessionId to list of previously sent messages (most recent first) */
  messagesBySession: Record<string, string[]>;
  /** Maximum number of messages to keep per session */
  maxHistorySize: number;
  
  /** Add a message to history for a specific session */
  addMessage: (sessionId: string, message: string) => void;
  /** Clear history for a specific session */
  clearHistory: (sessionId?: string) => void;
  /** Get message at index for a specific session (0 = most recent) */
  getMessage: (sessionId: string, index: number) => string | null;
  /** Get total count for a specific session */
  getCount: (sessionId: string) => number;
  /** Get all history for a specific session */
  getSessionHistory: (sessionId: string) => string[];
}

export const useInputHistoryStore = create<InputHistoryState>()(
  persist(
    (set, get) => ({
      messagesBySession: {},
      maxHistorySize: 100,
      
      addMessage: (sessionId: string, message: string) => {
        const trimmed = message.trim();
        if (!trimmed || !sessionId) return;
        
        set((state) => {
          const sessionHistory = state.messagesBySession[sessionId] || [];
          
          // Don't add duplicates in a row
          if (sessionHistory[0] === trimmed) {
            return state;
          }
          
          // Remove the message if it exists elsewhere in history
          const filtered = sessionHistory.filter(m => m !== trimmed);
          
          // Add to front, limit size
          const newMessages = [trimmed, ...filtered].slice(0, state.maxHistorySize);
          
          return {
            messagesBySession: {
              ...state.messagesBySession,
              [sessionId]: newMessages
            }
          };
        });
      },
      
      clearHistory: (sessionId?: string) => {
        if (!sessionId) {
          // Clear all history
          set({ messagesBySession: {} });
        } else {
          // Clear only specific session
          set((state) => {
            const newHistory = { ...state.messagesBySession };
            delete newHistory[sessionId];
            return { messagesBySession: newHistory };
          });
        }
      },
      
      getMessage: (sessionId: string, index: number) => {
        const { messagesBySession } = get();
        const sessionHistory = messagesBySession[sessionId] || [];
        if (index < 0 || index >= sessionHistory.length) return null;
        return sessionHistory[index];
      },
      
      getCount: (sessionId: string) => {
        const { messagesBySession } = get();
        return (messagesBySession[sessionId] || []).length;
      },
      
      getSessionHistory: (sessionId: string) => {
        const { messagesBySession } = get();
        return messagesBySession[sessionId] || [];
      },
    }),
    {
      name: 'bitfun-input-history',
      version: 2, // Bump version to migrate from old format
      migrate: (persistedState: any, version: number) => {
        if (version < 2) {
          // Migrate from old global format to new session-scoped format
          // Old format: { messages: string[] }
          // New format: { messagesBySession: Record<string, string[]> }
          return {
            messagesBySession: {},
            maxHistorySize: persistedState.maxHistorySize || 100,
            // Don't migrate old global history - users will start fresh
          };
        }
        return persistedState;
      },
    }
  )
);
