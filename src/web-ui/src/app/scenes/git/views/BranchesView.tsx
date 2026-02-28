/**
 * BranchesView — Left: branch list (switch/create/delete). Right: commit history for selected branch.
 */

import React, { useCallback, useState, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import {
  GitBranch,
  Plus,
  Trash2,
  GitCommit,
  Copy,
  RotateCcw,
  FileText,
  ChevronDown,
  ChevronRight,
} from 'lucide-react';
import { Button, IconButton, Tooltip, Search as SearchComponent } from '@/component-library';
import { gitService } from '@/tools/git/services';
import { useGitOperations } from '@/tools/git/hooks';
import { useNotification } from '@/shared/notification-system';
import { CreateBranchDialog } from '@/tools/git/components/CreateBranchDialog';
import type { GitBranch as GitBranchType } from '@/infrastructure/api/service-api/GitAPI';
import type { GitCommit as GitCommitType } from '@/infrastructure/api/service-api/GitAPI';
import './BranchesView.scss';

interface BranchesViewProps {
  workspacePath?: string;
}

const BranchesView: React.FC<BranchesViewProps> = ({ workspacePath }) => {
  const { t } = useTranslation('panels/git');
  const notification = useNotification();

  const [branches, setBranches] = useState<GitBranchType[]>([]);
  const [branchLoading, setBranchLoading] = useState(false);
  const [branchSearchQuery, setBranchSearchQuery] = useState('');
  const [selectedBranchName, setSelectedBranchName] = useState<string | null>(null);
  const [showCreateDialog, setShowCreateDialog] = useState(false);
  const [baseBranch, setBaseBranch] = useState('');

  const [commits, setCommits] = useState<GitCommitType[]>([]);
  const [commitLoading, setCommitLoading] = useState(false);
  const [commitSearchQuery, setCommitSearchQuery] = useState('');
  const [expandedCommits, setExpandedCommits] = useState<Set<string>>(new Set());
  const [isResetting, setIsResetting] = useState(false);

  const { isOperating, checkoutBranch, createBranch, deleteBranch } = useGitOperations({
    repositoryPath: workspacePath ?? '',
    autoRefresh: false,
  });

  const loadBranches = useCallback(async () => {
    if (!workspacePath) return;
    setBranchLoading(true);
    try {
      const result = await gitService.getBranches(workspacePath, true);
      const list = Array.isArray(result) ? result : [];
      setBranches(list);
      if (list.length > 0 && !selectedBranchName) {
        const current = list.find(b => b.current);
        setSelectedBranchName(current?.name ?? list[0]?.name ?? null);
      }
    } catch {
      setBranches([]);
    } finally {
      setBranchLoading(false);
    }
  }, [workspacePath]);

  const loadCommits = useCallback(
    async (branchRef: string | null) => {
      if (!workspacePath || !branchRef) {
        setCommits([]);
        return;
      }
      setCommitLoading(true);
      try {
        const result = await gitService.getCommits(workspacePath, { maxCount: 50 });
        const list = Array.isArray(result) ? result : [];
        setCommits([...list].reverse());
      } catch {
        setCommits([]);
      } finally {
        setCommitLoading(false);
      }
    },
    [workspacePath]
  );

  useEffect(() => {
    loadBranches();
  }, [loadBranches]);

  useEffect(() => {
    loadCommits(selectedBranchName);
  }, [selectedBranchName, loadCommits]);

  const filteredBranches = branchSearchQuery.trim()
    ? branches.filter(b => (b.name ?? '').toLowerCase().includes(branchSearchQuery.toLowerCase()))
    : branches;

  const filteredCommits = commitSearchQuery.trim()
    ? commits.filter(
        c =>
          (c.summary ?? (c as any).message ?? '').toLowerCase().includes(commitSearchQuery.toLowerCase()) ||
          ((c as any).author?.name ?? (c as any).author ?? '').toLowerCase().includes(commitSearchQuery.toLowerCase()) ||
          (c.hash ?? '').toLowerCase().includes(commitSearchQuery.toLowerCase())
      )
    : commits;

  const handleSelectBranch = useCallback((name: string) => {
    setSelectedBranchName(name);
  }, []);

  const handleSwitchBranch = useCallback(
    async (name: string) => {
      const result = await checkoutBranch(name);
      if (result.success) {
        notification.success(t('quickSwitch.notifications.switchSuccess', { branch: name }));
        loadBranches();
        setSelectedBranchName(name);
      } else notification.error(result.error || t('quickSwitch.errors.switchFailed'));
    },
    [checkoutBranch, notification, t, loadBranches]
  );

  const handleCreateFrom = useCallback((base: string) => {
    setBaseBranch(base);
    setShowCreateDialog(true);
  }, []);

  const handleCreateConfirm = useCallback(
    async (newName: string) => {
      const result = await createBranch(newName.trim(), baseBranch);
      if (result.success) {
        setShowCreateDialog(false);
        setBaseBranch('');
        loadBranches();
      }
    },
    [createBranch, baseBranch, loadBranches]
  );

  const handleDeleteBranch = useCallback(
    async (name: string, isCurrent: boolean) => {
      if (isCurrent) {
        notification.warning(t('notifications.cannotDeleteCurrentBranch'));
        return;
      }
      if (!confirm(t('confirm.deleteBranch', { branch: name }))) return;
      const result = await deleteBranch(name, false);
      if (result.success) {
        loadBranches();
        if (selectedBranchName === name) setSelectedBranchName(branches.find(b => b.name !== name)?.name ?? null);
      } else notification.error(result.error || 'Delete failed');
    },
    [deleteBranch, notification, t, loadBranches, selectedBranchName, branches]
  );

  const toggleCommitExpand = useCallback((hash: string) => {
    setExpandedCommits(prev => {
      const next = new Set(prev);
      next.has(hash) ? next.delete(hash) : next.add(hash);
      return next;
    });
  }, []);

  const handleCopyHash = useCallback(
    async (hash: string) => {
      try {
        await navigator.clipboard.writeText(hash);
        notification.success(t('branchHistory.copied') || 'Copied');
      } catch {
        notification.error('Copy failed');
      }
    },
    [notification, t]
  );

  const handleResetToCommit = useCallback(
    async (hash: string) => {
      if (!workspacePath) return;
      if (!confirm(t('confirm.resetToCommit', { hash: hash.substring(0, 7) }))) return;
      setIsResetting(true);
      try {
        const result = await gitService.resetToCommit(workspacePath, hash, 'mixed');
        if (result.success) {
          notification.success(t('notifications.resetSuccess', { hash: hash.substring(0, 7) }));
          loadBranches();
          loadCommits(selectedBranchName);
        } else notification.error(result.error || 'Reset failed');
      } finally {
        setIsResetting(false);
      }
    },
    [workspacePath, notification, t, selectedBranchName, loadCommits]
  );

  if (!workspacePath) {
    return (
      <div className="bitfun-git-scene-branches">
        <div className="bitfun-git-scene-branches__placeholder">
          <GitBranch size={48} />
          <p>{t('tabs.branches')}</p>
          <p className="bitfun-git-scene-branches__hint">Open a workspace to see branches.</p>
        </div>
      </div>
    );
  }

  return (
    <div className="bitfun-git-scene-branches">
      <div className="bitfun-git-scene-branches__left">
        <div className="bitfun-git-scene-branches__toolbar">
          <div className="bitfun-git-scene-branches__toolbar-search">
            <SearchComponent
              placeholder={t('search.branches')}
              value={branchSearchQuery}
              onChange={setBranchSearchQuery}
              onClear={() => setBranchSearchQuery('')}
            />
          </div>
          <div className="bitfun-git-scene-branches__toolbar-actions">
            <Button
              size="small"
              variant="primary"
              onClick={() => handleCreateFrom(branches.find(b => b.current)?.name ?? selectedBranchName ?? '')}
              title={t('dialog.createNewBranch.title')}
              className="bitfun-git-scene-branches__create-btn"
            >
              <Plus size={14} />
              <span>{t('dialog.createNewBranch.confirm')}</span>
            </Button>
          </div>
        </div>
        <div className="bitfun-git-scene-branches__list">
          {branchLoading ? (
            <div className="bitfun-git-scene-branches__empty">{t('common.loading')}</div>
          ) : filteredBranches.length === 0 ? (
            <div className="bitfun-git-scene-branches__empty">
              {branchSearchQuery ? t('empty.noMatchingBranches') : t('empty.noBranches')}
            </div>
          ) : (
            filteredBranches.map((branch, idx) => (
              <div
                key={branch.name ?? idx}
                className={`bitfun-git-scene-branches__row ${branch.current ? 'bitfun-git-scene-branches__row--current' : ''} ${selectedBranchName === branch.name ? 'bitfun-git-scene-branches__row--selected' : ''}`}
                onClick={() => handleSelectBranch(branch.name)}
              >
                <div className="bitfun-git-scene-branches__info">
                  <GitBranch size={14} />
                  <span className="bitfun-git-scene-branches__name">{branch.name}</span>
                  {branch.current && <span className="bitfun-git-scene-branches__current-badge">{t('branch.current')}</span>}
                </div>
                <div className="bitfun-git-scene-branches__actions" onClick={e => e.stopPropagation()}>
                  {!branch.current && (
                    <Tooltip content={t('actions.switchBranch')}>
                      <IconButton size="xs" variant="ghost" onClick={() => handleSwitchBranch(branch.name)} disabled={isOperating}>
                        <GitCommit size={14} />
                      </IconButton>
                    </Tooltip>
                  )}
                  <Tooltip content={t('actions.createBranchFrom')}>
                    <IconButton size="xs" variant="ghost" onClick={() => handleCreateFrom(branch.name)} disabled={isOperating}>
                      <Plus size={14} />
                    </IconButton>
                  </Tooltip>
                  {!branch.current && (
                    <Tooltip content={t('actions.deleteBranch')}>
                      <IconButton size="xs" variant="ghost" onClick={() => handleDeleteBranch(branch.name, !!branch.current)} disabled={isOperating}>
                        <Trash2 size={14} />
                      </IconButton>
                    </Tooltip>
                  )}
                </div>
              </div>
            ))
          )}
        </div>
      </div>

      <div className="bitfun-git-scene-branches__right">
        <div className="bitfun-git-scene-branches__history-toolbar">
          <span className="bitfun-git-scene-branches__history-title">
            {selectedBranchName ? t('tabs.branchCommitHistory', { branch: selectedBranchName }) : t('tabs.commits')}
          </span>
          <SearchComponent
            placeholder={t('search.commits')}
            value={commitSearchQuery}
            onChange={setCommitSearchQuery}
            onClear={() => setCommitSearchQuery('')}
          />
        </div>
        <div className="bitfun-git-scene-branches__history-list">
          {!selectedBranchName ? (
            <div className="bitfun-git-scene-branches__empty">{t('empty.noCommits')}</div>
          ) : commitLoading ? (
            <div className="bitfun-git-scene-branches__empty">{t('common.loading')}</div>
          ) : filteredCommits.length === 0 ? (
            <div className="bitfun-git-scene-branches__empty">
              {commitSearchQuery ? t('empty.noMatchingCommits') : t('empty.noCommits')}
            </div>
          ) : (
            filteredCommits.map((commit, idx) => {
              const isExpanded = expandedCommits.has(commit.hash);
              const msg = (commit as any).message ?? commit.summary ?? '';
              const summary = msg.split('\n')[0];
              const body = msg.split('\n').slice(1).join('\n').trim();
              const author = (commit as any).author?.name ?? (commit as any).author ?? t('common.unknown');
              const files = (commit as any).files;
              return (
                <div
                  key={commit.hash ?? idx}
                  className={`bitfun-git-scene-branches__commit ${isExpanded ? 'bitfun-git-scene-branches__commit--expanded' : ''}`}
                >
                  <div className="bitfun-git-scene-branches__commit-header" onClick={() => toggleCommitExpand(commit.hash)}>
                    <button type="button" className="bitfun-git-scene-branches__expand">
                      {isExpanded ? <ChevronDown size={12} /> : <ChevronRight size={12} />}
                    </button>
                    <div className="bitfun-git-scene-branches__commit-info">
                      <div className="bitfun-git-scene-branches__commit-message">{summary}</div>
                      <div className="bitfun-git-scene-branches__commit-meta">
                        {author} · {commit.hash?.substring(0, 7)}
                      </div>
                    </div>
                    <div className="bitfun-git-scene-branches__commit-actions" onClick={e => e.stopPropagation()}>
                      <Tooltip content={t('actions.copyCommitHash')}>
                        <IconButton size="xs" variant="ghost" onClick={() => handleCopyHash(commit.hash)}>
                          <Copy size={14} />
                        </IconButton>
                      </Tooltip>
                      <Tooltip content={t('actions.resetToCommit')}>
                        <IconButton size="xs" variant="ghost" onClick={() => handleResetToCommit(commit.hash)} disabled={isResetting}>
                          <RotateCcw size={14} />
                        </IconButton>
                      </Tooltip>
                    </div>
                  </div>
                  {isExpanded && (
                    <div className="bitfun-git-scene-branches__commit-detail">
                      {body && <pre className="bitfun-git-scene-branches__commit-body">{body}</pre>}
                      {files && files.length > 0 && (
                        <div className="bitfun-git-scene-branches__files">
                          <span>
                            <FileText size={12} /> {t('commit.changedFiles', { count: files.length })}
                          </span>
                          <ul>
                            {(files as { path?: string }[]).map((f, i) => (
                              <li key={i}>{f.path ?? f}</li>
                            ))}
                          </ul>
                        </div>
                      )}
                    </div>
                  )}
                </div>
              );
            })
          )}
        </div>
      </div>

      <CreateBranchDialog
        isOpen={showCreateDialog}
        baseBranch={baseBranch}
        onConfirm={handleCreateConfirm}
        onCancel={() => {
          setShowCreateDialog(false);
          setBaseBranch('');
        }}
        isCreating={isOperating}
        existingBranches={branches.map(b => b.name).filter((n): n is string => Boolean(n))}
      />
    </div>
  );
};

export default BranchesView;
