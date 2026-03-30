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
import { diffService } from '../../../tools/editor/services';
import { createLogger } from '@/shared/utils/logger';
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
  fileName: string;
  additions: number;
  deletions: number;
  operationType: 'write' | 'edit' | 'delete';
  loading?: boolean;
  error?: string;
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

  const [isExpanded, setIsExpanded] = useState(false);
  const [fileStats, setFileStats] = useState<Map<string, FileStats>>(new Map());
  const [loadingStats, setLoadingStats] = useState(false);

  // Cache to avoid repeated requests for the same file.
  const statsCacheRef = useRef<StatsCache>({});
  const loadingFilesRef = useRef<Set<string>>(new Set());
  const previousSessionIdRef = useRef<string | undefined>(undefined);
  const CACHE_TTL = 10000;

  const initializedRef = useRef(false);


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
  const loadFileStats = useCallback(async (filesToLoad: typeof files) => {
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
      if (loadingFilesRef.current.has(file.filePath)) {
        return false;
      }
      const cached = statsCacheRef.current[file.filePath];
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
        loadingFilesRef.current.add(file.filePath);
      });

      await Promise.all(
        newFilesToLoad.map(async (file) => {
          let stats: FileStats | null = null;

          try {
            const diffData = await snapshotAPI.getOperationDiff(sessionId, file.filePath);
            const fileName = file.filePath.split(/[/\\]/).pop() || file.filePath;

            let additions = 0;
            let deletions = 0;
            let operationType: 'write' | 'edit' | 'delete' = 'edit';

            if (!diffData.originalContent && diffData.modifiedContent) {
              operationType = 'write';
              additions = diffData.modifiedContent.split('\n').length;
              deletions = 0;
            } else if (diffData.originalContent && !diffData.modifiedContent) {
              operationType = 'delete';
              additions = 0;
              deletions = diffData.originalContent.split('\n').length;
            } else if (diffData.originalContent && diffData.modifiedContent) {
              const result = await diffService.computeDiff(
                diffData.originalContent,
                diffData.modifiedContent,
                { timeout: 3000 }
              );
              additions = result.stats.additions;
              deletions = result.stats.deletions;
            }

            stats = {
              filePath: file.filePath,
              fileName,
              additions,
              deletions,
              operationType,
            };

            statsCacheRef.current[file.filePath] = {
              stats,
              timestamp: now,
            };
          } catch (error) {
            log.warn('Failed to get file stats', { filePath: file.filePath, error });

            const fileName = file.filePath.split(/[/\\]/).pop() || file.filePath;
            stats = {
              filePath: file.filePath,
              fileName,
              additions: 0,
              deletions: 0,
              operationType: 'edit',
              error: t('sessionFilesBadge.loadFailed'),
            };
          } finally {
            loadingFilesRef.current.delete(file.filePath);
          }

          // Keep only files with changes or errors (filter +0 -0).
          if (stats && (stats.additions > 0 || stats.deletions > 0 || stats.error)) {
            setFileStats(prev => {
              const newMap = new Map(prev);
              newMap.set(file.filePath, stats!);
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
  }, [sessionId, t]);

  useEffect(() => {
    const timeoutId = setTimeout(() => {
      if (files.length > 0) {
        loadFileStats(files);
      } else {
        setFileStats(new Map());
        statsCacheRef.current = {};
      }
    }, 300);

    return () => clearTimeout(timeoutId);
  }, [files, loadFileStats]);

  const totalStats = useMemo(() => {
    let totalAdditions = 0;
    let totalDeletions = 0;

    fileStats.forEach((stat) => {
      totalAdditions += stat.additions;
      totalDeletions += stat.deletions;
    });

    return { totalAdditions, totalDeletions };
  }, [fileStats]);

  const handleFileClick = useCallback(async (filePath: string) => {
    if (!sessionId) return;

    try {
      const diffData = await snapshotAPI.getOperationDiff(sessionId, filePath);
      if ((diffData.originalContent || '') === (diffData.modifiedContent || '')) {
        log.debug('Skipping empty session diff', { filePath, sessionId });
        return;
      }
      const fileName = filePath.split(/[/\\]/).pop() || filePath;

      window.dispatchEvent(new CustomEvent('expand-right-panel'));

      setTimeout(() => {
        createDiffEditorTab(
          filePath,
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
            <Tooltip key={stat.filePath} content={stat.filePath} placement="left">
              <div
                className={`file-row file-row--${stat.operationType} ${stat.error ? 'file-row--error' : ''}`}
                onClick={() => !stat.error && handleFileClick(stat.filePath)}
              >
                <span className="file-row__icon">
                  {getOperationIcon(stat.operationType)}
                </span>

                <span className="file-row__name">{stat.fileName}</span>

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
