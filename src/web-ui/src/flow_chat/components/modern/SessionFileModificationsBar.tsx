/**
 * Session file modifications bar.
 * Shows modified files, with vertical list, auto expand/collapse, and cached stats.
 */

import React, { useState, useEffect, useCallback, useMemo, useRef } from 'react';
import { FileEdit, FilePlus, Trash2, ChevronDown, ChevronUp } from 'lucide-react';
import { Tooltip } from '@/component-library';
import { useTranslation } from 'react-i18next';
import { useSnapshotState } from '../../../tools/snapshot_system/hooks/useSnapshotState';
import { createDiffEditorTab } from '../../../shared/utils/tabUtils';
import { snapshotAPI } from '../../../infrastructure/api';
import { useCurrentWorkspace } from '../../../infrastructure/contexts/WorkspaceContext';
import { createLogger } from '@/shared/utils/logger';
import { flowChatStore } from '../../store/FlowChatStore';
import type { FlowChatState } from '../../types/flow-chat';
import './SessionFileModificationsBar.scss';

const log = createLogger('SessionFileModificationsBar');

export interface SessionFileModificationsBarProps {
  /** Session ID. */
  sessionId?: string;
  /** Visible when there are messages. */
  visible: boolean;
  /** Whether the dialog is executing. */
  isExecuting?: boolean;
  /** Compact mode for narrow width. */
  compact?: boolean;
}

interface FileStats {
  filePath: string;
  sourceSessionId: string;
  sourceKind: 'parent' | 'review' | 'deep_review';
  fileName: string;
  additions: number;
  deletions: number;
  operationType: 'write' | 'edit' | 'delete';
  loading?: boolean;
  error?: string;
}

interface SourceFile {
  filePath: string;
  sourceSessionId: string;
  sourceKind: FileStats['sourceKind'];
}

interface StatsCache {
  [filePath: string]: {
    stats: FileStats;
    timestamp: number;
  };
}

/**
 * Session file modifications bar component.
 */
