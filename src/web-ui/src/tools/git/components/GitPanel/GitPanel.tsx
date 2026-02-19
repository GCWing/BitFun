/**
 * Git side panel.
 * Shows repository status and provides common Git actions.
 */

import React, { useState, useCallback, useEffect, useRef, useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { 
  GitBranch, 
  Plus, 
  History,
  AlertCircle,
  CheckCircle,
  RefreshCw,
  GitCommit,
  Trash2,
  RotateCcw,
  Copy,
  Eye,
  ChevronDown,
  ChevronRight,
  Search,
  X,
  FileText,
  File,
  Check,
  Circle,
  Minus,
  Sparkles,
  ArrowUp,
  ArrowDown,
  ScanSearch
} from 'lucide-react';
import { Button, Empty, Tooltip, InputDialog, CubeLoading, IconButton, Tabs, TabPane, Search as SearchComponent, Textarea } from '@/component-library';
import { PanelHeader } from '@/app/components/panels/base';
import { useGitState, useGitOperations, useGitAgent } from '../../hooks';
import { GitPanelProps } from '../../types';
import { CreateBranchDialog } from '../CreateBranchDialog';
import { gitService } from '../../services';
import { createTab, createDiffEditorTab, createCodeEditorTab } from '@/shared/utils/tabUtils';
import { globalEventBus } from '@/infrastructure/event-bus';
import { useNotification } from '@/shared/notification-system';
import { createLogger } from '@/shared/utils/logger';
import './GitPanel.scss';

const log = createLogger('GitPanel');

/** Splits a file path into a file name and a directory prefix. */
const getFileNameAndDir = (filePath: string): { fileName: string; dirPath: string } => {
  const normalized = filePath.replace(/\\/g, '/');
  const lastSlash = normalized.lastIndexOf('/');
  if (lastSlash === -1) {
    return { fileName: filePath, dirPath: '' };
  }
  return {
    fileName: normalized.substring(lastSlash + 1),
    dirPath: normalized.substring(0, lastSlash + 1)
  };
};

const GitPanel: React.FC<GitPanelProps> = ({
  workspacePath,
  isActive = false,
  onActivate,
  className = ''
}) => {
  const { t } = useTranslation('panels/git');
  const [forceReset, setForceReset] = useState(false);
  const loadingTimeoutRef = useRef<NodeJS.Timeout | null>(null);
  

  const [showCreateBranchDialog, setShowCreateBranchDialog] = useState(false);
  const [baseBranchForCreation, setBaseBranchForCreation] = useState('');
  

  const [showCreateBranchFromCommitDialog, setShowCreateBranchFromCommitDialog] = useState(false);
  const [commitHashForBranch, setCommitHashForBranch] = useState('');
  

  const [detailsTab, setDetailsTab] = useState<'changes' | 'commits' | 'branches'>('changes');
  const [commits, setCommits] = useState<any[]>([]);
  const [branches, setBranches] = useState<any[]>([]);
  const [loadingDetails, setLoadingDetails] = useState(false);
  

  const [expandedFileGroups, setExpandedFileGroups] = useState<Set<string>>(new Set(['unstaged', 'staged']));
  
  const MAX_RENDERED_FILES = 200;
  

  const [expandedCommits, setExpandedCommits] = useState<Set<string>>(new Set());
  

  const [searchQueries, setSearchQueries] = useState<{
    changes: string;
    commits: string;
    branches: string;
  }>({
    changes: '',
    commits: '',
    branches: ''
  });
  


  const [selectedFiles, setSelectedFiles] = useState<Set<string>>(new Set());
  

  const [quickCommitMessage, setQuickCommitMessage] = useState('');


  const repositoryPath = workspacePath || '';


  const {
    isRepository,
    isLoading: statusLoading,
    currentBranch,
    staged,
    unstaged,
    untracked,
    ahead,
    behind,
    hasChanges: hasChangesValue,
    refresh: refreshGitState,
    refreshDetailed,
  } = useGitState({
    repositoryPath,
    isActive,
    refreshOnActive: true,
    refreshOnMount: true,
    layers: ['basic', 'status'],
  });


  const status = useMemo(() => ({
    current_branch: currentBranch || 'unknown',
    staged: staged,
    unstaged: unstaged,
    untracked: untracked,
    ahead: ahead,
    behind: behind,
  }), [currentBranch, staged, unstaged, untracked, ahead, behind]);


  const hasChanges = useCallback(() => hasChangesValue, [hasChangesValue]);
  const needsSync = useCallback(() => ahead > 0 || behind > 0, [ahead, behind]);


  const repoLoading = statusLoading && !isRepository;

  const { isOperating, addFiles, commit, push, pull, checkoutBranch, createBranch, deleteBranch } = useGitOperations({
    repositoryPath,
    autoRefresh: false
  });

  const { 
    commitMessage: aiCommitMessage, 
    isGeneratingCommit, 
    quickGenerateCommit,
    cancelCommitGeneration,
    error: aiError,
    clearError: clearAiError,
  } = useGitAgent({ repoPath: repositoryPath });

  const notification = useNotification();

  const getFileStatusInfo = useCallback((status: string) => {
    const statusLower = (status || '').toLowerCase();
    
    if (statusLower.includes('m') || statusLower.includes('modified')) {
      return {
        className: 'bitfun-git-panel__file-status-indicator--modified',
        text: 'M'
      };
    } else if (statusLower.includes('a') || statusLower.includes('added')) {
      return {
        className: 'bitfun-git-panel__file-status-indicator--added',
        text: 'A'
      };
    } else if (statusLower.includes('d') || statusLower.includes('deleted')) {
      return {
        className: 'bitfun-git-panel__file-status-indicator--deleted',
        text: 'D'
      };
    } else if (statusLower.includes('r') || statusLower.includes('renamed')) {
      return {
        className: 'bitfun-git-panel__file-status-indicator--renamed',
        text: 'R'
      };
    }
    

    return {
      className: 'bitfun-git-panel__file-status-indicator--modified',
      text: 'M'
    };
  }, [t]);

  const handleRefresh = useCallback(async () => {
    try {
      await refreshGitState({
        force: true,
        layers: ['basic', 'status'],
        reason: 'manual',
      });
    } catch (error) {
      log.error('Failed to refresh git state', { workspacePath, error });
    }
  }, [refreshGitState, workspacePath, log]);

  const handleInitGitRepository = useCallback(() => {
    // Fill chat input so the agent can initialize the repository.
    const message = t('init.chatPrompt');
    

    globalEventBus.emit('fill-chat-input', {
      content: message
    });
  }, []);

  useEffect(() => {
    if (repoLoading || statusLoading) {

      loadingTimeoutRef.current = setTimeout(() => {
        setForceReset(true);

        setTimeout(() => {
          setForceReset(false);
          handleRefresh();
        }, 100);
      }, 10000);
    } else {

      if (loadingTimeoutRef.current) {
        clearTimeout(loadingTimeoutRef.current);
        loadingTimeoutRef.current = null;
      }
    }

    return () => {
      if (loadingTimeoutRef.current) {
        clearTimeout(loadingTimeoutRef.current);
      }
    };
  }, [repoLoading, statusLoading, handleRefresh]);



  const loadCommits = useCallback(async () => {
    if (!workspacePath) return;
    setLoadingDetails(true);
    try {
      const result = await gitService.getCommits(workspacePath, { maxCount: 20 });
      setCommits(result);
    } catch (error) {
      log.error('Failed to load commits', { workspacePath, error });
      setCommits([]);
    } finally {
      setLoadingDetails(false);
    }
  }, [workspacePath]);

  const loadBranches = useCallback(async () => {
    if (!workspacePath) return;
    setLoadingDetails(true);
    try {
      const result = await gitService.getBranches(workspacePath, true);
      setBranches(result);
    } catch (error) {
      log.error('Failed to load branches', { workspacePath, error });
      setBranches([]);
    } finally {
      setLoadingDetails(false);
    }
  }, [workspacePath]);

  const handleQuickAction = useCallback(async (action: string) => {
    if (!workspacePath || !status) {
      return;
    }

    switch (action) {
      case 'stage-all':
        try {
          const addResult = await addFiles({ files: [], all: true });
          if (addResult.success) {
            await refreshGitState();
          } else if (addResult.error) {
            notification.error(t('notifications.stageFailed', { error: addResult.error }));
          }
        } catch (err) {
          log.error('Failed to stage all files', { workspacePath, error: err });
          notification.error(t('notifications.stageFailed', { error: String(err) }));
        }
        break;
      case 'pull':
        await pull();
        await refreshGitState();
        break;
    }
  }, [workspacePath, status, addFiles, pull, notification, refreshGitState, log, t]);

  const getAllUnstagedFiles = useCallback((): string[] => {
    if (!status) return [];
    const unstagedPaths = (status.unstaged || []).map(f => f.path);
    const untrackedPaths = status.untracked || [];
    return [...unstagedPaths, ...untrackedPaths];
  }, [status]);

  const toggleFileSelection = useCallback((filePath: string) => {
    setSelectedFiles(prev => {
      const newSet = new Set(prev);
      if (newSet.has(filePath)) {
        newSet.delete(filePath);
      } else {
        newSet.add(filePath);
      }
      return newSet;
    });
  }, []);

  const toggleSelectAll = useCallback(() => {
    const allFiles = getAllUnstagedFiles();
    setSelectedFiles(prev => {

      if (allFiles.length > 0 && allFiles.every(f => prev.has(f))) {
        return new Set();
      }

      return new Set(allFiles);
    });
  }, [getAllUnstagedFiles]);

  const handleStageSelectedFiles = useCallback(async () => {
    if (selectedFiles.size === 0) {
      notification.warning(t('notifications.selectFilesToStage'));
      return;
    }
    
    try {
      const addResult = await addFiles({ files: Array.from(selectedFiles), all: false });
      if (addResult.success) {
        setSelectedFiles(new Set());
        await refreshGitState();
        notification.success(t('notifications.stageSuccess', { count: selectedFiles.size }));
      } else if (addResult.error) {
        notification.error(t('notifications.stageFailed', { error: addResult.error }));
      }
    } catch (err) {
      log.error('Failed to stage selected files', { fileCount: selectedFiles.size, error: err });
      notification.error(t('notifications.stageFailed', { error: String(err) }));
    }
  }, [selectedFiles, addFiles, refreshGitState, notification, log, t]);

  const isAllSelected = useCallback(() => {
    const allFiles = getAllUnstagedFiles();
    return allFiles.length > 0 && allFiles.every(f => selectedFiles.has(f));
  }, [getAllUnstagedFiles, selectedFiles]);

  const isPartialSelected = useCallback(() => {
    const allFiles = getAllUnstagedFiles();
    const selectedCount = allFiles.filter(f => selectedFiles.has(f)).length;
    return selectedCount > 0 && selectedCount < allFiles.length;
  }, [getAllUnstagedFiles, selectedFiles]);

  const handlePush = useCallback(async () => {
    if (!workspacePath) return;
    await push({ force: false });
  }, [workspacePath, push]);

  const handleDiscardFile = useCallback(async (filePath: string, fileType: 'staged' | 'unstaged' | 'untracked') => {
    if (!workspacePath) return;
    

    const confirmMsg = fileType === 'untracked' 
      ? t('confirm.deleteFile', { file: filePath })
      : t('confirm.discardFile', { file: filePath });
    
    if (!confirm(confirmMsg)) return;

    try {
      if (fileType === 'untracked') {

        const { workspaceAPI } = await import('@/infrastructure/api');
        const fullPath = workspacePath.replace(/\\/g, '/') + '/' + filePath.replace(/\\/g, '/');
        await workspaceAPI.deleteFile(fullPath);
      } else if (fileType === 'staged') {

        const unstageResult = await gitService.resetFiles(workspacePath, [filePath], true);
        if (!unstageResult.success) {
          throw new Error(unstageResult.error || 'Failed to unstage file');
        }
        

        const restoreResult = await gitService.resetFiles(workspacePath, [filePath], false);
        if (!restoreResult.success) {
          throw new Error(restoreResult.error || 'Failed to restore file');
        }
      } else {

        const restoreResult = await gitService.resetFiles(workspacePath, [filePath], false);
        if (!restoreResult.success) {
          throw new Error(restoreResult.error || 'Failed to restore file');
        }
      }
      

      await handleRefresh();
      notification.success(t('notifications.fileRestored'));
    } catch (error) {
      log.error('Failed to discard file', { filePath, fileType, error });
      notification.error(t('notifications.fileRestoreFailed', { error: (error as Error).message }));
    }
  }, [workspacePath, handleRefresh, notification, t]);

  const handleDiscardAllChanges = useCallback(async () => {
    if (!workspacePath) return;
    

    const confirmMsg = t('confirm.discardAllChanges');
    
    if (!confirm(confirmMsg)) return;

    try {

      const stagedFiles = status?.staged?.map(f => f.path) || [];
      const unstagedFiles = status?.unstaged?.map(f => f.path) || [];
      const untrackedFiles = status?.untracked || [];
      
      if (stagedFiles.length > 0) {
        const unstageResult = await gitService.resetFiles(workspacePath, stagedFiles, true);
        if (!unstageResult.success) {
          log.warn('Failed to unstage files', { fileCount: stagedFiles.length, error: unstageResult.error });
        }
      }
      
      const allChangedFiles = [...new Set([...stagedFiles, ...unstagedFiles])];
      if (allChangedFiles.length > 0) {
        const restoreResult = await gitService.resetFiles(workspacePath, allChangedFiles, false);
        if (!restoreResult.success) {
          throw new Error(restoreResult.error || 'Failed to restore file');
        }
      }
      
      if (untrackedFiles.length > 0) {
        const { workspaceAPI } = await import('@/infrastructure/api');
        for (const file of untrackedFiles) {
          try {
            const fullPath = workspacePath.replace(/\\/g, '/') + '/' + file.replace(/\\/g, '/');
            await workspaceAPI.deleteFile(fullPath);
          } catch (err) {
            log.warn('Failed to delete untracked file', { file, error: err });
          }
        }
      }
      

      await handleRefresh();
      notification.success(t('notifications.allChangesRestored'));
    } catch (error) {
      log.error('Failed to discard all changes', { workspacePath, error });
      notification.error(t('notifications.restoreFailed', { error: (error as Error).message }));
    }
  }, [workspacePath, status, handleRefresh, notification, t]);

  const handleQuickCommit = useCallback(async () => {
    if (!quickCommitMessage.trim()) {
      notification.warning(t('notifications.enterCommitMessage'));
      return;
    }
    
    if (!status?.staged?.length) {
      notification.warning(t('notifications.noStagedFiles'));
      return;
    }
    
    try {
      const result = await commit({ message: quickCommitMessage.trim() });
      if (result.success) {
        setQuickCommitMessage('');
        await handleRefresh();
        await loadCommits();
        notification.success(t('notifications.commitSuccess'));
      } else {
        notification.error(t('notifications.commitFailed', { error: result.error || t('common.unknownError') }));
      }
    } catch (error) {
      notification.error(t('notifications.commitFailed', { error: (error as Error).message }));
    }
  }, [quickCommitMessage, status, commit, handleRefresh, loadCommits, notification, t]);

  useEffect(() => {
    if (aiCommitMessage) {
      const fullMessage = aiCommitMessage.fullMessage;
      setQuickCommitMessage(fullMessage);
    }
  }, [aiCommitMessage]);

  useEffect(() => {
    if (aiError) {
      notification.error(t('notifications.aiGenerateFailed', { error: aiError }));
      clearAiError();
    }
  }, [aiError, notification, clearAiError, t]);

  const handleAIGenerateCommit = useCallback(async () => {
    if (!status?.staged?.length && !status?.unstaged?.length && !status?.untracked?.length) {
      notification.warning(t('notifications.noFilesToGenerate'));
      return;
    }
    clearAiError();
    await quickGenerateCommit();
  }, [status, quickGenerateCommit, clearAiError, notification, t]);

  const handleAICodeReview = useCallback(async () => {

    const filePaths: string[] = [
      ...(status?.staged || []).map(f => f.path),
      ...(status?.unstaged || []).map(f => f.path),
      ...(status?.untracked || []),
    ];

    if (filePaths.length === 0) {
      notification.warning(t('review.noFilesToReview'));
      return;
    }

    const filesList = filePaths.map(p => `- ${p}`).join('\n');
    const displayMessage = t('review.displayMessage', { files: filesList });
    const reviewMessage = t('review.requestMessage', { files: filesList });

    try {

      const { FlowChatManager } = await import('@/flow_chat/services/FlowChatManager');
      const flowChatManager = FlowChatManager.getInstance();

      await flowChatManager.sendMessage(
        reviewMessage,
        undefined,
        displayMessage,
        'CodeReview'
      );
    } catch (error) {
      log.error('Failed to send code review request', { fileCount: filePaths.length, error });
      notification.error(t('review.sendFailed'));
    }
  }, [status, notification, log, t]);

  useEffect(() => {
    if (detailsTab === 'commits' && commits.length === 0) {
      loadCommits();
    } else if (detailsTab === 'branches' && branches.length === 0) {
      loadBranches();
    }
  }, [detailsTab, commits.length, branches.length, loadCommits, loadBranches]);

  useEffect(() => {
    if (!repositoryPath) return;

    const handleBranchChanged = () => {
      if (branches.length > 0) {
        loadBranches();
      }
    };


    globalEventBus.on('git:branch:changed', handleBranchChanged);
    globalEventBus.on('git:state:changed', (event: any) => {

      if (event?.changedLayers?.includes('basic') && branches.length > 0) {
        loadBranches();
      }
    });

    return () => {
      globalEventBus.off('git:branch:changed', handleBranchChanged);
    };
  }, [repositoryPath, branches.length, loadBranches]);

  const handleSwitchBranch = useCallback(async (branchName: string) => {
    const result = await checkoutBranch(branchName);
    
    if (result.success) {
      await handleRefresh();
      await loadBranches();
    }
  }, [checkoutBranch, handleRefresh, loadBranches]);

  const handleCreateBranchFrom = useCallback((baseBranch: string) => {
    setBaseBranchForCreation(baseBranch);
    setShowCreateBranchDialog(true);
  }, []);

  const handleCreateBranchConfirm = useCallback(async (newBranchName: string) => {
    const result = await createBranch(newBranchName, baseBranchForCreation);
    if (result.success) {
      setShowCreateBranchDialog(false);

      await loadBranches();
    }

  }, [createBranch, baseBranchForCreation, loadBranches]);

  const handleCreateBranchCancel = useCallback(() => {
    setShowCreateBranchDialog(false);
    setBaseBranchForCreation('');
  }, []);

  const handleDeleteBranch = useCallback(async (branchName: string, isCurrent: boolean) => {
    if (isCurrent) {
      notification.warning(t('notifications.cannotDeleteCurrentBranch'));
      return;
    }

    if (!confirm(t('confirm.deleteBranch', { branch: branchName }))) return;

    const result = await deleteBranch(branchName, false);
    if (result.success) {

      await loadBranches();
    }

  }, [deleteBranch, loadBranches, notification, t]);

  const handleResetToCommit = useCallback(async (commitHash: string) => {
    if (!workspacePath) return;
    
    const confirmMsg = t('confirm.resetToCommit', { hash: commitHash.substring(0, 7) });
    if (!confirm(confirmMsg)) return;

    try {

      const result = await gitService.resetToCommit(workspacePath, commitHash, 'mixed');
      
      if (!result.success) {
        throw new Error(result.error || 'Failed to reset');
      }
      

      await handleRefresh();

      await loadCommits();
      
      notification.success(t('notifications.resetSuccess', { hash: commitHash.substring(0, 7) }));
    } catch (error) {
      log.error('Failed to reset to commit', { commitHash: commitHash.substring(0, 7), error });
      notification.error(t('notifications.resetFailed', { error: (error as Error).message }));
    }
  }, [workspacePath, handleRefresh, loadCommits, notification, t]);

  const handleCreateBranchFromCommit = useCallback((commitHash: string) => {
    setCommitHashForBranch(commitHash);
    setShowCreateBranchFromCommitDialog(true);
  }, []);

  const handleConfirmCreateBranchFromCommit = useCallback(async (newBranchName: string) => {
    const result = await createBranch(newBranchName.trim(), commitHashForBranch);
    if (result.success) {
      setShowCreateBranchFromCommitDialog(false);

      await loadBranches();
    }

  }, [createBranch, commitHashForBranch, loadBranches]);

  const handleCopyCommitHash = useCallback(async (commitHash: string) => {
    try {
      await navigator.clipboard.writeText(commitHash);
    } catch (error) {
      log.warn('Clipboard API failed, using fallback', { commitHash });
      const textArea = document.createElement('textarea');
      textArea.value = commitHash;
      textArea.style.position = 'fixed';
      textArea.style.left = '-999999px';
      document.body.appendChild(textArea);
      textArea.select();
      try {
        document.execCommand('copy');
      } catch (err) {
        log.error('Failed to copy commit hash', { commitHash, error: err });
      }
      document.body.removeChild(textArea);
    }
  }, [log]);


  const [loadingDiffFiles, setLoadingDiffFiles] = useState<Set<string>>(new Set());

  const handleOpenFileDiff = useCallback(async (filePath: string, status: string) => {
    if (!workspacePath) {
      log.error('Cannot open file diff: workspace path is missing');
      return;
    }

    const fileName = filePath.split(/[/\\]/).pop() || filePath;
    setLoadingDiffFiles(prev => new Set(prev).add(filePath));

    // Optimization: open/create the tab once to avoid initializing DiffEditor with empty content.
    setTimeout(async () => {
      const statusLower = (status || '').toLowerCase();
      const isDeleted = statusLower.includes('d') || statusLower.includes('deleted');

      try {
        const { workspaceAPI } = await import('@/infrastructure/api');
        const fullPath = `${workspacePath}/${filePath}`;
        let modifiedContent = '';
        
        if (isDeleted) {
          modifiedContent = '';
        } else {
          try {
            modifiedContent = await workspaceAPI.readFileContent(fullPath);
          } catch (error) {
            log.error('Failed to read modified content', { filePath, fullPath, error });
            throw error;
          }
        }

        if (status === 'Untracked') {
          createCodeEditorTab(fullPath, fileName, undefined, 'agent');
        } else {
          let originalContent = '';
          try {
            originalContent = await gitService.getFileContent(workspacePath, filePath, 'HEAD');
          } catch (error) {
            log.debug('Failed to read HEAD version, might be new file', { filePath, error });
          }

          createDiffEditorTab(
            filePath,
            fileName,
            originalContent,
            modifiedContent,
            false,
            'agent',
            workspacePath
          );
        }
      } catch (error) {
        log.error('Failed to open file diff', { filePath, status, error });
        const errorMessage = error instanceof Error ? error.message : String(error);
        notification.error(t('notifications.openDiffFailedWithPath', { error: errorMessage, file: filePath }));
      } finally {
        setLoadingDiffFiles(prev => {
          const newSet = new Set(prev);
          newSet.delete(filePath);
          return newSet;
        });
      }
    }, 0);
  }, [workspacePath, notification, log, t]);

  const toggleFileGroup = useCallback((groupId: string) => {
    setExpandedFileGroups(prev => {
      const newSet = new Set(prev);
      if (newSet.has(groupId)) {
        newSet.delete(groupId);
      } else {
        newSet.add(groupId);
      }
      return newSet;
    });
  }, []);

  const toggleCommitExpand = useCallback((commitHash: string, event?: React.MouseEvent) => {
    if (event) {
      event.stopPropagation();
    }
    setExpandedCommits(prev => {
      const newSet = new Set(prev);
      if (newSet.has(commitHash)) {
        newSet.delete(commitHash);
      } else {
        newSet.add(commitHash);
      }
      return newSet;
    });
  }, []);

  const updateSearchQuery = useCallback((tab: 'changes' | 'commits' | 'branches', query: string) => {
    setSearchQueries(prev => ({
      ...prev,
      [tab]: query
    }));
  }, []);

  const clearSearchQuery = useCallback((tab: 'changes' | 'commits' | 'branches') => {
    setSearchQueries(prev => ({
      ...prev,
      [tab]: ''
    }));
  }, []);

  const filteredFiles = useMemo(() => {
    if (!status) return { unstaged: [], untracked: [], staged: [] };
    
    const query = searchQueries.changes.toLowerCase().trim();
    if (!query) return { unstaged: status.unstaged || [], untracked: status.untracked || [], staged: status.staged || [] };

    return {
      unstaged: (status.unstaged || []).filter(file => file.path.toLowerCase().includes(query)),
      untracked: (status.untracked || []).filter(file => file.toLowerCase().includes(query)),
      staged: (status.staged || []).filter(file => file.path.toLowerCase().includes(query))
    };
  }, [status, searchQueries.changes]);

  const filteredCommits = useMemo(() => {
    const query = searchQueries.commits.toLowerCase().trim();
    if (!query) return commits;
    
    return commits.filter(commit => {
      const message = (commit.summary || commit.message || '').toLowerCase();
      const author = (commit.author?.name || '').toLowerCase();
      const hash = (commit.hash || '').toLowerCase();
      return message.includes(query) || author.includes(query) || hash.includes(query);
    });
  }, [commits, searchQueries.commits]);

  const filteredBranches = useMemo(() => {
    const query = searchQueries.branches.toLowerCase().trim();
    if (!query) return branches;

    return branches.filter(branch => 
      (branch.name || '').toLowerCase().includes(query)
    );
  }, [branches, searchQueries.branches]);

  const handleViewAllDiff = useCallback(() => {
    if (!workspacePath) {
      return;
    }

    window.dispatchEvent(new CustomEvent('expand-right-panel'));

    setTimeout(() => {
      createTab({
        type: 'git-diff',
        title: t('tabs.changeDiff'),
        mode: 'agent',
        data: {
          repositoryPath: workspacePath,
          showStaged: false
        },
        metadata: {
          duplicateCheckKey: `git-diff-${workspacePath}`
        },
        checkDuplicate: true
      });
    }, 250);
  }, [workspacePath, t]);


  if (!repoLoading && !isRepository) {
    return (
      <div className={`bitfun-git-panel bitfun-git-panel--not-repository ${className}`}>
        <PanelHeader
          title={t('title')}
        />
        <div className="bitfun-git-panel__content">
          <div className="bitfun-git-panel__init-container">
            <div className="bitfun-git-panel__init-decoration">
              <div className="bitfun-git-panel__init-line bitfun-git-panel__init-line--dashed" />
              <div className="bitfun-git-panel__init-dot" />
              <div className="bitfun-git-panel__init-line bitfun-git-panel__init-line--solid" />
            </div>
            
            <div className="bitfun-git-panel__init-card">
              <div className="bitfun-git-panel__init-icon">
                <GitBranch size={24} />
              </div>
              <div className="bitfun-git-panel__init-text">
                <h3>{t('init.title')}</h3>
                <p>{t('init.notRepository')}</p>
              </div>
              <button 
                className="bitfun-git-panel__init-button"
                onClick={handleInitGitRepository}
              >
                <Plus size={14} />
                <span>{t('init.initButton')}</span>
              </button>
            </div>
            
            <div className="bitfun-git-panel__init-decoration">
              <div className="bitfun-git-panel__init-line bitfun-git-panel__init-line--solid" />
              <div className="bitfun-git-panel__init-dot bitfun-git-panel__init-dot--muted" />
              <div className="bitfun-git-panel__init-line bitfun-git-panel__init-line--dashed" />
            </div>
            
            <div className="bitfun-git-panel__init-hint">
              <span>{t('init.hint')}</span>
            </div>
          </div>
        </div>
      </div>
    );
  }


  if ((repoLoading || statusLoading) && !forceReset) {
    return (
      <div className={`bitfun-git-panel bitfun-git-panel--loading ${className}`}>
        <PanelHeader
          title={t('title')}
          actions={
            <IconButton
              size="xs"
              onClick={(e) => {
                e.stopPropagation();
                setForceReset(true);
                setTimeout(() => {
                  setForceReset(false);
                  handleRefresh();
                }, 100);
              }}
              tooltip={t('actions.forceRefresh')}
            >
              <RefreshCw size={14} />
            </IconButton>
          }
        />
        <div className="bitfun-git-panel__content">
          <div className="bitfun-git-panel__loading-state">
            <CubeLoading size="medium" text={t('loading.text')} />
            <p style={{ fontSize: '10px', marginTop: '8px', opacity: 0.6 }}>
              {t('loading.hint')}
            </p>
          </div>
        </div>
      </div>
    );
  }

  return (
    <div className={`bitfun-git-panel ${isActive ? 'bitfun-git-panel--active' : ''} ${className}`} onClick={onActivate}>
      <PanelHeader
        title={t('title')}
        actions={
          <>
            <IconButton
              size="xs"
              onClick={(e) => {
                e.stopPropagation();
                handleRefresh();
              }}
              disabled={repoLoading || statusLoading}
              tooltip={t('actions.refresh')}
            >
              <RefreshCw size={14} className={repoLoading || statusLoading ? 'bitfun-git-panel__refresh-btn--spinning' : ''} />
            </IconButton>
            {isOperating && (
              <div className="bitfun-git-panel__operation-indicator">
                <div className="bitfun-git-panel__loading-spinner bitfun-git-panel__loading-spinner--small" />
              </div>
            )}
          </>
        }
      />

      <div className="bitfun-git-panel__content">

        <div className="bitfun-git-panel__commit-section">
          <div className="bitfun-git-panel__status-row">
            <div className="bitfun-git-panel__status-item">
              <GitBranch size={12} />
              <span className="bitfun-git-panel__status-branch">{status?.current_branch || 'unknown'}</span>
            </div>
            <div className="bitfun-git-panel__status-badges">
              {(status?.staged?.length || 0) > 0 && (
                <Tooltip content={t('status.staged')}>
                  <span className="bitfun-git-panel__status-badge bitfun-git-panel__status-badge--staged">
                    {status?.staged?.length}
                  </span>
                </Tooltip>
              )}
              {((status?.unstaged?.length || 0) + (status?.untracked?.length || 0)) > 0 && (
                <Tooltip content={t('status.unstaged')}>
                  <span className="bitfun-git-panel__status-badge bitfun-git-panel__status-badge--unstaged">
                    {(status?.unstaged?.length || 0) + (status?.untracked?.length || 0)}
                  </span>
                </Tooltip>
              )}
              {(status?.ahead || 0) > 0 && (
                <Tooltip content={t('status.ahead')}>
                  <span className="bitfun-git-panel__status-badge bitfun-git-panel__status-badge--ahead">
                    ↑{status?.ahead}
                  </span>
                </Tooltip>
              )}
              {(status?.behind || 0) > 0 && (
                <Tooltip content={t('status.behind')}>
                  <span className="bitfun-git-panel__status-badge bitfun-git-panel__status-badge--behind">
                    ↓{status?.behind}
                  </span>
                </Tooltip>
              )}
            </div>
            <div className="bitfun-git-panel__status-actions">
              <Tooltip content={t('actions.push')}>
                <IconButton
                  size="small"
                  variant="ghost"
                  onClick={handlePush}
                  disabled={isOperating}
                >
                  <ArrowUp size={14} />
                </IconButton>
              </Tooltip>
              <Tooltip content={t('actions.pull')}>
                <IconButton
                  size="small"
                  variant="ghost"
                  onClick={() => handleQuickAction('pull')}
                  disabled={isOperating}
                >
                  <ArrowDown size={14} />
                </IconButton>
              </Tooltip>
            </div>
          </div>

          <div className={`bitfun-git-panel__commit-input-wrapper ${isGeneratingCommit ? 'bitfun-git-panel__commit-input-wrapper--generating' : ''}`}>
            <Textarea
              className="bitfun-git-panel__commit-input"
              placeholder={status?.staged?.length ? t('commit.inputPlaceholder') : t('commit.inputPlaceholderNoStaged')}
              value={quickCommitMessage}
              onChange={(e) => setQuickCommitMessage(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === 'Enter' && !e.shiftKey && e.ctrlKey) {
                  e.preventDefault();
                  handleQuickCommit();
                }
              }}
              disabled={isOperating || isGeneratingCommit}
            />
            {isGeneratingCommit ? (
              <IconButton
                className="bitfun-git-panel__ai-btn bitfun-git-panel__ai-btn--cancel"
                variant="danger"
                size="xs"
                onClick={cancelCommitGeneration}
                tooltip={t('actions.cancelGenerate')}
              >
                <span className="bitfun-git-panel__cancel-ring" />
              </IconButton>
            ) : (
              <IconButton
                className="bitfun-git-panel__ai-btn"
                variant="ai"
                size="xs"
                onClick={handleAIGenerateCommit}
                disabled={isOperating}
                tooltip={t('actions.aiGenerateCommit')}
              >
                <Sparkles size={14} />
              </IconButton>
            )}
          </div>
          
          <div className="bitfun-git-panel__commit-buttons">
            <Button 
              size="small"
              variant={quickCommitMessage.trim() && status?.staged?.length ? 'primary' : 'secondary'}
              onClick={handleQuickCommit}
              disabled={!status?.staged?.length || !quickCommitMessage.trim() || isOperating || isGeneratingCommit}
              className="bitfun-git-panel__commit-btn"
            >
              {status?.staged?.length ? t('actions.commitWithCount', { count: status.staged.length }) : t('actions.commit')}
            </Button>
            <Tooltip content={t('actions.aiCodeReview')}>
              <Button
                size="small"
                variant="secondary"
                onClick={handleAICodeReview}
                disabled={isOperating || !(status?.staged?.length || status?.unstaged?.length || status?.untracked?.length)}
                className="bitfun-git-panel__review-btn"
              >
                <ScanSearch size={14} />
              </Button>
            </Tooltip>
          </div>
        </div>

        <div className="bitfun-git-panel__section">
          <div className="bitfun-git-panel__section-content">
              <div className="bitfun-git-panel__tabs">
                <IconButton
                  className={`bitfun-git-panel__tab ${detailsTab === 'changes' ? 'bitfun-git-panel__tab--active' : ''}`}
                  variant="ghost"
                  size="small"
                  onClick={() => setDetailsTab('changes')}
                >
                  <AlertCircle size={14} className="bitfun-git-panel__tab-icon" />
                  <span className="bitfun-git-panel__tab-text">{t('tabs.changes')}</span>
                </IconButton>
                <IconButton
                  className={`bitfun-git-panel__tab ${detailsTab === 'commits' ? 'bitfun-git-panel__tab--active' : ''}`}
                  variant="ghost"
                  size="small"
                  onClick={() => setDetailsTab('commits')}
                >
                  <History size={14} className="bitfun-git-panel__tab-icon" />
                  <span className="bitfun-git-panel__tab-text">{t('tabs.commits')}</span>
                </IconButton>
                <IconButton
                  className={`bitfun-git-panel__tab ${detailsTab === 'branches' ? 'bitfun-git-panel__tab--active' : ''}`}
                  variant="ghost"
                  size="small"
                  onClick={() => setDetailsTab('branches')}
                >
                  <GitBranch size={14} className="bitfun-git-panel__tab-icon" />
                  <span className="bitfun-git-panel__tab-text">{t('tabs.branches')}</span>
                </IconButton>
              </div>

              <div className="bitfun-git-panel__search-area">
                {detailsTab === 'changes' && (
                  <div className="bitfun-git-panel__search-row">
                    <IconButton
                      className={`bitfun-git-panel__icon-button ${!(status?.unstaged?.length || status?.untracked?.length || status?.staged?.length) ? 'bitfun-git-panel__icon-button--disabled' : ''}`}
                      variant="ghost"
                      size="small"
                      onClick={handleViewAllDiff}
                      disabled={!(status?.unstaged?.length || status?.untracked?.length || status?.staged?.length)}
                      tooltip={t('actions.viewAllDiff')}
                    >
                      <Eye size={16} />
                    </IconButton>
                    
                    <SearchComponent
                      className="bitfun-git-panel__search-box"
                      placeholder={t('search.files')}
                      value={searchQueries.changes}
                      onChange={(value) => updateSearchQuery('changes', value)}
                      onClear={() => clearSearchQuery('changes')}
                    />
                  </div>
                )}

                {detailsTab === 'commits' && (
                  <div className="bitfun-git-panel__search-row">
                    <SearchComponent
                      className="bitfun-git-panel__search-box"
                      placeholder={t('search.commits')}
                      value={searchQueries.commits}
                      onChange={(value) => updateSearchQuery('commits', value)}
                      onClear={() => clearSearchQuery('commits')}
                    />
                  </div>
                )}

                {detailsTab === 'branches' && (
                  <div className="bitfun-git-panel__search-row">
                    <IconButton
                      className="bitfun-git-panel__icon-button"
                      variant="ghost"
                      size="small"
                      onClick={() => {
                        if (!workspacePath) return;

                        createTab({
                          type: 'git-graph',
                          title: t('tabs.branchGraph'),
                          mode: 'agent',
                          data: {
                            repositoryPath: workspacePath,
                          },
                          metadata: {
                            duplicateCheckKey: `git-graph-${workspacePath}`
                          },
                          checkDuplicate: true
                        });
                      }}
                      tooltip={t('actions.viewBranchGraph')}
                    >
                      <History size={16} />
                    </IconButton>
                    
                    <SearchComponent
                      className="bitfun-git-panel__search-box"
                      placeholder={t('search.branches')}
                      value={searchQueries.branches}
                      onChange={(value) => updateSearchQuery('branches', value)}
                      onClear={() => clearSearchQuery('branches')}
                    />
                  </div>
                )}
              </div>

              <div className="bitfun-git-panel__tab-content">
                {detailsTab === 'changes' && (
                  <div className="bitfun-git-panel__list">
                    {status && (status.unstaged?.length || status.untracked?.length || status.staged?.length) ? (
                      <>
                        {filteredFiles.unstaged.length > 0 && (
                          <>
                            <div 
                              className={`bitfun-git-panel__list-header bitfun-git-panel__list-header--collapsible ${selectedFiles.size > 0 ? 'bitfun-git-panel__list-header--has-selection' : ''}`}
                            >
                              <IconButton
                                className={`bitfun-git-panel__checkbox-btn ${selectedFiles.size > 0 ? 'bitfun-git-panel__checkbox-btn--visible' : ''}`}
                                variant="ghost"
                                size="xs"
                                onClick={(e) => {
                                  e.stopPropagation();
                                  toggleSelectAll();
                                }}
                                tooltip={isAllSelected()
                                  ? t('selection.deselectAll')
                                  : t('selection.selectAll')
                                }
                              >
                                {isAllSelected() ? (
                                  <CheckCircle size={14} className="bitfun-git-panel__checkbox--checked" />
                                ) : isPartialSelected() ? (
                                  <Minus size={14} className="bitfun-git-panel__checkbox--partial" />
                                ) : (
                                  <Circle size={14} className="bitfun-git-panel__checkbox--unchecked" />
                                )}
                              </IconButton>
                              
                              <div 
                                className="bitfun-git-panel__list-header-content"
                                onClick={() => toggleFileGroup('unstaged')}
                              >
                                {expandedFileGroups.has('unstaged') ? (
                                  <ChevronDown size={12} />
                                ) : (
                                  <ChevronRight size={12} />
                                )}
                                <span>
                                  {searchQueries.changes
                                    ? t('fileGroups.unstagedWithFilter', {
                                        filtered: filteredFiles.unstaged.length,
                                        total: status.unstaged?.length || 0
                                      })
                                    : t('fileGroups.unstagedWithCount', { count: filteredFiles.unstaged.length })
                                  }
                                </span>
                              </div>
                              
                              <IconButton
                                className={`bitfun-git-panel__header-action-btn ${selectedFiles.size > 0 ? 'bitfun-git-panel__header-action-btn--active' : ''}`}
                                variant="ghost"
                                size="xs"
                                onClick={(e) => {
                                  e.stopPropagation();
                                  handleStageSelectedFiles();
                                }}
                                tooltip={selectedFiles.size > 0
                                  ? t('actions.stageSelected', { count: selectedFiles.size })
                                  : t('notifications.selectFilesToStage')
                                }
                                disabled={isOperating || selectedFiles.size === 0}
                              >
                                <Check size={14} />
                              </IconButton>
                            </div>
                            {expandedFileGroups.has('unstaged') && (() => {
                              const visibleFiles = filteredFiles.unstaged.slice(0, MAX_RENDERED_FILES);
                              const hiddenCount = filteredFiles.unstaged.length - visibleFiles.length;
                              return (
                                <>
                                  {visibleFiles.map((file, index) => {
                                    const statusInfo = getFileStatusInfo(file.status);
                                    const { fileName, dirPath } = getFileNameAndDir(file.path);
                                    const isSelected = selectedFiles.has(file.path);
                                    const isLoading = loadingDiffFiles.has(file.path);
                                    return (
                                      <div 
                                        key={`unstaged-${index}`}
                                        className={`bitfun-git-panel__list-item bitfun-git-panel__file-item bitfun-git-panel__file-item--clickable ${isSelected ? 'bitfun-git-panel__file-item--selected' : ''} ${isLoading ? 'bitfun-git-panel__file-item--loading' : ''}`}
                                        onClick={() => !isLoading && handleOpenFileDiff(file.path, file.status)}
                                        title={isLoading ? t('common.loading') : t('tooltips.viewDiff')}
                                      >
                                        <button
                                          className={`bitfun-git-panel__checkbox-btn ${isSelected ? 'bitfun-git-panel__checkbox-btn--visible' : ''}`}
                                          onClick={(e) => {
                                            e.stopPropagation();
                                            toggleFileSelection(file.path);
                                          }}
                                          title={isSelected ? t('selection.deselectFile') : t('selection.selectFile')}
                                        >
                                          {isSelected ? (
                                            <CheckCircle size={14} className="bitfun-git-panel__checkbox--checked" />
                                          ) : (
                                            <Circle size={14} className="bitfun-git-panel__checkbox--unchecked" />
                                          )}
                                        </button>
                                        <span className="bitfun-git-panel__file-name">{fileName}</span>
                                        {dirPath && <span className="bitfun-git-panel__file-dir">{dirPath}</span>}
                                        <span className={`bitfun-git-panel__file-status-badge ${statusInfo.className}`} title={file.status}>
                                          {statusInfo.text}
                                        </span>
                                        <IconButton
                                          className="bitfun-git-panel__file-action-btn"
                                          variant="ghost"
                                          size="xs"
                                          onClick={(e) => {
                                            e.stopPropagation();
                                            handleDiscardFile(file.path, 'unstaged');
                                          }}
                                          disabled={isOperating}
                                          tooltip={t('actions.discardFile')}
                                        >
                                          <RotateCcw size={12} />
                                        </IconButton>
                                      </div>
                                    );
                                  })}
                                  {hiddenCount > 0 && (
                                    <div className="bitfun-git-panel__list-item bitfun-git-panel__file-item bitfun-git-panel__file-overflow-hint" style={{ opacity: 0.6, fontSize: '11px', justifyContent: 'center', padding: '6px 8px' }}>
                                      {t('fileGroups.moreFiles', { count: hiddenCount })}
                                    </div>
                                  )}
                                </>
                              );
                            })()}
                          </>
                        )}
                        {filteredFiles.untracked.length > 0 && (
                          <>
                            <div 
                              className="bitfun-git-panel__list-header bitfun-git-panel__list-header--collapsible"
                              onClick={() => toggleFileGroup('untracked')}
                            >
                              {expandedFileGroups.has('untracked') ? (
                                <ChevronDown size={12} />
                              ) : (
                                <ChevronRight size={12} />
                              )}
                              <span>
                                {searchQueries.changes
                                  ? t('fileGroups.untrackedWithFilter', {
                                      filtered: filteredFiles.untracked.length,
                                      total: status.untracked?.length || 0
                                    })
                                  : t('fileGroups.untrackedWithCount', { count: filteredFiles.untracked.length })
                                }
                              </span>
                            </div>
                            {expandedFileGroups.has('untracked') && (() => {
                              const visibleFiles = filteredFiles.untracked.slice(0, MAX_RENDERED_FILES);
                              const hiddenCount = filteredFiles.untracked.length - visibleFiles.length;
                              return (
                                <>
                                  {visibleFiles.map((filePath, index) => {
                                    const { fileName, dirPath } = getFileNameAndDir(filePath);
                                    const isSelected = selectedFiles.has(filePath);
                                    const isLoading = loadingDiffFiles.has(filePath);
                                    return (
                                      <div 
                                        key={`untracked-${index}`}
                                        className={`bitfun-git-panel__list-item bitfun-git-panel__file-item bitfun-git-panel__file-item--clickable ${isSelected ? 'bitfun-git-panel__file-item--selected' : ''} ${isLoading ? 'bitfun-git-panel__file-item--loading' : ''}`}
                                        onClick={() => !isLoading && handleOpenFileDiff(filePath, 'Untracked')}
                                        title={isLoading ? t('common.loading') : t('tooltips.viewDiff')}
                                      >
                                        <IconButton
                                          className={`bitfun-git-panel__checkbox-btn ${isSelected ? 'bitfun-git-panel__checkbox-btn--visible' : ''}`}
                                          variant="ghost"
                                          size="xs"
                                          onClick={(e) => {
                                            e.stopPropagation();
                                            toggleFileSelection(filePath);
                                          }}
                                          tooltip={isSelected ? t('selection.deselectFile') : t('selection.selectFile')}
                                        >
                                          {isSelected ? (
                                            <CheckCircle size={14} className="bitfun-git-panel__checkbox--checked" />
                                          ) : (
                                            <Circle size={14} className="bitfun-git-panel__checkbox--unchecked" />
                                          )}
                                        </IconButton>
                                        <span className="bitfun-git-panel__file-name">{fileName}</span>
                                        {dirPath && <span className="bitfun-git-panel__file-dir">{dirPath}</span>}
                                        <Tooltip content={t('status.untracked')}>
                                          <span className="bitfun-git-panel__file-status-badge bitfun-git-panel__file-status-indicator--added">
                                            U
                                          </span>
                                        </Tooltip>
                                        <IconButton
                                          className="bitfun-git-panel__file-action-btn"
                                          variant="ghost"
                                          size="xs"
                                          onClick={(e) => {
                                            e.stopPropagation();
                                            handleDiscardFile(filePath, 'untracked');
                                          }}
                                          disabled={isOperating}
                                          tooltip={t('actions.deleteFile')}
                                        >
                                          <RotateCcw size={12} />
                                        </IconButton>
                                      </div>
                                    );
                                  })}
                                  {hiddenCount > 0 && (
                                    <div className="bitfun-git-panel__list-item bitfun-git-panel__file-item bitfun-git-panel__file-overflow-hint" style={{ opacity: 0.6, fontSize: '11px', justifyContent: 'center', padding: '6px 8px' }}>
                                      {t('fileGroups.moreFiles', { count: hiddenCount })}
                                    </div>
                                  )}
                                </>
                              );
                            })()}
                          </>
                        )}
                        {filteredFiles.staged.length > 0 && (
                          <>
                            <div 
                              className="bitfun-git-panel__list-header bitfun-git-panel__list-header--collapsible"
                              onClick={() => toggleFileGroup('staged')}
                            >
                              {expandedFileGroups.has('staged') ? (
                                <ChevronDown size={12} />
                              ) : (
                                <ChevronRight size={12} />
                              )}
                              <span>
                                {searchQueries.changes
                                  ? t('fileGroups.stagedWithFilter', {
                                      filtered: filteredFiles.staged.length,
                                      total: status.staged?.length || 0
                                    })
                                  : t('fileGroups.stagedWithCount', { count: filteredFiles.staged.length })
                                }
                              </span>
                            </div>
                            {expandedFileGroups.has('staged') && filteredFiles.staged.map((file, index) => {
                              const statusInfo = getFileStatusInfo(file.status);
                              const isLoading = loadingDiffFiles.has(file.path);
                              return (
                                <Tooltip content={isLoading ? t('common.loading') : t('tooltips.viewDiff')}>
                                  <div 
                                    key={`staged-${index}`} 
                                    className={`bitfun-git-panel__list-item bitfun-git-panel__file-item bitfun-git-panel__file-item--clickable ${isLoading ? 'bitfun-git-panel__file-item--loading' : ''}`}
                                    onClick={() => !isLoading && handleOpenFileDiff(file.path, file.status)}
                                  >
                                    <Tooltip content={file.path}>
                                      <span className="bitfun-git-panel__list-item-text">{file.path}</span>
                                    </Tooltip>
                                    <Tooltip content={file.status}>
                                      <span className={`bitfun-git-panel__file-status-badge ${statusInfo.className}`}>
                                        {statusInfo.text}
                                      </span>
                                    </Tooltip>
                                    <IconButton
                                      className="bitfun-git-panel__file-action-btn"
                                      variant="ghost"
                                      size="xs"
                                      onClick={(e) => {
                                        e.stopPropagation();
                                        handleDiscardFile(file.path, 'staged');
                                      }}
                                      disabled={isOperating}
                                      tooltip={t('actions.discardFile')}
                                    >
                                      <RotateCcw size={12} />
                                    </IconButton>
                                  </div>
                                </Tooltip>
                              );
                            })}
                          </>
                        )}
                        {searchQueries.changes && (filteredFiles.unstaged.length === 0 && filteredFiles.untracked.length === 0 && filteredFiles.staged.length === 0) && (
                          <div className="bitfun-git-panel__list-empty">{t('empty.noMatchingFiles')}</div>
                        )}
                      </>
                    ) : (
                      <div className="bitfun-git-panel__list-empty">{t('empty.noChanges')}</div>
                    )}
                  </div>
                )}

                {detailsTab === 'commits' && (
                  <div className="bitfun-git-panel__list">
                    {loadingDetails ? (
                      <div className="bitfun-git-panel__list-empty">{t('common.loading')}</div>
                    ) : filteredCommits.length > 0 ? (
                      <>
                        {filteredCommits.map((commit, index) => {
                          const isExpanded = expandedCommits.has(commit.hash);
                          const commitSummary = (commit.summary || commit.message || '').split('\n')[0];
                          const commitBody = (commit.message || '').split('\n').slice(1).join('\n').trim();
                          
                          return (
                            <div 
                              key={commit.hash || index} 
                              className={`bitfun-git-panel__list-item bitfun-git-panel__commit-item ${isExpanded ? 'bitfun-git-panel__commit-item--expanded' : ''} ${index === filteredCommits.length - 1 ? 'bitfun-git-panel__commit-item--last' : ''}`}
                            >
                              <div 
                                className={`bitfun-git-panel__timeline-node ${isExpanded ? 'bitfun-git-panel__timeline-node--expanded' : ''}`}
                                onClick={(e) => toggleCommitExpand(commit.hash, e)}
                                title={isExpanded ? t('tooltips.collapseDetails') : t('tooltips.expandDetails')}
                              >
                                <div className="bitfun-git-panel__timeline-dot" />
                              </div>
                              <div className="bitfun-git-panel__commit-header">
                                <div 
                                  className="bitfun-git-panel__commit-info"
                                  onClick={(e) => {
                                    e.stopPropagation();
                                    toggleCommitExpand(commit.hash);
                                  }}
                                >
                                  <Tooltip content={commitSummary} placement="top">
                                    <div className="bitfun-git-panel__commit-message">{commitSummary}</div>
                                  </Tooltip>
                                  <div className="bitfun-git-panel__commit-meta">
                                    <span className="bitfun-git-panel__commit-author">{commit.author || t('common.unknown')}</span>
                                    <span className="bitfun-git-panel__commit-hash">{commit.hash?.substring(0, 7)}</span>
                                  </div>
                                </div>
                                
                                <div className="bitfun-git-panel__commit-actions">
                                  <IconButton
                                    className="bitfun-git-panel__commit-action-btn"
                                    variant="ghost"
                                    size="xs"
                                    onClick={(e) => {
                                      e.stopPropagation();
                                      handleCopyCommitHash(commit.hash);
                                    }}
                                    tooltip={t('actions.copyCommitHash')}
                                  >
                                    <Copy size={14} />
                                  </IconButton>
                                  <IconButton
                                    className="bitfun-git-panel__commit-action-btn"
                                    variant="ghost"
                                    size="xs"
                                    onClick={(e) => {
                                      e.stopPropagation();
                                      handleResetToCommit(commit.hash);
                                    }}
                                    disabled={isOperating}
                                    tooltip={t('actions.resetToCommit')}
                                  >
                                    <RotateCcw size={14} />
                                  </IconButton>
                                  <IconButton
                                    className="bitfun-git-panel__commit-action-btn"
                                    variant="ghost"
                                    size="xs"
                                    onClick={(e) => {
                                      e.stopPropagation();
                                      handleCreateBranchFromCommit(commit.hash);
                                    }}
                                    disabled={isOperating}
                                    tooltip={t('actions.createBranchFromCommit')}
                                  >
                                    <Plus size={14} />
                                  </IconButton>
                                </div>
                              </div>

                              {isExpanded && (
                                <div className="bitfun-git-panel__commit-details" onClick={(e) => e.stopPropagation()}>
                                  {commitBody && (
                                    <div className="bitfun-git-panel__commit-body">
                                      <pre className="bitfun-git-panel__commit-body-text">{commitBody}</pre>
                                    </div>
                                  )}

                                  {commit.files && commit.files.length > 0 && (
                                    <div className="bitfun-git-panel__commit-files">
                                      <div className="bitfun-git-panel__commit-files-header">
                                        <FileText size={12} />
                                        <span>{t('commit.changedFiles', { count: commit.files.length })}</span>
                                      </div>
                                      <div className="bitfun-git-panel__commit-files-list">
                                        {commit.files.map((file: any, fileIndex: number) => (
                                          <div key={fileIndex} className="bitfun-git-panel__commit-file-item">
                                            <div className="bitfun-git-panel__commit-file-icon">
                                              <File size={12} />
                                            </div>
                                            <div className="bitfun-git-panel__commit-file-content">
                                              <div className="bitfun-git-panel__commit-file-path">{file.path}</div>
                                              <div className="bitfun-git-panel__commit-file-meta">
                                                <span className={`bitfun-git-panel__commit-file-status bitfun-git-panel__commit-file-status--${(file.status || '').toLowerCase()}`}>
                                                  {file.status}
                                                </span>
                                                {(file.additions > 0 || file.deletions > 0) && (
                                                  <span className="bitfun-git-panel__commit-file-changes">
                                                    {file.additions > 0 && <span className="bitfun-git-panel__file-additions">+{file.additions}</span>}
                                                    {file.deletions > 0 && <span className="bitfun-git-panel__file-deletions">-{file.deletions}</span>}
                                                  </span>
                                                )}
                                              </div>
                                            </div>
                                          </div>
                                        ))}
                                      </div>
                                    </div>
                                  )}
                                </div>
                              )}
                            </div>
                          );
                        })}
                      </>
                    ) : (
                      <div className="bitfun-git-panel__list-empty">
                        {searchQueries.commits ? t('empty.noMatchingCommits') : t('empty.noCommits')}
                      </div>
                    )}
                  </div>
                )}

                {detailsTab === 'branches' && (
                  <div className="bitfun-git-panel__list">
                    {loadingDetails ? (
                      <div className="bitfun-git-panel__list-empty">{t('common.loading')}</div>
                    ) : filteredBranches.length > 0 ? (
                      <>
                        {filteredBranches.map((branch, index) => (
                          <div 
                            key={branch.name || index} 
                            className={`bitfun-git-panel__list-item bitfun-git-panel__branch-item ${branch.current ? 'bitfun-git-panel__branch-item--current' : ''}`}
                            onClick={() => {

                              if (!workspacePath) return;
                              

                              window.dispatchEvent(new CustomEvent('expand-right-panel'));
                              

                              setTimeout(() => {
                                createTab({
                                  type: 'git-branch-history',
                                  title: t('tabs.branchCommitHistory', { branch: branch.name }),
                                  mode: 'agent',
                                  data: {
                                    repositoryPath: workspacePath,
                                    branchName: branch.name,
                                    currentBranch: status?.current_branch,
                                    maxCount: 100
                                  },
                                  metadata: {
                                    duplicateCheckKey: `git-branch-history-${workspacePath}-${branch.name}`
                                  },
                                  checkDuplicate: true
                                });
                              }, 250);
                            }}
                            style={{ cursor: 'pointer' }}
                          >
                            <Tooltip content={t('actions.viewBranchHistory')}>
                              <div className="bitfun-git-panel__branch-info">
                                <GitBranch size={14} className="bitfun-git-panel__icon--branch" />
                                <span className="bitfun-git-panel__list-item-text">{branch.name}</span>
                                {branch.current && <span className="bitfun-git-panel__branch-current-badge">{t('branch.current')}</span>}
                              </div>
                            </Tooltip>
                            
                            <div className="bitfun-git-panel__branch-actions">
                              {!branch.current && (
                                <IconButton
                                  className="bitfun-git-panel__branch-action-btn"
                                  variant="ghost"
                                  size="xs"
                                  onClick={(e) => {
                                    e.stopPropagation();
                                    handleSwitchBranch(branch.name);
                                  }}
                                  disabled={isOperating}
                                  tooltip={t('actions.switchBranch')}
                                >
                                  <GitCommit size={14} />
                                </IconButton>
                              )}
                              <IconButton
                                className="bitfun-git-panel__branch-action-btn"
                                variant="ghost"
                                size="xs"
                                onClick={(e) => {
                                  e.stopPropagation();
                                  handleCreateBranchFrom(branch.name);
                                }}
                                disabled={isOperating}
                                tooltip={t('actions.createBranchFrom')}
                              >
                                <Plus size={14} />
                              </IconButton>
                              {!branch.current && (
                                <IconButton
                                  className="bitfun-git-panel__branch-action-btn bitfun-git-panel__branch-action-btn--danger"
                                  variant="danger"
                                  size="xs"
                                  onClick={(e) => {
                                    e.stopPropagation();
                                    handleDeleteBranch(branch.name, branch.current);
                                  }}
                                  disabled={isOperating}
                                  tooltip={t('actions.deleteBranch')}
                                >
                                  <Trash2 size={14} />
                                </IconButton>
                              )}
                            </div>
                          </div>
                        ))}
                      </>
                    ) : (
                      <div className="bitfun-git-panel__list-empty">
                        {searchQueries.branches ? t('empty.noMatchingBranches') : t('empty.noBranches')}
                      </div>
                    )}
                  </div>
                )}
              </div>
            </div>
        </div>

      </div>

      <CreateBranchDialog
        isOpen={showCreateBranchDialog}
        baseBranch={baseBranchForCreation}
        onConfirm={handleCreateBranchConfirm}
        onCancel={handleCreateBranchCancel}
        isCreating={isOperating}
        existingBranches={branches.map(b => b.name)}
      />

      <InputDialog
        isOpen={showCreateBranchFromCommitDialog}
        onClose={() => setShowCreateBranchFromCommitDialog(false)}
        onConfirm={handleConfirmCreateBranchFromCommit}
        title={t('dialog.createBranch.title')}
        description={t('dialog.createBranch.description', { hash: commitHashForBranch.substring(0, 7) })}
        placeholder={t('dialog.createBranch.placeholder')}
        confirmText={t('dialog.createBranch.confirm')}
        cancelText={t('dialog.createBranch.cancel')}
        validator={(value) => {
          if (!/^[a-zA-Z0-9._\-/]+$/.test(value)) {
            return t('validation.branchNameInvalid');
          }
          if (branches.some(b => b.name === value)) {
            return t('validation.branchNameExists');
          }
          return null;
        }}
      />
    </div>
  );
};

export default GitPanel;