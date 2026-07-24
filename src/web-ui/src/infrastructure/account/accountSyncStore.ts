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
  operation_id: number;
  phase: AccountSyncPhase;
  percent: number;
  current: number | null;
  total: number | null;
  detail: string | null;
}

interface AccountSyncState {
  /** Monotonic generation used to ignore stale detached sync completions. */
  operationId: number;
  status: AccountSyncStatus;
  progress: AccountSyncProgress;
  lastResult: AutoSyncResult | null;
  lastError: string | null;
  /** Last sync direction; true uploads local settings, false downloads cloud settings. */
  lastSyncIsFirstLogin: boolean | null;
  setSyncing: (isFirstLogin: boolean) => void;
  applyProgress: (
    progress: Partial<AccountSyncProgress> & Pick<AccountSyncProgress, 'operation_id'> & { phase: string }
  ) => void;
  setDone: (result: AutoSyncResult) => void;
  setFailed: (error: string) => void;
  clear: () => void;
}

const createInitialProgress = (operationId = 0): AccountSyncProgress => ({
  operation_id: operationId,
  phase: 'starting',
  percent: 0,
  current: null,
  total: null,
  detail: null,
});

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
 * Survives Remote Connect dialog close/reopen so users can reopen My BitFun
 * and still see in-progress cloud sync after choosing local/cloud overwrite.
 */
export const useAccountSyncStore = create<AccountSyncState>((set) => ({
  operationId: 0,
  status: 'idle',
  progress: createInitialProgress(),
  lastResult: null,
  lastError: null,
  lastSyncIsFirstLogin: null,
  setSyncing: (isFirstLogin) =>
    set((state) => {
      const operationId = state.operationId + 1;
      return {
        operationId,
        status: 'syncing',
        lastError: null,
        lastSyncIsFirstLogin: isFirstLogin,
        progress: createInitialProgress(operationId),
      };
    }),
  applyProgress: (progress) =>
    set((state) => {
      if (progress.operation_id !== state.operationId) {
        return state;
      }
      return {
        status: progress.phase === 'failed' ? 'failed' : state.status === 'done' ? 'done' : 'syncing',
        progress: {
          operation_id: state.operationId,
          phase: normalizePhase(progress.phase),
          percent: typeof progress.percent === 'number'
            ? Math.max(0, Math.min(100, progress.percent))
            : state.progress.percent,
          current: progress.current ?? null,
          total: progress.total ?? null,
          detail: progress.detail ?? null,
        },
      };
    }),
  setDone: (result) =>
    set((state) => ({
      status: 'done',
      lastResult: result,
      lastError: null,
      progress: {
        operation_id: state.operationId,
        phase: 'done',
        percent: 100,
        current: result.sessions_exported,
        total: result.sessions_exported,
        detail: null,
      },
    })),
  setFailed: (error) =>
    set((state) => ({
      status: 'failed',
      lastError: error,
      progress: { ...state.progress, phase: 'failed' },
    })),
  clear: () =>
    set((state) => {
      const operationId = state.operationId + 1;
      return {
        operationId,
        status: 'idle',
        lastResult: null,
        lastError: null,
        lastSyncIsFirstLogin: null,
        progress: createInitialProgress(operationId),
      };
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
      const state = useAccountSyncStore.getState();
      // Logout/device removal invalidates the active operation by returning the
      // store to idle. Ignore late backend progress from the detached request.
      if (state.status === 'idle' || payload.operation_id !== state.operationId) {
        return;
      }
      state.applyProgress(payload);
    });
  } catch (error) {
    log.warn('Failed to register account sync progress listener', error);
  }
}