export const SessionFileModificationsBar: React.FC<SessionFileModificationsBarProps> = ({
  sessionId,
  visible,
  isExecuting = false,
  compact = false,
}) => {
  const { t } = useTranslation('flow-chat');
  const { files } = useSnapshotState(sessionId);
  const { workspace: currentWorkspace } = useCurrentWorkspace();
  const [flowChatState, setFlowChatState] = useState<FlowChatState>(() => flowChatStore.getState());
  const [reviewFiles, setReviewFiles] = useState<SourceFile[]>([]);

  const [isExpanded, setIsExpanded] = useState(false);
  const [fileStats, setFileStats] = useState<Map<string, FileStats>>(new Map());
  const [loadingStats, setLoadingStats] = useState(false);

  // Cache to avoid repeated requests for the same file.
  const statsCacheRef = useRef<StatsCache>({});
  const loadingFilesRef = useRef<Set<string>>(new Set());
  const previousSessionIdRef = useRef<string | undefined>(undefined);
  const CACHE_TTL = 10000;

  const initializedRef = useRef(false);

  useEffect(() => flowChatStore.subscribe(setFlowChatState), []);

  const reviewChildSessions = useMemo(() => {
    if (!sessionId) {
      return [];
    }

    return Array.from(flowChatState.sessions.values())
      .filter((session) =>
        session.parentSessionId === sessionId &&
        (session.sessionKind === 'review' || session.sessionKind === 'deep_review'),
      )
      .map((session) => ({
        sessionId: session.sessionId,
        sourceKind: session.sessionKind as 'review' | 'deep_review',
        freshness: `${session.lastActiveAt}:${session.lastFinishedAt ?? 0}:${session.updatedAt ?? 0}`,
      }));
  }, [flowChatState.sessions, sessionId]);

  const reviewChildSessionsKey = useMemo(
    () => reviewChildSessions
      .map((session) => `${session.sessionId}:${session.sourceKind}:${session.freshness}`)
      .join('|'),
    [reviewChildSessions],
  );

  useEffect(() => {
    let cancelled = false;

    if (!sessionId || reviewChildSessions.length === 0) {
      setReviewFiles([]);
      return;
    }

    Promise.all(
      reviewChildSessions.map(async (child) => {
        try {
          const childFiles = await snapshotAPI.getSessionFiles(child.sessionId);
          return childFiles.map((filePath): SourceFile => ({
            filePath,
            sourceSessionId: child.sessionId,
            sourceKind: child.sourceKind,
          }));
        } catch (error) {
          log.warn('Failed to load review child snapshot files', {
            parentSessionId: sessionId,
            childSessionId: child.sessionId,
            error,
          });
          return [];
        }
      }),
    ).then((groups) => {
      if (!cancelled) {
        setReviewFiles(groups.flat());
      }
    });

    return () => {
      cancelled = true;
    };
  }, [reviewChildSessions, reviewChildSessionsKey, sessionId]);

  const sourceFiles = useMemo<SourceFile[]>(() => {
    const parentFiles = files.map((file): SourceFile => ({
      filePath: file.filePath,
      sourceSessionId: sessionId ?? '',
      sourceKind: 'parent',
    }));
    return [...parentFiles, ...reviewFiles];
  }, [files, reviewFiles, sessionId]);

  useEffect(() => {
    const activeKeys = new Set(sourceFiles.map(file => `${file.sourceSessionId}:${file.filePath}`));
    setFileStats(prev => {
      let changed = false;
      const next = new Map<string, FileStats>();
      prev.forEach((stat, key) => {
        if (activeKeys.has(key)) {
          next.set(key, stat);
        } else {
          changed = true;
        }
      });
      return changed ? next : prev;
    });
  }, [sourceFiles]);


  useEffect(() => {
    if (!initializedRef.current && fileStats.size > 0) {
      initializedRef.current = true;
      setIsExpanded(!isExecuting);
    }
  }, [fileStats.size, isExecuting]);

  useEffect(() => {
    if (initializedRef.current) {
      if (!isExecuting && fileStats.size > 0) {
        setIsExpanded(true);
      }
    }
  }, [isExecuting, fileStats.size]);

  /**
   * Load diff stats for files with caching.
   */
  const loadFileStats = useCallback(async (filesToLoad: SourceFile[]) => {
    if (!sessionId || filesToLoad.length === 0) {
      return;
    }

    const sessionChanged = previousSessionIdRef.current !== sessionId;
    if (sessionChanged) {
      log.debug('Session changed, clearing old stats', {
        previousSession: previousSessionIdRef.current,
        currentSession: sessionId
      });
      previousSessionIdRef.current = sessionId;
      statsCacheRef.current = {};
      loadingFilesRef.current.clear();
      setFileStats(new Map());
    }

    const now = Date.now();

    const newFilesToLoad = filesToLoad.filter(file => {
      const sourceKey = `${file.sourceSessionId}:${file.filePath}`;
      if (loadingFilesRef.current.has(sourceKey)) {
        return false;
      }
      const cached = statsCacheRef.current[sourceKey];
      if (cached && now - cached.timestamp < CACHE_TTL) {
        if (!sessionChanged) {
          return false;
        }
      }
      return true;
    });

    if (newFilesToLoad.length === 0) {
      return;
    }

    log.debug('Loading file stats', { count: newFilesToLoad.length });

    setLoadingStats(true);

    try {
      newFilesToLoad.forEach(file => {
        loadingFilesRef.current.add(`${file.sourceSessionId}:${file.filePath}`);
      });

      await Promise.all(
        newFilesToLoad.map(async (file) => {
          const sourceKey = `${file.sourceSessionId}:${file.filePath}`;
          let stats: FileStats | null = null;

          try {
            const statsResp = await snapshotAPI.getSessionFileDiffStats(
              file.sourceSessionId,
              file.filePath,
              currentWorkspace?.rootPath,
            );
            const fileName = file.filePath.split(/[/\\]/).pop() || file.filePath;

            const additions = statsResp.linesAdded;
            const deletions = statsResp.linesRemoved;
            const operationType: 'write' | 'edit' | 'delete' =
              statsResp.changeKind === 'create'
                ? 'write'
                : statsResp.changeKind === 'delete'
                  ? 'delete'
                  : 'edit';

            stats = {
              filePath: file.filePath,
              sourceSessionId: file.sourceSessionId,
              sourceKind: file.sourceKind,
              fileName,
              additions,
              deletions,
              operationType,
            };

            statsCacheRef.current[sourceKey] = {
              stats,
              timestamp: now,
            };
          } catch (error) {
            log.warn('Failed to get file stats', { filePath: file.filePath, error });

            const fileName = file.filePath.split(/[/\\]/).pop() || file.filePath;
            stats = {
              filePath: file.filePath,
              sourceSessionId: file.sourceSessionId,
              sourceKind: file.sourceKind,
              fileName,
              additions: 0,
              deletions: 0,
              operationType: 'edit',
              error: t('sessionFilesBadge.loadFailed'),
            };
          } finally {
            loadingFilesRef.current.delete(sourceKey);
          }

          // Keep only files with changes or errors (filter +0 -0).
          if (stats && (stats.additions > 0 || stats.deletions > 0 || stats.error)) {
            setFileStats(prev => {
              const newMap = new Map(prev);
              newMap.set(sourceKey, stats!);
              return newMap;
            });
          }
        })
      );
    } catch (error) {
      log.error('Failed to load file stats', error);
    } finally {
      setLoadingStats(false);
    }
  }, [sessionId, t, currentWorkspace?.rootPath]);

  useEffect(() => {
    const timeoutId = setTimeout(() => {
      if (sourceFiles.length > 0) {
        loadFileStats(sourceFiles);
      } else {
        setFileStats(new Map());
        statsCacheRef.current = {};
      }
    }, 300);

    return () => clearTimeout(timeoutId);
  }, [sourceFiles, loadFileStats]);

  const totalStats = useMemo(() => {
    let totalAdditions = 0;
    let totalDeletions = 0;

    fileStats.forEach((stat) => {
      totalAdditions += stat.additions;
      totalDeletions += stat.deletions;
    });

    return { totalAdditions, totalDeletions };
  }, [fileStats]);

  const handleFileClick = useCallback(async (stat: FileStats) => {
    if (!sessionId) return;

    try {
      const diffData = await snapshotAPI.getOperationDiff(stat.sourceSessionId, stat.filePath);
      if ((diffData.originalContent || '') === (diffData.modifiedContent || '')) {
        log.debug('Skipping empty session diff', { filePath: stat.filePath, sessionId: stat.sourceSessionId });
        return;
      }
      const fileName = stat.filePath.split(/[/\\]/).pop() || stat.filePath;

      window.dispatchEvent(new CustomEvent('expand-right-panel'));

      setTimeout(() => {
        createDiffEditorTab(
          stat.filePath,
          fileName,
          diffData.originalContent || '',
          diffData.modifiedContent || '',
          false,
          'agent',
          currentWorkspace?.rootPath,
          undefined,
          false,
          {
            titleKind: 'diff',
            duplicateKeyPrefix: 'diff'
          }
        );
      }, 250);
    } catch (error) {
      log.error('Failed to open diff', error);
    }
  }, [sessionId, currentWorkspace?.rootPath]);

  const getOperationIcon = (operationType: 'write' | 'edit' | 'delete') => {
    switch (operationType) {
      case 'write':
        return <FilePlus size={14} className="icon-write" />;
      case 'delete':
        return <Trash2 size={14} className="icon-delete" />;
      default:
        return <FileEdit size={14} className="icon-edit" />;
    }
  };

  if (!visible || !sessionId || fileStats.size === 0) {
    return null;
  }

  return (
    <div className={`session-file-modifications-bar ${compact ? 'session-file-modifications-bar--compact' : ''}`}>
      <div className="session-file-modifications-bar__header">
        <div className="header-info">
          <span className="file-count">{t('sessionFileModificationsBar.filesCount', { count: fileStats.size })}</span>
          <span className="total-stats">
            {totalStats.totalAdditions > 0 && (
              <span className="stat-add">+{totalStats.totalAdditions}</span>
            )}
            {totalStats.totalDeletions > 0 && (
              <span className="stat-del">-{totalStats.totalDeletions}</span>
            )}
          </span>
        </div>

        <Tooltip
          content={isExpanded ? t('sessionFileModificationsBar.collapseList') : t('sessionFileModificationsBar.expandList')}
          placement="top"
        >
          <button
            className="expand-toggle-btn"
            onClick={() => setIsExpanded(!isExpanded)}
            disabled={loadingStats}
          >
            {isExpanded ? (
              <ChevronUp size={16} strokeWidth={2.5} />
            ) : (
              <ChevronDown size={16} strokeWidth={2.5} />
            )}
          </button>
        </Tooltip>
      </div>

      {isExpanded && (
        <div className="session-file-modifications-bar__list">
          {Array.from(fileStats.values()).map((stat) => (
            <Tooltip key={`${stat.sourceSessionId}:${stat.filePath}`} content={stat.filePath} placement="left">
              <div
                className={`file-row file-row--${stat.operationType} ${stat.error ? 'file-row--error' : ''}`}
                onClick={() => !stat.error && handleFileClick(stat)}
              >
                <span className="file-row__icon">
                  {getOperationIcon(stat.operationType)}
                </span>

                <span className="file-row__name">{stat.fileName}</span>
                {stat.sourceKind !== 'parent' ? (
                  <span className="file-row__source">
                    {stat.sourceKind === 'deep_review'
                      ? t('sessionFileModificationsBar.deepReviewSource', { defaultValue: 'Deep review' })
                      : t('sessionFileModificationsBar.reviewSource', { defaultValue: 'Review' })}
                  </span>
                ) : null}

                {stat.error ? (
                  <span className="file-row__error">{stat.error}</span>
                ) : (
                  <span className="file-row__stats">
                    {stat.additions > 0 && (
                      <span className="stat-add">+{stat.additions}</span>
                    )}
                    {stat.deletions > 0 && (
                      <span className="stat-del">-{stat.deletions}</span>
                    )}
                  </span>
                )}
              </div>
            </Tooltip>
          ))}
        </div>
      )}
    </div>
  );
};

SessionFileModificationsBar.displayName = 'SessionFileModificationsBar';
