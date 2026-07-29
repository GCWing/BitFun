import React, { useCallback, useEffect, useState } from 'react';
import {
  FolderGit2,
  GitBranch,
  RotateCcw,
  Save,
  Trash2,
} from 'lucide-react';
import {
  Button,
  ConfigPageLoading,
  ConfigPageMessage,
  ConfigPageRefreshButton,
  ConfirmDialog,
  Input,
  NumberInput,
  Switch,
} from '@/component-library';
import { configAPI, worktreeAPI } from '@/infrastructure/api';
import type {
  WorktreeCommandError,
  WorktreeProjectSummary,
  WorktreeSettings,
  WorktreeSummary,
} from '@/infrastructure/api/service-api/WorktreeAPI';
import { useI18n } from '@/infrastructure/i18n';
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

type DeletionBlockReason = 'associatedSessions' | 'locked' | 'missing';

function deletionBlockReason(worktree: WorktreeSummary): DeletionBlockReason | null {
  if (worktree.associatedSessionCount > 0) return 'associatedSessions';
  if (worktree.locked) return 'locked';
  if (worktree.missing) return 'missing';
  return null;
}

const WorktreesConfig: React.FC = () => {
  const { t } = useI18n('worktrees');
  const [settings, setSettings] = useState(DEFAULT_SETTINGS);
  const [settingsLoading, setSettingsLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [settingsMessage, setSettingsMessage] = useState<PageMessage | null>(null);
  const [projects, setProjects] = useState<WorktreeProjectSummary[]>([]);
  const [projectsLoading, setProjectsLoading] = useState(true);
  const [projectsMessage, setProjectsMessage] = useState<PageMessage | null>(null);
  const [deleteTarget, setDeleteTarget] = useState<DeleteTarget | null>(null);
  const [deletingWorktreeId, setDeletingWorktreeId] = useState<string | null>(null);

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

  const loadProjects = useCallback(async () => {
    setProjectsLoading(true);
    setProjectsMessage(null);
    try {
      setProjects(await worktreeAPI.listProjects());
    } catch {
      setProjectsMessage({ type: 'error', text: t('management.loadFailed') });
    } finally {
      setProjectsLoading(false);
    }
  }, [t]);

  useEffect(() => {
    void loadSettings();
    void loadProjects();
    return worktreeAPI.onChanged(() => {
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
    setProjectsMessage(null);
    try {
      const discardLocalWork =
        target.worktree.dirty || target.worktree.hasUnpublishedCommits;
      await worktreeAPI.remove(
        target.projectWorkspacePath,
        target.worktree.worktreeId,
        createDeleteRequestId(),
        discardLocalWork,
      );
      await loadProjects();
      setProjectsMessage({
        type: 'success',
        text: t('management.deleted', { path: target.worktree.path }),
      });
    } catch (error) {
      const code = (error as Partial<WorktreeCommandError> | null)?.code;
      const text = (() => {
        switch (code) {
          case 'worktree_busy':
            return t('management.errors.associatedSessions');
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
      setProjectsMessage({
        type: 'error',
        text,
      });
    } finally {
      setDeletingWorktreeId(null);
    }
  };

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
        case 'associatedSessions':
          return t('management.protection.associatedSessions');
        case 'locked':
          return t('management.protection.locked');
        case 'missing':
          return t('management.protection.missing');
        default:
          return null;
      }
    })();
    const branchLabel = worktree.branch
      ?? t('labels.detached', { commit: worktree.head.slice(0, 7) });
    const forceDelete = worktree.dirty || worktree.hasUnpublishedCommits;
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
        className="bitfun-worktrees-config__worktree"
        key={worktree.worktreeId}
        data-worktree-id={worktree.worktreeId}
      >
        <div className="bitfun-worktrees-config__worktree-main">
          <div className="bitfun-worktrees-config__worktree-copy">
            <h5 className="bitfun-worktrees-config__worktree-title">{branchLabel}</h5>
            <code className="bitfun-worktrees-config__path" title={worktree.path}>
              {worktree.path}
            </code>
            <div className="bitfun-worktrees-config__metadata">
              <span>{lifecycleLabel}</span>
              {worktree.dirty && <span>{t('management.state.dirty')}</span>}
              {worktree.hasUnpublishedCommits && (
                <span>{t('management.state.unpublishedCommits')}</span>
              )}
              {worktree.locked && <span>{t('management.state.locked')}</span>}
              {worktree.missing && <span>{t('management.state.missing')}</span>}
              {worktree.runningSessionCount > 0 && (
                <span>
                  {t('management.state.activeSessions', {
                    count: worktree.runningSessionCount,
                  })}
                </span>
              )}
            </div>
          </div>
          <div className="bitfun-worktrees-config__delete-control" title={blockReason ?? undefined}>
            <Button
              variant="danger"
              size="small"
              disabled={Boolean(blockCode) || deletingWorktreeId !== null}
              isLoading={deletingWorktreeId === worktree.worktreeId}
              onClick={() => setDeleteTarget({
                projectWorkspacePath: project.projectWorkspacePath,
                worktree,
              })}
              aria-label={t('management.delete.actionLabel', { path: worktree.path })}
            >
              <Trash2 size={14} aria-hidden />
              {t('management.delete.action')}
            </Button>
          </div>
        </div>

        <div className="bitfun-worktrees-config__sessions">
          <div className="bitfun-worktrees-config__sessions-label">
            {t('management.sessions.title')}
          </div>
          {worktree.sessions.length > 0 ? (
            <ul className="bitfun-worktrees-config__session-list">
              {worktree.sessions.map(session => (
                <li key={session.sessionId}>
                  <span>{session.sessionName}</span>
                  <span className="bitfun-worktrees-config__session-status">
                    {session.status === 'archived'
                      ? t('management.sessions.status.archived')
                      : session.status === 'completed'
                      ? t('shared:statuses.done')
                      : t('management.sessions.status.active')}
                  </span>
                </li>
              ))}
            </ul>
          ) : (
            <div className="bitfun-worktrees-config__sessions-empty">
              {t('management.sessions.empty')}
            </div>
          )}
          {blockReason && (
            <div className="bitfun-worktrees-config__protection-note">
              {blockReason}
            </div>
          )}
          {forceDelete && !blockReason && (
            <div className="bitfun-worktrees-config__protection-note bitfun-worktrees-config__protection-note--warning">
              {t('management.delete.forceHint')}
            </div>
          )}
        </div>
      </article>
    );
  };

  const renderProjects = () => {
    if (projectsLoading) {
      return <ConfigPageLoading text={t('management.loading')} />;
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
            key={project.projectWorkspacePath}
          >
            <header className="bitfun-worktrees-config__project-header">
              <h4 title={project.projectWorkspacePath}>
                {project.projectWorkspacePath}
              </h4>
              <span>
                {t('management.worktreeCount', { count: project.worktrees.length })}
              </span>
            </header>
            <div className="bitfun-worktrees-config__worktree-list">
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

  return (
    <ConfigPageLayout className="bitfun-worktrees-config">
      <ConfigPageHeader
        icon={<GitBranch size={20} aria-hidden />}
        title={t('settings.title')}
        subtitle={t('settings.description')}
      />
      <ConfigPageContent>
        {renderSettings()}
        <ConfigPageSection
          className="bitfun-worktrees-config__management-section"
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
          {renderProjects()}
        </ConfigPageSection>
      </ConfigPageContent>

      <ConfirmDialog
        isOpen={deleteTarget !== null}
        onClose={() => setDeleteTarget(null)}
        onConfirm={() => void confirmDelete()}
        title={deletingWithLocalWork
          ? t('management.delete.forceTitle')
          : t('management.delete.title')}
        message={deletingWithLocalWork
          ? t('management.delete.forceMessage')
          : t('management.delete.message')}
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
