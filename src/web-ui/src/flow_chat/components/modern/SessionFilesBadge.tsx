/**
 * Session file change badge.
 * Shows compact file change stats in FlowChatHeader.
 */

import React, { useState, useCallback, useMemo, useEffect, useRef } from 'react';
import {
  FileEdit,
  FilePlus,
  SearchCheck,
  Sparkles,
  Trash2,
  ChevronDown,
  ChevronUp,
  Zap,
  GitCommitHorizontal,
  GitPullRequest,
} from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { useSnapshotState } from '../../../tools/snapshot_system/hooks/useSnapshotState';
import { createDiffEditorTab } from '../../../shared/utils/tabUtils';
import { snapshotAPI } from '../../../infrastructure/api';
import { useWorkspaceContext } from '../../../infrastructure/contexts/WorkspaceContext';
import { notificationService } from '../../../shared/notification-system';
import { createLogger } from '@/shared/utils/logger';
import { runWithConcurrencyLimit } from '@/shared/utils/runWithConcurrencyLimit';
import { createBtwChildSession } from '../../services/BtwThreadService';
import { openBtwSessionInAuxPane } from '../../services/openBtwSession';
import {
  buildDeepReviewPromptFromSessionFiles,
  launchDeepReviewSession,
} from '../../services/DeepReviewService';
import { insertReviewSessionSummaryMarker } from '../../services/ReviewSessionMarkerService';
import { useDeepReviewConsent } from '../DeepReviewConsentDialog';
import {
  REVIEW_READY_GLINT_DURATION_MS,
  shouldTriggerReviewReadyGlint,
} from './reviewReadyGlint';
import { flowChatStore } from '../../store/FlowChatStore';
import type { DialogTurn } from '../../types/flow-chat';
import { useSessionReviewActivity } from '../../hooks/useSessionReviewActivity';
import { useSessionStateMachine } from '../../hooks/useSessionStateMachine';
import { SessionExecutionState } from '../../state-machine/types';
import { isReviewActivityBlocking } from '../../utils/sessionReviewActivity';
import {
  aiExperienceConfigService,
  DEFAULT_QUICK_ACTIONS,
  type QuickAction,
} from '@/infrastructure/config/services/AIExperienceConfigService';
import { resolveQuickActionText } from '@/infrastructure/config/services/quickActionLocalization';
import './SessionFilesBadge.scss';

const log = createLogger('SessionFilesBadge');

const REVIEW_EXCLUDED_EXTENSIONS = new Set([
  '.7z', '.avi', '.avif', '.bin', '.bmp', '.class', '.dll', '.doc', '.docx', '.eot', '.exe',
  '.gif', '.gz', '.ico', '.jar', '.jpeg', '.jpg', '.lock', '.map', '.min.js', '.min.css',
  '.mov', '.mp3', '.mp4', '.otf', '.pdf', '.png', '.rar', '.so', '.svg', '.tar', '.tgz',
  '.tiff', '.ttf', '.wav', '.webm', '.webp', '.woff', '.woff2', '.xz', '.zip',
]);

const REVIEW_EXCLUDED_FILENAMES = new Set([
  'bun.lock', 'bun.lockb', 'Cargo.lock', 'composer.lock', 'Gemfile.lock', 'package-lock.json',
  'pnpm-lock.yaml', 'poetry.lock', 'Podfile.lock', 'yarn.lock',
]);

const REVIEW_EXCLUDED_PATH_SEGMENTS = new Set([
  '.cache', '.next', '.nuxt', '.output', '.parcel-cache', '.svelte-kit', '.turbo',
  'build', 'coverage', 'dist', 'node_modules', 'out', 'target',
]);

