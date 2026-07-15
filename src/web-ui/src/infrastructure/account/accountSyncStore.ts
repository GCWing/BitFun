import { create } from 'zustand';
import type { AutoSyncResult } from '@/infrastructure/api/service-api/RemoteConnectAPI';
import { api } from '@/infrastructure/api/service-api/ApiClient';
import { createLogger } from '@/shared/utils/logger';

const log = createLogger('AccountSyncStore');

export type AccountSyncStatus = 'idle' | 'syncing' | 'done' | 'failed';

export type AccountSyncPhase =
  | 'starting'
  | 'uploading_settings'
  | 'downloading_settings'
  | 'applying_settings'
  | 'settings_done'
  | 'listing_sessions'
  | 'exporting_sessions'
  | 'done'
  | 'failed';

export interface AccountSyncProgress {
  phase: AccountSyncPhase;
  percent: number;
  current: number | null;
  total: number | null;
  detail: string | null;
}

interface AccountSyncState {
  status: AccountSyncStatus;
  progress: AccountSyncProgress;
  lastResult: AutoSyncResult | null;
  lastError: string | null;
  setSyncing: () => void;
  applyProgress: (progress: Partial<AccountSyncProgress> & { phase: string }) => void;
  setDone: (result: AutoSyncResult) => void;
  setFailed: (error: string) => void;
  clear: () => void;
}

const INITIAL_PROGRESS: AccountSyncProgress = {
  phase: 'starting',
  percent: 0,
  current: null,
  total: null,
  detail: null,
};

function normalizePhase(phase: string): AccountSyncPhase {
  switch (phase) {
    case 'uploading_settings':
    case 'downloading_settings':
    case 'applying_settings':
    case 'settings_done':
    case 'listing_sessions':
    case 'exporting_sessions':
    case 'done':
    case 'failed':
    case 'starting':
      return phase;
    // Legacy phases from older builds that still imported cloud sessions.
    case 'fetching_remote_sessions':
    case 'importing_sessions':
      return 'exporting_sessions';
    default:
      return 'starting';
  }
}

/**
 * Survives AccountLoginDialog close/reopen so users can reopen Online Devices
 * and still see in-progress cloud sync after choosing local/cloud overwrite.
 */
export const useAccountSyncStore = create<AccountSyncState>((set) => ({
  status: 'idle',
  progress: INITIAL_PROGRESS,
  lastResult: null,
  lastError: null,
  setSyncing: () =>
    set({
      status: 'syncing',
      lastError: null,
      progress: { ...INITIAL_PROGRESS, phase: 'starting', percent: 0 },
    }),
  applyProgress: (progress) =>
    set((state) => ({
      status: progress.phase === 'failed' ? 'failed' : state.status === 'done' ? 'done' : 'syncing',
      progress: {
        phase: normalizePhase(progress.phase),
        percent: typeof progress.percent === 'number'
          ? Math.max(0, Math.min(100, progress.percent))
          : state.progress.percent,
        current: progress.current ?? null,
        total: progress.total ?? null,
        detail: progress.detail ?? null,
      },
    })),
  setDone: (result) =>
    set({
      status: 'done',
      lastResult: result,
      lastError: null,
      progress: {
        phase: 'done',
        percent: 100,
        current: result.sessions_exported,
        total: result.sessions_exported,
        detail: null,
      },
    }),
  setFailed: (error) =>
    set((state) => ({
      status: 'failed',
      lastError: error,
      progress: { ...state.progress, phase: 'failed' },
    })),
  clear: () =>
    set({
      status: 'idle',
      lastResult: null,
      lastError: null,
      progress: INITIAL_PROGRESS,
    }),
}));

let progressUnlisten: (() => void) | null = null;

/** Register once so progress updates continue while the dialog is closed. */
export function ensureAccountSyncProgressListener(): void {
  if (progressUnlisten) {
    return;
  }
  try {
    progressUnlisten = api.listen<AccountSyncProgress>('account://sync-progress', (payload) => {
      if (!payload?.phase) {
        return;
      }
      useAccountSyncStore.getState().applyProgress(payload);
    });
  } catch (error) {
    log.warn('Failed to register account sync progress listener', error);
  }
}
