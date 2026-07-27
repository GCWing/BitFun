import React, { useEffect, useMemo, useState } from 'react';
import { GitBranch, Loader2 } from 'lucide-react';
import { Button, Checkbox, Input, Modal, Select } from '@/component-library';
import { configAPI, gitAPI } from '@/infrastructure/api';
import { useI18n } from '@/infrastructure/i18n';
import type { GitStatus } from '@/infrastructure/api/service-api/GitAPI';
import './WorktreeLauncherModal.scss';

export type WorktreeSessionMode = 'agentic' | 'Cowork';

export interface WorktreeLauncherSubmit {
  mode: WorktreeSessionMode;
  baseRef: string;
  copyLocalChanges: boolean;
}

interface WorktreeSettings {
  defaultTarget: 'local' | 'managedWorktree';
  rootPath: string;
  branchPrefix: string;
  copyLocalChanges: boolean;
}

interface WorktreeLauncherModalProps {
  isOpen: boolean;
  projectWorkspacePath: string;
  projectName: string;
  remote?: boolean;
  initialMode?: WorktreeSessionMode;
  onClose: () => void;
  onSubmit: (request: WorktreeLauncherSubmit) => Promise<void>;
}

const DEFAULT_SETTINGS: WorktreeSettings = {
  defaultTarget: 'local',
  rootPath: '~/.bitfun/worktrees',
  branchPrefix: 'bitfun/',
  copyLocalChanges: false,
};

function changeCount(status: GitStatus | null): number {
  if (!status) return 0;
  return (
    status.staged.length
    + status.unstaged.length
    + status.untracked.length
    + status.conflicts.length
  );
}

