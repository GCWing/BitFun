import React, { useEffect, useMemo, useState } from 'react';
import {
  Archive,
  ExternalLink,
  GitBranch,
  Loader2,
  Plus,
  RefreshCw,
  Trash2,
} from 'lucide-react';
import { Button, ConfirmDialog, InputDialog, Modal } from '@/component-library';
import { configAPI, workspaceAPI, worktreeAPI } from '@/infrastructure/api';
import type { WorktreeSummary } from '@/infrastructure/api/service-api/WorktreeAPI';
import { WorktreeCommandError } from '@/infrastructure/api/service-api/WorktreeAPI';
import { useI18n } from '@/infrastructure/i18n';
import { notificationService } from '@/shared/notification-system';
import './WorktreeManagerModal.scss';

interface WorktreeManagerModalProps {
  isOpen: boolean;
  projectWorkspacePath: string;
  worktrees: WorktreeSummary[];
  loading: boolean;
  error?: string | null;
  onClose: () => void;
  onRefresh: () => Promise<void>;
  onCreateWorktree: () => void;
  onCreateSession: (worktree: WorktreeSummary) => Promise<void>;
}

function requestId(): string {
  return globalThis.crypto?.randomUUID?.() ?? `worktree-${Date.now()}-${Math.random()}`;
}

