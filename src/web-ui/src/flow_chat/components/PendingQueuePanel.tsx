/**
 * Pending queue panel
 *
 * Renders the per-session list of "queued" user messages above the chat input.
 * Each card supports inline edit, optional "send now" (mid-turn steering), and delete.
 *
 * UX notes:
 * - Click anywhere on the preview text to start editing.
 * - Cmd/Ctrl+Enter saves the edit; Esc cancels.
 * - Clicking "send now" eagerly inserts a steering message into the live round
 *   so the user sees feedback instantly; the backend confirmation event is
 *   deduped via `steeringId`.
 */

import { useCallback, useEffect, useMemo, useRef, useState, useSyncExternalStore } from 'react';
import { useTranslation } from 'react-i18next';
import {
  Pencil,
  ArrowUp,
  Trash2,
  Check,
  X as XIcon,
  Inbox,
  Loader2,
} from 'lucide-react';
import { Tooltip, IconButton } from '@/component-library';
import { agentAPI } from '@/infrastructure/api/service-api/AgentAPI';
import { stateMachineManager } from '../state-machine';
import { FlowChatStore } from '../store/FlowChatStore';
import { pendingQueueManager } from '../services/flow-chat-manager/PendingQueueModule';
import { FlowChatManager } from '../services/FlowChatManager';
import { interruptedTurnRecoveryGate } from '../services/interruptedTurnRecoveryGate';
import { insertSteeringItemIfAbsent } from '../services/flow-chat-manager/EventHandlerModule';
import { notificationService } from '../../shared/notification-system';
import { createLogger } from '@/shared/utils/logger';
import type { QueuedMessage, SteeringImage } from '../types/flow-chat';
import { isAcpFlowSession } from '../utils/acpSession';
import './PendingQueuePanel.scss';

const log = createLogger('PendingQueuePanel');

interface PendingQueuePanelProps {
  sessionId: string | undefined;
  className?: string;
}

