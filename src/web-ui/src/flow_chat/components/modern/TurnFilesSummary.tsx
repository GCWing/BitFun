/**
 * Turn files summary.
 * Shows a compact button with file count and line changes for a single completed turn.
 * Click to expand a popover listing all files modified in that turn.
 * Each file opens a diff editor tab when clicked.
 */

import React, { useState, useCallback, useEffect, useRef, useMemo } from 'react';
import {
  FileEdit,
  FilePlus,
  Trash2,
  ChevronDown,
  ChevronUp,
  FileCode2,
} from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { snapshotAPI } from '../../../infrastructure/api';
import { createDiffEditorTab } from '../../../shared/utils/tabUtils';
import { useWorkspaceContext } from '../../../infrastructure/contexts/WorkspaceContext';
import { createLogger } from '@/shared/utils/logger';
import { runWithConcurrencyLimit } from '@/shared/utils/runWithConcurrencyLimit';
import './TurnFilesSummary.scss';

const log = createLogger('TurnFilesSummary');

const CACHE_TTL = 60000;
const DIFF_STATS_MAX_CONCURRENCY = 3;

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

export interface TurnFilesSummaryProps {
  sessionId?: string;
  turnIndex: number;
}

export const TurnFilesSummary: React.FC<TurnFilesSummaryProps> = ({
  sessionId,
  turnIndex,
}) => {
  const { t } = useTranslation('flow-chat');
  const { currentWorkspace } = useWorkspaceContext();
  const [isExpanded, setIsExpanded] = useState(false);
  const [files, setFiles] = useState<string[]>([]);
  const [fileStats, setFileStats] = useState<Map<string, FileStats>>(new Map());
  const [loading, setLoading] = useState(false);
  const [loadingStats, setLoadingStats] = useState(false);

  const containerRef = useRef<HTMLDivElement>(null);
  const popoverRef = useRef<HTMLDivElement>(null);
  const statsCacheRef = useRef<StatsCache>({});
  const loadingFilesRef = useRef<Set<string>>(new Set());
  const activeFilePathsRef = useRef<Set<string>>(new Set());
  const loadedTurnRef = useRef<number | null>(null);

  // Load turn files when the turn completes.
  useEffect(() => {
    if (!sessionId) {
      setFiles([]);
      setFileStats(new Map());
      return;
    }

    if (loadedTurnRef.current === turnIndex) {
      return;
    }

    let cancelled = false;
    setLoading(true);
    setFiles([]);
    setFileStats(new Map());

    snapshotAPI.getTurnFiles(sessionId, turnIndex, currentWorkspace?.rootPath)
      .then((result) => {
        if (cancelled) return;
        loadedTurnRef.current = turnIndex;
        setFiles(result);
      })
      .catch((error) => {
        if (cancelled) return;
        log.warn('Failed to load turn files', { sessionId, turnIndex, error });
        setFiles([]);
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });

    return () => {
      cancelled = true;
    };
  }, [sessionId, turnIndex, currentWorkspace?.rootPath]);

  // Sync active file paths.
  useEffect(() => {
    const activeFilePaths = new Set(files);
    activeFilePathsRef.current = activeFilePaths;

    setFileStats(prev => {
      let changed = false;
      const next = new Map<string, FileStats>();
      prev.forEach((stat, filePath) => {
        if (activeFilePaths.has(filePath)) {
          next.set(filePath, stat);
        } else {
          changed = true;
        }
      });
      return changed ? next : prev;
    });

    for (const filePath of Object.keys(statsCacheRef.current)) {
      if (!activeFilePaths.has(filePath)) {
        delete statsCacheRef.current[filePath];
      }
    }

    for (const filePath of Array.from(loadingFilesRef.current)) {
      if (!activeFilePaths.has(filePath)) {
        loadingFilesRef.current.delete(filePath);
      }
    }
  }, [files]);

  // Fetch per-file diff stats.
  const loadFileStats = useCallback(async (filePaths: string[]) => {
    if (!sessionId || filePaths.length === 0) {
      return;
    }

    const now = Date.now();

    const newFilesToLoad = filePaths.filter(filePath => {
      if (loadingFilesRef.current.has(filePath)) {
        return false;
      }
      const cached = statsCacheRef.current[filePath];
      if (cached && now - cached.timestamp < CACHE_TTL) {
        return false;
      }
      return true;
    });

    if (newFilesToLoad.length === 0) {
      return;
    }

    setLoadingStats(true);

    try {
      newFilesToLoad.forEach(filePath => {
        loadingFilesRef.current.add(filePath);
      });

      const batchResults = await runWithConcurrencyLimit(
        newFilesToLoad,
        DIFF_STATS_MAX_CONCURRENCY,
        async (filePath) => {
          let stats: FileStats | null = null;

          try {
            const statsResp = await snapshotAPI.getSessionFileDiffStats(
              sessionId,
              filePath,
              currentWorkspace?.rootPath,
            );
            const fileName = filePath.split(/[/\\]/).pop() || filePath;

            const additions = statsResp.linesAdded;
            const deletions = statsResp.linesRemoved;
            const operationType: 'write' | 'edit' | 'delete' =
              statsResp.changeKind === 'create'
                ? 'write'
                : statsResp.changeKind === 'delete'
                  ? 'delete'
                  : 'edit';

            stats = {
              filePath,
              fileName,
              additions,
              deletions,
              operationType,
            };

            if (activeFilePathsRef.current.has(filePath)) {
              statsCacheRef.current[filePath] = {
                stats,
                timestamp: now,
              };
            }
          } catch (error) {
            log.warn('Failed to get file stats', { filePath, error });
            const fileName = filePath.split(/[/\\]/).pop() || filePath;
            stats = {
              filePath,
              fileName,
              additions: 0,
              deletions: 0,
              operationType: 'edit',
              error: t('sessionFilesBadge.loadFailed'),
            };
          } finally {
            loadingFilesRef.current.delete(filePath);
          }

          return { filePath, stats };
        },
      );

      setFileStats(prev => {
        const newMap = new Map(prev);
        for (const { filePath, stats } of batchResults) {
          if (
            activeFilePathsRef.current.has(filePath) &&
            stats &&
            (stats.additions > 0 || stats.deletions > 0 || stats.error)
          ) {
            newMap.set(filePath, stats);
          }
        }
        return newMap;
      });
    } catch (error) {
      log.error('Failed to load file stats', error);
    } finally {
      setLoadingStats(false);
    }
  }, [sessionId, t, currentWorkspace?.rootPath]);

  // Reload stats when the file list changes.
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

  // Close popover when clicking outside.
  useEffect(() => {
    if (!isExpanded) return;

    const handleClickOutside = (event: MouseEvent) => {
      const target = event.target as Node;
      const clickedContainer = !!containerRef.current?.contains(target);
      const clickedPopover = !!popoverRef.current?.contains(target);
      if (!clickedContainer && !clickedPopover) {
        setIsExpanded(false);
      }
    };

    const timeoutId = setTimeout(() => {
      document.addEventListener('mousedown', handleClickOutside);
    }, 0);

    return () => {
      clearTimeout(timeoutId);
      document.removeEventListener('mousedown', handleClickOutside);
    };
  }, [isExpanded]);

  // Open diff for the selected file.
  const handleFileClick = useCallback(async (filePath: string) => {
    if (!sessionId) return;

    try {
      const diffData = await snapshotAPI.getOperationDiff(sessionId, filePath);
      if ((diffData.originalContent || '') === (diffData.modifiedContent || '')) {
        log.debug('Skipping empty diff', { filePath, sessionId });
        setIsExpanded(false);
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

      setIsExpanded(false);
    } catch (error) {
      log.error('Failed to open diff', { filePath, error });
    }
  }, [sessionId, currentWorkspace?.rootPath]);

  const getOperationIcon = (operationType: 'write' | 'edit' | 'delete') => {
    switch (operationType) {
      case 'write':
        return <FilePlus size={12} className="icon-write" />;
      case 'delete':
        return <Trash2 size={12} className="icon-delete" />;
      default:
        return <FileEdit size={12} className="icon-edit" />;
    }
  };

  // Compute totals.
  const totalStats = useMemo(() => {
    let totalAdditions = 0;
    let totalDeletions = 0;

    fileStats.forEach((stat) => {
      totalAdditions += stat.additions;
      totalDeletions += stat.deletions;
    });

    return { totalAdditions, totalDeletions };
  }, [fileStats]);

  const toggleHint = useMemo(() => {
    if (fileStats.size === 0) return '';
    const head = t('turnFilesSummary.filesCount', {
      count: fileStats.size,
    });
    const deltas: string[] = [];
    if (totalStats.totalAdditions > 0) deltas.push(`+${totalStats.totalAdditions}`);
    if (totalStats.totalDeletions > 0) deltas.push(`-${totalStats.totalDeletions}`);
    const cue = t('turnFilesSummary.expandCue');
    return deltas.length > 0 ? `${head} · ${deltas.join(' ')} · ${cue}` : `${head} · ${cue}`;
  }, [fileStats.size, totalStats.totalAdditions, totalStats.totalDeletions, t]);

  if (!sessionId || loading || files.length === 0) {
    return null;
  }

  const showFileStatsSummary = fileStats.size > 0;

  return (
    <div
      ref={containerRef}
      className={`turn-files-summary ${isExpanded ? 'turn-files-summary--expanded' : ''}`}
    >
      {showFileStatsSummary ? (
        <button
          className="turn-files-summary__button"
          onClick={() => setIsExpanded(prev => !prev)}
          disabled={loadingStats}
          type="button"
          title={toggleHint}
          aria-label={toggleHint}
          aria-expanded={isExpanded}
        >
          <FileCode2 size={12} className="turn-files-summary__icon" />
          <span className="turn-files-summary__count">
            {t('turnFilesSummary.filesCount', { count: fileStats.size })}
          </span>
          {totalStats.totalAdditions > 0 && (
            <span className="turn-files-summary__stats turn-files-summary__stats--add">
              +{totalStats.totalAdditions}
            </span>
          )}
          {totalStats.totalDeletions > 0 && (
            <span className="turn-files-summary__stats turn-files-summary__stats--del">
              -{totalStats.totalDeletions}
            </span>
          )}
          {isExpanded ? (
            <ChevronUp size={12} className="turn-files-summary__arrow" />
          ) : (
            <ChevronDown size={12} className="turn-files-summary__arrow" />
          )}
        </button>
      ) : null}

      {showFileStatsSummary && isExpanded && (
        <div
          ref={popoverRef}
          className="turn-files-summary__popover"
        >
          <div className="turn-files-summary__popover-summary">
            <span className="turn-files-summary__popover-summary-count">
              {t('turnFilesSummary.filesCount', {
                count: fileStats.size,
              })}
            </span>
            {(totalStats.totalAdditions > 0 || totalStats.totalDeletions > 0) && (
              <span className="turn-files-summary__popover-summary-stats">
                {totalStats.totalAdditions > 0 && (
                  <span className="turn-files-summary__stats turn-files-summary__stats--add">
                    +{totalStats.totalAdditions}
                  </span>
                )}
                {totalStats.totalDeletions > 0 && (
                  <span className="turn-files-summary__stats turn-files-summary__stats--del">
                    -{totalStats.totalDeletions}
                  </span>
                )}
              </span>
            )}
          </div>
          <div className="turn-files-summary__list">
            {Array.from(fileStats.values()).map((stat) => (
              <div
                key={stat.filePath}
                className={`turn-files-summary__file-item turn-files-summary__file-item--${stat.operationType} ${
                  stat.error ? 'turn-files-summary__file-item--error' : ''
                }`}
                onClick={() => !stat.error && handleFileClick(stat.filePath)}
                title={stat.error ? stat.error : t('turnFilesSummary.clickToViewDiff')}
              >
                <span className="turn-files-summary__file-icon">
                  {getOperationIcon(stat.operationType)}
                </span>

                <span className="turn-files-summary__file-name">{stat.fileName}</span>

                {stat.error ? (
                  <span className="turn-files-summary__file-error">{stat.error}</span>
                ) : (
                  <span className="turn-files-summary__file-stats">
                    {stat.additions > 0 && (
                      <span className="turn-files-summary__file-stat turn-files-summary__file-stat--add">
                        +{stat.additions}
                      </span>
                    )}
                    {stat.deletions > 0 && (
                      <span className="turn-files-summary__file-stat turn-files-summary__file-stat--del">
                        -{stat.deletions}
                      </span>
                    )}
                  </span>
                )}
              </div>
            ))}
          </div>
        </div>
      )}
    </div>
  );
};
