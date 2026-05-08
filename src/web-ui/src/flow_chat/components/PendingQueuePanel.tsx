/**
 * Pending queue panel
 *
 * Renders the per-session list of "queued" user messages above the chat input.
 * Each card supports inline edit, "send now" (mid-turn steering), and delete.
 *
 * UX notes:
 * - Click anywhere on the preview text to start editing.
 * - Cmd/Ctrl+Enter saves the edit; Esc cancels.
 * - Clicking "send now" eagerly inserts a UserSteeringBubble into the live
 *   round so the user sees feedback instantly; the backend confirmation event
 *   is deduped via `steeringId`.
 */

import { useCallback, useEffect, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import {
  Pencil,
  Send,
  Trash2,
  Check,
  X as XIcon,
  Inbox,
  Loader2,
} from 'lucide-react';
import { Tooltip, IconButton } from '@/component-library';
import { agentAPI } from '@/infrastructure/api/service-api/AgentAPI';
import { stateMachineManager } from '../state-machine';
import { pendingQueueManager } from '../services/flow-chat-manager/PendingQueueModule';
import { FlowChatManager } from '../services/FlowChatManager';
import { insertSteeringItemIfAbsent } from '../services/flow-chat-manager/EventHandlerModule';
import { notificationService } from '../../shared/notification-system';
import { createLogger } from '@/shared/utils/logger';
import type { QueuedMessage } from '../types/flow-chat';
import './PendingQueuePanel.scss';

const log = createLogger('PendingQueuePanel');

interface PendingQueuePanelProps {
  sessionId: string | undefined;
  className?: string;
}

export function PendingQueuePanel({ sessionId, className }: PendingQueuePanelProps): JSX.Element | null {
  const { t } = useTranslation('flow-chat');
  const [items, setItems] = useState<QueuedMessage[]>(() =>
    sessionId ? pendingQueueManager.list(sessionId) : [],
  );
  const [editingId, setEditingId] = useState<string | null>(null);
  const [editingDraft, setEditingDraft] = useState('');

  useEffect(() => {
    if (!sessionId) {
      setItems([]);
      return;
    }
    setItems(pendingQueueManager.list(sessionId));
    const unsubscribe = pendingQueueManager.subscribe((sid, snapshot) => {
      if (sid === sessionId) setItems(snapshot);
    });
    return unsubscribe;
  }, [sessionId]);

  const handleEditStart = useCallback((item: QueuedMessage) => {
    setEditingId(item.id);
    setEditingDraft(item.displayMessage ?? item.content);
  }, []);

  const handleEditCancel = useCallback(() => {
    setEditingId(null);
    setEditingDraft('');
  }, []);

  const handleEditSave = useCallback(
    (item: QueuedMessage) => {
      if (!sessionId) return;
      const trimmed = editingDraft.trim();
      if (!trimmed) {
        notificationService.warning(t('pendingQueue.errors.emptyContent'), { duration: 3000 });
        return;
      }
      pendingQueueManager.update(sessionId, item.id, {
        content: trimmed,
        displayMessage: trimmed,
      });
      setEditingId(null);
      setEditingDraft('');
    },
    [editingDraft, sessionId, t],
  );

  const handleDelete = useCallback(
    (item: QueuedMessage) => {
      if (!sessionId) return;
      pendingQueueManager.remove(sessionId, item.id);
    },
    [sessionId],
  );

  const handleSendNow = useCallback(
    async (item: QueuedMessage) => {
      if (!sessionId) return;
      const machine = stateMachineManager.get(sessionId);
      const ctx = machine?.getContext();
      const dialogTurnId = ctx?.currentDialogTurnId ?? null;

      if (!dialogTurnId) {
        // Turn already finished — fall back to the regular drain path so the
        // item starts a new turn instead.
        log.info('Send now fallback: no active dialog turn, using drain path', {
          sessionId,
          itemId: item.id,
        });
        try {
          // Move this specific item to the head, then trigger drain.
          const allItems = pendingQueueManager.list(sessionId);
          if (allItems.length > 1 && allItems[0]?.id !== item.id) {
            pendingQueueManager.clear(sessionId);
            pendingQueueManager.enqueue({
              sessionId,
              content: item.content,
              displayMessage: item.displayMessage,
              agentType: item.agentType,
              imageContexts: item.imageContexts,
              imageDisplayData: item.imageDisplayData,
            });
            for (const other of allItems) {
              if (other.id === item.id) continue;
              pendingQueueManager.enqueue({
                sessionId,
                content: other.content,
                displayMessage: other.displayMessage,
                agentType: other.agentType,
                imageContexts: other.imageContexts,
                imageDisplayData: other.imageDisplayData,
              });
            }
          }
          await FlowChatManager.getInstance().drainPendingQueueForSession(sessionId);
        } catch (err) {
          log.error('Send now fallback failed', { sessionId, itemId: item.id, err });
          notificationService.error(t('pendingQueue.errors.sendNowFailed'), { duration: 4000 });
        }
        return;
      }

      pendingQueueManager.setStatus(sessionId, item.id, 'sending_now');
      try {
        const resp = await agentAPI.steerDialogTurn({
          sessionId,
          dialogTurnId,
          content: item.content,
          displayContent: item.displayMessage ?? item.content,
        });
        // Optimistically render the steering bubble in the running round so the
        // user sees their message land immediately. The backend
        // `UserSteeringInjected` event will dedupe by the same `steeringId`.
        if (resp?.steeringId) {
          try {
            insertSteeringItemIfAbsent({
              sessionId,
              turnId: dialogTurnId,
              steeringId: resp.steeringId,
              content: item.displayMessage ?? item.content,
              status: 'pending',
            });
          } catch (renderErr) {
            log.warn('Optimistic steering render failed', { renderErr });
          }
        }
        pendingQueueManager.remove(sessionId, item.id);
      } catch (err) {
        log.error('Send now (steering) failed', { sessionId, itemId: item.id, err });
        pendingQueueManager.setStatus(sessionId, item.id, 'queued');
        notificationService.error(t('pendingQueue.errors.sendNowFailed'), { duration: 4000 });
      }
    },
    [sessionId, t],
  );

  const visibleItems = useMemo(() => items, [items]);

  if (!sessionId || visibleItems.length === 0) {
    return null;
  }

  return (
    <div
      className={`bitfun-pending-queue-panel ${className ?? ''}`.trim()}
      data-testid="pending-queue-panel"
      onClick={e => {
        e.stopPropagation();
      }}
    >
      <div className="bitfun-pending-queue-panel__header">
        <Inbox size={12} className="bitfun-pending-queue-panel__header-icon" />
        <span className="bitfun-pending-queue-panel__title">
          {t('pendingQueue.title', { count: visibleItems.length })}
          <span className="bitfun-pending-queue-panel__hint">
            {' · '}
            {t('pendingQueue.hint')}
          </span>
        </span>
      </div>
      <ul className="bitfun-pending-queue-panel__list">
        {visibleItems.map((item, index) => {
          const isEditing = editingId === item.id;
          const isSendingNow = item.status === 'sending_now';
          const isSending = item.status === 'sending' || isSendingNow;
          const isFailed = item.status === 'failed' || (item.retryCount ?? 0) > 0;
          const previewText = item.displayMessage ?? item.content;
          const itemClass = [
            'bitfun-pending-queue-panel__item',
            isEditing && 'bitfun-pending-queue-panel__item--editing',
            isSending && 'bitfun-pending-queue-panel__item--sending',
            isFailed && 'bitfun-pending-queue-panel__item--failed',
          ]
            .filter(Boolean)
            .join(' ');
          return (
            <li key={item.id} className={itemClass}>
              <span className="bitfun-pending-queue-panel__index">{index + 1}</span>
              <div className="bitfun-pending-queue-panel__content">
                {isEditing ? (
                  <>
                    <textarea
                      className="bitfun-pending-queue-panel__editor"
                      value={editingDraft}
                      autoFocus
                      rows={Math.min(6, Math.max(2, editingDraft.split('\n').length))}
                      onChange={e => setEditingDraft(e.target.value)}
                      onKeyDown={e => {
                        if (e.key === 'Enter' && (e.metaKey || e.ctrlKey)) {
                          e.preventDefault();
                          handleEditSave(item);
                        } else if (e.key === 'Escape') {
                          e.preventDefault();
                          handleEditCancel();
                        }
                      }}
                    />
                    <div className="bitfun-pending-queue-panel__editor-hint">
                      {t('pendingQueue.editorHint')}
                    </div>
                  </>
                ) : isSendingNow ? (
                  <>
                    <div
                      className="bitfun-pending-queue-panel__preview"
                      title={previewText}
                    >
                      {previewText || (
                        <span className="bitfun-pending-queue-panel__preview-empty">
                          {t('pendingQueue.emptyPlaceholder')}
                        </span>
                      )}
                    </div>
                    <div className="bitfun-pending-queue-panel__sending-label">
                      <Loader2 size={11} />
                      {t('pendingQueue.statusSending')}
                    </div>
                  </>
                ) : (
                  <>
                    <div
                      className="bitfun-pending-queue-panel__preview"
                      title={t('pendingQueue.actions.edit')}
                      role="button"
                      tabIndex={0}
                      onClick={() => handleEditStart(item)}
                      onKeyDown={e => {
                        if (e.key === 'Enter' || e.key === ' ') {
                          e.preventDefault();
                          handleEditStart(item);
                        }
                      }}
                    >
                      {previewText || (
                        <span className="bitfun-pending-queue-panel__preview-empty">
                          {t('pendingQueue.emptyPlaceholder')}
                        </span>
                      )}
                    </div>
                    {isFailed && (
                      <div className="bitfun-pending-queue-panel__failed-label">
                        {t('pendingQueue.statusFailed')}
                      </div>
                    )}
                  </>
                )}
              </div>
              <div className="bitfun-pending-queue-panel__actions">
                {isEditing ? (
                  <>
                    <Tooltip content={t('pendingQueue.actions.saveEdit')}>
                      <IconButton
                        size="small"
                        className="bitfun-pending-queue-panel__btn bitfun-pending-queue-panel__btn--primary"
                        onClick={() => handleEditSave(item)}
                        aria-label={t('pendingQueue.actions.saveEdit')}
                      >
                        <Check size={14} />
                      </IconButton>
                    </Tooltip>
                    <Tooltip content={t('pendingQueue.actions.cancelEdit')}>
                      <IconButton
                        size="small"
                        className="bitfun-pending-queue-panel__btn"
                        onClick={handleEditCancel}
                        aria-label={t('pendingQueue.actions.cancelEdit')}
                      >
                        <XIcon size={14} />
                      </IconButton>
                    </Tooltip>
                  </>
                ) : (
                  <>
                    <Tooltip content={t('pendingQueue.actions.edit')}>
                      <IconButton
                        size="small"
                        className="bitfun-pending-queue-panel__btn"
                        disabled={isSending}
                        onClick={() => handleEditStart(item)}
                        aria-label={t('pendingQueue.actions.edit')}
                      >
                        <Pencil size={14} />
                      </IconButton>
                    </Tooltip>
                    <Tooltip content={t('pendingQueue.tooltip.sendNow')}>
                      <IconButton
                        size="small"
                        className="bitfun-pending-queue-panel__btn bitfun-pending-queue-panel__btn--primary"
                        disabled={isSending}
                        onClick={() => {
                          void handleSendNow(item);
                        }}
                        aria-label={t('pendingQueue.actions.sendNow')}
                      >
                        {isSendingNow ? (
                          <Loader2 size={14} className="bitfun-pending-queue-panel__spin" />
                        ) : (
                          <Send size={14} />
                        )}
                      </IconButton>
                    </Tooltip>
                    <Tooltip content={t('pendingQueue.actions.delete')}>
                      <IconButton
                        size="small"
                        className="bitfun-pending-queue-panel__btn bitfun-pending-queue-panel__btn--danger"
                        disabled={isSending}
                        onClick={() => handleDelete(item)}
                        aria-label={t('pendingQueue.actions.delete')}
                      >
                        <Trash2 size={14} />
                      </IconButton>
                    </Tooltip>
                  </>
                )}
              </div>
            </li>
          );
        })}
      </ul>
    </div>
  );
}

export default PendingQueuePanel;
