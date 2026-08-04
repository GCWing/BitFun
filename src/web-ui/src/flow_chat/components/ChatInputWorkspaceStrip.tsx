/**
 * Workspace label + Git branch (left) and optional usage report control (right).
 */

import React, { useEffect, useMemo, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import { useTranslation } from 'react-i18next';
import {
  Activity,
  Check,
  EyeOff,
  GitBranch,
  RefreshCw,
  Shield,
  ShieldAlert,
  ShieldCheck,
  Square,
  SquareCheck,
} from 'lucide-react';
import { ThreadGoalStripButton } from './thread-goal/ThreadGoalStripButton';
import type { ThreadGoalSnapshot } from '../services/goalService';
import { Tooltip, IconButton } from '@/component-library';
import { useGitState } from '@/tools/git/hooks/useGitState';
import type { SessionExecutionTarget } from '@/infrastructure/api/service-api/WorktreeAPI';
import { getAppearanceOverlayHost } from '@/infrastructure/appearance/runtime/AppearanceOverlayHost';
import { useI18n } from '@/infrastructure/i18n';
import { useAnchoredPopoverPosition } from '@/shared/utils/useAnchoredPopoverPosition';
import { DispatchResultDialog } from '@/features/dispatch/DispatchResultDialog';
import { DispatchTargetPicker } from '@/features/dispatch/DispatchTargetPicker';
import type { DispatchSelection, DispatchTarget } from '@/features/dispatch/types';
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
    disabled?: boolean;
    options?: Array<Exclude<ChatInputPermissionMode, 'acp'>>;
    scopeLabel?: string;
    onChange?: (mode: Exclude<ChatInputPermissionMode, 'acp'>) => void | Promise<void>;
    onHide?: () => void | Promise<void>;
  };
  /** Keep the strip on cached Git state while historical content is still restoring. */
  deferPassiveGitRefresh?: boolean;
  /** Resolved target bound to the active session. */
  executionTarget?: SessionExecutionTarget;
  /**
   * Per-session worktree isolation, rendered next to the branch for Git workspaces.
   * Omitted when the session cannot host a worktree at all (remote, no session).
   */
  worktreeControl?: {
    /** Desired state, including an armed worktree not created until first send. */
    enabled: boolean;
    /** Locked once the session has a transcript — its history describes one directory. */
    locked: boolean;
    /** Why the control is locked, when a transcript is not the reason. */
    lockedReason?: 'dispatch';
    onChange: (enabled: boolean) => void;
  };
  /** Immutable per-session dispatch destination. Hidden on embedded/mini composers. */
  dispatchControl?: {
    target: DispatchTarget;
    sourceWorkspacePath?: string;
    locked: boolean;
    onSelectLocal?: () => void;
    onSelectTarget: (selection: DispatchSelection) => void;
    /** Target worktree can be committed and synced from running onward. */
    syncableJobId?: string;
    branch?: string;
    baselineWorktreePath?: string;
    baselineMissing?: boolean;
  };
}

export type ChatInputPermissionMode = 'ask' | 'auto' | 'full_access' | 'reject' | 'acp';

