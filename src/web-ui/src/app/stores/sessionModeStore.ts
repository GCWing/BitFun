/**
 * sessionModeStore — tracks the active session creation mode.
 *
 * Two modes:
 *   - 'code'   → standard AI coding session (default)
 *   - 'cowork' → collaborative multi-user session (placeholder)
 */

import { create } from 'zustand';

export type SessionMode = 'code' | 'cowork';

interface SessionModeState {
  mode: SessionMode;
  setMode: (mode: SessionMode) => void;
}

export const useSessionModeStore = create<SessionModeState>((set) => ({
  mode: 'code',
  setMode: (mode) => set({ mode }),
}));