export function PendingQueuePanel({ sessionId, className }: PendingQueuePanelProps): JSX.Element | null {
  const { t } = useTranslation('flow-chat');
  const sendNowInFlightIdsRef = useRef(new Set<string>());
  useSyncExternalStore(
    interruptedTurnRecoveryGate.subscribe,
    interruptedTurnRecoveryGate.getSnapshot,
    interruptedTurnRecoveryGate.getSnapshot,
  );
  const recoveryInFlight = interruptedTurnRecoveryGate.isSessionInFlight(sessionId);
  const [items, setItems] = useState<QueuedMessage[]>(() =>
    sessionId ? pendingQueueManager.list(sessionId) : [],
  );
  const [isAcpSession, setIsAcpSession] = useState<boolean>(() => {
    if (!sessionId) return false;
    return isAcpFlowSession(FlowChatStore.getInstance().getState().sessions.get(sessionId));
  });
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

  useEffect(() => {
    if (!sessionId) {
      setIsAcpSession(false);
      return;
    }

    const store = FlowChatStore.getInstance();
    const sync = () => {
      setIsAcpSession(isAcpFlowSession(store.getState().sessions.get(sessionId)));
    };

    sync();
    const unsubscribe = store.subscribe(sync);
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
      if (!sessionId || recoveryInFlight) return;
      if (sendNowInFlightIdsRef.current.has(item.id)) return;
      sendNowInFlightIdsRef.current.add(item.id);

      try {
        const machine = stateMachineManager.get(sessionId);
        const dialogTurnId = machine?.getContext().currentDialogTurnId ?? null;

        // A running turn takes the message through the steering channel, which
        // carries the whole payload — text, attachments and metadata alike.
        // ACP agents own their execution loop and expose no mid-turn injection
        // point, so they take the drain path below instead.
        if (dialogTurnId && !isAcpSession) {
          pendingQueueManager.setStatus(sessionId, item.id, 'sending_now');
          try {
            const resp = await agentAPI.steerDialogTurn({
              sessionId,
              dialogTurnId,
              content: item.content,
              displayContent: item.displayMessage ?? item.content,
              imageContexts: item.imageContexts,
              userMessageMetadata: item.userMessageMetadata,
            });
            // Optimistically render the steering bubble in the running round so
            // the user sees their message land immediately. The backend
            // `UserSteeringInjected` event dedupes by the same `steeringId`.
            if (resp?.steeringId) {
              try {
                insertSteeringItemIfAbsent({
                  sessionId,
                  turnId: dialogTurnId,
                  steeringId: resp.steeringId,
                  content: item.displayMessage ?? item.content,
                  images: item.imageDisplayData as SteeringImage[] | undefined,
                  status: 'pending',
                });
              } catch (renderErr) {
                log.warn('Optimistic steering render failed', { renderErr });
              }
            }
            pendingQueueManager.remove(sessionId, item.id);
            return;
          } catch (err) {
            // Most often the turn finished between the click and the request.
            // Fall through to the drain path rather than reporting a failure the
            // user cannot act on.
            log.warn('Steering rejected, falling back to the drain path', {
              sessionId,
              itemId: item.id,
              err,
            });
            pendingQueueManager.setStatus(sessionId, item.id, 'queued');
          }
        }

        // No turn to inject into. Move the item to the head and send it at the
        // first opportunity: right now if the session is idle, otherwise the
        // IDLE drain listener picks it up the moment the current turn ends.
        try {
          if (!pendingQueueManager.promoteForExplicitDrain(sessionId, item.id)) {
            log.warn('Send now item is no longer queued', { sessionId, itemId: item.id });
            return;
          }
          await FlowChatManager.getInstance().drainPendingQueueForSession(sessionId, {
            allowInterruptedRecoveryAbandon: true,
          });
        } catch (err) {
          log.error('Send now fallback failed', { sessionId, itemId: item.id, err });
          notificationService.error(t('pendingQueue.errors.sendNowFailed'), { duration: 4000 });
        }
      } finally {
        sendNowInFlightIdsRef.current.delete(item.id);
      }
    },
    [isAcpSession, recoveryInFlight, sessionId, t],
  );

  const visibleItems = useMemo(() => items, [items]);

  if (!sessionId || visibleItems.length === 0) {
    return null;
  }

  return (
    <div data-bf-component="pending-queue-panel" data-bf-part="root"
      className={`bitfun-pending-queue-panel ${className ?? ''}`.trim()}
      data-testid="pending-queue-panel"
      onClick={e => {
        e.stopPropagation();
      }}
    >
      <div data-bf-component="pending-queue-panel" data-bf-part="header" className="bitfun-pending-queue-panel__header">
        <Inbox size={10} className="bitfun-pending-queue-panel__header-icon" />
        <span data-bf-component="pending-queue-panel" data-bf-part="title" className="bitfun-pending-queue-panel__title">
          {t('pendingQueue.title', { count: visibleItems.length })}
          <span className="bitfun-pending-queue-panel__hint">
            {' · '}
            {t('pendingQueue.hint')}
          </span>
        </span>
      </div>
      <ul data-bf-component="pending-queue-panel" data-bf-part="list" className="bitfun-pending-queue-panel__list">
        {visibleItems.map(item => {
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
            <li data-bf-component="pending-queue-panel" data-bf-part="item" data-bf-state={[
              isEditing && 'editing',
              isSending && 'sending',
              isFailed && 'failed',
            ].filter(Boolean).join(' ') || undefined} key={item.id} className={itemClass}>
              <div data-bf-component="pending-queue-panel" data-bf-part="content" className="bitfun-pending-queue-panel__content">
                {isEditing ? (
                  <>
                    <textarea
                      data-bf-component="pending-queue-panel"
                      data-bf-part="editor"
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
                      data-bf-component="pending-queue-panel"
                      data-bf-part="preview"
                      className="bitfun-pending-queue-panel__preview"
                      title={previewText}
                    >
                      {previewText || (
                        <span className="bitfun-pending-queue-panel__preview-empty">
                          {t('pendingQueue.emptyPlaceholder')}
                        </span>
                      )}
                    </div>
                    <div data-bf-component="pending-queue-panel" data-bf-part="status" className="bitfun-pending-queue-panel__sending-label">
                      <Loader2 size={11} />
                      {t('pendingQueue.statusSending')}
                    </div>
                  </>
                ) : (
                  <>
                    <div
                      data-bf-component="pending-queue-panel"
                      data-bf-part="preview"
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
                      <div data-bf-component="pending-queue-panel" data-bf-part="status" className="bitfun-pending-queue-panel__failed-label">
                        {t('pendingQueue.statusFailed')}
                      </div>
                    )}
                  </>
                )}
              </div>
              <div data-bf-component="pending-queue-panel" data-bf-part="actions" className="bitfun-pending-queue-panel__actions">
                {isEditing ? (
                  <>
                    <Tooltip content={t('pendingQueue.actions.saveEdit')}>
                      <IconButton
                        data-bf-component="pending-queue-panel"
                        data-bf-part="action"
                        size="small"
                        className="bitfun-pending-queue-panel__btn bitfun-pending-queue-panel__btn--primary"
                        onClick={() => handleEditSave(item)}
                        aria-label={t('pendingQueue.actions.saveEdit')}
                      >
                        <Check size={12} strokeWidth={2.25} />
                      </IconButton>
                    </Tooltip>
                    <Tooltip content={t('pendingQueue.actions.cancelEdit')}>
                      <IconButton
                        data-bf-component="pending-queue-panel"
                        data-bf-part="action"
                        size="small"
                        className="bitfun-pending-queue-panel__btn"
                        onClick={handleEditCancel}
                        aria-label={t('pendingQueue.actions.cancelEdit')}
                      >
                        <XIcon size={12} strokeWidth={2.25} />
                      </IconButton>
                    </Tooltip>
                  </>
                ) : (
                  <>
                    <Tooltip content={t('pendingQueue.actions.edit')}>
                      <IconButton
                        data-bf-component="pending-queue-panel"
                        data-bf-part="action"
                        size="small"
                        className="bitfun-pending-queue-panel__btn"
                        disabled={isSending}
                        onClick={() => handleEditStart(item)}
                        aria-label={t('pendingQueue.actions.edit')}
                      >
                        <Pencil size={12} strokeWidth={2.25} />
                      </IconButton>
                    </Tooltip>
                    <Tooltip content={t('pendingQueue.tooltip.sendNow')}>
                      <IconButton
                        data-bf-component="pending-queue-panel"
                        data-bf-part="action"
                        size="small"
                        className="bitfun-pending-queue-panel__btn bitfun-pending-queue-panel__btn--primary"
                        disabled={isSending || recoveryInFlight}
                        onClick={() => {
                          void handleSendNow(item);
                        }}
                        aria-label={t('pendingQueue.actions.sendNow')}
                      >
                        {isSendingNow ? (
                          <Loader2 size={12} strokeWidth={2.5} className="bitfun-pending-queue-panel__spin" />
                        ) : (
                          <ArrowUp size={12} strokeWidth={2.5} />
                        )}
                      </IconButton>
                    </Tooltip>
                    <Tooltip content={t('pendingQueue.actions.delete')}>
                      <IconButton
                        data-bf-component="pending-queue-panel"
                        data-bf-part="action"
                        size="small"
                        className="bitfun-pending-queue-panel__btn bitfun-pending-queue-panel__btn--danger"
                        disabled={isSending}
                        onClick={() => handleDelete(item)}
                        aria-label={t('pendingQueue.actions.delete')}
                      >
                        <Trash2 size={12} strokeWidth={2.25} />
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
