/**
 * User message item component.
 * Renders user input messages.
 */

import React, { useState, useCallback, useRef, useEffect, useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { Copy, Check, RotateCcw, Loader2, ArrowDownToLine, X } from 'lucide-react';
import type { DialogTurn } from '../../types/flow-chat';
import { useFlowChatContext } from './FlowChatContext';
import { useActiveSession } from '../../store/modernFlowChatStore';
import { flowChatStore } from '../../store/FlowChatStore';
import { snapshotAPI } from '@/infrastructure/api';
import { notificationService } from '@/shared/notification-system';
import { globalEventBus } from '@/infrastructure/event-bus';
import { ReproductionStepsBlock, Tooltip, confirmDanger } from '@/component-library';
import { createLogger } from '@/shared/utils/logger';
import './UserMessageItem.scss';

const log = createLogger('UserMessageItem');

interface UserMessageItemProps {
  message: DialogTurn['userMessage'];
  turnId: string;
}

export const UserMessageItem = React.memo<UserMessageItemProps>(
  ({ message, turnId }) => {
    const { t } = useTranslation('flow-chat');
    const {
      config,
      sessionId,
      activeSessionOverride,
      allowUserMessageRollback = true,
    } = useFlowChatContext();
    const activeSessionFromStore = useActiveSession();
    const activeSession = activeSessionOverride ?? activeSessionFromStore;
    const [copied, setCopied] = useState(false);
    const [expanded, setExpanded] = useState(false);
    const [hasOverflow, setHasOverflow] = useState(false);
    const [isRollingBack, setIsRollingBack] = useState(false);
    const [lightboxImage, setLightboxImage] = useState<string | null>(null);
    const containerRef = useRef<HTMLDivElement>(null);
    const contentRef = useRef<HTMLDivElement>(null);
    const messageContent = typeof message?.content === 'string' ? message.content : String(message?.content || '');
    const messageImages = useMemo(() => message?.images ?? [], [message?.images]);

    const turnIndex = activeSession?.dialogTurns.findIndex(t => t.id === turnId) ?? -1;
    const dialogTurn = turnIndex >= 0 ? activeSession?.dialogTurns[turnIndex] : null;
    const isFailed = dialogTurn?.status === 'error';
    const canRollback = allowUserMessageRollback && !!sessionId && turnIndex >= 0 && !isRollingBack;

    const { displayText, reproductionSteps } = useMemo(() => {
      const reproductionRegex = /<reproduction_steps>([\s\S]*?)<\/reproduction_steps\s*>?/g;
      const reproductionMatch = reproductionRegex.exec(messageContent);
      const reproduction = reproductionMatch ? reproductionMatch[1].trim() : null;

      let cleaned = messageContent.replace(reproductionRegex, '').trim();

      // Strip [Image: ...] context lines when images are shown as thumbnails.
      if (messageImages.length > 0) {
        cleaned = cleaned
          .replace(/\[Image:.*?\]\n(?:Path:.*?\n|Image ID:.*?\n)?/g, '')
          .trim();
      }

      return { displayText: cleaned, reproductionSteps: reproduction };
    }, [messageContent, messageImages]);
    
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
      if (!canRollback || !sessionId) return;

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
        const restoredFiles = await snapshotAPI.rollbackToTurn(sessionId, turnIndex, true);

        // 1) Truncate local dialog turns from this index.
        flowChatStore.truncateDialogTurnsFrom(sessionId, turnIndex);

        // 2) Refresh file tree and open editors.
        const { globalEventBus } = await import('@/infrastructure/event-bus');
        globalEventBus.emit('file-tree:refresh');
        restoredFiles.forEach(filePath => {
          globalEventBus.emit('editor:file-changed', { filePath });
        });

        // 3) Restore the original user input back into the chat input box,
        //    but only when the input is empty to avoid clobbering pending edits.
        if (messageContent.trim().length > 0) {
          globalEventBus.emit('fill-chat-input', {
            content: messageContent,
            onlyIfEmpty: true,
          });
        }

        notificationService.success(t('message.rollbackSuccess'));
      } catch (error) {
        log.error('Rollback failed', error);
        notificationService.error(`${t('message.rollbackFailed')}: ${error instanceof Error ? error.message : String(error)}`);
      } finally {
        setIsRollingBack(false);
      }
    }, [canRollback, sessionId, t, turnIndex, messageContent]);
    
    // Toggle expanded state.
    const handleToggleExpand = useCallback(() => {
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
    
    return (
      <div 
        ref={containerRef}
        className={`user-message-item ${expanded ? 'user-message-item--expanded' : ''}${isFailed ? ' user-message-item--failed' : ''}`}
      >
        {config?.showTimestamps && (
          <div className="user-message-item__timestamp">
            {new Date(message.timestamp).toLocaleTimeString()}
          </div>
        )}
        <div className="user-message-item__main">
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
          <div className="user-message-item__actions">
            <button
              className={`user-message-item__copy-btn ${copied ? 'copied' : ''}`}
              onClick={handleCopy}
              title={copied ? t('message.copyFailed') : t('message.copy')}
            >
              {copied ? <Check size={14} /> : <Copy size={14} />}
            </button>
            {isFailed ? (
              <Tooltip content={t('message.fillToInput')}>
                <button
                  className="user-message-item__copy-btn"
                  onClick={handleFillToInput}
                >
                  <ArrowDownToLine size={14} />
                </button>
              </Tooltip>
            ) : allowUserMessageRollback ? (
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
