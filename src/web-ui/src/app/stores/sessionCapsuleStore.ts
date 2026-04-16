/**
 * sessionCapsuleStore — controls the centered session-list dialog, task detail dialog,
 * and the left-side floating task management panel.
 *
 * UnifiedTopBar calls openSessionListDialog / toggleTaskPanel;
 * SessionListDialog and SessionCapsule read/write this state.
 */

import { create } from 'zustand';

interface SessionCapsuleStore {
  sessionListDialogOpen: boolean;
  openSessionListDialog: () => void;
  closeSessionListDialog: () => void;

  taskDetailSessionId: string | null;
  openTaskDetail: (sessionId: string) => void;
  closeTaskDetail: () => void;

  /** Whether the left-side floating task management panel is open. */
  taskPanelOpen: boolean;
  openTaskPanel: () => void;
  closeTaskPanel: () => void;
  toggleTaskPanel: () => void;
}

export const useSessionCapsuleStore = create<SessionCapsuleStore>((set) => ({
  sessionListDialogOpen: false,
  openSessionListDialog: () => set({ sessionListDialogOpen: true }),
  closeSessionListDialog: () => set({ sessionListDialogOpen: false }),

  taskDetailSessionId: null,
  openTaskDetail: (sessionId: string) => set({ taskDetailSessionId: sessionId }),
  closeTaskDetail: () => set({ taskDetailSessionId: null }),

  taskPanelOpen: false,
  openTaskPanel: () => set({ taskPanelOpen: true }),
  closeTaskPanel: () => set({ taskPanelOpen: false }),
  toggleTaskPanel: () => set((s) => ({ taskPanelOpen: !s.taskPanelOpen })),
}));