export const WorktreeManagerModal: React.FC<WorktreeManagerModalProps> = ({
  isOpen,
  projectWorkspacePath,
  worktrees,
  loading,
  error,
  onClose,
  onRefresh,
  onCreateWorktree,
  onCreateSession,
}) => {
  const { t } = useI18n('worktrees');
  const [branchTarget, setBranchTarget] = useState<WorktreeSummary | null>(null);
  const [removeTarget, setRemoveTarget] = useState<WorktreeSummary | null>(null);
  const [forceStage, setForceStage] = useState(false);
  const [pendingId, setPendingId] = useState<string | null>(null);
  const [branchPrefix, setBranchPrefix] = useState('bitfun/');

  useEffect(() => {
    if (!isOpen) return;
    void configAPI
      .getConfig('app.worktrees', { skipRetryOnNotFound: true })
      .then(value => {
        if (value && typeof value.branchPrefix === 'string') {
          setBranchPrefix(value.branchPrefix);
        }
      })
      .catch(() => undefined);
  }, [isOpen]);

  const visibleWorktrees = useMemo(
    () => worktrees.filter(worktree => !worktree.isMain),
    [worktrees],
  );
  const lifecycleLabel = (worktree: WorktreeSummary): string => {
    if (worktree.lifecycle === 'permanent') return t('labels.lifecycle.permanent');
    if (worktree.lifecycle === 'external') return t('labels.lifecycle.external');
    return t('labels.lifecycle.managed');
  };

  const runMutation = async (
    worktree: WorktreeSummary,
    operation: () => Promise<unknown>,
    successMessage: string,
  ): Promise<unknown | null> => {
    setPendingId(worktree.worktreeId);
    try {
      await operation();
      notificationService.success(successMessage, { duration: 2500 });
      await onRefresh();
      return null;
    } catch (operationError) {
      notificationService.error(
        operationError instanceof Error ? operationError.message : String(operationError),
        { duration: 4500 },
      );
      return operationError;
    } finally {
      setPendingId(null);
    }
  };

  const confirmRemove = async () => {
    if (!removeTarget) return;
    const hasBlockingSessions = removeTarget.runningSessionCount > 0;
    if (hasBlockingSessions) {
      setRemoveTarget(null);
      setForceStage(false);
      return;
    }
    const needsForce = removeTarget.dirty || removeTarget.hasUnpublishedCommits;
    if (needsForce && !forceStage) {
      setForceStage(true);
      return;
    }
    const target = removeTarget;
    const operationError = await runMutation(
      target,
      () => worktreeAPI.remove(
        projectWorkspacePath,
        target.worktreeId,
        requestId(),
        needsForce,
      ),
      t('manager.removed'),
    );
    if (!operationError) {
      setRemoveTarget(null);
      setForceStage(false);
    } else if (
      operationError instanceof WorktreeCommandError
      && (operationError.code === 'dirty_worktree'
        || operationError.code === 'unpublished_commits')
    ) {
      setForceStage(true);
    }
  };

  const removeRisks = removeTarget
    ? [
        removeTarget.dirty ? t('manager.risks.dirty') : null,
        removeTarget.hasUnpublishedCommits ? t('manager.risks.unpublished') : null,
        removeTarget.associatedSessionCount > 0
          ? t('manager.risks.sessions', { count: removeTarget.associatedSessionCount })
          : null,
        removeTarget.runningSessionCount > 0
          ? t('manager.risks.running', { count: removeTarget.runningSessionCount })
          : null,
      ].filter((value): value is string => !!value)
    : [];

  return (
    <>
      <Modal
        isOpen={isOpen}
        onClose={onClose}
        title={t('manager.title')}
        size="large"
        contentInset
        testId="worktree-manager"
      >
        <div className="bitfun-worktree-manager">
          <div className="bitfun-worktree-manager__toolbar">
            <p>{t('manager.description')}</p>
            <div>
              <Button
                variant="ghost"
                size="small"
                onClick={() => void onRefresh()}
                disabled={loading}
                aria-label={t('manager.refresh')}
              >
                <RefreshCw size={14} aria-hidden />
                {t('manager.refresh')}
              </Button>
              <Button variant="primary" size="small" onClick={onCreateWorktree}>
                <Plus size={14} aria-hidden />
                {t('manager.create')}
              </Button>
            </div>
          </div>

          {loading ? (
            <div className="bitfun-worktree-manager__state" aria-live="polite">
              <Loader2 size={15} className="is-spinning" aria-hidden />
              {t('manager.loading')}
            </div>
          ) : null}
          {error ? (
            <div className="bitfun-worktree-manager__error" role="alert">{error}</div>
          ) : null}
          {!loading && !error && visibleWorktrees.length === 0 ? (
            <div className="bitfun-worktree-manager__empty">
              <GitBranch size={24} aria-hidden />
              <strong>{t('manager.emptyTitle')}</strong>
              <span>{t('manager.emptyDescription')}</span>
            </div>
          ) : null}

          <div className="bitfun-worktree-manager__list">
            {visibleWorktrees.map(worktree => {
              const pending = pendingId === worktree.worktreeId;
              const revision = worktree.branch || t('labels.detached', {
                commit: worktree.head.slice(0, 10),
              });
              return (
                <article
                  key={worktree.worktreeId}
                  className="bitfun-worktree-manager__item"
                  data-testid="worktree-manager-item"
                  data-worktree-id={worktree.worktreeId}
                >
                  <div className="bitfun-worktree-manager__item-copy">
                    <div className="bitfun-worktree-manager__item-heading">
                      <GitBranch size={14} aria-hidden />
                      <strong>{revision}</strong>
                      <span>{lifecycleLabel(worktree)}</span>
                      {worktree.dirty ? <span>{t('labels.dirty')}</span> : null}
                      {worktree.missing ? <span>{t('labels.missing')}</span> : null}
                    </div>
                    <code>{worktree.path}</code>
                    <span>
                      {t('manager.sessionCount', { count: worktree.associatedSessionCount })}
                      {worktree.hasUnpublishedCommits
                        ? ` · ${t('labels.unpublished')}`
                        : ''}
                    </span>
                  </div>
                  <div className="bitfun-worktree-manager__actions">
                    {!worktree.missing ? (
                      <>
                        <Button
                          variant="ghost"
                          size="small"
                          onClick={() => void workspaceAPI.revealInExplorer(worktree.path)}
                          disabled={pending}
                        >
                          <ExternalLink size={13} aria-hidden />
                          {t('manager.open')}
                        </Button>
                        <Button
                          variant="ghost"
                          size="small"
                          onClick={() => void onCreateSession(worktree)}
                          disabled={pending}
                        >
                          <Plus size={13} aria-hidden />
                          {t('manager.newSession')}
                        </Button>
                        {!worktree.branch ? (
                          <Button
                            variant="ghost"
                            size="small"
                            onClick={() => setBranchTarget(worktree)}
                            disabled={pending}
                          >
                            <GitBranch size={13} aria-hidden />
                            {t('manager.createBranch')}
                          </Button>
                        ) : null}
                      </>
                    ) : (
                      <Button
                        variant="ghost"
                        size="small"
                        onClick={() => void runMutation(
                          worktree,
                          () => worktreeAPI.recreate(
                            projectWorkspacePath,
                            worktree.worktreeId,
                            requestId(),
                          ),
                          t('manager.recreated'),
                        )}
                        disabled={pending}
                      >
                        <RefreshCw size={13} aria-hidden />
                        {t('manager.recreate')}
                      </Button>
                    )}
                    {worktree.lifecycle === 'managed' ? (
                      <Button
                        variant="ghost"
                        size="small"
                        onClick={() => void runMutation(
                          worktree,
                          () => worktreeAPI.promote(
                            projectWorkspacePath,
                            worktree.worktreeId,
                            requestId(),
                          ),
                          t('manager.promoted'),
                        )}
                        disabled={pending}
                      >
                        <Archive size={13} aria-hidden />
                        {t('manager.keep')}
                      </Button>
                    ) : null}
                    <Button
                      variant="danger"
                      size="small"
                      onClick={() => {
                        setForceStage(false);
                        setRemoveTarget(worktree);
                      }}
                      disabled={pending || worktree.locked}
                    >
                      <Trash2 size={13} aria-hidden />
                      {t('manager.remove')}
                    </Button>
                  </div>
                </article>
              );
            })}
          </div>
        </div>
      </Modal>

      <InputDialog
        isOpen={!!branchTarget}
        onClose={() => setBranchTarget(null)}
        onConfirm={branch => {
          const target = branchTarget;
          if (!target) return;
          void runMutation(
            target,
            () => worktreeAPI.createBranch(
              projectWorkspacePath,
              target.worktreeId,
              branch,
              requestId(),
            ),
            t('manager.branchCreated'),
          );
        }}
        title={t('manager.branchDialog.title')}
        description={t('manager.branchDialog.description')}
        defaultValue={`${branchPrefix}${branchTarget?.worktreeId.slice(0, 8) ?? ''}`}
        confirmText={t('manager.createBranch')}
        validator={value => value.trim() ? null : t('manager.branchDialog.required')}
      />

      <ConfirmDialog
        isOpen={!!removeTarget}
        onClose={() => {
          setRemoveTarget(null);
          setForceStage(false);
        }}
        onConfirm={() => void confirmRemove()}
        title={
          forceStage
            ? t('manager.removeDialog.forceTitle')
            : t('manager.removeDialog.title')
        }
        type={forceStage ? 'error' : 'warning'}
        message={
          <div className="bitfun-worktree-manager__risk-list">
            <p>
              {removeTarget?.runningSessionCount
                ? t('manager.removeDialog.blocked')
                : forceStage
                  ? t('manager.removeDialog.forceMessage')
                  : t('manager.removeDialog.message')}
            </p>
            {removeRisks.length > 0 ? (
              <ul>
                {removeRisks.map(risk => <li key={risk}>{risk}</li>)}
              </ul>
            ) : (
              <span>{t('manager.risks.clean')}</span>
            )}
          </div>
        }
        preview={removeTarget?.path}
        confirmText={
          removeTarget?.runningSessionCount
            ? t('actions.close')
            : forceStage
              ? t('manager.removeDialog.forceConfirm')
              : t('manager.remove')
        }
        cancelText={t('actions.cancel')}
        confirmDanger={!removeTarget?.runningSessionCount}
        showCancel={!removeTarget?.runningSessionCount}
      />
    </>
  );
};

export default WorktreeManagerModal;
