import React, {
  useCallback,
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
} from 'react';
import {
  FolderGit2,
  GitBranch,
  LoaderCircle,
  MessageSquareText,
  RotateCcw,
  Save,
  Trash2,
} from 'lucide-react';
import { openAgentCompanionSession } from '@/app/services/openAgentCompanionSession';
import {
  Button,
  ConfigPageLoading,
  ConfigPageMessage,
  ConfigPageRefreshButton,
  ConfirmDialog,
  IconButton,
  Input,
  NumberInput,
  Switch,
} from '@/component-library';
import { confirmWarning } from '@/component-library/components/ConfirmDialog/confirmService';
import { flowChatManager } from '@/flow_chat/services/FlowChatManager';
import { configAPI, worktreeAPI } from '@/infrastructure/api';
import { sessionAPI } from '@/infrastructure/api/service-api/SessionAPI';
import type {
  WorktreeCommandError,
  WorktreeProjectSummary,
  WorktreeSessionSummary,
  WorktreeSettings,
  WorktreeSummary,
} from '@/infrastructure/api/service-api/WorktreeAPI';
import { useI18n } from '@/infrastructure/i18n';
import { notificationService } from '@/shared/notification-system';
import {
  ConfigPageContent,
  ConfigPageHeader,
  ConfigPageLayout,
  ConfigPageRow,
  ConfigPageSection,
} from './common';
import './WorktreesConfig.scss';

const AUTO_DELETE_LIMIT_MIN = 1;
const AUTO_DELETE_LIMIT_MAX = 100;
const WORKTREE_REMOVE_ANIMATION_MS = 180;

const DEFAULT_SETTINGS: WorktreeSettings = {
  rootPath: '~/.bitfun/worktrees',
  branchPrefix: 'bitfun/',
  copyLocalChanges: false,
  autoDeleteEnabled: true,
  autoDeleteLimit: 15,
};

interface DeleteTarget {
  projectWorkspacePath: string;
  worktree: WorktreeSummary;
}

interface ScrollSnapshot {
  anchorTop: number;
  scrollContainer: HTMLElement;
  scrollTop: number;
}

type PageMessage = {
  type: 'success' | 'error' | 'info' | 'warning';
  text: string;
};

function normalizeSettings(configured: unknown): WorktreeSettings {
  const value = configured && typeof configured === 'object'
    ? configured as Partial<WorktreeSettings>
    : {};
  const configuredLimit = typeof value.autoDeleteLimit === 'number'
    && Number.isFinite(value.autoDeleteLimit)
    ? Math.round(value.autoDeleteLimit)
    : DEFAULT_SETTINGS.autoDeleteLimit;

  return {
    rootPath: typeof value.rootPath === 'string'
      ? value.rootPath
      : DEFAULT_SETTINGS.rootPath,
    branchPrefix: typeof value.branchPrefix === 'string'
      ? value.branchPrefix
      : DEFAULT_SETTINGS.branchPrefix,
    copyLocalChanges: typeof value.copyLocalChanges === 'boolean'
      ? value.copyLocalChanges
      : DEFAULT_SETTINGS.copyLocalChanges,
    autoDeleteEnabled: typeof value.autoDeleteEnabled === 'boolean'
      ? value.autoDeleteEnabled
      : DEFAULT_SETTINGS.autoDeleteEnabled,
    autoDeleteLimit: Math.min(
      AUTO_DELETE_LIMIT_MAX,
      Math.max(AUTO_DELETE_LIMIT_MIN, configuredLimit),
    ),
  };
}

function createDeleteRequestId(): string {
  return globalThis.crypto?.randomUUID?.()
    ?? `worktree-settings-delete-${Date.now()}-${Math.random()}`;
}

type DeletionBlockReason = 'locked' | 'missing';

function deletionBlockReason(worktree: WorktreeSummary): DeletionBlockReason | null {
  if (worktree.locked) return 'locked';
  if (worktree.missing) return 'missing';
  return null;
}

function workspaceName(path: string): string {
  const parts = path.replace(/\\/g, '/').split('/').filter(Boolean);
  return parts.at(-1) ?? path;
}