export const WorktreeLauncherModal: React.FC<WorktreeLauncherModalProps> = ({
  isOpen,
  projectWorkspacePath,
  projectName,
  remote = false,
  initialMode = 'agentic',
  onClose,
  onSubmit,
}) => {
  const { t } = useI18n('worktrees');
  const [mode, setMode] = useState<WorktreeSessionMode>(initialMode);
  const [baseRef, setBaseRef] = useState('HEAD');
  const [baseCommit, setBaseCommit] = useState('');
  const [sourceHead, setSourceHead] = useState('');
  const [status, setStatus] = useState<GitStatus | null>(null);
  const [settings, setSettings] = useState(DEFAULT_SETTINGS);
  const [copyLocalChanges, setCopyLocalChanges] = useState(false);
  const [loading, setLoading] = useState(false);
  const [probing, setProbing] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [availabilityError, setAvailabilityError] = useState<string | null>(null);

  useEffect(() => {
    if (!isOpen) return;
    setMode(initialMode);
    setError(null);
    setSourceHead('');
    setAvailabilityError(remote ? t('launcher.remoteUnsupported') : null);
    setProbing(!remote);
    let cancelled = false;

    if (remote) {
      setStatus(null);
      setBaseCommit('');
      return;
    }

    void Promise.all([
      gitAPI.getRepositoryBasic(projectWorkspacePath),
      gitAPI.getStatus(projectWorkspacePath, 'worktree_launcher'),
      configAPI.getConfig('app.worktrees', { skipRetryOnNotFound: true }),
      gitAPI.resolveRevision(projectWorkspacePath, 'HEAD').catch(() => ''),
    ])
      .then(([repository, nextStatus, configured, headCommit]) => {
        if (cancelled) return;
        const nextSettings = {
          ...DEFAULT_SETTINGS,
          ...(configured && typeof configured === 'object' ? configured : {}),
        } as WorktreeSettings;
        const suggestedRef = repository.current_branch?.trim() || 'HEAD';
        setSettings(nextSettings);
        setBaseRef(suggestedRef);
        setSourceHead(headCommit);
        setStatus(nextStatus);
        setCopyLocalChanges(
          nextSettings.copyLocalChanges
          && changeCount(nextStatus) > 0
          && !!headCommit,
        );
        if (!headCommit) {
          setAvailabilityError(t('launcher.unbornRepository'));
        }
      })
      .catch(() => {
        if (!cancelled) {
          setAvailabilityError(t('launcher.notGitRepository'));
          setStatus(null);
        }
      })
      .finally(() => {
        if (!cancelled) setProbing(false);
      });

    return () => {
      cancelled = true;
    };
  }, [initialMode, isOpen, projectWorkspacePath, remote, t]);

  useEffect(() => {
    if (!isOpen || remote || availabilityError || !baseRef.trim()) {
      setBaseCommit('');
      return;
    }
    let cancelled = false;
    const timer = window.setTimeout(() => {
      void gitAPI
        .resolveRevision(projectWorkspacePath, baseRef.trim())
        .then(commit => {
          if (!cancelled) {
            setBaseCommit(commit);
            setError(null);
          }
        })
        .catch(resolveError => {
          if (!cancelled) {
            setBaseCommit('');
            const message = resolveError instanceof Error
              ? resolveError.message.toLowerCase()
              : String(resolveError).toLowerCase();
            if (
              baseRef.trim() === 'HEAD'
              && (
                message.includes('unborn')
                || message.includes('initial commit')
                || message.includes('unknown revision')
                || message.includes('needed a single revision')
              )
            ) {
              setAvailabilityError(t('launcher.unbornRepository'));
              setError(null);
            } else {
              setError(t('launcher.invalidBaseRef'));
            }
          }
        });
    }, 180);
    return () => {
      cancelled = true;
      window.clearTimeout(timer);
    };
  }, [availabilityError, baseRef, isOpen, projectWorkspacePath, remote, t]);

  const dirtyCount = changeCount(status);
  const canCopyLocalChanges =
    dirtyCount > 0 && !!sourceHead && baseCommit === sourceHead;
  useEffect(() => {
    if (!canCopyLocalChanges) {
      setCopyLocalChanges(false);
    } else if (settings.copyLocalChanges) {
      setCopyLocalChanges(true);
    }
  }, [canCopyLocalChanges, settings.copyLocalChanges]);
  const targetPreview = useMemo(
    () => `${settings.rootPath.replace(/\/$/, '')}/${projectName}/…`,
    [projectName, settings.rootPath],
  );
  const canSubmit = !probing && !availabilityError && !!baseCommit && !loading;

  const submit = async () => {
    if (!canSubmit) return;
    setLoading(true);
    setError(null);
    try {
      await onSubmit({
        mode,
        baseRef: baseRef.trim(),
        copyLocalChanges: copyLocalChanges && dirtyCount > 0,
      });
      onClose();
    } catch (submitError) {
      setError(submitError instanceof Error ? submitError.message : String(submitError));
    } finally {
      setLoading(false);
    }
  };

  return (
    <Modal
      isOpen={isOpen}
      onClose={onClose}
      title={t('launcher.title')}
      size="medium"
      contentInset
      closeOnOverlayClick={!loading}
      testId="worktree-launcher"
    >
      <div
        className="bitfun-worktree-launcher"
        onKeyDown={event => {
          if (event.key === 'Enter' && (event.metaKey || event.ctrlKey)) {
            event.preventDefault();
            void submit();
          }
        }}
      >
        <p className="bitfun-worktree-launcher__intro">{t('launcher.description')}</p>

        <div className="bitfun-worktree-launcher__field">
          <label htmlFor="worktree-session-mode">{t('launcher.mode')}</label>
          <Select
            id="worktree-session-mode"
            value={mode}
            options={[
              { value: 'agentic', label: t('launcher.codeMode') },
              { value: 'Cowork', label: t('launcher.coworkMode') },
            ]}
            onChange={value => setMode(value as WorktreeSessionMode)}
            disabled={loading}
          />
        </div>

        <div className="bitfun-worktree-launcher__field">
          <label htmlFor="worktree-base-ref">{t('launcher.baseRef')}</label>
          <Input
            id="worktree-base-ref"
            value={baseRef}
            onChange={event => setBaseRef(event.target.value)}
            placeholder="HEAD"
            disabled={loading || probing || !!availabilityError}
            autoFocus
          />
          <span className="bitfun-worktree-launcher__hint">
            {baseCommit
              ? t('launcher.resolvedCommit', { commit: baseCommit.slice(0, 12) })
              : t('launcher.baseRefHint')}
          </span>
        </div>

        <div className="bitfun-worktree-launcher__field">
          <span className="bitfun-worktree-launcher__label">{t('launcher.targetPath')}</span>
          <code className="bitfun-worktree-launcher__path">{targetPreview}</code>
        </div>

        {dirtyCount > 0 ? (
          <div className="bitfun-worktree-launcher__changes">
            <Checkbox
              checked={copyLocalChanges}
              onChange={event => setCopyLocalChanges(event.target.checked)}
              disabled={loading || !canCopyLocalChanges}
              label={t('launcher.copyChanges')}
              description={
                t('launcher.copyChangesSummary', {
                  count: dirtyCount,
                  staged: status?.staged.length ?? 0,
                  unstaged: status?.unstaged.length ?? 0,
                  untracked: status?.untracked.length ?? 0,
                })
                + (
                  canCopyLocalChanges
                    ? ''
                    : ` ${t('launcher.copyChangesRequiresHead')}`
                )
              }
            />
          </div>
        ) : null}

        {probing ? (
          <div className="bitfun-worktree-launcher__state" aria-live="polite">
            <Loader2 size={14} className="is-spinning" aria-hidden />
            {t('launcher.checking')}
          </div>
        ) : null}
        {availabilityError || error ? (
          <div className="bitfun-worktree-launcher__error" role="alert">
            {availabilityError || error}
          </div>
        ) : null}

        <div className="bitfun-worktree-launcher__footer">
          <Button variant="ghost" size="small" onClick={onClose} disabled={loading}>
            {t('actions.cancel')}
          </Button>
          <Button
            variant="primary"
            size="small"
            onClick={() => void submit()}
            disabled={!canSubmit}
            isLoading={loading}
          >
            <GitBranch size={14} aria-hidden />
            {t('launcher.create')}
          </Button>
        </div>
      </div>
    </Modal>
  );
};

export default WorktreeLauncherModal;