function shouldReviewFile(filePath: string): boolean {
  const normalizedPath = filePath.replace(/\\/g, '/');
  const segments = normalizedPath
    .split('/')
    .map(segment => segment.trim().toLowerCase())
    .filter(Boolean);

  if (segments.some(segment => REVIEW_EXCLUDED_PATH_SEGMENTS.has(segment))) {
    return false;
  }

  const fileName = normalizedPath.split('/').pop()?.trim() || normalizedPath;
  const lowerFileName = fileName.toLowerCase();

  if (REVIEW_EXCLUDED_FILENAMES.has(fileName) || REVIEW_EXCLUDED_FILENAMES.has(lowerFileName)) {
    return false;
  }

  if (lowerFileName.endsWith('.min.js') || lowerFileName.endsWith('.min.css')) {
    return false;
  }

  const extMatch = lowerFileName.match(/(\.[^.]+)$/);
  const extension = extMatch?.[1];

  if (extension && REVIEW_EXCLUDED_EXTENSIONS.has(extension)) {
    return false;
  }

  return true;
}

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

type LatestTurnSnapshot = {
  turnId: string | null;
  status: DialogTurn['status'] | null;
};

function getLatestTurnSnapshot(sessionId?: string): LatestTurnSnapshot {
  if (!sessionId) {
    return { turnId: null, status: null };
  }

  const session = flowChatStore.getState().sessions.get(sessionId);
  const latestTurn = session?.dialogTurns[session.dialogTurns.length - 1];
  return {
    turnId: latestTurn?.id ?? null,
    status: latestTurn?.status ?? null,
  };
}

