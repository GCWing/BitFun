/**
 * Session file change badge.
 * Shows compact file change stats in FlowChatHeader.
 */

import React, { useState, useCallback, useMemo, useEffect, useRef } from 'react';
import { FileEdit, FilePlus, Trash2, ChevronDown, ChevronUp } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { useSnapshotState } from '../../../tools/snapshot_system/hooks/useSnapshotState';
import { createDiffEditorTab } from '../../../shared/utils/tabUtils';
import { snapshotAPI } from '../../../infrastructure/api';
import { useWorkspaceContext } from '../../../infrastructure/contexts/WorkspaceContext';
import { diffService } from '../../../tools/editor/services';
import { createLogger } from '@/shared/utils/logger';
import './SessionFilesBadge.scss';

const log = createLogger('SessionFilesBadge');

export interface SessionFilesBadgeProps {
  /** Session ID. */
  sessionId?: string;
  /** Disabled state. */
  disabled?: boolean;
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
 * Session file change badge.
 */
export const SessionFilesBadge: React.FC<SessionFilesBadgeProps> = ({
  sessionId,
  disabled = false,
}) => {
  const { t } = useTranslation('flow-chat');
  const { files } = useSnapshotState(sessionId);
  const { currentWorkspace } = useWorkspaceContext();
  const [isExpanded, setIsExpanded] = useState(false);
  const [fileStats, setFileStats] = useState<Map<string, FileStats>>(new Map());
  const [loadingStats, setLoadingStats] = useState(false);

  const statsCacheRef = useRef<StatsCache>({});
  const loadingFilesRef = useRef<Set<string>>(new Set());
  const previousSessionIdRef = useRef<string | undefined>(undefined);
  const CACHE_TTL = 10000;

  const badgeRef = useRef<HTMLDivElement>(null);
  const popoverRef = useRef<HTMLDivElement>(null);

  // Reset cached state when the session changes.
  useEffect(() => {
    if (previousSessionIdRef.current !== sessionId) {
      previousSessionIdRef.current = sessionId;
      statsCacheRef.current = {};
      loadingFilesRef.current.clear();
      setFileStats(new Map());
      setIsExpanded(false);
    }
  }, [sessionId, t]);

  // Close the popover when clicking outside.
  useEffect(() => {
    if (!isExpanded) return;

    const handleClickOutside = (event: MouseEvent) => {
      const target = event.target as Node;
      if (
        badgeRef.current &&
        popoverRef.current &&
        !badgeRef.current.contains(target) &&
        !popoverRef.current.contains(target)
      ) {
        setIsExpanded(false);
      }
    };

    // Delay binding to avoid immediate trigger.
    const timeoutId = setTimeout(() => {
      document.addEventListener('mousedown', handleClickOutside);
    }, 0);

    return () => {
      clearTimeout(timeoutId);
      document.removeEventListener('mousedown', handleClickOutside);
    };
  }, [isExpanded]);

  /**
   * Fetch per-file diff stats with caching.
   */
  const loadFileStats = useCallback(async (filesToLoad: typeof files) => {
    if (!sessionId || filesToLoad.length === 0) {
      return;
    }

    const now = Date.now();

    const newFilesToLoad = filesToLoad.filter(file => {
      if (loadingFilesRef.current.has(file.filePath)) {
        return false;
      }
      const cached = statsCacheRef.current[file.filePath];
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

          // Keep only files with changes or errors (filter +0/-0).
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

  // Open diff for the selected file.
  const handleFileClick = useCallback(async (filePath: string) => {
    if (!sessionId) return;

    try {
      const diffData = await snapshotAPI.getOperationDiff(sessionId, filePath);
      if ((diffData.originalContent || '') === (diffData.modifiedContent || '')) {
        log.debug('Skipping empty session diff', { filePath, sessionId });
        setIsExpanded(false);
        return;
      }
      const fileName = filePath.split(/[/\\]/).pop() || filePath;

      // Expand the right panel.
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

  // Trigger CodeReview agent for the current session's changes.
  const handleReviewClick = useCallback(async (e: React.MouseEvent) => {
    e.stopPropagation();
    if (!sessionId || fileStats.size === 0) return;

    const filePaths = Array.from(fileStats.keys());
    const fileList = filePaths.map(p => `- ${p}`).join('\n');

    const displayMessage = t('sessionFilesBadge.review.displayMessage', { files: fileList });
    const reviewMessage = t('sessionFilesBadge.review.prompt', { files: fileList });

    try {
      const { FlowChatManager } = await import('../../services/FlowChatManager');
      const flowChatManager = FlowChatManager.getInstance();

      await flowChatManager.sendMessage(
        reviewMessage,
        sessionId,
        displayMessage,
        'CodeReview'
      );

      setIsExpanded(false);
    } catch (error) {
      log.error('Failed to send review request', { sessionId, fileCount: fileStats.size, error });
    }
  }, [fileStats, sessionId, t]);

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

  // Hide when there is no session, no changes, or disabled.
  if (!sessionId || fileStats.size === 0 || disabled) {
    return null;
  }

  return (
    <div
      ref={badgeRef}
      className={`session-files-badge ${isExpanded ? 'session-files-badge--expanded' : ''}`}
    >
      <button
        className="session-files-badge__button"
        onClick={() => setIsExpanded(!isExpanded)}
        disabled={loadingStats}
        type="button"
      >
        <span className="session-files-badge__count">
          {fileStats.size} {t('sessionFilesBadge.files')}
        </span>
        {totalStats.totalAdditions > 0 && (
          <span className="session-files-badge__stats session-files-badge__stats--add">
            +{totalStats.totalAdditions}
          </span>
        )}
        {totalStats.totalDeletions > 0 && (
          <span className="session-files-badge__stats session-files-badge__stats--del">
            -{totalStats.totalDeletions}
          </span>
        )}
        {isExpanded ? (
          <ChevronUp size={12} className="session-files-badge__arrow" />
        ) : (
          <ChevronDown size={12} className="session-files-badge__arrow" />
        )}
      </button>

      <button
        className="session-files-badge__review-btn"
        onClick={handleReviewClick}
        disabled={loadingStats}
        title={t('sessionFilesBadge.reviewAll')}
        type="button"
      >
        <span className="session-files-badge__review-text">{t('sessionFilesBadge.reviewLabel')}</span>
      </button>

      {isExpanded && (
        <div
          ref={popoverRef}
          className="session-files-badge__popover"
        >
          <div className="session-files-badge__list">
            {Array.from(fileStats.values()).map((stat) => (
              <div
                key={stat.filePath}
                className={`session-files-badge__file-item session-files-badge__file-item--${stat.operationType} ${
                  stat.error ? 'session-files-badge__file-item--error' : ''
                }`}
                onClick={() => !stat.error && handleFileClick(stat.filePath)}
                title={stat.error ? stat.error : t('sessionFilesBadge.clickToViewDiff')}
              >
                <span className="session-files-badge__file-icon">
                  {getOperationIcon(stat.operationType)}
                </span>

                <span className="session-files-badge__file-name">{stat.fileName}</span>

                {stat.error ? (
                  <span className="session-files-badge__file-error">{stat.error}</span>
                ) : (
                  <span className="session-files-badge__file-stats">
                    {stat.additions > 0 && (
                      <span className="session-files-badge__file-stat session-files-badge__file-stat--add">
                        +{stat.additions}
                      </span>
                    )}
                    {stat.deletions > 0 && (
                      <span className="session-files-badge__file-stat session-files-badge__file-stat--del">
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

SessionFilesBadge.displayName = 'SessionFilesBadge';
