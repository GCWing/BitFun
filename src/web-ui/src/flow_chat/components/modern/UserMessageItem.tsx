/**
 * User message item component.
 * Renders user input messages.
 */

import React, { useState, useCallback, useRef, useEffect, useMemo } from 'react';
import { Copy, Check, RotateCcw, Loader2, ArrowDownToLine, X, CircleUser, Pencil } from 'lucide-react';
import type { DialogTurn, FlowUserSteeringItem } from '../../types/flow-chat';
import { flowChatManager } from '../../services/FlowChatManager';
import { useFlowChatContext } from './FlowChatContext';
import { useActiveSession } from '../../store/modernFlowChatStore';
import { flowChatStore } from '../../store/FlowChatStore';
import { useMessageEditStore } from '../../store/messageEditStore';
import { snapshotAPI } from '@/infrastructure/api';
import { useI18n } from '@/infrastructure/i18n';
import { notificationService } from '@/shared/notification-system';
import { globalEventBus } from '@/infrastructure/event-bus';
import { shouldIgnoreCardToggleClick } from '@/shared/utils/textSelection';
import { ReproductionStepsBlock, Tooltip, confirmDanger, ToolProcessingDots } from '@/component-library';
import { UserMessageEditComposer } from './UserMessageEditComposer';
import {
  describeUserMessageEditImpact,
  editAndRerunUserMessage,
} from '../../services/UserMessageEditService';
import { createLogger } from '@/shared/utils/logger';
import type { SessionUsageReport } from '@/infrastructure/api/service-api/SessionAPI';
import { SessionUsageReportCard } from '../usage/SessionUsageReportCard';
import type { SessionUsagePanelTab } from '../usage/sessionUsagePanelTypes';
import { coerceSessionUsageReport } from '../usage/usageReportUtils';
import { resolveSessionRelationship } from '../../utils/sessionMetadata';
import './UserMessageItem.scss';

const log = createLogger('UserMessageItem');

interface UserMessageItemProps {
  message: DialogTurn['userMessage'];
  turnId: string;
  steeringStatus?: FlowUserSteeringItem['status'];
}

