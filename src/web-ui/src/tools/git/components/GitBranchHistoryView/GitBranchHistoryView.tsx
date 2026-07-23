/**
 * Git branch commit history view.
 * Shows a branch's commits and supports cherry-pick when applicable.
 */

import React, { useState, useEffect, useCallback, useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { Copy, RefreshCw, ChevronDown, ChevronUp, GitBranch, Square, CheckSquare } from 'lucide-react';
import { Search, IconButton, Select, Button } from '@/component-library';
import { gitAPI } from '@/infrastructure/api';
import { useNotification } from '@/shared/notification-system';
import type { GitGraphNode } from '@/infrastructure/api/service-api/GitAPI';
import { i18nService } from '@/infrastructure/i18n';
import { createLogger } from '@/shared/utils/logger';
import './GitBranchHistoryView.scss';

const log = createLogger('GitBranchHistoryView');

export interface GitBranchHistoryViewProps {
  /** Repository path */
  repositoryPath: string;
  /** Branch name */
  branchName: string;
  /** Current branch name (used to determine if cherry-pick is allowed) */
  currentBranch?: string;
  /** Maximum number of commits to load */
  maxCount?: number;
  /** Class name */
  className?: string;
  /** Cherry-pick success callback */
  onCherryPickSuccess?: (commitHashes: string[]) => void;
}

interface CommitInfo {
  hash: string;
  message: string;
  author: string;
  authorEmail: string;
  date: string;
  timestamp: number;
  refs: { name: string; refType: string; isCurrent: boolean }[];
}

function formatRelativeTime(timestamp: number): string {
  const now = Date.now() / 1000;
  const diff = now - timestamp;
  
  if (diff < 60) return i18nService.t('panels/git:relativeTime.justNow');
  if (diff < 3600) {
    return i18nService.t('panels/git:relativeTime.minutesAgo', { count: Math.floor(diff / 60) });
  }
  if (diff < 86400) {
    return i18nService.t('panels/git:relativeTime.hoursAgo', { count: Math.floor(diff / 3600) });
  }
  if (diff < 604800) {
    return i18nService.t('panels/git:relativeTime.daysAgo', { count: Math.floor(diff / 86400) });
  }
  if (diff < 2592000) {
    return i18nService.t('panels/git:relativeTime.weeksAgo', { count: Math.floor(diff / 604800) });
  }
  if (diff < 31536000) {
    return i18nService.t('panels/git:relativeTime.monthsAgo', { count: Math.floor(diff / 2592000) });
  }
  return i18nService.t('panels/git:relativeTime.yearsAgo', { count: Math.floor(diff / 31536000) });
}

function convertToCommitInfo(node: GitGraphNode): CommitInfo {
  return {
    hash: node.hash,
    message: node.message,
    author: node.authorName,
    authorEmail: node.authorEmail,
    date: new Date(node.timestamp * 1000).toISOString(),
    timestamp: node.timestamp,
    refs: node.refs
  };
}

export const GitBranchHistoryView: React.FC<GitBranchHistoryViewProps> = ({
  repositoryPath,
  branchName,
  currentBranch,
  maxCount = 100,
  className = '',
  onCherryPickSuccess
}) => {
  const { t } = useTranslation('panels/git');
  const [commits, setCommits] = useState<CommitInfo[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [expandedCommits, setExpandedCommits] = useState<Set<string>>(new Set());
  const [searchQuery, setSearchQuery] = useState('');
  const [authorFilter, setAuthorFilter] = useState<string>('');
  const [copiedHash, setCopiedHash] = useState<string | null>(null);
  

  const [selectedCommits, setSelectedCommits] = useState<Set<string>>(new Set());
  const [isCherryPicking, setIsCherryPicking] = useState(false);
  

  const notification = useNotification();
  

  const canCherryPick = currentBranch && branchName !== currentBranch;


  const loadCommits = useCallback(async () => {
    setLoading(true);
    setError(null);
    
    try {

      const graphData = await gitAPI.getGraph(repositoryPath, maxCount, branchName);
      
      if (!graphData || !graphData.nodes) {
        throw new Error(i18nService.t('panels/git:branchHistory.loadFailed'));
      }
      

      const parsedCommits = graphData.nodes.slice(0, maxCount).map(convertToCommitInfo);
      setCommits(parsedCommits);
    } catch (err) {
      log.error('Failed to load commit history', { repositoryPath, branchName, error: err });
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setLoading(false);
    }
  }, [repositoryPath, branchName, maxCount]);


  useEffect(() => {
    loadCommits();
  }, [loadCommits]);


  const toggleCommitExpand = useCallback((hash: string) => {
    setExpandedCommits(prev => {
      const newSet = new Set(prev);
      if (newSet.has(hash)) {
        newSet.delete(hash);
      } else {
        newSet.add(hash);
      }
      return newSet;
    });
  }, []);


  const handleCopyHash = useCallback(async (hash: string, e: React.MouseEvent) => {
    e.stopPropagation();
    try {
      await navigator.clipboard.writeText(hash);
      setCopiedHash(hash);
      setTimeout(() => setCopiedHash(null), 2000);
    } catch (err) {
      log.error('Failed to copy hash', { hash, error: err });
    }
  }, []);


  const uniqueAuthors = useMemo(() => {
    const authorsSet = new Set<string>();
    commits.forEach(commit => {
      if (commit.author) {
        authorsSet.add(commit.author);
      }
    });
    return Array.from(authorsSet).sort((a, b) => a.localeCompare(b));
  }, [commits]);


  const filteredCommits = useMemo(() => {
    let result = commits;
    

    if (authorFilter) {
      result = result.filter(commit => commit.author === authorFilter);
    }
    

    if (searchQuery.trim()) {
      const query = searchQuery.toLowerCase();
      result = result.filter(commit => 
        commit.message.toLowerCase().includes(query) ||
        commit.author.toLowerCase().includes(query) ||
        commit.hash.toLowerCase().includes(query)
      );
    }
    
    return result;
  }, [commits, searchQuery, authorFilter]);


  const toggleCommitSelection = useCallback((hash: string, e: React.MouseEvent) => {
    e.stopPropagation();
    setSelectedCommits(prev => {
      const newSet = new Set(prev);
      if (newSet.has(hash)) {
        newSet.delete(hash);
      } else {
        newSet.add(hash);
      }
      return newSet;
    });
  }, []);


  const toggleSelectAll = useCallback(() => {
    if (selectedCommits.size === filteredCommits.length) {
      setSelectedCommits(new Set());
    } else {
      setSelectedCommits(new Set(filteredCommits.map(c => c.hash)));
    }
  }, [filteredCommits, selectedCommits.size]);


  const handleCherryPick = useCallback(async () => {
    if (selectedCommits.size === 0) return;
    
    setIsCherryPicking(true);
    const hashesToPick = Array.from(selectedCommits);

    const sortedHashes = hashesToPick.sort((a, b) => {
      const commitA = commits.find(c => c.hash === a);
      const commitB = commits.find(c => c.hash === b);
      return (commitA?.timestamp || 0) - (commitB?.timestamp || 0);
    });
    
    const successHashes: string[] = [];
    const failedHashes: { hash: string; error: string }[] = [];
    
    for (const hash of sortedHashes) {
      try {
        const result = await gitAPI.cherryPick(repositoryPath, hash, false);
        
        if (result.success) {
          successHashes.push(hash);
        } else {
          failedHashes.push({ hash, error: result.error || t('branchHistory.cherryPickFailed') });

          break;
        }
      } catch (err) {
        const errorMsg = err instanceof Error ? err.message : t('branchHistory.cherryPickOperationFailed');
        failedHashes.push({ hash, error: errorMsg });
        break;
      }
    }
    
    setIsCherryPicking(false);
    

    if (failedHashes.length === 0 && successHashes.length > 0) {
      notification.success(t('branchHistory.notifications.cherryPickSuccess', {
        count: successHashes.length,
        branch: currentBranch
      }));
      setSelectedCommits(new Set());
      onCherryPickSuccess?.(successHashes);
    } else if (failedHashes.length > 0) {
      const failedHash = failedHashes[0].hash.substring(0, 7);
      const errorMsg = failedHashes[0].error;
      if (successHashes.length > 0) {
        notification.warning(t('branchHistory.notifications.cherryPickPartial', {
          count: successHashes.length,
          hash: failedHash,
          error: errorMsg
        }));
      } else {
        notification.error(t('branchHistory.notifications.cherryPickFailedWithError', {
          hash: failedHash,
          error: errorMsg
        }));
      }

      setSelectedCommits(new Set(failedHashes.map(f => f.hash)));
    }
  }, [selectedCommits, commits, repositoryPath, currentBranch, notification, onCherryPickSuccess, t]);

  if (loading) {
    return (
      <div className={`git-branch-history-view git-branch-history-view--loading ${className}`}>
        <div className="git-branch-history-view__loading">
          <div className="git-branch-history-view__spinner" />
          <p>{t('branchHistory.loading', { branch: branchName })}</p>
        </div>
      </div>
    );
  }

  if (error) {
    return (
      <div className={`git-branch-history-view git-branch-history-view--error ${className}`}>
        <div className="git-branch-history-view__error">
          <p>{t('branchHistory.loadFailedWithMessage', { error })}</p>
          <Button variant="secondary" size="small" onClick={loadCommits}>
            {t('branchHistory.retry')}
          </Button>
        </div>
      </div>
    );
  }

  return (
    <div className={`git-branch-history-view ${className}`}>
      <div className="git-branch-history-view__header">
        <div className="git-branch-history-view__header-left">
          {canCherryPick && (
            <div 
              className="git-branch-history-view__select-all"
              onClick={toggleSelectAll}
              title={selectedCommits.size === filteredCommits.length
                ? t('branchHistory.selection.deselectAll')
                : t('branchHistory.selection.selectAll')
              }
            >
              {selectedCommits.size === filteredCommits.length && filteredCommits.length > 0 ? (
                <CheckSquare size={16} className="git-branch-history-view__checkbox--checked" />
              ) : selectedCommits.size > 0 ? (
                <div className="git-branch-history-view__checkbox--partial">
                  <Square size={16} />
                  <div className="git-branch-history-view__checkbox-partial-mark" />
                </div>
              ) : (
                <Square size={16} className="git-branch-history-view__checkbox" />
              )}
            </div>
          )}
          
          <Search
            value={searchQuery}
            onChange={setSearchQuery}
            placeholder={t('search.commits')}
            size="small"
            clearable
            enterToSearch={false}
            className="git-branch-history-view__search"
          />
          
          {uniqueAuthors.length > 0 && (
            <Select
              options={[
                { label: t('branchHistory.allAuthors'), value: '' },
                ...uniqueAuthors.map(author => ({ label: author, value: author }))
              ]}
              value={authorFilter}
              onChange={(val) => setAuthorFilter(val as string)}
              placeholder={t('branchHistory.authorPlaceholder')}
              size="small"
              clearable={!!authorFilter}
              className="git-branch-history-view__author-select"
            />
          )}
        </div>
        
        <div className="git-branch-history-view__header-right">
          {canCherryPick && (
            <Button
              size="small"
              variant="primary"
              onClick={handleCherryPick}
              disabled={isCherryPicking || selectedCommits.size === 0}
              className="git-branch-history-view__cherry-pick-btn"
            >
              {isCherryPicking ? (
                <>
                  <div className="git-branch-history-view__spinner git-branch-history-view__spinner--small" />
                  {t('branchHistory.cherryPickRunning')}
                </>
              ) : (
                <>
                  <GitBranch size={14} />
                  {t('branchHistory.cherryPick')}
                  {selectedCommits.size > 0 ? ` (${selectedCommits.size})` : ''}
                </>
              )}
            </Button>
          )}
          
          <IconButton
            size="xs"
            variant="ghost"
            onClick={loadCommits}
            title={t('branchHistory.refresh')}
          >
            <RefreshCw size={14} />
          </IconButton>
        </div>
      </div>

      <div className="git-branch-history-view__content">
        {filteredCommits.length === 0 ? (
          <div className="git-branch-history-view__empty">
            <p>{searchQuery ? t('empty.noMatchingCommits') : t('empty.noCommits')}</p>
          </div>
        ) : (
          <div className="git-branch-history-view__commits">
            {filteredCommits.map((commit, index) => {
              const isExpanded = expandedCommits.has(commit.hash);
              const isSelected = selectedCommits.has(commit.hash);
              
              return (
                <div 
                  key={commit.hash}
                  className={`git-branch-history-view__commit ${isExpanded ? 'git-branch-history-view__commit--expanded' : ''} ${isSelected ? 'git-branch-history-view__commit--selected' : ''}`}
                >
                  <div 
                    className="git-branch-history-view__commit-main"
                    onClick={() => toggleCommitExpand(commit.hash)}
                  >
                    {canCherryPick && (
                      <div 
                        className="git-branch-history-view__commit-checkbox"
                        onClick={(e) => toggleCommitSelection(commit.hash, e)}
                      >
                        {isSelected ? (
                          <CheckSquare size={16} className="git-branch-history-view__checkbox--checked" />
                        ) : (
                          <Square size={16} className="git-branch-history-view__checkbox" />
                        )}
                      </div>
                    )}
                    
                    <div className="git-branch-history-view__timeline">
                      <div className="git-branch-history-view__timeline-node" title={commit.author}>
                        {commit.author.charAt(0).toUpperCase()}
                      </div>
                      {index < filteredCommits.length - 1 && (
                        <div className="git-branch-history-view__timeline-line" />
                      )}
                    </div>
                    
                    <div className="git-branch-history-view__commit-info">
                      <div className="git-branch-history-view__commit-message">
                        {commit.message}
                      </div>
                      <div className="git-branch-history-view__commit-meta">
                        <span className="git-branch-history-view__commit-author">
                          {commit.author}
                        </span>
                        <span className="git-branch-history-view__commit-time">
                          {formatRelativeTime(commit.timestamp)}
                        </span>
                        <span 
                          className={`git-branch-history-view__commit-hash ${copiedHash === commit.hash ? 'git-branch-history-view__commit-hash--copied' : ''}`}
                          onClick={(e) => handleCopyHash(commit.hash, e)}
                          title={copiedHash === commit.hash ? t('branchHistory.copied') : t('branchHistory.copy')}
                        >
                          {commit.hash.substring(0, 7)}
                          <Copy size={9} className="git-branch-history-view__copy-icon" />
                        </span>
                      </div>
                    </div>
                    
                    <IconButton 
                      className="git-branch-history-view__expand-btn"
                      size="xs"
                      variant="ghost"
                    >
                      {isExpanded ? <ChevronUp size={14} /> : <ChevronDown size={14} />}
                    </IconButton>
                  </div>
                  
                  {isExpanded && (
                    <div className="git-branch-history-view__commit-details">
                      <div className="git-branch-history-view__detail-row">
                        <span className="git-branch-history-view__detail-label">
                          {t('branchHistory.details.fullHash')}
                        </span>
                        <span className="git-branch-history-view__detail-value git-branch-history-view__detail-value--mono">
                          {commit.hash}
                        </span>
                      </div>
                      <div className="git-branch-history-view__detail-row">
                        <span className="git-branch-history-view__detail-label">
                          {t('branchHistory.details.commitTime')}
                        </span>
                        <span className="git-branch-history-view__detail-value">
                          {i18nService.formatDate(new Date(commit.date))}
                        </span>
                      </div>
                    </div>
                  )}
                </div>
              );
            })}
          </div>
        )}
      </div>
    </div>
  );
};

export default GitBranchHistoryView;

