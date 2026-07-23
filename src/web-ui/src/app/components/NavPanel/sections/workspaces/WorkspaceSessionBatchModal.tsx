import React, { useCallback, useEffect, useMemo, useState } from 'react';
import { Archive, Bot, ClipboardList, Code2, FolderKanban, Loader2, Trash2 } from 'lucide-react';
import { Button, Checkbox, Modal } from '@/component-library';
import { useI18n } from '@/infrastructure/i18n';
import { sessionAPI } from '@/infrastructure/api/service-api/SessionAPI';
import type { SessionMetadata } from '@/shared/types/session-history';
import { sessionBelongsToWorkspaceNavRow, compareSessionMetadataForDisplay } from '@/flow_chat/utils/sessionOrdering';
import { deriveSessionRelationshipFromMetadata, resolveSessionRelationship } from '@/flow_chat/utils/sessionMetadata';
import { flowChatManager } from '@/flow_chat/services/FlowChatManager';
import { confirmWarning } from '@/component-library/components/ConfirmDialog/confirmService';
import { notificationService } from '@/shared/notification-system';
import { createLogger } from '@/shared/utils/logger';
import './WorkspaceSessionBatchModal.scss';

interface WorkspaceSessionBatchModalProps {
  isOpen: boolean;
  onClose: () => void;
  workspacePath: string;
  workspaceLabel: string;
  remoteConnectionId?: string | null;
  remoteSshHost?: string | null;
}

type BatchActionKind = 'archive' | 'delete' | null;

interface SessionBatchItem {
  metadata: SessionMetadata;
  parentSessionId?: string | null;
  displayAsChild: boolean;
}

const log = createLogger('WorkspaceSessionBatchModal');

type SessionMode = 'code' | 'cowork' | 'claw';

function resolveSessionMode(agentType: string | undefined): SessionMode {
  const normalized = agentType?.trim().toLowerCase() ?? '';
  if (normalized === 'cowork') {
    return 'cowork';
  }
  if (normalized === 'claw') {
    return 'claw';
  }
  return 'code';
}

function buildSessionBatchItems(sessions: SessionMetadata[]): SessionBatchItem[] {
  const sortedSessions = [...sessions].sort(compareSessionMetadataForDisplay);
  const knownIds = new Set(sortedSessions.map(session => session.sessionId));
  return sortedSessions.map(metadata => {
    const relationship = resolveSessionRelationship(deriveSessionRelationshipFromMetadata(metadata));
    return {
      metadata,
      parentSessionId: relationship.parentSessionId,
      displayAsChild: Boolean(relationship.parentSessionId && knownIds.has(relationship.parentSessionId)),
    };
  });
}

function getDeletionPlan(selectedIds: Set<string>, sessions: SessionBatchItem[]): { rootIds: string[]; allIds: string[] } {
  const parentById = new Map<string, string | null | undefined>();
  const childrenByParent = new Map<string, string[]>();

  sessions.forEach(session => {
    const sessionId = session.metadata.sessionId;
    parentById.set(sessionId, session.parentSessionId);
    if (session.parentSessionId) {
      const siblings = childrenByParent.get(session.parentSessionId) || [];
      siblings.push(sessionId);
      childrenByParent.set(session.parentSessionId, siblings);
    }
  });

  const rootIds = Array.from(selectedIds).filter(sessionId => {
    let cursor = parentById.get(sessionId);
    while (cursor) {
      if (selectedIds.has(cursor)) {
        return false;
      }
      cursor = parentById.get(cursor);
    }
    return true;
  });

  const allIds = new Set<string>();
  const stack = [...rootIds];
  while (stack.length > 0) {
    const sessionId = stack.pop()!;
    if (allIds.has(sessionId)) {
      continue;
    }
    allIds.add(sessionId);
    const children = childrenByParent.get(sessionId) || [];
    children.forEach(childId => stack.push(childId));
  }

  return {
    rootIds,
    allIds: Array.from(allIds),
  };
}

