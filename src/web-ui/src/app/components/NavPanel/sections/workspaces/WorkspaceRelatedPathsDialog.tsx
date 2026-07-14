import React, { useEffect, useMemo, useState } from 'react';
import { Button, Input, Modal, Textarea } from '@/component-library';
import { useI18n } from '@/infrastructure/i18n';
import { useWorkspaceContext } from '@/infrastructure/contexts/WorkspaceContext';
import { sshApi } from '@/features/ssh-remote/sshApi';
import RemoteFileBrowser from '@/features/ssh-remote/RemoteFileBrowser';
import { createLogger } from '@/shared/utils/logger';
import { isRemoteWorkspace, type RelatedPath, type WorkspaceInfo } from '@/shared/types';
import { FolderOpen, Link2, Plus, Trash2 } from 'lucide-react';
import './WorkspaceRelatedPathsDialog.scss';

const log = createLogger('WorkspaceRelatedPathsDialog');

interface WorkspaceRelatedPathsDialogProps {
  workspace: WorkspaceInfo;
  isOpen: boolean;
  onClose: () => void;
}

interface DraftRelatedPath {
  id: string;
  path: string;
  description: string;
}

function createDraft(path?: Partial<RelatedPath>): DraftRelatedPath {
  return {
    id: `${Date.now()}-${Math.random().toString(36).slice(2, 8)}`,
    path: path?.path ?? '',
    description: path?.description ?? '',
  };
}

function normalizeDrafts(drafts: DraftRelatedPath[]): RelatedPath[] {
  return drafts.map(draft => ({
    path: draft.path.trim(),
    ...(draft.description.trim()
      ? { description: draft.description.trim() }
      : {}),
  }));
}

