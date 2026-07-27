/**
 * Workspace label + Git branch (left) and optional usage report control (right).
 */

import React, { useEffect, useMemo, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import {
  Activity,
  Archive,
  Check,
  EyeOff,
  FolderOpen,
  GitBranch,
  Settings2,
  Shield,
  ShieldAlert,
  ShieldCheck,
} from 'lucide-react';
import { ThreadGoalStripButton } from './thread-goal/ThreadGoalStripButton';
import type { ThreadGoalSnapshot } from '../services/goalService';
import { Tooltip, IconButton, InputDialog } from '@/component-library';
import { useGitState } from '@/tools/git/hooks/useGitState';
import { configAPI, workspaceAPI, worktreeAPI } from '@/infrastructure/api';
import type {
  SessionExecutionTarget,
  WorktreeSummary,
} from '@/infrastructure/api/service-api/WorktreeAPI';
import { useI18n } from '@/infrastructure/i18n';
import { notificationService } from '@/shared/notification-system';
import { openWorktreeManager } from '@/shared/services/worktreeUIEvents';
import { isSamePath } from '@/shared/utils/pathUtils';
import './ChatInputWorkspaceStrip.scss';

export interface ChatInputWorkspaceStripProps {
  /** Repo root for git status; may come from session when global workspace is unset. */
  repositoryPath: string;
  /** Resolved display name (workspace title or folder basename). */
  workspaceLabel: string;
  /** Session usage report (/usage) — icon on the right when visible. */
  usageReport?: {
    visible: boolean;
    onOpen: () => void;
  };
  /** Thread goal menu (/goal) — icon on the right when visible. */
  threadGoal?: {
    visible: boolean;
    goal: ThreadGoalSnapshot | null;
    onOpen: () => void;
  };
  /** Global native-tool permission mode exposed as a compact strip control. */
  permissionControl?: {
    mode: ChatInputPermissionMode;
    saving?: boolean;
    onChange?: (mode: Exclude<ChatInputPermissionMode, 'acp'>) => void | Promise<void>;
    onHide?: () => void | Promise<void>;
  };
  /** Keep the strip on cached Git state while historical content is still restoring. */
  deferPassiveGitRefresh?: boolean;
  /** Resolved target bound to the active session. */
  executionTarget?: SessionExecutionTarget;
  /** Main project that owns the active worktree session. */
  projectWorkspacePath?: string;
}

export type ChatInputPermissionMode = 'ask' | 'auto' | 'full_access' | 'acp';

const NATIVE_PERMISSION_MODES: Array<Exclude<ChatInputPermissionMode, 'acp'>> = [
  'ask',
  'auto',
  'full_access',
];

export const ChatInputWorkspaceStrip: React.FC<ChatInputWorkspaceStripProps> = ({
  repositoryPath,
  workspaceLabel,
  usageReport,
  threadGoal,
  permissionControl,
  deferPassiveGitRefresh = false,
  executionTarget,
  projectWorkspacePath,
}) => {
  const { t } = useTranslation('flow-chat');
  const { t: tWorktrees } = useI18n('worktrees');
  const permissionRootRef = useRef<HTMLDivElement>(null);
  const worktreeRootRef = useRef<HTMLDivElement>(null);
  const [permissionMenuOpen, setPermissionMenuOpen] = useState(false);
  const [worktreeMenuOpen, setWorktreeMenuOpen] = useState(false);
  const [branchDialogOpen, setBranchDialogOpen] = useState(false);
  const [branchPrefix, setBranchPrefix] = useState('bitfun/');
  const [worktreeMutationPending, setWorktreeMutationPending] = useState(false);
  const [liveWorktree, setLiveWorktree] = useState<WorktreeSummary | null>(null);
  const trimmedPath = repositoryPath.trim();
  const label = workspaceLabel.trim();

  const { currentBranch, isRepository, refreshBasic } = useGitState({
    repositoryPath: trimmedPath,
    layers: ['basic'],
    isActive: !deferPassiveGitRefresh,
    refreshOnMount: !deferPassiveGitRefresh,
    refreshOnActive: false,
    debugSource: 'chat_input_workspace_strip',
  });

  const showUsage = usageReport?.visible && !!usageReport.onOpen;
  const showGoal = threadGoal?.visible && !!threadGoal.onOpen;
  const showPermission = !!permissionControl;
  const showRightActions = showPermission || showUsage || showGoal;
  const isWorktree = !!executionTarget?.worktreeId;
  const effectiveWorktreePath = liveWorktree?.path || executionTarget?.rootPath || trimmedPath;
  const effectiveWorktreeLifecycle =
    liveWorktree?.lifecycle || executionTarget?.lifecycle;
  const permissionCopy = {
    ask: {
      label: t('chatInput.permissionMode.ask.label'),
      description: t('chatInput.permissionMode.ask.description'),
    },
    auto: {
      label: t('chatInput.permissionMode.auto.label'),
      description: t('chatInput.permissionMode.auto.description'),
    },
    full_access: {
      label: t('chatInput.permissionMode.fullAccess.label'),
      description: t('chatInput.permissionMode.fullAccess.description'),
    },
    acp: {
      label: t('chatInput.permissionMode.acp.label'),
      description: t('chatInput.permissionMode.acp.tooltip'),
    },
  } satisfies Record<ChatInputPermissionMode, { label: string; description: string }>;

  useEffect(() => {
    if (!permissionMenuOpen && !worktreeMenuOpen) return;

    const handlePointerDown = (event: PointerEvent) => {
      if (!permissionRootRef.current?.contains(event.target as Node)) {
        setPermissionMenuOpen(false);
      }
      if (!worktreeRootRef.current?.contains(event.target as Node)) {
        setWorktreeMenuOpen(false);
      }
    };
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        setPermissionMenuOpen(false);
        setWorktreeMenuOpen(false);
      }
    };

    document.addEventListener('pointerdown', handlePointerDown);
    document.addEventListener('keydown', handleKeyDown);
    return () => {
      document.removeEventListener('pointerdown', handlePointerDown);
      document.removeEventListener('keydown', handleKeyDown);
    };
  }, [permissionMenuOpen, worktreeMenuOpen]);

  useEffect(() => {
    if (!worktreeMenuOpen) return;
    void configAPI
      .getConfig('app.worktrees', { skipRetryOnNotFound: true })
      .then(value => {
        if (value && typeof value.branchPrefix === 'string') {
          setBranchPrefix(value.branchPrefix);
        }
      })
      .catch(() => undefined);
  }, [worktreeMenuOpen]);

  useEffect(() => {
    const worktreeId = executionTarget?.worktreeId;
    if (!worktreeId || !projectWorkspacePath) {
      setLiveWorktree(null);
      return;
    }
    let cancelled = false;
    const refreshWorktree = async () => {
      try {
        const worktrees = await worktreeAPI.list(projectWorkspacePath);
        if (!cancelled) {
          setLiveWorktree(
            worktrees.find(worktree => worktree.worktreeId === worktreeId) ?? null,
          );
          await refreshBasic();
        }
      } catch {
        if (!cancelled) setLiveWorktree(null);
      }
    };
    void refreshWorktree();
    const unsubscribe = worktreeAPI.onChanged(event => {
      if (
        !event.projectWorkspacePath
        || isSamePath(event.projectWorkspacePath, projectWorkspacePath)
      ) {
        void refreshWorktree();
      }
    });
    return () => {
      cancelled = true;
      unsubscribe();
    };
  }, [executionTarget?.worktreeId, projectWorkspacePath, refreshBasic]);

  const branchTooltipContent = useMemo(
    () =>
      isRepository && currentBranch?.trim()
        ? currentBranch.trim()
        : t('workspaceStrip.branchTooltipUnavailable'),
    [currentBranch, isRepository, t],
  );

  if (!label && !showRightActions) {
    return null;
  }

  const branchLabel = liveWorktree?.branch?.trim()
    || executionTarget?.branch?.trim()
    || (isWorktree && currentBranch?.trim())
    || (isWorktree && executionTarget?.baseCommit
      ? tWorktrees('labels.detached', { commit: executionTarget.baseCommit.slice(0, 9) })
      : isRepository && currentBranch?.trim()
      ? currentBranch.trim()
      : '—');

  const workspaceTooltipContent = trimmedPath || label;
  const permissionMode = permissionControl?.mode ?? 'ask';
  const permissionModeLabel = permissionCopy[permissionMode].label;
  const permissionTooltip = permissionMode === 'acp'
    ? t('chatInput.permissionMode.acp.tooltip')
    : t('chatInput.permissionMode.current', { mode: permissionModeLabel });
  const PermissionIcon = permissionMode === 'auto'
    ? ShieldCheck
    : permissionMode === 'full_access'
      ? ShieldAlert
      : Shield;
  const showPermissionLabel = permissionMode !== 'acp';

  const split = !!label && showRightActions;
  const actionsOnly = !label && showRightActions;

  return (
    <div
      className={[
        'bitfun-chat-input-workspace-strip',
        split && 'bitfun-chat-input-workspace-strip--split',
        actionsOnly && 'bitfun-chat-input-workspace-strip--actions-only',
      ]
        .filter(Boolean)
        .join(' ')}
      data-testid="chat-input-workspace-strip"
    >
      {label ? (
        <div className="bitfun-chat-input-workspace-strip__main">
          <Tooltip content={workspaceTooltipContent} placement="top">
            <span className="bitfun-chat-input-workspace-strip__chip bitfun-chat-input-workspace-strip__chip--workspace">
              <span className="bitfun-chat-input-workspace-strip__workspace">{label}</span>
            </span>
          </Tooltip>
          <span className="bitfun-chat-input-workspace-strip__sep" aria-hidden>
            {' / '}
          </span>
          <Tooltip
            content={isWorktree ? effectiveWorktreePath : branchTooltipContent}
            placement="top"
          >
            <div
              ref={worktreeRootRef}
              className="bitfun-chat-input-workspace-strip__worktree"
            >
            <button
              type="button"
              className="bitfun-chat-input-workspace-strip__chip bitfun-chat-input-workspace-strip__chip--branch"
              onClick={() => isWorktree && setWorktreeMenuOpen(open => !open)}
              aria-haspopup={isWorktree ? 'menu' : undefined}
              aria-expanded={isWorktree ? worktreeMenuOpen : undefined}
              disabled={!isWorktree}
            >
              <GitBranch
                className="bitfun-chat-input-workspace-strip__branch-icon"
                size={11}
                strokeWidth={2}
                aria-hidden
              />
              <span className="bitfun-chat-input-workspace-strip__branch">{branchLabel}</span>
            </button>
            {isWorktree && worktreeMenuOpen ? (
              <div
                className="bitfun-chat-input-workspace-strip__worktree-menu"
                role="menu"
                aria-label={tWorktrees('strip.menuLabel')}
              >
                <button
                  type="button"
                  role="menuitem"
                  onClick={() => {
                    setWorktreeMenuOpen(false);
                    void workspaceAPI.revealInExplorer(effectiveWorktreePath);
                  }}
                >
                  <FolderOpen size={13} aria-hidden />
                  {tWorktrees('strip.open')}
                </button>
                {!liveWorktree?.branch && !executionTarget.branch ? (
                  <button
                    type="button"
                    role="menuitem"
                    onClick={() => {
                      setWorktreeMenuOpen(false);
                      setBranchDialogOpen(true);
                    }}
                  >
                    <GitBranch size={13} aria-hidden />
                    {tWorktrees('manager.createBranch')}
                  </button>
                ) : null}
                {effectiveWorktreeLifecycle === 'managed' ? (
                  <button
                    type="button"
                    role="menuitem"
                    disabled={worktreeMutationPending}
                    onClick={() => {
                      if (!projectWorkspacePath || !executionTarget.worktreeId) return;
                      setWorktreeMutationPending(true);
                      void worktreeAPI
                        .promote(
                          projectWorkspacePath,
                          executionTarget.worktreeId,
                          globalThis.crypto?.randomUUID?.() ?? `worktree-${Date.now()}`,
                        )
                        .then(() => notificationService.success(tWorktrees('manager.promoted')))
                        .catch(error => notificationService.error(
                          error instanceof Error ? error.message : String(error),
                        ))
                        .finally(() => {
                          setWorktreeMutationPending(false);
                          setWorktreeMenuOpen(false);
                        });
                    }}
                  >
                    <Archive size={13} aria-hidden />
                    {tWorktrees('manager.keep')}
                  </button>
                ) : null}
                <button
                  type="button"
                  role="menuitem"
                  onClick={() => {
                    setWorktreeMenuOpen(false);
                    if (projectWorkspacePath) {
                      openWorktreeManager(projectWorkspacePath);
                    }
                  }}
                >
                  <Settings2 size={13} aria-hidden />
                  {tWorktrees('manager.title')}
                </button>
              </div>
            ) : null}
            </div>
          </Tooltip>
        </div>
      ) : null}

      {showRightActions ? (
        <div className="bitfun-chat-input-workspace-strip__actions">
          {showPermission ? (
            <div
              ref={permissionRootRef}
              className="bitfun-chat-input-workspace-strip__permission"
            >
              <Tooltip content={permissionTooltip} placement="top">
                <button
                  type="button"
                  className={[
                    'bitfun-chat-input-workspace-strip__permission-trigger',
                    `bitfun-chat-input-workspace-strip__permission-trigger--${permissionMode}`,
                    permissionMenuOpen && 'bitfun-chat-input-workspace-strip__permission-trigger--open',
                  ]
                    .filter(Boolean)
                    .join(' ')}
                  aria-label={permissionTooltip}
                  aria-haspopup={permissionMode === 'acp' ? undefined : 'menu'}
                  aria-expanded={permissionMode === 'acp' ? undefined : permissionMenuOpen}
                  disabled={permissionControl.saving || permissionMode === 'acp'}
                  data-testid="chat-input-permission-trigger"
                  data-permission-mode={permissionMode}
                  onClick={event => {
                    event.stopPropagation();
                    setPermissionMenuOpen(open => !open);
                  }}
                >
                  <PermissionIcon size={12} strokeWidth={2} aria-hidden />
                  {showPermissionLabel ? (
                    <span className="bitfun-chat-input-workspace-strip__permission-label">
                      {permissionModeLabel}
                    </span>
                  ) : null}
                </button>
              </Tooltip>

              {permissionMenuOpen && permissionMode !== 'acp' ? (
                <div
                  className="bitfun-chat-input-workspace-strip__permission-menu"
                  role="menu"
                  aria-label={t('chatInput.permissionMode.menuLabel')}
                  data-testid="chat-input-permission-menu"
                >
                  <div className="bitfun-chat-input-workspace-strip__permission-menu-header">
                    <span>{t('chatInput.permissionMode.menuLabel')}</span>
                    <span>{t('chatInput.permissionMode.globalScope')}</span>
                  </div>
                  <div className="bitfun-chat-input-workspace-strip__permission-options">
                    {NATIVE_PERMISSION_MODES.map(mode => {
                      const selected = permissionMode === mode;
                      const copy = permissionCopy[mode];
                      return (
                        <button
                          key={mode}
                          type="button"
                          role="menuitemradio"
                          aria-checked={selected}
                          className={[
                            'bitfun-chat-input-workspace-strip__permission-option',
                            selected && 'bitfun-chat-input-workspace-strip__permission-option--selected',
                          ]
                            .filter(Boolean)
                            .join(' ')}
                          disabled={permissionControl.saving}
                          data-testid={`chat-input-permission-option-${mode}`}
                          onClick={event => {
                            event.stopPropagation();
                            setPermissionMenuOpen(false);
                            if (!selected) {
                              void permissionControl.onChange?.(mode);
                            }
                          }}
                        >
                          <span className="bitfun-chat-input-workspace-strip__permission-option-copy">
                            <span className="bitfun-chat-input-workspace-strip__permission-option-label">
                              {copy.label}
                            </span>
                            <span className="bitfun-chat-input-workspace-strip__permission-option-description">
                              {copy.description}
                            </span>
                          </span>
                          {selected ? <Check size={14} strokeWidth={2.2} aria-hidden /> : null}
                        </button>
                      );
                    })}
                  </div>
                  {permissionControl.onHide ? (
                    <>
                      <div className="bitfun-chat-input-workspace-strip__permission-menu-divider" role="separator" />
                      <button
                        type="button"
                        role="menuitem"
                        className="bitfun-chat-input-workspace-strip__permission-visibility-action"
                        data-testid="chat-input-permission-hide-control"
                        onClick={event => {
                          event.stopPropagation();
                          setPermissionMenuOpen(false);
                          void permissionControl.onHide?.();
                        }}
                      >
                        <EyeOff size={14} strokeWidth={2} aria-hidden />
                        <span>{t('chatInput.permissionMode.hideControl')}</span>
                      </button>
                    </>
                  ) : null}
                </div>
              ) : null}
            </div>
          ) : null}
          {showGoal ? (
            <ThreadGoalStripButton
              goal={threadGoal.goal}
              onOpen={threadGoal.onOpen}
            />
          ) : null}
          {showUsage ? (
            <Tooltip content={t('usage.runtime.tooltip')}>
              <IconButton
                className="bitfun-chat-input-workspace-strip__usage-btn"
                variant="ghost"
                size="xs"
                type="button"
                aria-label={t('usage.runtime.open')}
                onClick={e => {
                  e.stopPropagation();
                  usageReport.onOpen();
                }}
              >
                <Activity size={14} strokeWidth={2} aria-hidden />
              </IconButton>
            </Tooltip>
          ) : null}
        </div>
      ) : null}
      <InputDialog
        isOpen={branchDialogOpen}
        onClose={() => setBranchDialogOpen(false)}
        onConfirm={branch => {
          if (!projectWorkspacePath || !executionTarget?.worktreeId) return;
          setWorktreeMutationPending(true);
          void worktreeAPI
            .createBranch(
              projectWorkspacePath,
              executionTarget.worktreeId,
              branch,
              globalThis.crypto?.randomUUID?.() ?? `worktree-${Date.now()}`,
            )
            .then(() => notificationService.success(tWorktrees('manager.branchCreated')))
            .catch(error => notificationService.error(
              error instanceof Error ? error.message : String(error),
            ))
            .finally(() => setWorktreeMutationPending(false));
        }}
        title={tWorktrees('manager.branchDialog.title')}
        description={tWorktrees('manager.branchDialog.description')}
        defaultValue={`${branchPrefix}${executionTarget?.worktreeId?.slice(0, 8) ?? ''}`}
        confirmText={tWorktrees('manager.createBranch')}
      />
    </div>
  );
};

ChatInputWorkspaceStrip.displayName = 'ChatInputWorkspaceStrip';