const WorkspaceSessionBatchModal: React.FC<WorkspaceSessionBatchModalProps> = ({
  isOpen,
  onClose,
  workspacePath,
  workspaceLabel,
  remoteConnectionId = null,
  remoteSshHost = null,
}) => {
  const { t, formatDate, formatRelativeTime } = useI18n('common');
  const [sessions, setSessions] = useState<SessionBatchItem[]>([]);
  const [selectedSessionIds, setSelectedSessionIds] = useState<Set<string>>(new Set());
  const [isLoading, setIsLoading] = useState(false);
  const [actionKind, setActionKind] = useState<BatchActionKind>(null);
  const [loadFailed, setLoadFailed] = useState(false);

  const loadSessions = useCallback(async () => {
    setIsLoading(true);
    setLoadFailed(false);
    try {
      const metadataList = await sessionAPI.listSessions(
        workspacePath,
        remoteConnectionId || undefined,
        remoteSshHost || undefined
      );
      const filtered = metadataList.filter(metadata => {
        if (metadata.status === 'archived') {
          return false;
        }
        if (
          !sessionBelongsToWorkspaceNavRow(
            metadata,
            workspacePath,
            remoteConnectionId,
            remoteSshHost
          )
        ) {
          return false;
        }
        const relationship = resolveSessionRelationship(deriveSessionRelationshipFromMetadata(metadata));
        return !relationship.isSubagent;
      });
      setSessions(buildSessionBatchItems(filtered));
      setSelectedSessionIds(new Set());
    } catch (error) {
      log.error('Failed to load workspace sessions for batch management', { error, workspacePath });
      setLoadFailed(true);
    } finally {
      setIsLoading(false);
    }
  }, [remoteConnectionId, remoteSshHost, workspacePath]);

  useEffect(() => {
    if (!isOpen) {
      setSelectedSessionIds(new Set());
      setActionKind(null);
      setLoadFailed(false);
      return;
    }
    void loadSessions();
  }, [isOpen, loadSessions]);

  const allSessionIds = useMemo(
    () => sessions.map(session => session.metadata.sessionId),
    [sessions]
  );
  const selectedCount = selectedSessionIds.size;
  const allSelected = allSessionIds.length > 0 && selectedCount === allSessionIds.length;
  const partiallySelected = selectedCount > 0 && selectedCount < allSessionIds.length;
  const isBusy = isLoading || actionKind !== null;
  const hasSessions = sessions.length > 0;

  const toggleSessionSelection = useCallback((sessionId: string) => {
    setSelectedSessionIds(prev => {
      const next = new Set(prev);
      if (next.has(sessionId)) {
        next.delete(sessionId);
      } else {
        next.add(sessionId);
      }
      return next;
    });
  }, []);

  const handleToggleSelectAll = useCallback(() => {
    setSelectedSessionIds(prev => {
      if (prev.size === allSessionIds.length) {
        return new Set();
      }
      return new Set(allSessionIds);
    });
  }, [allSessionIds]);

  const handleInvertSelection = useCallback(() => {
    setSelectedSessionIds(prev => new Set(allSessionIds.filter(sessionId => !prev.has(sessionId))));
  }, [allSessionIds]);

  const refreshWorkspaceSessions = useCallback(async () => {
    await flowChatManager.refreshWorkspaceSessions({
      rootPath: workspacePath,
      connectionId: remoteConnectionId || undefined,
      sshHost: remoteSshHost || undefined,
    });
  }, [remoteConnectionId, remoteSshHost, workspacePath]);

  const handleArchiveSelected = useCallback(async () => {
    if (selectedCount === 0) {
      return;
    }
    const confirmed = await confirmWarning(
      t('nav.sessions.bulkArchiveConfirmTitle'),
      t('nav.sessions.bulkArchiveConfirmMessage', { count: selectedCount })
    );
    if (!confirmed) {
      return;
    }

    const selectedIds = Array.from(selectedSessionIds);
    setActionKind('archive');
    try {
      const results = await Promise.allSettled(
        selectedIds.map(sessionId => flowChatManager.archiveChatSession(sessionId))
      );
      const successCount = results.filter(result => result.status === 'fulfilled').length;
      if (successCount > 0) {
        await refreshWorkspaceSessions();
        window.dispatchEvent(new CustomEvent('bitfun:session-archived'));
        notificationService.success(t('nav.sessions.archivedAll', { count: successCount }), { duration: 3000 });
      }
      if (successCount !== selectedIds.length) {
        notificationService.error(t('nav.sessions.bulkArchiveFailed'), { duration: 4000 });
      }
      await loadSessions();
    } catch (error) {
      log.error('Failed to archive selected sessions', { error, workspacePath });
      notificationService.error(t('nav.sessions.bulkArchiveFailed'), { duration: 4000 });
    } finally {
      setActionKind(null);
    }
  }, [
    loadSessions,
    refreshWorkspaceSessions,
    selectedCount,
    selectedSessionIds,
    t,
    workspacePath,
  ]);

  const handleDeleteSelected = useCallback(async () => {
    if (selectedCount === 0) {
      return;
    }
    const confirmed = await confirmWarning(
      t('nav.sessions.bulkDeleteConfirmTitle'),
      t('nav.sessions.bulkDeleteConfirmMessage', { count: selectedCount })
    );
    if (!confirmed) {
      return;
    }

    const deletionPlan = getDeletionPlan(selectedSessionIds, sessions);
    setActionKind('delete');
    try {
      const successIds = new Set<string>();

      for (const rootId of deletionPlan.rootIds) {
        const cascadeIds = getDeletionPlan(new Set([rootId]), sessions).allIds;
        try {
          await flowChatManager.deleteChatSession(rootId);
          cascadeIds.forEach(id => successIds.add(id));
        } catch (error) {
          log.error('Failed to delete selected root session', {
            error,
            rootSessionId: rootId,
            workspacePath,
          });
        }
      }

      if (successIds.size > 0) {
        await refreshWorkspaceSessions();
        notificationService.success(t('nav.sessions.deletedSelected', { count: successIds.size }), { duration: 3000 });
      }
      if (successIds.size !== deletionPlan.allIds.length) {
        notificationService.error(t('nav.sessions.bulkDeleteFailed'), { duration: 4000 });
      }
      await loadSessions();
    } catch (error) {
      log.error('Failed to delete selected sessions', { error, workspacePath });
      notificationService.error(t('nav.sessions.bulkDeleteFailed'), { duration: 4000 });
    } finally {
      setActionKind(null);
    }
  }, [
    loadSessions,
    refreshWorkspaceSessions,
    selectedCount,
    selectedSessionIds,
    sessions,
    t,
    workspacePath,
  ]);

  return (
    <Modal
      isOpen={isOpen}
      onClose={isBusy ? () => {} : onClose}
      title={t('nav.sessions.manage')}
      size="xlarge"
      contentClassName="modal__content--fill-flex workspace-session-batch-modal__content-shell"
      closeOnOverlayClick={!isBusy}
    >
      <div className="workspace-session-batch-modal">
        <div className="workspace-session-batch-modal__hero">
          <div className="workspace-session-batch-modal__hero-icon">
            <FolderKanban size={18} />
          </div>
          <div className="workspace-session-batch-modal__hero-copy">
            <div className="workspace-session-batch-modal__workspace">{workspaceLabel}</div>
            <div className="workspace-session-batch-modal__description">
              {t('nav.sessions.batchManageDescription')}
            </div>
          </div>
        </div>

        <div className="workspace-session-batch-modal__toolbar">
          <div className="workspace-session-batch-modal__toolbar-main">
            <Checkbox
              checked={allSelected}
              indeterminate={partiallySelected}
              onChange={() => { handleToggleSelectAll(); }}
              disabled={isBusy || allSessionIds.length === 0}
              label={allSelected ? t('actions.deselectAll') : t('actions.selectAll')}
            />
            {selectedCount > 0 ? (
              <div className="workspace-session-batch-modal__toolbar-actions">
                <Button
                  type="button"
                  variant="ghost"
                  size="small"
                  onClick={handleInvertSelection}
                  disabled={isBusy || allSessionIds.length === 0}
                >
                  {t('actions.invertSelection')}
                </Button>
              </div>
            ) : null}
            <div className="workspace-session-batch-modal__toolbar-summary">
              {hasSessions
                ? t('nav.sessions.batchSelectionSummary', { count: selectedCount, total: sessions.length })
                : t('nav.sessions.noSessionsToManage')}
            </div>
          </div>
        </div>

        <div className="workspace-session-batch-modal__list">
          {isLoading ? (
            <div className="workspace-session-batch-modal__state">
              <Loader2 size={16} className="workspace-session-batch-modal__spinner" />
              <span>{t('nav.sessions.loading')}</span>
            </div>
          ) : loadFailed ? (
            <div className="workspace-session-batch-modal__state is-error">
              <span>{t('nav.sessions.batchLoadFailed')}</span>
              <Button type="button" variant="secondary" size="small" onClick={() => { void loadSessions(); }}>
                {t('actions.retry')}
              </Button>
            </div>
          ) : sessions.length === 0 ? (
            <div className="workspace-session-batch-modal__state">
              <span>{t('nav.sessions.noSessionsToManage')}</span>
            </div>
          ) : (
            sessions.map(({ metadata, displayAsChild }) => {
              const isSelected = selectedSessionIds.has(metadata.sessionId);
              const sessionMode = resolveSessionMode(metadata.agentType);
              const SessionIcon =
                sessionMode === 'cowork'
                  ? ClipboardList
                  : sessionMode === 'claw'
                    ? Bot
                    : Code2;
              return (
                <label
                  key={metadata.sessionId}
                  className={`workspace-session-batch-modal__row${displayAsChild ? ' is-child' : ''}${isSelected ? ' is-selected' : ''}`}
                >
                  <div className="workspace-session-batch-modal__row-check">
                    <Checkbox
                      checked={isSelected}
                      onChange={() => { toggleSessionSelection(metadata.sessionId); }}
                      disabled={isBusy}
                    />
                  </div>
                  <div className={`workspace-session-batch-modal__row-icon is-${sessionMode}`}>
                    <SessionIcon size={15} />
                  </div>
                  <div className="workspace-session-batch-modal__row-content">
                    <div className="workspace-session-batch-modal__row-head">
                      <div className="workspace-session-batch-modal__row-title">
                        {metadata.sessionName || t('nav.sessions.untitled')}
                      </div>
                      <div
                        className="workspace-session-batch-modal__row-updated"
                        title={formatDate(metadata.lastActiveAt, {
                          year: 'numeric',
                          month: 'short',
                          day: 'numeric',
                          hour: '2-digit',
                          minute: '2-digit',
                        })}
                      >
                        {formatRelativeTime(metadata.lastActiveAt)}
                      </div>
                    </div>
                    {displayAsChild ? (
                      <div className="workspace-session-batch-modal__row-meta">
                        <span className="workspace-session-batch-modal__pill">
                          {t('nav.sessions.batchChildSession')}
                        </span>
                      </div>
                    ) : null}
                  </div>
                </label>
              );
            })
          )}
        </div>

        <div className="workspace-session-batch-modal__footer">
          <Button type="button" variant="ghost" onClick={onClose} disabled={isBusy}>
            {t('actions.cancel')}
          </Button>
          <Button
            type="button"
            variant="secondary"
            onClick={() => { void handleArchiveSelected(); }}
            disabled={isBusy || selectedCount === 0}
            isLoading={actionKind === 'archive'}
          >
            <Archive size={14} />
            <span>{t('nav.sessions.archiveSelected')}</span>
          </Button>
          <Button
            type="button"
            variant="danger"
            onClick={() => { void handleDeleteSelected(); }}
            disabled={isBusy || selectedCount === 0}
            isLoading={actionKind === 'delete'}
          >
            <Trash2 size={14} />
            <span>{t('nav.sessions.deleteSelected')}</span>
          </Button>
        </div>
      </div>
    </Modal>
  );
};

export default WorkspaceSessionBatchModal;
