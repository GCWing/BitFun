/**
 * terminalSceneStore — tracks which terminal session is currently
 * displayed inside the standalone Terminal scene.
 *
 * When the user clicks a shell entry in NavPanel while NOT in the
 * AI-Agent scene, we set activeSessionId here and openScene('terminal').
 * TerminalScene reads this to render the specific ConnectedTerminal
 * instead of the list panel.
 *
 * Set to null when the session exits or the user closes the view.
 */

import { create } from 'zustand';

interface TerminalSceneState {
  /** sessionId to render in the terminal scene; null = show list panel. */
  activeSessionId: string | null;
  setActiveSession: (sessionId: string | null) => void;
}

export const useTerminalSceneStore = create<TerminalSceneState>(set => ({
  activeSessionId: null,
  setActiveSession: sessionId => set({ activeSessionId: sessionId }),
}));
