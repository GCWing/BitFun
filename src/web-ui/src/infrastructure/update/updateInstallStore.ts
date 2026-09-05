import { create } from 'zustand';
import { createLogger } from '@/shared/utils/logger';
import { systemAPI } from '@/infrastructure/api';
import { installUpdateWithProgress, type UpdateDownloadProgressPayload } from './installUpdateWithProgress';

const log = createLogger('UpdateInstallStore');

export type UpdateInstallStatus = 'idle' | 'downloading' | 'ready' | 'installing' | 'error';

interface UpdateInstallState {
  status: UpdateInstallStatus;
  progress: UpdateDownloadProgressPayload;
  error: string | null;
  startedAt: number | null;
  version: string | null;
  promptOpen: boolean;
  initialized: boolean;
  initialize: () => Promise<void>;
  startInstall: (replacePending?: boolean) => Promise<void>;
  requestInstall: () => void;
  confirmInstall: () => Promise<void>;
  deferInstall: () => void;
  clearError: () => void;
}

const initialProgress: UpdateDownloadProgressPayload = { downloaded: 0, total: null };
let initialization: Promise<void> | null = null;

export const useUpdateInstallStore = create<UpdateInstallState>((set, get) => ({
  status: 'idle', progress: initialProgress, error: null, startedAt: null,
  version: null, promptOpen: false, initialized: false,

  initialize: async () => {
    if (get().initialized) return;
    if (initialization) return initialization;
    initialization = (async () => {
      try {
        const pending = await systemAPI.getPendingUpdate();
        if (pending) set({ status: 'ready', version: pending.version });
      } catch (error) {
        log.error('Failed to restore pending update', error);
        set({ status: 'error', error: String(error) });
      } finally {
        set({ initialized: true });
      }
    })();
    try { await initialization; } finally { initialization = null; }
  },

  // Both daily and manual prompts use this download-only preparation step.
  startInstall: async (replacePending = false) => {
    await get().initialize();
    if (['downloading', 'installing'].includes(get().status)) return;
    if (get().status === 'ready' && !replacePending) return;
    set({ status: 'downloading', progress: initialProgress, error: null, startedAt: Date.now(), promptOpen: false });
    try {
      const pending = await installUpdateWithProgress(progress => set({ progress }));
      set({ status: 'ready', version: pending.version, promptOpen: true });
    } catch (error) {
      log.error('Update download failed', error);
      set({ status: 'error', error: error instanceof Error ? error.message : String(error) });
    }
  },

  requestInstall: () => {
    if (get().status === 'ready') set({ promptOpen: true, error: null });
  },

  confirmInstall: async () => {
    const { status, version, promptOpen } = get();
    if (status !== 'ready' || !version || !promptOpen) return;
    set({ status: 'installing', error: null });
    try {
      await systemAPI.installPendingUpdate(version);
      // Successful installation restarts the host. Keep the action locked until exit.
    } catch (error) {
      log.error('Update installation failed', error);
      set({ status: 'ready', error: error instanceof Error ? error.message : String(error) });
    }
  },

  deferInstall: () => {
    if (get().status === 'ready') set({ promptOpen: false, error: null });
  },

  clearError: () => {
    set({ status: get().version ? 'ready' : 'idle', error: null, promptOpen: false });
  },
}));