function isTurnActivelyRunning(status: DialogTurn['status'] | null): boolean {
  return (
    status === 'pending' ||
    status === 'image_analyzing' ||
    status === 'processing' ||
    status === 'finishing' ||
    status === 'cancelling'
  );
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
  const [isReviewMenuOpen, setIsReviewMenuOpen] = useState(false);
  const [showReviewReadyGlint, setShowReviewReadyGlint] = useState(false);
  const [launchingReviewMode, setLaunchingReviewMode] = useState<'review' | 'deep_review' | null>(null);
  const [fileStats, setFileStats] = useState<Map<string, FileStats>>(new Map());
  const [loadingStats, setLoadingStats] = useState(false);
  const reviewActivity = useSessionReviewActivity(sessionId);
  const sessionMachine = useSessionStateMachine(sessionId ?? null);
  const isSessionProcessing =
    sessionMachine?.currentState === SessionExecutionState.PROCESSING ||
    sessionMachine?.currentState === SessionExecutionState.FINISHING;
  const isReviewLaunchOrActivityBlocking =
    launchingReviewMode !== null ||
    isReviewActivityBlocking(reviewActivity);
  /** Includes loadingStats: used for review launches and “review ready” affordances. */
  const isReviewActionLocked =
    loadingStats ||
    isReviewLaunchOrActivityBlocking;
  const [latestTurnSnapshot, setLatestTurnSnapshot] = useState<LatestTurnSnapshot>(() =>
    getLatestTurnSnapshot(sessionId),
  );

  const statsCacheRef = useRef<StatsCache>({});
  const loadingFilesRef = useRef<Set<string>>(new Set());
  const previousSessionIdRef = useRef<string | undefined>(undefined);
  const observedProcessingTurnIdRef = useRef<string | null>(null);
  const promptedReviewReadyTurnIdRef = useRef<string | null>(null);
  const reviewReadyGlintTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const CACHE_TTL = 60000;
  const DIFF_STATS_MAX_CONCURRENCY = 3;

  const [quickActions, setQuickActions] = useState<QuickAction[]>(() => {
    const stored = aiExperienceConfigService.getSettings().quick_actions;
    return (stored && stored.length > 0) ? stored : DEFAULT_QUICK_ACTIONS;
  });

  const badgeRef = useRef<HTMLDivElement>(null);
  const popoverRef = useRef<HTMLDivElement>(null);
  const reviewMenuRef = useRef<HTMLDivElement>(null);
  const { confirmDeepReviewLaunch, deepReviewConsentDialog } = useDeepReviewConsent();

  const clearReviewReadyGlint = useCallback(() => {
    setShowReviewReadyGlint(false);
    if (reviewReadyGlintTimeoutRef.current) {
      clearTimeout(reviewReadyGlintTimeoutRef.current);
      reviewReadyGlintTimeoutRef.current = null;
    }
  }, []);

  // Sync quick actions when settings change.
  useEffect(() => {
    return aiExperienceConfigService.addChangeListener((settings) => {
      const actions = settings.quick_actions;
      setQuickActions((actions && actions.length > 0) ? actions : DEFAULT_QUICK_ACTIONS);
    });
  }, []);

  // Reset cached state when the session changes.
  useEffect(() => {
    if (previousSessionIdRef.current !== sessionId) {
      previousSessionIdRef.current = sessionId;
      statsCacheRef.current = {};
      loadingFilesRef.current.clear();
      setFileStats(new Map());
      setIsExpanded(false);
      setIsReviewMenuOpen(false);
      clearReviewReadyGlint();
      setLaunchingReviewMode(null);
      observedProcessingTurnIdRef.current = null;
      promptedReviewReadyTurnIdRef.current = null;
      setLatestTurnSnapshot(getLatestTurnSnapshot(sessionId));
    }
  }, [clearReviewReadyGlint, sessionId, t]);

  useEffect(() => () => {
    if (reviewReadyGlintTimeoutRef.current) {
      clearTimeout(reviewReadyGlintTimeoutRef.current);
    }
  }, []);

  useEffect(() => {
    const syncLatestTurn = () => {
      const nextSnapshot = getLatestTurnSnapshot(sessionId);
      setLatestTurnSnapshot(prev => (
        prev.turnId === nextSnapshot.turnId && prev.status === nextSnapshot.status
          ? prev
          : nextSnapshot
      ));
    };

    syncLatestTurn();
    const unsubscribe = flowChatStore.subscribe(syncLatestTurn);
    return unsubscribe;
  }, [sessionId]);

  useEffect(() => {
    const { turnId, status } = latestTurnSnapshot;
    if (!turnId) {
      observedProcessingTurnIdRef.current = null;
      promptedReviewReadyTurnIdRef.current = null;
      clearReviewReadyGlint();
      return;
    }

    if (isTurnActivelyRunning(status)) {
      observedProcessingTurnIdRef.current = turnId;
      promptedReviewReadyTurnIdRef.current = null;
      clearReviewReadyGlint();
      setIsReviewMenuOpen(false);
    }
  }, [clearReviewReadyGlint, latestTurnSnapshot]);

  // Close the popovers when clicking outside.
  useEffect(() => {
    if (!isExpanded && !isReviewMenuOpen) return;

    const handleClickOutside = (event: MouseEvent) => {
      const target = event.target as Node;
      const clickedBadge = !!badgeRef.current?.contains(target);
      const clickedFilesPopover = !!popoverRef.current?.contains(target);
      const clickedReviewMenu = !!reviewMenuRef.current?.contains(target);
      if (!clickedBadge && !clickedFilesPopover && !clickedReviewMenu) {
        setIsExpanded(false);
        setIsReviewMenuOpen(false);
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
  }, [isExpanded, isReviewMenuOpen]);

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

      const batchResults = await runWithConcurrencyLimit(
        newFilesToLoad,
        DIFF_STATS_MAX_CONCURRENCY,
        async (file) => {
          let stats: FileStats | null = null;

          try {
            const statsResp = await snapshotAPI.getSessionFileDiffStats(
              sessionId,
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

          return { filePath: file.filePath, stats };
        },
      );

      setFileStats((prev) => {
        const newMap = new Map(prev);
        for (const { filePath, stats } of batchResults) {
          if (stats && (stats.additions > 0 || stats.deletions > 0 || stats.error)) {
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

  useEffect(() => {
    if (fileStats.size === 0) {
      setIsExpanded(false);
    }
  }, [fileStats.size]);

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

  const fileChangeToggleHint = useMemo(() => {
    if (fileStats.size === 0) return '';
    const head = t('sessionFilesBadge.filesSummaryCount', {
      count: fileStats.size,
      defaultValue: '{{count}} files',
    });
    const deltas: string[] = [];
    if (totalStats.totalAdditions > 0) deltas.push(`+${totalStats.totalAdditions}`);
    if (totalStats.totalDeletions > 0) deltas.push(`-${totalStats.totalDeletions}`);
    const cue = t('sessionFilesBadge.expandChangeListCue', { defaultValue: 'Expand list' });
    return deltas.length > 0 ? `${head} · ${deltas.join(' ')} · ${cue}` : `${head} · ${cue}`;
  }, [fileStats.size, totalStats.totalAdditions, totalStats.totalDeletions, t]);

  const fileChangeToggleAriaCollapsed = useMemo(() => {
    if (fileStats.size === 0) return '';
    const head = t('sessionFilesBadge.filesSummaryCount', {
      count: fileStats.size,
      defaultValue: '{{count}} files',
    });
    const deltas: string[] = [];
    if (totalStats.totalAdditions > 0) deltas.push(`+${totalStats.totalAdditions}`);
    if (totalStats.totalDeletions > 0) deltas.push(`-${totalStats.totalDeletions}`);
    const cue = t('sessionFilesBadge.expandChangeListAriaCue', {
      defaultValue: 'Expand to show files',
    });
    return deltas.length > 0 ? `${head}, ${deltas.join(' ')}, ${cue}` : `${head}, ${cue}`;
  }, [fileStats.size, totalStats.totalAdditions, totalStats.totalDeletions, t]);

  const reviewableFileCount = useMemo(() => {
    return Array.from(fileStats.keys()).filter(shouldReviewFile).length;
  }, [fileStats]);
  const reviewActionAvailable =
    !disabled &&
    reviewableFileCount > 0 &&
    !isReviewActionLocked &&
    !isSessionProcessing;

  const areReviewMenuItemsDisabled =
    loadingStats ||
    reviewableFileCount === 0 ||
    isSessionProcessing ||
    isReviewLaunchOrActivityBlocking;

  useEffect(() => {
    if (shouldTriggerReviewReadyGlint({
      currentTurnId: latestTurnSnapshot.turnId,
      currentTurnStatus: latestTurnSnapshot.status,
      observedProcessingTurnId: observedProcessingTurnIdRef.current,
      promptedTurnId: promptedReviewReadyTurnIdRef.current,
      nextReviewableCount: reviewableFileCount,
      loadingStats,
      reviewActionAvailable,
      sessionProcessing: isSessionProcessing,
    })) {
      setShowReviewReadyGlint(true);
      promptedReviewReadyTurnIdRef.current = latestTurnSnapshot.turnId;
      if (reviewReadyGlintTimeoutRef.current) {
        clearTimeout(reviewReadyGlintTimeoutRef.current);
      }
      reviewReadyGlintTimeoutRef.current = setTimeout(() => {
        setShowReviewReadyGlint(false);
        reviewReadyGlintTimeoutRef.current = null;
      }, REVIEW_READY_GLINT_DURATION_MS);
    }
  }, [isSessionProcessing, latestTurnSnapshot, loadingStats, reviewActionAvailable, reviewableFileCount]);

  useEffect(() => {
    if (showReviewReadyGlint && !reviewActionAvailable) {
      clearReviewReadyGlint();
    }
  }, [clearReviewReadyGlint, reviewActionAvailable, showReviewReadyGlint]);

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
    if (!sessionId || fileStats.size === 0 || isReviewActionLocked) return;
    setIsReviewMenuOpen(false);

    const filePaths = Array.from(fileStats.keys());
    const reviewableFilePaths = filePaths.filter(shouldReviewFile);
    const skippedCount = filePaths.length - reviewableFilePaths.length;

    if (reviewableFilePaths.length === 0) {
      notificationService.warning(
        t('sessionFilesBadge.review.noEligibleFiles', {
          defaultValue: 'No reviewable files remain after excluded files were filtered out.',
        }),
        { duration: 3500 }
      );
      return;
    }

    if (skippedCount > 0) {
      notificationService.info(
        t('sessionFilesBadge.review.filteredNotice', {
          included: reviewableFilePaths.length,
          skipped: skippedCount,
          defaultValue:
            'Review will analyze {{included}} files and skip {{skipped}} excluded files such as lock, generated, or binary assets.',
        }),
        { duration: 3500 }
      );
    }

    const fileList = reviewableFilePaths.map(p => `- ${p}`).join('\n');
    const displayMessage = skippedCount > 0
      ? t('sessionFilesBadge.review.displayMessageFiltered', {
          files: fileList,
          skipped: skippedCount,
          defaultValue:
            'Review filtered files:\n{{files}}\n\nSkipped {{skipped}} excluded files.',
        })
      : t('sessionFilesBadge.review.displayMessage', { files: fileList });
    const reviewMessage = skippedCount > 0
      ? t('sessionFilesBadge.review.promptFiltered', {
          files: fileList,
          skipped: skippedCount,
          defaultValue:
            'Please review the following modified files in this session:\n\n{{files}}\n\nDo not review the {{skipped}} skipped files because they matched the excluded lock, generated, or binary file rules.',
        })
      : t('sessionFilesBadge.review.prompt', { files: fileList });

    setLaunchingReviewMode('review');
    try {
      const { FlowChatManager } = await import('../../services/FlowChatManager');
      const flowChatManager = FlowChatManager.getInstance();
      const reviewThreadTitle = t('sessionFilesBadge.review.threadTitle', {
        defaultValue: 'Code review',
      });
      const created = await createBtwChildSession({
        parentSessionId: sessionId,
        workspacePath: currentWorkspace?.rootPath,
        childSessionName: reviewThreadTitle,
        sessionKind: 'review',
        agentType: 'CodeReview',
        enableTools: true,
        safeMode: true,
        autoCompact: true,
        enableContextCompression: true,
        addMarker: false,
      });
      const { childSessionId } = created;
      insertReviewSessionSummaryMarker({
        parentSessionId: sessionId,
        childSessionId,
        kind: 'review',
        title: reviewThreadTitle,
        requestedFiles: reviewableFilePaths,
        parentDialogTurnId: created.parentDialogTurnId,
      });

      openBtwSessionInAuxPane({
        childSessionId,
        parentSessionId: sessionId,
        workspacePath: currentWorkspace?.rootPath,
        expand: true,
      });

      await flowChatManager.sendMessage(
        reviewMessage,
        childSessionId,
        displayMessage
      );

      setIsExpanded(false);
    } catch (error) {
      log.error('Failed to send review request', {
        sessionId,
        fileCount: reviewableFilePaths.length,
        skippedCount,
        error,
      });
    } finally {
      setLaunchingReviewMode(null);
    }
  }, [fileStats, isReviewActionLocked, sessionId, t, currentWorkspace?.rootPath]);

  const handleDeepReviewClick = useCallback(async (e: React.MouseEvent) => {
    e.stopPropagation();
    if (!sessionId || fileStats.size === 0 || isReviewActionLocked) return;
    setIsReviewMenuOpen(false);

    const filePaths = Array.from(fileStats.keys());
    const reviewableFilePaths = filePaths.filter(shouldReviewFile);
    const skippedCount = filePaths.length - reviewableFilePaths.length;

    if (reviewableFilePaths.length === 0) {
      notificationService.warning(
        t('sessionFilesBadge.review.noEligibleFiles', {
          defaultValue: 'No reviewable files remain after excluded files were filtered out.',
        }),
        { duration: 3500 }
      );
      return;
    }

    const confirmed = await confirmDeepReviewLaunch();
    if (!confirmed) {
      return;
    }
    setLaunchingReviewMode('deep_review');

    if (skippedCount > 0) {
      notificationService.info(
        t('sessionFilesBadge.review.filteredNotice', {
          included: reviewableFilePaths.length,
          skipped: skippedCount,
          defaultValue:
            'Review will analyze {{included}} files and skip {{skipped}} excluded files such as lock, generated, or binary assets.',
        }),
        { duration: 3500 }
      );
    }

    const fileList = reviewableFilePaths.map(p => `- ${p}`).join('\n');
    const displayMessage = skippedCount > 0
      ? t('sessionFilesBadge.deepReview.displayMessageFiltered', {
          files: fileList,
          skipped: skippedCount,
          defaultValue:
            'Deep review filtered files:\n{{files}}\n\nSkipped {{skipped}} excluded files.',
        })
      : t('sessionFilesBadge.deepReview.displayMessage', {
          files: fileList,
          defaultValue: 'Deep review modified files:\n{{files}}',
        });

    try {
      const prompt = await buildDeepReviewPromptFromSessionFiles(
        reviewableFilePaths,
        undefined,
        currentWorkspace?.rootPath,
      );

      await launchDeepReviewSession({
        parentSessionId: sessionId,
        workspacePath: currentWorkspace?.rootPath,
        prompt,
        displayMessage,
        childSessionName: t('sessionFilesBadge.deepReview.threadTitle', {
          defaultValue: 'Deep review',
        }),
        requestedFiles: reviewableFilePaths,
      });

      setIsExpanded(false);
    } catch (error) {
      log.error('Failed to send deep review request', {
        sessionId,
        fileCount: reviewableFilePaths.length,
        skippedCount,
        error,
      });
    } finally {
      setLaunchingReviewMode(null);
    }
  }, [confirmDeepReviewLaunch, fileStats, isReviewActionLocked, sessionId, t, currentWorkspace?.rootPath]);

  const handleQuickActionClick = useCallback(async (action: QuickAction) => {
    if (!sessionId || isSessionProcessing) return;
    setIsReviewMenuOpen(false);
    const actionText = resolveQuickActionText(action, t);
    try {
      const { FlowChatManager } = await import('../../services/FlowChatManager');
      await FlowChatManager.getInstance().sendMessage(
        actionText.prompt,
        sessionId,
        actionText.label,
      );
    } catch (error) {
      log.error('Failed to trigger quick action', { actionId: action.id, error });
    }
  }, [sessionId, isSessionProcessing, t]);

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

  const activeReviewMode = launchingReviewMode ?? (reviewActivity?.isBlocking ? reviewActivity.kind : null) ?? null;
  const reviewButtonTitle = activeReviewMode
    ? t('sessionFilesBadge.reviewRunningHint', {
        defaultValue: 'Wait for the current review to finish or stop it from the review page.',
      })
    : t('sessionFilesBadge.actionsMenuHint', { defaultValue: 'Quick actions' });

  // Hide when there is no session or parent disabled. Actions menu (reviews + quick actions)
  // renders first; file-change summary appears after we have stats.
  if (!sessionId || disabled) {
    return null;
  }

  const showFileStatsSummary = fileStats.size > 0;

  return (
    <>
      <div
        ref={badgeRef}
        className={`session-files-badge ${isExpanded ? 'session-files-badge--expanded' : ''}`}
      >
      <div
        ref={reviewMenuRef}
        className="session-files-badge__review-menu"
      >
        <button
          className={[
            'session-files-badge__review-btn',
            showReviewReadyGlint && 'session-files-badge__review-btn--glint',
            activeReviewMode && 'session-files-badge__review-btn--running',
          ].filter(Boolean).join(' ')}
          onClick={(event) => {
            event.stopPropagation();
            if (isReviewLaunchOrActivityBlocking) return;
            setIsReviewMenuOpen((open) => {
              const next = !open;
              if (next) setIsExpanded(false);
              return next;
            });
          }}
          disabled={isReviewLaunchOrActivityBlocking}
          title={reviewButtonTitle}
          type="button"
          aria-label={reviewButtonTitle}
          aria-haspopup="menu"
          aria-expanded={isReviewMenuOpen && !isReviewLaunchOrActivityBlocking}
          aria-busy={Boolean(activeReviewMode)}
        >
          <span className="session-files-badge__review-actions-label">
            {activeReviewMode
              ? t('sessionFilesBadge.actionsButtonRunning', { defaultValue: 'Busy…' })
              : t('sessionFilesBadge.actionsButton', { defaultValue: 'Actions' })}
          </span>
          {!activeReviewMode ? (
            <ChevronDown
              size={12}
              className={[
                'session-files-badge__review-menu-chevron',
                isReviewMenuOpen && !isReviewLaunchOrActivityBlocking && 'session-files-badge__review-menu-chevron--open',
              ].filter(Boolean).join(' ')}
              aria-hidden
            />
          ) : null}
        </button>

        {isReviewMenuOpen && !isReviewLaunchOrActivityBlocking && (
          <div className="session-files-badge__review-menu-popover" role="menu">
            <button
              className="session-files-badge__review-menu-item"
              onClick={handleReviewClick}
              type="button"
              role="menuitem"
              disabled={areReviewMenuItemsDisabled}
            >
              <SearchCheck size={12} className="session-files-badge__review-icon session-files-badge__review-icon--standard" />
              <span>{t('sessionFilesBadge.reviewModeStandard')}</span>
            </button>
            <button
              className="session-files-badge__review-menu-item session-files-badge__review-menu-item--deep"
              onClick={handleDeepReviewClick}
              type="button"
              role="menuitem"
              disabled={areReviewMenuItemsDisabled}
            >
              <Sparkles size={12} className="session-files-badge__review-icon" />
              <span>{t('sessionFilesBadge.reviewModeDeep')}</span>
            </button>

            {quickActions.filter(a => a.enabled).length > 0 && (
              <div className="session-files-badge__review-menu-separator" role="separator" />
            )}

            {quickActions.filter(a => a.enabled).map(action => {
              const actionText = resolveQuickActionText(action, t);
              return (
                <button
                  key={action.id}
                  className="session-files-badge__review-menu-item"
                  onClick={() => { void handleQuickActionClick(action); }}
                  type="button"
                  role="menuitem"
                  disabled={isSessionProcessing}
                >
                  {action.id === 'commit' ? (
                    <GitCommitHorizontal size={12} className="session-files-badge__review-icon" />
                  ) : action.id === 'create_pr' ? (
                    <GitPullRequest size={12} className="session-files-badge__review-icon" />
                  ) : (
                    <Zap size={12} className="session-files-badge__review-icon" />
                  )}
                  <span>{actionText.label}</span>
                </button>
              );
            })}
          </div>
        )}
      </div>

      {showFileStatsSummary ? (
      <button
        className="session-files-badge__button"
        onClick={() => {
          setIsExpanded((prev) => {
            const next = !prev;
            if (next) setIsReviewMenuOpen(false);
            return next;
          });
        }}
        disabled={loadingStats}
        type="button"
        title={fileChangeToggleHint}
        aria-label={
          isExpanded
            ? t('sessionFilesBadge.collapseFileDiffList', { defaultValue: 'Collapse file change list' })
            : fileChangeToggleAriaCollapsed
        }
        aria-expanded={isExpanded}
      >
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
      ) : null}

      {showFileStatsSummary && isExpanded && (
        <div
          ref={popoverRef}
          className="session-files-badge__popover"
        >
          <div className="session-files-badge__popover-summary">
            <span className="session-files-badge__popover-summary-count">
              {t('sessionFilesBadge.filesSummaryCount', {
                count: fileStats.size,
                defaultValue: '{{count}} files',
              })}
            </span>
            {(totalStats.totalAdditions > 0 || totalStats.totalDeletions > 0) && (
              <span className="session-files-badge__popover-summary-stats">
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
              </span>
            )}
          </div>
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
      {deepReviewConsentDialog}
    </>
  );
};

SessionFilesBadge.displayName = 'SessionFilesBadge';