export const WorkspaceRelatedPathsDialog: React.FC<WorkspaceRelatedPathsDialogProps> = ({
  workspace,
  isOpen,
  onClose,
}) => {
  const { t } = useI18n('common');
  const { updateWorkspaceRelatedPaths } = useWorkspaceContext();
  const [drafts, setDrafts] = useState<DraftRelatedPath[]>([]);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [browsingIndex, setBrowsingIndex] = useState<number | null>(null);
  const [remoteHomePath, setRemoteHomePath] = useState<string | undefined>(undefined);

  const remoteWorkspace = isRemoteWorkspace(workspace);
  const connectionId = workspace.connectionId?.trim() || undefined;
  const relatedPathCount = drafts.length;
  const scopeDescription = remoteWorkspace
    ? t('nav.workspaces.relatedPaths.dialog.remoteScope', {
        connectionName: workspace.connectionName || workspace.name,
      })
    : t('nav.workspaces.relatedPaths.dialog.localScope');

  useEffect(() => {
    if (!isOpen) {
      return;
    }

    setDrafts((workspace.relatedPaths ?? []).map(path => createDraft(path)));
    setSaving(false);
    setError(null);
  }, [isOpen, workspace.relatedPaths]);

  useEffect(() => {
    if (!isOpen || !remoteWorkspace || !connectionId) {
      setRemoteHomePath(undefined);
      return;
    }

    let cancelled = false;
    void sshApi
      .getServerInfo(connectionId)
      .then(info => {
        if (!cancelled) {
          setRemoteHomePath(info?.homeDir?.trim() || undefined);
        }
      })
      .catch(fetchError => {
        log.warn('Failed to load remote server info for related directories', {
          workspaceId: workspace.id,
          error: fetchError,
        });
      });

    return () => {
      cancelled = true;
    };
  }, [connectionId, isOpen, remoteWorkspace, workspace.id]);

  const normalizedDrafts = useMemo(() => normalizeDrafts(drafts), [drafts]);
  const hasInvalidDraft = normalizedDrafts.some(draft => !draft.path);
  const isUnchanged = JSON.stringify(normalizedDrafts) === JSON.stringify(workspace.relatedPaths ?? []);

  const setDraftValue = (
    draftId: string,
    field: 'path' | 'description',
    value: string
  ) => {
    setDrafts(current =>
      current.map(draft => (draft.id === draftId ? { ...draft, [field]: value } : draft))
    );
    setError(null);
  };

  const handleAddDraft = () => {
    setDrafts(current => [...current, createDraft()]);
    setError(null);
  };

  const handleRemoveDraft = (draftId: string) => {
    setDrafts(current => current.filter(draft => draft.id !== draftId));
    setError(null);
  };

  const handleSelectLocalDirectory = async (index: number) => {
    try {
      const { pickWorkspaceDirectory } = await import(
        '@/infrastructure/peer-device/pickWorkspaceDirectory'
      );
      const selected = await pickWorkspaceDirectory({
        title: t('nav.workspaces.relatedPaths.dialog.selectDirectoryTitle'),
        defaultPath: drafts[index]?.path || workspace.rootPath,
      });

      if (selected?.trim()) {
        setDraftValue(drafts[index].id, 'path', selected);
      }
    } catch (selectionError) {
      log.error('Failed to select related directory', { workspaceId: workspace.id, error: selectionError });
      setError(t('nav.workspaces.relatedPaths.messages.selectFailed'));
    }
  };

  const handleSave = async () => {
    if (hasInvalidDraft) {
      setError(t('nav.workspaces.relatedPaths.validation.pathRequired'));
      return;
    }

    setSaving(true);
    setError(null);
    try {
      await updateWorkspaceRelatedPaths(workspace.id, normalizedDrafts);
      onClose();
    } catch (saveError) {
      log.error('Failed to save related directories', { workspaceId: workspace.id, error: saveError });
      setError(
        saveError instanceof Error
          ? saveError.message
          : t('nav.workspaces.relatedPaths.messages.saveFailed')
      );
    } finally {
      setSaving(false);
    }
  };

  return (
    <>
      <Modal
        isOpen={isOpen}
        onClose={() => {
          if (!saving) {
            onClose();
          }
        }}
        title={t('nav.workspaces.relatedPaths.dialog.title')}
        size="large"
        contentInset
        contentClassName="workspace-related-paths-dialog__modal"
      >
        <div className="workspace-related-paths-dialog">
          <div className="workspace-related-paths-dialog__intro">
            <div className="workspace-related-paths-dialog__intro-icon">
              <Link2 size={18} />
            </div>
            <div className="workspace-related-paths-dialog__intro-copy">
              <div className="workspace-related-paths-dialog__intro-title">
                {t('nav.workspaces.relatedPaths.dialog.heading')}
              </div>
              <div className="workspace-related-paths-dialog__intro-text">
                {t('nav.workspaces.relatedPaths.dialog.description')}
              </div>
              <div className="workspace-related-paths-dialog__scope">
                {scopeDescription}
              </div>
            </div>
            <div className="workspace-related-paths-dialog__intro-badge">
              <span className="workspace-related-paths-dialog__intro-badge-label">
                {t('nav.workspaces.relatedPaths.badge', { count: relatedPathCount })}
              </span>
            </div>
          </div>

          {drafts.length === 0 ? (
            <div className="workspace-related-paths-dialog__empty">
              {t('nav.workspaces.relatedPaths.dialog.empty')}
            </div>
          ) : (
            <div className="workspace-related-paths-dialog__list">
              {drafts.map((draft, index) => (
                <div key={draft.id} className="workspace-related-paths-dialog__card">
                  <div className="workspace-related-paths-dialog__card-header">
                    <span className="workspace-related-paths-dialog__card-index">
                      {t('nav.workspaces.relatedPaths.dialog.itemLabel', { index: index + 1 })}
                    </span>
                    <button
                      type="button"
                      className="workspace-related-paths-dialog__remove"
                      onClick={() => handleRemoveDraft(draft.id)}
                      aria-label={t('actions.remove')}
                    >
                      <Trash2 size={14} />
                    </button>
                  </div>

                  <div className="workspace-related-paths-dialog__path-row">
                    <Input
                      className="workspace-related-paths-dialog__path-input"
                      value={draft.path}
                      onChange={event => setDraftValue(draft.id, 'path', event.target.value)}
                      placeholder={t('nav.workspaces.relatedPaths.dialog.pathPlaceholder')}
                      disabled={saving}
                      variant="filled"
                      size="small"
                    />
                    <Button
                      type="button"
                      className="workspace-related-paths-dialog__select"
                      variant="secondary"
                      size="small"
                      onClick={() =>
                        remoteWorkspace
                          ? setBrowsingIndex(index)
                          : void handleSelectLocalDirectory(index)
                      }
                      disabled={saving || (remoteWorkspace && !connectionId)}
                    >
                      <FolderOpen size={14} />
                      <span>{t('actions.select')}</span>
                    </Button>
                  </div>

                  <Textarea
                    className="workspace-related-paths-dialog__description"
                    value={draft.description}
                    onChange={event => setDraftValue(draft.id, 'description', event.target.value)}
                    placeholder={t('nav.workspaces.relatedPaths.dialog.descriptionPlaceholder')}
                    disabled={saving}
                    autoResize
                    rows={2}
                    variant="outlined"
                  />
                </div>
              ))}
            </div>
          )}

          {error ? (
            <div className="workspace-related-paths-dialog__error" role="alert">
              {error}
            </div>
          ) : null}

          <div className="workspace-related-paths-dialog__footer">
            <Button
              type="button"
              className="workspace-related-paths-dialog__add"
              variant="dashed"
              size="small"
              onClick={handleAddDraft}
              disabled={saving}
            >
              <Plus size={14} />
              <span>{t('nav.workspaces.relatedPaths.dialog.add')}</span>
            </Button>

            <div className="workspace-related-paths-dialog__footer-actions">
              <Button
                type="button"
                variant="secondary"
                size="small"
                onClick={onClose}
                disabled={saving}
              >
                {t('actions.cancel')}
              </Button>
              <Button
                type="button"
                variant="primary"
                size="small"
                onClick={() => void handleSave()}
                disabled={saving || hasInvalidDraft || isUnchanged}
              >
                {saving ? t('status.saving') : t('actions.save')}
              </Button>
            </div>
          </div>
        </div>
      </Modal>

      {remoteWorkspace && connectionId && browsingIndex !== null ? (
        <RemoteFileBrowser
          connectionId={connectionId}
          initialPath={drafts[browsingIndex]?.path || workspace.rootPath}
          homePath={remoteHomePath}
          selectDirectoriesOnly
          onSelect={(path: string) => {
            setDraftValue(drafts[browsingIndex].id, 'path', path);
            setBrowsingIndex(null);
          }}
          onCancel={() => setBrowsingIndex(null)}
        />
      ) : null}
    </>
  );
};

export default WorkspaceRelatedPathsDialog;
