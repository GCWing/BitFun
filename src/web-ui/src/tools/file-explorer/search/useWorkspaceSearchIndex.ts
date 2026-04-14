import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { workspaceAPI } from '@/infrastructure/api';
import type {
  WorkspaceSearchIndexStatus,
  WorkspaceSearchIndexTaskHandle,
} from '@/infrastructure/api/service-api/tauri-commands';
import { createLogger } from '@/shared/utils/logger';

const log = createLogger('useWorkspaceSearchIndex');
const ACTIVE_TASK_POLL_MS = 1000;
const IDLE_STATUS_POLL_MS = 5000;

export interface UseWorkspaceSearchIndexOptions {
  workspacePath?: string;
  enabled?: boolean;
}

export interface UseWorkspaceSearchIndexResult {
  indexStatus: WorkspaceSearchIndexStatus | null;
  loading: boolean;
  refreshing: boolean;
  actionRunning: boolean;
  error: string | null;
  supported: boolean;
  hasActiveTask: boolean;
  refreshStatus: (silent?: boolean) => Promise<WorkspaceSearchIndexStatus | null>;
  buildIndex: () => Promise<WorkspaceSearchIndexTaskHandle | null>;
  rebuildIndex: () => Promise<WorkspaceSearchIndexTaskHandle | null>;
}

function isTaskActive(status: WorkspaceSearchIndexStatus | null): boolean {
  const state = status?.activeTask?.state;
  return state === 'queued' || state === 'running';
}

export function useWorkspaceSearchIndex(
  options: UseWorkspaceSearchIndexOptions = {}
): UseWorkspaceSearchIndexResult {
  const { workspacePath, enabled = true } = options;
  const supported = Boolean(workspacePath && enabled);

  const [indexStatus, setIndexStatus] = useState<WorkspaceSearchIndexStatus | null>(null);
  const [loading, setLoading] = useState(false);
  const [refreshing, setRefreshing] = useState(false);
  const [actionRunning, setActionRunning] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const mountedRef = useRef(true);
  const pollTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const clearPollTimer = useCallback(() => {
    if (pollTimerRef.current) {
      clearTimeout(pollTimerRef.current);
      pollTimerRef.current = null;
    }
  }, []);

  const refreshStatus = useCallback(
    async (silent: boolean = false): Promise<WorkspaceSearchIndexStatus | null> => {
      if (!workspacePath || !enabled) {
        if (mountedRef.current) {
          setIndexStatus(null);
          setError(null);
        }
        return null;
      }

      if (mountedRef.current) {
        if (silent) {
          setRefreshing(true);
        } else {
          setLoading(true);
        }
      }

      try {
        const status = await workspaceAPI.getSearchRepoStatus(workspacePath);
        if (!mountedRef.current) {
          return status;
        }
        setIndexStatus(status);
        setError(null);
        return status;
      } catch (err) {
        if (!mountedRef.current) {
          return null;
        }
        const message = err instanceof Error ? err.message : 'Failed to load search index status';
        log.warn('Failed to refresh workspace search index status', {
          workspacePath,
          error: err,
        });
        setError(message);
        return null;
      } finally {
        if (mountedRef.current) {
          setLoading(false);
          setRefreshing(false);
        }
      }
    },
    [enabled, workspacePath]
  );

  const runIndexAction = useCallback(
    async (
      action: 'build' | 'rebuild'
    ): Promise<WorkspaceSearchIndexTaskHandle | null> => {
      if (!workspacePath || !enabled) {
        return null;
      }

      setActionRunning(true);
      try {
        const result =
          action === 'build'
            ? await workspaceAPI.buildSearchIndex(workspacePath)
            : await workspaceAPI.rebuildSearchIndex(workspacePath);
        if (mountedRef.current) {
          setIndexStatus({
            repoStatus: result.repoStatus,
            activeTask: result.task,
          });
          setError(null);
        }
        return result;
      } catch (err) {
        if (mountedRef.current) {
          const message = err instanceof Error ? err.message : `Failed to ${action} search index`;
          setError(message);
        }
        return null;
      } finally {
        if (mountedRef.current) {
          setActionRunning(false);
        }
      }
    },
    [enabled, workspacePath]
  );

  const buildIndex = useCallback(async () => runIndexAction('build'), [runIndexAction]);
  const rebuildIndex = useCallback(async () => runIndexAction('rebuild'), [runIndexAction]);

  useEffect(() => {
    mountedRef.current = true;
    return () => {
      mountedRef.current = false;
      clearPollTimer();
    };
  }, [clearPollTimer]);

  useEffect(() => {
    clearPollTimer();

    if (!supported) {
      setIndexStatus(null);
      setLoading(false);
      setRefreshing(false);
      setActionRunning(false);
      setError(null);
      return;
    }

    let cancelled = false;

    const scheduleNext = (status: WorkspaceSearchIndexStatus | null) => {
      if (cancelled || !mountedRef.current) {
        return;
      }
      const delay = isTaskActive(status) ? ACTIVE_TASK_POLL_MS : IDLE_STATUS_POLL_MS;
      pollTimerRef.current = setTimeout(() => {
        void refreshStatus(true).then((nextStatus) => {
          scheduleNext(nextStatus);
        });
      }, delay);
    };

    void refreshStatus(false).then((status) => {
      if (!cancelled) {
        scheduleNext(status);
      }
    });

    return () => {
      cancelled = true;
      clearPollTimer();
    };
  }, [clearPollTimer, refreshStatus, supported, workspacePath]);

  return useMemo(
    () => ({
      indexStatus,
      loading,
      refreshing,
      actionRunning,
      error,
      supported,
      hasActiveTask: isTaskActive(indexStatus),
      refreshStatus,
      buildIndex,
      rebuildIndex,
    }),
    [
      actionRunning,
      buildIndex,
      error,
      indexStatus,
      loading,
      rebuildIndex,
      refreshStatus,
      refreshing,
      supported,
    ]
  );
}

export default useWorkspaceSearchIndex;
