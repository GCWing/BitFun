import React, { useCallback, useEffect, useMemo, useState } from 'react';
import { ChevronDown, GitBranch, Loader2, MoreHorizontal, Plus } from 'lucide-react';
import { Tooltip } from '@/component-library';
import { agentAPI, worktreeAPI } from '@/infrastructure/api';
import type { WorktreeSummary } from '@/infrastructure/api/service-api/WorktreeAPI';
import { useI18n } from '@/infrastructure/i18n';
import { flowChatManager } from '@/flow_chat/services/FlowChatManager';
import { openMainSession } from '@/flow_chat/services/sessionActivation';
import { notificationService } from '@/shared/notification-system';
import {
  OPEN_WORKTREE_LAUNCHER_EVENT,
  OPEN_WORKTREE_MANAGER_EVENT,
  type WorktreeLauncherMode,
} from '@/shared/services/worktreeUIEvents';
import { isSamePath } from '@/shared/utils/pathUtils';
import SessionsSection from '../sessions/SessionsSection';
import WorktreeLauncherModal, {
  type WorktreeLauncherSubmit,
} from './WorktreeLauncherModal';
import WorktreeManagerModal from './WorktreeManagerModal';
import './ProjectWorktrees.scss';

interface ProjectWorktreesProps {
  projectWorkspacePath: string;
  projectWorkspaceId: string;
  projectName: string;
  enabled: boolean;
  remote?: boolean;
  isActiveWorkspace: boolean;
  isVisible: boolean;
  onActivateProject: (workspaceId: string) => Promise<unknown>;
}

