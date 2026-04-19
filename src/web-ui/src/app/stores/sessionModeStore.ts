/**
 * sessionModeStore — tracks the active session creation mode.
 *
 * Three modes:
 *   - 'code'   → standard AI coding session (default)
 *   - 'cowork' → collaborative Cowork session
 *   - 'design' → dedicated Design session
 */

import { create } from 'zustand';

export type SessionMode = 'code' | 'cowork' | 'design';

interface SessionModeState {
  mode: SessionMode;
  setMode: (mode: SessionMode) => void;
}

export const useSessionModeStore = create<SessionModeState>((set) => ({
  mode: 'code',
  setMode: (mode) => set({ mode }),
}));