const NATIVE_PERMISSION_MODES: Array<Exclude<ChatInputPermissionMode, 'acp' | 'reject'>> = [
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
  worktreeControl,
  dispatchControl,
}) => {
  const { t } = useTranslation('flow-chat');
  const { t: tWorktrees } = useI18n('worktrees');
  const { t: tCommon } = useI18n('common');
  const permissionRootRef = useRef<HTMLDivElement>(null);
  const permissionTriggerRef = useRef<HTMLButtonElement>(null);
  const permissionMenuRef = useRef<HTMLDivElement>(null);
  const [permissionMenuOpen, setPermissionMenuOpen] = useState(false);
  const [resultDialogOpen, setResultDialogOpen] = useState(false);
  const permissionMenuLayout = useAnchoredPopoverPosition({
    open: permissionMenuOpen,
    anchorRef: permissionTriggerRef,
    popoverRef: permissionMenuRef,
    preferredPlacement: 'top',
    alignment: 'end',
    gap: 7,
    layoutRevision: `${permissionControl?.options?.length ?? 0}:${Boolean(permissionControl?.onHide)}`,
  });
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

  // Toggling worktree isolation moves the execution root under a live strip.
  // The shared Git cache holds nothing for the new directory, and useGitState
  // only auto-refreshes on mount, so ask for the new branch explicitly.
  const previousRepositoryPathRef = useRef(trimmedPath);
  useEffect(() => {
    if (previousRepositoryPathRef.current === trimmedPath) return;
    previousRepositoryPathRef.current = trimmedPath;
    if (trimmedPath) {
      void refreshBasic();
    }
  }, [refreshBasic, trimmedPath]);

  const showUsage = usageReport?.visible && !!usageReport.onOpen;
  const showGoal = threadGoal?.visible && !!threadGoal.onOpen;
  const showPermission = !!permissionControl;
  const showDispatchResult = !!dispatchControl?.syncableJobId;
  const isWorktree = !!executionTarget?.worktreeId;
  const worktreeEnabled = worktreeControl?.enabled ?? isWorktree;
  const worktreeEnabledRef = useRef(worktreeEnabled);
  worktreeEnabledRef.current = worktreeEnabled;
  // Dispatch delivers work as a Git worktree of the controller's repository, so
  // it is only meaningful where a worktree itself is — the same condition the
  // isolation toggle uses, evaluated from the same Git probe.
  const isGitWorkspace = isRepository || isWorktree || worktreeEnabled;
  const showWorktreeToggle = !!worktreeControl && isGitWorkspace;
  const showDispatchPicker = !!dispatchControl && isGitWorkspace;
  const showRightActions =
    showDispatchPicker || showDispatchResult || showPermission || showUsage || showGoal;
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
    reject: {
      label: t('chatInput.permissionMode.reject.label'),
      description: t('chatInput.permissionMode.reject.description'),
    },
    acp: {
      label: t('chatInput.permissionMode.acp.label'),
      description: t('chatInput.permissionMode.acp.tooltip'),
    },
  } satisfies Record<ChatInputPermissionMode, { label: string; description: string }>;

  useEffect(() => {
    if (!permissionMenuOpen) return;

    const handlePointerDown = (event: PointerEvent) => {
      const target = event.target as Node;
      if (
        !permissionRootRef.current?.contains(target)
        && !permissionMenuRef.current?.contains(target)
      ) {
        setPermissionMenuOpen(false);
      }
    };
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        setPermissionMenuOpen(false);
      }
    };

    document.addEventListener('pointerdown', handlePointerDown);
    document.addEventListener('keydown', handleKeyDown);
    return () => {
      document.removeEventListener('pointerdown', handlePointerDown);
      document.removeEventListener('keydown', handleKeyDown);
    };
  }, [permissionMenuOpen]);

  const dispatchBranch = dispatchControl?.locked
    && worktreeControl?.lockedReason === 'dispatch'
    ? dispatchControl?.branch?.trim()
    : undefined;
  const branchTooltipContent = useMemo(
    () =>
      dispatchBranch
        || (isRepository && currentBranch?.trim()
        ? currentBranch.trim()
        : t('workspaceStrip.branchTooltipUnavailable')),
    [currentBranch, dispatchBranch, isRepository, t],
  );

  if (!label && !showRightActions) {
    return null;
  }

  const branchLabel = dispatchBranch
    || executionTarget?.branch?.trim()
    || (isWorktree && currentBranch?.trim())
    || (isWorktree && executionTarget?.baseCommit
      ? tWorktrees('labels.detached', { commit: executionTarget.baseCommit.slice(0, 9) })
      : isRepository && currentBranch?.trim()
      ? currentBranch.trim()
      : '—');

  const workspaceTooltipContent = trimmedPath || label;
  const worktreeToggleDisabled = !!worktreeControl?.locked;
  let worktreeTooltip = tWorktrees('strip.toggleOffDescription');
  if (worktreeControl?.lockedReason === 'dispatch') {
    worktreeTooltip = tWorktrees('strip.dispatchBaseline');
  } else if (worktreeControl?.locked) {
    worktreeTooltip = tWorktrees('strip.toggleLocked');
  } else if (worktreeEnabled && !isWorktree) {
    worktreeTooltip = tWorktrees('strip.togglePendingOnDescription');
  } else if (!worktreeEnabled && isWorktree) {
    worktreeTooltip = tWorktrees('strip.togglePendingOffDescription');
  } else if (isWorktree) {
    worktreeTooltip = tWorktrees('strip.toggleOnDescription', { path: trimmedPath });
  }
  const permissionMode = permissionControl?.mode ?? 'ask';
  const permissionModes = permissionControl?.options ?? NATIVE_PERMISSION_MODES;
  const permissionDisabled =
    permissionControl?.disabled
    || permissionControl?.saving
    || permissionMode === 'acp';
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

  const handleWorktreeToggle = () => {
    if (!worktreeControl || worktreeToggleDisabled) {
      return;
    }
    const nextEnabled = !worktreeEnabledRef.current;
    worktreeEnabledRef.current = nextEnabled;
    worktreeControl.onChange(nextEnabled);
  };

  const split = !!label && showRightActions;
  const actionsOnly = !label && showRightActions;

  return (
    <div data-bf-component="chat-input-workspace-strip" data-bf-part="root"
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
        <div data-bf-component="chat-input-workspace-strip" data-bf-part="main" className="bitfun-chat-input-workspace-strip__main">
          <Tooltip content={workspaceTooltipContent} placement="top">
            <span className="bitfun-chat-input-workspace-strip__chip bitfun-chat-input-workspace-strip__chip--workspace">
              <span data-bf-component="chat-input-workspace-strip" data-bf-part="workspace" className="bitfun-chat-input-workspace-strip__workspace">{label}</span>
            </span>
          </Tooltip>
          <span className="bitfun-chat-input-workspace-strip__sep" aria-hidden>
            {' / '}
          </span>
          <Tooltip content={branchTooltipContent} placement="top">
            <span className="bitfun-chat-input-workspace-strip__chip bitfun-chat-input-workspace-strip__chip--branch">
              <GitBranch
                className="bitfun-chat-input-workspace-strip__branch-icon"
                size={11}
                strokeWidth={2}
                aria-hidden
              />
              <span data-bf-component="chat-input-workspace-strip" data-bf-part="branch" className="bitfun-chat-input-workspace-strip__branch">{branchLabel}</span>
            </span>
          </Tooltip>
          {showWorktreeToggle ? (
            <Tooltip content={worktreeTooltip} placement="top">
              <button
                type="button"
                role="switch"
                aria-checked={worktreeEnabled}
                aria-label={tWorktrees('strip.toggleLabel')}
                className={[
                  'bitfun-chat-input-workspace-strip__chip',
                  'bitfun-chat-input-workspace-strip__chip--worktree',
                  worktreeEnabled && 'bitfun-chat-input-workspace-strip__chip--worktree-on',
                ]
                  .filter(Boolean)
                  .join(' ')}
                disabled={worktreeToggleDisabled}
                data-testid="chat-input-worktree-toggle"
                data-worktree-enabled={worktreeEnabled ? 'true' : 'false'}
                data-worktree-materialized={isWorktree ? 'true' : 'false'}
                onClick={handleWorktreeToggle}
              >
                {worktreeEnabled ? (
                  <SquareCheck
                    className="bitfun-chat-input-workspace-strip__worktree-icon"
                    size={11}
                    strokeWidth={2}
                    aria-hidden
                  />
                ) : (
                  <Square
                    className="bitfun-chat-input-workspace-strip__worktree-icon"
                    size={11}
                    strokeWidth={2}
                    aria-hidden
                  />
                )}
                <span>{tWorktrees('strip.toggleLabel')}</span>
              </button>
            </Tooltip>
          ) : null}
        </div>
      ) : null}

      {showRightActions ? (
        <div
          className="bitfun-chat-input-workspace-strip__actions"
          data-bf-component="chat-input-workspace-strip"
          data-bf-part="actions"
        >
          {showDispatchPicker && dispatchControl ? (
            <DispatchTargetPicker
              target={dispatchControl.target}
              sourceWorkspacePath={dispatchControl.sourceWorkspacePath}
              locked={dispatchControl.locked}
              onSelectLocal={dispatchControl.onSelectLocal}
              onSelectTarget={dispatchControl.onSelectTarget}
            />
          ) : null}
          {dispatchControl?.syncableJobId ? (
            <>
              <Tooltip content={tCommon('dispatch.syncTitle')} placement="top">
                <button
                  type="button"
                  className="bitfun-chat-input-workspace-strip__dispatch-result"
                  onClick={() => setResultDialogOpen(true)}
                  data-testid="dispatch-sync-trigger"
                >
                  <RefreshCw size={11} strokeWidth={2} aria-hidden />
                  <span>{tCommon('dispatch.syncAction')}</span>
                </button>
              </Tooltip>
              <DispatchResultDialog
                open={resultDialogOpen}
                jobId={dispatchControl.syncableJobId}
                branch={dispatchControl.branch}
                baselineWorktreePath={dispatchControl.baselineWorktreePath}
                baselineMissing={dispatchControl.baselineMissing}
                targetLabel={dispatchControl.target.kind !== 'local'
                  ? dispatchControl.target.displayName
                  : undefined}
                onClose={() => setResultDialogOpen(false)}
              />
            </>
          ) : null}
          {showPermission ? (
            <div
              ref={permissionRootRef}
              data-bf-component="chat-input-workspace-strip"
              data-bf-part="permission"
              className="bitfun-chat-input-workspace-strip__permission"
            >
              <Tooltip content={permissionTooltip} placement="top">
                <button
                  ref={permissionTriggerRef}
                  type="button"
                  className={[
                    'bitfun-chat-input-workspace-strip__permission-trigger',
                    `bitfun-chat-input-workspace-strip__permission-trigger--${permissionMode}`,
                    permissionMenuOpen && 'bitfun-chat-input-workspace-strip__permission-trigger--open',
                  ]
                    .filter(Boolean)
                    .join(' ')}
                  aria-label={permissionTooltip}
                  aria-haspopup={permissionDisabled ? undefined : 'menu'}
                  aria-expanded={permissionDisabled ? undefined : permissionMenuOpen}
                  disabled={permissionDisabled}
                  data-testid="chat-input-permission-trigger"
                  data-permission-mode={permissionMode}
                  onClick={event => {
                    event.stopPropagation();
                    if (!permissionDisabled) {
                      setPermissionMenuOpen(open => !open);
                    }
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

              {permissionMenuOpen && permissionMode !== 'acp' ? createPortal(
                <div
                  ref={permissionMenuRef}
                  data-bf-component="chat-input-workspace-strip"
                  data-bf-part="permissionMenu"
                  data-bf-state="open"
                  data-bf-placement={permissionMenuLayout?.placement ?? 'top'}
                  className="bitfun-chat-input-workspace-strip__permission-menu"
                  style={{
                    top: `${permissionMenuLayout?.top ?? 0}px`,
                    left: `${permissionMenuLayout?.left ?? 0}px`,
                    visibility: permissionMenuLayout ? 'visible' : 'hidden',
                  }}
                  role="menu"
                  aria-label={t('chatInput.permissionMode.menuLabel')}
                  data-testid="chat-input-permission-menu"
                >
                  <div className="bitfun-chat-input-workspace-strip__permission-menu-header">
                    <span>{t('chatInput.permissionMode.menuLabel')}</span>
                    <span>
                      {permissionControl.scopeLabel ?? t('chatInput.permissionMode.globalScope')}
                    </span>
                  </div>
                  <div data-bf-component="chat-input-workspace-strip" data-bf-part="permissionOptions" className="bitfun-chat-input-workspace-strip__permission-options">
                    {permissionModes.map(mode => {
                      const selected = permissionMode === mode;
                      const copy = permissionCopy[mode];
                      return (
                        <button data-bf-component="chat-input-workspace-strip" data-bf-part="permissionOption"
                          data-bf-state={selected ? 'selected' : undefined}
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
                </div>,
                getAppearanceOverlayHost(),
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
                data-bf-component="chat-input-workspace-strip"
                data-bf-part="usageAction"
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
    </div>
  );
};

ChatInputWorkspaceStrip.displayName = 'ChatInputWorkspaceStrip';