export const ProjectWorktrees: React.FC<ProjectWorktreesProps> = ({
  projectWorkspacePath,
  projectWorkspaceId,
  projectName,
  enabled,
  remote = false,
  isActiveWorkspace,
  isVisible,
  onActivateProject,
}) => {
  const { t } = useI18n('worktrees');
  const [worktrees, setWorktrees] = useState<WorktreeSummary[]>([]);
  const [loading, setLoading] = useState(false);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [launcherOpen, setLauncherOpen] = useState(false);
  const [launcherMode, setLauncherMode] = useState<WorktreeLauncherMode>('agentic');
  const [managerOpen, setManagerOpen] = useState(false);
  const [collapsed, setCollapsed] = useState<Set<string>>(() => new Set());

  const refresh = useCallback(async () => {
    if (!enabled) {
      setWorktrees([]);
      return;
    }
    setLoading(true);
    setLoadError(null);
    try {
      setWorktrees(await worktreeAPI.list(projectWorkspacePath));
    } catch (error) {
      setLoadError(error instanceof Error ? error.message : String(error));
    } finally {
      setLoading(false);
    }
  }, [enabled, projectWorkspacePath]);

  useEffect(() => {
    if (!enabled) return;
    void refresh();
    const unsubscribeWorktrees = worktreeAPI.onChanged(event => {
      if (
        !event.projectWorkspacePath
        || isSamePath(event.projectWorkspacePath, projectWorkspacePath)
      ) {
        void refresh();
      }
    });
    const unsubscribeCreated = agentAPI.onSessionCreated(() => {
      void refresh();
    });
    const unsubscribeState = agentAPI.onSessionStateChanged(() => {
      void refresh();
    });
    return () => {
      unsubscribeWorktrees();
      unsubscribeCreated();
      unsubscribeState();
    };
  }, [enabled, projectWorkspacePath, refresh]);

  useEffect(() => {
    const handleOpenManager = (event: Event) => {
      const detail = (event as CustomEvent<{ projectWorkspacePath?: string }>).detail;
      if (
        detail?.projectWorkspacePath
        && isSamePath(detail.projectWorkspacePath, projectWorkspacePath)
      ) {
        setManagerOpen(true);
      }
    };
    window.addEventListener(OPEN_WORKTREE_MANAGER_EVENT, handleOpenManager);
    return () => window.removeEventListener(OPEN_WORKTREE_MANAGER_EVENT, handleOpenManager);
  }, [projectWorkspacePath]);

  useEffect(() => {
    const handleOpenLauncher = (event: Event) => {
      const detail = (event as CustomEvent<{
        projectWorkspacePath?: string;
        mode?: WorktreeLauncherMode;
      }>).detail;
      if (
        detail?.projectWorkspacePath
        && isSamePath(detail.projectWorkspacePath, projectWorkspacePath)
      ) {
        setLauncherMode(detail.mode ?? 'agentic');
        setLauncherOpen(true);
      }
    };
    window.addEventListener(OPEN_WORKTREE_LAUNCHER_EVENT, handleOpenLauncher);
    return () => window.removeEventListener(OPEN_WORKTREE_LAUNCHER_EVENT, handleOpenLauncher);
  }, [projectWorkspacePath]);

  const managedWorktrees = useMemo(
    () => worktrees.filter(worktree => !worktree.isMain),
    [worktrees],
  );
  const lifecycleLabel = (worktree: WorktreeSummary): string => {
    if (worktree.lifecycle === 'permanent') return t('labels.lifecycle.permanent');
    if (worktree.lifecycle === 'external') return t('labels.lifecycle.external');
    return t('labels.lifecycle.managed');
  };

  const createSession = useCallback(async (
    target: WorktreeSummary,
    mode: WorktreeLauncherMode = 'agentic',
  ) => {
    if (target.missing) {
      setManagerOpen(true);
      return;
    }
    try {
      const sessionId = await flowChatManager.createChatSession(
        {
          workspacePath: target.path,
          projectWorkspacePath,
          executionTargetRequest: {
            kind: 'existingWorktree',
            worktreeId: target.worktreeId,
          },
        },
        mode,
      );
      await openMainSession(sessionId, {
        workspaceId: projectWorkspaceId,
        activateWorkspace: onActivateProject,
      });
    } catch (error) {
      notificationService.error(
        error instanceof Error ? error.message : String(error),
        { duration: 4500 },
      );
    }
  }, [onActivateProject, projectWorkspaceId, projectWorkspacePath]);

  const createManagedSession = useCallback(async (request: WorktreeLauncherSubmit) => {
    const sessionId = await flowChatManager.createChatSession(
      {
        workspacePath: projectWorkspacePath,
        projectWorkspacePath,
        executionTargetRequest: {
          kind: 'newManagedWorktree',
          baseRef: request.baseRef,
          copyLocalChanges: request.copyLocalChanges,
        },
      },
      request.mode,
    );
    await openMainSession(sessionId, {
      workspaceId: projectWorkspaceId,
      activateWorkspace: onActivateProject,
    });
    await refresh();
  }, [onActivateProject, projectWorkspaceId, projectWorkspacePath, refresh]);

  return (
    <>
      {enabled && isVisible && (managedWorktrees.length > 0 || loading || loadError) ? (
        <div
          className="bitfun-project-worktrees"
          data-testid="nav-project-worktrees"
          data-project-path={projectWorkspacePath}
        >
          <div className="bitfun-project-worktrees__heading">
            <span>{t('sidebar.worktrees')}</span>
            {loading ? <Loader2 size={11} className="is-spinning" aria-hidden /> : null}
            <button
              type="button"
              onClick={() => setManagerOpen(true)}
              aria-label={t('manager.title')}
              data-testid="nav-worktree-manager-button"
            >
              <MoreHorizontal size={12} aria-hidden />
            </button>
          </div>
          {loadError ? (
            <button
              type="button"
              className="bitfun-project-worktrees__error"
              onClick={() => void refresh()}
            >
              {t('sidebar.loadFailed')}
            </button>
          ) : null}
          {managedWorktrees.map(worktree => {
            const isCollapsed = collapsed.has(worktree.worktreeId);
            const revision = worktree.branch || worktree.head.slice(0, 9);
            return (
              <section
                key={worktree.worktreeId}
                className="bitfun-project-worktrees__group"
                data-worktree-id={worktree.worktreeId}
              >
                <div className="bitfun-project-worktrees__row">
                  <button
                    type="button"
                    className="bitfun-project-worktrees__toggle"
                    onClick={() => {
                      setCollapsed(current => {
                        const next = new Set(current);
                        if (next.has(worktree.worktreeId)) {
                          next.delete(worktree.worktreeId);
                        } else {
                          next.add(worktree.worktreeId);
                        }
                        return next;
                      });
                    }}
                    aria-expanded={!isCollapsed}
                  >
                    <ChevronDown
                      size={11}
                      className={isCollapsed ? 'is-collapsed' : ''}
                      aria-hidden
                    />
                    <GitBranch size={11} aria-hidden />
                    <span>{revision}</span>
                  </button>
                  <div className="bitfun-project-worktrees__badges">
                    <span>{lifecycleLabel(worktree)}</span>
                    {worktree.dirty ? <span>{t('labels.dirty')}</span> : null}
                    {worktree.missing ? <span>{t('labels.missing')}</span> : null}
                    {worktree.runningSessionCount > 0
                      ? <span>{t('labels.running', { count: worktree.runningSessionCount })}</span>
                      : null}
                  </div>
                  <Tooltip
                    content={
                      worktree.missing
                        ? t('sidebar.recreateRequired')
                        : t('sidebar.newSession')
                    }
                    placement="right"
                  >
                    <button
                      type="button"
                      className="bitfun-project-worktrees__new-session"
                      onClick={() => void createSession(worktree)}
                      disabled={worktree.missing}
                      aria-label={t('sidebar.newSession')}
                    >
                      <Plus size={11} aria-hidden />
                    </button>
                  </Tooltip>
                </div>
                {!isCollapsed ? (
                  <SessionsSection
                    workspaceId={projectWorkspaceId}
                    workspacePath={projectWorkspacePath}
                    isActiveWorkspace={isActiveWorkspace}
                    isVisible={isVisible}
                    worktreeId={worktree.worktreeId}
                  />
                ) : null}
              </section>
            );
          })}
        </div>
      ) : null}

      <WorktreeLauncherModal
        isOpen={launcherOpen}
        projectWorkspacePath={projectWorkspacePath}
        projectName={projectName}
        remote={remote}
        initialMode={launcherMode}
        onClose={() => setLauncherOpen(false)}
        onSubmit={createManagedSession}
      />
      <WorktreeManagerModal
        isOpen={managerOpen}
        projectWorkspacePath={projectWorkspacePath}
        worktrees={worktrees}
        loading={loading}
        error={loadError}
        onClose={() => setManagerOpen(false)}
        onRefresh={refresh}
        onCreateWorktree={() => {
          setLauncherMode('agentic');
          setLauncherOpen(true);
        }}
        onCreateSession={worktree => createSession(worktree)}
      />
    </>
  );
};

export default ProjectWorktrees;
