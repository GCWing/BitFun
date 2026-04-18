/**
 * Live App scene store — app catalog + lifecycle state.
 */
import { create } from 'zustand';
import type { MiniAppMeta } from '@/infrastructure/api/service-api/MiniAppAPI';

interface LiveAppState {
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

export const useLiveAppStore = create<LiveAppState>((set) => ({
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
    set((state) =>
      state.openedAppIds.includes(id) ? state : { openedAppIds: [...state.openedAppIds, id] }
    ),
  closeApp: (id) =>
    set((state) => ({
      openedAppIds: state.openedAppIds.filter((value) => value !== id),
    })),
  setRunningWorkerIds: (ids) => set({ runningWorkerIds: Array.from(new Set(ids)) }),
  markWorkerRunning: (id) =>
    set((state) =>
      state.runningWorkerIds.includes(id) ? state : { runningWorkerIds: [...state.runningWorkerIds, id] }
    ),
  markWorkerStopped: (id) =>
    set((state) => ({
      runningWorkerIds: state.runningWorkerIds.filter((value) => value !== id),
    })),
}));