function removeWorktreeFromProjects(
  projects: WorktreeProjectSummary[],
  projectWorkspacePath: string,
  worktreeId: string,
): WorktreeProjectSummary[] {
  return projects
    .map(project => project.projectWorkspacePath === projectWorkspacePath
      ? {
          ...project,
          worktrees: project.worktrees.filter(worktree => worktree.worktreeId !== worktreeId),
        }
      : project)
    .filter(project => project.worktrees.length > 0);
}

function removalAnimationDuration(): number {
  return globalThis.matchMedia?.('(prefers-reduced-motion: reduce)').matches
    ? 0
    : WORKTREE_REMOVE_ANIMATION_MS;
}

function waitFor(ms: number): Promise<void> {
  return ms > 0
    ? new Promise(resolve => globalThis.setTimeout(resolve, ms))
    : Promise.resolve();
}

const WorktreesConfig: React.FC = () => {
  const { t } = useI18n('worktrees');
  const [settings, setSettings] = useState(DEFAULT_SETTINGS);
  const [settingsLoading, setSettingsLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [settingsMessage, setSettingsMessage] = useState<PageMessage | null>(null);
  const [projects, setProjects] = useState<WorktreeProjectSummary[]>([]);
  const [projectsLoading, setProjectsLoading] = useState(true);
  const [projectsInitialized, setProjectsInitialized] = useState(false);
  const [projectsMessage, setProjectsMessage] = useState<PageMessage | null>(null);
  const [deleteTarget, setDeleteTarget] = useState<DeleteTarget | null>(null);
  const [deletingWorktreeId, setDeletingWorktreeId] = useState<string | null>(null);
  const [removingWorktreeId, setRemovingWorktreeId] = useState<string | null>(null);
  const [openingSessionId, setOpeningSessionId] = useState<string | null>(null);
  const [projectsLayoutRevision, setProjectsLayoutRevision] = useState(0);
  const projectsResultsRef = useRef<HTMLDivElement>(null);
  const projectsRequestIdRef = useRef(0);
  const pendingScrollSnapshotRef = useRef<ScrollSnapshot | null>(null);
  const worktreeMutationInFlightRef = useRef(false);
  const pendingWorktreeRefreshRef = useRef(false);

  const loadSettings = useCallback(async () => {
    setSettingsLoading(true);
    setSettingsMessage(null);
    try {
      const configured = await configAPI.getConfig('app.worktrees', {
        skipRetryOnNotFound: true,
      });
      setSettings(normalizeSettings(configured));
    } catch {
      setSettingsMessage({ type: 'error', text: t('settings.loadFailed') });
    } finally {
      setSettingsLoading(false);
    }
  }, [t]);

  const captureScrollSnapshot = useCallback((): ScrollSnapshot | null => {
    const anchor = projectsResultsRef.current;
    const scrollContainer = anchor?.closest<HTMLElement>('.bitfun-config-page-layout');
    if (!anchor || !scrollContainer) {
      return null;
    }
    return {
      anchorTop: anchor.getBoundingClientRect().top,
      scrollContainer,
      scrollTop: scrollContainer.scrollTop,
    };
  }, []);

  const commitProjectsLayoutChange = useCallback((update: () => void) => {
    pendingScrollSnapshotRef.current = captureScrollSnapshot();
    update();
    setProjectsLayoutRevision(current => current + 1);
  }, [captureScrollSnapshot]);

  useLayoutEffect(() => {
    const snapshot = pendingScrollSnapshotRef.current;
    const anchor = projectsResultsRef.current;
    if (!snapshot || !anchor || !snapshot.scrollContainer.isConnected) {
      pendingScrollSnapshotRef.current = null;
      return;
    }

    const anchorDelta = anchor.getBoundingClientRect().top - snapshot.anchorTop;
    snapshot.scrollContainer.scrollTop = snapshot.scrollTop + anchorDelta;
    pendingScrollSnapshotRef.current = null;
  }, [projectsLayoutRevision]);

  const loadProjects = useCallback(async (): Promise<boolean> => {
    const requestId = ++projectsRequestIdRef.current;
    setProjectsLoading(true);
    try {
      const nextProjects = await worktreeAPI.listProjects();
      if (requestId !== projectsRequestIdRef.current) {
        return false;
      }
      commitProjectsLayoutChange(() => {
        setProjects(nextProjects);
        setProjectsMessage(null);
        setProjectsInitialized(true);
        setProjectsLoading(false);
      });
      return true;
    } catch {
      if (requestId !== projectsRequestIdRef.current) {
        return false;
      }
      commitProjectsLayoutChange(() => {
        setProjectsMessage({ type: 'error', text: t('management.loadFailed') });
        setProjectsInitialized(true);
        setProjectsLoading(false);
      });
      return false;
    }
  }, [commitProjectsLayoutChange, t]);

  useEffect(() => {
    void loadSettings();
    void loadProjects();
    return worktreeAPI.onChanged(() => {
      if (worktreeMutationInFlightRef.current) {
        pendingWorktreeRefreshRef.current = true;
        return;
      }
      void loadProjects();
    });
  }, [loadProjects, loadSettings]);

  const save = async () => {
    if (!settings.rootPath.trim() || !settings.branchPrefix.trim()) {
      setSettingsMessage({ type: 'error', text: t('settings.required') });
      return;
    }
    if (
      settings.autoDeleteLimit < AUTO_DELETE_LIMIT_MIN
      || settings.autoDeleteLimit > AUTO_DELETE_LIMIT_MAX
    ) {
      setSettingsMessage({
        type: 'error',
        text: t('settings.autoDeleteLimit.invalid', {
          min: AUTO_DELETE_LIMIT_MIN,
          max: AUTO_DELETE_LIMIT_MAX,
        }),
      });
      return;
    }

    setSaving(true);
    setSettingsMessage(null);
    try {
      const normalized = {
        ...settings,
        rootPath: settings.rootPath.trim(),
        branchPrefix: settings.branchPrefix.trim(),
        autoDeleteLimit: Math.round(settings.autoDeleteLimit),
      };
      await configAPI.setConfig('app.worktrees', normalized);
      setSettings(normalized);
      setSettingsMessage({ type: 'success', text: t('settings.saved') });
    } catch {
      setSettingsMessage({ type: 'error', text: t('settings.saveFailed') });
    } finally {
      setSaving(false);
    }
  };

  const confirmDelete = async () => {
    const target = deleteTarget;
    if (!target) return;

    setDeleteTarget(null);
    setDeletingWorktreeId(target.worktree.worktreeId);
    worktreeMutationInFlightRef.current = true;
    pendingWorktreeRefreshRef.current = false;
    try {
      const discardLocalWork =
        target.worktree.dirty || target.worktree.hasUnpublishedCommits;
      await worktreeAPI.remove(
        target.projectWorkspacePath,
        target.worktree.worktreeId,
        createDeleteRequestId(),
        discardLocalWork,
      );
      setRemovingWorktreeId(target.worktree.worktreeId);
      await waitFor(removalAnimationDuration());
      commitProjectsLayoutChange(() => {
        setProjects(current => removeWorktreeFromProjects(
          current,
          target.projectWorkspacePath,
          target.worktree.worktreeId,
        ));
        setRemovingWorktreeId(null);
      });
      notificationService.success(
        t('management.deleted', { path: target.worktree.path }),
        { duration: 2400 },
      );
      await loadProjects();
      pendingWorktreeRefreshRef.current = false;
    } catch (error) {
      const code = (error as Partial<WorktreeCommandError> | null)?.code;
      const text = (() => {
        switch (code) {
          case 'worktree_locked':
            return t('management.errors.locked');
          case 'dirty_worktree':
            return t('management.errors.dirty');
          case 'unpublished_commits':
            return t('management.errors.unpublishedCommits');
          case 'worktree_not_found':
            return t('management.errors.notFound');
          case 'remote_unsupported':
            return t('management.errors.remoteUnsupported');
          default:
            return t('management.errors.deleteFailed');
        }
      })();
      commitProjectsLayoutChange(() => {
        setProjectsMessage({
          type: 'error',
          text,
        });
      });
    } finally {
      worktreeMutationInFlightRef.current = false;
      setDeletingWorktreeId(null);
      setRemovingWorktreeId(null);
      if (pendingWorktreeRefreshRef.current) {
        pendingWorktreeRefreshRef.current = false;
        void loadProjects();
      }
    }
  };

  const openAssociatedSession = useCallback(async (
    projectWorkspacePath: string,
    session: WorktreeSessionSummary,
  ) => {
    if (openingSessionId) return;

    setOpeningSessionId(session.sessionId);
    try {
      if (session.archived) {
        const shouldRestore = await confirmWarning(
          t('management.sessions.restoreTitle'),
          t('management.sessions.restoreMessage', { name: session.sessionName }),
        );
        if (!shouldRestore) {
          return;
        }
        await sessionAPI.unarchiveSession(session.sessionId, projectWorkspacePath);
        await flowChatManager.refreshWorkspaceSessions({
          rootPath: projectWorkspacePath,
        });
      }

      let opened = await openAgentCompanionSession(session.sessionId);
      if (!opened) {
        await flowChatManager.refreshWorkspaceSessions({
          rootPath: projectWorkspacePath,
        });
        opened = await openAgentCompanionSession(session.sessionId);
      }
      if (!opened) {
        throw new Error('Associated session was not found after refreshing the workspace');
      }
    } catch {
      notificationService.error(t('management.sessions.openFailed'), {
        duration: 3200,
      });
    } finally {
      setOpeningSessionId(null);
    }
  }, [openingSessionId, t]);

  const renderSettings = () => {
    if (settingsLoading) {
      return <ConfigPageLoading text={t('settings.loading')} />;
    }

    return (
      <>
        <ConfigPageMessage message={settingsMessage} />
        <ConfigPageSection
          title={t('settings.isolation.title')}
          description={t('settings.isolation.description')}
        >
          <ConfigPageRow
            label={t('settings.rootPath.label')}
            description={t('settings.rootPath.description')}
          >
            <Input
              value={settings.rootPath}
              onChange={event => setSettings(current => ({
                ...current,
                rootPath: event.target.value,
              }))}
              disabled={saving}
            />
          </ConfigPageRow>
          <ConfigPageRow
            label={t('settings.branchPrefix.label')}
            description={t('settings.branchPrefix.description')}
          >
            <Input
              value={settings.branchPrefix}
              onChange={event => setSettings(current => ({
                ...current,
                branchPrefix: event.target.value,
              }))}
              disabled={saving}
            />
          </ConfigPageRow>
          <ConfigPageRow
            label={t('settings.copyChanges.label')}
            description={t('settings.copyChanges.description')}
            align="center"
          >
            <Switch
              checked={settings.copyLocalChanges}
              onChange={event => setSettings(current => ({
                ...current,
                copyLocalChanges: event.target.checked,
              }))}
              disabled={saving}
            />
          </ConfigPageRow>
          <ConfigPageRow
            label={t('settings.autoDelete.label')}
            description={t('settings.autoDelete.description')}
            align="center"
          >
            <Switch
              checked={settings.autoDeleteEnabled}
              onChange={event => setSettings(current => ({
                ...current,
                autoDeleteEnabled: event.target.checked,
              }))}
              disabled={saving}
            />
          </ConfigPageRow>
          <ConfigPageRow
            label={t('settings.autoDeleteLimit.label')}
            description={t('settings.autoDeleteLimit.description')}
            align="center"
          >
            <NumberInput
              value={settings.autoDeleteLimit}
              onChange={value => setSettings(current => ({
                ...current,
                autoDeleteLimit: value,
              }))}
              min={AUTO_DELETE_LIMIT_MIN}
              max={AUTO_DELETE_LIMIT_MAX}
              showButtons={false}
              disableWheel
              disabled={saving || !settings.autoDeleteEnabled}
            />
          </ConfigPageRow>
        </ConfigPageSection>
        <div className="bitfun-worktrees-config__actions">
          <Button
            variant="ghost"
            size="small"
            onClick={() => setSettings(DEFAULT_SETTINGS)}
            disabled={saving}
          >
            <RotateCcw size={14} aria-hidden />
            {t('settings.reset')}
          </Button>
          <Button
            variant="primary"
            size="small"
            onClick={() => void save()}
            isLoading={saving}
          >
            <Save size={14} aria-hidden />
            {t('settings.save')}
          </Button>
        </div>
      </>
    );
  };

  const renderWorktree = (
    project: WorktreeProjectSummary,
    worktree: WorktreeSummary,
  ) => {
    const blockCode = deletionBlockReason(worktree);
    const blockReason = (() => {
      switch (blockCode) {
        case 'locked':
          return t('management.protection.locked');
        case 'missing':
          return t('management.protection.missing');
        default:
          return null;
      }
    })();
    const sessionNames = worktree.sessions
      .map(session => session.sessionName)
      .join(', ');
    const branchLabel = worktree.branch
      ?? t('labels.detached', { commit: worktree.head.slice(0, 7) });
    const lifecycleLabel = (() => {
      switch (worktree.lifecycle) {
        case 'permanent':
          return t('management.lifecycle.permanent');
        case 'external':
          return t('management.lifecycle.external');
        default:
          return t('management.lifecycle.managed');
      }
    })();

    return (
      <article
        className={[
          'bitfun-worktrees-config__worktree',
          removingWorktreeId === worktree.worktreeId
            && 'bitfun-worktrees-config__worktree--removing',
        ].filter(Boolean).join(' ')}
        key={worktree.worktreeId}
        data-worktree-id={worktree.worktreeId}
      >
        <div className="bitfun-worktrees-config__worktree-main">
          <div className="bitfun-worktrees-config__worktree-copy">
            <div className="bitfun-worktrees-config__worktree-heading">
              <h5 className="bitfun-worktrees-config__worktree-title">{branchLabel}</h5>
              <div className="bitfun-worktrees-config__metadata">
                {worktree.lifecycle !== 'managed' && <span>{lifecycleLabel}</span>}
                {worktree.dirty && <span>{t('management.state.dirty')}</span>}
                {worktree.hasUnpublishedCommits && (
                  <span>{t('management.state.unpublishedCommits')}</span>
                )}
                {worktree.locked && <span>{t('management.state.locked')}</span>}
                {worktree.missing && <span>{t('management.state.missing')}</span>}
              </div>
            </div>
            <code className="bitfun-worktrees-config__path" title={worktree.path}>
              {worktree.path}
            </code>
            {worktree.associatedSessionCount > 0 && (
              <div
                className="bitfun-worktrees-config__sessions-summary"
                title={sessionNames}
              >
                <MessageSquareText size={13} aria-hidden />
                <span>
                  {t('management.sessions.summary', {
                    count: worktree.associatedSessionCount,
                  })}
                </span>
                <span className="bitfun-worktrees-config__session-links">
                  {worktree.sessions.map(session => (
                    <button
                      key={session.sessionId}
                      type="button"
                      className="bitfun-worktrees-config__session-link"
                      disabled={openingSessionId !== null}
                      title={t('management.sessions.openLabel', {
                        name: session.sessionName,
                      })}
                      onClick={() => void openAssociatedSession(
                        project.projectWorkspacePath,
                        session,
                      )}
                    >
                      {openingSessionId === session.sessionId && (
                        <LoaderCircle
                          className="bitfun-worktrees-config__session-link-spinner"
                          size={12}
                          aria-hidden
                        />
                      )}
                      <span>{session.sessionName}</span>
                      {session.archived && (
                        <span className="bitfun-worktrees-config__session-link-state">
                          {t('management.sessions.status.archived')}
                        </span>
                      )}
                    </button>
                  ))}
                </span>
              </div>
            )}
          </div>
          <div className="bitfun-worktrees-config__delete-control">
            <IconButton
              variant="danger"
              size="small"
              disabled={Boolean(blockCode) || deletingWorktreeId !== null}
              isLoading={deletingWorktreeId === worktree.worktreeId}
              tooltip={blockReason ?? t('management.delete.action')}
              title={blockReason ?? undefined}
              onClick={() => setDeleteTarget({
                projectWorkspacePath: project.projectWorkspacePath,
                worktree,
              })}
              aria-label={t('management.delete.actionLabel', { path: worktree.path })}
            >
              <Trash2 size={14} aria-hidden />
            </IconButton>
          </div>
        </div>
      </article>
    );
  };

  const renderProjectsSkeleton = () => (
    <div className="bitfun-worktrees-config__skeleton">
      <span className="bitfun-sr-only" role="status">
        {t('management.loading')}
      </span>
      <div className="bitfun-worktrees-config__skeleton-header" aria-hidden="true">
        <span />
        <span />
      </div>
      <div className="bitfun-worktrees-config__skeleton-list" aria-hidden="true">
        {[0, 1, 2].map(index => (
          <div className="bitfun-worktrees-config__skeleton-row" key={index}>
            <span />
            <span />
            <span />
          </div>
        ))}
      </div>
    </div>
  );

  const renderProjects = () => {
    if (!projectsInitialized && projectsLoading) {
      return renderProjectsSkeleton();
    }
    if (projects.length === 0 && !projectsMessage) {
      return (
        <div className="bitfun-worktrees-config__empty">
          <FolderGit2 size={22} aria-hidden />
          <div>
            <h4>{t('management.empty.title')}</h4>
            <p>{t('management.empty.description')}</p>
          </div>
        </div>
      );
    }
    if (projects.length === 0) {
      return null;
    }

    return (
      <div className="bitfun-worktrees-config__projects">
        {projects.map(project => (
          <section
            className="bitfun-worktrees-config__project"
            data-bf-component="worktrees-config"
            data-bf-part="project"
            key={project.projectWorkspacePath}
          >
            <header className="bitfun-worktrees-config__project-header">
              <div className="bitfun-worktrees-config__project-identity">
                <h4>{workspaceName(project.projectWorkspacePath)}</h4>
                <code title={project.projectWorkspacePath}>
                  {project.projectWorkspacePath}
                </code>
              </div>
              <span>
                {t('management.worktreeCount', { count: project.worktrees.length })}
              </span>
            </header>
            <div
              className="bitfun-worktrees-config__worktree-list"
              data-bf-component="worktrees-config"
              data-bf-part="worktreeList"
            >
              {project.worktrees.map(worktree => renderWorktree(project, worktree))}
            </div>
          </section>
        ))}
      </div>
    );
  };

  const deletingWithLocalWork = Boolean(
    deleteTarget?.worktree.dirty || deleteTarget?.worktree.hasUnpublishedCommits,
  );
  const deletingWithSessions = Boolean(deleteTarget?.worktree.associatedSessionCount);
  const deleteMessage = (() => {
    if (deletingWithLocalWork && deletingWithSessions) {
      return t('management.delete.forceMessageWithSessions');
    }
    if (deletingWithLocalWork) {
      return t('management.delete.forceMessage');
    }
    if (deletingWithSessions) {
      return t('management.delete.messageWithSessions');
    }
    return t('management.delete.message');
  })();

  return (
    <ConfigPageLayout
      className="bitfun-worktrees-config"
      data-bf-component="worktrees-config"
      data-bf-part="root"
    >
      <ConfigPageHeader
        icon={<GitBranch size={20} aria-hidden />}
        title={t('settings.title')}
        subtitle={t('settings.description')}
      />
      <ConfigPageContent>
        {renderSettings()}
        <ConfigPageSection
          className="bitfun-worktrees-config__management-section"
          mouseGlowSurface={false}
          title={t('management.title')}
          description={t('management.description')}
          extra={(
            <ConfigPageRefreshButton
              tooltip={t('management.refresh')}
              onClick={() => void loadProjects()}
              loading={projectsLoading}
              disabled={deletingWorktreeId !== null}
            />
          )}
        >
          <ConfigPageMessage message={projectsMessage} />
          {projectsMessage?.type === 'error' && projects.length === 0 && (
            <Button
              variant="secondary"
              size="small"
              onClick={() => void loadProjects()}
            >
              {t('management.retry')}
            </Button>
          )}
          <div
            ref={projectsResultsRef}
            data-bf-component="worktrees-config"
            data-bf-part="results"
            className={[
              'bitfun-worktrees-config__results',
              projectsLoading
                && projectsInitialized
                && 'bitfun-worktrees-config__results--refreshing',
            ].filter(Boolean).join(' ')}
            aria-busy={projectsLoading}
          >
            {projectsLoading && projectsInitialized && (
              <>
                <div
                  className="bitfun-worktrees-config__refresh-progress"
                  aria-hidden="true"
                >
                  <span />
                </div>
                <span className="bitfun-sr-only" role="status">
                  {t('management.loading')}
                </span>
              </>
            )}
            {renderProjects()}
          </div>
        </ConfigPageSection>
      </ConfigPageContent>

      <ConfirmDialog
        isOpen={deleteTarget !== null}
        onClose={() => setDeleteTarget(null)}
        onConfirm={() => void confirmDelete()}
        title={deletingWithLocalWork
          ? t('management.delete.forceTitle')
          : t('management.delete.title')}
        message={deleteMessage}
        preview={deleteTarget?.worktree.path}
        type={deletingWithLocalWork ? 'error' : 'warning'}
        confirmDanger
        confirmText={deletingWithLocalWork
          ? t('management.delete.forceAction')
          : t('management.delete.action')}
        cancelText={t('management.delete.cancel')}
      />
    </ConfigPageLayout>
  );
};

export default WorktreesConfig;