export const UserMessageItem = React.memo<UserMessageItemProps>(
  ({ message, turnId, steeringStatus }) => {
    const { t, formatDate } = useI18n('flow-chat');
    const {
      config,
      sessionId,
      activeSessionOverride,
      allowUserMessageRollback = true,
      allowUserMessageEdit = true,
    } = useFlowChatContext();
    const activeSessionFromStore = useActiveSession();
    const activeSession = activeSessionOverride ?? activeSessionFromStore;
    const [copied, setCopied] = useState(false);
    const [expanded, setExpanded] = useState(false);
    const [hasOverflow, setHasOverflow] = useState(false);
    const [isRollingBack, setIsRollingBack] = useState(false);
    const [lightboxImage, setLightboxImage] = useState<string | null>(null);
    const {
      editingTurnId,
      draft: editDraft,
      isSubmitting: isEditSubmitting,
      beginEdit,
      cancelEdit,
      setDraft: setEditDraft,
      setSubmitting: setEditSubmitting,
    } = useMessageEditStore();
    const containerRef = useRef<HTMLDivElement>(null);
    const contentRef = useRef<HTMLDivElement>(null);
    const messageContent = typeof message?.content === 'string' ? message.content : String(message?.content || '');
    const messageImages = useMemo(() => message?.images ?? [], [message?.images]);
    const isUsageReportMessage = message?.metadata?.localCommandKind === 'usage_report';
    const isGoalLoadingMessage = Boolean(message?.metadata?.threadGoalKickoff);
    const isThreadGoalContinuationCheck = Boolean(message?.metadata?.threadGoalContinuation);
    const isThreadGoalSystemMessage = Boolean(
      message?.metadata?.threadGoalKickoff
      || message?.metadata?.threadGoalObjectiveUpdated
      || message?.metadata?.threadGoalContinuation
    );
    const isUsageReportLoading = message?.metadata?.usageReportStatus === 'loading';
    const usageReport = coerceSessionUsageReport(message?.metadata?.usageReport);
    const sessionRelationship = useMemo(
      () => resolveSessionRelationship(activeSession),
      [activeSession]
    );
    const canShowRollbackAction = allowUserMessageRollback && !sessionRelationship.isSubagent;

    const currentSession = activeSessionOverride
      ?? (sessionId ? flowChatStore.getState().sessions.get(sessionId) ?? null : null)
      ?? activeSessionFromStore;
    const turnIndex = currentSession?.dialogTurns.findIndex(t => t.id === turnId) ?? -1;
    const dialogTurn = turnIndex >= 0 ? currentSession?.dialogTurns[turnIndex] : null;
    const isFailed = dialogTurn?.status === 'error';
    const isEditing = editingTurnId === turnId;
    const resolvedSessionId = sessionId ?? currentSession?.sessionId;
    const historyActionsBlockedByPartialRestore = currentSession?.isPartial === true;
    const isSystemTriggered = Boolean(
      message?.metadata?.triggerSource && message.metadata.triggerSource !== 'desktop_ui',
    );
    const canRollback =
      !steeringStatus &&
      canShowRollbackAction &&
      !!resolvedSessionId &&
      turnIndex >= 0 &&
      !historyActionsBlockedByPartialRestore &&
      !isRollingBack &&
      !isEditSubmitting;
    const canEditBase =
      allowUserMessageEdit &&
      !!resolvedSessionId &&
      turnIndex >= 0 &&
      !historyActionsBlockedByPartialRestore &&
      !isThreadGoalSystemMessage &&
      !isSystemTriggered &&
      !steeringStatus;
    const canEdit = canEditBase && !isEditSubmitting && !isRollingBack;
    const canShowEditAction = allowUserMessageEdit && !isFailed && !isThreadGoalSystemMessage;
    const editDisabledReason = isSystemTriggered
      ? t('message.cannotEdit')
      : steeringStatus
        ? t('message.cannotEdit')
        : historyActionsBlockedByPartialRestore
          ? t('message.editDisabledHistoryNotReady')
        : !resolvedSessionId || turnIndex < 0
          ? t('message.editDisabledHistoryNotReady')
          : t('message.cannotEdit');
    const steeringTag = steeringStatus === 'pending'
      ? {
          className: 'user-message-item__steering-tag--pending',
          label: t('steering.statusPending'),
        }
      : null;

    const { displayText, reproductionSteps } = useMemo(() => {
      const reproductionRegex = /<reproduction_steps>([\s\S]*?)<\/reproduction_steps\s*>?/g;
      const reproductionMatch = reproductionRegex.exec(messageContent);
      const reproduction = reproductionMatch ? reproductionMatch[1].trim() : null;

      let cleaned = messageContent.replace(reproductionRegex, '').trim();
      if (isThreadGoalContinuationCheck) {
        cleaned = cleaned.replace(/\s*\n+\s*/g, ' ').trim();
      }

      // Strip [Image: ...] context lines when images are shown as thumbnails.
      if (messageImages.length > 0) {
        cleaned = cleaned
          .replace(/\[Image:.*?\]\n(?:Path:.*?\n|Image ID:.*?\n)?/g, '')
          .trim();
      }

      return { displayText: cleaned, reproductionSteps: reproduction };
    }, [isThreadGoalContinuationCheck, messageContent, messageImages]);
    
    // Check whether content overflows.
    useEffect(() => {
      const checkOverflow = () => {
        if (contentRef.current && !expanded) {
          const element = contentRef.current;
          // Detect truncated text.
          const isOverflowing = element.scrollHeight > element.clientHeight || 
                                element.scrollWidth > element.clientWidth;
          setHasOverflow(isOverflowing);
        } else {
          setHasOverflow(false);
        }
      };
      
      checkOverflow();
      
      window.addEventListener('resize', checkOverflow);
      
      return () => {
        window.removeEventListener('resize', checkOverflow);
      };
    }, [displayText, expanded]);
    
    // Copy the user message.
    const handleCopy = useCallback(async (e: React.MouseEvent) => {
      e.stopPropagation(); // Prevent toggle via bubbling.
      try {
        await navigator.clipboard.writeText(messageContent);
        setCopied(true);
        setTimeout(() => setCopied(false), 2000);
      } catch (error) {
        log.error('Failed to copy', error);
      }
    }, [messageContent]);

    const handleRollback = useCallback(async (e: React.MouseEvent) => {
      e.stopPropagation();
      if (!canRollback || !resolvedSessionId) return;

      const index = turnIndex + 1;
      const confirmed = await confirmDanger(
        t('message.rollbackDialogTitle', { index }),
        (
          <>
            <p className="confirm-dialog__message-intro">{t('message.rollbackDialogIntro')}</p>
            <ul className="confirm-dialog__bullet-list">
              <li>{t('message.rollbackDialogBulletFiles')}</li>
              <li>{t('message.rollbackDialogBulletHistory')}</li>
            </ul>
          </>
        )
      );
      if (!confirmed) return;

      setIsRollingBack(true);
      try {
        const restoredFiles = await snapshotAPI.rollbackToTurn(resolvedSessionId, turnIndex, true);

        // 1) Truncate local dialog turns from this index.
        flowChatStore.truncateDialogTurnsFrom(resolvedSessionId, turnIndex);

        // 2) Refresh file tree and open editors.
        const { globalEventBus } = await import('@/infrastructure/event-bus');
        globalEventBus.emit('file-tree:refresh');
        restoredFiles.forEach(filePath => {
          globalEventBus.emit('editor:file-changed', { filePath });
        });

        // 3) Restore the original user input back into the chat input box.
        //    Rollback is an explicit user action — always fill to avoid the
        //    content silently disappearing when the input already has text.
        if (messageContent.trim().length > 0) {
          globalEventBus.emit('fill-chat-input', {
            content: messageContent,
          });
        }

        notificationService.success(t('message.rollbackSuccess'));
      } catch (error) {
        log.error('Rollback failed', error);
        notificationService.error(`${t('message.rollbackFailed')}: ${error instanceof Error ? error.message : String(error)}`);
      } finally {
        setIsRollingBack(false);
      }
    }, [canRollback, resolvedSessionId, t, turnIndex, messageContent]);

    const handleBeginEdit = useCallback((e: React.MouseEvent) => {
      e.stopPropagation();
      if (!canEdit) return;
      beginEdit(turnId, messageContent);
    }, [beginEdit, canEdit, messageContent, turnId]);

    const handleSubmitEdit = useCallback(async () => {
      if (!resolvedSessionId || turnIndex < 0 || isEditSubmitting) return;

      const editedContent = editDraft.trim();
      if (!editedContent || editedContent === messageContent.trim()) {
        cancelEdit();
        return;
      }

      const impact = describeUserMessageEditImpact(resolvedSessionId);
      const confirmed = await confirmDanger(
        t('message.editDialogTitle', { index: turnIndex + 1 }),
        (
          <>
            <p className="confirm-dialog__message-intro">{t('message.editDialogIntro')}</p>
            <ul className="confirm-dialog__bullet-list">
              {impact.willStopRunningTask && <li>{t('message.editDialogBulletStopRunning')}</li>}
              {impact.willRestoreFiles && <li>{t('message.editDialogBulletFiles')}</li>}
              {impact.willDeleteTurns && <li>{t('message.editDialogBulletHistory')}</li>}
              {impact.willRerun && <li>{t('message.editDialogBulletRerun')}</li>}
            </ul>
          </>
        )
      );
      if (!confirmed) return;

      setEditSubmitting(true);
      try {
        await editAndRerunUserMessage({
          sessionId: resolvedSessionId,
          turnId,
          turnIndex,
          originalContent: messageContent,
          editedContent,
          agentType: currentSession?.mode,
          rerun: (content, agentType) => flowChatManager.sendMessage(
            content,
            resolvedSessionId,
            undefined,
            agentType,
          ),
        });
        cancelEdit();
        notificationService.success(t('message.editSuccess'));
      } catch (error) {
        log.error('Message edit failed', { sessionId: resolvedSessionId, turnId, error });
        notificationService.error(`${t('message.editFailed')}: ${error instanceof Error ? error.message : String(error)}`);
      } finally {
        setEditSubmitting(false);
      }
    }, [
      cancelEdit,
      currentSession?.mode,
      editDraft,
      isEditSubmitting,
      messageContent,
      resolvedSessionId,
      setEditSubmitting,
      t,
      turnId,
      turnIndex,
    ]);
    
    // Toggle expanded state.
    const handleToggleExpand = useCallback((event: React.MouseEvent<HTMLDivElement>) => {
      if (shouldIgnoreCardToggleClick(event, contentRef.current)) {
        return;
      }

      // Only allow expand/collapse when there is overflow.
      if (!hasOverflow && !expanded) {
        return;
      }
      setExpanded(prev => !prev);
    }, [hasOverflow, expanded]);
    
    // Fill content into the input (failed state only).
    const handleFillToInput = useCallback((e: React.MouseEvent) => {
      e.stopPropagation();
      globalEventBus.emit('fill-chat-input', {
        content: messageContent
      });
    }, [messageContent]);

    const handleOpenUsageReport = useCallback((report: SessionUsageReport, initialTab?: SessionUsagePanelTab) => {
      void import('../../services/openSessionUsageReport').then(({ openSessionUsagePanel }) => {
        openSessionUsagePanel({
          report,
          markdown: messageContent,
          sessionId: currentSession?.sessionId ?? resolvedSessionId,
          workspacePath: currentSession?.workspacePath,
          initialTab,
          title: t('usage.title'),
          expand: true,
        });
      });
    }, [currentSession?.sessionId, currentSession?.workspacePath, messageContent, resolvedSessionId, t]);
    
    // Collapse when clicking outside.
    useEffect(() => {
      if (!expanded) return;
      
      const handleClickOutside = (e: MouseEvent) => {
        if (containerRef.current && !containerRef.current.contains(e.target as Node)) {
          setExpanded(false);
        }
      };
      
      document.addEventListener('mousedown', handleClickOutside);
      return () => {
        document.removeEventListener('mousedown', handleClickOutside);
      };
    }, [expanded]);

    // Avoid zero-size errors by rendering a placeholder instead of null.
    if (!message) {
      return <div style={{ minHeight: '1px' }} />;
    }

    if (isUsageReportMessage) {
      return (
        <SessionUsageReportCard
          report={usageReport}
          markdown={messageContent}
          generatedAt={message.metadata?.generatedAt}
          isLoading={isUsageReportLoading}
          onOpenDetails={usageReport ? handleOpenUsageReport : undefined}
        />
      );
    }

    if (isGoalLoadingMessage) {
      return (
        <div className="session-usage-report-card session-usage-report-card--loading" aria-live="polite">
          <div className="session-usage-report-card__loading-main">
            <ToolProcessingDots className="session-usage-report-card__loading-dots" size={12} />
            <div>
              <h3 className="session-usage-report-card__loading-title">{messageContent}</h3>
            </div>
          </div>
        </div>
      );
    }
    
    return (
      <div 
        ref={containerRef}
        className={`user-message-item ${expanded ? 'user-message-item--expanded' : ''}${isFailed ? ' user-message-item--failed' : ''}`}
      >
        {config?.showTimestamps && (
          <div className="user-message-item__timestamp">
            {formatDate(new Date(message.timestamp), {
              hour: '2-digit',
              minute: '2-digit',
            })}
          </div>
        )}
        {isEditing ? (
          <UserMessageEditComposer
            value={editDraft}
            isSubmitting={isEditSubmitting}
            submitLabel={t('message.saveEdit')}
            cancelLabel={t('message.cancelEdit')}
            placeholder={t('message.editPlaceholder')}
            onChange={setEditDraft}
            onSubmit={handleSubmitEdit}
            onCancel={cancelEdit}
          />
        ) : (
          <div className="user-message-item__main">
            {isFailed && (
            <span className="user-message-item__failed-avatar" aria-hidden>
              <CircleUser size={18} strokeWidth={1.75} />
            </span>
          )}
          <div
            className={
              isFailed
                ? 'user-message-item__failed-inline-cluster'
                : 'user-message-item__main-contents-bridge'
            }
          >
            {isFailed ? (
              <div className="user-message-item__failed-body">
                <div 
                  ref={contentRef}
                  className="user-message-item__content"
                  onClick={handleToggleExpand}
                  title={(hasOverflow || expanded) ? (expanded ? t('message.clickToCollapse') : t('message.clickToExpand')) : undefined}
                  style={{
                    cursor: (hasOverflow || expanded) ? 'pointer' : 'text',
                  }}
                >
                  {displayText}
                </div>
                {steeringTag && (
                  <div className={`user-message-item__steering-tag ${steeringTag.className}`}>
                    {steeringTag.label}
                  </div>
                )}
              </div>
            ) : (
              <>
                <div 
                  ref={contentRef}
                  className="user-message-item__content"
                  onClick={handleToggleExpand}
                  title={(hasOverflow || expanded) ? (expanded ? t('message.clickToCollapse') : t('message.clickToExpand')) : undefined}
                  style={{
                    cursor: (hasOverflow || expanded) ? 'pointer' : 'text',
                  }}
                >
                  {displayText}
                </div>
                {steeringTag && (
                  <div className={`user-message-item__steering-tag ${steeringTag.className}`}>
                    {steeringTag.label}
                  </div>
                )}
              </>
            )}
            <div className="user-message-item__actions">
              <button
                className={`user-message-item__copy-btn ${copied ? 'copied' : ''}`}
                onClick={handleCopy}
                title={copied ? t('message.copyFailed') : t('message.copy')}
              >
                {copied ? <Check size={14} /> : <Copy size={14} />}
              </button>
              {canShowEditAction && (
                <Tooltip content={canEdit ? t('message.edit') : editDisabledReason}>
                  <button
                    type="button"
                    className="user-message-item__edit-btn"
                    onClick={handleBeginEdit}
                    disabled={!canEdit}
                    title={canEdit ? t('message.edit') : editDisabledReason}
                  >
                    <Pencil size={14} />
                  </button>
                </Tooltip>
              )}
              {isFailed ? (
                <Tooltip content={t('message.fillToInput')}>
                  <button
                    className="user-message-item__copy-btn"
                    onClick={handleFillToInput}
                  >
                    <ArrowDownToLine size={14} />
                  </button>
                </Tooltip>
              ) : canShowRollbackAction && !steeringStatus ? (
                <Tooltip content={canRollback ? t('message.rollbackTo', { index: turnIndex + 1 }) : t('message.cannotRollback')}>
                  <button
                    className="user-message-item__rollback-btn"
                    onClick={handleRollback}
                    disabled={!canRollback}
                  >
                    {isRollingBack ? (
                      <Loader2 size={14} className="user-message-item__rollback-spinner" />
                    ) : (
                      <RotateCcw size={14} />
                    )}
                  </button>
                </Tooltip>
              ) : null}
            </div>
            </div>
          </div>
        )}

        {message.images && message.images.length > 0 && (
          <div className="user-message-item__images">
            {message.images.map(img => {
              const src = img.dataUrl || (img.imagePath ? `https://asset.localhost/${encodeURIComponent(img.imagePath)}` : undefined);
              return src ? (
                <div key={img.id} className="user-message-item__image-thumb" onClick={(e) => { e.stopPropagation(); setLightboxImage(src); }}>
                  <img src={src} alt={img.name} />
                </div>
              ) : null;
            })}
          </div>
        )}

        {reproductionSteps && (
          <div className="user-message-item__blocks">
            {reproductionSteps && <ReproductionStepsBlock steps={reproductionSteps} />}
          </div>
        )}

        {lightboxImage && (
          <div className="user-message-item__lightbox" onClick={() => setLightboxImage(null)}>
            <button className="user-message-item__lightbox-close" onClick={() => setLightboxImage(null)}>
              <X size={20} />
            </button>
            <img src={lightboxImage} alt="Preview" onClick={(e) => e.stopPropagation()} />
          </div>
        )}
      </div>
    );
  }
);

UserMessageItem.displayName = 'UserMessageItem';
