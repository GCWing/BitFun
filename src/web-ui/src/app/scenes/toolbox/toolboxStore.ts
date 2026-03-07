/**
 * Toolbox scene store — apps list + lifecycle (launched app IDs).
 */
import { create } from 'zustand';
import type { MiniAppMeta } from '@/infrastructure/api/service-api/MiniAppAPI';

interface ToolboxState {
  apps: MiniAppMeta[];
  loading: boolean;
  /** App IDs whose scenes are currently open in the viewport. */
  openedAppIds: string[];
  /** App IDs whose JS workers are currently running. */
  runningWorkerIds: string[];

  setApps: (apps: MiniAppMeta[]) => void;
  setLoading: (loading: boolean) => void;
  openApp: (id: string) => void;
  closeApp: (id: string) => void;
  setRunningWorkerIds: (ids: string[]) => void;
  markWorkerRunning: (id: string) => void;
  markWorkerStopped: (id: string) => void;
}

export const useToolboxStore = create<ToolboxState>((set) => ({
  apps: [],
  loading: false,
  openedAppIds: [],
  runningWorkerIds: [],

  setApps: (apps) =>
    set((state) => {
      const validIds = new Set(apps.map((app) => app.id));
      return {
        apps,
        openedAppIds: state.openedAppIds.filter((id) => validIds.has(id)),
        runningWorkerIds: state.runningWorkerIds.filter((id) => validIds.has(id)),
      };
    }),
  setLoading: (loading) => set({ loading }),

  openApp: (id) =>
    set((s) =>
      s.openedAppIds.includes(id) ? s : { openedAppIds: [...s.openedAppIds, id] }
    ),
  closeApp: (id) =>
    set((s) => ({
      openedAppIds: s.openedAppIds.filter((x) => x !== id),
    })),
  setRunningWorkerIds: (ids) => set({ runningWorkerIds: Array.from(new Set(ids)) }),
  markWorkerRunning: (id) =>
    set((s) =>
      s.runningWorkerIds.includes(id) ? s : { runningWorkerIds: [...s.runningWorkerIds, id] }
    ),
  markWorkerStopped: (id) =>
    set((s) => ({
      runningWorkerIds: s.runningWorkerIds.filter((x) => x !== id),
    })),
}));
