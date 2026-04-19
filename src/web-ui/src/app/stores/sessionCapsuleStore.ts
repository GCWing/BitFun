/**
 * sessionCapsuleStore — task detail dialog, left SessionCapsule expand signal,
 * and the left-side floating task management panel (TaskManagementPanel).
 *
 * UnifiedTopBar "view all tasks" calls requestExpandSessionList so the capsule
 * expands instead of opening a separate modal.
 */

import { create } from 'zustand';

interface SessionCapsuleStore {
  /** Incremented to ask SessionCapsule to expand the left task list (e.g. from top bar). */
  sessionListExpandNonce: number;
  requestExpandSessionList: () => void;

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
  sessionListExpandNonce: 0,
  requestExpandSessionList: () =>
    set((s) => ({
      sessionListExpandNonce: s.sessionListExpandNonce + 1,
      sessionListDialogOpen: false,
    })),

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
