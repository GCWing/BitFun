import { create } from 'zustand';
import { createLogger } from '@/shared/utils/logger';
import {
  installUpdateWithProgress,
  type UpdateDownloadProgressPayload
} from './installUpdateWithProgress';

const log = createLogger('UpdateInstallStore');

export type UpdateInstallStatus = 'idle' | 'downloading' | 'installed' | 'error';

interface UpdateInstallState {
  status: UpdateInstallStatus;
  progress: UpdateDownloadProgressPayload;
  error: string | null;
  startedAt: number | null;
  startInstall: () => Promise<void>;
  clearError: () => void;
  clearInstalled: () => void;
}

const initialProgress: UpdateDownloadProgressPayload = {
  downloaded: 0,
  total: null
};

export const useUpdateInstallStore = create<UpdateInstallState>((set, get) => ({
  status: 'idle',
  progress: initialProgress,
  error: null,
  startedAt: null,

  startInstall: async () => {
    const status = get().status;
    if (status === 'downloading' || status === 'installed') {
      return;
    }

    set({
      status: 'downloading',
      progress: initialProgress,
      error: null,
      startedAt: Date.now()
    });

    try {
      await installUpdateWithProgress(progress => {
        set({ progress });
      });
      set({ status: 'installed', error: null });
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      log.error('Background update install failed', error);
      set({ status: 'error', error: message });
    }
  },

  clearError: () => {
    set({
      status: 'idle',
      error: null,
      progress: initialProgress,
      startedAt: null
    });
  },

  clearInstalled: () => {
    set({
      status: 'idle',
      error: null,
      progress: initialProgress,
      startedAt: null
    });
  }
}));
